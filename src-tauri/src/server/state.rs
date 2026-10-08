use crate::models::peer::DeviceInfo;
use crate::models::transfer::{TransferRequest, TransferStatus};
use crate::storage::file_manager::FileManager;
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, RwLock};

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
}

/// Shared server state across all Axum handlers.
pub struct ServerState {
    pub device_info: Arc<RwLock<DeviceInfo>>,
    pub download_dir: PathBuf,
    pub consent_policy: ConsentPolicy,
    pub sessions: Arc<RwLock<HashMap<String, SessionState>>>,
}

impl ServerState {
    /// Creates a new `ServerState` with `AutoAccept` consent policy.
    pub fn new(device_info: DeviceInfo, download_dir: PathBuf) -> Self {
        Self::with_consent(device_info, download_dir, ConsentPolicy::AutoAccept)
    }

    /// Creates a new `ServerState` with a specific consent policy.
    pub fn with_consent(
        device_info: DeviceInfo,
        download_dir: PathBuf,
        consent_policy: ConsentPolicy,
    ) -> Self {
        Self {
            device_info: Arc::new(RwLock::new(device_info)),
            download_dir,
            consent_policy,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Returns a clone of the current device info.
    pub async fn get_device_info(&self) -> DeviceInfo {
        self.device_info.read().await.clone()
    }

    /// Updates the port in `device_info`.
    pub async fn set_port(&self, port: u16) {
        let mut info = self.device_info.write().await;
        info.port = port;
    }

    /// Returns the download directory reference.
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
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
            let part_path = FileManager::get_part_path(&self.download_dir, &file.name, &token);
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
        };

        self.sessions.write().await.insert(token.clone(), session);
        (token, offsets)
    }

    /// Retrieves session data by session token.
    pub async fn get_session(&self, token: &str) -> Option<SessionState> {
        self.sessions.read().await.get(token).cloned()
    }
}
