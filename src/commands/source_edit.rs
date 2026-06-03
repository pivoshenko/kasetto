//! Shared plumbing for the config-editing commands (`add` / `remove`).

use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::model::Scope;

/// Resolve a local, writable config path. Remote configs (HTTP/S) cannot be
/// edited in place, so they are rejected with a hint to pass a local `--config`.
pub(super) fn resolve_local_config_path(config_override: Option<&str>) -> Result<PathBuf> {
    let raw = match config_override {
        Some(c) => c.to_string(),
        None => crate::default_config_path(),
    };
    if raw.contains("://") {
        return Err(err(format!(
            "cannot edit remote config `{raw}`; pass a local file with --config <path>"
        )));
    }
    Ok(PathBuf::from(raw))
}

/// Run a plain sync against the freshly edited config so installs and the lock
/// catch up with the change. `add` installs the new source; `remove` prunes the
/// orphaned assets via sync's existing stale-cleanup pass.
pub(super) fn sync_after(
    path: &Path,
    scope: Option<Scope>,
    quiet: bool,
    plain: bool,
) -> Result<()> {
    let config_path = path.to_string_lossy();
    crate::commands::sync::run(&crate::commands::sync::SyncOptions {
        config_path: &config_path,
        dry_run: false,
        quiet,
        as_json: false,
        plain,
        verbose: 0,
        scope_override: scope,
        update: false,
        update_only: Vec::new(),
        locked: false,
    })
}
