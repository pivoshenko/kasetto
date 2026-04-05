//! OpenAI Codex `~/.codex/config.toml` — `mcp_servers` map.

use std::fs;
use std::path::Path;

use crate::error::{err, Result};
use toml::Value as Toml;

use super::pack::read_source_mcp_servers;

pub(super) fn merge_codex_config_toml(source_path: &Path, target_path: &Path) -> Result<()> {
    let src_map = read_source_mcp_servers(source_path)?;
    let mut root = load_or_empty_toml(target_path)?;
    let root_tbl = root
        .as_table_mut()
        .ok_or_else(|| err("Codex config root must be a TOML table"))?;
    let mcp_entry = root_tbl
        .entry("mcp_servers")
        .or_insert_with(|| Toml::Table(Default::default()));
    let mcp_tbl = mcp_entry
        .as_table_mut()
        .ok_or_else(|| err("Codex mcp_servers must be a TOML table"))?;

    for (name, cfg) in src_map {
        if mcp_tbl.contains_key(&name) {
            continue;
        }
        let table = json_mcp_server_to_codex_toml_table(&cfg)?;
        mcp_tbl.insert(name, Toml::Table(table));
    }

    write_codex_toml(target_path, &root)
}

pub(super) fn remove_server(server_name: &str, target_path: &Path) -> Result<()> {
    let mut root = load_or_empty_toml(target_path)?;
    if let Some(mcp) = root
        .as_table_mut()
        .and_then(|t| t.get_mut("mcp_servers"))
        .and_then(|m| m.as_table_mut())
    {
        mcp.remove(server_name);
    }
    write_codex_toml(target_path, &root)
}

pub(super) fn servers_present(server_names: &[String], target_path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(target_path) else {
        return false;
    };
    let Ok(val) = text.parse::<Toml>() else {
        return false;
    };
    let Some(map) = val.get("mcp_servers").and_then(|v| v.as_table()) else {
        return false;
    };
    server_names.iter().all(|name| map.contains_key(name))
}

fn load_or_empty_toml(target_path: &Path) -> Result<Toml> {
    if !target_path.exists() {
        return Ok(Toml::Table(Default::default()));
    }
    let text = fs::read_to_string(target_path)?;
    text.parse::<Toml>().map_err(|e| {
        err(format!(
            "invalid Codex config TOML {}: {e}",
            target_path.display()
        ))
    })
}

fn write_codex_toml(target_path: &Path, root: &Toml) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let out = toml::to_string_pretty(root)
        .map_err(|e| err(format!("failed to serialize Codex config.toml: {e}")))?;
    fs::write(target_path, out)?;
    Ok(())
}

fn json_mcp_server_to_codex_toml_table(
    v: &serde_json::Value,
) -> Result<toml::map::Map<String, Toml>> {
    let obj = v
        .as_object()
        .ok_or_else(|| err("each mcpServers entry must be a JSON object for Codex"))?;
    let mut out = toml::map::Map::new();

    let url = obj
        .get("url")
        .and_then(|u| u.as_str())
        .or_else(|| obj.get("serverUrl").and_then(|u| u.as_str()));

    let ty = obj.get("type").and_then(|t| t.as_str());
    let is_remote =
        url.is_some() || matches!(ty, Some("http" | "https" | "sse" | "streamable-http"));

    if is_remote {
        let Some(url) = url else {
            return Err(err(
                "remote MCP entry for Codex needs a string `url` (or `serverUrl`)",
            ));
        };
        out.insert("url".into(), Toml::String(url.to_string()));

        if let Some(h) = obj.get("headers").and_then(|x| x.as_object()) {
            let mut ht = toml::map::Map::new();
            for (k, v) in h {
                if let Some(s) = v.as_str() {
                    ht.insert(k.clone(), Toml::String(s.to_string()));
                }
            }
            if !ht.is_empty() {
                out.insert("http_headers".into(), Toml::Table(ht));
            }
        }
        return Ok(out);
    }

    let cmd = obj
        .get("command")
        .and_then(|c| c.as_str())
        .ok_or_else(|| err("Codex stdio MCP needs a string `command` (or use `url` for remote)"))?;
    out.insert("command".into(), Toml::String(cmd.to_string()));

    if let Some(args) = obj.get("args").and_then(|a| a.as_array()) {
        let arr: Vec<Toml> = args
            .iter()
            .map(|x| {
                Toml::String(match x {
                    serde_json::Value::String(s) => s.clone(),
                    _ => x.to_string(),
                })
            })
            .collect();
        if !arr.is_empty() {
            out.insert("args".into(), Toml::Array(arr));
        }
    }

    if let Some(env) = obj.get("env").and_then(|e| e.as_object()) {
        let mut et = toml::map::Map::new();
        for (k, v) in env {
            let s = match v {
                serde_json::Value::String(s) => s.clone(),
                _ => v.to_string().trim_matches('"').to_string(),
            };
            et.insert(k.clone(), Toml::String(s));
        }
        if !et.is_empty() {
            out.insert("env".into(), Toml::Table(et));
        }
    }

    Ok(out)
}
