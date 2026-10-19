use easy_share_lib::storage::settings::{load_settings, save_settings, AppSettings};
use std::fs;
use std::path::PathBuf;

fn setup_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("easy_share_test_settings_{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("failed to create temp test dir");
    dir
}

#[test]
fn test_load_settings_defaults_when_nonexistent() {
    let dir = setup_test_dir("defaults");
    let settings = load_settings(&dir);
    assert_eq!(settings.device_name, None);
    assert_eq!(settings.download_dir, None);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_save_and_load_settings() {
    let dir = setup_test_dir("save_load");
    let settings = AppSettings {
        device_name: Some("Work-Laptop-Pro".to_string()),
        download_dir: Some(dir.join("MyDownloads").to_string_lossy().to_string()),
        speed_limit_bps: Some(5_242_880),
        minimize_to_tray: None,
        notifications_enabled: None,
        encryption_enabled: None,
    };

    let save_res = save_settings(&dir, &settings);
    assert!(save_res.is_ok(), "Failed to save settings: {:?}", save_res);

    let loaded = load_settings(&dir);
    assert_eq!(loaded.device_name, Some("Work-Laptop-Pro".to_string()));
    assert_eq!(
        loaded.download_dir,
        Some(dir.join("MyDownloads").to_string_lossy().to_string())
    );
    assert_eq!(loaded.speed_limit_bps, Some(5_242_880));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_load_settings_corrupted_json_returns_default() {
    let dir = setup_test_dir("corrupted");
    let file_path = dir.join("settings.json");
    fs::write(&file_path, "{ broken invalid json }").unwrap();

    let loaded = load_settings(&dir);
    assert_eq!(loaded.device_name, None);
    assert_eq!(loaded.download_dir, None);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_app_state_set_download_dir_persists() {
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::state::ServerState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let dir = setup_test_dir("app_state_download_dir");
    let initial_download_dir = dir.join("initial_downloads");
    let new_download_dir = dir.join("custom_downloads");
    fs::create_dir_all(&initial_download_dir).unwrap();

    let info = DeviceInfo::new("Original-Device", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, initial_download_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));
    let app_state = AppState::with_config_dir(
        server_state.clone(),
        None,
        initial_download_dir.clone(),
        dir.clone(),
        pending_consents,
    );

    assert_eq!(app_state.get_download_dir(), initial_download_dir);
    assert_eq!(server_state.download_dir(), initial_download_dir);

    // Set new download directory
    let res = app_state.set_download_dir(new_download_dir.to_string_lossy().to_string()).await;
    assert!(res.is_ok(), "Expected set_download_dir to succeed: {:?}", res);
    assert_eq!(app_state.get_download_dir(), new_download_dir);
    assert_eq!(server_state.download_dir(), new_download_dir);
    assert!(new_download_dir.is_dir());

    // Verify settings were persisted
    let settings = load_settings(&dir);
    assert_eq!(
        settings.download_dir,
        Some(new_download_dir.to_string_lossy().to_string())
    );

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_app_state_set_device_name_validations_and_persists() {
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::state::ServerState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let dir = setup_test_dir("app_state_device_name");
    let download_dir = dir.join("downloads");
    fs::create_dir_all(&download_dir).unwrap();

    let info = DeviceInfo::new("Original-Device", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, download_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));
    let app_state = AppState::with_config_dir(
        server_state.clone(),
        None,
        download_dir.clone(),
        dir.clone(),
        pending_consents,
    );

    // 1. Invalid names should fail
    assert!(app_state.set_device_name("   ".to_string()).await.is_err());
    assert!(app_state.set_device_name("".to_string()).await.is_err());
    assert!(app_state.set_device_name("a".repeat(33)).await.is_err());

    // 2. Valid name succeeds
    let res = app_state.set_device_name("Safar's PC".to_string()).await;
    assert!(res.is_ok(), "Expected set_device_name to succeed: {:?}", res);

    let updated_info = app_state.get_device_info().await;
    assert_eq!(updated_info.device_name, "Safar's PC");

    // 3. Verify persisted in settings
    let settings = load_settings(&dir);
    assert_eq!(settings.device_name, Some("Safar's PC".to_string()));

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_settings_tray_and_notifications_defaults_and_persistence() {
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::state::ServerState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let dir = setup_test_dir("tray_notifications_settings");
    let download_dir = dir.join("downloads");
    fs::create_dir_all(&download_dir).unwrap();

    // 1. Defaults should be true when missing from settings.json
    let settings = load_settings(&dir);
    assert!(settings.minimize_to_tray());
    assert!(settings.notifications_enabled());

    let info = DeviceInfo::new("Test-Device", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, download_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));
    let app_state = AppState::with_config_dir(
        server_state.clone(),
        None,
        download_dir.clone(),
        dir.clone(),
        pending_consents,
    );

    let (min_to_tray, notifs) = app_state.get_tray_settings().await;
    assert!(min_to_tray);
    assert!(notifs);

    // 2. Set to false and verify persistence
    app_state.set_tray_settings(false, false).await.unwrap();

    let (min_to_tray2, notifs2) = app_state.get_tray_settings().await;
    assert!(!min_to_tray2);
    assert!(!notifs2);

    let loaded = load_settings(&dir);
    assert_eq!(loaded.minimize_to_tray, Some(false));
    assert_eq!(loaded.notifications_enabled, Some(false));
    assert!(!loaded.minimize_to_tray());
    assert!(!loaded.notifications_enabled());

    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_settings_encryption_default_and_persistence() {
    use easy_share_lib::app_state::AppState;
    use easy_share_lib::models::peer::DeviceInfo;
    use easy_share_lib::server::state::ServerState;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let dir = setup_test_dir("encryption_settings");
    let download_dir = dir.join("downloads");
    fs::create_dir_all(&download_dir).unwrap();

    // 1. Default should be true when missing from settings.json
    let settings = load_settings(&dir);
    assert!(settings.encryption_enabled());

    let info = DeviceInfo::new("Test-Device", "desktop", "windows", "2.0.0", 5050);
    let server_state = Arc::new(ServerState::new(info, download_dir.clone()));
    let pending_consents = Arc::new(RwLock::new(HashMap::new()));
    let app_state = AppState::with_config_dir(
        server_state.clone(),
        None,
        download_dir.clone(),
        dir.clone(),
        pending_consents,
    );

    assert!(app_state.is_encryption_enabled().await);

    // 2. Set to false and verify persistence
    app_state.set_encryption_enabled(false).await.unwrap();
    assert!(!app_state.is_encryption_enabled().await);

    let loaded = load_settings(&dir);
    assert_eq!(loaded.encryption_enabled, Some(false));
    assert!(!loaded.encryption_enabled());

    // 3. Set back to true
    app_state.set_encryption_enabled(true).await.unwrap();
    assert!(app_state.is_encryption_enabled().await);

    let _ = fs::remove_dir_all(&dir);
}
