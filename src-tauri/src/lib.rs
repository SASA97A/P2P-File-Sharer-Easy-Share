pub mod app_state;
pub mod client;
pub mod commands;
pub mod crypto;
pub mod discovery;
pub mod models;
pub mod notifications;
pub mod pairing;
pub mod server;
pub mod storage;
pub mod tray;

use app_state::AppState;
use discovery::{DiscoveryConfig, DiscoveryEvent, DiscoveryService, PeerEvent};
use models::peer::DeviceInfo;
use server::routes::start_server;
use server::state::{ConsentPolicy, ConsentRequest, ServerState, ServerTransferEvent};
use storage::file_manager::FileManager;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, RwLock};

/// Initializes all backend services (HTTP Transfer Server, mDNS/UDP Discovery, Consent Event Bridge).
pub async fn setup_services(
    app: &tauri::AppHandle,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    // 0. Resolve config directory and load persisted settings
    let config_dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| FileManager::get_downloads_dir());
    let settings = storage::settings::load_settings(&config_dir);

    // 1. Determine device identity (using persisted custom name if set)
    let device_name = settings.device_name.unwrap_or_else(|| {
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "EasyShare-Device".to_string())
    });
    let os = std::env::consts::OS.to_string();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let initial_port = 5050;

    // Stable install id, created once; paired devices remember this device by it
    let device_id = match settings.device_id.clone() {
        Some(id) => id,
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            let mut updated = storage::settings::load_settings(&config_dir);
            updated.device_id = Some(id.clone());
            let _ = storage::settings::save_settings(&config_dir, &updated);
            id
        }
    };

    let device_info = DeviceInfo::new(
        device_name,
        "desktop",
        os,
        version,
        initial_port,
    )
    .with_device_id(device_id);

    // 2. Resolve and ensure download directory exists (using persisted custom path if set)
    let download_dir = if let Some(custom_dir) = settings.download_dir {
        let path = std::path::PathBuf::from(custom_dir);
        if FileManager::ensure_download_dir(&path).is_ok() {
            path
        } else {
            let default_dir = FileManager::get_downloads_dir();
            FileManager::ensure_download_dir(&default_dir)?;
            default_dir
        }
    } else {
        let default_dir = FileManager::get_downloads_dir();
        FileManager::ensure_download_dir(&default_dir)?;
        default_dir
    };

    // 3. Create consent request channel for interactive handshake prompts
    let (consent_tx, mut consent_rx) = mpsc::channel::<ConsentRequest>(64);
    let consent_policy = ConsentPolicy::Channel(consent_tx);

    // 4. Create ServerState with consent policy
    let server_state = Arc::new(ServerState::with_consent(
        device_info.clone(),
        download_dir.clone(),
        consent_policy,
    ));

    // 5. Start Axum transfer server with fallback ports if 5050 is busy
    let mut bound_success = None;
    if let Ok(res) = start_server(server_state.clone(), initial_port).await {
        bound_success = Some(res);
    } else {
        for port in (initial_port + 1)..=(initial_port + 20) {
            if let Ok(res) = start_server(server_state.clone(), port).await {
                bound_success = Some(res);
                break;
            }
        }
    }

    let (_bound_port, _server_task) = match bound_success {
        Some(res) => res,
        None => start_server(server_state.clone(), 0).await?,
    };

    let active_device_info = server_state.get_device_info().await;

    // 6. Start Discovery Service
    let (discovery_svc, _disc_rx) =
        DiscoveryService::new(active_device_info, DiscoveryConfig::default()).await?;
    let discovery_handle = discovery_svc.start().await?;

    let pending_consents = Arc::new(RwLock::new(HashMap::new()));

    // 7. Bridge Discovery events to Tauri frontend
    let mut disc_sub = discovery_handle.subscribe();
    let app_disc = app.clone();
    tokio::spawn(async move {
        loop {
            // A lagged receiver only missed events; keep forwarding instead of exiting for good.
            let event = match disc_sub.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            match event {
                DiscoveryEvent::Peer(PeerEvent::Discovered(peer)) => {
                    let _ = app_disc.emit("peer-found", &peer);
                }
                DiscoveryEvent::Peer(PeerEvent::Lost(peer)) => {
                    let _ = app_disc.emit("peer-lost", &peer);
                }
                DiscoveryEvent::Peer(PeerEvent::Updated(peer)) => {
                    let _ = app_disc.emit("peer-updated", &peer);
                }
            }
        }
    });

    // 8. Bridge incoming consent requests to Tauri frontend
    let pending_consents_bridge = pending_consents.clone();
    let app_consent = app.clone();
    tokio::spawn(async move {
        while let Some(consent_req) = consent_rx.recv().await {
            let req_id = consent_req.transfer_request.request_id.clone();
            let req_payload = consent_req.transfer_request.clone();

            {
                let mut map = pending_consents_bridge.write().await;
                map.insert(req_id.clone(), consent_req.responder);
            }

            // The server gives up waiting after 60s and drops its receiver. Once that happens,
            // remove the stale entry and tell the UI to close the prompt.
            let expiry_map = pending_consents_bridge.clone();
            let expiry_app = app_consent.clone();
            let expiry_id = req_id.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(61)).await;
                let mut map = expiry_map.write().await;
                if map.get(&expiry_id).is_some_and(|responder| responder.is_closed()) {
                    map.remove(&expiry_id);
                    let _ = expiry_app.emit(
                        "transfer-request-expired",
                        &serde_json::json!({ "request_id": expiry_id }),
                    );
                }
            });

            let _ = app_consent.emit("transfer-requested", &req_payload);
            notifications::notify_incoming_transfer(
                &app_consent,
                &req_payload.sender_name,
                req_payload.files.len(),
                req_payload.total_bytes,
            );
        }
    });

    // 9. Bridge receiver-side transfer progress & completion events to Tauri frontend
    let mut server_events = server_state.subscribe_events();
    let app_server = app.clone();
    let s_state = server_state.clone();
    let c_dir = config_dir.clone();
    tokio::spawn(async move {
        use std::collections::HashSet;
        let mut recorded_tokens = HashSet::new();

        loop {
            // A lagged receiver only missed events; keep forwarding instead of exiting for good.
            let event = match server_events.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            match event {
                ServerTransferEvent::Progress(progress) => {
                    let _ = app_server.emit("receive-progress", &progress);
                    tray::update_tray_tooltip(
                        &app_server,
                        &tray::format_transfer_tooltip(
                            "Receiving",
                            progress.percent,
                            Some(progress.speed_bps),
                            Some(&progress.file_name),
                        ),
                    );
                }
                ServerTransferEvent::Completed(completed) => {
                    let _ = app_server.emit("receive-completed", &completed);

                    if !recorded_tokens.contains(&completed.session_token) {
                        if let Some(session) = s_state.get_session(&completed.session_token).await {
                            let all_finished = session.request.files.iter().all(|f| {
                                session.file_progress.get(&f.file_id).copied().unwrap_or(0) >= f.size
                            });
                            if all_finished {
                                recorded_tokens.insert(completed.session_token.clone());
                                tray::update_tray_tooltip(&app_server, "Easy Share - Ready");
                                notifications::notify_transfer_completed(
                                    &app_server,
                                    &session.request.sender_name,
                                    session.request.files.len(),
                                    true,
                                );
                                let record = storage::history::TransferHistoryRecord {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_token: Some(completed.session_token.clone()),
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_millis() as i64)
                                        .unwrap_or(0),
                                    direction: storage::history::TransferDirection::Received,
                                    peer_name: session.request.sender_name.clone(),
                                    peer_ip: "Direct / Wi-Fi".to_string(),
                                    status: storage::history::HistoryStatus::Completed,
                                    files: session
                                        .request
                                        .files
                                        .iter()
                                        .map(|f| storage::history::HistoryFileItem {
                                            name: f.name.clone(),
                                            path: session.saved_paths.get(&f.file_id).cloned(),
                                            size: f.size,
                                        })
                                        .collect(),
                                    total_bytes: session.request.total_bytes,
                                    error_message: None,
                                };
                                if storage::history::add_record(&c_dir, record).is_ok() {
                                    let history = storage::history::load_history(&c_dir);
                                    let _ = app_server.emit("history-updated", &history);
                                }
                            }
                        }
                    }
                }
                ServerTransferEvent::Cancelled(cancelled) => {
                    let _ = app_server.emit("receive-cancelled", &cancelled);
                    tray::update_tray_tooltip(&app_server, "Easy Share - Ready");

                    if !recorded_tokens.contains(&cancelled.session_token) {
                        recorded_tokens.insert(cancelled.session_token.clone());
                        if let Some(session) = s_state.get_session(&cancelled.session_token).await {
                            let record = storage::history::TransferHistoryRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_token: Some(cancelled.session_token.clone()),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0),
                                direction: storage::history::TransferDirection::Received,
                                peer_name: session.request.sender_name.clone(),
                                peer_ip: "Direct / Wi-Fi".to_string(),
                                status: storage::history::HistoryStatus::Cancelled,
                                files: session
                                    .request
                                    .files
                                    .iter()
                                    .map(|f| storage::history::HistoryFileItem {
                                        name: f.name.clone(),
                                        path: session.saved_paths.get(&f.file_id).cloned(),
                                        size: f.size,
                                    })
                                    .collect(),
                                total_bytes: session.request.total_bytes,
                                error_message: Some("Transfer cancelled".to_string()),
                            };
                            if storage::history::add_record(&c_dir, record).is_ok() {
                                let history = storage::history::load_history(&c_dir);
                                let _ = app_server.emit("history-updated", &history);
                            }
                        }
                    }
                }
            }
        }
    });

    // 10. Background maintenance task to prune server sessions idle for more than 1 hour
    let prune_state = server_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            prune_state
                .prune_expired_sessions(std::time::Duration::from_secs(3600))
                .await;
        }
    });

    let app_state = AppState::with_config_dir(
        server_state,
        Some(discovery_handle),
        download_dir,
        config_dir,
        pending_consents,
    );

    Ok(app_state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::restore_main_window(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                match setup_services(&app_handle).await {
                    Ok(state) => {
                        app_handle.manage(state);
                    }
                    Err(err) => {
                        eprintln!("Failed to initialize Easy Share services: {}", err);
                    }
                }
            });

            if let Err(e) = tray::setup_tray(app.handle()) {
                eprintln!("Failed to setup system tray: {}", e);
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(state) = window.try_state::<AppState>() {
                    let minimize = tauri::async_runtime::block_on(async {
                        state.get_tray_settings().await.0
                    });
                    if minimize {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_my_device_info,
            commands::get_my_pairing_info,
            commands::regenerate_pairing_pin,
            commands::get_pairing_status,
            commands::set_pairing_mode,
            commands::get_paired_devices_status,
            commands::set_remember_paired_devices,
            commands::forget_paired_devices,
            commands::connect_by_pin,
            commands::connect_by_address,
            commands::get_discovered_peers,
            commands::add_manual_peer,
            commands::respond_transfer_request,
            commands::start_transfer,
            commands::pause_transfer,
            commands::resume_transfer,
            commands::cancel_transfer,
            commands::get_download_dir,
            commands::set_download_dir,
            commands::get_speed_limit,
            commands::set_speed_limit,
            commands::set_device_name,
            commands::open_download_dir,
            commands::get_tray_settings,
            commands::set_tray_settings,
            commands::get_encryption_settings,
            commands::set_encryption_settings,
            commands::inspect_paths,
            commands::pick_files,
            commands::pick_folder,
            commands::get_transfer_history,
            commands::delete_history_record,
            commands::clear_transfer_history,
            commands::show_in_folder,
            commands::open_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running easy-share application");
}
