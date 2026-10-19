use std::net::IpAddr;

/// Detects if a network adapter is virtual (e.g. Docker, WSL, VMware, VirtualBox, Tailscale, VPN).
pub fn is_virtual_adapter(name: &str) -> bool {
    let lower = name.to_lowercase();
    let virtual_patterns = [
        "docker",
        "vethernet",
        "vmnet",
        "virtual",
        "vbox",
        "tailscale",
        "zerotier",
        "wireguard",
        "wg",
        "wsl",
        "tap",
        "tun",
        "utun",
        "hyper-v",
        "loopback",
        "bridge",
        "br-",
        "dummy",
        "flannel",
        "cni",
        "vpn",
        "nordlynx",
        "proton",
        "hamachi",
        "openvpn",
        "anyconnect",
    ];

    for pattern in &virtual_patterns {
        if lower.contains(pattern) {
            return true;
        }
    }
    false
}

/// Checks if an IP address is usable for peer-to-peer LAN communication
/// (excludes loopback, APIPA link-local, unspecified, and multicast).
pub fn is_usable_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // Loopback (127.x.x.x)
            if ipv4.is_loopback() {
                return false;
            }
            // Unspecified (0.0.0.0)
            if ipv4.is_unspecified() {
                return false;
            }
            // APIPA / Link-local (169.254.x.x)
            if octets[0] == 169 && octets[1] == 254 {
                return false;
            }
            // Multicast (224.0.0.0 - 239.255.255.255)
            if ipv4.is_multicast() {
                return false;
            }
            // Broadcast (255.255.255.255)
            if ipv4.is_broadcast() {
                return false;
            }
            true
        }
        IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() || ipv6.is_unspecified() || ipv6.is_multicast() {
                return false;
            }
            // Link-local unicast fe80::/10
            let segments = ipv6.segments();
            if (segments[0] & 0xffc0) == 0xfe80 {
                return false;
            }
            true
        }
    }
}

/// Scores a network interface and IP combination to prioritize physical LAN adapters.
pub fn score_interface(name: &str, ip: &IpAddr) -> i32 {
    let mut score = 0;

    if !is_usable_ip(ip) {
        score -= 2000;
    }

    if is_virtual_adapter(name) {
        score -= 1000;
    }

    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 192.168.0.0/16
            if octets[0] == 192 && octets[1] == 168 {
                score += 100;
            // 10.0.0.0/8
            } else if octets[0] == 10 {
                score += 90;
            // 172.16.0.0/12 (172.16.0.0 to 172.31.255.255)
            } else if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                score += 80;
            } else if is_usable_ip(ip) {
                score += 50;
            }
        }
        IpAddr::V6(_) => {
            if is_usable_ip(ip) {
                score += 10;
            }
        }
    }

    let lower = name.to_lowercase();
    if lower.contains("ethernet")
        || lower.contains("eth")
        || lower.contains("en")
        || lower.contains("lan")
        || lower.contains("gigabit")
        || lower.contains("pcie")
    {
        score += 50;
    } else if lower.contains("wi-fi")
        || lower.contains("wifi")
        || lower.contains("wlan")
        || lower.contains("wl")
        || lower.contains("wireless")
    {
        score += 20;
    }

    score
}

/// Selects the best physical LAN IP from a list of interface names and IPs.
pub fn get_best_physical_ip_from_list(ifaces: &[(String, IpAddr)]) -> Option<IpAddr> {
    let mut scored: Vec<(&IpAddr, i32)> = ifaces
        .iter()
        .filter(|(name, ip)| is_usable_ip(ip) && !is_virtual_adapter(name))
        .map(|(name, ip)| (ip, score_interface(name, ip)))
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.first().map(|(ip, _)| **ip)
}

/// Discovers the best physical LAN IP on the host system.
pub fn get_best_physical_ip() -> Option<IpAddr> {
    if let Ok(netifas) = local_ip_address::list_afinet_netifas() {
        if let Some(ip) = get_best_physical_ip_from_list(&netifas) {
            return Some(ip);
        }
    }
    local_ip_address::local_ip().ok()
}

/// Returns all usable candidate physical IPs on the host system.
pub fn list_candidate_ips() -> Vec<IpAddr> {
    if let Ok(netifas) = local_ip_address::list_afinet_netifas() {
        let mut scored: Vec<(IpAddr, i32)> = netifas
            .into_iter()
            .filter(|(name, ip)| is_usable_ip(ip) && !is_virtual_adapter(name))
            .map(|(name, ip)| {
                let score = score_interface(&name, &ip);
                (ip, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.into_iter().map(|(ip, _)| ip).collect()
    } else {
        local_ip_address::local_ip().ok().into_iter().collect()
    }
}

/// Extracts deduplicated /24 candidate subnets sorted by adapter priority (Ethernet first).
pub fn get_candidate_subnets_from_list(ifaces: &[(String, IpAddr)]) -> Vec<std::net::Ipv4Addr> {
    let mut scored: Vec<(std::net::Ipv4Addr, i32)> = ifaces
        .iter()
        .filter(|(name, ip)| is_usable_ip(ip) && !is_virtual_adapter(name))
        .filter_map(|(name, ip)| match ip {
            IpAddr::V4(v4) => {
                let score = score_interface(name, ip);
                let octets = v4.octets();
                let subnet = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 0);
                Some((subnet, score))
            }
            IpAddr::V6(_) => None,
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    let mut result = Vec::new();
    for (subnet, _) in scored {
        if !result.contains(&subnet) {
            result.push(subnet);
        }
    }
    result
}

/// Returns all candidate /24 subnets on the host system, prioritized with Ethernet subnets first.
pub fn get_candidate_subnets() -> Vec<std::net::Ipv4Addr> {
    if let Ok(netifas) = local_ip_address::list_afinet_netifas() {
        let subnets = get_candidate_subnets_from_list(&netifas);
        if !subnets.is_empty() {
            return subnets;
        }
    }
    if let Ok(IpAddr::V4(v4)) = local_ip_address::local_ip() {
        let octets = v4.octets();
        vec![std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 0)]
    } else {
        vec![std::net::Ipv4Addr::new(127, 0, 0, 0)]
    }
}
