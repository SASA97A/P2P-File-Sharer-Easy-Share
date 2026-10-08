use easy_share_lib::client::{ClientError, FileToSend, TransferClient};
use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
use easy_share_lib::models::transfer::{FileMetadata, TransferRequest, TransferStatus};
use easy_share_lib::server::{start_server, ConsentPolicy, ConsentRequest, ServerState};
use easy_share_lib::storage::file_manager::FileManager;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

fn setup_e2e_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_e2e_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create e2e temp dir");
    dir
}

fn create_deterministic_test_file(path: &PathBuf, size_bytes: usize, seed: u8) -> String {
    let mut data = Vec::with_capacity(size_bytes);
    for i in 0..size_bytes {
        data.push(((i as u64 * 31 + seed as u64) % 256) as u8);
    }
    fs::write(path, &data).expect("failed to write test file");
    FileManager::calculate_blake3_file(path).expect("failed to calculate blake3")
}

fn create_node_device(name: &str, room: Option<&str>, port: u16) -> DeviceInfo {
    DeviceInfo::new(
        name,
        "desktop",
        "windows",
        "2.0.0",
        room.map(|s| s.to_string()),
        port,
    )
}

/// Helper struct representing an in-process running node
struct RunningNode {
    pub name: String,
    pub device_info: DeviceInfo,
    pub server_state: Arc<ServerState>,
    pub port: u16,
    pub download_dir: PathBuf,
    pub consent_rx: Option<mpsc::Receiver<ConsentRequest>>,
    _handle: tokio::task::JoinHandle<()>,
}

impl RunningNode {
    pub async fn start_with_consent_channel(
        name: &str,
        room: Option<&str>,
        dir_prefix: &str,
    ) -> Self {
        let download_dir = setup_e2e_dir(dir_prefix);
        let device_info = create_node_device(name, room, 0);

        let (consent_tx, consent_rx) = mpsc::channel::<ConsentRequest>(32);
        let consent_policy = ConsentPolicy::Channel(consent_tx);

        let server_state = Arc::new(ServerState::with_consent(
            device_info.clone(),
            download_dir.clone(),
            consent_policy,
        ));

        let (port, handle) = start_server(server_state.clone(), 0)
            .await
            .expect("node server failed to start on dynamic port");

        Self {
            name: name.to_string(),
            device_info: server_state.get_device_info().await,
            server_state,
            port,
            download_dir,
            consent_rx: Some(consent_rx),
            _handle: handle,
        }
    }

    pub async fn start_auto_accept(name: &str, room: Option<&str>, dir_prefix: &str) -> Self {
        let download_dir = setup_e2e_dir(dir_prefix);
        let device_info = create_node_device(name, room, 0);

        let server_state = Arc::new(ServerState::new(device_info.clone(), download_dir.clone()));

        let (port, handle) = start_server(server_state.clone(), 0)
            .await
            .expect("node server failed to start on dynamic port");

        Self {
            name: name.to_string(),
            device_info: server_state.get_device_info().await,
            server_state,
            port,
            download_dir,
            consent_rx: None,
            _handle: handle,
        }
    }

    pub fn to_peer_info(&self) -> PeerInfo {
        PeerInfo::new(
            &self.name,
            "desktop",
            "windows",
            self.device_info.room_id.clone(),
            "127.0.0.1",
            self.port,
        )
    }

    pub fn cleanup(self) {
        let _ = fs::remove_dir_all(self.download_dir);
    }
}

/// E2E Test 1: Full multi-file transfer with interactive consent acceptance,
/// chunk streaming, BLAKE3 verification, and temporary .part cleanup.
#[tokio::test]
async fn test_e2e_multi_file_transfer_with_interactive_consent() {
    let node_a_sender_dir = setup_e2e_dir("sender_node_a");
    let mut node_b_receiver = RunningNode::start_with_consent_channel(
        "Receiver-Node-B",
        Some("Engineering-Lab"),
        "receiver_node_b",
    )
    .await;

    // 1. Discovery Simulation: Sender queries receiver node info endpoint
    let client_a = TransferClient::with_chunk_size(512 * 1024); // 512 KB chunks
    let peer_b = node_b_receiver.to_peer_info();

    let probed_info = client_a
        .probe_peer(&peer_b)
        .await
        .expect("Node A failed to probe Node B");
    assert_eq!(probed_info.device_name, "Receiver-Node-B");
    assert_eq!(probed_info.room_id, Some("Engineering-Lab".to_string()));
    assert_eq!(probed_info.port, node_b_receiver.port);

    // 2. Prepare 3 distinct files of varying sizes on Node A
    // File 1: 4 MB multi-megabyte binary payload
    let f1_path = node_a_sender_dir.join("large_binary.bin");
    let f1_size = 4 * 1024 * 1024;
    let f1_hash = create_deterministic_test_file(&f1_path, f1_size, 0x1A);

    // File 2: 256 KB document
    let f2_path = node_a_sender_dir.join("specification.pdf");
    let f2_size = 256 * 1024;
    let f2_hash = create_deterministic_test_file(&f2_path, f2_size, 0x2B);

    // File 3: 1 KB configuration / text file
    let f3_path = node_a_sender_dir.join("config.json");
    let f3_size = 1024;
    let f3_hash = create_deterministic_test_file(&f3_path, f3_size, 0x3C);

    let file_to_send_1 = FileToSend::from_path("f-001", &f1_path).unwrap();
    let file_to_send_2 = FileToSend::from_path("f-002", &f2_path).unwrap();
    let file_to_send_3 = FileToSend::from_path("f-003", &f3_path).unwrap();

    let files_to_send = vec![file_to_send_1, file_to_send_2, file_to_send_3];

    // 3. Initiate Transfer Request Handshake from Node A to Node B
    let metadata_list: Vec<FileMetadata> = files_to_send.iter().map(|f| f.to_metadata()).collect();
    let transfer_req = TransferRequest::new(
        "req-e2e-001",
        "Sender-Node-A",
        "windows",
        Some("Engineering-Lab".to_string()),
        metadata_list,
    );

    let client_a_clone = client_a.clone();
    let peer_b_clone = peer_b.clone();
    let req_clone = transfer_req.clone();

    let handshake_task = tokio::spawn(async move {
        client_a_clone
            .request_transfer(&peer_b_clone, &req_clone)
            .await
    });

    // 4. Node B receives consent prompt via its channel and grants consent
    let mut rx = node_b_receiver.consent_rx.take().unwrap();
    let consent_req = rx
        .recv()
        .await
        .expect("Node B did not receive consent request");

    assert_eq!(consent_req.transfer_request.sender_name, "Sender-Node-A");
    assert_eq!(consent_req.transfer_request.files.len(), 3);
    assert_eq!(
        consent_req.transfer_request.files[0].blake3_hash.as_deref(),
        Some(f1_hash.as_str())
    );

    // Accept transfer
    consent_req
        .responder
        .send(true)
        .expect("failed to send consent response");

    let transfer_response = handshake_task
        .await
        .expect("handshake task panicked")
        .expect("handshake failed");

    assert_eq!(transfer_response.status, TransferStatus::Accepted);
    let session_token = transfer_response
        .session_token
        .expect("expected session token");
    assert!(!session_token.is_empty());

    // 5. Node A transmits all chunks for each file and finishes each
    let total_progress_reports = Arc::new(AtomicU64::new(0));

    for file in &files_to_send {
        let progress_counter = total_progress_reports.clone();
        client_a
            .send_file_resumable(&peer_b, &session_token, file, move |_sent, _total| {
                progress_counter.fetch_add(1, Ordering::SeqCst);
            })
            .await
            .expect("failed to stream file chunks");

        let finish_resp = client_a
            .finish_transfer(&peer_b, &session_token, &file.file_id)
            .await
            .expect("failed to finalize file transfer");

        assert_eq!(finish_resp.status, TransferStatus::Completed);
        assert_eq!(finish_resp.file_id, file.file_id);
    }

    assert!(total_progress_reports.load(Ordering::SeqCst) >= 8);

    // 6. Verify Node B's downloaded directory
    let b_f1 = node_b_receiver.download_dir.join("large_binary.bin");
    let b_f2 = node_b_receiver.download_dir.join("specification.pdf");
    let b_f3 = node_b_receiver.download_dir.join("config.json");

    assert!(b_f1.exists(), "File 1 does not exist in Node B download dir");
    assert!(b_f2.exists(), "File 2 does not exist in Node B download dir");
    assert!(b_f3.exists(), "File 3 does not exist in Node B download dir");

    assert_eq!(FileManager::get_file_size(&b_f1).unwrap(), f1_size as u64);
    assert_eq!(FileManager::get_file_size(&b_f2).unwrap(), f2_size as u64);
    assert_eq!(FileManager::get_file_size(&b_f3).unwrap(), f3_size as u64);

    assert_eq!(FileManager::calculate_blake3_file(&b_f1).unwrap(), f1_hash);
    assert_eq!(FileManager::calculate_blake3_file(&b_f2).unwrap(), f2_hash);
    assert_eq!(FileManager::calculate_blake3_file(&b_f3).unwrap(), f3_hash);

    // Verify no leftover .part files exist
    let entries = fs::read_dir(&node_b_receiver.download_dir).unwrap();
    let mut part_count = 0;
    for entry in entries {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        if name.ends_with(".part") {
            part_count += 1;
        }
    }
    assert_eq!(part_count, 0, "Leftover .part files found in receiver dir");

    // Cleanup
    let _ = fs::remove_dir_all(&node_a_sender_dir);
    node_b_receiver.cleanup();
}

/// E2E Test 2: Resumption after simulated network interruption
/// Sends a partial chunk payload, disconnects, queries transfer status, and resumes to completion.
#[tokio::test]
async fn test_e2e_transfer_interruption_and_resumption() {
    let node_a_sender_dir = setup_e2e_dir("resume_sender");
    let node_b_receiver =
        RunningNode::start_auto_accept("Receiver-Node-B", None, "resume_receiver").await;

    let client_a = TransferClient::with_chunk_size(512 * 1024); // 512 KB chunk size
    let peer_b = node_b_receiver.to_peer_info();

    // Create 6 MB binary payload
    let f_path = node_a_sender_dir.join("dataset.raw");
    let f_size = 6 * 1024 * 1024; // 6 MB
    let expected_hash = create_deterministic_test_file(&f_path, f_size, 0x4D);
    let file_to_send = FileToSend::from_path("f-resume-01", &f_path).unwrap();

    // Handshake
    let req = TransferRequest::new(
        "req-resume",
        "Sender-Node-A",
        "windows",
        None,
        vec![file_to_send.to_metadata()],
    );

    let resp = client_a
        .request_transfer(&peer_b, &req)
        .await
        .expect("handshake failed");
    let session_token = resp.session_token.expect("expected session token");

    // Phase 1: Upload only the first 2.5 MB (5 chunks * 512 KB) then stop abruptly
    let interrupted_limit = (2.5 * 1024.0 * 1024.0) as u64; // 2,621,440 bytes
    let bytes_sent_p1 = client_a
        .send_file_chunks_with_limit(
            &peer_b.base_url(),
            &session_token,
            &file_to_send,
            0,
            Some(interrupted_limit),
            |_, _| {},
        )
        .await
        .expect("phase 1 upload failed");

    assert_eq!(bytes_sent_p1, interrupted_limit);

    // Phase 2: Interruption check - query receiver status endpoint
    let status_res = client_a
        .get_transfer_status(&peer_b, &session_token, &file_to_send.file_id)
        .await
        .expect("failed to query status");

    assert_eq!(status_res.bytes_received, interrupted_limit);
    assert_eq!(status_res.status, TransferStatus::Partial);

    // Phase 3: Resume transfer from the reported offset to end of file
    let resumed_reports = Arc::new(AtomicU64::new(0));
    let resumed_clone = resumed_reports.clone();

    client_a
        .send_file_resumable(&peer_b, &session_token, &file_to_send, move |sent, total| {
            resumed_clone.fetch_add(1, Ordering::SeqCst);
            assert!(sent >= interrupted_limit, "sent byte offset should start >= interrupted offset");
            assert_eq!(total, f_size as u64);
        })
        .await
        .expect("resumed transfer failed");

    assert!(resumed_reports.load(Ordering::SeqCst) >= 7);

    // Phase 4: Finalize file transfer and verify receiver commits full file
    let finish_resp = client_a
        .finish_transfer(&peer_b, &session_token, &file_to_send.file_id)
        .await
        .expect("finish_transfer failed");

    assert_eq!(finish_resp.status, TransferStatus::Completed);

    let committed_file = node_b_receiver.download_dir.join("dataset.raw");
    assert!(committed_file.exists(), "committed file does not exist");
    assert_eq!(
        FileManager::get_file_size(&committed_file).unwrap(),
        f_size as u64
    );
    assert_eq!(
        FileManager::calculate_blake3_file(&committed_file).unwrap(),
        expected_hash
    );

    // Verify .part cleanup
    let part_file = FileManager::get_part_path(
        &node_b_receiver.download_dir,
        "dataset.raw",
        &session_token,
    );
    assert!(!part_file.exists(), "part file must be removed after commit");

    // Cleanup
    let _ = fs::remove_dir_all(&node_a_sender_dir);
    node_b_receiver.cleanup();
}

/// E2E Test 3: Consent rejection
/// Node B rejects the transfer request; transfer is aborted immediately with zero files written.
#[tokio::test]
async fn test_e2e_transfer_consent_rejection() {
    let node_a_sender_dir = setup_e2e_dir("decline_sender");
    let mut node_b_receiver = RunningNode::start_with_consent_channel(
        "Receiver-Node-B",
        None,
        "decline_receiver",
    )
    .await;

    let client_a = TransferClient::new();
    let peer_b = node_b_receiver.to_peer_info();

    let f_path = node_a_sender_dir.join("confidential.docx");
    let f_hash = create_deterministic_test_file(&f_path, 64 * 1024, 0x99);
    let file_to_send = FileToSend::from_path("f-decline", &f_path).unwrap();

    let req = TransferRequest::new(
        "req-decline-01",
        "Sender-Node-A",
        "windows",
        None,
        vec![file_to_send.to_metadata()],
    );

    let client_a_clone = client_a.clone();
    let peer_b_clone = peer_b.clone();
    let req_clone = req.clone();

    let handshake_task = tokio::spawn(async move {
        client_a_clone
            .request_transfer(&peer_b_clone, &req_clone)
            .await
    });

    // Node B receives prompt and declines
    let mut rx = node_b_receiver.consent_rx.take().unwrap();
    let consent_req = rx
        .recv()
        .await
        .expect("did not receive consent request");

    assert_eq!(consent_req.transfer_request.sender_name, "Sender-Node-A");
    consent_req
        .responder
        .send(false) // Decline transfer
        .expect("failed to send decline response");

    let transfer_response = handshake_task
        .await
        .expect("handshake task panicked")
        .expect("handshake response expected");

    assert_eq!(transfer_response.status, TransferStatus::Declined);
    assert!(transfer_response.session_token.is_none());

    // Verify receiver directory remains completely empty
    let entries: Vec<_> = fs::read_dir(&node_b_receiver.download_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entries.len(), 0, "No files or .part files should be created on decline");

    // Cleanup
    let _ = fs::remove_dir_all(&node_a_sender_dir);
    node_b_receiver.cleanup();
}

/// E2E Test 4: Transfer cancellation mid-stream cleans up temporary artifacts
#[tokio::test]
async fn test_e2e_transfer_cancel_mid_stream_cleanup() {
    let node_a_sender_dir = setup_e2e_dir("cancel_sender");
    let node_b_receiver =
        RunningNode::start_auto_accept("Receiver-Node-B", None, "cancel_receiver").await;

    let client_a = TransferClient::with_chunk_size(512 * 1024);
    let peer_b = node_b_receiver.to_peer_info();

    let f_path = node_a_sender_dir.join("large_archive.zip");
    let f_size = 4 * 1024 * 1024;
    let _ = create_deterministic_test_file(&f_path, f_size, 0xEE);
    let file_to_send = FileToSend::from_path("f-cancel-01", &f_path).unwrap();

    let req = TransferRequest::new(
        "req-cancel-01",
        "Sender-Node-A",
        "windows",
        None,
        vec![file_to_send.to_metadata()],
    );

    let resp = client_a
        .request_transfer(&peer_b, &req)
        .await
        .expect("handshake failed");
    let session_token = resp.session_token.expect("expected session token");

    // Send 1 chunk (512 KB)
    let _ = client_a
        .send_file_chunks_with_limit(
            &peer_b.base_url(),
            &session_token,
            &file_to_send,
            0,
            Some(512 * 1024),
            |_, _| {},
        )
        .await
        .expect("first chunk failed");

    // Cancel transfer
    client_a
        .cancel_transfer(&peer_b, &session_token, Some("User cancelled file".to_string()))
        .await
        .expect("cancel_transfer failed");

    // Subsequent chunk upload attempt should be rejected
    let next_chunk_res = client_a
        .send_chunk(
            &peer_b,
            &session_token,
            &file_to_send.file_id,
            512 * 1024,
            vec![1, 2, 3, 4],
        )
        .await;
    assert!(next_chunk_res.is_err(), "sending chunk after cancel should fail");

    // Final file should not exist
    let committed_file = node_b_receiver.download_dir.join("large_archive.zip");
    assert!(!committed_file.exists());

    // Cleanup
    let _ = fs::remove_dir_all(&node_a_sender_dir);
    node_b_receiver.cleanup();
}

/// E2E Test 5: Peer discovery, room filtering, and multi-node communication
#[tokio::test]
async fn test_e2e_peer_info_and_room_filtering() {
    let node_alpha1 =
        RunningNode::start_auto_accept("Alpha-1", Some("DesignTeam"), "alpha1").await;
    let node_alpha2 =
        RunningNode::start_auto_accept("Alpha-2", Some("DesignTeam"), "alpha2").await;
    let node_beta =
        RunningNode::start_auto_accept("Beta-1", Some("FinanceTeam"), "beta1").await;

    let client = TransferClient::new();

    let peer_a1 = node_alpha1.to_peer_info();
    let peer_a2 = node_alpha2.to_peer_info();
    let peer_b = node_beta.to_peer_info();

    let info_a1 = client.probe_peer(&peer_a1).await.unwrap();
    let info_a2 = client.probe_peer(&peer_a2).await.unwrap();
    let info_b = client.probe_peer(&peer_b).await.unwrap();

    assert_eq!(info_a1.room_id, Some("DesignTeam".to_string()));
    assert_eq!(info_a2.room_id, Some("DesignTeam".to_string()));
    assert_eq!(info_b.room_id, Some("FinanceTeam".to_string()));

    // Verify room filtering logic (peers in same room vs different room)
    let my_room = Some("DesignTeam".to_string());
    let all_peers = vec![info_a1, info_a2, info_b];

    let filtered_peers: Vec<_> = all_peers
        .into_iter()
        .filter(|p| match (&my_room, &p.room_id) {
            (Some(my_r), Some(peer_r)) => my_r == peer_r,
            (None, None) => true,
            _ => false,
        })
        .collect();

    assert_eq!(filtered_peers.len(), 2);
    assert!(filtered_peers.iter().any(|p| p.device_name == "Alpha-1"));
    assert!(filtered_peers.iter().any(|p| p.device_name == "Alpha-2"));
    assert!(!filtered_peers.iter().any(|p| p.device_name == "Beta-1"));

    node_alpha1.cleanup();
    node_alpha2.cleanup();
    node_beta.cleanup();
}
