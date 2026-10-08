use crate::client::TransferClient;
use crate::discovery::{
    DiscoveryConfig, DiscoveryEvent, DiscoveryHandle, DiscoveryService, PeerEvent,
};
use crate::models::peer::{DeviceInfo, PeerInfo};
use crate::server::state::ServerState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, RwLock};

/// Unified application state shared across Tauri IPC command handlers and background tasks.
pub struct AppState {
    pub server_state: Arc<ServerState>,
    pub discovery_handle: Arc<RwLock<Option<DiscoveryHandle>>>,
    pub transfer_client: TransferClient,
    pub download_dir: PathBuf,
    pub pending_consents: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
}

impl AppState {
    /// Creates a new `AppState` instance.
    pub fn new(
        server_state: Arc<ServerState>,
        discovery_handle: Option<DiscoveryHandle>,
        download_dir: PathBuf,
        pending_consents: Arc<RwLock<HashMap<String, oneshot::Sender<bool>>>>,
    ) -> Self {
        Self {
            server_state,
            discovery_handle: Arc::new(RwLock::new(discovery_handle)),
            transfer_client: TransferClient::new(),
            download_dir,
            pending_consents,
        }
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

    /// Restarts discovery broadcast and listener with new DeviceInfo (e.g. after Room ID update).
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

    /// Returns a reference to the download directory.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }
}
