use serde::Serialize;

use crate::lock::LockFile;
use crate::model::{InstalledSkill, Scope};

#[derive(Clone, Serialize)]
pub(crate) struct AssetEntry {
    pub name: String,
    pub scope: Scope,
    pub pack_file: String,
    pub source: String,
}

/// Command rows for the list TUI / JSON (one row per managed command in the lock).
pub(crate) fn command_asset_entries(lock: &LockFile, scope: Scope) -> Vec<AssetEntry> {
    let mut out: Vec<AssetEntry> = lock
        .assets
        .iter()
        .filter(|(_, a)| a.kind == "command")
        .map(|(_, a)| AssetEntry {
            name: a.name.clone(),
            scope,
            pack_file: String::new(),
            source: a.source.clone(),
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// MCP rows for the list TUI / JSON (one row per managed server name in the lock).
pub(crate) fn mcp_asset_entries(lock: &LockFile, scope: Scope) -> Vec<AssetEntry> {
    let mut out = Vec::new();
    for name in lock.list_installed_mcps() {
        let (pack_file, source) = lock
            .assets
            .iter()
            .filter(|(_, a)| a.kind == "mcp")
            .find(|(_, a)| a.destination.split(',').any(|s| !s.is_empty() && s == name))
            .map(|(_, a)| (a.name.clone(), a.source.clone()))
            .unwrap_or_default();
        out.push(AssetEntry {
            name,
            scope,
            pack_file,
            source,
        });
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

pub(crate) struct BrowseInput {
    pub skills: Vec<InstalledSkill>,
    pub mcps: Vec<AssetEntry>,
    pub commands: Vec<AssetEntry>,
    pub plain: bool,
}
