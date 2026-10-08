pub mod app_state;
pub mod client;
pub mod commands;
pub mod discovery;
pub mod models;
pub mod server;
pub mod storage;

use app_state::AppState;
use discovery::{DiscoveryConfig, DiscoveryEvent, DiscoveryService, PeerEvent};
use models::peer::DeviceInfo;
use server::routes::start_server;
use server::state::{ConsentPolicy, ConsentRequest, ServerState};
use storage::file_manager::FileManager;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, RwLock};

/// Initializes all backend services (HTTP Transfer Server, mDNS/UDP Discovery, Consent Event Bridge).
pub async fn setup_services(
    app: &tauri::AppHandle,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    // 1. Determine device identity
    let device_name = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "EasyShare-Device".to_string());
    let os = std::env::consts::OS.to_string();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let initial_port = 5050;

    let device_info = DeviceInfo::new(
        device_name,
        "desktop",
        os,
        version,
        None,
        initial_port,
    );

    // 2. Resolve and ensure download directory exists
    let download_dir = FileManager::get_downloads_dir();
    FileManager::ensure_download_dir(&download_dir)?;

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
        while let Ok(event) = disc_sub.recv().await {
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
                map.insert(req_id, consent_req.responder);
            }

            let _ = app_consent.emit("transfer-requested", &req_payload);
        }
    });

    let app_state = AppState::new(
        server_state,
        Some(discovery_handle),
        download_dir,
        pending_consents,
    );

    Ok(app_state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_my_device_info,
            commands::get_discovered_peers,
            commands::add_manual_peer,
            commands::set_room_id,
            commands::respond_transfer_request,
            commands::start_transfer,
            commands::cancel_transfer,
            commands::get_download_dir,
            commands::open_download_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running easy-share application");
}
