use crate::app_state::AppState;
use crate::client::FileToSend;
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
}

/// Detailed representation of a selected file on the host machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedFile {
    pub name: String,
    pub path: String,
    pub size: u64,
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
            });
        } else if path_ref.is_dir() {
            if let Ok(entries) = std::fs::read_dir(path_ref) {
                for entry in entries.flatten() {
                    let sub_path = entry.path();
                    if sub_path.is_file() {
                        let name = sub_path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unnamed_file".to_string());
                        let size = std::fs::metadata(&sub_path).map(|m| m.len()).unwrap_or(0);
                        results.push(SelectedFile {
                            name,
                            path: sub_path.to_string_lossy().to_string(),
                            size,
                        });
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

    // 1. Check if any cached peer already has this PIN explicitly (e.g. from QR scan)
    let mut matching_peer = peers
        .iter()
        .find(|p| {
            p.pairing_pin
                .as_deref()
                .and_then(crate::pairing::pin::normalize_pin)
                .map(|p_pin| p_pin == normalized)
                .unwrap_or(false)
        })
        .cloned();

    // 2. Try authenticating directly against discovered peers via /api/v1/pair
    if matching_peer.is_none() {
        for p in &peers {
            if let Ok(info) = state.transfer_client.pair_peer(p, &normalized).await {
                let mut paired = p.clone();
                paired.device_name = info.device_name;
                paired.pairing_pin = Some(normalized.clone());
                matching_peer = Some(paired);
                break;
            }
        }
    }

    // 3. Fall back to sweeping the local subnet for matching PIN
    let peer = match matching_peer {
        Some(p) => p,
        None => {
            let local_ip = crate::discovery::nic::get_best_physical_ip()
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "127.0.0.1".to_string());
            crate::pairing::scanner::sweep_subnet_for_pin(
                &state.transfer_client,
                &local_ip,
                &normalized,
                5050,
                50,
                250,
            )
            .await?
        }
    };

    state.add_manual_peer(peer.clone()).await;
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
    let peer =
        crate::pairing::scanner::resolve_peer_by_address(&state.transfer_client, &address).await?;
    state.add_manual_peer(peer.clone()).await;
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
        let _ = responder.send(accept);
        Ok(())
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
        tasks.push(tokio::spawn(async move {
            let mut file_obj = FileToSend::from_path_async(file_id, path)
                .await
                .map_err(|e| format!("Failed to read file {}: {}", raw_path, e))?;

            if let Some(custom_name) = custom_name {
                if !custom_name.trim().is_empty() {
                    file_obj.name = custom_name;
                }
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

    let transfer_request = TransferRequest::new(
        request_id,
        my_info.device_name,
        my_info.os,
        metadata_vec,
    );

    let response = state
        .transfer_client
        .request_transfer(&peer, &transfer_request)
        .await
        .map_err(|e| format!("Transfer handshake failed: {}", e))?;

    match response.status {
        TransferStatus::Accepted => {
            let session_token = response.session_token.ok_or_else(|| {
                "Receiver accepted transfer but did not return a session token".to_string()
            })?;

            let client = state.transfer_client.clone();
            let peer_clone = peer.clone();
            let token_clone = session_token.clone();
            let app_handle = app.clone();
            let total_files = files_to_send.len();
            let total_batch_bytes = files_to_send.iter().map(|f| f.size).sum::<u64>();

            tokio::spawn(async move {
                for file in files_to_send {
                    let file_id = file.file_id.clone();
                    let file_name = file.name.clone();
                    let total_bytes = file.size;

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
                        },
                    );

                    let app_cb = app_handle.clone();
                    let fid_cb = file_id.clone();
                    let fname_cb = file_name.clone();
                    let tok_cb = token_clone.clone();

                    let send_res = client
                        .send_file_resumable(
                            &peer_clone,
                            &token_clone,
                            &file,
                            move |sent, total| {
                                let percent = if total > 0 {
                                    ((sent as f64 / total as f64) * 100.0) as u32
                                } else {
                                    100
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
                                    },
                                );
                            },
                        )
                        .await;

                    if let Err(err) = send_res {
                        let _ = app_handle.emit(
                            "transfer-error",
                            &TransferErrorPayload {
                                session_token: token_clone.clone(),
                                file_id: Some(file_id.clone()),
                                file_name: Some(file_name.clone()),
                                error: err.to_string(),
                            },
                        );
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
                            let _ = app_handle.emit(
                                "transfer-error",
                                &TransferErrorPayload {
                                    session_token: token_clone.clone(),
                                    file_id: Some(file.file_id.clone()),
                                    file_name: Some(file.name.clone()),
                                    error: format!("Failed to finalize transfer: {}", err),
                                },
                            );
                            return;
                        }
                    }
                }

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
    state: State<'_, AppState>,
    session_token: String,
    peer: Option<PeerInfo>,
) -> Result<(), String> {
    if let Some(peer_info) = peer {
        let _ = state
            .transfer_client
            .cancel_transfer(&peer_info, &session_token, Some("Cancelled by user".to_string()))
            .await;
    }

    // Cancel in local server state if this device was the receiver
    let mut sessions = state.server_state.sessions.write().await;
    if let Some(session) = sessions.get_mut(&session_token) {
        session.status = TransferStatus::Cancelled;
    }

    Ok(())
}

/// Returns the current download directory path.
#[tauri::command]
pub async fn get_download_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.download_dir.to_string_lossy().to_string())
}

/// Opens the download directory in the native file manager (Explorer/Finder/Files).
#[tauri::command]
pub async fn open_download_dir(state: State<'_, AppState>) -> Result<(), String> {
    let dir = &state.download_dir;
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
