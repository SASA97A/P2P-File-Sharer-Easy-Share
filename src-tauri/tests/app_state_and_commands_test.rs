use easy_share_lib::app_state::AppState;
use easy_share_lib::client::{FileToSend, TransferClient};
use easy_share_lib::commands::{
    cancel_transfer, get_discovered_peers, get_download_dir, get_my_device_info,
    respond_transfer_request, FileSelection, ProgressPayload, TransferCompletedPayload,
    TransferErrorPayload,
};
use easy_share_lib::discovery::{DiscoveryConfig, DiscoveryService};
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
        Some("Design".to_string()),
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
    assert_eq!(dev_info.room_id, Some("Design".to_string()));
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
    let (tx, mut rx) = mpsc::channel::<ConsentRequest>(10);
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
    let req1 = TransferRequest::new(
        "req-test-1",
        "Sender-Alice",
        "windows",
        None,
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
        Some("Engineering".to_string()),
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
    assert_eq!(probed.room_id, Some("Engineering".to_string()));

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
    let rx_info = DeviceInfo::new("Receiver-PC", "desktop", "windows", "2.0.0", None, 0);
    let rx_state = Arc::new(ServerState::new(rx_info, receiver_dir.clone()));
    let (rx_port, _rx_task) = start_server(rx_state.clone(), 0).await.unwrap();

    let peer = PeerInfo::new(
        "Receiver-PC",
        "desktop",
        "windows",
        None,
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
        None,
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
