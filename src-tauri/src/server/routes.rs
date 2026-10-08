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
use serde::Deserialize;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tower_http::cors::{Any, CorsLayer};

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
    Json(state.get_device_info().await)
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
        FileManager::get_part_path(state.download_dir(), &file.name, &query.session_token);
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
    let file_name = {
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

        file.name.clone()
    };

    let part_path =
        FileManager::get_part_path(state.download_dir(), &file_name, &query.session_token);

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

    let mut bytes_written: u64 = 0;
    let mut stream = body.into_data_stream();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Error reading body stream: {}", e),
            )
        })?;

        file.write_all(&chunk).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to write chunk to disk: {}", e),
            )
        })?;

        bytes_written += chunk.len() as u64;
    }

    file.flush().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to flush .part file: {}", e),
        )
    })?;

    let current_total = query.offset + bytes_written;

    // Update progress in session
    {
        let mut sessions = state.sessions.write().await;
        if let Some(session) = sessions.get_mut(&query.session_token) {
            session
                .file_progress
                .insert(query.file_id.clone(), current_total);
        }
    }

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
        FileManager::get_part_path(state.download_dir(), &file.name, &payload.session_token);

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

    // Verify BLAKE3 checksum if provided
    if let Some(ref expected_hash) = file.blake3_hash {
        let matches = FileManager::verify_blake3_file(&part_path, expected_hash).map_err(|e| {
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

    let dest_path = FileManager::get_unique_dest_path(state.download_dir(), &file.name);
    FileManager::commit_part_file(&part_path, &dest_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to commit .part file: {}", e),
        )
    })?;

    Ok(Json(FinishResponse::completed(
        payload.file_id,
        dest_path.to_string_lossy(),
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
            FileManager::get_part_path(state.download_dir(), &file.name, &payload.session_token);
        if part_path.exists() {
            let _ = tokio::fs::remove_file(part_path).await;
        }
    }

    Ok(StatusCode::OK)
}

/// Builds the Axum application router with all routes and CORS.
pub fn create_router(state: Arc<ServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/v1/info", get(handle_info))
        .route("/api/v1/transfer/request", post(handle_transfer_request))
        .route("/api/v1/transfer/status", get(handle_transfer_status))
        .route("/api/v1/transfer/chunk", post(handle_transfer_chunk))
        .route("/api/v1/transfer/finish", post(handle_transfer_finish))
        .route("/api/v1/transfer/cancel", post(handle_transfer_cancel))
        .layer(cors)
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
