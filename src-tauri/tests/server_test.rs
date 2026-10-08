use easy_share_lib::models::peer::DeviceInfo;
use easy_share_lib::models::transfer::{
    CancelRequest, ChunkResponse, FileMetadata, FinishRequest, FinishResponse, TransferRequest,
    TransferResponse, TransferStatus, TransferStatusQueryResponse,
};
use easy_share_lib::server::{start_server, ConsentPolicy, ConsentRequest, ServerState};
use easy_share_lib::storage::file_manager::FileManager;
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_test_server_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create test dir");
    dir
}

fn sample_device_info() -> DeviceInfo {
    DeviceInfo::new(
        "Test-Device",
        "desktop",
        "windows",
        "2.0.0",
        Some("Engineering".to_string()),
        0,
    )
}

#[tokio::test]
async fn test_server_startup_dynamic_port_and_device_info() {
    let temp_dir = setup_test_dir("startup");
    let device_info = sample_device_info();
    let state = Arc::new(ServerState::new(device_info.clone(), temp_dir.clone()));

    let (port, _server_handle) = start_server(state.clone(), 0)
        .await
        .expect("server should start on dynamic port");

    assert!(port > 0, "dynamic port should be non-zero");

    let client = Client::new();
    let res = client
        .get(format!("http://127.0.0.1:{}/api/v1/info", port))
        .send()
        .await
        .expect("failed to request info endpoint");

    assert_eq!(res.status(), 200);

    let info: DeviceInfo = res.json().await.expect("failed to parse DeviceInfo json");
    assert_eq!(info.device_name, "Test-Device");
    assert_eq!(info.device_type, "desktop");
    assert_eq!(info.os, "windows");
    assert_eq!(info.version, "2.0.0");
    assert_eq!(info.room_id, Some("Engineering".to_string()));
    assert_eq!(info.port, port);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_transfer_request_accepted_and_declined() {
    let temp_dir = setup_test_dir("request_handshake");
    let client = Client::new();

    // 1. Auto-Accept Server
    let state_accept = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port_accept, _h1) = start_server(state_accept, 0)
        .await
        .expect("failed to start accept server");

    let request_payload = TransferRequest::new(
        "req-001",
        "Sender-Device",
        "linux",
        Some("Engineering".to_string()),
        vec![FileMetadata::new("f1", "doc.pdf", 1024, None)],
    );

    let res_accept = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/request",
            port_accept
        ))
        .json(&request_payload)
        .send()
        .await
        .expect("request failed");

    assert_eq!(res_accept.status(), 200);
    let resp: TransferResponse = res_accept.json().await.expect("invalid json response");
    assert_eq!(resp.status, TransferStatus::Accepted);
    assert!(resp.session_token.is_some());
    let token = resp.session_token.unwrap();
    assert!(!token.is_empty());
    assert_eq!(resp.existing_offsets.get("f1"), Some(&0));

    // 2. Auto-Decline Server
    let state_decline = Arc::new(ServerState::with_consent(
        sample_device_info(),
        temp_dir.clone(),
        ConsentPolicy::AutoDecline("Storage full".to_string()),
    ));
    let (port_decline, _h2) = start_server(state_decline, 0)
        .await
        .expect("failed to start decline server");

    let res_decline = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/request",
            port_decline
        ))
        .json(&request_payload)
        .send()
        .await
        .expect("request failed");

    assert_eq!(res_decline.status(), 403);
    let resp_declined: TransferResponse =
        res_decline.json().await.expect("invalid json response");
    assert_eq!(resp_declined.status, TransferStatus::Declined);
    assert_eq!(resp_declined.reason, Some("Storage full".to_string()));
    assert!(resp_declined.session_token.is_none());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_transfer_consent_interactive_channel() {
    let temp_dir = setup_test_dir("consent_channel");
    let (tx, mut rx) = mpsc::channel::<ConsentRequest>(10);

    let state = Arc::new(ServerState::with_consent(
        sample_device_info(),
        temp_dir.clone(),
        ConsentPolicy::Channel(tx),
    ));

    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let request_payload = TransferRequest::new(
        "req-interactive",
        "Sender-Alice",
        "macos",
        None,
        vec![FileMetadata::new("file-1", "photo.jpg", 5000, None)],
    );

    let client = Client::new();
    let req_handle = tokio::spawn(async move {
        client
            .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
            .json(&request_payload)
            .send()
            .await
    });

    // Simulate UI receiving the prompt and accepting
    let consent_req = rx
        .recv()
        .await
        .expect("should receive consent request in channel");
    assert_eq!(consent_req.transfer_request.sender_name, "Sender-Alice");
    assert_eq!(consent_req.transfer_request.files.len(), 1);
    consent_req
        .responder
        .send(true)
        .expect("failed to send consent response");

    let res = req_handle
        .await
        .expect("request task panicked")
        .expect("request failed");
    assert_eq!(res.status(), 200);
    let resp: TransferResponse = res.json().await.expect("failed to parse json");
    assert_eq!(resp.status, TransferStatus::Accepted);
    assert!(resp.session_token.is_some());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_chunk_upload_status_and_streaming() {
    let temp_dir = setup_test_dir("chunk_stream");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    // Handshake
    let request_payload = TransferRequest::new(
        "req-chunk",
        "Sender",
        "windows",
        None,
        vec![FileMetadata::new("f-test", "test.bin", 100, None)],
    );

    let handshake_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
        .json(&request_payload)
        .send()
        .await
        .expect("handshake failed");
    let handshake: TransferResponse = handshake_res.json().await.unwrap();
    let token = handshake.session_token.unwrap();

    // Initial status check: 0 bytes received
    let status_res = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/transfer/status?session_token={}&file_id=f-test",
            port, token
        ))
        .send()
        .await
        .expect("status request failed");
    assert_eq!(status_res.status(), 200);
    let status: TransferStatusQueryResponse = status_res.json().await.unwrap();
    assert_eq!(status.file_id, "f-test");
    assert_eq!(status.bytes_received, 0);

    // Upload Chunk 1: bytes 0..50
    let chunk1_data = vec![0xAA; 50];
    let chunk1_res = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-test&offset=0",
            port, token
        ))
        .header("Content-Type", "application/octet-stream")
        .body(chunk1_data)
        .send()
        .await
        .expect("chunk 1 upload failed");
    assert_eq!(chunk1_res.status(), 200);
    let chunk1_resp: ChunkResponse = chunk1_res.json().await.unwrap();
    assert_eq!(chunk1_resp.file_id, "f-test");
    assert_eq!(chunk1_resp.bytes_written, 50);
    assert_eq!(chunk1_resp.current_total, 50);

    // Status check after chunk 1: 50 bytes received
    let status_res2 = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/transfer/status?session_token={}&file_id=f-test",
            port, token
        ))
        .send()
        .await
        .unwrap();
    let status2: TransferStatusQueryResponse = status_res2.json().await.unwrap();
    assert_eq!(status2.bytes_received, 50);
    assert_eq!(status2.status, TransferStatus::Partial);

    // Upload Chunk 2: bytes 50..100
    let chunk2_data = vec![0xBB; 50];
    let chunk2_res = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-test&offset=50",
            port, token
        ))
        .header("Content-Type", "application/octet-stream")
        .body(chunk2_data)
        .send()
        .await
        .expect("chunk 2 upload failed");
    assert_eq!(chunk2_res.status(), 200);
    let chunk2_resp: ChunkResponse = chunk2_res.json().await.unwrap();
    assert_eq!(chunk2_resp.bytes_written, 50);
    assert_eq!(chunk2_resp.current_total, 100);

    // Verify raw .part file content on disk
    let part_path = FileManager::get_part_path(&temp_dir, "test.bin", &token);
    let file_bytes = fs::read(&part_path).expect("failed to read .part file");
    assert_eq!(file_bytes.len(), 100);
    assert_eq!(&file_bytes[0..50], &vec![0xAA; 50][..]);
    assert_eq!(&file_bytes[50..100], &vec![0xBB; 50][..]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_resumed_chunk_upload_seekable() {
    let temp_dir = setup_test_dir("chunk_resumption");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    let request_payload = TransferRequest::new(
        "req-resume",
        "Sender",
        "windows",
        None,
        vec![FileMetadata::new("f-resume", "data.iso", 1000, None)],
    );

    let handshake_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
        .json(&request_payload)
        .send()
        .await
        .unwrap();
    let handshake: TransferResponse = handshake_res.json().await.unwrap();
    let token = handshake.session_token.unwrap();

    // Upload first 400 bytes
    let first_chunk = vec![0x11; 400];
    let _ = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-resume&offset=0",
            port, token
        ))
        .body(first_chunk)
        .send()
        .await
        .unwrap();

    // Check status reports 400 bytes
    let status_res = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/transfer/status?session_token={}&file_id=f-resume",
            port, token
        ))
        .send()
        .await
        .unwrap();
    let status: TransferStatusQueryResponse = status_res.json().await.unwrap();
    assert_eq!(status.bytes_received, 400);

    // Resume from offset 400 with 600 bytes
    let second_chunk = vec![0x22; 600];
    let res_chunk2 = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-resume&offset=400",
            port, token
        ))
        .body(second_chunk)
        .send()
        .await
        .unwrap();
    assert_eq!(res_chunk2.status(), 200);

    let part_path = FileManager::get_part_path(&temp_dir, "data.iso", &token);
    let file_bytes = fs::read(&part_path).expect("failed to read .part file");
    assert_eq!(file_bytes.len(), 1000);
    assert_eq!(&file_bytes[0..400], &vec![0x11; 400][..]);
    assert_eq!(&file_bytes[400..1000], &vec![0x22; 600][..]);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_transfer_finish_checksum_validation_and_commit() {
    let temp_dir = setup_test_dir("finish_commit");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    let payload = b"Hello, Easy Share P2P High Speed Transfer!".to_vec();
    let expected_hash = FileManager::calculate_blake3_bytes(&payload);

    let request_payload = TransferRequest::new(
        "req-finish",
        "Sender",
        "windows",
        None,
        vec![FileMetadata::new(
            "f-final",
            "greeting.txt",
            payload.len() as u64,
            Some(expected_hash),
        )],
    );

    let handshake_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
        .json(&request_payload)
        .send()
        .await
        .unwrap();
    let handshake: TransferResponse = handshake_res.json().await.unwrap();
    let token = handshake.session_token.unwrap();

    // Upload payload
    let _ = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-final&offset=0",
            port, token
        ))
        .body(payload.clone())
        .send()
        .await
        .unwrap();

    // Call Finish
    let finish_req = FinishRequest::new(&token, "f-final");
    let finish_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/finish", port))
        .json(&finish_req)
        .send()
        .await
        .unwrap();

    assert_eq!(finish_res.status(), 200);
    let finish_resp: FinishResponse = finish_res.json().await.unwrap();
    assert_eq!(finish_resp.status, TransferStatus::Completed);
    assert_eq!(finish_resp.file_id, "f-final");

    // Verify final committed file exists and has correct content
    let final_path = PathBuf::from(&finish_resp.saved_path);
    assert!(final_path.exists(), "final committed file must exist");
    assert_eq!(fs::read(&final_path).unwrap(), payload);

    // Verify .part file is removed
    let part_path = FileManager::get_part_path(&temp_dir, "greeting.txt", &token);
    assert!(!part_path.exists(), ".part file must be removed after commit");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_transfer_finish_blake3_hash_mismatch_rejected() {
    let temp_dir = setup_test_dir("hash_mismatch");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    let original_payload = b"Original uncorrupted data".to_vec();
    let expected_hash = FileManager::calculate_blake3_bytes(&original_payload);

    let corrupted_payload = b"Corrupted modified data!!".to_vec();

    let request_payload = TransferRequest::new(
        "req-corrupt",
        "Sender",
        "windows",
        None,
        vec![FileMetadata::new(
            "f-corrupt",
            "corrupt.txt",
            corrupted_payload.len() as u64,
            Some(expected_hash),
        )],
    );

    let handshake_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
        .json(&request_payload)
        .send()
        .await
        .unwrap();
    let handshake: TransferResponse = handshake_res.json().await.unwrap();
    let token = handshake.session_token.unwrap();

    // Upload corrupted data
    let _ = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-corrupt&offset=0",
            port, token
        ))
        .body(corrupted_payload)
        .send()
        .await
        .unwrap();

    // Finish should fail
    let finish_req = FinishRequest::new(&token, "f-corrupt");
    let finish_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/finish", port))
        .json(&finish_req)
        .send()
        .await
        .unwrap();

    assert_eq!(finish_res.status(), 400);

    // Final file should NOT exist
    let dest_path = temp_dir.join("corrupt.txt");
    assert!(!dest_path.exists());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_transfer_finish_size_mismatch_rejected() {
    let temp_dir = setup_test_dir("size_mismatch");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    let request_payload = TransferRequest::new(
        "req-size-mismatch",
        "Sender",
        "windows",
        None,
        vec![FileMetadata::new("f-size", "big.bin", 1000, None)],
    );

    let handshake_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
        .json(&request_payload)
        .send()
        .await
        .unwrap();
    let handshake: TransferResponse = handshake_res.json().await.unwrap();
    let token = handshake.session_token.unwrap();

    // Upload only 500 bytes when 1000 were expected
    let _ = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-size&offset=0",
            port, token
        ))
        .body(vec![0u8; 500])
        .send()
        .await
        .unwrap();

    // Finish should fail
    let finish_req = FinishRequest::new(&token, "f-size");
    let finish_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/finish", port))
        .json(&finish_req)
        .send()
        .await
        .unwrap();

    assert_eq!(finish_res.status(), 400);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_security_invalid_session_token_rejected() {
    let temp_dir = setup_test_dir("security");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    // Invalid token on status query
    let status_res = client
        .get(format!(
            "http://127.0.0.1:{}/api/v1/transfer/status?session_token=fake-token&file_id=f1",
            port
        ))
        .send()
        .await
        .unwrap();
    assert!(status_res.status().is_client_error());

    // Invalid token on chunk upload
    let chunk_res = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token=fake-token&file_id=f1&offset=0",
            port
        ))
        .body(vec![0u8; 10])
        .send()
        .await
        .unwrap();
    assert!(chunk_res.status().is_client_error());

    // Invalid token on finish
    let finish_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/finish", port))
        .json(&FinishRequest::new("fake-token", "f1"))
        .send()
        .await
        .unwrap();
    assert!(finish_res.status().is_client_error());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_transfer_cancel() {
    let temp_dir = setup_test_dir("cancel");
    let state = Arc::new(ServerState::new(sample_device_info(), temp_dir.clone()));
    let (port, _handle) = start_server(state, 0)
        .await
        .expect("failed to start server");

    let client = Client::new();

    let request_payload = TransferRequest::new(
        "req-cancel",
        "Sender",
        "windows",
        None,
        vec![FileMetadata::new("f-cancel", "cancel.bin", 1000, None)],
    );

    let handshake_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/request", port))
        .json(&request_payload)
        .send()
        .await
        .unwrap();
    let handshake: TransferResponse = handshake_res.json().await.unwrap();
    let token = handshake.session_token.unwrap();

    // Upload a chunk
    let _ = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-cancel&offset=0",
            port, token
        ))
        .body(vec![0xCC; 200])
        .send()
        .await
        .unwrap();

    // Send Cancel
    let cancel_res = client
        .post(format!("http://127.0.0.1:{}/api/v1/transfer/cancel", port))
        .json(&CancelRequest::new(&token, Some("User cancelled".to_string())))
        .send()
        .await
        .unwrap();
    assert_eq!(cancel_res.status(), 200);

    // Subsequent chunk should fail (session cancelled)
    let chunk_after_cancel = client
        .post(format!(
            "http://127.0.0.1:{}/api/v1/transfer/chunk?session_token={}&file_id=f-cancel&offset=200",
            port, token
        ))
        .body(vec![0xCC; 200])
        .send()
        .await
        .unwrap();
    assert!(chunk_after_cancel.status().is_client_error());

    let _ = fs::remove_dir_all(&temp_dir);
}
