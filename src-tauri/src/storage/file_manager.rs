use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    FileNotFound(PathBuf),
    InvalidPath(String),
    HashMismatch { expected: String, actual: String },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Io(err) => write!(f, "I/O error: {}", err),
            StorageError::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
            StorageError::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            StorageError::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "BLAKE3 hash mismatch: expected {}, got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for StorageError {
    fn from(err: io::Error) -> Self {
        StorageError::Io(err)
    }
}

const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub struct FileManager;

impl FileManager {
    /// Sanitizes an incoming file name against directory traversal, invalid characters,
    /// and Windows reserved device names.
    pub fn sanitize_filename(name: &str) -> String {
        // Step 1: Normalize directory separators and extract the final path component
        let normalized = name.replace('\\', "/");
        let raw_filename = normalized
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
            .last()
            .unwrap_or("unnamed_file");

        // Step 2: Replace invalid filesystem characters and ASCII control characters
        let mut sanitized: String = raw_filename
            .chars()
            .map(|c| match c {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                c if (c as u32) <= 0x1f => '_',
                c => c,
            })
            .collect();

        // Step 3: Trim trailing dots and spaces (Windows restriction)
        while sanitized.ends_with('.') || sanitized.ends_with(' ') {
            sanitized.pop();
        }

        // If trimming or sanitization resulted in empty or dot-only string
        if sanitized.is_empty() || sanitized.chars().all(|c| c == '.' || c == '_' || c == ' ') {
            sanitized = "unnamed_file".to_string();
        }

        // Step 4: Check against Windows reserved device names (case-insensitive)
        let stem = match sanitized.find('.') {
            Some(idx) => &sanitized[..idx],
            None => &sanitized,
        };
        let stem_upper = stem.to_ascii_uppercase();

        if WINDOWS_RESERVED_NAMES.contains(&stem_upper.as_str()) {
            sanitized = format!("_{}", sanitized);
        }

        sanitized
    }

    /// Returns the user's Downloads directory, or a sensible fallback.
    pub fn get_downloads_dir() -> PathBuf {
        dirs::download_dir().unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join("Downloads"))
                .unwrap_or_else(|| PathBuf::from("."))
        })
    }

    /// Derives the temporary `.part` file path for an active transfer session.
    pub fn get_part_path(downloads_dir: &Path, file_name: &str, session_token: &str) -> PathBuf {
        let sanitized = Self::sanitize_filename(file_name);
        downloads_dir.join(format!("{}.{}.part", sanitized, session_token))
    }

    /// Generates a unique destination path in `downloads_dir`, incrementing a suffix
    /// (`file(1).ext`, `file(2).ext`) if collisions exist.
    pub fn get_unique_dest_path(downloads_dir: &Path, file_name: &str) -> PathBuf {
        let sanitized = Self::sanitize_filename(file_name);
        let initial_path = downloads_dir.join(&sanitized);
        if !initial_path.exists() {
            return initial_path;
        }

        let path_obj = Path::new(&sanitized);
        let stem = path_obj
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = path_obj.extension().and_then(|e| e.to_str());

        let mut counter = 1;
        loop {
            let candidate_name = match ext {
                Some(extension) if !extension.is_empty() => {
                    format!("{}({}).{}", stem, counter, extension)
                }
                _ => format!("{}({})", stem, counter),
            };
            let candidate_path = downloads_dir.join(&candidate_name);
            if !candidate_path.exists() {
                return candidate_path;
            }
            counter += 1;
        }
    }

    /// Atomically moves/renames the temporary `.part` file to the final destination path.
    pub fn commit_part_file(part_path: &Path, dest_path: &Path) -> Result<(), StorageError> {
        if !part_path.exists() {
            return Err(StorageError::FileNotFound(part_path.to_path_buf()));
        }
        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::rename(part_path, dest_path)?;
        Ok(())
    }

    /// Calculates the BLAKE3 hash of an in-memory byte slice as a hex string.
    pub fn calculate_blake3_bytes(data: &[u8]) -> String {
        blake3::hash(data).to_hex().to_string()
    }

    /// Calculates the BLAKE3 hash of a file on disk using streaming chunks.
    pub fn calculate_blake3_file(path: &Path) -> Result<String, StorageError> {
        if !path.exists() {
            return Err(StorageError::FileNotFound(path.to_path_buf()));
        }
        let mut file = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }

        Ok(hasher.finalize().to_hex().to_string())
    }

    /// Verifies that a file's BLAKE3 hash matches the expected hex hash string.
    pub fn verify_blake3_file(path: &Path, expected_hash: &str) -> Result<bool, StorageError> {
        let actual_hash = Self::calculate_blake3_file(path)?;
        Ok(actual_hash.eq_ignore_ascii_case(expected_hash))
    }

    /// Returns the size in bytes of a file on disk.
    pub fn get_file_size(path: &Path) -> Result<u64, StorageError> {
        let metadata = fs::metadata(path)?;
        Ok(metadata.len())
    }
}
