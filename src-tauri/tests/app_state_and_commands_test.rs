use easy_share_lib::app_state::AppState;
use easy_share_lib::client::{FileToSend, TransferClient};
use easy_share_lib::commands::{
    build_pairing_payload, inspect_file_paths, FileSelection, PairingInfoPayload, ProgressPayload,
    SelectedFile,
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
    .with_device_id("test-host-id")
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
    assert_eq!(app_state.download_dir(), temp_dir);
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
                speed_bps: 0,
                eta_seconds: None,
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

#[test]
fn test_transfer_batch_completed_payload_serialization() {
    use easy_share_lib::commands::TransferBatchCompletedPayload;

    let payload = TransferBatchCompletedPayload {
        session_token: "sess-12345".to_string(),
        total_files: 5,
        total_bytes: 10485760,
    };

    let serialized = serde_json::to_string(&payload).expect("should serialize batch payload");
    let deserialized: TransferBatchCompletedPayload =
        serde_json::from_str(&serialized).expect("should deserialize batch payload");

    assert_eq!(deserialized, payload);
    assert_eq!(deserialized.total_files, 5);
    assert_eq!(deserialized.total_bytes, 10485760);
}

#[test]
fn test_tray_settings_payload_serialization() {
    use easy_share_lib::commands::TraySettingsPayload;

    let payload = TraySettingsPayload {
        minimize_to_tray: true,
        notifications_enabled: false,
    };

    let serialized = serde_json::to_string(&payload).expect("should serialize tray settings payload");
    let deserialized: TraySettingsPayload =
        serde_json::from_str(&serialized).expect("should deserialize tray settings payload");

    assert_eq!(deserialized, payload);

    // Test camelCase alias deserialization from frontend
    let json_camel = r#"{"minimizeToTray": false, "notificationsEnabled": true}"#;
    let des_camel: TraySettingsPayload = serde_json::from_str(json_camel).unwrap();
    assert!(!des_camel.minimize_to_tray);
    assert!(des_camel.notifications_enabled);
}

#[test]
fn test_format_transfer_tooltip_formatting() {
    use easy_share_lib::tray::format_transfer_tooltip;

    let tip1 = format_transfer_tooltip("Receiving", 45, Some(14_889_779), Some("project.zip"));
    assert_eq!(tip1, "Easy Share - Receiving (45% · 14.2 MB/s) (project.zip)");

    let tip2 = format_transfer_tooltip("Sending", 90, Some(512_000), None);
    assert_eq!(tip2, "Easy Share - Sending (90% · 500 KB/s)");

    let tip3 = format_transfer_tooltip("Receiving", 10, None, None);
    assert_eq!(tip3, "Easy Share - Receiving (10%)");
}

#[test]
fn test_format_bytes_for_notifications() {
    use easy_share_lib::notifications::format_bytes;

    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
    assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
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
    target_info.device_id = Some("direct-target-id".to_string());

    let target_server_state = Arc::new(ServerState::new(target_info, temp_dir.clone()));
    target_server_state.set_pairing_open(true);
    let (target_port, _target_task) = start_server(target_server_state.clone(), 0)
        .await
        .expect("target server must start");

    let client = TransferClient::new();

    // 1. Direct connect by raw IP:Port (does not leak secret PIN)
    let direct_addr = format!("127.0.0.1:{}", target_port);
    let peer = easy_share_lib::pairing::resolve_peer_by_address(&client, &direct_addr)
        .await
        .expect("should resolve peer by address");
    assert_eq!(peer.device_name, "Direct-Target");
    assert_eq!(peer.port, target_port);
    assert_eq!(peer.pairing_pin, None, "Plain IP resolution must not leak remote PIN");

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

    // 3. A wrong PIN does not pair
    let normalized_wrong =
        easy_share_lib::pairing::pin::normalize_pin("111 - 222").expect("should normalize");
    assert!(client.pair_open_peer(&peer, &normalized_wrong).await.is_err());
    assert_eq!(client.pairing_count().await, 0);

    // 4. The right PIN pairs over SPAKE2 with the device that is showing it
    let normalized_input =
        easy_share_lib::pairing::pin::normalize_pin("482 - 910").expect("should normalize");
    let authenticated = client
        .pair_open_peer(&peer, &normalized_input)
        .await
        .expect("pairing should succeed with correct PIN");
    assert_eq!(authenticated.device_name, "Direct-Target");
    assert_eq!(client.pairing_count().await, 1);
}

#[tokio::test]
async fn test_inspect_file_paths_resolution() {
    let temp_dir = setup_test_dir("inspect_files");
    let file1_path = temp_dir.join("doc1.txt");
    let file2_path = temp_dir.join("data.bin");
    let sub_dir = temp_dir.join("subfolder");
    let file3_path = sub_dir.join("doc3.txt");

    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(&file1_path, b"Hello World").unwrap(); // 11 bytes
    fs::write(&file2_path, vec![0u8; 1024]).unwrap(); // 1024 bytes
    fs::write(&file3_path, b"Nested file content").unwrap(); // 19 bytes

    // 1. Inspect single files
    let paths = vec![
        file1_path.to_string_lossy().to_string(),
        file2_path.to_string_lossy().to_string(),
    ];
    let selected: Vec<SelectedFile> = inspect_file_paths(&paths);
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].name, "doc1.txt");
    assert_eq!(selected[0].size, 11);
    assert_eq!(selected[1].name, "data.bin");
    assert_eq!(selected[1].size, 1024);

    // 2. Inspect folder (recurses entries)
    let folder_paths = vec![sub_dir.to_string_lossy().to_string()];
    let folder_selected = inspect_file_paths(&folder_paths);
    assert_eq!(folder_selected.len(), 1);
    assert_eq!(folder_selected[0].name, "doc3.txt");
    assert_eq!(folder_selected[0].size, 19);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_inspect_file_paths_deep_recursion() {
    let temp_dir = setup_test_dir("inspect_deep_recursion");
    let file_root = temp_dir.join("file_root.txt");
    let empty_dir = temp_dir.join("empty_dir");
    let level1 = temp_dir.join("level1");
    let file_l1 = level1.join("file_level1.txt");
    let level2 = level1.join("level2");
    let file_l2 = level2.join("file_level2.txt");
    let level3 = level2.join("level3");
    let file_l3 = level3.join("file_level3.txt");

    fs::create_dir_all(&empty_dir).unwrap();
    fs::create_dir_all(&level3).unwrap();

    fs::write(&file_root, vec![1u8; 10]).unwrap();
    fs::write(&file_l1, vec![2u8; 20]).unwrap();
    fs::write(&file_l2, vec![3u8; 30]).unwrap();
    fs::write(&file_l3, vec![4u8; 40]).unwrap();

    let paths = vec![temp_dir.to_string_lossy().to_string()];
    let selected = inspect_file_paths(&paths);

    let total_size: u64 = selected.iter().map(|f| f.size).sum();
    let mut names: Vec<String> = selected.into_iter().map(|f| f.name).collect();
    names.sort();

    assert_eq!(names.len(), 4, "Must find all 4 nested files recursively");
    assert_eq!(
        names,
        vec![
            "file_level1.txt".to_string(),
            "file_level2.txt".to_string(),
            "file_level3.txt".to_string(),
            "file_root.txt".to_string(),
        ]
    );
    assert_eq!(total_size, 100, "Total size of all recursive files must match");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_inspect_file_paths_preserves_relative_directory_tree() {
    let temp_root = std::env::temp_dir().join("easy_share_test_dir_inspect");
    let _ = std::fs::remove_dir_all(&temp_root);
    let sub = temp_root.join("Project").join("src");
    std::fs::create_dir_all(&sub).unwrap();
    let file_path = sub.join("main.rs");
    std::fs::write(&file_path, "fn main() {}").unwrap();

    let results = inspect_file_paths(&[temp_root.join("Project")]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "main.rs");
    assert_eq!(results[0].relative_path, Some("Project/src/main.rs".to_string()));

    let _ = std::fs::remove_dir_all(&temp_root);
}

#[tokio::test]
async fn test_app_state_cancellation_token_lifecycle() {
    let temp_dir = setup_test_dir("cancellation_token");
    let info = sample_device_info();
    let server_state = Arc::new(ServerState::new(info.clone(), temp_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));

    let app_state = AppState::new(server_state, None, temp_dir.clone(), pending_consents);

    let session_token = "session-test-token-12345";
    let token = tokio_util::sync::CancellationToken::new();

    // 1. Initially not cancelled
    assert!(!token.is_cancelled());

    // 2. Register token in AppState
    app_state
        .register_cancellation_token(session_token, token.clone())
        .await;

    // 3. Cancelling non-existent session returns false
    let not_found = app_state.cancel_session("non-existent-session").await;
    assert!(!not_found);
    assert!(!token.is_cancelled());

    // 4. Cancelling registered session cancels the token and returns true
    let cancelled = app_state.cancel_session(session_token).await;
    assert!(cancelled);
    assert!(token.is_cancelled());

    // 5. Token is now removed from registry, cancelling again returns false
    let cancelled_again = app_state.cancel_session(session_token).await;
    assert!(!cancelled_again);

    // 6. Test remove_cancellation_token
    let token2 = tokio_util::sync::CancellationToken::new();
    let session_token2 = "session-test-token-67890";
    app_state
        .register_cancellation_token(session_token2, token2.clone())
        .await;
    app_state.remove_cancellation_token(session_token2).await;
    assert!(!app_state.cancel_session(session_token2).await);
    assert!(!token2.is_cancelled());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Starts a receiver that accepts pairing with `pin`, returning its state and peer handle.
async fn start_pairable_receiver(dir: &std::path::Path, pin: &str) -> (Arc<ServerState>, PeerInfo) {
    let info = DeviceInfo::new("Remembered-Receiver", "desktop", "windows", "2.0.0", 0)
        .with_pairing_pin(pin)
        .with_device_id("remembered-receiver-id");
    let state = Arc::new(ServerState::new(info, dir.to_path_buf()));
    state.set_pairing_open(true);
    let (port, _task) = start_server(state.clone(), 0).await.expect("receiver must start");
    let peer = PeerInfo::new("Remembered-Receiver", "desktop", "windows", "127.0.0.1", port);
    (state, peer)
}

#[tokio::test]
async fn test_remember_paired_devices_survives_restart_on_both_sides() {
    let sender_cfg = setup_test_dir("remember_sender_cfg");
    let receiver_cfg = setup_test_dir("remember_receiver_cfg");

    // Receiver side: an AppState with remembering on owns the server state
    let (receiver_server, peer) = start_pairable_receiver(&receiver_cfg, "246802").await;
    let receiver_app = AppState::with_config_dir(
        receiver_server.clone(),
        None,
        receiver_cfg.clone(),
        receiver_cfg.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );
    receiver_app.set_remember_paired_devices(true).await.unwrap();

    // Sender side
    let sender_server = Arc::new(ServerState::new(sample_device_info(), sender_cfg.clone()));
    let sender_app = AppState::with_config_dir(
        sender_server,
        None,
        sender_cfg.clone(),
        sender_cfg.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );
    sender_app.set_remember_paired_devices(true).await.unwrap();

    sender_app.transfer_client.pair_peer(&peer, "246802").await.expect("pairing");
    sender_app.save_paired_devices().await;
    let pairing = sender_app
        .transfer_client
        .get_pairing("remembered-receiver-id")
        .await
        .expect("sender keeps pairing");

    // "Restart" both: fresh states loaded from the same config dirs
    let receiver_restarted = Arc::new(ServerState::new(
        DeviceInfo::new("Remembered-Receiver", "desktop", "windows", "2.0.0", 0),
        receiver_cfg.clone(),
    ));
    let _receiver_app2 = AppState::with_config_dir(
        receiver_restarted.clone(),
        None,
        receiver_cfg.clone(),
        receiver_cfg.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );
    assert_eq!(
        receiver_restarted.pairing_key(&pairing.pair_id),
        Some(pairing.key.clone()),
        "receiver must remember the pairing after restart"
    );

    let sender_restarted = AppState::with_config_dir(
        Arc::new(ServerState::new(sample_device_info(), sender_cfg.clone())),
        None,
        sender_cfg.clone(),
        sender_cfg.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );
    let reloaded = sender_restarted
        .transfer_client
        .get_pairing("remembered-receiver-id")
        .await
        .expect("sender must remember the pairing after restart");
    assert_eq!(reloaded.key, pairing.key);
    assert_eq!(reloaded.pair_id, pairing.pair_id);

    // Turning remembering off deletes the saved pairings
    sender_restarted.set_remember_paired_devices(false).await.unwrap();
    assert!(!sender_cfg
        .join(easy_share_lib::storage::pairings::TRUSTED_PAIRINGS_FILE)
        .exists());

    let _ = fs::remove_dir_all(&sender_cfg);
    let _ = fs::remove_dir_all(&receiver_cfg);
}

#[tokio::test]
async fn test_pairings_are_not_saved_when_remember_is_off() {
    let sender_cfg = setup_test_dir("no_remember_sender_cfg");
    let receiver_cfg = setup_test_dir("no_remember_receiver_cfg");
    let (_receiver_server, peer) = start_pairable_receiver(&receiver_cfg, "135791").await;

    let sender_app = AppState::with_config_dir(
        Arc::new(ServerState::new(sample_device_info(), sender_cfg.clone())),
        None,
        sender_cfg.clone(),
        sender_cfg.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );
    assert!(!sender_app.paired_devices_status().await.remember, "remembering defaults to off");

    sender_app.transfer_client.pair_peer(&peer, "135791").await.expect("pairing");
    sender_app.save_paired_devices().await;
    assert!(!sender_cfg
        .join(easy_share_lib::storage::pairings::TRUSTED_PAIRINGS_FILE)
        .exists());
    assert!(!receiver_cfg
        .join(easy_share_lib::storage::pairings::ACCEPTED_PAIRINGS_FILE)
        .exists());

    let _ = fs::remove_dir_all(&sender_cfg);
    let _ = fs::remove_dir_all(&receiver_cfg);
}

#[tokio::test]
async fn test_forgotten_pairing_blocks_sending_until_paired_again() {
    let receiver_cfg = setup_test_dir("needs_repair_receiver_cfg");
    let (receiver_server, peer) = start_pairable_receiver(&receiver_cfg, "864200").await;
    let client = TransferClient::new();

    client.pair_peer(&peer, "864200").await.expect("pairing");
    assert!(!client.needs_repair("remembered-receiver-id").await);

    // The receiver forgets (e.g. restarted without remembering devices)
    receiver_server.forget_accepted_pairings();
    client.mark_needs_repair("remembered-receiver-id").await;
    assert!(client.needs_repair("remembered-receiver-id").await);
    assert!(client.get_pairing("remembered-receiver-id").await.is_none());

    // The block survives a save/load round-trip
    let exported = client.export_pairings().await;
    let restored = TransferClient::new();
    restored.import_pairings(exported);
    assert!(restored.needs_repair("remembered-receiver-id").await);

    // Pairing again lifts the block
    client.pair_peer(&peer, "864200").await.expect("re-pairing");
    assert!(!client.needs_repair("remembered-receiver-id").await);
    assert!(client.get_pairing("remembered-receiver-id").await.is_some());

    let _ = fs::remove_dir_all(&receiver_cfg);
}
