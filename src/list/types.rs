use crate::model::InstalledSkill;

#[derive(Clone)]
pub(crate) struct AssetEntry {
    pub name: String,
}

/// Rows for the MCP tab in the list TUI (kasetto-tracked servers only).
pub(crate) fn mcp_asset_entries(names: &[String]) -> Vec<AssetEntry> {
    names
        .iter()
        .map(|name| AssetEntry { name: name.clone() })
        .collect()
}

pub(crate) struct BrowseInput {
    pub skills: Vec<InstalledSkill>,
    pub mcps: Vec<AssetEntry>,
}
