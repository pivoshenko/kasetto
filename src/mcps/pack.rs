//! Read `mcpServers` definitions from a pack JSON file.

use std::fs;
use std::path::Path;

use crate::error::{err, Result};

pub(super) fn read_source_mcp_servers(
    source_path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let source_text = fs::read_to_string(source_path)?;
    let source: serde_json::Value = serde_json::from_str(&source_text)
        .map_err(|e| err(format!("invalid MCP JSON {}: {e}", source_path.display())))?;
    Ok(source
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default())
}
