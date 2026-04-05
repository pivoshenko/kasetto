use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_data, now_iso};
use crate::model::{Scope, SkillEntry, State, SyncFailure};

pub(crate) const LOCK_FILENAME: &str = "kasetto.lock";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub(crate) struct AssetEntry {
    pub kind: String,
    pub name: String,
    pub hash: String,
    pub source: String,
    pub destination: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LockFile {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillEntry>,
    #[serde(default)]
    pub assets: BTreeMap<String, AssetEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_report: Option<String>,
}

fn default_version() -> u8 {
    1
}

impl Default for LockFile {
    fn default() -> Self {
        Self {
            version: 1,
            last_run: None,
            skills: BTreeMap::new(),
            assets: BTreeMap::new(),
            latest_report: None,
        }
    }
}

impl LockFile {
    pub(crate) fn state(&self) -> State {
        State {
            version: self.version,
            last_run: self.last_run.clone(),
            skills: self.skills.clone(),
        }
    }

    pub(crate) fn apply_state(&mut self, state: &State) {
        self.last_run = state.last_run.clone();
        self.skills = state.skills.clone();
    }

    pub(crate) fn get_tracked_asset(&self, kind: &str, id: &str) -> Option<(String, String)> {
        self.assets.get(id).and_then(|a| {
            if a.kind == kind {
                Some((a.hash.clone(), a.destination.clone()))
            } else {
                None
            }
        })
    }

    pub(crate) fn save_tracked_asset(
        &mut self,
        kind: &str,
        id: &str,
        name: &str,
        hash: &str,
        source: &str,
        destination: &str,
    ) {
        self.assets.insert(
            id.to_string(),
            AssetEntry {
                kind: kind.to_string(),
                name: name.to_string(),
                hash: hash.to_string(),
                source: source.to_string(),
                destination: destination.to_string(),
                updated_at: now_iso(),
            },
        );
    }

    pub(crate) fn remove_tracked_asset(&mut self, id: &str) {
        self.assets.remove(id);
    }

    pub(crate) fn list_tracked_asset_ids(&self, kind: &str) -> Vec<(&str, &str)> {
        self.assets
            .iter()
            .filter(|(_, a)| a.kind == kind)
            .map(|(id, a)| (id.as_str(), a.destination.as_str()))
            .collect()
    }

    pub(crate) fn clear_all(&mut self) {
        self.skills.clear();
        self.assets.clear();
        self.latest_report = None;
        self.last_run = None;
    }

    pub(crate) fn save_report_json(&mut self, report_json: &str) {
        self.latest_report = Some(report_json.to_string());
    }

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

    pub(crate) fn list_installed_mcps(&self) -> Vec<String> {
        let mut servers: Vec<String> = self
            .list_tracked_asset_ids("mcp")
            .iter()
            .flat_map(|(_, dest_csv)| {
                dest_csv
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
            })
            .collect();
        servers.sort();
        servers.dedup();
        servers
    }
}

/// Resolve the lock file path for the given scope.
pub(crate) fn lock_path(scope: Scope, project_root: &Path) -> Result<PathBuf> {
    match scope {
        Scope::Project => Ok(project_root.join(LOCK_FILENAME)),
        Scope::Global => Ok(dirs_kasetto_data()?.join(LOCK_FILENAME)),
    }
}

/// Load the lock file from disk (or return a default empty one if missing).
pub(crate) fn load_lock(scope: Scope, project_root: &Path) -> Result<LockFile> {
    let path = lock_path(scope, project_root)?;
    if !path.exists() {
        return Ok(LockFile::default());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| err(format!("failed to read lock file {}: {e}", path.display())))?;
    if text.trim().is_empty() {
        return Ok(LockFile::default());
    }
    let lock: LockFile = serde_yaml::from_str(&text)
        .map_err(|e| err(format!("failed to parse lock file {}: {e}", path.display())))?;
    Ok(lock)
}

/// Write the lock file to disk, creating parent directories if needed.
pub(crate) fn save_lock(lock: &LockFile, scope: Scope, project_root: &Path) -> Result<PathBuf> {
    let path = lock_path(scope, project_root)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let yaml = serde_yaml::to_string(lock)
        .map_err(|e| err(format!("failed to serialize lock file: {e}")))?;
    fs::write(&path, yaml)?;
    Ok(path)
}

/// Delete the lock file if it exists.
#[allow(dead_code)]
pub(crate) fn remove_lock(scope: Scope, project_root: &Path) -> Result<()> {
    let path = lock_path(scope, project_root)?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| {
            err(format!(
                "failed to remove lock file {}: {e}",
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

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn round_trip_empty_lock_file() {
        let dir = temp_dir("kasetto-lock-empty");
        fs::create_dir_all(&dir).unwrap();

        let lock = LockFile::default();
        save_lock(&lock, Scope::Project, &dir).unwrap();

        let loaded = load_lock(Scope::Project, &dir).unwrap();
        assert_eq!(loaded.version, 1);
        assert!(loaded.skills.is_empty());
        assert!(loaded.assets.is_empty());
        assert!(loaded.last_run.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_with_skills_and_assets() {
        let dir = temp_dir("kasetto-lock-data");
        fs::create_dir_all(&dir).unwrap();

        let mut lock = LockFile::default();
        lock.last_run = Some("12345".to_string());
        lock.skills.insert(
            "src::skill-a".to_string(),
            SkillEntry {
                destination: "/tmp/skill-a".into(),
                hash: "abc".into(),
                skill: "skill-a".into(),
                description: "desc".into(),
                source: "src".into(),
                source_revision: "rev1".into(),
                updated_at: "100".into(),
            },
        );
        lock.save_tracked_asset(
            "mcp",
            "mcp::src::pack.json",
            "pack.json",
            "h1",
            "src",
            "srv1,srv2",
        );

        save_lock(&lock, Scope::Project, &dir).unwrap();
        let loaded = load_lock(Scope::Project, &dir).unwrap();

        assert_eq!(loaded.last_run.as_deref(), Some("12345"));
        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(loaded.skills["src::skill-a"].hash, "abc");
        assert_eq!(loaded.assets.len(), 1);

        let asset = loaded.get_tracked_asset("mcp", "mcp::src::pack.json");
        assert_eq!(asset, Some(("h1".into(), "srv1,srv2".into())));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_default_when_missing() {
        let dir = temp_dir("kasetto-lock-missing");
        fs::create_dir_all(&dir).unwrap();

        let lock = load_lock(Scope::Project, &dir).unwrap();
        assert_eq!(lock.version, 1);
        assert!(lock.skills.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_all_empties_everything() {
        let mut lock = LockFile::default();
        lock.last_run = Some("999".to_string());
        lock.skills.insert(
            "k".to_string(),
            SkillEntry {
                skill: "s".into(),
                hash: "h".into(),
                ..Default::default()
            },
        );
        lock.save_tracked_asset("mcp", "id", "n", "h", "s", "d");
        lock.save_report_json(r#"{"actions":[]}"#);

        lock.clear_all();

        assert!(lock.skills.is_empty());
        assert!(lock.assets.is_empty());
        assert!(lock.latest_report.is_none());
        assert!(lock.last_run.is_none());
    }

    #[test]
    fn list_tracked_asset_ids_filters_by_kind() {
        let mut lock = LockFile::default();
        lock.save_tracked_asset("mcp", "mcp::a", "a", "h", "s", "d1");
        lock.save_tracked_asset("other", "other::b", "b", "h", "s", "d2");

        let mcps = lock.list_tracked_asset_ids("mcp");
        assert_eq!(mcps.len(), 1);
        assert_eq!(mcps[0], ("mcp::a", "d1"));
    }

    #[test]
    fn remove_tracked_asset_deletes_entry() {
        let mut lock = LockFile::default();
        lock.save_tracked_asset("mcp", "mcp::a", "a", "h", "s", "d");
        assert!(lock.get_tracked_asset("mcp", "mcp::a").is_some());

        lock.remove_tracked_asset("mcp::a");
        assert!(lock.get_tracked_asset("mcp", "mcp::a").is_none());
    }

    #[test]
    fn load_latest_failures_extracts_broken_actions() {
        let mut lock = LockFile::default();
        lock.save_report_json(
            r#"{
            "actions": [
                {"status": "installed", "skill": "good", "source": "s"},
                {"status": "broken", "skill": "bad", "source": "s", "error": "missing"},
                {"status": "source_error", "skill": "err", "source": "s2", "error": "timeout"}
            ]
        }"#,
        );

        let failures = lock.load_latest_failures();
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].name, "bad");
        assert_eq!(failures[0].reason, "missing");
        assert_eq!(failures[1].name, "err");
    }

    #[test]
    fn load_latest_failures_includes_mcp_broken_actions() {
        let mut lock = LockFile::default();
        lock.save_report_json(
            r#"{
            "actions": [
                {"status": "broken", "skill": "mcp:pack.json", "source": "https://example.com/r", "error": "invalid JSON"}
            ]
        }"#,
        );

        let failures = lock.load_latest_failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].name, "mcp:pack.json");
        assert_eq!(failures[0].reason, "invalid JSON");
        assert_eq!(failures[0].source, "https://example.com/r");
    }

    #[test]
    fn list_installed_mcps_deduplicates() {
        let mut lock = LockFile::default();
        lock.save_tracked_asset("mcp", "a", "a", "h", "s", "srv1,srv2");
        lock.save_tracked_asset("mcp", "b", "b", "h", "s", "srv2,srv3");

        let mcps = lock.list_installed_mcps();
        assert_eq!(mcps, vec!["srv1", "srv2", "srv3"]);
    }

    #[test]
    fn state_round_trip() {
        let mut lock = LockFile::default();
        let mut state = State::default();
        state.last_run = Some("ts".to_string());
        state.skills.insert(
            "k".to_string(),
            SkillEntry {
                skill: "s".into(),
                hash: "h".into(),
                ..Default::default()
            },
        );

        lock.apply_state(&state);
        let recovered = lock.state();

        assert_eq!(recovered.last_run, state.last_run);
        assert_eq!(recovered.skills.len(), 1);
        assert_eq!(recovered.skills["k"].skill, "s");
    }
}
