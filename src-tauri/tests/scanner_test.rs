use easy_share_lib::client::TransferClient;
use easy_share_lib::models::peer::DeviceInfo;
use easy_share_lib::pairing::scanner::{parse_address_or_url, resolve_peer_by_address, sweep_subnet_for_pin};
use easy_share_lib::server::{start_server, ServerState};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_test_scanner_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create test dir");
    dir
}

fn sample_device_info(name: &str, pin: &str) -> DeviceInfo {
    DeviceInfo::new(
        name,
        "desktop",
        "windows",
        "2.0.0",
        0,
    )
    .with_pairing_pin(pin)
}

#[test]
fn test_parse_address_or_url() {
    // Plain IP
    let (ip, port) = parse_address_or_url("192.168.1.50").expect("should parse raw IP");
    assert_eq!(ip, "192.168.1.50");
    assert_eq!(port, 5050);

    // IP with port
    let (ip, port) = parse_address_or_url("192.168.1.50:5050").expect("should parse IP:port");
    assert_eq!(ip, "192.168.1.50");
    assert_eq!(port, 5050);

    let (ip, port) = parse_address_or_url("10.0.0.1:8080").expect("should parse custom port");
    assert_eq!(ip, "10.0.0.1");
    assert_eq!(port, 8080);

    // HTTP URL
    let (ip, port) = parse_address_or_url("http://192.168.1.50:5050").expect("should parse HTTP URL");
    assert_eq!(ip, "192.168.1.50");
    assert_eq!(port, 5050);

    let (ip, port) = parse_address_or_url("http://192.168.1.50/api/v1/info").expect("should parse HTTP URL with path");
    assert_eq!(ip, "192.168.1.50");
    assert_eq!(port, 5050);

    let (ip, port) = parse_address_or_url("https://192.168.1.50:9090").expect("should parse HTTPS URL");
    assert_eq!(ip, "192.168.1.50");
    assert_eq!(port, 9090);

    // EasyShare URL
    let (ip, port) = parse_address_or_url("easyshare://pair?ip=192.168.1.50&port=5050&pin=839421&name=Alice")
        .expect("should parse EasyShare URL");
    assert_eq!(ip, "192.168.1.50");
    assert_eq!(port, 5050);

    let (ip, port) = parse_address_or_url("easyshare://pair?ip=10.0.0.2&port=7070")
        .expect("should parse EasyShare URL");
    assert_eq!(ip, "10.0.0.2");
    assert_eq!(port, 7070);

    let (ip, port) = parse_address_or_url("easyshare://pair?ip=10.0.0.3")
        .expect("should default port to 5050 in EasyShare URL");
    assert_eq!(ip, "10.0.0.3");
    assert_eq!(port, 5050);

    // IPv6 bracketed
    let (ip, port) = parse_address_or_url("[::1]:5050").expect("should parse bracketed IPv6");
    assert_eq!(ip, "::1");
    assert_eq!(port, 5050);

    // Invalid
    assert!(parse_address_or_url("").is_err());
    assert!(parse_address_or_url("   ").is_err());
}

#[tokio::test]
async fn test_resolve_peer_by_address_formats() {
    let temp_dir = setup_test_dir("resolve_peer");
    let device = sample_device_info("Resolve-Target", "123456");
    let state = Arc::new(ServerState::new(device, temp_dir.clone()));

    let (port, _server_handle) = start_server(state.clone(), 0)
        .await
        .expect("server should start");

    let client = TransferClient::new();

    // 1. IP:Port format
    let peer = resolve_peer_by_address(&client, &format!("127.0.0.1:{}", port))
        .await
        .expect("should resolve IP:port");
    assert_eq!(peer.device_name, "Resolve-Target");
    assert_eq!(peer.ip, "127.0.0.1");
    assert_eq!(peer.port, port);
    assert_eq!(peer.pairing_pin, Some("123456".to_string()));

    // 2. HTTP URL format
    let peer = resolve_peer_by_address(&client, &format!("http://127.0.0.1:{}/api/v1/info", port))
        .await
        .expect("should resolve HTTP URL");
    assert_eq!(peer.device_name, "Resolve-Target");
    assert_eq!(peer.port, port);

    // 3. EasyShare URL format
    let easy_url = format!("easyshare://pair?ip=127.0.0.1&port={}&pin=123456&name=Resolve-Target", port);
    let peer = resolve_peer_by_address(&client, &easy_url)
        .await
        .expect("should resolve EasyShare URL");
    assert_eq!(peer.device_name, "Resolve-Target");
    assert_eq!(peer.port, port);
    assert_eq!(peer.pairing_pin, Some("123456".to_string()));

    // 4. Non-existent server fails cleanly
    let fail_result = resolve_peer_by_address(&client, "127.0.0.1:1").await;
    assert!(fail_result.is_err(), "resolving closed port should fail");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_sweep_subnet_for_pin_success_and_short_circuit() {
    let temp_dir = setup_test_dir("sweep_success");
    let target_pin = "839421";
    let device = sample_device_info("Subnet-Target", target_pin);
    let state = Arc::new(ServerState::new(device, temp_dir.clone()));

    // Server listens on 127.0.0.1:{port}
    let (port, _server_handle) = start_server(state.clone(), 0)
        .await
        .expect("server should start");

    let client = TransferClient::new();

    // Local IP is 127.0.0.2, so candidate IPs will include 127.0.0.1 (where our server is listening)
    let peer = sweep_subnet_for_pin(&client, "127.0.0.2", "839 - 421", port, 50, 500)
        .await
        .expect("should discover device on subnet with formatted PIN");

    assert_eq!(peer.device_name, "Subnet-Target");
    assert_eq!(peer.ip, "127.0.0.1");
    assert_eq!(peer.port, port);
    assert_eq!(peer.pairing_pin, Some("839421".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_sweep_subnet_for_pin_rejections() {
    let temp_dir = setup_test_dir("sweep_rejections");
    let device = sample_device_info("Subnet-Target-2", "654321");
    let state = Arc::new(ServerState::new(device, temp_dir.clone()));

    let (port, _server_handle) = start_server(state.clone(), 0)
        .await
        .expect("server should start");

    let client = TransferClient::new();

    // Invalid PIN format
    let invalid_pin_err = sweep_subnet_for_pin(&client, "127.0.0.2", "invalid-pin", port, 50, 100).await;
    assert!(invalid_pin_err.is_err());
    assert!(invalid_pin_err.unwrap_err().contains("PIN"));

    // Invalid local IP
    let invalid_ip_err = sweep_subnet_for_pin(&client, "not_an_ip", "654321", port, 50, 100).await;
    assert!(invalid_ip_err.is_err());
    assert!(invalid_ip_err.unwrap_err().contains("IPv4"));

    // Non-matching PIN on subnet
    let not_found_err = sweep_subnet_for_pin(&client, "127.0.0.2", "999999", port, 50, 100).await;
    assert!(not_found_err.is_err());
    assert!(not_found_err.unwrap_err().contains("No device found"));

    let _ = fs::remove_dir_all(&temp_dir);
}
