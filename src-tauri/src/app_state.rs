use crate::client::TransferClient;
use crate::discovery::{
    DiscoveryConfig, DiscoveryEvent, DiscoveryHandle, DiscoveryService, PeerEvent,
};
use crate::models::peer::{DeviceInfo, PeerInfo};
use crate::server::state::ServerState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, RwLock};
use tokio_util::sync::CancellationToken;

/// Unified application state shared across Tauri IPC command handlers and background tasks.
pub struct AppState {
    pub server_state: Arc<ServerState>,
    pub discovery_handle: Arc<RwLock<Option<DiscoveryHandle>>>,
    pub transfer_client: TransferClient,
    pub download_dir: Arc<std::sync::RwLock<PathBuf>>,
    pub config_dir: PathBuf,
    pub pending_consents: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
    pub cancellation_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl AppState {
    /// Creates a new `AppState` instance.
    pub fn new(
        server_state: Arc<ServerState>,
        discovery_handle: Option<DiscoveryHandle>,
        download_dir: PathBuf,
        pending_consents: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
    ) -> Self {
        let config_dir = download_dir.clone();
        Self::with_config_dir(
            server_state,
            discovery_handle,
            download_dir,
            config_dir,
            pending_consents,
        )
    }

    /// Creates a new `AppState` instance with an explicit configuration directory.
    pub fn with_config_dir(
        server_state: Arc<ServerState>,
        discovery_handle: Option<DiscoveryHandle>,
        download_dir: PathBuf,
        config_dir: PathBuf,
        pending_consents: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
    ) -> Self {
        Self {
            server_state,
            discovery_handle: Arc::new(RwLock::new(discovery_handle)),
            transfer_client: TransferClient::new(),
            download_dir: Arc::new(std::sync::RwLock::new(download_dir)),
            config_dir,
            pending_consents,
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers an active transfer cancellation token.
    pub async fn register_cancellation_token(&self, session_token: &str, token: CancellationToken) {
        self.cancellation_tokens
            .write()
            .await
            .insert(session_token.to_string(), token);
    }

    /// Triggers cancellation of an active session token if registered.
    /// Returns true if a token was found and cancelled.
    pub async fn cancel_session(&self, session_token: &str) -> bool {
        if let Some(token) = self.cancellation_tokens.write().await.remove(session_token) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Removes a cancellation token after normal completion or error.
    pub async fn remove_cancellation_token(&self, session_token: &str) {
        self.cancellation_tokens.write().await.remove(session_token);
    }

    /// Returns a clone of the current device info.
    pub async fn get_device_info(&self) -> DeviceInfo {
        self.server_state.get_device_info().await
    }

    /// Regenerates the pairing PIN and returns the new PIN.
    pub async fn regenerate_pairing_pin(&self) -> String {
        self.server_state.regenerate_pairing_pin().await
    }

    /// Returns the currently discovered peers.
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        if let Some(handle) = self.discovery_handle.read().await.as_ref() {
            handle.get_peers().await
        } else {
            Vec::new()
        }
    }

    /// Adds a manual peer to the discovery tracker.
    pub async fn add_manual_peer(&self, peer: PeerInfo) {
        if let Some(handle) = self.discovery_handle.read().await.as_ref() {
            let _ = handle.add_manual_peer(&peer.ip, peer.port).await;
        }
    }

    /// Restarts discovery broadcast and listener with new DeviceInfo (e.g. after network or device info update).
    pub async fn restart_discovery(&self, app: AppHandle, new_info: DeviceInfo) -> Result<(), String> {
        let mut handle_lock = self.discovery_handle.write().await;
        if let Some(handle) = handle_lock.take() {
            handle.stop().await;
        }

        let (service, _rx) = DiscoveryService::new(new_info, DiscoveryConfig::default())
            .await
            .map_err(|e| format!("Failed to initialize discovery service: {}", e))?;

        let new_handle = service
            .start()
            .await
            .map_err(|e| format!("Failed to start discovery service: {}", e))?;

        // Subscribe to peer events and forward to frontend
        let mut sub = new_handle.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = sub.recv().await {
                match event {
                    DiscoveryEvent::Peer(PeerEvent::Discovered(peer)) => {
                        let _ = app.emit("peer-found", &peer);
                    }
                    DiscoveryEvent::Peer(PeerEvent::Lost(peer)) => {
                        let _ = app.emit("peer-lost", &peer);
                    }
                    DiscoveryEvent::Peer(PeerEvent::Updated(peer)) => {
                        let _ = app.emit("peer-updated", &peer);
                    }
                }
            }
        });

        *handle_lock = Some(new_handle);
        Ok(())
    }

    /// Returns the download directory.
    pub fn get_download_dir(&self) -> PathBuf {
        self.download_dir.read().unwrap().clone()
    }

    /// Compatibility accessor for download directory.
    pub fn download_dir(&self) -> PathBuf {
        self.get_download_dir()
    }

    /// Updates the download directory, ensures it exists, updates server state, and persists settings.
    pub async fn set_download_dir(&self, new_path_str: String) -> Result<PathBuf, String> {
        let trimmed = new_path_str.trim();
        if trimmed.is_empty() {
            return Err("Download path cannot be empty".to_string());
        }
        let new_path = PathBuf::from(trimmed);

        crate::storage::file_manager::FileManager::ensure_download_dir(&new_path)
            .map_err(|e| format!("Failed to access or create directory {:?}: {}", new_path, e))?;

        *self.download_dir.write().unwrap() = new_path.clone();
        self.server_state.set_download_dir(new_path.clone());

        let mut settings = crate::storage::settings::load_settings(&self.config_dir);
        settings.download_dir = Some(new_path.to_string_lossy().to_string());
        crate::storage::settings::save_settings(&self.config_dir, &settings)?;

        Ok(new_path)
    }

    /// Updates the local device display name, updates server state, and persists settings.
    pub async fn set_device_name(&self, new_name_str: String) -> Result<DeviceInfo, String> {
        let trimmed = new_name_str.trim();
        if trimmed.is_empty() {
            return Err("Device name cannot be empty".to_string());
        }
        if trimmed.len() > 32 {
            return Err("Device name must be 32 characters or less".to_string());
        }

        let updated_info = self.server_state.set_device_name(trimmed.to_string()).await;

        let mut settings = crate::storage::settings::load_settings(&self.config_dir);
        settings.device_name = Some(trimmed.to_string());
        crate::storage::settings::save_settings(&self.config_dir, &settings)?;

        Ok(updated_info)
    }
}
