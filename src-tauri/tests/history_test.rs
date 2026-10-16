use easy_share_lib::storage::history::{
    clear_history, delete_record, load_history, save_history, HistoryFileItem, HistoryStatus,
    TransferDirection, TransferHistoryRecord,
};
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_test_history_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temp test dir");
    dir
}

fn sample_record(id: &str, file_name: &str, dir: TransferDirection) -> TransferHistoryRecord {
    TransferHistoryRecord {
        id: id.to_string(),
        session_token: Some(format!("token-{}", id)),
        timestamp: 1700000000000,
        direction: dir,
        peer_name: "Pixel-7-Pro".to_string(),
        peer_ip: "192.168.1.55".to_string(),
        status: HistoryStatus::Completed,
        files: vec![HistoryFileItem {
            name: file_name.to_string(),
            path: Some(format!("C:\\Downloads\\{}", file_name)),
            size: 1048576,
        }],
        total_bytes: 1048576,
        error_message: None,
    }
}

#[test]
fn test_load_history_defaults_when_nonexistent() {
    let dir = setup_test_dir("defaults");
    let records = load_history(&dir);
    assert!(records.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_save_and_load_history() {
    let dir = setup_test_dir("save_load");
    let r1 = sample_record("rec-1", "doc.pdf", TransferDirection::Received);
    let r2 = sample_record("rec-2", "photo.png", TransferDirection::Sent);

    let save_res = save_history(&dir, &[r1.clone(), r2.clone()]);
    assert!(save_res.is_ok(), "Failed to save history: {:?}", save_res);

    let loaded = load_history(&dir);
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].id, "rec-1");
    assert_eq!(loaded[0].direction, TransferDirection::Received);
    assert_eq!(loaded[1].id, "rec-2");
    assert_eq!(loaded[1].direction, TransferDirection::Sent);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_add_record_prepends_newest() {
    use easy_share_lib::storage::history::add_record;

    let dir = setup_test_dir("add_record");
    let r1 = sample_record("rec-first", "first.zip", TransferDirection::Received);
    let r2 = sample_record("rec-second", "second.zip", TransferDirection::Sent);

    assert!(add_record(&dir, r1).is_ok());
    assert!(add_record(&dir, r2).is_ok());

    let loaded = load_history(&dir);
    assert_eq!(loaded.len(), 2);
    // Newest must be first (index 0)
    assert_eq!(loaded[0].id, "rec-second");
    assert_eq!(loaded[1].id, "rec-first");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_delete_record() {
    use easy_share_lib::storage::history::add_record;

    let dir = setup_test_dir("delete_record");
    let r1 = sample_record("rec-1", "file1.txt", TransferDirection::Received);
    let r2 = sample_record("rec-2", "file2.txt", TransferDirection::Sent);

    assert!(add_record(&dir, r1).is_ok());
    assert!(add_record(&dir, r2).is_ok());

    let delete_res = delete_record(&dir, "rec-1");
    assert!(delete_res.is_ok());

    let loaded = load_history(&dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, "rec-2");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_clear_history() {
    use easy_share_lib::storage::history::add_record;

    let dir = setup_test_dir("clear_history");
    let r1 = sample_record("rec-1", "file1.txt", TransferDirection::Received);
    assert!(add_record(&dir, r1).is_ok());

    let clear_res = clear_history(&dir);
    assert!(clear_res.is_ok());

    let loaded = load_history(&dir);
    assert!(loaded.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_load_history_corrupted_json_returns_empty() {
    let dir = setup_test_dir("corrupted");
    let file_path = dir.join("history.json");
    fs::write(&file_path, "{ broken invalid json }").unwrap();

    let loaded = load_history(&dir);
    assert!(loaded.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_app_state_history_workflow() {
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::state::ServerState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let dir = setup_test_dir("app_state_history");
    let download_dir = dir.join("downloads");
    fs::create_dir_all(&download_dir).unwrap();

    let info = DeviceInfo::new("History-Host", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, download_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));
    let app_state = AppState::with_config_dir(
        server_state,
        None,
        download_dir,
        dir.clone(),
        pending_consents,
    );

    // Initial history empty
    assert!(app_state.get_history().is_empty());

    // Add record
    let rec1 = sample_record("rec-101", "report.xlsx", TransferDirection::Received);
    let add_res = app_state.add_history_record(rec1.clone());
    assert!(add_res.is_ok());

    let rec2 = sample_record("rec-102", "archive.tar", TransferDirection::Sent);
    let _ = app_state.add_history_record(rec2);

    let current = app_state.get_history();
    assert_eq!(current.len(), 2);
    assert_eq!(current[0].id, "rec-102");
    assert_eq!(current[1].id, "rec-101");

    // Delete record
    let del_res = app_state.delete_history_record("rec-101");
    assert_eq!(del_res, Ok(true));
    assert_eq!(app_state.get_history().len(), 1);

    // Clear history
    let clear_res = app_state.clear_history();
    assert!(clear_res.is_ok());
    assert!(app_state.get_history().is_empty());

    let _ = fs::remove_dir_all(&dir);
}
