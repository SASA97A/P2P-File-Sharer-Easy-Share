use std::time::{Duration, Instant};

#[test]
fn test_throughput_tracker_initial_values() {
    use easy_share_lib::client::metrics::ThroughputTracker;

    let mut tracker = ThroughputTracker::new();
    let now = Instant::now();
    let (speed, eta) = tracker.update(0, 1_000_000, now);
    assert_eq!(speed, 0);
    assert_eq!(eta, None);
}

#[test]
fn test_throughput_tracker_speed_and_eta_calculation() {
    use easy_share_lib::client::metrics::ThroughputTracker;

    let mut tracker = ThroughputTracker::with_alpha(1.0); // instant weighting for predictable test
    let t0 = Instant::now();
    tracker.update(0, 1_000_000, t0);

    // 1 second later, 200,000 bytes sent
    let t1 = t0 + Duration::from_secs(1);
    let (speed, eta) = tracker.update(200_000, 1_000_000, t1);

    // Speed should be ~200,000 B/s
    assert_eq!(speed, 200_000);
    // Remaining bytes: 800,000 / 200,000 B/s = 4 seconds
    assert_eq!(eta, Some(4));

    // When bytes complete
    let t2 = t1 + Duration::from_secs(4);
    let (_speed, eta_done) = tracker.update(1_000_000, 1_000_000, t2);
    assert_eq!(eta_done, Some(0));
}

#[test]
fn test_throughput_tracker_zero_speed_eta_none() {
    use easy_share_lib::client::metrics::ThroughputTracker;

    let mut tracker = ThroughputTracker::with_alpha(1.0);
    let t0 = Instant::now();
    tracker.update(0, 500_000, t0);

    let t1 = t0 + Duration::from_secs(1);
    // No bytes sent
    let (speed, eta) = tracker.update(0, 500_000, t1);
    assert_eq!(speed, 0);
    assert_eq!(eta, None);
}

#[test]
fn test_progress_payload_with_metrics_serialization() {
    use easy_share_lib::commands::ProgressPayload;

    let payload = ProgressPayload {
        session_token: "tok-123".to_string(),
        file_id: "f-1".to_string(),
        file_name: "test.zip".to_string(),
        sent_bytes: 50_000,
        total_bytes: 100_000,
        percent: 50,
        speed_bps: 25_000,
        eta_seconds: Some(2),
    };

    let json = serde_json::to_string(&payload).expect("Failed to serialize");
    assert!(json.contains("\"speed_bps\":25000"));
    assert!(json.contains("\"eta_seconds\":2"));

    let deserialized: ProgressPayload = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized, payload);
}

#[tokio::test]
async fn test_transfer_control_pause_and_resume() {
    use easy_share_lib::client::sender::TransferControl;

    let ctrl = TransferControl::new();
    assert!(!ctrl.is_paused());
    assert!(!ctrl.is_cancelled());

    ctrl.pause();
    assert!(ctrl.is_paused());

    ctrl.resume();
    assert!(!ctrl.is_paused());

    ctrl.cancel();
    assert!(ctrl.is_cancelled());
}

#[tokio::test]
async fn test_sender_chunk_streaming_pauses_and_resumes() {
    use easy_share_lib::client::sender::{FileToSend, TransferClient, TransferControl};
    use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
    use easy_share_lib::models::transfer::TransferRequest;
    use easy_share_lib::server::{start_server, ServerState};
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let test_dir = std::env::temp_dir().join("easy_share_test_pause_resume");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let rx_info = DeviceInfo::new("Receiver-Pause", "desktop", "windows", "2.0.0", 0);
    let rx_state = Arc::new(ServerState::new(rx_info, test_dir.join("rx")));
    let (rx_port, _rx_task) = start_server(rx_state.clone(), 0).await.unwrap();

    let peer = PeerInfo::new("Receiver-Pause", "desktop", "windows", "127.0.0.1", rx_port);

    // Prepare a 2MB file sent in 256KB chunks (8 chunks total)
    let src_file = test_dir.join("payload.bin");
    let data = vec![0xEEu8; 2 * 1024 * 1024];
    fs::write(&src_file, &data).unwrap();

    let file_to_send = FileToSend::from_path("f_pause", &src_file).unwrap();
    let req = TransferRequest::new("req_p", "Sender-Pause", "desktop", vec![file_to_send.to_metadata()]);

    let client = TransferClient::with_chunk_size(256 * 1024);
    let resp = client.request_transfer(&peer, &req).await.unwrap();
    let token = resp.session_token.unwrap();

    let ctrl = Arc::new(TransferControl::new());
    let ctrl_for_task = ctrl.clone();
    let ctrl_in_cb = ctrl.clone();

    let paused_flag = Arc::new(AtomicBool::new(false));
    let paused_flag_clone = paused_flag.clone();

    let client_clone = client.clone();
    let peer_clone = peer.clone();
    let token_clone = token.clone();
    let file_clone = file_to_send.clone();

    let transfer_task = tokio::spawn(async move {
        client_clone
            .send_file_resumable_controlled(
                &peer_clone,
                &token_clone,
                &file_clone,
                Some(&ctrl_for_task),
                move |sent, _total| {
                    if sent >= 512 * 1024 && !paused_flag_clone.load(Ordering::SeqCst) {
                        // Pause when 512 KB has been sent
                        paused_flag_clone.store(true, Ordering::SeqCst);
                        ctrl_in_cb.pause();
                    }
                },
            )
            .await
    });

    // Wait until pause flag is tripped
    while !paused_flag.load(Ordering::SeqCst) {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Assert that transfer control is paused
    assert!(ctrl.is_paused());

    // Give it a short sleep to ensure it stays paused
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    assert!(!transfer_task.is_finished(), "Transfer must still be paused and not finished");

    // Resume the transfer
    ctrl.resume();
    assert!(!ctrl.is_paused());

    // Await completion
    let send_res = transfer_task.await.unwrap();
    assert!(send_res.is_ok(), "Transfer should finish successfully after resuming");

    let finish_res = client.finish_transfer(&peer, &token, &file_to_send.file_id).await.unwrap();
    assert_eq!(finish_res.file_id, "f_pause");

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_sender_chunk_streaming_respects_speed_limit() {
    use std::fs;
    use std::sync::Arc;
    use std::time::Instant;
    use easy_share_lib::client::sender::{FileToSend, TransferClient, TransferControl};
    use easy_share_lib::models::peer::{DeviceInfo, PeerInfo};
    use easy_share_lib::models::transfer::TransferRequest;
    use easy_share_lib::server::routes::start_server;
    use easy_share_lib::server::state::ServerState;

    let test_dir = std::env::temp_dir().join("easy_share_test_speed_limit_streaming");
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    let rx_info = DeviceInfo::new("Receiver-Throttle", "desktop", "windows", "2.0.0", 0);
    let rx_state = Arc::new(ServerState::new(rx_info, test_dir.join("rx")));
    let (rx_port, _rx_task) = start_server(rx_state.clone(), 0).await.unwrap();

    let peer = PeerInfo::new("Receiver-Throttle", "desktop", "windows", "127.0.0.1", rx_port);

    // 256 KB file sent in 64 KB chunks (4 chunks)
    let src_file = test_dir.join("throttled.bin");
    let data = vec![0xBBu8; 256 * 1024];
    fs::write(&src_file, &data).unwrap();

    let file_to_send = FileToSend::from_path("f_throttle", &src_file).unwrap();
    let req = TransferRequest::new("req_t", "Sender-Throttle", "desktop", vec![file_to_send.to_metadata()]);

    let client = TransferClient::with_chunk_size(64 * 1024);
    let resp = client.request_transfer(&peer, &req).await.unwrap();
    let token = resp.session_token.unwrap();

    // 400 KB/s: 256 KB should take at least ~500ms
    let ctrl = TransferControl::new().with_speed_limit(400 * 1024);

    let start_time = Instant::now();
    let res = client
        .send_file_resumable_controlled(
            &peer,
            &token,
            &file_to_send,
            Some(&ctrl),
            |_sent, _total| {},
        )
        .await;

    let elapsed = start_time.elapsed();
    assert!(res.is_ok(), "Transfer should succeed with throttling: {:?}", res);
    assert!(
        elapsed >= std::time::Duration::from_millis(400),
        "Expected elapsed time >= 400ms due to speed limit, got {:?}",
        elapsed
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_app_state_pause_and_resume_lifecycle() {
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::client::TransferControl;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::ServerState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let test_dir = std::env::temp_dir().join("easy_share_test_app_state_pause");
    let info = DeviceInfo::new("TestNode", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, test_dir.clone()));

    let app_state = AppState::new(
        server_state,
        None,
        test_dir.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );

    let token = "session-pause-123";
    let ctrl = TransferControl::new();

    app_state.register_transfer_control(token, ctrl).await;
    assert!(!app_state.is_transfer_paused(token).await);

    let paused = app_state.pause_transfer(token).await;
    assert!(paused);
    assert!(app_state.is_transfer_paused(token).await);

    let resumed = app_state.resume_transfer(token).await;
    assert!(resumed);
    assert!(!app_state.is_transfer_paused(token).await);

    let cancelled = app_state.cancel_session(token).await;
    assert!(cancelled);
    assert!(!app_state.is_transfer_paused(token).await);
}

#[test]
fn test_transfer_control_speed_limit_get_set() {
    use easy_share_lib::client::sender::TransferControl;

    let ctrl = TransferControl::new();
    assert_eq!(ctrl.speed_limit(), 0);

    ctrl.set_speed_limit(10_000_000);
    assert_eq!(ctrl.speed_limit(), 10_000_000);

    let ctrl2 = TransferControl::new().with_speed_limit(5_000_000);
    assert_eq!(ctrl2.speed_limit(), 5_000_000);
}

#[tokio::test]
async fn test_app_state_speed_limit_updates_active_controls() {
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::client::sender::TransferControl;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::state::ServerState;

    let test_dir = std::env::temp_dir().join("easy_share_test_app_state_speed_limit");
    let info = DeviceInfo::new("TestNode", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, test_dir.clone()));

    let app_state = AppState::new(
        server_state,
        None,
        test_dir.clone(),
        Arc::new(RwLock::new(HashMap::new())),
    );

    assert_eq!(app_state.get_speed_limit().await, 0);

    let token = "session-speed-limit-123";
    let ctrl = TransferControl::new();
    app_state.register_transfer_control(token, ctrl.clone()).await;

    app_state.set_speed_limit(5_242_880).await;
    assert_eq!(app_state.get_speed_limit().await, 5_242_880);
    assert_eq!(ctrl.speed_limit(), 5_242_880);
}



