use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};

/// Wrapper for loading, mutating, and saving agent settings JSON files.
pub(crate) struct SettingsFile {
    path: PathBuf,
    pub data: serde_json::Value,
}

impl SettingsFile {
    /// Load an existing JSON file or start with an empty `{}`.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let data = if path.exists() {
            let text = fs::read_to_string(path)?;
            serde_json::from_str(&text)
                .map_err(|e| err(format!("invalid settings JSON {}: {e}", path.display())))?
        } else {
            serde_json::json!({})
        };
        Ok(Self {
            path: path.to_path_buf(),
            data,
        })
    }

    /// Write pretty-printed JSON back to disk, creating parent dirs if needed.
    pub(crate) fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(&self.data)?)?;
        Ok(())
    }
}
