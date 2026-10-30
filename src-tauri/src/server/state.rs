use crate::models::peer::DeviceInfo;
use crate::models::transfer::{TransferRequest, TransferStatus};
use crate::storage::file_manager::FileManager;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
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
    /// Last time a chunk or finish request touched this session; pruning is based on idleness.
    pub last_activity: Instant,
    pub file_progress: HashMap<String, u64>,
    /// Final on-disk path of each committed file, keyed by file_id. Set only after the
    /// .part file is committed, so it reflects sanitizing, folders and collision renames.
    pub saved_paths: HashMap<String, String>,
    pub tracker: Arc<std::sync::Mutex<crate::client::metrics::ThroughputTracker>>,
    pub session_key: Option<crate::crypto::SessionKey>,
}

/// Maximum `/pair/start` attempts accepted per rolling minute, across all clients.
pub const PAIR_ATTEMPTS_PER_MINUTE: u32 = 5;
/// Unconfirmed pairing attempts after which the PIN is replaced with a fresh one.
pub const PAIR_ATTEMPTS_BEFORE_PIN_ROTATION: u32 = 10;
/// How long a started pairing run waits for its confirmation.
const PAIR_PENDING_TTL: Duration = Duration::from_secs(60);
const MAX_PENDING_PAIRS: usize = 64;
const MAX_STORED_PAIRINGS: usize = 256;
/// How long pairing stays open after the PIN was last shown, if the dialog is never closed.
pub const PAIRING_OPEN_WINDOW: Duration = Duration::from_secs(600);

/// Why a pairing run was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairError {
    /// This device is not showing its PIN, so it takes no pairing attempts at all.
    NotOpen,
    RateLimited,
    Invalid(String),
}

/// PIN pairing bookkeeping: completed pairings, runs awaiting confirmation, and attempt limits.
#[derive(Debug)]
pub struct PairingRegistry {
    keys: HashMap<String, crate::pairing::pake::PairingKey>,
    order: VecDeque<String>,
    pending: HashMap<String, (crate::pairing::pake::ServerPending, Instant)>,
    window_start: Instant,
    attempts_in_window: u32,
    unconfirmed_attempts: u32,
    open_until: Option<Instant>,
    attempts_per_minute: u32,
}

impl PairingRegistry {
    fn new() -> Self {
        Self {
            keys: HashMap::new(),
            order: VecDeque::new(),
            pending: HashMap::new(),
            window_start: Instant::now(),
            attempts_in_window: 0,
            unconfirmed_attempts: 0,
            open_until: None,
            attempts_per_minute: PAIR_ATTEMPTS_PER_MINUTE,
        }
    }
}

/// Metadata result returned when creating a session on the receiver.
#[derive(Debug, Clone)]
pub struct SessionCreated {
    pub token: String,
    pub offsets: HashMap<String, u64>,
    pub encryption_mode: Option<String>,
    pub ephemeral_public_key: Option<String>,
    pub pin_verified: Option<bool>,
    pub pair_auth: Option<String>,
}

/// Shared server state across all Axum handlers.
pub struct ServerState {
    pub device_info: Arc<RwLock<DeviceInfo>>,
    pub download_dir: Arc<std::sync::RwLock<PathBuf>>,
    pub consent_policy: ConsentPolicy,
    pub sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    pub event_tx: broadcast::Sender<ServerTransferEvent>,
    pub encryption_enabled: Arc<std::sync::atomic::AtomicBool>,
    pub pairing: Arc<std::sync::Mutex<PairingRegistry>>,
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
            pairing: Arc::new(std::sync::Mutex::new(PairingRegistry::new())),
            encryption_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Returns whether end-to-end encryption is enabled on the server.
    pub fn is_encryption_enabled(&self) -> bool {
        self.encryption_enabled.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Sets whether end-to-end encryption is enabled on the server.
    pub fn set_encryption_enabled(&self, enabled: bool) {
        self.encryption_enabled.store(enabled, std::sync::atomic::Ordering::SeqCst);
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
        info.pairing_open = self.is_pairing_open();
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

    fn pairing_registry(&self) -> std::sync::MutexGuard<'_, PairingRegistry> {
        self.pairing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Answers a SPAKE2 pairing run started by a client typing this device's PIN.
    /// Every start counts as a failed attempt until the client confirms it, because a client
    /// that guessed wrong simply never calls `/pair/finish`.
    /// Returns `(pair_id, msg_b, confirm_b_hex)`.
    pub async fn pair_start(&self, msg_a: &[u8]) -> Result<(String, Vec<u8>, String), PairError> {
        let rotate_pin = {
            let mut reg = self.pairing_registry();
            if !reg.open_until.is_some_and(|until| Instant::now() < until) {
                return Err(PairError::NotOpen);
            }
            if reg.window_start.elapsed() >= Duration::from_secs(60) {
                reg.window_start = Instant::now();
                reg.attempts_in_window = 0;
            }
            if reg.attempts_in_window >= reg.attempts_per_minute {
                return Err(PairError::RateLimited);
            }
            reg.attempts_in_window += 1;
            reg.unconfirmed_attempts += 1;
            if reg.unconfirmed_attempts > PAIR_ATTEMPTS_BEFORE_PIN_ROTATION {
                reg.unconfirmed_attempts = 1;
                true
            } else {
                false
            }
        };
        if rotate_pin {
            self.regenerate_pairing_pin().await;
        }

        let pin = self
            .get_pairing_pin()
            .await
            .ok_or_else(|| PairError::Invalid("Pairing is not available".to_string()))?;
        let (pending, msg_b, confirm_b) =
            crate::pairing::pake::server_respond(&pin, msg_a).map_err(PairError::Invalid)?;

        let pair_id = format!("pair-{}", uuid::Uuid::new_v4());
        let mut reg = self.pairing_registry();
        reg.pending.retain(|_, (_, at)| at.elapsed() < PAIR_PENDING_TTL);
        if reg.pending.len() >= MAX_PENDING_PAIRS {
            if let Some(oldest) = reg
                .pending
                .iter()
                .min_by_key(|(_, (_, at))| *at)
                .map(|(id, _)| id.clone())
            {
                reg.pending.remove(&oldest);
            }
        }
        reg.pending.insert(pair_id.clone(), (pending, Instant::now()));
        Ok((pair_id, msg_b, confirm_b))
    }

    /// Completes a pairing run once the client proves it knew the PIN, and stores the key.
    pub fn pair_finish(&self, pair_id: &str, confirm_a_hex: &str) -> Result<(), String> {
        let mut reg = self.pairing_registry();
        let (pending, started) = reg
            .pending
            .remove(pair_id)
            .ok_or_else(|| "Unknown or expired pairing attempt".to_string())?;
        if started.elapsed() >= PAIR_PENDING_TTL {
            return Err("Unknown or expired pairing attempt".to_string());
        }
        let key = crate::pairing::pake::server_verify(&pending, confirm_a_hex)?;

        reg.unconfirmed_attempts = 0;
        if reg.order.len() >= MAX_STORED_PAIRINGS {
            if let Some(oldest) = reg.order.pop_front() {
                reg.keys.remove(&oldest);
            }
        }
        reg.order.push_back(pair_id.to_string());
        reg.keys.insert(pair_id.to_string(), key);
        Ok(())
    }

    /// Opens pairing (the PIN is on screen) for `PAIRING_OPEN_WINDOW`, or closes it.
    /// Opening again extends the window.
    pub fn set_pairing_open(&self, open: bool) {
        self.pairing_registry().open_until = open.then(|| Instant::now() + PAIRING_OPEN_WINDOW);
    }

    /// Overrides the per-minute pairing attempt limit (default `PAIR_ATTEMPTS_PER_MINUTE`).
    pub fn set_pair_attempts_per_minute(&self, limit: u32) {
        self.pairing_registry().attempts_per_minute = limit;
    }

    /// Whether this device currently accepts pairing runs.
    pub fn is_pairing_open(&self) -> bool {
        self.pairing_registry()
            .open_until
            .is_some_and(|until| Instant::now() < until)
    }

    /// Returns the pairing key for a completed pairing.
    pub fn pairing_key(&self, pair_id: &str) -> Option<crate::pairing::pake::PairingKey> {
        self.pairing_registry().keys.get(pair_id).cloned()
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

    /// Creates a new session entry in state, negotiating encryption and discovering partial offsets.
    pub async fn create_session(&self, request: TransferRequest) -> Result<SessionCreated, String> {
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

        let mut session_key = None;
        let mut encryption_mode = None;
        let mut ephemeral_public_key = None;
        let mut pin_verified = None;
        let mut pair_auth = None;

        if self.is_encryption_enabled()
            && request.encryption_mode.as_deref() == Some("ChaCha20Poly1305")
            && request.ephemeral_public_key.is_some()
        {
            let sender_pub_hex = request.ephemeral_public_key.as_ref().unwrap();
            let sender_pub = crate::crypto::public_key_from_hex(sender_pub_hex)
                .map_err(|e| format!("Invalid sender ephemeral public key: {}", e))?;

            let pairing_key = match request.pair_id.as_deref() {
                Some(pair_id) => {
                    let key = self.pairing_key(pair_id).ok_or_else(|| {
                        "Unknown pairing; pair with this device again".to_string()
                    })?;
                    let auth = request.pair_auth.as_deref().unwrap_or("");
                    if !crate::pairing::pake::verify_request_auth(
                        &key,
                        &request.request_id,
                        sender_pub_hex,
                        auth,
                    ) {
                        return Err("Pairing verification failed".to_string());
                    }
                    pin_verified = Some(true);
                    Some(key)
                }
                None => {
                    pin_verified = Some(false);
                    None
                }
            };

            let (receiver_secret, receiver_pub) = crate::crypto::generate_ephemeral_keypair();
            let receiver_pub_hex = crate::crypto::public_key_to_hex(&receiver_pub);
            let key = crate::crypto::derive_session_key(
                receiver_secret,
                &sender_pub,
                pairing_key.as_ref().map(|k| &k.as_bytes()[..]).unwrap_or(&[]),
                &token,
            );
            pair_auth = pairing_key.as_ref().map(|k| {
                crate::pairing::pake::response_auth(k, &request.request_id, &token, &receiver_pub_hex)
            });

            session_key = Some(key);
            encryption_mode = Some("ChaCha20Poly1305".to_string());
            ephemeral_public_key = Some(receiver_pub_hex);
        }

        let session = SessionState {
            request,
            status: TransferStatus::Accepted,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            file_progress: offsets.clone(),
            saved_paths: HashMap::new(),
            tracker: Arc::new(std::sync::Mutex::new(
                crate::client::ThroughputTracker::new(),
            )),
            session_key,
        };

        self.sessions.write().await.insert(token.clone(), session);
        Ok(SessionCreated {
            token,
            offsets,
            encryption_mode,
            ephemeral_public_key,
            pin_verified,
            pair_auth,
        })
    }

    /// Retrieves session data by session token.
    pub async fn get_session(&self, token: &str) -> Option<SessionState> {
        self.sessions.read().await.get(token).cloned()
    }

    /// Prunes transfer sessions that have been idle (no chunk or finish request) for longer
    /// than `max_idle`, so long-running active transfers are never cut off mid-way.
    /// Returns the number of evicted sessions.
    pub async fn prune_expired_sessions(&self, max_idle: std::time::Duration) -> usize {
        let mut sessions = self.sessions.write().await;
        let before_count = sessions.len();
        sessions.retain(|_token, session| session.last_activity.elapsed() < max_idle);
        before_count - sessions.len()
    }
}
