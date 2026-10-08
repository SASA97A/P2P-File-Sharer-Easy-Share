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

/// Resolves a remote peer given a raw address, URL, or EasyShare QR pairing URL.
/// Probes the peer's `/api/v1/info` endpoint and returns `PeerInfo` on success.
pub async fn resolve_peer_by_address(
    client: &TransferClient,
    addr_or_url: &str,
) -> Result<PeerInfo, String> {
    let (ip, port) = parse_address_or_url(addr_or_url)?;
    let base_url = format!("http://{}:{}", ip, port);

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
        room_id: device_info.room_id,
        ip,
        port: resolved_port,
        pairing_pin: device_info.pairing_pin,
    })
}

/// Sweeps the local `/24` subnet in parallel to discover a peer with a matching pairing PIN.
///
/// Returns immediately with the matching `PeerInfo` as soon as any device responds with
/// the target PIN.
pub async fn sweep_subnet_for_pin(
    client: &TransferClient,
    local_ip: &str,
    target_pin: &str,
    default_port: u16,
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

    let mut join_set = JoinSet::new();

    for target_ip in candidate_ips {
        let sem = semaphore.clone();
        let client = client.clone();
        let pin = normalized_pin.clone();

        join_set.spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            let base_url = format!("http://{}:{}", target_ip, default_port);
            let probe_fut = client.probe_peer_url(&base_url);

            match timeout(probe_timeout, probe_fut).await {
                Ok(Ok(device_info)) => {
                    let pin_matches = device_info
                        .pairing_pin
                        .as_deref()
                        .and_then(normalize_pin)
                        .map(|p| p == pin)
                        .unwrap_or(false);

                    if pin_matches {
                        let resolved_port = if device_info.port > 0 {
                            device_info.port
                        } else {
                            default_port
                        };

                        Some(PeerInfo {
                            device_name: device_info.device_name,
                            device_type: device_info.device_type,
                            os: device_info.os,
                            room_id: device_info.room_id,
                            ip: target_ip,
                            port: resolved_port,
                            pairing_pin: device_info.pairing_pin,
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        });
    }

    while let Some(res) = join_set.join_next().await {
        if let Ok(Some(peer_info)) = res {
            // Short-circuit: abort all remaining in-flight probes
            join_set.abort_all();
            return Ok(peer_info);
        }
    }

    Err(format!(
        "No device found with pairing PIN {} on local subnet",
        normalized_pin
    ))
}
