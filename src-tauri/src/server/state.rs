use crate::models::peer::DeviceInfo;
use crate::models::transfer::{TransferRequest, TransferStatus};
use crate::storage::file_manager::FileManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};

/// Payload emitted for receiver-side real-time progress events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiveProgressPayload {
    pub session_token: String,
    pub file_id: String,
    pub file_name: String,
    pub received_bytes: u64,
    pub total_bytes: u64,
    pub percent: u32,
    #[serde(default)]
    pub speed_bps: u64,
    #[serde(default)]
    pub eta_seconds: Option<u64>,
}

/// Payload emitted for receiver-side completion notification events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiveCompletedPayload {
    pub session_token: String,
    pub file_id: String,
    pub file_name: String,
    pub saved_path: String,
}

/// Payload emitted when a transfer session is cancelled on the receiver side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiveCancelledPayload {
    pub session_token: String,
}

/// High-level receiver-side transfer events emitted by the Axum server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerTransferEvent {
    Progress(ReceiveProgressPayload),
    Completed(ReceiveCompletedPayload),
    Cancelled(ReceiveCancelledPayload),
}

/// Consent request sent to UI / controller for user decision.
#[derive(Debug)]
pub struct ConsentRequest {
    pub transfer_request: TransferRequest,
    pub responder: oneshot::Sender<bool>,
}

/// Policy determining how incoming transfer requests are evaluated.
#[derive(Clone)]
pub enum ConsentPolicy {
    AutoAccept,
    AutoDecline(String),
    Channel(mpsc::Sender<ConsentRequest>),
    Custom(
        Arc<
            dyn Fn(&TransferRequest) -> Pin<Box<dyn Future<Output = bool> + Send>>
                + Send
                + Sync,
        >,
    ),
}

/// Internal state for an active or past transfer session.
#[derive(Debug, Clone)]
pub struct SessionState {
    pub request: TransferRequest,
    pub status: TransferStatus,
    pub created_at: Instant,
    pub file_progress: HashMap<String, u64>,
    pub tracker: Arc<std::sync::Mutex<crate::client::metrics::ThroughputTracker>>,
}

/// Shared server state across all Axum handlers.
pub struct ServerState {
    pub device_info: Arc<RwLock<DeviceInfo>>,
    pub download_dir: Arc<std::sync::RwLock<PathBuf>>,
    pub consent_policy: ConsentPolicy,
    pub sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    pub event_tx: broadcast::Sender<ServerTransferEvent>,
}

impl ServerState {
    /// Creates a new `ServerState` with `AutoAccept` consent policy.
    pub fn new(device_info: DeviceInfo, download_dir: PathBuf) -> Self {
        Self::with_consent(device_info, download_dir, ConsentPolicy::AutoAccept)
    }

    /// Creates a new `ServerState` with a specific consent policy.
    pub fn with_consent(
        mut device_info: DeviceInfo,
        download_dir: PathBuf,
        consent_policy: ConsentPolicy,
    ) -> Self {
        if device_info.pairing_pin.is_none() {
            device_info.pairing_pin = Some(crate::pairing::pin::generate_pin());
        }
        let (event_tx, _) = broadcast::channel(256);
        Self {
            device_info: Arc::new(RwLock::new(device_info)),
            download_dir: Arc::new(std::sync::RwLock::new(download_dir)),
            consent_policy,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribes to receiver-side transfer events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<ServerTransferEvent> {
        self.event_tx.subscribe()
    }

    /// Emits a receiver-side transfer event across the broadcast channel.
    pub fn emit_event(&self, event: ServerTransferEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Returns a clone of the current device info.
    pub async fn get_device_info(&self) -> DeviceInfo {
        self.device_info.read().await.clone()
    }

    /// Returns a sanitized public clone of device info with the pairing PIN stripped
    /// to prevent network enumeration or unauthorized secret harvesting.
    pub async fn get_public_device_info(&self) -> DeviceInfo {
        let mut info = self.device_info.read().await.clone();
        info.pairing_pin = None;
        info
    }

    /// Updates the port in `device_info`.
    pub async fn set_port(&self, port: u16) {
        let mut info = self.device_info.write().await;
        info.port = port;
    }

    /// Regenerates the 6-digit numeric pairing PIN, updates `device_info`, and returns the new PIN.
    pub async fn regenerate_pairing_pin(&self) -> String {
        let new_pin = crate::pairing::pin::generate_pin();
        let mut info = self.device_info.write().await;
        info.pairing_pin = Some(new_pin.clone());
        new_pin
    }

    /// Sets a specific pairing PIN in `device_info`.
    pub async fn set_pairing_pin(&self, pin: impl Into<String>) {
        let mut info = self.device_info.write().await;
        info.pairing_pin = Some(pin.into());
    }

    /// Returns the active pairing PIN if set.
    pub async fn get_pairing_pin(&self) -> Option<String> {
        let info = self.device_info.read().await;
        info.pairing_pin.clone()
    }

    /// Returns the download directory.
    pub fn download_dir(&self) -> PathBuf {
        self.download_dir.read().unwrap().clone()
    }

    /// Updates the download directory.
    pub fn set_download_dir(&self, dir: PathBuf) {
        *self.download_dir.write().unwrap() = dir;
    }

    /// Updates the local device display name.
    pub async fn set_device_name(&self, new_name: String) -> DeviceInfo {
        let mut info = self.device_info.write().await;
        info.device_name = new_name;
        info.clone()
    }

    /// Evaluates consent for an incoming `TransferRequest` according to the configured policy.
    pub async fn evaluate_consent(&self, request: &TransferRequest) -> Result<(), String> {
        match &self.consent_policy {
            ConsentPolicy::AutoAccept => Ok(()),
            ConsentPolicy::AutoDecline(reason) => Err(reason.clone()),
            ConsentPolicy::Channel(tx) => {
                let (resp_tx, resp_rx) = oneshot::channel();
                let consent_req = ConsentRequest {
                    transfer_request: request.clone(),
                    responder: resp_tx,
                };
                if tx.send(consent_req).await.is_err() {
                    return Err("Consent channel receiver dropped".to_string());
                }
                match tokio::time::timeout(Duration::from_secs(60), resp_rx).await {
                    Ok(Ok(true)) => Ok(()),
                    Ok(Ok(false)) => Err("Transfer declined by receiver".to_string()),
                    Ok(Err(_)) => Err("Consent response channel closed".to_string()),
                    Err(_) => Err("Consent prompt timed out after 60 seconds".to_string()),
                }
            }
            ConsentPolicy::Custom(cb) => {
                if cb(request).await {
                    Ok(())
                } else {
                    Err("Transfer declined".to_string())
                }
            }
        }
    }

    /// Creates a new session entry in state, generating a unique token and discovering partial offsets.
    pub async fn create_session(&self, request: TransferRequest) -> (String, HashMap<String, u64>) {
        let token = format!("sess-{}", uuid::Uuid::new_v4());
        let mut offsets = HashMap::new();

        for file in &request.files {
            let part_path = FileManager::get_part_path(&self.download_dir(), &file.name, &file.file_key());
            let offset = if part_path.exists() {
                FileManager::get_file_size(&part_path).unwrap_or(0)
            } else {
                0
            };
            offsets.insert(file.file_id.clone(), offset);
        }

        let session = SessionState {
            request,
            status: TransferStatus::Accepted,
            created_at: Instant::now(),
            file_progress: offsets.clone(),
            tracker: Arc::new(std::sync::Mutex::new(
                crate::client::ThroughputTracker::new(),
            )),
        };

        self.sessions.write().await.insert(token.clone(), session);
        (token, offsets)
    }

    /// Retrieves session data by session token.
    pub async fn get_session(&self, token: &str) -> Option<SessionState> {
        self.sessions.read().await.get(token).cloned()
    }

    /// Prunes expired transfer sessions from state whose age exceeds `max_age`.
    /// Returns the number of evicted sessions.
    pub async fn prune_expired_sessions(&self, max_age: std::time::Duration) -> usize {
        let mut sessions = self.sessions.write().await;
        let before_count = sessions.len();
        sessions.retain(|_token, session| session.created_at.elapsed() < max_age);
        before_count - sessions.len()
    }
}
