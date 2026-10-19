use crate::models::peer::DeviceInfo;
use crate::models::transfer::{
    CancelRequest, ChunkResponse, FinishRequest, FinishResponse, TransferRequest, TransferResponse,
    TransferStatus, TransferStatusQueryResponse,
};
use crate::server::state::ServerState;
use crate::storage::file_manager::FileManager;
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

#[derive(Debug)]
pub enum ServerError {
    Io(std::io::Error),
    BindFailed(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServerError::Io(e) => write!(f, "Server I/O error: {}", e),
            ServerError::BindFailed(msg) => write!(f, "Server bind error: {}", msg),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<std::io::Error> for ServerError {
    fn from(e: std::io::Error) -> Self {
        ServerError::Io(e)
    }
}

/// GET /api/v1/info
pub async fn handle_info(State(state): State<Arc<ServerState>>) -> Json<DeviceInfo> {
    Json(state.get_public_device_info().await)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairRequest {
    pub pin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairResponse {
    pub success: bool,
    pub device_info: DeviceInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// POST /api/v1/pair
pub async fn handle_pair(
    State(state): State<Arc<ServerState>>,
    Json(payload): Json<PairRequest>,
) -> Result<Json<PairResponse>, (StatusCode, Json<PairResponse>)> {
    let host_pin = state.get_pairing_pin().await;
    let normalized_input = crate::pairing::pin::normalize_pin(&payload.pin);
    let normalized_host = host_pin
        .as_deref()
        .and_then(crate::pairing::pin::normalize_pin);

    let matches = match (normalized_input, normalized_host) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    };

    let pub_info = state.get_public_device_info().await;

    if matches {
        Ok(Json(PairResponse {
            success: true,
            device_info: pub_info,
            message: None,
        }))
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(PairResponse {
                success: false,
                device_info: pub_info,
                message: Some("Invalid pairing PIN".to_string()),
            }),
        ))
    }
}

/// POST /api/v1/transfer/request
pub async fn handle_transfer_request(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<TransferRequest>,
) -> Result<Json<TransferResponse>, (StatusCode, Json<TransferResponse>)> {
    match state.evaluate_consent(&request).await {
        Ok(()) => {
            let (token, offsets) = state.create_session(request).await;
            Ok(Json(TransferResponse::accepted(token, offsets)))
        }
        Err(reason) => Err((
            StatusCode::FORBIDDEN,
            Json(TransferResponse::declined(reason)),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub session_token: String,
    pub file_id: String,
}

/// GET /api/v1/transfer/status
pub async fn handle_transfer_status(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<TransferStatusQueryResponse>, (StatusCode, String)> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&query.session_token).ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid or expired session token".to_string(),
    ))?;

    if session.status == TransferStatus::Cancelled {
        return Err((
            StatusCode::BAD_REQUEST,
            "Transfer session has been cancelled".to_string(),
        ));
    }

    let file = session
        .request
        .files
        .iter()
        .find(|f| f.file_id == query.file_id)
        .ok_or((
            StatusCode::NOT_FOUND,
            "File ID not found in session".to_string(),
        ))?;

    let part_path =
        FileManager::get_part_path(&state.download_dir(), &file.name, &file.file_key());
    let bytes_received = if part_path.exists() {
        FileManager::get_file_size(&part_path).unwrap_or(0)
    } else {
        0
    };

    let status = if bytes_received == 0 {
        TransferStatus::Pending
    } else if bytes_received >= file.size {
        TransferStatus::Completed
    } else {
        TransferStatus::Partial
    };

    Ok(Json(TransferStatusQueryResponse {
        file_id: query.file_id,
        bytes_received,
        status,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ChunkQuery {
    pub session_token: String,
    pub file_id: String,
    pub offset: u64,
}

/// POST /api/v1/transfer/chunk
pub async fn handle_transfer_chunk(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<ChunkQuery>,
    body: Body,
) -> Result<Json<ChunkResponse>, (StatusCode, String)> {
    let (file_name, file_size, file_key) = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&query.session_token).ok_or((
            StatusCode::UNAUTHORIZED,
            "Invalid or expired session token".to_string(),
        ))?;

        if session.status == TransferStatus::Cancelled {
            return Err((
                StatusCode::BAD_REQUEST,
                "Transfer session has been cancelled".to_string(),
            ));
        }

        let file = session
            .request
            .files
            .iter()
            .find(|f| f.file_id == query.file_id)
            .ok_or((
                StatusCode::NOT_FOUND,
                "File ID not found in session".to_string(),
            ))?;

        (file.name.clone(), file.size, file.file_key())
    };

    if query.offset > file_size {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Chunk offset ({}) exceeds declared file size ({})",
                query.offset, file_size
            ),
        ));
    }

    let part_path = FileManager::get_part_path(&state.download_dir(), &file_name, &file_key);

    if let Some(parent) = part_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create directories: {}", e),
                )
            })?;
        }
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&part_path)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to open .part file: {}", e),
            )
        })?;

    file.seek(std::io::SeekFrom::Start(query.offset))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to seek in .part file: {}", e),
            )
        })?;

    let mut writer = tokio::io::BufWriter::with_capacity(256 * 1024, file);
    let mut bytes_written: u64 = 0;
    let mut stream = body.into_data_stream();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Error reading body stream: {}", e),
            )
        })?;

        let chunk_len = chunk.len() as u64;
        if query.offset + bytes_written + chunk_len > file_size {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Chunk data exceeds declared file size (offset={}, written={}, chunk_len={}, file_size={})",
                    query.offset, bytes_written, chunk_len, file_size
                ),
            ));
        }

        writer.write_all(&chunk).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write chunk to disk: {}", e),
            )
        })?;

        bytes_written += chunk.len() as u64;
    }

    writer.flush().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to flush buffer: {}", e),
        )
    })?;

    let current_total = query.offset + bytes_written;

    // Update progress in session
    let (speed_bps, eta_seconds) = {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&query.session_token) {
            session
                .file_progress
                .insert(query.file_id.clone(), current_total);
            let mut tr = session.tracker.lock().unwrap();
            tr.update(current_total, file_size, Instant::now())
        } else {
            (0, None)
        }
    };

    let percent = if file_size > 0 {
        ((current_total as f64 / file_size as f64) * 100.0) as u32
    } else {
        100
    };
    state.emit_event(crate::server::state::ServerTransferEvent::Progress(
        crate::server::state::ReceiveProgressPayload {
            session_token: query.session_token.clone(),
            file_id: query.file_id.clone(),
            file_name: file_name.clone(),
            received_bytes: current_total,
            total_bytes: file_size,
            percent,
            speed_bps,
            eta_seconds,
        },
    ));

    Ok(Json(ChunkResponse {
        file_id: query.file_id,
        bytes_written,
        current_total,
    }))
}

/// POST /api/v1/transfer/finish
pub async fn handle_transfer_finish(
    State(state): State<Arc<ServerState>>,
    Json(payload): Json<FinishRequest>,
) -> Result<Json<FinishResponse>, (StatusCode, String)> {
    let file = {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&payload.session_token).ok_or((
            StatusCode::UNAUTHORIZED,
            "Invalid or expired session token".to_string(),
        ))?;

        if session.status == TransferStatus::Cancelled {
            return Err((
                StatusCode::BAD_REQUEST,
                "Transfer session has been cancelled".to_string(),
            ));
        }

        session
            .request
            .files
            .iter()
            .find(|f| f.file_id == payload.file_id)
            .cloned()
            .ok_or((
                StatusCode::NOT_FOUND,
                "File ID not found in session".to_string(),
            ))?
    };

    let part_path =
        FileManager::get_part_path(&state.download_dir(), &file.name, &file.file_key());

    if !part_path.exists() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Partial transfer file does not exist on disk".to_string(),
        ));
    }

    // Verify file size
    let size_on_disk = FileManager::get_file_size(&part_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read file size: {}", e),
        )
    })?;

    if size_on_disk != file.size {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "File size mismatch: expected {} bytes, got {} bytes on disk",
                file.size, size_on_disk
            ),
        ));
    }

    // Verify BLAKE3 checksum asynchronously if provided
    if let Some(ref expected_hash) = file.blake3_hash {
        let matches =
            FileManager::verify_blake3_file_async(part_path.clone(), expected_hash.clone())
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to compute BLAKE3 hash: {}", e),
                    )
                })?;

        if !matches {
            return Err((
                StatusCode::BAD_REQUEST,
                "BLAKE3 checksum verification failed: hash mismatch".to_string(),
            ));
        }
    }

    let dest_path = FileManager::resolve_destination_path(
        &state.download_dir(),
        &file.name,
        file.relative_path.as_deref(),
    );
    let part_path_clone = part_path.clone();
    let dest_path_clone = dest_path.clone();
    tokio::task::spawn_blocking(move || {
        FileManager::commit_part_file(&part_path_clone, &dest_path_clone)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to join commit task: {}", e),
        )
    })?
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to commit .part file: {}", e),
        )
    })?;

    let saved_path_str = dest_path.to_string_lossy().to_string();

    // Update session state: mark file progress as completed and update session status
    {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&payload.session_token) {
            session.file_progress.insert(payload.file_id.clone(), file.size);
            let all_finished = session.request.files.iter().all(|f| {
                session.file_progress.get(&f.file_id).copied().unwrap_or(0) >= f.size
            });
            if all_finished {
                session.status = TransferStatus::Completed;
            }
        }
    }

    state.emit_event(crate::server::state::ServerTransferEvent::Completed(
        crate::server::state::ReceiveCompletedPayload {
            session_token: payload.session_token.clone(),
            file_id: payload.file_id.clone(),
            file_name: file.name.clone(),
            saved_path: saved_path_str.clone(),
        },
    ));

    Ok(Json(FinishResponse::completed(
        payload.file_id,
        saved_path_str,
    )))
}

/// POST /api/v1/transfer/cancel
pub async fn handle_transfer_cancel(
    State(state): State<Arc<ServerState>>,
    Json(payload): Json<CancelRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&payload.session_token).ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid or expired session token".to_string(),
    ))?;

    session.status = TransferStatus::Cancelled;

    for file in &session.request.files {
        let part_path =
            FileManager::get_part_path(&state.download_dir(), &file.name, &file.file_key());
        if part_path.exists() {
            let _ = tokio::fs::remove_file(part_path).await;
        }
    }

    state.emit_event(crate::server::state::ServerTransferEvent::Cancelled(
        crate::server::state::ReceiveCancelledPayload {
            session_token: payload.session_token.clone(),
        },
    ));

    Ok(StatusCode::OK)
}

/// Builds the Axum application router with all routes.
/// Wildcard CORS is omitted to prevent cross-origin drive-by browser exploitation.
pub fn create_router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/api/v1/info", get(handle_info))
        .route("/api/v1/pair", post(handle_pair))
        .route("/api/v1/transfer/request", post(handle_transfer_request))
        .route("/api/v1/transfer/status", get(handle_transfer_status))
        .route("/api/v1/transfer/chunk", post(handle_transfer_chunk))
        .route("/api/v1/transfer/finish", post(handle_transfer_finish))
        .route("/api/v1/transfer/cancel", post(handle_transfer_cancel))
        .with_state(state)
}

/// Starts the embedded Axum HTTP transfer server.
pub async fn start_server(
    state: Arc<ServerState>,
    port: u16,
) -> Result<(u16, tokio::task::JoinHandle<()>), ServerError> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound_port = listener.local_addr()?.port();

    state.set_port(bound_port).await;

    let app = create_router(state);

    let handle = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            eprintln!("Axum server error: {}", err);
        }
    });

    Ok((bound_port, handle))
}
