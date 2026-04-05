use std::path::PathBuf;

use crate::error::{err, Result};

pub(crate) fn dirs_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| err("HOME is not set"))
}

/// [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/latest/) config home:
/// `XDG_CONFIG_HOME`, or `$HOME/.config` when unset or empty.
pub(crate) fn dirs_xdg_config_home() -> Result<PathBuf> {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Ok(dirs_home()?.join(".config")),
    }
}

/// Per-user Kasetto configuration directory: `$XDG_CONFIG_HOME/kasetto`.
pub(crate) fn dirs_kasetto_config() -> Result<PathBuf> {
    Ok(dirs_xdg_config_home()?.join("kasetto"))
}

/// [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/latest/) data home:
/// `XDG_DATA_HOME`, or `$HOME/.local/share` when unset or empty.
pub(crate) fn dirs_xdg_data_home() -> Result<PathBuf> {
    match std::env::var("XDG_DATA_HOME") {
        Ok(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Ok(dirs_home()?.join(".local/share")),
    }
}

/// Per-user Kasetto data directory (lock file, etc.): `$XDG_DATA_HOME/kasetto`.
pub(crate) fn dirs_kasetto_data() -> Result<PathBuf> {
    Ok(dirs_xdg_data_home()?.join("kasetto"))
}
