//! Merge pack `mcpServers` into JSON-based agent settings.

use std::path::Path;

use crate::error::{err, Result};
use crate::fsops::SettingsFile;

type ServerMap = serde_json::Map<String, serde_json::Value>;

/// Shared scaffolding: load target settings, ensure the root object key exists,
/// apply `transform` to each entry from the (already secret-injected) source
/// map, save. `overwrite` replaces an existing same-named entry (used by the
/// `--update` rotation path); otherwise existing entries are preserved.
fn merge_into_json_key(
    src_map: &ServerMap,
    target_path: &Path,
    root_key: &str,
    overwrite: bool,
    transform: fn(&str, serde_json::Value) -> Result<serde_json::Value>,
) -> Result<()> {
    let mut sf = SettingsFile::load(target_path)?;
    let target_obj = sf
        .data
        .as_object_mut()
        .ok_or_else(|| err("settings file is not a JSON object"))?;
    let section = target_obj
        .entry(root_key)
        .or_insert_with(|| serde_json::json!({}));

    if let Some(dst_map) = section.as_object_mut() {
        for (key, value) in src_map {
            if overwrite || !dst_map.contains_key(key) {
                dst_map.insert(key.clone(), transform(key, value.clone())?);
            }
        }
    }
    sf.save()?;
    Ok(())
}

pub(super) fn merge_mcp_servers_object(
    src_map: &ServerMap,
    target_path: &Path,
    overwrite: bool,
) -> Result<()> {
    merge_into_json_key(src_map, target_path, "mcpServers", overwrite, |_name, v| {
        Ok(v)
    })
}

pub(super) fn merge_vscode_servers_object(
    src_map: &ServerMap,
    target_path: &Path,
    overwrite: bool,
) -> Result<()> {
    merge_into_json_key(src_map, target_path, "servers", overwrite, |_name, v| {
        Ok(normalize_vscode_server(v))
    })
}

pub(super) fn merge_opencode_mcp_object(
    src_map: &ServerMap,
    target_path: &Path,
    overwrite: bool,
) -> Result<()> {
    merge_into_json_key(src_map, target_path, "mcp", overwrite, |name, v| {
        mcp_entry_to_opencode(name, &v)
    })
}

fn normalize_vscode_server(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        if !obj.contains_key("type") {
            if obj.contains_key("command") {
                obj.insert("type".into(), serde_json::json!("stdio"));
            } else if obj.contains_key("url") {
                obj.insert("type".into(), serde_json::json!("http"));
            }
        }
    }
    value
}

fn mcp_entry_to_opencode(name: &str, v: &serde_json::Value) -> Result<serde_json::Value> {
    let Some(obj) = v.as_object() else {
        return Err(err(format!(
            "MCP server {name} must be a JSON object for OpenCode merge"
        )));
    };

    if let Some(url) = obj
        .get("url")
        .and_then(|u| u.as_str())
        .or_else(|| obj.get("serverUrl").and_then(|u| u.as_str()))
    {
        let mut out = serde_json::Map::new();
        out.insert("type".into(), serde_json::json!("remote"));
        out.insert("url".into(), serde_json::json!(url));
        out.insert("enabled".into(), serde_json::json!(true));
        if let Some(h) = obj.get("headers").and_then(|x| x.as_object()) {
            out.insert("headers".into(), serde_json::Value::Object(h.clone()));
        }
        return Ok(serde_json::Value::Object(out));
    }

    let cmd = obj.get("command").and_then(|c| c.as_str()).ok_or_else(|| {
        err(format!(
            "MCP server {name} needs `command` or `url` for OpenCode"
        ))
    })?;

    let mut cmd_arr = vec![serde_json::json!(cmd)];
    if let Some(args) = obj.get("args").and_then(|a| a.as_array()) {
        cmd_arr.extend(args.iter().cloned());
    }

    let mut out = serde_json::Map::new();
    out.insert("type".into(), serde_json::json!("local"));
    out.insert("command".into(), serde_json::Value::Array(cmd_arr));
    out.insert("enabled".into(), serde_json::json!(true));
    if let Some(env) = obj.get("env").and_then(|e| e.as_object()) {
        out.insert("environment".into(), serde_json::Value::Object(env.clone()));
    }
    Ok(serde_json::Value::Object(out))
}
