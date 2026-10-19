use crate::client::TransferClient;
use crate::models::peer::PeerInfo;
use crate::pairing::pin::normalize_pin;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::timeout;
use url::Url;

/// Default port for EasyShare peer HTTP service.
pub const DEFAULT_PEER_PORT: u16 = 5050;

/// Fallback port range probed if peer cannot be contacted on `DEFAULT_PEER_PORT`.
pub const FALLBACK_PEER_PORTS: std::ops::RangeInclusive<u16> = 5051..=5055;

/// Parses an address or URL string into an IP/host and port tuple.
///
/// Supported formats:
/// - Plain IP: `192.168.1.50` -> `("192.168.1.50", 5050)`
/// - IP:Port: `192.168.1.50:5050` -> `("192.168.1.50", 5050)`
/// - Bracketed IPv6: `[::1]:5050` -> `("::1", 5050)`
/// - HTTP(S) URL: `http://192.168.1.50:5050/api/v1/info` -> `("192.168.1.50", 5050)`
/// - EasyShare URL: `easyshare://pair?ip=192.168.1.50&port=5050&pin=839421` -> `("192.168.1.50", 5050)`
pub fn parse_address_or_url(input: &str) -> Result<(String, u16), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Address or URL cannot be empty".to_string());
    }

    // EasyShare custom URI scheme
    if trimmed.starts_with("easyshare://") {
        if let Ok(parsed) = Url::parse(trimmed) {
            let mut ip_opt = None;
            let mut port_opt = None;
            for (key, val) in parsed.query_pairs() {
                if key == "ip" {
                    ip_opt = Some(val.to_string());
                } else if key == "port" {
                    if let Ok(p) = val.parse::<u16>() {
                        port_opt = Some(p);
                    }
                }
            }

            let ip = ip_opt
                .or_else(|| parsed.host_str().map(|s| s.to_string()))
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "EasyShare URL missing host or ip parameter".to_string())?;
            let port = port_opt
                .or_else(|| parsed.port())
                .unwrap_or(DEFAULT_PEER_PORT);
            return Ok((ip, port));
        }
    }

    // HTTP / HTTPS URL scheme
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Ok(parsed) = Url::parse(trimmed) {
            let host = parsed
                .host_str()
                .ok_or_else(|| "URL is missing host".to_string())?;
            let port = parsed.port().unwrap_or(DEFAULT_PEER_PORT);
            return Ok((host.to_string(), port));
        }
    }

    // Bracketed IPv6 format: [::1]:port or [::1]
    if trimmed.starts_with('[') {
        if let Some(closing_bracket) = trimmed.rfind(']') {
            let host = &trimmed[1..closing_bracket];
            let remainder = &trimmed[closing_bracket + 1..];
            if let Some(port_str) = remainder.strip_prefix(':') {
                let port = port_str
                    .parse::<u16>()
                    .map_err(|_| format!("Invalid port number: {}", port_str))?;
                return Ok((host.to_string(), port));
            } else {
                return Ok((host.to_string(), DEFAULT_PEER_PORT));
            }
        }
    }

    // IP:port or hostname:port
    if let Some((host, port_str)) = trimmed.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            if host.is_empty() {
                return Err("Host cannot be empty".to_string());
            }
            return Ok((host.to_string(), port));
        }
    }

    // Raw IP or Hostname without port
    Ok((trimmed.to_string(), DEFAULT_PEER_PORT))
}

/// Extracts an optional 6-digit pairing PIN parameter from an EasyShare or HTTP URL if present.
pub fn extract_pin_from_url(input: &str) -> Option<String> {
    if let Ok(parsed) = Url::parse(input) {
        for (key, val) in parsed.query_pairs() {
            if key == "pin" {
                return normalize_pin(&val);
            }
        }
    }
    None
}

/// Resolves a remote peer given a raw address, URL, or EasyShare QR pairing URL.
/// Probes the peer's `/api/v1/info` endpoint and returns `PeerInfo` on success.
pub async fn resolve_peer_by_address(
    client: &TransferClient,
    addr_or_url: &str,
) -> Result<PeerInfo, String> {
    let (ip, port) = parse_address_or_url(addr_or_url)?;
    let base_url = format!("http://{}:{}", ip, port);
    let embedded_pin = extract_pin_from_url(addr_or_url);

    let probe_fut = client.probe_peer_url(&base_url);
    let device_info = timeout(Duration::from_secs(5), probe_fut)
        .await
        .map_err(|_| format!("Connection timed out probing peer at {}", base_url))?
        .map_err(|e| format!("Failed to probe peer at {}: {}", base_url, e))?;

    let resolved_port = if device_info.port > 0 {
        device_info.port
    } else {
        port
    };

    Ok(PeerInfo {
        device_name: device_info.device_name,
        device_type: device_info.device_type,
        os: device_info.os,
        ip,
        port: resolved_port,
        pairing_pin: embedded_pin,
    })
}

async fn sweep_ips_on_port(
    client: &TransferClient,
    candidate_ips: &[String],
    port: u16,
    normalized_pin: &str,
    semaphore: Arc<Semaphore>,
    probe_timeout: Duration,
) -> Option<PeerInfo> {
    let mut join_set = JoinSet::new();

    for target_ip in candidate_ips {
        let sem = semaphore.clone();
        let client = client.clone();
        let pin = normalized_pin.to_string();
        let target_ip = target_ip.clone();

        join_set.spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            let base_url = format!("http://{}:{}", target_ip, port);
            let pair_fut = client.pair_peer_url(&base_url, &pin);

            match timeout(probe_timeout, pair_fut).await {
                Ok(Ok(device_info)) => {
                    let resolved_port = if device_info.port > 0 {
                        device_info.port
                    } else {
                        port
                    };

                    Some(PeerInfo {
                        device_name: device_info.device_name,
                        device_type: device_info.device_type,
                        os: device_info.os,
                        ip: target_ip,
                        port: resolved_port,
                        pairing_pin: Some(pin),
                    })
                }
                _ => None,
            }
        });
    }

    while let Some(res) = join_set.join_next().await {
        if let Ok(Some(peer_info)) = res {
            // Short-circuit: abort all remaining in-flight probes
            join_set.abort_all();
            return Some(peer_info);
        }
    }

    None
}

/// Sweeps the local `/24` subnet across an ordered list of candidate ports to find a peer with matching PIN.
pub async fn sweep_subnet_for_pin_ports(
    client: &TransferClient,
    local_ip: &str,
    target_pin: &str,
    ports: &[u16],
    concurrency_limit: usize,
    timeout_ms: u64,
) -> Result<PeerInfo, String> {
    let normalized_pin = normalize_pin(target_pin)
        .ok_or_else(|| format!("Invalid 6-digit pairing PIN: '{}'", target_pin))?;

    let parsed_ip: Ipv4Addr = local_ip
        .trim()
        .parse()
        .map_err(|_| format!("Invalid local IPv4 address: '{}'", local_ip))?;

    let octets = parsed_ip.octets();
    let prefix = format!("{}.{}.{}.", octets[0], octets[1], octets[2]);

    // Generate candidate IPs 1..=254 excluding local IP itself
    let candidate_ips: Vec<String> = (1u8..=254)
        .filter(|&last_octet| last_octet != octets[3])
        .map(|last_octet| format!("{}{}", prefix, last_octet))
        .collect();

    let concurrency = concurrency_limit.max(1);
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let probe_timeout = Duration::from_millis(timeout_ms.max(10));

    for &port in ports {
        if let Some(peer) = sweep_ips_on_port(
            client,
            &candidate_ips,
            port,
            &normalized_pin,
            semaphore.clone(),
            probe_timeout,
        )
        .await
        {
            return Ok(peer);
        }
    }

    Err(format!(
        "No device found with pairing PIN {} on local subnet",
        normalized_pin
    ))
}

/// Sweeps the local `/24` subnet in parallel to discover a peer with a matching pairing PIN.
///
/// If `default_port` matches `DEFAULT_PEER_PORT` (5050), it probes port 5050 first and
/// automatically falls back to probing ports 5051..=5055 if no matching device was found.
pub async fn sweep_subnet_for_pin(
    client: &TransferClient,
    local_ip: &str,
    target_pin: &str,
    default_port: u16,
    concurrency_limit: usize,
    timeout_ms: u64,
) -> Result<PeerInfo, String> {
    if default_port == DEFAULT_PEER_PORT {
        let mut ports = vec![DEFAULT_PEER_PORT];
        ports.extend(FALLBACK_PEER_PORTS);
        sweep_subnet_for_pin_ports(
            client,
            local_ip,
            target_pin,
            &ports,
            concurrency_limit,
            timeout_ms,
        )
        .await
    } else {
        sweep_subnet_for_pin_ports(
            client,
            local_ip,
            target_pin,
            &[default_port],
            concurrency_limit,
            timeout_ms,
        )
        .await
    }
}

/// Sweeps all candidate physical subnets (Ethernet subnets first, then Wi-Fi).
/// Short-circuits immediately upon discovering a matching peer.
pub async fn sweep_all_subnets_for_pin(
    client: &TransferClient,
    target_pin: &str,
    default_port: u16,
    concurrency_limit: usize,
    timeout_ms: u64,
) -> Result<PeerInfo, String> {
    let mut candidate_subnets = crate::discovery::nic::get_candidate_subnets();
    let loopback_subnet = std::net::Ipv4Addr::new(127, 0, 0, 0);
    if !candidate_subnets.contains(&loopback_subnet) {
        candidate_subnets.push(loopback_subnet);
    }

    let normalized = normalize_pin(target_pin)
        .ok_or_else(|| format!("Invalid 6-digit pairing PIN: '{}'", target_pin))?;

    for subnet in candidate_subnets {
        let base_ip = format!("{}.{}.{}.0", subnet.octets()[0], subnet.octets()[1], subnet.octets()[2]);
        if let Ok(peer) = sweep_subnet_for_pin(
            client,
            &base_ip,
            &normalized,
            default_port,
            concurrency_limit,
            timeout_ms,
        )
        .await
        {
            return Ok(peer);
        }
    }

    Err(format!(
        "No device found with pairing PIN {} on any active local subnet",
        normalized
    ))
}
