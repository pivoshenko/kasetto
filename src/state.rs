use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_cache, hash_str};
use crate::lock::lock_path;
use crate::model::{Scope, SyncFailure};

/// Machine-local, run-specific state kept *out* of the committed `kasetto.lock`
/// so the lock stays portable across machines and users. This mirrors how `uv`
/// keeps machine state in its cache directory, separate from `uv.lock`.
///
/// Everything here is regenerated on the next `sync`; the file is safe to delete.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct RuntimeState {
    /// Unix timestamp of the last successful sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    /// Serialized JSON of the most recent sync `Report` (used by `doctor`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_report: Option<String>,
    /// `entry id -> unix timestamp` of when this machine last installed/updated
    /// each skill. Drives the "updated N ago" display in `list`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub installed_at: BTreeMap<String, String>,
}

impl RuntimeState {
    pub(crate) fn updated_at(&self, id: &str) -> String {
        self.installed_at.get(id).cloned().unwrap_or_default()
    }

    pub(crate) fn set_updated_at(&mut self, id: &str, ts: String) {
        self.installed_at.insert(id.to_string(), ts);
    }

    pub(crate) fn forget(&mut self, id: &str) {
        self.installed_at.remove(id);
    }

    pub(crate) fn save_report_json(&mut self, report_json: &str) {
        self.latest_report = Some(report_json.to_string());
    }

    /// Extract failed actions from the cached report for `doctor`.
    pub(crate) fn load_latest_failures(&self) -> Vec<SyncFailure> {
        let Some(report_json) = &self.latest_report else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(report_json) else {
            return Vec::new();
        };
        let mut failed = Vec::new();
        if let Some(actions) = value.get("actions").and_then(|v| v.as_array()) {
            for action in actions {
                let status = action.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if status != "broken" && status != "source_error" {
                    continue;
                }
                failed.push(SyncFailure {
                    name: action
                        .get("skill")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-")
                        .to_string(),
                    source: action
                        .get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-")
                        .to_string(),
                    reason: action
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown reason")
                        .to_string(),
                });
            }
        }
        failed
    }
}

/// Machine-local state file path, keyed by the lock file location so project
/// and global scopes (and distinct projects) never collide. Lives in the cache
/// directory and is never committed.
pub(crate) fn runtime_state_path(scope: Scope, project_root: &Path) -> Result<PathBuf> {
    let lock = lock_path(scope, project_root)?;
    let key = hash_str(&lock.to_string_lossy());
    Ok(dirs_kasetto_cache()?
        .join("runtime")
        .join(format!("{key}.json")))
}

pub(crate) fn load_runtime_state(scope: Scope, project_root: &Path) -> Result<RuntimeState> {
    let path = runtime_state_path(scope, project_root)?;
    if !path.exists() {
        return Ok(RuntimeState::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| err(format!("failed to read state file {}: {e}", path.display())))?;
    if text.trim().is_empty() {
        return Ok(RuntimeState::default());
    }
    serde_json::from_str(&text).map_err(|e| {
        err(format!(
            "failed to parse state file {}: {e}",
            path.display()
        ))
    })
}

pub(crate) fn save_runtime_state(
    state: &RuntimeState,
    scope: Scope,
    project_root: &Path,
) -> Result<()> {
    let path = runtime_state_path(scope, project_root)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| err(format!("failed to serialize state file: {e}")))?;
    fs::write(&path, json)?;
    Ok(())
}

pub(crate) fn clear_runtime_state(scope: Scope, project_root: &Path) -> Result<()> {
    let path = runtime_state_path(scope, project_root)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| {
            err(format!(
                "failed to remove state file {}: {e}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn round_trip_runtime_state() {
        let cache = unique_root("kasetto-state-cache");
        std::env::set_var("XDG_CACHE_HOME", &cache);
        let root = unique_root("kasetto-state-proj");
        fs::create_dir_all(&root).unwrap();

        let mut state = RuntimeState {
            last_run: Some("123".into()),
            ..Default::default()
        };
        state.set_updated_at("src::a", "100".into());
        state.save_report_json(r#"{"actions":[]}"#);

        save_runtime_state(&state, Scope::Project, &root).unwrap();
        let loaded = load_runtime_state(Scope::Project, &root).unwrap();

        assert_eq!(loaded.last_run.as_deref(), Some("123"));
        assert_eq!(loaded.updated_at("src::a"), "100");
        assert_eq!(loaded.updated_at("missing"), "");

        clear_runtime_state(Scope::Project, &root).unwrap();
        assert!(load_runtime_state(Scope::Project, &root)
            .unwrap()
            .last_run
            .is_none());

        let _ = fs::remove_dir_all(&cache);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_latest_failures_extracts_failed_actions() {
        let mut state = RuntimeState::default();
        state.save_report_json(
            r#"{"actions":[
                {"status":"installed","skill":"good","source":"s"},
                {"status":"broken","skill":"bad","source":"s","error":"missing"},
                {"status":"source_error","skill":"err","source":"s2","error":"timeout"}
            ]}"#,
        );
        let failures = state.load_latest_failures();
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].name, "bad");
        assert_eq!(failures[1].reason, "timeout");
    }
}
