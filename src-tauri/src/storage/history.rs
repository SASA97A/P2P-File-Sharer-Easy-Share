use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub const HISTORY_FILE_NAME: &str = "history.json";
/// Where an unreadable history file is copied before it can be overwritten.
pub const HISTORY_CORRUPT_BACKUP_NAME: &str = "history.json.corrupt";

/// Serializes read-modify-write cycles so concurrent completions cannot drop each other's records.
static HISTORY_LOCK: Mutex<()> = Mutex::new(());

fn lock_history() -> MutexGuard<'static, ()> {
    HISTORY_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransferDirection {
    Sent,
    Received,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryStatus {
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryFileItem {
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferHistoryRecord {
    pub id: String,
    #[serde(default)]
    pub session_token: Option<String>,
    pub timestamp: i64,
    pub direction: TransferDirection,
    pub peer_name: String,
    pub peer_ip: String,
    pub status: HistoryStatus,
    #[serde(default)]
    pub files: Vec<HistoryFileItem>,
    pub total_bytes: u64,
    #[serde(default)]
    pub error_message: Option<String>,
}

/// Loads transfer history records from the given config directory.
/// Returns empty vector if file does not exist or has invalid JSON. Invalid JSON is first
/// copied to `history.json.corrupt` so the next save cannot destroy the only copy.
pub fn load_history(config_dir: &Path) -> Vec<TransferHistoryRecord> {
    // Readers take the lock too: on Windows, renaming over a file another thread has open fails.
    let _guard = lock_history();
    load_history_unlocked(config_dir)
}

fn load_history_unlocked(config_dir: &Path) -> Vec<TransferHistoryRecord> {
    let history_path = config_dir.join(HISTORY_FILE_NAME);
    if !history_path.is_file() {
        return Vec::new();
    }

    match fs::read_to_string(&history_path) {
        Ok(content) => match serde_json::from_str::<Vec<TransferHistoryRecord>>(&content) {
            Ok(records) => records,
            Err(_) => {
                let _ = fs::copy(&history_path, config_dir.join(HISTORY_CORRUPT_BACKUP_NAME));
                Vec::new()
            }
        },
        Err(_) => Vec::new(),
    }
}

/// Saves transfer history records to the specified config directory.
pub fn save_history(config_dir: &Path, records: &[TransferHistoryRecord]) -> Result<(), String> {
    let _guard = lock_history();
    save_history_unlocked(config_dir, records)
}

fn save_history_unlocked(config_dir: &Path, records: &[TransferHistoryRecord]) -> Result<(), String> {
    if !config_dir.exists() {
        fs::create_dir_all(config_dir)
            .map_err(|e| format!("Failed to create config directory {:?}: {}", config_dir, e))?;
    }

    let history_path = config_dir.join(HISTORY_FILE_NAME);
    let serialized = serde_json::to_string_pretty(records)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    // Write to a temp file and rename over the original so a crash mid-write never leaves a
    // truncated history.json behind.
    let tmp_path = config_dir.join(format!("{}.tmp", HISTORY_FILE_NAME));
    fs::write(&tmp_path, serialized)
        .map_err(|e| format!("Failed to write history file {:?}: {}", tmp_path, e))?;
    fs::rename(&tmp_path, &history_path)
        .map_err(|e| format!("Failed to replace history file {:?}: {}", history_path, e))?;

    Ok(())
}

/// Adds a new record to the beginning of the history log and persists it.
pub fn add_record(config_dir: &Path, record: TransferHistoryRecord) -> Result<TransferHistoryRecord, String> {
    let _guard = lock_history();
    let mut history = load_history_unlocked(config_dir);
    history.insert(0, record.clone());
    save_history_unlocked(config_dir, &history)?;
    Ok(record)
}

/// Deletes a history record by ID and persists the updated log.
pub fn delete_record(config_dir: &Path, record_id: &str) -> Result<bool, String> {
    let _guard = lock_history();
    let mut history = load_history_unlocked(config_dir);
    let original_len = history.len();
    history.retain(|r| r.id != record_id);

    if history.len() != original_len {
        save_history_unlocked(config_dir, &history)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Clears all history records and persists an empty list.
pub fn clear_history(config_dir: &Path) -> Result<(), String> {
    let _guard = lock_history();
    save_history_unlocked(config_dir, &[])
}
