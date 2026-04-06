//! MCP pack merge / removal across agent-native config formats.

mod codex;
mod merge;
mod pack;

use std::fs;
use std::path::Path;

use crate::error::Result;
use crate::fsops::SettingsFile;
use crate::model::McpSettingsFormat;

/// Merge MCP server definitions from a pack JSON into an agent-native config file.
/// The pack must have a top-level `"mcpServers"` object.
pub(crate) fn merge_mcp_config(
    source_path: &Path,
    target: &crate::model::McpSettingsTarget,
) -> Result<()> {
    match target.format {
        McpSettingsFormat::McpServers => merge::merge_mcp_servers_object(source_path, &target.path),
        McpSettingsFormat::VsCodeServers => {
            merge::merge_vscode_servers_object(source_path, &target.path)
        }
        McpSettingsFormat::OpenCode => merge::merge_opencode_mcp_object(source_path, &target.path),
        McpSettingsFormat::CodexToml => codex::merge_codex_config_toml(source_path, &target.path),
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
    use crate::model::McpSettingsTarget;
    use std::fs;
    use toml::Value as TomlVal;

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    fn mcp_target(path: std::path::PathBuf) -> McpSettingsTarget {
        McpSettingsTarget {
            path,
            format: McpSettingsFormat::McpServers,
        }
    }

    #[test]
    fn merge_mcp_config_creates_target_from_scratch() {
        let dir = temp_dir("kasetto-mcps-create");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.json");
        let target = dir.join("settings.json");

        fs::write(
            &source,
            r#"{"mcpServers":{"git-tools":{"command":"git-mcp"}}}"#,
        )
        .unwrap();

        merge_mcp_config(&source, &mcp_target(target.clone())).expect("merge");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(val["mcpServers"]["git-tools"]["command"], "git-mcp");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_mcp_config_preserves_existing_servers() {
        let dir = temp_dir("kasetto-mcps-merge");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.json");
        let target = dir.join("settings.json");

        fs::write(
            &target,
            r#"{"mcpServers":{"existing":{"command":"keep-me"}}}"#,
        )
        .unwrap();
        fs::write(
            &source,
            r#"{"mcpServers":{"new-server":{"command":"new-cmd"}}}"#,
        )
        .unwrap();

        merge_mcp_config(&source, &mcp_target(target.clone())).expect("merge");

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
        let source = dir.join("source.json");
        let target = dir.join("settings.json");

        fs::write(
            &target,
            r#"{"mcpServers":{"airflow":{"command":"uvx","env":{"AIRFLOW_PASSWORD":"real-secret"}}}}"#,
        )
        .unwrap();
        fs::write(
            &source,
            r#"{"mcpServers":{"airflow":{"command":"uvx","env":{"AIRFLOW_PASSWORD":"__FROM_SOURCE_PACK__"}}}}"#,
        )
        .unwrap();

        merge_mcp_config(&source, &mcp_target(target.clone())).expect("merge");

        let val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(
            val["mcpServers"]["airflow"]["env"]["AIRFLOW_PASSWORD"],
            "real-secret"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_codex_writes_config_toml() {
        let dir = temp_dir("kasetto-mcps-codex");
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.json");
        let target = dir.join("config.toml");
        fs::write(
            &source,
            r#"{"mcpServers":{"demo":{"command":"uvx","args":["p"],"env":{"K":"v"}}}}"#,
        )
        .unwrap();
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::CodexToml,
        };
        merge_mcp_config(&source, &tgt).expect("merge");
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
        let source = dir.join("source.json");
        let target = dir.join("config.toml");
        fs::write(&target, "model = \"gpt-5.1\"\n").unwrap();
        fs::write(
            &source,
            r#"{"mcpServers":{"new":{"command":"npx","args":["-y","x"]}}}"#,
        )
        .unwrap();
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::CodexToml,
        };
        merge_mcp_config(&source, &tgt).expect("merge");
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
        let source = dir.join("source.json");
        let target = dir.join("mcp.json");
        fs::write(
            &source,
            r#"{"mcpServers":{"mem":{"command":"npx","args":["-y","@x/y"]}}}"#,
        )
        .unwrap();
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::VsCodeServers,
        };
        merge_mcp_config(&source, &tgt).expect("merge");
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
        let source = dir.join("source.json");
        let target = dir.join("opencode.json");
        fs::write(
            &source,
            r#"{"mcpServers":{"tool":{"command":"uvx","args":["pkg"],"env":{"K":"v"}}}}"#,
        )
        .unwrap();
        let tgt = McpSettingsTarget {
            path: target.clone(),
            format: McpSettingsFormat::OpenCode,
        };
        merge_mcp_config(&source, &tgt).expect("merge");
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
            &mcp_target(settings.clone())
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
            &mcp_target(settings.clone())
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
