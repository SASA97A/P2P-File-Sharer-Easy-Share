use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// Persisted application preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub download_dir: Option<String>,
}

/// Loads application settings from the specified directory.
/// Returns default settings if the file does not exist or is corrupted.
pub fn load_settings(config_dir: &Path) -> AppSettings {
    let settings_path = config_dir.join(SETTINGS_FILE_NAME);
    if !settings_path.is_file() {
        return AppSettings::default();
    }

    match fs::read_to_string(&settings_path) {
        Ok(content) => serde_json::from_str::<AppSettings>(&content).unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

/// Saves application settings to the specified directory.
pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    if !config_dir.exists() {
        fs::create_dir_all(config_dir)
            .map_err(|e| format!("Failed to create config directory {:?}: {}", config_dir, e))?;
    }

    let settings_path = config_dir.join(SETTINGS_FILE_NAME);
    let serialized = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&settings_path, serialized)
        .map_err(|e| format!("Failed to write settings file {:?}: {}", settings_path, e))?;

    Ok(())
}
