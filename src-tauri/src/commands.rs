use crate::app_state::AppState;
use crate::client::{ClientError, FileToSend};
use crate::models::peer::{DeviceInfo, PeerInfo};
use crate::models::transfer::{TransferRequest, TransferStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

/// Input descriptor for a file to transfer chosen from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSelection {
    #[serde(alias = "fullPath", alias = "filePath", alias = "file_path")]
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "fileId", alias = "id")]
    pub file_id: Option<String>,
    #[serde(default, alias = "relativePath", alias = "relative_path")]
    pub relative_path: Option<String>,
}

/// Detailed representation of a selected file on the host machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedFile {
    pub name: String,
    pub path: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relative_path: Option<String>,
}

/// Recursively resolves file metadata (name, size, absolute path) from a slice of path references.
pub fn inspect_file_paths<P: AsRef<std::path::Path>>(paths: &[P]) -> Vec<SelectedFile> {
    let mut results = Vec::new();
    for p in paths {
        let path_ref = p.as_ref();
        if path_ref.is_file() {
            let name = path_ref
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed_file".to_string());
            let size = std::fs::metadata(path_ref).map(|m| m.len()).unwrap_or(0);
            results.push(SelectedFile {
                name,
                path: path_ref.to_string_lossy().to_string(),
                size,
                relative_path: None,
            });
        } else if path_ref.is_dir() {
            let root_name = path_ref
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "folder".to_string());

            let mut dir_stack = vec![path_ref.to_path_buf()];
            while let Some(current_dir) = dir_stack.pop() {
                if let Ok(entries) = std::fs::read_dir(&current_dir) {
                    for entry in entries.flatten() {
                        let sub_path = entry.path();
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                dir_stack.push(sub_path);
                            } else if file_type.is_file() {
                                let name = sub_path
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "unnamed_file".to_string());
                                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

                                let relative_path = if let Ok(rel) = sub_path.strip_prefix(path_ref) {
                                    let inner = rel.to_string_lossy().replace('\\', "/");
                                    format!("{}/{}", root_name, inner)
                                } else {
                                    name.clone()
                                };

                                results.push(SelectedFile {
                                    name,
                                    path: sub_path.to_string_lossy().to_string(),
                                    size,
                                    relative_path: Some(relative_path),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    results
}

/// Progress event payload sent to frontend during transfer streaming.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub session_token: String,
    pub file_id: String,
    pub file_name: String,
    pub sent_bytes: u64,
    pub total_bytes: u64,
    pub percent: u32,
    #[serde(default)]
    pub speed_bps: u64,
    #[serde(default)]
    pub eta_seconds: Option<u64>,
}

/// Event payload emitted when an individual file transfer completes successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferCompletedPayload {
    pub session_token: String,
    pub file_id: String,
    pub file_name: String,
    pub saved_path: String,
}

/// Event payload emitted when an entire batch of file transfers completes successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferBatchCompletedPayload {
    pub session_token: String,
    pub total_files: usize,
    pub total_bytes: u64,
}

/// Event payload emitted when an active transfer session is cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferCancelledPayload {
    pub session_token: String,
    pub reason: String,
}

/// Event payload emitted when a transfer fails or is aborted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferErrorPayload {
    pub session_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub error: String,
}

/// Payload containing local pairing credentials, formatted PIN, QR SVG, and direct URL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingInfoPayload {
    pub pin: String,
    pub formatted_pin: String,
    pub qr_svg: String,
    pub direct_url: String,
    pub ip: String,
    pub port: u16,
}

/// Builds the pairing payload containing PIN, QR SVG, and direct pairing URL for the current device.
pub async fn build_pairing_payload(state: &AppState) -> Result<PairingInfoPayload, String> {
    let device_info = state.get_device_info().await;
    let pin = match device_info.pairing_pin {
        Some(p) => p,
        None => state.regenerate_pairing_pin().await,
    };
    let ip = crate::discovery::nic::get_best_physical_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = device_info.port;
    let direct_url =
        crate::pairing::qr::build_pairing_url(&ip, port, &pin, &device_info.device_name);
    let qr_svg = crate::pairing::qr::generate_qr_svg(&direct_url)?;
    let formatted_pin = crate::pairing::pin::format_pin(&pin);

    Ok(PairingInfoPayload {
        pin,
        formatted_pin,
        qr_svg,
        direct_url,
        ip,
        port,
    })
}

/// Returns the current device identity and server metadata.
#[tauri::command]
pub async fn get_my_device_info(state: State<'_, AppState>) -> Result<DeviceInfo, String> {
    Ok(state.get_device_info().await)
}

/// Returns the local device's pairing info including 6-digit PIN, QR SVG, and direct URL.
/// Viewing it does not open pairing; the user turns pairing on explicitly.
#[tauri::command]
pub async fn get_my_pairing_info(
    state: State<'_, AppState>,
) -> Result<PairingInfoPayload, String> {
    build_pairing_payload(&state).await
}

/// Regenerates the 6-digit pairing PIN and returns the updated pairing info payload.
#[tauri::command]
pub async fn regenerate_pairing_pin(
    state: State<'_, AppState>,
) -> Result<PairingInfoPayload, String> {
    state.regenerate_pairing_pin().await;
    build_pairing_payload(&state).await
}

/// Whether this device accepts pairing, and for how much longer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingStatusPayload {
    pub open: bool,
    pub remaining_secs: u64,
}

fn pairing_status(state: &AppState) -> PairingStatusPayload {
    match state.server_state.pairing_remaining() {
        Some(left) => PairingStatusPayload {
            open: true,
            remaining_secs: left.as_secs().max(1),
        },
        None => PairingStatusPayload {
            open: false,
            remaining_secs: 0,
        },
    }
}

/// Returns whether pairing is open and the time left.
#[tauri::command]
pub async fn get_pairing_status(state: State<'_, AppState>) -> Result<PairingStatusPayload, String> {
    Ok(pairing_status(&state))
}

/// Returns whether paired devices are remembered across restarts, and how many there are.
#[tauri::command]
pub async fn get_paired_devices_status(
    state: State<'_, AppState>,
) -> Result<crate::app_state::PairedDevicesStatus, String> {
    Ok(state.paired_devices_status().await)
}

/// Turns "Remember paired devices" on (pairings saved to disk) or off (saved pairings deleted).
#[tauri::command]
pub async fn set_remember_paired_devices(
    state: State<'_, AppState>,
    remember: bool,
) -> Result<crate::app_state::PairedDevicesStatus, String> {
    state.set_remember_paired_devices(remember).await
}

/// Forgets every pairing in both directions; devices must pair again with a PIN.
#[tauri::command]
pub async fn forget_paired_devices(
    state: State<'_, AppState>,
) -> Result<crate::app_state::PairedDevicesStatus, String> {
    Ok(state.forget_paired_devices().await)
}

/// Turns pairing off (`minutes = 0`) or on for 10 or 30 minutes, restarting the timer.
#[tauri::command]
pub async fn set_pairing_mode(
    state: State<'_, AppState>,
    minutes: u64,
) -> Result<PairingStatusPayload, String> {
    let window = match minutes {
        0 => None,
        m if crate::server::state::PAIRING_WINDOW_CHOICES_MINUTES.contains(&m) => {
            Some(std::time::Duration::from_secs(m * 60))
        }
        other => return Err(format!("Unsupported pairing duration: {} minutes", other)),
    };
    state.server_state.set_pairing_window(window);
    Ok(pairing_status(&state))
}

/// Connects to a remote peer on the local network by scanning for a matching 6-digit PIN.
#[tauri::command]
pub async fn connect_by_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    pin: String,
) -> Result<PeerInfo, String> {
    let normalized = crate::pairing::pin::normalize_pin(&pin)
        .ok_or_else(|| format!("Invalid 6-digit pairing PIN: '{}'", pin))?;

    let peers = state.get_peers().await;

    // 1. Find discovered peers that are showing a PIN right now (pairing dialog open), then run
    //    the SPAKE2 pairing only with those. The PIN is never sent, and devices that are not
    //    pairing are never sent an attempt.
    let mut matching_peer = None;
    let mut mismatched_device = None;
    for p in &peers {
        match state.transfer_client.pair_open_peer(p, &normalized).await {
            Ok(info) => {
                let mut paired = p.clone();
                paired.device_name = info.device_name;
                paired.pairing_pin = None;
                matching_peer = Some(paired);
                break;
            }
            Err(crate::client::ClientError::Other(msg)) if msg == "Pairing PIN does not match" => {
                mismatched_device = Some(p.device_name.clone());
            }
            Err(_) => {}
        }
    }

    // A nearby device is pairing but rejected the PIN: most likely a typo, so stop here
    // instead of sweeping the network.
    if matching_peer.is_none() {
        if let Some(name) = mismatched_device {
            return Err(format!(
                "The PIN does not match {}. Check the PIN shown on that device.",
                name
            ));
        }
    }

    // 2. Fall back to sweeping all candidate subnets (Ethernet first, then Wi-Fi) for matching PIN
    let peer = match matching_peer {
        Some(p) => p,
        None => {
            crate::pairing::scanner::sweep_all_subnets_for_pin(
                &state.transfer_client,
                &normalized,
                5050,
                50,
                400,
            )
            .await?
        }
    };

    state.add_manual_peer(peer.clone()).await;
    state.save_paired_devices().await;
    let _ = app.emit("peer-found", &peer);
    Ok(peer)
}

/// Connects to a remote peer directly using an IP address, IP:port, HTTP URL, or EasyShare QR URL.
#[tauri::command]
pub async fn connect_by_address(
    app: AppHandle,
    state: State<'_, AppState>,
    address: String,
) -> Result<PeerInfo, String> {
    let mut peer =
        crate::pairing::scanner::resolve_peer_by_address(&state.transfer_client, &address).await?;

    // A QR code carries the PIN: pair right away so the PIN is never kept or sent onwards.
    if let Some(pin) = peer.pairing_pin.take() {
        state
            .transfer_client
            .pair_peer(&peer, &pin)
            .await
            .map_err(|e| format!("Pairing with {} failed: {}", peer.device_name, e))?;
    }

    state.add_manual_peer(peer.clone()).await;
    state.save_paired_devices().await;
    let _ = app.emit("peer-found", &peer);
    Ok(peer)
}

/// Returns the currently discovered active peers on the network.
#[tauri::command]
pub async fn get_discovered_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>, String> {
    Ok(state.get_peers().await)
}

/// Probes a remote peer at `ip:port` and adds it to the peer list.
#[tauri::command]
pub async fn add_manual_peer(
    app: AppHandle,
    state: State<'_, AppState>,
    ip: String,
    port: Option<u16>,
) -> Result<PeerInfo, String> {
    let port = port.unwrap_or(5050);
    let url = format!("http://{}:{}", ip, port);

    let peer = match state.transfer_client.probe_peer_url(&url).await {
        Ok(info) => PeerInfo::new(
            info.device_name,
            info.device_type,
            info.os,
            ip,
            port,
        ),
        Err(_) => PeerInfo::new(
            format!("Manual-{}", ip),
            "unknown",
            "unknown",
            ip,
            port,
        ),
    };

    state.add_manual_peer(peer.clone()).await;
    state.save_paired_devices().await;
    let _ = app.emit("peer-found", &peer);
    Ok(peer)
}

/// Responds to an interactive transfer consent prompt (Accept/Decline).
#[tauri::command]
pub async fn respond_transfer_request(
    state: State<'_, AppState>,
    request_id: String,
    accept: bool,
) -> Result<(), String> {
    let mut pending = state.pending_consents.write().await;
    if let Some(responder) = pending.remove(&request_id) {
        // send fails when the sender side already timed out and stopped waiting
        responder
            .send(accept)
            .map_err(|_| "This transfer request has expired".to_string())
    } else {
        Err("Pending transfer request not found or expired".to_string())
    }
}

/// Initiates an outbound file transfer to a target peer.
#[tauri::command]
pub async fn start_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    peer: PeerInfo,
    files: Vec<FileSelection>,
) -> Result<String, String> {
    if files.is_empty() {
        return Err("No files selected for transfer".to_string());
    }

    // Verify file existence first
    for sel in &files {
        let path = PathBuf::from(&sel.path);
        if !path.exists() {
            return Err(format!("File does not exist: {}", sel.path));
        }
    }

    // Offload BLAKE3 hash calculation concurrently to Tokio blocking threadpool
    let mut tasks = Vec::with_capacity(files.len());
    for (idx, sel) in files.into_iter().enumerate() {
        let path = PathBuf::from(&sel.path);
        let file_id = sel
            .file_id
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("f_{}_{}", idx, uuid::Uuid::new_v4()));
        let raw_path = sel.path.clone();
        let custom_name = sel.name;
        let rel_path = sel.relative_path;
        tasks.push(tokio::spawn(async move {
            let mut file_obj = FileToSend::from_path_async(file_id, path)
                .await
                .map_err(|e| format!("Failed to read file {}: {}", raw_path, e))?;

            if let Some(custom_name) = custom_name {
                if !custom_name.trim().is_empty() {
                    file_obj.name = custom_name;
                }
            }
            if let Some(rel) = rel_path {
                file_obj = file_obj.with_relative_path(rel);
            }
            Ok::<FileToSend, String>(file_obj)
        }));
    }

    let mut files_to_send = Vec::with_capacity(tasks.len());
    for task in tasks {
        let file_obj = task
            .await
            .map_err(|e| format!("File preparation task failed: {}", e))??;
        files_to_send.push(file_obj);
    }

    let my_info = state.server_state.get_device_info().await;
    let request_id = uuid::Uuid::new_v4().to_string();
    let metadata_vec = files_to_send.iter().map(|f| f.to_metadata()).collect();

    let encryption_enabled = state.is_encryption_enabled().await;
    let mut ephemeral_secret_opt = None;

    let mut transfer_request = TransferRequest::new(
        request_id.clone(),
        my_info.device_name,
        my_info.os,
        metadata_vec,
    );

    let mut pairing = None;
    let mut peer_device_id = peer.device_id.clone();
    if encryption_enabled {
        // Pairings are kept by the peer's stable device id; ask the peer if discovery didn't say
        if peer_device_id.is_none() {
            peer_device_id = state
                .transfer_client
                .probe_peer(&peer)
                .await
                .ok()
                .and_then(|info| info.device_id);
        }

        if let Some(ref id) = peer_device_id {
            pairing = state.transfer_client.get_pairing(id).await;
        }
        if pairing.is_none() {
            if let Some(ref pin) = peer.pairing_pin {
                let info = state
                    .transfer_client
                    .pair_peer(&peer, pin)
                    .await
                    .map_err(|e| format!("Pairing with {} failed: {}", peer.device_name, e))?;
                peer_device_id = info.device_id.or(peer_device_id);
                if let Some(ref id) = peer_device_id {
                    pairing = state.transfer_client.get_pairing(id).await;
                }
                state.save_paired_devices().await;
            }
        }

        // A device that forgot our pairing must be paired again; never fall back to sending
        // without the pairing check.
        if pairing.is_none() {
            if let Some(ref id) = peer_device_id {
                if state.transfer_client.needs_repair(id).await {
                    return Err(format!(
                        "{} no longer recognizes this device. Pair with it again using its PIN before sending.",
                        peer.device_name
                    ));
                }
            }
        }

        let (sender_secret, sender_pub) = crate::crypto::generate_ephemeral_keypair();
        let sender_pub_hex = crate::crypto::public_key_to_hex(&sender_pub);
        transfer_request.encryption_mode = Some("ChaCha20Poly1305".to_string());
        if let Some(ref p) = pairing {
            transfer_request.pair_id = Some(p.pair_id.clone());
            transfer_request.pair_auth = Some(crate::pairing::pake::request_auth(
                &p.key,
                &request_id,
                &sender_pub_hex,
            ));
        }
        transfer_request.ephemeral_public_key = Some(sender_pub_hex);
        ephemeral_secret_opt = Some(sender_secret);
    }

    let response = match state
        .transfer_client
        .request_transfer(&peer, &transfer_request)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            let msg = e.to_string();
            // The peer no longer knows this pairing (it restarted without remembering devices,
            // or forgot them). Block sending to it until it is paired again.
            if let (Some(p), true) = (pairing.as_ref(), msg.contains("Unknown pairing")) {
                state.transfer_client.mark_needs_repair(&p.device_id).await;
                state.save_paired_devices().await;
                return Err(format!(
                    "{} no longer recognizes this device. Pair with it again using its PIN before sending.",
                    peer.device_name
                ));
            }
            return Err(format!("Transfer handshake failed: {}", msg));
        }
    };

    match response.status {
        TransferStatus::Accepted => {
            let session_token = response.session_token.clone().ok_or_else(|| {
                "Receiver accepted transfer but did not return a session token".to_string()
            })?;

            let mut session_key = None;
            // We asked for encryption, so anything short of a verified encrypted session is
            // refused rather than sent in plaintext.
            if let Some(sender_secret) = ephemeral_secret_opt {
                match crate::client::negotiate_session_key(
                    sender_secret,
                    pairing.as_ref(),
                    &request_id,
                    &session_token,
                    &response,
                ) {
                    Ok(key) => {
                        state
                            .transfer_client
                            .register_session_key(&session_token, key.clone())
                            .await;
                        session_key = Some(key);
                    }
                    Err(err) => {
                        use crate::client::KeyNegotiationError;
                        let (reason, message) = match err {
                            KeyNegotiationError::NotEncrypted => (
                                "Encryption required",
                                format!(
                                    "{} has encryption turned off. Turn it on there, or turn off encryption here, to send.",
                                    peer.device_name
                                ),
                            ),
                            KeyNegotiationError::InvalidKey => (
                                "Invalid encryption key",
                                format!(
                                    "{} sent an invalid encryption key; nothing was sent",
                                    peer.device_name
                                ),
                            ),
                            KeyNegotiationError::PairingUnproven => (
                                "Pairing verification failed",
                                format!(
                                    "{} could not prove the pairing; pair with it again",
                                    peer.device_name
                                ),
                            ),
                        };
                        let _ = state
                            .transfer_client
                            .cancel_transfer(&peer, &session_token, Some(reason.to_string()))
                            .await;
                        return Err(message);
                    }
                }
            }

            let cancel_token = tokio_util::sync::CancellationToken::new();
            let speed_limit = state.get_speed_limit().await;
            let transfer_control = crate::client::TransferControl::from_cancellation_token(cancel_token.clone())
                .with_speed_limit(speed_limit)
                .with_session_key(session_key);
            state
                .register_cancellation_token(&session_token, cancel_token.clone())
                .await;
            state
                .register_transfer_control(&session_token, transfer_control.clone())
                .await;

            let client = state.transfer_client.clone();
            let peer_clone = peer.clone();
            let token_clone = session_token.clone();
            let app_handle = app.clone();
            let control_clone = transfer_control.clone();
            let tokens_registry = state.cancellation_tokens.clone();
            let controls_registry = state.transfer_controls.clone();
            let total_files = files_to_send.len();
            let total_batch_bytes = files_to_send.iter().map(|f| f.size).sum::<u64>();
            let files_to_send_clone = files_to_send.clone();
            let config_dir = state.config_dir.clone();

            tokio::spawn(async move {
                let record_sender_history = |status: crate::storage::history::HistoryStatus, err_msg: Option<String>| {
                    let record = crate::storage::history::TransferHistoryRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_token: Some(token_clone.clone()),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0),
                        direction: crate::storage::history::TransferDirection::Sent,
                        peer_name: peer_clone.device_name.clone(),
                        peer_ip: peer_clone.ip.clone(),
                        status,
                        files: files_to_send_clone
                            .iter()
                            .map(|f| crate::storage::history::HistoryFileItem {
                                name: f.name.clone(),
                                path: Some(f.path.to_string_lossy().to_string()),
                                size: f.size,
                            })
                            .collect(),
                        total_bytes: total_batch_bytes,
                        error_message: err_msg,
                    };
                    if crate::storage::history::add_record(&config_dir, record).is_ok() {
                        let hist = crate::storage::history::load_history(&config_dir);
                        let _ = app_handle.emit("history-updated", &hist);
                    }
                };

                for file in files_to_send {
                    if control_clone.is_cancelled() {
                        let _ = app_handle.emit(
                            "transfer-cancelled",
                            &TransferCancelledPayload {
                                session_token: token_clone.clone(),
                                reason: "Transfer cancelled by user".to_string(),
                            },
                        );
                        let _ = client
                            .cancel_transfer(
                                &peer_clone,
                                &token_clone,
                                Some("Cancelled by user".to_string()),
                            )
                            .await;
                        record_sender_history(
                            crate::storage::history::HistoryStatus::Cancelled,
                            Some("Transfer cancelled by user".to_string()),
                        );
                        tokens_registry.write().await.remove(&token_clone);
                        controls_registry.write().await.remove(&token_clone);
                        return;
                    }

                    let file_id = file.file_id.clone();
                    let file_name = file.name.clone();
                    let total_bytes = file.size;

                    let tracker = std::sync::Arc::new(std::sync::Mutex::new(
                        crate::client::ThroughputTracker::new(),
                    ));

                    // Initial 0% progress event
                    let _ = app_handle.emit(
                        "transfer-progress",
                        &ProgressPayload {
                            session_token: token_clone.clone(),
                            file_id: file_id.clone(),
                            file_name: file_name.clone(),
                            sent_bytes: 0,
                            total_bytes,
                            percent: 0,
                            speed_bps: 0,
                            eta_seconds: None,
                        },
                    );

                    let app_cb = app_handle.clone();
                    let fid_cb = file_id.clone();
                    let fname_cb = file_name.clone();
                    let tok_cb = token_clone.clone();
                    let tracker_cb = tracker.clone();

                    let send_res = client
                        .send_file_resumable_controlled(
                            &peer_clone,
                            &token_clone,
                            &file,
                            Some(&control_clone),
                            move |sent, total| {
                                let percent = if total > 0 {
                                    ((sent as f64 / total as f64) * 100.0) as u32
                                } else {
                                    100
                                };
                                let (speed_bps, eta_seconds) = {
                                    let mut tr = tracker_cb.lock().unwrap();
                                    tr.update(sent, total, std::time::Instant::now())
                                };
                                let _ = app_cb.emit(
                                    "transfer-progress",
                                    &ProgressPayload {
                                        session_token: tok_cb.clone(),
                                        file_id: fid_cb.clone(),
                                        file_name: fname_cb.clone(),
                                        sent_bytes: sent,
                                        total_bytes: total,
                                        percent,
                                        speed_bps,
                                        eta_seconds,
                                    },
                                );
                                crate::tray::update_tray_tooltip(
                                    &app_cb,
                                    &crate::tray::format_transfer_tooltip(
                                        "Sending",
                                        percent,
                                        Some(speed_bps),
                                        Some(&fname_cb),
                                    ),
                                );
                            },
                        )
                        .await;

                    if let Err(err) = send_res {
                        if matches!(err, ClientError::Cancelled) || control_clone.is_cancelled() {
                            let _ = app_handle.emit(
                                "transfer-cancelled",
                                &TransferCancelledPayload {
                                    session_token: token_clone.clone(),
                                    reason: "Transfer cancelled by user".to_string(),
                                },
                            );
                            let _ = client
                                .cancel_transfer(
                                    &peer_clone,
                                    &token_clone,
                                    Some("Cancelled by user".to_string()),
                                )
                                .await;
                            record_sender_history(
                                crate::storage::history::HistoryStatus::Cancelled,
                                Some("Transfer cancelled by user".to_string()),
                            );
                            crate::tray::update_tray_tooltip(&app_handle, "Easy Share - Ready");
                            tokens_registry.write().await.remove(&token_clone);
                            controls_registry.write().await.remove(&token_clone);
                            return;
                        }

                        if err.is_cancelled_by_peer() {
                            let _ = app_handle.emit(
                                "transfer-cancelled",
                                &TransferCancelledPayload {
                                    session_token: token_clone.clone(),
                                    reason: "Transfer cancelled by receiver".to_string(),
                                },
                            );
                            record_sender_history(
                                crate::storage::history::HistoryStatus::Cancelled,
                                Some("Transfer cancelled by receiver".to_string()),
                            );
                            crate::tray::update_tray_tooltip(&app_handle, "Easy Share - Ready");
                            tokens_registry.write().await.remove(&token_clone);
                            controls_registry.write().await.remove(&token_clone);
                            return;
                        }

                        let err_msg = err.to_string();
                        let _ = app_handle.emit(
                            "transfer-error",
                            &TransferErrorPayload {
                                session_token: token_clone.clone(),
                                file_id: Some(file_id.clone()),
                                file_name: Some(file_name.clone()),
                                error: err_msg.clone(),
                            },
                        );
                        record_sender_history(
                            crate::storage::history::HistoryStatus::Failed,
                            Some(err_msg),
                        );
                        crate::tray::update_tray_tooltip(&app_handle, "Easy Share - Ready");
                        tokens_registry.write().await.remove(&token_clone);
                        controls_registry.write().await.remove(&token_clone);
                        return;
                    }

                    if control_clone.is_cancelled() {
                        let _ = app_handle.emit(
                            "transfer-cancelled",
                            &TransferCancelledPayload {
                                session_token: token_clone.clone(),
                                reason: "Transfer cancelled by user".to_string(),
                            },
                        );
                        let _ = client
                            .cancel_transfer(
                                &peer_clone,
                                &token_clone,
                                Some("Cancelled by user".to_string()),
                            )
                            .await;
                        record_sender_history(
                            crate::storage::history::HistoryStatus::Cancelled,
                            Some("Transfer cancelled by user".to_string()),
                        );
                        crate::tray::update_tray_tooltip(&app_handle, "Easy Share - Ready");
                        tokens_registry.write().await.remove(&token_clone);
                        controls_registry.write().await.remove(&token_clone);
                        return;
                    }

                    // Finalize the file with checksum verification
                    match client
                        .finish_transfer(&peer_clone, &token_clone, &file.file_id)
                        .await
                    {
                        Ok(finish_resp) => {
                            let _ = app_handle.emit(
                                "transfer-completed",
                                &TransferCompletedPayload {
                                    session_token: token_clone.clone(),
                                    file_id: file.file_id.clone(),
                                    file_name: file.name.clone(),
                                    saved_path: finish_resp.saved_path,
                                },
                            );
                        }
                        Err(err) => {
                            let err_msg = format!("Failed to finalize transfer: {}", err);
                            let _ = app_handle.emit(
                                "transfer-error",
                                &TransferErrorPayload {
                                    session_token: token_clone.clone(),
                                    file_id: Some(file.file_id.clone()),
                                    file_name: Some(file.name.clone()),
                                    error: err_msg.clone(),
                                },
                            );
                            record_sender_history(
                                crate::storage::history::HistoryStatus::Failed,
                                Some(err_msg),
                            );
                            crate::tray::update_tray_tooltip(&app_handle, "Easy Share - Ready");
                            tokens_registry.write().await.remove(&token_clone);
                            controls_registry.write().await.remove(&token_clone);
                            return;
                        }
                    }
                }

                // Clean up token from registry
                tokens_registry.write().await.remove(&token_clone);
                controls_registry.write().await.remove(&token_clone);

                // Record successful batch completion in history
                record_sender_history(crate::storage::history::HistoryStatus::Completed, None);

                crate::tray::update_tray_tooltip(&app_handle, "Easy Share - Ready");
                crate::notifications::notify_transfer_completed(
                    &app_handle,
                    &peer_clone.device_name,
                    total_files,
                    false,
                );

                // Emit batch completion when all files finish successfully
                let _ = app_handle.emit(
                    "transfer-batch-completed",
                    &TransferBatchCompletedPayload {
                        session_token: token_clone,
                        total_files,
                        total_bytes: total_batch_bytes,
                    },
                );
            });

            Ok(session_token)
        }
        TransferStatus::Declined => {
            let reason = response
                .reason
                .unwrap_or_else(|| "Declined by recipient".to_string());
            Err(format!("Transfer declined: {}", reason))
        }
        status => Err(format!(
            "Transfer rejected with unexpected status: {:?}",
            status
        )),
    }
}

/// Cancels an active transfer session.
#[tauri::command]
pub async fn cancel_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    session_token: String,
    peer: Option<PeerInfo>,
) -> Result<(), String> {
    // 1. Cancel active outbound transfer via CancellationToken if present
    state.cancel_session(&session_token).await;

    // 2. Notify remote peer
    if let Some(peer_info) = peer {
        let _ = state
            .transfer_client
            .cancel_transfer(&peer_info, &session_token, Some("Cancelled by user".to_string()))
            .await;
    }

    // 3. If this device was the receiver, cancel the session: refuse further chunks, delete
    //    the partial files and record it in history
    state.server_state.cancel_session(&session_token, false).await;

    crate::tray::update_tray_tooltip(&app, "Easy Share - Ready");

    Ok(())
}

/// Pauses an active outbound transfer session.
#[tauri::command]
pub async fn pause_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    session_token: String,
) -> Result<bool, String> {
    let paused = state.pause_transfer(&session_token).await;
    if paused {
        crate::tray::update_tray_tooltip(&app, "Easy Share - Paused");
        let _ = app.emit("transfer-paused", serde_json::json!({ "session_token": session_token }));
    }
    Ok(paused)
}

/// Resumes a paused outbound transfer session.
#[tauri::command]
pub async fn resume_transfer(
    app: AppHandle,
    state: State<'_, AppState>,
    session_token: String,
) -> Result<bool, String> {
    let resumed = state.resume_transfer(&session_token).await;
    if resumed {
        let _ = app.emit("transfer-resumed", serde_json::json!({ "session_token": session_token }));
    }
    Ok(resumed)
}

/// Returns the current download directory path.
#[tauri::command]
pub async fn get_download_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.get_download_dir().to_string_lossy().to_string())
}

/// Updates the download directory path and ensures it exists.
#[tauri::command]
pub async fn set_download_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let new_path = state.set_download_dir(path).await?;
    Ok(new_path.to_string_lossy().to_string())
}

/// Updates the local device display name and notifies the frontend.
#[tauri::command]
pub async fn set_device_name(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<DeviceInfo, String> {
    let updated_info = state.set_device_name(name).await?;
    let _ = app.emit("device-info-updated", &updated_info);
    Ok(updated_info)
}

/// Opens the download directory in the native file manager (Explorer/Finder/Files).
#[tauri::command]
pub async fn open_download_dir(state: State<'_, AppState>) -> Result<(), String> {
    let dir = state.get_download_dir();
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }
    Ok(())
}

/// Returns the current global upload speed limit in bytes per second (0 = unlimited).
#[tauri::command]
pub async fn get_speed_limit(state: State<'_, AppState>) -> Result<u64, String> {
    Ok(state.get_speed_limit().await)
}

/// Updates the global upload speed limit in bytes per second and updates active transfers.
#[tauri::command]
pub async fn set_speed_limit(
    state: State<'_, AppState>,
    limit_bps: u64,
) -> Result<u64, String> {
    state.set_speed_limit(limit_bps).await;
    Ok(limit_bps)
}

/// Inspects an array of absolute filesystem paths (e.g. from native window drag-drop)
/// and resolves their file metadata (name, size, full path).
#[tauri::command]
pub async fn inspect_paths(paths: Vec<String>) -> Result<Vec<SelectedFile>, String> {
    Ok(inspect_file_paths(&paths))
}

/// Spawns a native OS file dialog allowing user to pick one or more files.
/// Returns resolved file metadata for all selected items.
#[tauri::command]
pub async fn pick_files() -> Result<Vec<SelectedFile>, String> {
    let files = rfd::AsyncFileDialog::new()
        .set_title("Select files to send")
        .pick_files()
        .await;

    match files {
        Some(handles) => {
            let paths: Vec<PathBuf> = handles
                .into_iter()
                .map(|h| h.path().to_path_buf())
                .collect();
            Ok(inspect_file_paths(&paths))
        }
        None => Ok(Vec::new()),
    }
}

/// Spawns a native OS folder dialog allowing user to pick a directory.
/// Returns resolved file metadata for files within the directory.
#[tauri::command]
pub async fn pick_folder() -> Result<Vec<SelectedFile>, String> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select folder to send")
        .pick_folder()
        .await;

    match folder {
        Some(handle) => {
            let paths = vec![handle.path().to_path_buf()];
            Ok(inspect_file_paths(&paths))
        }
        None => Ok(Vec::new()),
    }
}

/// Returns the full list of transfer history records.
#[tauri::command]
pub async fn get_transfer_history(
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::history::TransferHistoryRecord>, String> {
    Ok(state.get_history())
}

/// Deletes a specific transfer history record by ID and emits history-updated.
#[tauri::command]
pub async fn delete_history_record(
    app: AppHandle,
    state: State<'_, AppState>,
    record_id: String,
) -> Result<bool, String> {
    let deleted = state.delete_history_record(&record_id)?;
    if deleted {
        let history = state.get_history();
        let _ = app.emit("history-updated", &history);
    }
    Ok(deleted)
}

/// Clears all transfer history records and emits history-updated.
#[tauri::command]
pub async fn clear_transfer_history(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.clear_history()?;
    let _ = app.emit("history-updated", &Vec::<crate::storage::history::TransferHistoryRecord>::new());
    Ok(())
}

/// Opens the native OS file explorer and highlights/selects the specified file.
#[tauri::command]
pub async fn show_in_folder(path: String) -> Result<(), String> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path))
            .spawn()
            .map_err(|e| format!("Failed to open file in explorer: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open file in finder: {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        let parent = file_path.parent().unwrap_or(&file_path);
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    Ok(())
}

/// Opens the specified file using the system's default application.
///
/// Uses the OS open API directly rather than `cmd /c start`, so shell metacharacters in
/// peer-chosen file names (`&`, `^`, `|`) are never interpreted as commands.
#[tauri::command]
pub async fn open_file(path: String) -> Result<(), String> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    tauri_plugin_opener::open_path(&file_path, None::<&str>)
        .map_err(|e| format!("Failed to open file: {}", e))
}

/// Payload describing system tray and notification preferences.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraySettingsPayload {
    #[serde(alias = "minimizeToTray")]
    pub minimize_to_tray: bool,
    #[serde(alias = "notificationsEnabled")]
    pub notifications_enabled: bool,
}

/// Returns the current system tray and desktop notification settings.
#[tauri::command]
pub async fn get_tray_settings(state: State<'_, AppState>) -> Result<TraySettingsPayload, String> {
    let (minimize_to_tray, notifications_enabled) = state.get_tray_settings().await;
    Ok(TraySettingsPayload {
        minimize_to_tray,
        notifications_enabled,
    })
}

/// Updates and persists system tray and desktop notification settings.
#[tauri::command]
pub async fn set_tray_settings(
    state: State<'_, AppState>,
    minimize_to_tray: bool,
    notifications_enabled: bool,
) -> Result<TraySettingsPayload, String> {
    let (min, notif) = state
        .set_tray_settings(minimize_to_tray, notifications_enabled)
        .await?;
    Ok(TraySettingsPayload {
        minimize_to_tray: min,
        notifications_enabled: notif,
    })
}

/// The networks this device can use, and the one it is using now.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkOptionsPayload {
    pub options: Vec<crate::discovery::nic::NetworkOption>,
    pub selected: Option<String>,
}

fn network_options_payload() -> NetworkOptionsPayload {
    let options = crate::discovery::nic::list_network_options();
    let current = crate::discovery::nic::get_best_physical_ip().map(|ip| ip.to_string());
    let selected = options
        .iter()
        .find(|option| Some(&option.ip) == current.as_ref())
        .map(|option| option.adapter.clone());
    NetworkOptionsPayload { options, selected }
}

/// Returns the networks this device can use.
#[tauri::command]
pub async fn get_network_options() -> Result<NetworkOptionsPayload, String> {
    Ok(network_options_payload())
}

/// Selects the network whose address other devices should use to reach this one.
#[tauri::command]
pub async fn set_network_adapter(
    state: State<'_, AppState>,
    adapter: String,
) -> Result<NetworkOptionsPayload, String> {
    let known = crate::discovery::nic::list_network_options()
        .iter()
        .any(|option| option.adapter == adapter);
    if !known {
        return Err(format!("Network '{}' is no longer available", adapter));
    }
    state.set_preferred_adapter(adapter).await?;
    Ok(network_options_payload())
}

/// Returns whether end-to-end encryption is enabled.
#[tauri::command]
pub async fn get_encryption_settings(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.is_encryption_enabled().await)
}

/// Updates and persists the end-to-end encryption preference.
#[tauri::command]
pub async fn set_encryption_settings(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<bool, String> {
    state.set_encryption_enabled(enabled).await
}
