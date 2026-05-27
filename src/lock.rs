use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::dirs_kasetto_data;
use crate::model::{Scope, SkillEntry, State, LOCK_VERSION};

pub(crate) const LOCK_FILENAME: &str = "kasetto.lock";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub(crate) struct AssetEntry {
    pub kind: String,
    pub name: String,
    pub hash: String,
    pub source: String,
    /// For commands: install paths relative to the scope root (CSV).
    /// For MCPs: the merged server names (CSV).
    pub destination: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LockFile {
    #[serde(default = "default_version")]
    pub version: u8,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillEntry>,
    #[serde(default)]
    pub assets: BTreeMap<String, AssetEntry>,
}

fn default_version() -> u8 {
    LOCK_VERSION
}

impl Default for LockFile {
    fn default() -> Self {
        Self {
            version: LOCK_VERSION,
            skills: BTreeMap::new(),
            assets: BTreeMap::new(),
        }
    }
}

impl LockFile {
    pub(crate) fn state(&self) -> State {
        State {
            version: self.version,
            skills: self.skills.clone(),
        }
    }

    pub(crate) fn apply_state(&mut self, state: &State) {
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
    }

    pub(crate) fn list_installed_commands(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .assets
            .iter()
            .filter(|(_, a)| a.kind == "command")
            .map(|(_, a)| a.name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
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
/// Stamps the current schema version so a migrated older lock is relabeled.
pub(crate) fn save_lock(lock: &mut LockFile, scope: Scope, project_root: &Path) -> Result<PathBuf> {
    lock.version = LOCK_VERSION;
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

        let mut lock = LockFile::default();
        save_lock(&mut lock, Scope::Project, &dir).unwrap();

        let loaded = load_lock(Scope::Project, &dir).unwrap();
        assert_eq!(loaded.version, 2);
        assert!(loaded.skills.is_empty());
        assert!(loaded.assets.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trip_with_skills_and_assets() {
        let dir = temp_dir("kasetto-lock-data");
        fs::create_dir_all(&dir).unwrap();

        let mut lock = LockFile::default();
        lock.skills.insert(
            "src::skill-a".to_string(),
            SkillEntry {
                destination: ".claude/skills/skill-a".into(),
                hash: "abc".into(),
                skill: "skill-a".into(),
                description: "desc".into(),
                source: "src".into(),
                source_revision: "rev1".into(),
                scope: None,
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

        save_lock(&mut lock, Scope::Project, &dir).unwrap();
        let loaded = load_lock(Scope::Project, &dir).unwrap();

        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(loaded.skills["src::skill-a"].hash, "abc");
        assert_eq!(loaded.assets.len(), 1);

        let asset = loaded.get_tracked_asset("mcp", "mcp::src::pack.json");
        assert_eq!(asset, Some(("h1".into(), "srv1,srv2".into())));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_v1_lock_loads_and_restamps_on_save() {
        let dir = temp_dir("kasetto-lock-legacy");
        fs::create_dir_all(&dir).unwrap();

        // A v1 lock carrying fields that no longer exist in the schema plus an
        // absolute destination. Unknown fields must be ignored, absolute paths
        // honored, and the version relabeled to the current schema on save.
        let legacy = "version: 1\n\
last_run: '111'\n\
latest_report: '{\"actions\":[]}'\n\
skills:\n\
\x20 src::a:\n\
\x20\x20\x20 destination: /abs/path/.claude/skills/a\n\
\x20\x20\x20 hash: h\n\
\x20\x20\x20 skill: a\n\
\x20\x20\x20 source: src\n\
\x20\x20\x20 source_revision: local\n\
\x20\x20\x20 updated_at: '111'\n\
assets: {}\n";
        fs::write(dir.join(LOCK_FILENAME), legacy).unwrap();

        let mut loaded = load_lock(Scope::Project, &dir).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(
            loaded.skills["src::a"].destination,
            "/abs/path/.claude/skills/a"
        );

        save_lock(&mut loaded, Scope::Project, &dir).unwrap();
        let resaved = fs::read_to_string(dir.join(LOCK_FILENAME)).unwrap();
        assert!(resaved.starts_with("version: 2"));
        assert!(!resaved.contains("last_run"));
        assert!(!resaved.contains("latest_report"));
        assert!(!resaved.contains("updated_at"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_returns_default_when_missing() {
        let dir = temp_dir("kasetto-lock-missing");
        fs::create_dir_all(&dir).unwrap();

        let lock = load_lock(Scope::Project, &dir).unwrap();
        assert_eq!(lock.version, 2);
        assert!(lock.skills.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_all_empties_everything() {
        let mut lock = LockFile::default();
        lock.skills.insert(
            "k".to_string(),
            SkillEntry {
                skill: "s".into(),
                hash: "h".into(),
                ..Default::default()
            },
        );
        lock.save_tracked_asset("mcp", "id", "n", "h", "s", "d");

        lock.clear_all();

        assert!(lock.skills.is_empty());
        assert!(lock.assets.is_empty());
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

        assert_eq!(recovered.skills.len(), 1);
        assert_eq!(recovered.skills["k"].skill, "s");
    }
}
