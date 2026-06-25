//! MCP pack merge / removal across agent-native config formats.

mod codex;
mod merge;

use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::fsops::SettingsFile;
use crate::model::McpSettingsFormat;

/// Merge MCP server definitions into an agent-native config file. `src_map` is
/// the pack's `mcpServers` object, already secret-injected in the sync phase.
/// `overwrite` replaces an existing same-named server (the `--update` rotation
/// path); otherwise existing entries are preserved.
pub(crate) fn merge_mcp_config(
    src_map: &serde_json::Map<String, serde_json::Value>,
    target: &crate::model::McpSettingsTarget,
    overwrite: bool,
) -> Result<()> {
    match target.format {
        McpSettingsFormat::McpServers => {
            merge::merge_mcp_servers_object(src_map, &target.path, overwrite)
        }
        McpSettingsFormat::VsCodeServers => {
            merge::merge_vscode_servers_object(src_map, &target.path, overwrite)
        }
        McpSettingsFormat::OpenCode => {
            merge::merge_opencode_mcp_object(src_map, &target.path, overwrite)
        }
        McpSettingsFormat::CodexToml => {
            codex::merge_codex_config_toml(src_map, &target.path, overwrite)
        }
    }
}

pub(crate) fn remove_mcp_server(
    server_name: &str,
    target: &crate::model::McpSettingsTarget,
) -> Result<()> {
    if !target.path.exists() {
        return Ok(());
    }
    match target.format {
        McpSettingsFormat::CodexToml => codex::remove_server(server_name, &target.path),
        McpSettingsFormat::McpServers => {
            json_remove_top_level_key(server_name, &target.path, "mcpServers")
        }
        McpSettingsFormat::VsCodeServers => {
            json_remove_top_level_key(server_name, &target.path, "servers")
        }
        McpSettingsFormat::OpenCode => json_remove_top_level_key(server_name, &target.path, "mcp"),
    }
}

fn json_remove_top_level_key(server_name: &str, path: &Path, object_key: &str) -> Result<()> {
    let mut sf = SettingsFile::load(path)?;
    if let Some(map) = sf.data.get_mut(object_key).and_then(|v| v.as_object_mut()) {
        map.remove(server_name);
    }
    sf.save()?;
    Ok(())
}

pub(crate) fn servers_present_in_settings(
    server_names: &[String],
    target: &crate::model::McpSettingsTarget,
) -> bool {
    if server_names.is_empty() {
        return true;
    }
    match target.format {
        McpSettingsFormat::CodexToml => codex::servers_present(server_names, &target.path),
        McpSettingsFormat::McpServers => {
            json_all_keys_present(server_names, &target.path, "mcpServers")
        }
        McpSettingsFormat::VsCodeServers => {
            json_all_keys_present(server_names, &target.path, "servers")
        }
        McpSettingsFormat::OpenCode => json_all_keys_present(server_names, &target.path, "mcp"),
    }
}

fn json_all_keys_present(server_names: &[String], path: &Path, root_key: &str) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let Some(map) = val.get(root_key).and_then(|v| v.as_object()) else {
        return false;
    };
    server_names.iter().all(|name| map.contains_key(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::McpSettingsTarget;
    use std::fs;
    use toml::Value as TomlVal;

    fn mcp_target(path: std::path::PathBuf) -> McpSettingsTarget {
        McpSettingsTarget {
            path,
            format: McpSettingsFormat::McpServers,
        }
    }

    /// Parse a pack JSON string into its `mcpServers` object (what the sync
    /// phase hands to `merge_mcp_config` after secret injection).
    fn servers(json: &str) -> serde_json::Map<String, serde_json::Value> {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v.get("mcpServers")
            .and_then(|m| m.as_object())
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn merge_mcp_config_creates_target_from_scratch() {
        let dir = temp_dir("kasetto-mcps-create");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("settings.json");
        let src = servers(r#"{"mcpServers":{"git-tools":{"command":"git-mcp"}}}"#);

        merge_mcp_config(&src, &mcp_target(target.clone()), false).expect("merge");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(val["mcpServers"]["git-tools"]["command"], "git-mcp");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_mcp_config_preserves_existing_servers() {
        let dir = temp_dir("kasetto-mcps-merge");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("settings.json");

        fs::write(
            &target,
            r#"{"mcpServers":{"existing":{"command":"keep-me"}}}"#,
        )
        .unwrap();
        let src = servers(r#"{"mcpServers":{"new-server":{"command":"new-cmd"}}}"#);

        merge_mcp_config(&src, &mcp_target(target.clone()), false).expect("merge");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(val["mcpServers"]["existing"]["command"], "keep-me");
        assert_eq!(val["mcpServers"]["new-server"]["command"], "new-cmd");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_mcp_config_does_not_overwrite_existing_key() {
        let dir = temp_dir("kasetto-mcps-no-overwrite");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("settings.json");

        fs::write(
            &target,
            r#"{"mcpServers":{"airflow":{"command":"uvx","env":{"AIRFLOW_PASSWORD":"real-secret"}}}}"#,
        )
        .unwrap();
        let src = servers(
            r#"{"mcpServers":{"airflow":{"command":"uvx","env":{"AIRFLOW_PASSWORD":"__FROM_SOURCE_PACK__"}}}}"#,
        );

        merge_mcp_config(&src, &mcp_target(target.clone()), false).expect("merge");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(
            val["mcpServers"]["airflow"]["env"]["AIRFLOW_PASSWORD"],
            "real-secret"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_mcp_config_overwrite_replaces_existing_key() {
        let dir = temp_dir("kasetto-mcps-overwrite");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("settings.json");

        fs::write(
            &target,
            r#"{"mcpServers":{"vercel":{"url":"https://mcp.vercel.com","headers":{"Authorization":"Bearer old-token"}}}}"#,
        )
        .unwrap();
        // The `--update` rotation path: overwrite=true replaces the managed
        // entry with the freshly-injected one.
        let src = servers(
            r#"{"mcpServers":{"vercel":{"url":"https://mcp.vercel.com","headers":{"Authorization":"Bearer new-token"}}}}"#,
        );

        merge_mcp_config(&src, &mcp_target(target.clone()), true).expect("merge");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(
            val["mcpServers"]["vercel"]["headers"]["Authorization"],
            "Bearer new-token"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_codex_writes_config_toml() {
        let dir = temp_dir("kasetto-mcps-codex");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("config.toml");
        let src =
            servers(r#"{"mcpServers":{"demo":{"command":"uvx","args":["p"],"env":{"K":"v"}}}}"#);
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::CodexToml,
        };
        merge_mcp_config(&src, &tgt, false).expect("merge");
        let parsed: TomlVal = fs::read_to_string(&target).unwrap().parse().unwrap();
        let mcp = parsed.get("mcp_servers").unwrap().as_table().unwrap();
        assert_eq!(mcp["demo"]["command"].as_str().unwrap(), "uvx");
        let args = mcp["demo"]["args"].as_array().unwrap();
        assert_eq!(args[0].as_str().unwrap(), "p");
        assert_eq!(mcp["demo"]["env"]["K"].as_str().unwrap(), "v");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_codex_preserves_unrelated_toml_keys() {
        let dir = temp_dir("kasetto-mcps-codex-merge");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("config.toml");
        fs::write(&target, "model = \"gpt-5.1\"\n").unwrap();
        let src = servers(r#"{"mcpServers":{"new":{"command":"npx","args":["-y","x"]}}}"#);
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::CodexToml,
        };
        merge_mcp_config(&src, &tgt, false).expect("merge");
        let parsed: TomlVal = fs::read_to_string(&target).unwrap().parse().unwrap();
        assert_eq!(
            parsed.get("model").and_then(|v| v.as_str()).unwrap(),
            "gpt-5.1"
        );
        assert!(parsed
            .get("mcp_servers")
            .unwrap()
            .as_table()
            .unwrap()
            .contains_key("new"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_codex_mcp_server_entry() {
        let dir = temp_dir("kasetto-mcps-codex-rm");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(
            &path,
            r#"[mcp_servers.a]
command = "a"
[mcp_servers.b]
command = "b"
"#,
        )
        .unwrap();
        let tgt = McpSettingsTarget {
            path: path.clone(),
            format: McpSettingsFormat::CodexToml,
        };
        remove_mcp_server("a", &tgt).expect("remove");
        let parsed: TomlVal = fs::read_to_string(&path).unwrap().parse().unwrap();
        let mcp = parsed["mcp_servers"].as_table().unwrap();
        assert!(!mcp.contains_key("a"));
        assert!(mcp.contains_key("b"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_vscode_adds_stdio_type() {
        let dir = temp_dir("kasetto-mcps-vscode");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("mcp.json");
        let src = servers(r#"{"mcpServers":{"mem":{"command":"npx","args":["-y","@x/y"]}}}"#);
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::VsCodeServers,
        };
        merge_mcp_config(&src, &tgt, false).expect("merge");
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(val["servers"]["mem"]["type"], "stdio");
        assert_eq!(val["servers"]["mem"]["command"], "npx");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_opencode_local_command() {
        let dir = temp_dir("kasetto-mcps-opencode");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("opencode.json");
        let src =
            servers(r#"{"mcpServers":{"tool":{"command":"uvx","args":["pkg"],"env":{"K":"v"}}}}"#);
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::OpenCode,
        };
        merge_mcp_config(&src, &tgt, false).expect("merge");
        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(val["mcp"]["tool"]["type"], "local");
        assert_eq!(val["mcp"]["tool"]["command"][0], "uvx");
        assert_eq!(val["mcp"]["tool"]["environment"]["K"], "v");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_mcp_server_deletes_entry() {
        let dir = temp_dir("kasetto-mcps-remove");
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.json");

        fs::write(
            &settings,
            r#"{"mcpServers":{"a":{"cmd":"1"},"b":{"cmd":"2"}}}"#,
        )
        .unwrap();

        remove_mcp_server("a", &mcp_target(settings.clone())).expect("remove");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
        assert!(val["mcpServers"]["a"].is_null());
        assert_eq!(val["mcpServers"]["b"]["cmd"], "2");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_mcp_server_noop_on_missing_file() {
        let path = temp_dir("kasetto-mcps-noop").join("nonexistent.json");
        remove_mcp_server(
            "some-server",
            &McpSettingsTarget {
                path,
                format: McpSettingsFormat::McpServers,
            },
        )
        .unwrap();
    }

    #[test]
    fn servers_present_all_exist() {
        let dir = temp_dir("kasetto-mcps-present");
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.json");
        fs::write(
            &settings,
            r#"{"mcpServers":{"airflow":{"cmd":"a"},"git":{"cmd":"g"}}}"#,
        )
        .unwrap();

        assert!(servers_present_in_settings(
            &["airflow".into(), "git".into()],
            &mcp_target(settings)
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn servers_present_missing_server() {
        let dir = temp_dir("kasetto-mcps-missing");
        fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.json");
        fs::write(&settings, r#"{"mcpServers":{"git":{"cmd":"g"}}}"#).unwrap();

        assert!(!servers_present_in_settings(
            &["airflow".into(), "git".into()],
            &mcp_target(settings)
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn servers_present_missing_file() {
        let path = temp_dir("kasetto-mcps-nofile").join("nope.json");
        assert!(!servers_present_in_settings(
            &["airflow".into()],
            &mcp_target(path)
        ));
    }

    #[test]
    fn servers_present_empty_list() {
        let path = temp_dir("kasetto-mcps-empty").join("nope.json");
        assert!(servers_present_in_settings(&[], &mcp_target(path)));
    }
}
