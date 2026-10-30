//! On-disk storage for PIN pairings, used only when "Remember paired devices" is on.
//!
//! Two files, each written by exactly one side so they never race:
//! - `pairings-accepted.json`: pairings other devices made with this one (server side)
//! - `pairings-trusted.json`: devices this one paired with (client side), plus devices that
//!   forgot their pairing and must be paired again before anything is sent to them
//!
//! Keys are stored hex-encoded in the user's config directory, which only the user can read.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const ACCEPTED_PAIRINGS_FILE: &str = "pairings-accepted.json";
pub const TRUSTED_PAIRINGS_FILE: &str = "pairings-trusted.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredAcceptedPairing {
    pub pair_id: String,
    pub key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredTrustedPairing {
    pub device_id: String,
    pub device_name: String,
    pub pair_id: String,
    pub key_hex: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedPairingsFile {
    #[serde(default)]
    pub pairings: Vec<StoredTrustedPairing>,
    /// Device ids whose pairing was rejected by the device; sending to them is refused
    #[serde(default)]
    pub needs_repair: Vec<String>,
}

fn load_json<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_json<T: Serialize>(config_dir: &Path, file_name: &str, value: &T) -> Result<(), String> {
    fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config directory {:?}: {}", config_dir, e))?;
    let path = config_dir.join(file_name);
    let tmp = config_dir.join(format!("{}.tmp", file_name));
    let serialized =
        serde_json::to_string_pretty(value).map_err(|e| format!("Failed to serialize pairings: {}", e))?;
    fs::write(&tmp, serialized).map_err(|e| format!("Failed to write {:?}: {}", tmp, e))?;
    fs::rename(&tmp, &path).map_err(|e| format!("Failed to replace {:?}: {}", path, e))
}

fn remove(config_dir: &Path, file_name: &str) {
    let _ = fs::remove_file(config_dir.join(file_name));
}

pub fn load_accepted(config_dir: &Path) -> Vec<StoredAcceptedPairing> {
    load_json(&config_dir.join(ACCEPTED_PAIRINGS_FILE))
}

pub fn save_accepted(config_dir: &Path, pairings: &[StoredAcceptedPairing]) -> Result<(), String> {
    save_json(config_dir, ACCEPTED_PAIRINGS_FILE, &pairings)
}

pub fn remove_accepted(config_dir: &Path) {
    remove(config_dir, ACCEPTED_PAIRINGS_FILE);
}

pub fn load_trusted(config_dir: &Path) -> TrustedPairingsFile {
    load_json(&config_dir.join(TRUSTED_PAIRINGS_FILE))
}

pub fn save_trusted(config_dir: &Path, file: &TrustedPairingsFile) -> Result<(), String> {
    save_json(config_dir, TRUSTED_PAIRINGS_FILE, file)
}

pub fn remove_trusted(config_dir: &Path) {
    remove(config_dir, TRUSTED_PAIRINGS_FILE);
}
