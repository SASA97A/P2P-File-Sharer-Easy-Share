use easy_share_lib::app_state::AppState;
use easy_share_lib::client::{FileToSend, TransferClient};
use easy_share_lib::commands::{
    build_pairing_payload, FileSelection, PairingInfoPayload, ProgressPayload,
};
use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
use easy_share_lib::models::transfer::{FileMetadata, TransferRequest, TransferStatus};
use easy_share_lib::server::{start_server, ConsentPolicy, ConsentRequest, ServerState};
use easy_share_lib::storage::file_manager::FileManager;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_test_ipc_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create test dir");
    dir
}

fn sample_device_info() -> DeviceInfo {
    DeviceInfo::new(
        "Test-Host",
        "desktop",
        "windows",
        "2.0.0",
        5050,
    )
}

#[tokio::test]
async fn test_app_state_creation_and_device_info() {
    let temp_dir = setup_test_dir("app_state");
    let info = sample_device_info();
    let server_state = Arc::new(ServerState::new(info.clone(), temp_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));

    let app_state = AppState::new(server_state, None, temp_dir.clone(), pending_consents);

    let dev_info = app_state.get_device_info().await;
    assert_eq!(dev_info.device_name, "Test-Host");
    assert_eq!(dev_info.os, "windows");
    assert_eq!(app_state.download_dir(), &temp_dir);
    assert!(dev_info.pairing_pin.is_some());
    let initial_pin = dev_info.pairing_pin.unwrap();
    assert_eq!(initial_pin.len(), 6);

    let new_pin = app_state.regenerate_pairing_pin().await;
    assert_eq!(new_pin.len(), 6);
    let updated_info = app_state.get_device_info().await;
    assert_eq!(updated_info.pairing_pin, Some(new_pin));
}

#[tokio::test]
async fn test_pending_consent_workflow_accept_and_decline() {
    let temp_dir = setup_test_dir("consent");
    let info = sample_device_info();
    let (tx, _rx) = mpsc::channel::<ConsentRequest>(10);
    let server_state = Arc::new(ServerState::with_consent(
        info.clone(),
        temp_dir.clone(),
        ConsentPolicy::Channel(tx),
    ));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));

    let _app_state = AppState::new(
        server_state.clone(),
        None,
        temp_dir.clone(),
        pending_consents.clone(),
    );

    // Test 1: User Accepts
    let (resp_tx1, resp_rx1) = oneshot::channel();
    let _req1 = TransferRequest::new(
        "req-test-1",
        "Sender-Alice",
        "windows",
        vec![FileMetadata::new("f1", "doc.pdf", 1024, None)],
    );

    {
        let mut map = pending_consents.write().await;
        map.insert("req-test-1".to_string(), resp_tx1);
    }

    // Direct simulation of respond_transfer_request logic
    {
        let mut map = pending_consents.write().await;
        let responder = map.remove("req-test-1").expect("should find request");
        responder.send(true).expect("send should succeed");
    }

    let decision1 = resp_rx1.await.expect("receiver should get decision");
    assert!(decision1, "decision should be true (accepted)");

    // Test 2: User Declines
    let (resp_tx2, resp_rx2) = oneshot::channel();
    {
        let mut map = pending_consents.write().await;
        map.insert("req-test-2".to_string(), resp_tx2);
    }

    {
        let mut map = pending_consents.write().await;
        let responder = map.remove("req-test-2").expect("should find request");
        responder.send(false).expect("send should succeed");
    }

    let decision2 = resp_rx2.await.expect("receiver should get decision");
    assert!(!decision2, "decision should be false (declined)");

    // Test 3: Unknown request ID
    let mut map = pending_consents.write().await;
    assert!(map.remove("non-existent-id").is_none());
}

#[tokio::test]
async fn test_manual_peer_probe_and_fallback() {
    let temp_dir = setup_test_dir("manual_peer");
    let target_info = DeviceInfo::new(
        "Target-Node",
        "desktop",
        "linux",
        "2.0.0",
        0,
    );
    let target_server_state = Arc::new(ServerState::new(target_info, temp_dir.clone()));
    let (target_port, _target_task) = start_server(target_server_state, 0)
        .await
        .expect("server must start");

    let client = TransferClient::new();

    // 1. Probe reachable target
    let url = format!("http://127.0.0.1:{}", target_port);
    let probed = client
        .probe_peer_url(&url)
        .await
        .expect("probe should succeed");
    assert_eq!(probed.device_name, "Target-Node");
    assert_eq!(probed.os, "linux");

    // 2. Probe unreachable port
    let dead_url = "http://127.0.0.1:59999";
    let dead_probe = client.probe_peer_url(dead_url).await;
    assert!(dead_probe.is_err(), "probe on dead port must fail");
}

#[tokio::test]
async fn test_file_selection_deserialization_and_validation() {
    let json_data = r#"[
        {"path": "C:\\temp\\test.txt", "name": "custom.txt"},
        {"fullPath": "C:\\temp\\image.png"},
        {"filePath": "C:\\temp\\video.mp4"}
    ]"#;

    let selections: Vec<FileSelection> =
        serde_json::from_str(json_data).expect("deserialization should succeed");
    assert_eq!(selections.len(), 3);
    assert_eq!(selections[0].path, "C:\\temp\\test.txt");
    assert_eq!(selections[0].name, Some("custom.txt".to_string()));
    assert_eq!(selections[1].path, "C:\\temp\\image.png");
    assert_eq!(selections[2].path, "C:\\temp\\video.mp4");
}

#[tokio::test]
async fn test_full_client_server_transfer_with_events_payload() {
    let receiver_dir = setup_test_dir("rx_e2e");
    let sender_dir = setup_test_dir("tx_e2e");

    // 1. Receiver Node
    let rx_info = DeviceInfo::new("Receiver-PC", "desktop", "windows", "2.0.0", 0);
    let rx_state = Arc::new(ServerState::new(rx_info, receiver_dir.clone()));
    let (rx_port, _rx_task) = start_server(rx_state.clone(), 0).await.unwrap();

    let peer = PeerInfo::new(
        "Receiver-PC",
        "desktop",
        "windows",
        "127.0.0.1",
        rx_port,
    );

    // 2. Sender prepares a 2 MB test file
    let source_file = sender_dir.join("sample_data.bin");
    let payload_bytes = vec![0xABu8; 2 * 1024 * 1024]; // 2 MB
    fs::write(&source_file, &payload_bytes).unwrap();

    let file_to_send = FileToSend::from_path("f1", &source_file).unwrap();
    let req = TransferRequest::new(
        "req-1",
        "Sender-PC",
        "windows",
        vec![file_to_send.to_metadata()],
    );

    let client = TransferClient::new();
    let handshake_resp = client.request_transfer(&peer, &req).await.unwrap();
    assert_eq!(handshake_resp.status, TransferStatus::Accepted);
    let token = handshake_resp.session_token.unwrap();

    // 3. Track progress payloads
    let progress_records = Arc::new(RwLock::new(Vec::<ProgressPayload>::new()));
    let progress_records_cb = progress_records.clone();
    let tok_cb = token.clone();

    client
        .send_file_resumable(&peer, &token, &file_to_send, move |sent, total| {
            let percent = ((sent as f64 / total as f64) * 100.0) as u32;
            let rec = ProgressPayload {
                session_token: tok_cb.clone(),
                file_id: "f1".to_string(),
                file_name: "sample_data.bin".to_string(),
                sent_bytes: sent,
                total_bytes: total,
                percent,
            };
            tokio::spawn({
                let list = progress_records_cb.clone();
                async move {
                    list.write().await.push(rec);
                }
            });
        })
        .await
        .unwrap();

    // 4. Finish
    let finish_resp = client
        .finish_transfer(&peer, &token, &file_to_send.file_id)
        .await
        .unwrap();
    assert_eq!(finish_resp.status, TransferStatus::Completed);

    let received_path = PathBuf::from(&finish_resp.saved_path);
    assert!(received_path.exists());
    assert_eq!(fs::metadata(&received_path).unwrap().len(), 2 * 1024 * 1024);

    let received_hash = FileManager::calculate_blake3_file(&received_path).unwrap();
    assert_eq!(
        received_hash,
        file_to_send.blake3_hash.as_ref().unwrap().clone()
    );
}

#[tokio::test]
async fn test_build_pairing_payload_and_regeneration() {
    let temp_dir = setup_test_dir("pairing_payload");
    let info = sample_device_info();
    let server_state = Arc::new(ServerState::new(info.clone(), temp_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));

    let app_state = AppState::new(server_state, None, temp_dir.clone(), pending_consents);

    let payload = build_pairing_payload(&app_state)
        .await
        .expect("should build pairing payload");

    assert_eq!(payload.pin.len(), 6);
    assert_eq!(
        payload.formatted_pin,
        format!("{} - {}", &payload.pin[..3], &payload.pin[3..])
    );
    assert!(payload.qr_svg.contains("<svg"));
    assert!(payload.qr_svg.contains("</svg>"));
    assert!(payload.direct_url.starts_with("easyshare://pair?"));
    assert!(payload.direct_url.contains(&format!("pin={}", payload.pin)));
    assert!(payload.direct_url.contains("name=Test-Host"));
    assert_eq!(payload.port, 5050);

    // Test JSON Serialization / Deserialization
    let json = serde_json::to_string(&payload).expect("serialization should succeed");
    let deserialized: PairingInfoPayload =
        serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(payload, deserialized);

    // Test Regeneration
    let new_pin = app_state.regenerate_pairing_pin().await;
    let payload2 = build_pairing_payload(&app_state)
        .await
        .expect("should build updated pairing payload");
    assert_eq!(payload2.pin, new_pin);
    assert_eq!(
        payload2.formatted_pin,
        format!("{} - {}", &new_pin[..3], &new_pin[3..])
    );
    assert!(payload2.direct_url.contains(&format!("pin={}", new_pin)));
}

#[tokio::test]
async fn test_connect_by_address_and_pin_matching() {
    let temp_dir = setup_test_dir("pairing_connect");
    let mut target_info = DeviceInfo::new(
        "Direct-Target",
        "desktop",
        "windows",
        "2.0.0",
        0,
    );
    target_info.pairing_pin = Some("482910".to_string());

    let target_server_state = Arc::new(ServerState::new(target_info, temp_dir.clone()));
    let (target_port, _target_task) = start_server(target_server_state, 0)
        .await
        .expect("target server must start");

    let client = TransferClient::new();

    // 1. Direct connect by raw IP:Port
    let direct_addr = format!("127.0.0.1:{}", target_port);
    let peer = easy_share_lib::pairing::resolve_peer_by_address(&client, &direct_addr)
        .await
        .expect("should resolve peer by address");
    assert_eq!(peer.device_name, "Direct-Target");
    assert_eq!(peer.port, target_port);
    assert_eq!(peer.pairing_pin, Some("482910".to_string()));

    // 2. Direct connect by EasyShare QR URL
    let qr_url = format!(
        "easyshare://pair?ip=127.0.0.1&port={}&pin=482910&name=Direct-Target",
        target_port
    );
    let peer_qr = easy_share_lib::pairing::resolve_peer_by_address(&client, &qr_url)
        .await
        .expect("should resolve peer by QR URL");
    assert_eq!(peer_qr.device_name, "Direct-Target");
    assert_eq!(peer_qr.port, target_port);
    assert_eq!(peer_qr.pairing_pin, Some("482910".to_string()));

    // 3. Connect by PIN matching against cached peer
    let cached_peers = vec![peer.clone()];
    let normalized_input =
        easy_share_lib::pairing::pin::normalize_pin("482 - 910").expect("should normalize");
    let matched = cached_peers.into_iter().find(|p| {
        p.pairing_pin
            .as_deref()
            .and_then(easy_share_lib::pairing::pin::normalize_pin)
            .map(|p_pin| p_pin == normalized_input)
            .unwrap_or(false)
    });
    assert!(matched.is_some());
    assert_eq!(matched.unwrap().device_name, "Direct-Target");
}
