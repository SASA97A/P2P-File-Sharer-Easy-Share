use easy_share_lib::client::{ClientError, FileToSend, TransferClient};
use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
use easy_share_lib::models::transfer::{FileMetadata, TransferRequest, TransferStatus};
use easy_share_lib::server::{start_server, ConsentPolicy, ServerState};
use easy_share_lib::storage::file_manager::FileManager;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_test_client_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create test dir");
    dir
}

fn sample_device_info(port: u16) -> DeviceInfo {
    DeviceInfo::new(
        "Receiver-Device",
        "desktop",
        "windows",
        "2.0.0",
        Some("Room123".to_string()),
        port,
    )
}

fn create_test_file(path: &PathBuf, size_bytes: usize) -> String {
    let mut data = Vec::with_capacity(size_bytes);
    for i in 0..size_bytes {
        data.push((i % 251) as u8);
    }
    fs::write(path, &data).expect("failed to write test file");
    FileManager::calculate_blake3_file(path).expect("failed to calculate blake3")
}

#[tokio::test]
async fn test_probe_peer_success() {
    let temp_dir = setup_test_dir("probe");
    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        temp_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::new();
    let peer = PeerInfo::new("Receiver-Device", "desktop", "windows", Some("Room123".to_string()), "127.0.0.1", port);

    let info = client.probe_peer(&peer).await.expect("failed to probe peer");
    assert_eq!(info.device_name, "Receiver-Device");
    assert_eq!(info.os, "windows");
    assert_eq!(info.port, port);
    assert_eq!(info.room_id, Some("Room123".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_request_transfer_accepted() {
    let temp_dir = setup_test_dir("req_accepted");
    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        temp_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::new();
    let peer = PeerInfo::new("Receiver-Device", "desktop", "windows", None, "127.0.0.1", port);

    let files = vec![FileMetadata::new("f-1", "test.bin", 1024, None)];
    let req = TransferRequest::new("req-123", "Sender-Device", "windows", None, files);

    let resp = client.request_transfer(&peer, &req).await.expect("request failed");
    assert_eq!(resp.status, TransferStatus::Accepted);
    assert!(resp.session_token.is_some());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_request_transfer_declined() {
    let temp_dir = setup_test_dir("req_declined");
    let state = Arc::new(ServerState::with_consent(
        sample_device_info(0),
        temp_dir.clone(),
        ConsentPolicy::AutoDecline("User is busy".to_string()),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::new();
    let peer = PeerInfo::new("Receiver-Device", "desktop", "windows", None, "127.0.0.1", port);

    let files = vec![FileMetadata::new("f-1", "test.bin", 1024, None)];
    let req = TransferRequest::new("req-123", "Sender-Device", "windows", None, files);

    let resp = client.request_transfer(&peer, &req).await.expect("request should return declined response");
    assert_eq!(resp.status, TransferStatus::Declined);
    assert_eq!(resp.reason.as_deref(), Some("User is busy"));
    assert!(resp.session_token.is_none());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_send_file_resumable_and_finish_full_flow() {
    let server_dir = setup_test_dir("full_flow_server");
    let client_dir = setup_test_dir("full_flow_client");

    let file_path = client_dir.join("payload.bin");
    let file_size = 2 * 1024 * 1024; // 2 MB
    let expected_hash = create_test_file(&file_path, file_size);

    let file_to_send = FileToSend::from_path("f-full", &file_path).expect("failed to create FileToSend");
    assert_eq!(file_to_send.size, file_size as u64);
    assert_eq!(file_to_send.blake3_hash.as_deref(), Some(expected_hash.as_str()));

    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        server_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::with_chunk_size(512 * 1024); // 512 KB chunks
    let peer = PeerInfo::new("Receiver", "desktop", "windows", None, "127.0.0.1", port);

    // Handshake
    let req = TransferRequest::new("req-full", "Sender", "windows", None, vec![file_to_send.to_metadata()]);
    let resp = client.request_transfer(&peer, &req).await.expect("handshake failed");
    let token = resp.session_token.expect("expected session token");

    // Send file with progress tracking
    let progress_calls = Arc::new(AtomicU64::new(0));
    let progress_calls_clone = progress_calls.clone();
    let max_reported_bytes = Arc::new(AtomicU64::new(0));
    let max_reported_clone = max_reported_bytes.clone();

    client
        .send_file_resumable(&peer, &token, &file_to_send, move |sent, total| {
            progress_calls_clone.fetch_add(1, Ordering::SeqCst);
            max_reported_clone.store(sent, Ordering::SeqCst);
            assert_eq!(total, file_size as u64);
        })
        .await
        .expect("send_file_resumable failed");

    assert!(progress_calls.load(Ordering::SeqCst) >= 4); // 2MB in 512KB chunks = at least 4 chunks
    assert_eq!(max_reported_bytes.load(Ordering::SeqCst), file_size as u64);

    // Finish transfer
    let finish_resp = client
        .finish_transfer(&peer, &token, &file_to_send.file_id)
        .await
        .expect("finish_transfer failed");

    assert_eq!(finish_resp.status, TransferStatus::Completed);
    assert_eq!(finish_resp.file_id, "f-full");

    let received_path = PathBuf::from(finish_resp.saved_path);
    assert!(received_path.exists(), "received file does not exist on server");
    let received_size = FileManager::get_file_size(&received_path).expect("failed to get size");
    assert_eq!(received_size, file_size as u64);

    let received_hash = FileManager::calculate_blake3_file(&received_path).expect("failed to hash");
    assert_eq!(received_hash, expected_hash);

    let _ = fs::remove_dir_all(&server_dir);
    let _ = fs::remove_dir_all(&client_dir);
}

#[tokio::test]
async fn test_resumption_after_partial_transfer() {
    let server_dir = setup_test_dir("resume_server");
    let client_dir = setup_test_dir("resume_client");

    let file_path = client_dir.join("large_payload.bin");
    let file_size = 5 * 1024 * 1024; // 5 MB
    let expected_hash = create_test_file(&file_path, file_size);

    let file_to_send = FileToSend::from_path("f-resume", &file_path).expect("failed to create FileToSend");

    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        server_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::with_chunk_size(512 * 1024); // 512 KB chunks
    let peer = PeerInfo::new("Receiver", "desktop", "windows", None, "127.0.0.1", port);

    // Handshake
    let req = TransferRequest::new("req-resume", "Sender", "windows", None, vec![file_to_send.to_metadata()]);
    let resp = client.request_transfer(&peer, &req).await.expect("handshake failed");
    let token = resp.session_token.expect("expected session token");

    // Phase 1: Upload first 2 MB (4 chunks of 512 KB) then stop
    let bytes_sent_p1 = client
        .send_file_chunks_with_limit(&peer.base_url(), &token, &file_to_send, 0, Some(2 * 1024 * 1024), |_, _| {})
        .await
        .expect("partial upload failed");
    assert_eq!(bytes_sent_p1, 2 * 1024 * 1024);

    // Query status to verify server has 2 MB written
    let status = client
        .get_transfer_status(&peer, &token, &file_to_send.file_id)
        .await
        .expect("status query failed");
    assert_eq!(status.bytes_received, 2 * 1024 * 1024);
    assert_eq!(status.status, TransferStatus::Partial);

    // Phase 2: Resume transfer from reported offset to EOF
    let progress_calls = Arc::new(AtomicU64::new(0));
    let progress_calls_clone = progress_calls.clone();

    client
        .send_file_resumable(&peer, &token, &file_to_send, move |sent, total| {
            progress_calls_clone.fetch_add(1, Ordering::SeqCst);
            assert!(sent >= 2 * 1024 * 1024);
            assert_eq!(total, file_size as u64);
        })
        .await
        .expect("resumed transfer failed");

    // Finish transfer and verify server validates whole file
    let finish_resp = client
        .finish_transfer(&peer, &token, &file_to_send.file_id)
        .await
        .expect("finish_transfer failed");

    assert_eq!(finish_resp.status, TransferStatus::Completed);
    let received_path = PathBuf::from(finish_resp.saved_path);
    assert!(received_path.exists());
    let received_size = FileManager::get_file_size(&received_path).expect("failed to get size");
    assert_eq!(received_size, file_size as u64);

    let received_hash = FileManager::calculate_blake3_file(&received_path).expect("failed to hash");
    assert_eq!(received_hash, expected_hash);

    let _ = fs::remove_dir_all(&server_dir);
    let _ = fs::remove_dir_all(&client_dir);
}

#[tokio::test]
async fn test_progress_reporting_monotonically_increasing() {
    let server_dir = setup_test_dir("progress_server");
    let client_dir = setup_test_dir("progress_client");

    let file_path = client_dir.join("progress.bin");
    let file_size = 3 * 1024 * 1024; // 3 MB
    let _ = create_test_file(&file_path, file_size);

    let file_to_send = FileToSend::from_path("f-prog", &file_path).expect("failed to create FileToSend");

    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        server_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::with_chunk_size(512 * 1024);
    let peer = PeerInfo::new("Receiver", "desktop", "windows", None, "127.0.0.1", port);

    let req = TransferRequest::new("req-prog", "Sender", "windows", None, vec![file_to_send.to_metadata()]);
    let resp = client.request_transfer(&peer, &req).await.expect("handshake failed");
    let token = resp.session_token.expect("expected session token");

    let updates = Arc::new(std::sync::Mutex::new(Vec::new()));
    let updates_clone = updates.clone();

    client
        .send_file_resumable(&peer, &token, &file_to_send, move |sent, total| {
            let mut list = updates_clone.lock().unwrap();
            list.push((sent, total));
        })
        .await
        .expect("send failed");

    let recorded = updates.lock().unwrap().clone();
    assert!(!recorded.is_empty());
    let mut prev_sent = 0;
    for (sent, total) in &recorded {
        assert_eq!(*total, file_size as u64);
        assert!(*sent >= prev_sent, "progress went backwards: {} < {}", sent, prev_sent);
        prev_sent = *sent;
    }
    assert_eq!(recorded.last().unwrap().0, file_size as u64);

    let _ = fs::remove_dir_all(&server_dir);
    let _ = fs::remove_dir_all(&client_dir);
}

#[tokio::test]
async fn test_cancel_transfer() {
    let server_dir = setup_test_dir("cancel_server");
    let client_dir = setup_test_dir("cancel_client");

    let file_path = client_dir.join("cancel.bin");
    let file_size = 2 * 1024 * 1024;
    let _ = create_test_file(&file_path, file_size);

    let file_to_send = FileToSend::from_path("f-cancel", &file_path).expect("failed to create FileToSend");

    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        server_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::with_chunk_size(512 * 1024);
    let peer = PeerInfo::new("Receiver", "desktop", "windows", None, "127.0.0.1", port);

    let req = TransferRequest::new("req-cancel", "Sender", "windows", None, vec![file_to_send.to_metadata()]);
    let resp = client.request_transfer(&peer, &req).await.expect("handshake failed");
    let token = resp.session_token.expect("expected session token");

    // Upload first 512 KB
    let _ = client
        .send_file_chunks_with_limit(&peer.base_url(), &token, &file_to_send, 0, Some(512 * 1024), |_, _| {})
        .await
        .expect("initial chunk failed");

    // Cancel transfer
    client
        .cancel_transfer(&peer, &token, Some("User cancelled".to_string()))
        .await
        .expect("cancel_transfer failed");

    // Subsequent chunk or finish should fail
    let next_chunk_res = client
        .send_chunk(&peer, &token, &file_to_send.file_id, 512 * 1024, vec![1, 2, 3])
        .await;
    assert!(next_chunk_res.is_err(), "chunk after cancel should fail");

    let _ = fs::remove_dir_all(&server_dir);
    let _ = fs::remove_dir_all(&client_dir);
}

#[tokio::test]
async fn test_finish_hash_mismatch_fails() {
    let server_dir = setup_test_dir("mismatch_server");
    let client_dir = setup_test_dir("mismatch_client");

    let file_path = client_dir.join("mismatch.bin");
    let file_size = 1024;
    let _ = create_test_file(&file_path, file_size);

    // Create FileToSend with wrong expected hash
    let mut file_to_send = FileToSend::from_path("f-mismatch", &file_path).expect("failed to create FileToSend");
    file_to_send.blake3_hash = Some("0000000000000000000000000000000000000000000000000000000000000000".to_string());

    let state = Arc::new(ServerState::new(
        sample_device_info(0),
        server_dir.clone(),
    ));

    let (port, _handle) = start_server(state, 0).await.expect("failed to start server");
    let client = TransferClient::new();
    let peer = PeerInfo::new("Receiver", "desktop", "windows", None, "127.0.0.1", port);

    let req = TransferRequest::new("req-mismatch", "Sender", "windows", None, vec![file_to_send.to_metadata()]);
    let resp = client.request_transfer(&peer, &req).await.expect("handshake failed");
    let token = resp.session_token.expect("expected session token");

    // Send chunks
    client
        .send_file_resumable(&peer, &token, &file_to_send, |_, _| {})
        .await
        .expect("chunk upload succeeded");

    // Finish should fail due to hash mismatch on server
    let finish_res = client.finish_transfer(&peer, &token, &file_to_send.file_id).await;
    assert!(finish_res.is_err(), "finish should fail on hash mismatch");

    let _ = fs::remove_dir_all(&server_dir);
    let _ = fs::remove_dir_all(&client_dir);
}

#[tokio::test]
async fn test_send_file_non_existent() {
    let client = TransferClient::new();
    let peer = PeerInfo::new("Receiver", "desktop", "windows", None, "127.0.0.1", 5050);
    let non_existent = FileToSend::new("f-none", "non_existent_file_path_123.bin", "none.bin", 100, None);

    let res = client.send_file_resumable(&peer, "fake-token", &non_existent, |_, _| {}).await;
    assert!(res.is_err());
    match res.err().unwrap() {
        ClientError::FileNotFound(_) | ClientError::Io(_) => (),
        other => panic!("expected FileNotFound or Io error, got {:?}", other),
    }
}
