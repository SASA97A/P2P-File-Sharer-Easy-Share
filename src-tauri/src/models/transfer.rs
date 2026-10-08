use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status states for transfer lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Accepted,
    Declined,
    Pending,
    Partial,
    InProgress,
    Completed,
    Cancelled,
    Failed,
}

/// Metadata for an individual file in a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file_id: String,
    pub name: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blake3_hash: Option<String>,
}

impl FileMetadata {
    pub fn new(
        file_id: impl Into<String>,
        name: impl Into<String>,
        size: u64,
        blake3_hash: Option<String>,
    ) -> Self {
        Self {
            file_id: file_id.into(),
            name: name.into(),
            size,
            blake3_hash,
        }
    }
}

/// Handshake request payload sent by the sender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferRequest {
    pub request_id: String,
    pub sender_name: String,
    pub sender_os: String,
    pub total_bytes: u64,
    pub files: Vec<FileMetadata>,
}

impl TransferRequest {
    pub fn new(
        request_id: impl Into<String>,
        sender_name: impl Into<String>,
        sender_os: impl Into<String>,
        files: Vec<FileMetadata>,
    ) -> Self {
        let total_bytes = files.iter().map(|f| f.size).sum();
        Self {
            request_id: request_id.into(),
            sender_name: sender_name.into(),
            sender_os: sender_os.into(),
            total_bytes,
            files,
        }
    }
}

/// Handshake response payload returned by the receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferResponse {
    pub status: TransferStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub existing_offsets: HashMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl TransferResponse {
    pub fn accepted(session_token: impl Into<String>, existing_offsets: HashMap<String, u64>) -> Self {
        Self {
            status: TransferStatus::Accepted,
            session_token: Some(session_token.into()),
            existing_offsets,
            reason: None,
        }
    }

    pub fn declined(reason: impl Into<String>) -> Self {
        Self {
            status: TransferStatus::Declined,
            session_token: None,
            existing_offsets: HashMap::new(),
            reason: Some(reason.into()),
        }
    }
}

/// Query parameters / header metadata for chunk upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkHeader {
    pub session_token: String,
    pub file_id: String,
    pub offset: u64,
}

impl ChunkHeader {
    pub fn new(session_token: impl Into<String>, file_id: impl Into<String>, offset: u64) -> Self {
        Self {
            session_token: session_token.into(),
            file_id: file_id.into(),
            offset,
        }
    }
}

/// Response returned after successfully writing a chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkResponse {
    pub file_id: String,
    pub bytes_written: u64,
    pub current_total: u64,
}

/// Response returned when querying the current status/offset of a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferStatusQueryResponse {
    pub file_id: String,
    pub bytes_received: u64,
    pub status: TransferStatus,
}

/// Request sent to finalize a file transfer and trigger checksum validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishRequest {
    pub session_token: String,
    pub file_id: String,
}

impl FinishRequest {
    pub fn new(session_token: impl Into<String>, file_id: impl Into<String>) -> Self {
        Self {
            session_token: session_token.into(),
            file_id: file_id.into(),
        }
    }
}

/// Response returned after finalization and file commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinishResponse {
    pub status: TransferStatus,
    pub file_id: String,
    pub saved_path: String,
}

impl FinishResponse {
    pub fn completed(file_id: impl Into<String>, saved_path: impl Into<String>) -> Self {
        Self {
            status: TransferStatus::Completed,
            file_id: file_id.into(),
            saved_path: saved_path.into(),
        }
    }
}

/// Request sent to cancel an active transfer session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelRequest {
    pub session_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl CancelRequest {
    pub fn new(session_token: impl Into<String>, reason: Option<String>) -> Self {
        Self {
            session_token: session_token.into(),
            reason,
        }
    }
}
