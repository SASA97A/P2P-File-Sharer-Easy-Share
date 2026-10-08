use easy_share_lib::discovery::nic::{
    get_best_physical_ip_from_list, is_usable_ip, is_virtual_adapter, score_interface,
};
use easy_share_lib::discovery::udp::{
    create_broadcast_socket, HeartbeatMessage, PeerTracker,
};
use easy_share_lib::discovery::{
    DiscoveryConfig, DiscoveryEvent, DiscoveryService, PeerEvent,
};
use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::time::sleep;

#[test]
fn test_nic_virtual_adapter_detection() {
    // Virtual adapters that should be detected and filtered
    assert!(is_virtual_adapter("docker0"));
    assert!(is_virtual_adapter("docker_gwbridge"));
    assert!(is_virtual_adapter("vEthernet (Default Switch)"));
    assert!(is_virtual_adapter("vEthernet (WSL)"));
    assert!(is_virtual_adapter("vmnet1"));
    assert!(is_virtual_adapter("vmnet8"));
    assert!(is_virtual_adapter("tailscale0"));
    assert!(is_virtual_adapter("Tailscale"));
    assert!(is_virtual_adapter("tun0"));
    assert!(is_virtual_adapter("tap0"));
    assert!(is_virtual_adapter("utun2"));
    assert!(is_virtual_adapter("wg0"));
    assert!(is_virtual_adapter("Hyper-V Virtual Ethernet Adapter"));
    assert!(is_virtual_adapter("VirtualBox Host-Only Ethernet Adapter"));
    assert!(is_virtual_adapter("Software Loopback Interface 1"));

    // Physical adapters that should NOT be marked as virtual
    assert!(!is_virtual_adapter("Wi-Fi"));
    assert!(!is_virtual_adapter("Ethernet"));
    assert!(!is_virtual_adapter("eth0"));
    assert!(!is_virtual_adapter("wlan0"));
    assert!(!is_virtual_adapter("en0"));
    assert!(!is_virtual_adapter("enp3s0"));
    assert!(!is_virtual_adapter("Realtek PCIe GbE Family Controller"));
    assert!(!is_virtual_adapter("Intel(R) Wi-Fi 6 AX200 160MHz"));
}

#[test]
fn test_nic_ip_usability_and_scoring() {
    let loopback = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let apipa = IpAddr::V4(Ipv4Addr::new(169, 254, 10, 20));
    let unspecified = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
    let lan_192 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50));
    let lan_10 = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 15));
    let lan_172 = IpAddr::V4(Ipv4Addr::new(172, 16, 5, 88));

    assert!(!is_usable_ip(&loopback));
    assert!(!is_usable_ip(&apipa));
    assert!(!is_usable_ip(&unspecified));
    assert!(is_usable_ip(&lan_192));
    assert!(is_usable_ip(&lan_10));
    assert!(is_usable_ip(&lan_172));

    // Scoring tests
    let physical_score = score_interface("Wi-Fi", &lan_192);
    let virtual_score = score_interface("vEthernet (WSL)", &lan_192);
    let apipa_score = score_interface("Ethernet", &apipa);
    let loopback_score = score_interface("Loopback", &loopback);

    assert!(physical_score > 0);
    assert!(physical_score > virtual_score);
    assert!(physical_score > apipa_score);
    assert!(physical_score > loopback_score);

    // Test selection from list
    let ifaces = vec![
        ("Software Loopback 1".to_string(), loopback),
        ("vEthernet (Default Switch)".to_string(), IpAddr::V4(Ipv4Addr::new(172, 28, 16, 1))),
        ("docker0".to_string(), IpAddr::V4(Ipv4Addr::new(172, 17, 0, 1))),
        ("tailscale0".to_string(), IpAddr::V4(Ipv4Addr::new(100, 64, 1, 5))),
        ("Ethernet 2 (APIPA)".to_string(), apipa),
        ("Wi-Fi".to_string(), lan_192),
    ];

    let best_ip = get_best_physical_ip_from_list(&ifaces);
    assert_eq!(best_ip, Some(lan_192));
}

#[tokio::test]
async fn test_udp_heartbeat_broadcast_and_listener() {
    let port = 41250; // Use test port to avoid conflict with standard 41234
    let addr = format!("127.0.0.1:{}", port);

    // Receiver socket
    let rx_socket = create_broadcast_socket(port).expect("failed to bind rx socket");

    // Sender socket
    let tx_socket = create_broadcast_socket(0).expect("failed to bind tx socket");

    let heartbeat = HeartbeatMessage {
        magic: "EASYSHARE_DISCOVERY".to_string(),
        device_name: "Alice-Laptop".to_string(),
        device_type: "desktop".to_string(),
        os: "windows".to_string(),
        version: "2.0.0".to_string(),
        room_id: Some("Office-Room".to_string()),
        port: 5050,
        ip: Some("127.0.0.1".to_string()),
        pairing_pin: Some("123456".to_string()),
    };

    let payload = serde_json::to_vec(&heartbeat).expect("failed to serialize heartbeat");
    tx_socket
        .send_to(&payload, &addr)
        .await
        .expect("failed to send heartbeat");

    let mut buf = [0u8; 2048];
    let (len, _src) = rx_socket.recv_from(&mut buf).await.expect("failed to recv");
    let received_heartbeat: HeartbeatMessage =
        serde_json::from_slice(&buf[..len]).expect("failed to decode heartbeat");

    assert_eq!(received_heartbeat.device_name, "Alice-Laptop");
    assert_eq!(received_heartbeat.device_type, "desktop");
    assert_eq!(received_heartbeat.os, "windows");
    assert_eq!(received_heartbeat.version, "2.0.0");
    assert_eq!(received_heartbeat.room_id, Some("Office-Room".to_string()));
    assert_eq!(received_heartbeat.port, 5050);
    assert_eq!(received_heartbeat.pairing_pin, Some("123456".to_string()));

    let peer_info = received_heartbeat.to_peer_info("127.0.0.1");
    assert_eq!(peer_info.device_name, "Alice-Laptop");
    assert_eq!(peer_info.ip, "127.0.0.1");
    assert_eq!(peer_info.port, 5050);
    assert_eq!(peer_info.pairing_pin, Some("123456".to_string()));
}

#[tokio::test]
async fn test_peer_tracker_deduplication_and_expiration() {
    let mut tracker = PeerTracker::new(Duration::from_millis(200));

    let peer_a = PeerInfo {
        device_name: "Peer-A".to_string(),
        device_type: "desktop".to_string(),
        os: "windows".to_string(),
        room_id: Some("Room-1".to_string()),
        ip: "192.168.1.10".to_string(),
        port: 5050,
        pairing_pin: None,
    };

    let peer_b = PeerInfo {
        device_name: "Peer-B".to_string(),
        device_type: "mobile".to_string(),
        os: "android".to_string(),
        room_id: None,
        ip: "192.168.1.11".to_string(),
        port: 5050,
        pairing_pin: None,
    };

    // First time seeing peer A -> emit Discovered
    let event1 = tracker.record_seen(peer_a.clone());
    assert_eq!(event1, Some(PeerEvent::Discovered(peer_a.clone())));
    assert_eq!(tracker.len(), 1);

    // Second time seeing peer A -> no Discovered event (deduplicated)
    let event2 = tracker.record_seen(peer_a.clone());
    assert_eq!(event2, None);
    assert_eq!(tracker.len(), 1);

    // First time seeing peer B -> emit Discovered
    let event3 = tracker.record_seen(peer_b.clone());
    assert_eq!(event3, Some(PeerEvent::Discovered(peer_b.clone())));
    assert_eq!(tracker.len(), 2);

    // Prune before expiration -> no Lost events
    let expired_none = tracker.prune_expired();
    assert!(expired_none.is_empty());
    assert_eq!(tracker.len(), 2);

    // Wait past expiration TTL
    sleep(Duration::from_millis(250)).await;

    // Prune expired peers -> should return Lost events for both
    let expired = tracker.prune_expired();
    assert_eq!(expired.len(), 2);
    assert_eq!(tracker.len(), 0);

    // Verify both peers received Lost events
    let lost_a = PeerEvent::Lost(peer_a);
    let lost_b = PeerEvent::Lost(peer_b);
    assert!(expired.contains(&lost_a));
    assert!(expired.contains(&lost_b));
}

#[tokio::test]
async fn test_discovery_service_udp_end_to_end() {
    let dev_a = DeviceInfo::new("Node-A", "desktop", "windows", "2.0.0", Some("Lab".into()), 5051);
    let dev_b = DeviceInfo::new("Node-B", "mobile", "android", "2.0.0", Some("Lab".into()), 5052);

    let config_a = DiscoveryConfig {
        broadcast_interval: Duration::from_millis(100),
        peer_ttl: Duration::from_millis(500),
        udp_port: 41255,
        enable_mdns: false,
        enable_udp: true,
    };

    let config_b = DiscoveryConfig {
        broadcast_interval: Duration::from_millis(100),
        peer_ttl: Duration::from_millis(500),
        udp_port: 41255,
        enable_mdns: false,
        enable_udp: true,
    };

    let (service_a, mut rx_a) = DiscoveryService::new(dev_a, config_a).await.expect("service a failed");
    let handle_a = service_a.start().await.expect("start a failed");

    let (service_b, mut rx_b) = DiscoveryService::new(dev_b, config_b).await.expect("service b failed");
    let handle_b = service_b.start().await.expect("start b failed");

    // Wait for nodes to discover each other
    let mut discovered_by_a = false;
    let mut discovered_by_b = false;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline && (!discovered_by_a || !discovered_by_b) {
        tokio::select! {
            Ok(event) = rx_a.recv() => {
                if let DiscoveryEvent::Peer(PeerEvent::Discovered(peer)) = event {
                    if peer.device_name == "Node-B" {
                        discovered_by_a = true;
                    }
                }
            }
            Ok(event) = rx_b.recv() => {
                if let DiscoveryEvent::Peer(PeerEvent::Discovered(peer)) = event {
                    if peer.device_name == "Node-A" {
                        discovered_by_b = true;
                    }
                }
            }
            _ = sleep(Duration::from_millis(50)) => {}
        }
    }

    assert!(discovered_by_a, "Node A should discover Node B");
    assert!(discovered_by_b, "Node B should discover Node A");

    // Stop node B and verify node A emits PeerLost after TTL
    handle_b.stop().await;

    let mut lost_b_by_a = false;
    let expire_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < expire_deadline && !lost_b_by_a {
        tokio::select! {
            Ok(event) = rx_a.recv() => {
                if let DiscoveryEvent::Peer(PeerEvent::Lost(peer)) = event {
                    if peer.device_name == "Node-B" {
                        lost_b_by_a = true;
                    }
                }
            }
            _ = sleep(Duration::from_millis(50)) => {}
        }
    }

    assert!(lost_b_by_a, "Node A should detect Node B was lost after expiration");

    handle_a.stop().await;
}

#[tokio::test]
async fn test_manual_peer_addition_and_query() {
    let dev = DeviceInfo::new("Self-Node", "desktop", "windows", "2.0.0", None, 5050);
    let config = DiscoveryConfig {
        broadcast_interval: Duration::from_secs(10),
        peer_ttl: Duration::from_secs(30),
        udp_port: 41258,
        enable_mdns: false,
        enable_udp: false,
    };

    let (service, mut rx) = DiscoveryService::new(dev, config).await.expect("service create");
    let handle = service.start().await.expect("service start");

    let initial_peers = handle.get_peers().await;
    assert!(initial_peers.is_empty());

    let added_peer = handle.add_manual_peer("192.168.1.99", 5050).await.expect("add peer");
    assert_eq!(added_peer.ip, "192.168.1.99");
    assert_eq!(added_peer.port, 5050);

    let event = rx.recv().await.expect("event recv");
    match event {
        DiscoveryEvent::Peer(PeerEvent::Discovered(peer)) => {
            assert_eq!(peer.ip, "192.168.1.99");
            assert_eq!(peer.port, 5050);
        }
        _ => panic!("Expected PeerEvent::Discovered"),
    }

    let current_peers = handle.get_peers().await;
    assert_eq!(current_peers.len(), 1);
    assert_eq!(current_peers[0].ip, "192.168.1.99");

    handle.stop().await;
}

#[test]
fn test_discovery_config_default_values() {
    let config = DiscoveryConfig::default();
    assert_eq!(config.broadcast_interval, Duration::from_secs(5));
    assert_eq!(config.peer_ttl, Duration::from_secs(15));
    assert_eq!(config.udp_port, 41234);
    assert!(config.enable_mdns);
    assert!(config.enable_udp);
}

