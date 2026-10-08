use crate::models::peer::{DeviceInfo, PeerInfo};
use crate::models::transfer::{
    CancelRequest, ChunkResponse, FileMetadata, FinishRequest, FinishResponse, TransferRequest,
    TransferResponse, TransferStatusQueryResponse,
};
use crate::storage::file_manager::FileManager;
use std::fmt;
use std::io::SeekFrom;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Default chunk buffer size: 1 MB (1,048,576 bytes).
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// Errors that may occur during client transfer operations.
#[derive(Debug)]
pub enum ClientError {
    Http(reqwest::Error),
    Io(std::io::Error),
    ServerDeclined { reason: String },
    StatusError { status: reqwest::StatusCode, message: String },
    Serialization(serde_json::Error),
    FileNotFound(PathBuf),
    InvalidOffset { expected: u64, actual: u64 },
    Blake3Mismatch { expected: String, actual: String },
    Other(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Http(e) => write!(f, "HTTP error: {}", e),
            ClientError::Io(e) => write!(f, "I/O error: {}", e),
            ClientError::ServerDeclined { reason } => write!(f, "Transfer declined by peer: {}", reason),
            ClientError::StatusError { status, message } => {
                write!(f, "Server returned status {}: {}", status, message)
            }
            ClientError::Serialization(e) => write!(f, "Serialization error: {}", e),
            ClientError::FileNotFound(p) => write!(f, "File not found: {}", p.display()),
            ClientError::InvalidOffset { expected, actual } => {
                write!(f, "Invalid transfer offset: expected <= {}, got {}", expected, actual)
            }
            ClientError::Blake3Mismatch { expected, actual } => {
                write!(f, "BLAKE3 checksum mismatch: expected {}, got {}", expected, actual)
            }
            ClientError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ClientError::Http(e) => Some(e),
            ClientError::Io(e) => Some(e),
            ClientError::Serialization(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(e: reqwest::Error) -> Self {
        ClientError::Http(e)
    }
}

impl From<std::io::Error> for ClientError {
    fn from(e: std::io::Error) -> Self {
        ClientError::Io(e)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(e: serde_json::Error) -> Self {
        ClientError::Serialization(e)
    }
}

/// Description of a local file queued for transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileToSend {
    pub file_id: String,
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub blake3_hash: Option<String>,
}

impl FileToSend {
    pub fn new(
        file_id: impl Into<String>,
        path: impl Into<PathBuf>,
        name: impl Into<String>,
        size: u64,
        blake3_hash: Option<String>,
    ) -> Self {
        Self {
            file_id: file_id.into(),
            path: path.into(),
            name: name.into(),
            size,
            blake3_hash,
        }
    }

    /// Creates a `FileToSend` instance by inspecting the file at `path` on disk
    /// and pre-calculating its BLAKE3 hash.
    pub fn from_path(
        file_id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Result<Self, ClientError> {
        let path = path.into();
        if !path.exists() {
            return Err(ClientError::FileNotFound(path));
        }

        let metadata = std::fs::metadata(&path).map_err(ClientError::Io)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
        let size = metadata.len();

        let hash = FileManager::calculate_blake3_file(&path).map_err(|e| match e {
            crate::storage::file_manager::StorageError::Io(io_err) => ClientError::Io(io_err),
            crate::storage::file_manager::StorageError::FileNotFound(p) => ClientError::FileNotFound(p),
            _ => ClientError::Other(format!("{:?}", e)),
        })?;

        Ok(Self {
            file_id: file_id.into(),
            path,
            name,
            size,
            blake3_hash: Some(hash),
        })
    }

    /// Converts this file description to wire metadata (`FileMetadata`).
    pub fn to_metadata(&self) -> FileMetadata {
        FileMetadata::new(
            self.file_id.clone(),
            self.name.clone(),
            self.size,
            self.blake3_hash.clone(),
        )
    }
}

/// Asynchronous client for communicating with remote Easy Share peer servers.
#[derive(Debug, Clone)]
pub struct TransferClient {
    client: reqwest::Client,
    chunk_size: usize,
}

impl Default for TransferClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TransferClient {
    /// Creates a new `TransferClient` with default settings (1 MB chunk size).
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    /// Creates a new `TransferClient` with a customized chunk size.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            client: reqwest::Client::new(),
            chunk_size,
        }
    }

    /// Creates a `TransferClient` with a custom `reqwest::Client`.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }

    /// Creates a `TransferClient` with custom client and chunk size.
    pub fn with_client_and_chunk_size(client: reqwest::Client, chunk_size: usize) -> Self {
        Self {
            client,
            chunk_size,
        }
    }

    /// Returns the configured chunk size in bytes.
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Returns a reference to the underlying reqwest client.
    pub fn inner_client(&self) -> &reqwest::Client {
        &self.client
    }

    // =========================================================================
    // API Methods (PeerInfo overloads)
    // =========================================================================

    /// Queries the peer's device info endpoint (`/api/v1/info`).
    pub async fn probe_peer(&self, peer: &PeerInfo) -> Result<DeviceInfo, ClientError> {
        self.probe_peer_url(&peer.base_url()).await
    }

    /// Sends a transfer handshake request (`/api/v1/transfer/request`).
    pub async fn request_transfer(
        &self,
        peer: &PeerInfo,
        request: &TransferRequest,
    ) -> Result<TransferResponse, ClientError> {
        self.request_transfer_url(&peer.base_url(), request).await
    }

    /// Queries the current transfer progress / byte offset (`/api/v1/transfer/status`).
    pub async fn get_transfer_status(
        &self,
        peer: &PeerInfo,
        session_token: &str,
        file_id: &str,
    ) -> Result<TransferStatusQueryResponse, ClientError> {
        self.get_transfer_status_url(&peer.base_url(), session_token, file_id)
            .await
    }

    /// Sends a raw byte chunk to the receiver (`/api/v1/transfer/chunk`).
    pub async fn send_chunk(
        &self,
        peer: &PeerInfo,
        session_token: &str,
        file_id: &str,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<ChunkResponse, ClientError> {
        self.send_chunk_url(&peer.base_url(), session_token, file_id, offset, data)
            .await
    }

    /// Finalizes a file transfer (`/api/v1/transfer/finish`).
    pub async fn finish_transfer(
        &self,
        peer: &PeerInfo,
        session_token: &str,
        file_id: &str,
    ) -> Result<FinishResponse, ClientError> {
        self.finish_transfer_url(&peer.base_url(), session_token, file_id)
            .await
    }

    /// Cancels an ongoing transfer session (`/api/v1/transfer/cancel`).
    pub async fn cancel_transfer(
        &self,
        peer: &PeerInfo,
        session_token: &str,
        reason: Option<String>,
    ) -> Result<(), ClientError> {
        self.cancel_transfer_url(&peer.base_url(), session_token, reason)
            .await
    }

    /// Transmits a file to a remote peer, resuming from the server's current byte offset
    /// and reporting progress to the provided callback after each chunk.
    pub async fn send_file_resumable<F>(
        &self,
        peer: &PeerInfo,
        session_token: &str,
        file: &FileToSend,
        progress_cb: F,
    ) -> Result<(), ClientError>
    where
        F: FnMut(u64, u64) + Send,
    {
        self.send_file_resumable_url(&peer.base_url(), session_token, file, progress_cb)
            .await
    }

    // =========================================================================
    // Base URL Implementations
    // =========================================================================

    /// Probes peer device info using explicit base URL.
    pub async fn probe_peer_url(&self, base_url: &str) -> Result<DeviceInfo, ClientError> {
        let url = format!("{}/api/v1/info", base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(ClientError::StatusError { status, message });
        }

        let info = resp.json::<DeviceInfo>().await?;
        Ok(info)
    }

    /// Sends a transfer handshake request using explicit base URL.
    pub async fn request_transfer_url(
        &self,
        base_url: &str,
        request: &TransferRequest,
    ) -> Result<TransferResponse, ClientError> {
        let url = format!("{}/api/v1/transfer/request", base_url.trim_end_matches('/'));
        let resp = self.client.post(&url).json(request).send().await?;

        if resp.status().is_success() {
            let body = resp.json::<TransferResponse>().await?;
            Ok(body)
        } else if resp.status() == reqwest::StatusCode::FORBIDDEN {
            // Declined by receiver - payload contains TransferResponse
            let body = resp.json::<TransferResponse>().await?;
            Ok(body)
        } else {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            Err(ClientError::StatusError { status, message })
        }
    }

    /// Queries transfer status using explicit base URL.
    pub async fn get_transfer_status_url(
        &self,
        base_url: &str,
        session_token: &str,
        file_id: &str,
    ) -> Result<TransferStatusQueryResponse, ClientError> {
        let url = format!("{}/api/v1/transfer/status", base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .query(&[("session_token", session_token), ("file_id", file_id)])
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(ClientError::StatusError { status, message });
        }

        let status_resp = resp.json::<TransferStatusQueryResponse>().await?;
        Ok(status_resp)
    }

    /// Sends an individual chunk using explicit base URL.
    pub async fn send_chunk_url(
        &self,
        base_url: &str,
        session_token: &str,
        file_id: &str,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<ChunkResponse, ClientError> {
        let url = format!("{}/api/v1/transfer/chunk", base_url.trim_end_matches('/'));
        let offset_str = offset.to_string();
        let resp = self
            .client
            .post(&url)
            .query(&[
                ("session_token", session_token),
                ("file_id", file_id),
                ("offset", &offset_str),
            ])
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(data)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(ClientError::StatusError { status, message });
        }

        let chunk_resp = resp.json::<ChunkResponse>().await?;
        Ok(chunk_resp)
    }

    /// Finalizes file transfer using explicit base URL.
    pub async fn finish_transfer_url(
        &self,
        base_url: &str,
        session_token: &str,
        file_id: &str,
    ) -> Result<FinishResponse, ClientError> {
        let url = format!("{}/api/v1/transfer/finish", base_url.trim_end_matches('/'));
        let payload = FinishRequest::new(session_token, file_id);
        let resp = self.client.post(&url).json(&payload).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(ClientError::StatusError { status, message });
        }

        let finish_resp = resp.json::<FinishResponse>().await?;
        Ok(finish_resp)
    }

    /// Cancels a transfer session using explicit base URL.
    pub async fn cancel_transfer_url(
        &self,
        base_url: &str,
        session_token: &str,
        reason: Option<String>,
    ) -> Result<(), ClientError> {
        let url = format!("{}/api/v1/transfer/cancel", base_url.trim_end_matches('/'));
        let payload = CancelRequest::new(session_token, reason);
        let resp = self.client.post(&url).json(&payload).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let message = resp.text().await.unwrap_or_default();
            return Err(ClientError::StatusError { status, message });
        }

        Ok(())
    }

    /// Sends chunks of a file starting from `start_offset` up to an optional byte limit.
    /// Returns the total bytes transmitted in this invocation.
    pub async fn send_file_chunks_with_limit<F>(
        &self,
        base_url: &str,
        session_token: &str,
        file: &FileToSend,
        start_offset: u64,
        max_bytes_to_send: Option<u64>,
        mut progress_cb: F,
    ) -> Result<u64, ClientError>
    where
        F: FnMut(u64, u64) + Send,
    {
        if !file.path.exists() {
            return Err(ClientError::FileNotFound(file.path.clone()));
        }

        if start_offset > file.size {
            return Err(ClientError::InvalidOffset {
                expected: file.size,
                actual: start_offset,
            });
        }

        // Initial progress callback with starting offset
        progress_cb(start_offset, file.size);

        if start_offset == file.size {
            return Ok(0);
        }

        let mut file_handle = tokio::fs::File::open(&file.path).await?;
        if start_offset > 0 {
            file_handle.seek(SeekFrom::Start(start_offset)).await?;
        }

        let mut current_offset = start_offset;
        let mut total_sent_in_call = 0u64;

        while current_offset < file.size {
            if let Some(limit) = max_bytes_to_send {
                if total_sent_in_call >= limit {
                    break;
                }
            }

            let remaining_file = file.size - current_offset;
            let mut chunk_len = (self.chunk_size as u64).min(remaining_file);

            if let Some(limit) = max_bytes_to_send {
                let remaining_limit = limit - total_sent_in_call;
                chunk_len = chunk_len.min(remaining_limit);
            }

            if chunk_len == 0 {
                break;
            }

            let mut buffer = vec![0u8; chunk_len as usize];
            file_handle.read_exact(&mut buffer).await?;

            let chunk_resp = self
                .send_chunk_url(base_url, session_token, &file.file_id, current_offset, buffer)
                .await?;

            let written = chunk_resp.bytes_written;
            current_offset += written;
            total_sent_in_call += written;

            progress_cb(current_offset, file.size);
        }

        Ok(total_sent_in_call)
    }

    /// Automatically discovers current remote offset and streams remaining chunks to peer.
    pub async fn send_file_resumable_url<F>(
        &self,
        base_url: &str,
        session_token: &str,
        file: &FileToSend,
        progress_cb: F,
    ) -> Result<(), ClientError>
    where
        F: FnMut(u64, u64) + Send,
    {
        if !file.path.exists() {
            return Err(ClientError::FileNotFound(file.path.clone()));
        }

        let status = self
            .get_transfer_status_url(base_url, session_token, &file.file_id)
            .await?;

        let start_offset = status.bytes_received;

        self.send_file_chunks_with_limit(
            base_url,
            session_token,
            file,
            start_offset,
            None,
            progress_cb,
        )
        .await?;

        Ok(())
    }
}
