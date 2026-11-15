//! Package that contains plugin marketplace parsing and the settings merge that
//! hands plugin installs to Claude Code and Copilot CLI.
//!
//! Kasetto never copies a plugin. It declares the marketplace under
//! `extraKnownMarketplaces` and turns plugins on under `enabledPlugins`; the tool
//! clones the marketplace and installs the plugin at its next session start.
//! Deleting those keys disables the plugin, but the tool keeps its own clone and
//! cache.

use std::fs;
use std::path::{Component, Path};

use serde_json::{Map, Value};

use crate::error::{err, Result};
use crate::fsops::{hash_dir, hash_str, resolve_path, SettingsFile};
use crate::model::PluginSourceSpec;
use crate::source::{is_local_source, parse_repo_url, RepoUrl};

const MARKETPLACES: &str = "extraKnownMarketplaces";
const ENABLED: &str = "enabledPlugins";

/// A parsed `.claude-plugin/marketplace.json`.
#[derive(Debug)]
pub(crate) struct Marketplace {
    pub name: String,
    pub plugins: Vec<MarketplacePlugin>,
    /// Entry names Claude Code would refuse as plugin ids. They are skipped rather
    /// than failing the marketplace, so one bad sibling cannot disable the rest.
    pub invalid: Vec<String>,
}

/// One `plugins[]` entry of a marketplace.
#[derive(Debug)]
pub(crate) struct MarketplacePlugin {
    pub name: String,
    /// The plugin directory when its `source` is a `./` path inside the
    /// marketplace repository, so its content can be hashed into the lock.
    pub relative_dir: Option<String>,
    entry: Value,
}

impl MarketplacePlugin {
    /// Content hash for the lock: the marketplace entry, plus the plugin tree when
    /// it lives in the marketplace repository. An external-source plugin is pinned
    /// by its entry alone, since Claude Code fetches it from elsewhere.
    pub(crate) fn hash(&self, root: &Path) -> Result<String> {
        let mut digest = serde_json::to_string(&self.entry)?;
        if let Some(dir) = &self.relative_dir {
            digest.push_str(&hash_dir(&root.join(dir))?);
        }
        Ok(hash_str(&digest))
    }
}

/// `plugin@marketplace`, the key Claude Code and Copilot CLI use in `enabledPlugins`.
pub(crate) fn plugin_id(plugin: &str, marketplace: &str) -> String {
    format!("{plugin}@{marketplace}")
}

/// Read `<root>/.claude-plugin/marketplace.json`. The marketplace name comes from
/// the file, never the URL: `cursor/plugins` is named `cursor-plugins`.
pub(crate) fn read_marketplace(root: &Path) -> Result<Marketplace> {
    let path = root.join(".claude-plugin/marketplace.json");
    let text = fs::read_to_string(&path).map_err(|_| {
        err(format!(
            "no .claude-plugin/marketplace.json in source (looked at {})",
            path.display()
        ))
    })?;
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| err(format!("invalid marketplace.json {}: {e}", path.display())))?;
    let name = doc
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| err("marketplace.json has no `name`"))?;
    check_id_part("marketplace", name)?;
    let entries = doc
        .get("plugins")
        .and_then(Value::as_array)
        .ok_or_else(|| err("marketplace.json has no `plugins` array"))?;
    let mut plugins = Vec::new();
    let mut invalid = Vec::new();
    for entry in entries {
        let Some(plugin_name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if check_id_part("plugin", plugin_name).is_err() {
            invalid.push(plugin_name.to_string());
            continue;
        }
        let relative_dir = entry
            .get("source")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix("./"))
            .map(|s| s.trim_end_matches('/').to_string());
        plugins.push(MarketplacePlugin {
            name: plugin_name.to_string(),
            relative_dir,
            entry: entry.clone(),
        });
    }
    Ok(Marketplace {
        name: name.to_string(),
        plugins,
        invalid,
    })
}

/// Both halves of a plugin id allow only `A-Z a-z 0-9 . _ -`, starting with a
/// letter or digit; Claude Code refuses anything else.
fn check_id_part(what: &str, value: &str) -> Result<()> {
    let mut chars = value.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !value.contains("..");
    if valid {
        Ok(())
    } else {
        Err(err(format!(
            "invalid {what} name `{value}` in marketplace.json"
        )))
    }
}

/// The `extraKnownMarketplaces.<name>.source` object for a config entry.
///
/// `ref` carries the config's tag or branch, never a resolved commit: Claude Code
/// before 2.1.324 passes `ref` to `git clone --branch`, so a commit SHA there
/// fails silently and unloads the plugin.
pub(crate) fn marketplace_source(spec: &PluginSourceSpec, cfg_dir: &Path) -> Result<Value> {
    let mut source = Map::new();
    if is_local_source(&spec.source) {
        let mut root = resolve_path(cfg_dir, &spec.source);
        if let Some(sub) = &spec.sub_dir {
            root = root.join(sub);
        }
        let root: std::path::PathBuf = root
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        source.insert("source".into(), "directory".into());
        source.insert("path".into(), root.to_string_lossy().into_owned().into());
        return Ok(Value::Object(source));
    }
    match parse_repo_url(&spec.source)? {
        RepoUrl::GitHub {
            host, owner, repo, ..
        } if host == "github.com" => {
            source.insert("source".into(), "github".into());
            source.insert("repo".into(), format!("{owner}/{repo}").into());
        }
        _ => {
            source.insert("source".into(), "git".into());
            source.insert("url".into(), spec.source.clone().into());
        }
    }
    if let Some(r) = spec.git_ref.as_ref().or(spec.branch.as_ref()) {
        source.insert("ref".into(), r.clone().into());
    }
    if let Some(sub) = &spec.sub_dir {
        let sub = sub.trim_matches('/');
        source.insert(
            "path".into(),
            format!("{sub}/.claude-plugin/marketplace.json").into(),
        );
    }
    Ok(Value::Object(source))
}

/// Lock hash of a marketplace `source` object. serde_json maps are key-sorted,
/// so the serialization is canonical.
pub(crate) fn source_hash(source: &Value) -> String {
    hash_str(&source.to_string())
}

/// Hash of the `source` a settings file declares for marketplace `name`, if any.
/// Sync compares it with what it would write and with what the lock says Kasetto
/// wrote, to tell its own entry from one the user added by hand.
pub(crate) fn declared_source_hash(path: &Path, name: &str) -> Result<Option<String>> {
    let sf = SettingsFile::load(path)?;
    Ok(sf
        .data
        .get(MARKETPLACES)
        .and_then(|m| m.get(name))
        .map(|entry| entry.get("source").map(source_hash).unwrap_or_default()))
}

/// Declare the marketplace (when `declare` is set) and enable `plugin_ids`.
/// Every other key in the file is left alone; an existing entry for the same
/// marketplace keeps fields the tool added, such as `autoUpdate`.
pub(crate) fn write_plugins(
    path: &Path,
    name: &str,
    source: &Value,
    declare: bool,
    plugin_ids: &[String],
) -> Result<()> {
    let mut sf = SettingsFile::load(path)?;
    let root = sf
        .data
        .as_object_mut()
        .ok_or_else(|| err(format!("{} is not a JSON object", path.display())))?;
    if declare {
        let entry = object_at(root, MARKETPLACES)?
            .entry(name.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        let entry = entry.as_object_mut().ok_or_else(|| {
            err(format!(
                "{MARKETPLACES}.{name} in {} is not an object",
                path.display()
            ))
        })?;
        entry.insert("source".into(), source.clone());
    }
    let enabled = object_at(root, ENABLED)?;
    for id in plugin_ids {
        enabled.insert(id.clone(), Value::Bool(true));
    }
    sf.save()
}

fn object_at<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>> {
    root.entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| err(format!("settings key `{key}` is not an object")))
}

/// The current `enabledPlugins[id]` value in a settings file, if the key is set.
pub(crate) fn plugin_value(path: &Path, id: &str) -> Result<Option<Value>> {
    let sf = SettingsFile::load(path)?;
    Ok(sf.data.get(ENABLED).and_then(|m| m.get(id)).cloned())
}

pub(crate) fn plugin_enabled(path: &Path, id: &str) -> bool {
    SettingsFile::load(path)
        .ok()
        .and_then(|sf| sf.data.get(ENABLED)?.get(id)?.as_bool())
        .unwrap_or(false)
}

/// Remove `enabledPlugins[id]`. A missing file or key is not an error.
pub(crate) fn remove_plugin(path: &Path, id: &str) -> Result<()> {
    remove_entry(path, ENABLED, id, |_| true).map(|_| ())
}

/// Remove `extraKnownMarketplaces[name]`, but only while its `source` still hashes
/// to `expected_hash` (what Kasetto wrote). Returns whether it was removed.
pub(crate) fn remove_marketplace(path: &Path, name: &str, expected_hash: &str) -> Result<bool> {
    remove_entry(path, MARKETPLACES, name, |entry| {
        entry
            .get("source")
            .is_some_and(|s| source_hash(s) == expected_hash)
    })
}

fn remove_entry(
    path: &Path,
    key: &str,
    name: &str,
    owned: impl Fn(&Value) -> bool,
) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let mut sf = SettingsFile::load(path)?;
    let Some(map) = sf.data.get_mut(key).and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    if !map.get(name).is_some_and(&owned) {
        return Ok(false);
    }
    map.remove(name);
    if map.is_empty() {
        if let Some(root) = sf.data.as_object_mut() {
            root.remove(key);
        }
    }
    sf.save()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::PluginsField;

    const PSTACK: &str = r#"{
  "name": "cursor-plugins",
  "owner": { "name": "Cursor" },
  "plugins": [
    { "name": "origin-apps", "source": "./origin-apps" },
    { "name": "pstack", "source": "./harness/pstack/claude-code" },
    { "name": "remote", "source": { "source": "github", "repo": "o/r" } }
  ]
}"#;

    fn spec(source: &str, git_ref: Option<&str>, sub_dir: Option<&str>) -> PluginSourceSpec {
        PluginSourceSpec {
            source: source.into(),
            branch: None,
            git_ref: git_ref.map(str::to_string),
            sub_dir: sub_dir.map(str::to_string),
            plugins: PluginsField::List(vec!["pstack".into()]),
        }
    }

    #[test]
    fn reads_marketplace_name_and_plugin_sources() {
        let root = temp_dir("kasetto-mp-read");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(root.join(".claude-plugin/marketplace.json"), PSTACK).unwrap();
        let mp = read_marketplace(&root).unwrap();
        assert_eq!(mp.name, "cursor-plugins");
        let names: Vec<_> = mp.plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["origin-apps", "pstack", "remote"]);
        assert_eq!(
            mp.plugins[1].relative_dir.as_deref(),
            Some("harness/pstack/claude-code")
        );
        assert!(mp.plugins[2].relative_dir.is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_missing_or_invalid_marketplace() {
        let root = temp_dir("kasetto-mp-bad");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        assert!(read_marketplace(&root).is_err());
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{"name": "../evil", "plugins": []}"#,
        )
        .unwrap();
        assert!(read_marketplace(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_plugin_name_is_skipped_not_fatal() {
        let root = temp_dir("kasetto-mp-bad-sibling");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin/marketplace.json"),
            r#"{"name": "acme", "plugins": [{"name": "good"}, {"name": "Origin Apps"}]}"#,
        )
        .unwrap();
        let mp = read_marketplace(&root).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].name, "good");
        assert_eq!(mp.invalid, vec!["Origin Apps".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plugin_hash_tracks_the_relative_plugin_tree() {
        let root = temp_dir("kasetto-mp-hash");
        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::create_dir_all(root.join("harness/pstack/claude-code")).unwrap();
        fs::write(root.join(".claude-plugin/marketplace.json"), PSTACK).unwrap();
        let file = root.join("harness/pstack/claude-code/x.md");
        fs::write(&file, "one").unwrap();
        let mp = read_marketplace(&root).unwrap();
        let before = mp.plugins[1].hash(&root).unwrap();
        fs::write(&file, "two").unwrap();
        assert_ne!(before, mp.plugins[1].hash(&root).unwrap());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn github_source_uses_repo_and_config_ref() {
        let s = marketplace_source(
            &spec("https://github.com/cursor/plugins", Some("v1"), None),
            Path::new("/cfg"),
        )
        .unwrap();
        assert_eq!(
            s,
            serde_json::json!({"source": "github", "repo": "cursor/plugins", "ref": "v1"})
        );
    }

    #[test]
    fn other_hosts_use_git_url_and_sub_dir_sets_path() {
        let s = marketplace_source(
            &spec("https://gitlab.com/g/p", None, Some("market/")),
            Path::new("/cfg"),
        )
        .unwrap();
        assert_eq!(
            s,
            serde_json::json!({
                "source": "git",
                "url": "https://gitlab.com/g/p",
                "path": "market/.claude-plugin/marketplace.json"
            })
        );
    }

    #[test]
    fn local_source_is_an_absolute_directory() {
        let s = marketplace_source(&spec("./mp", None, None), Path::new("/cfg")).unwrap();
        assert_eq!(s["source"], "directory");
        // Built with the platform separator, which Windows writes as `\cfg\mp`
        let want = Path::new("/cfg").join("mp");
        assert_eq!(s["path"], want.to_string_lossy().as_ref());
    }

    #[test]
    fn write_then_remove_touches_only_kasetto_keys() {
        let dir = temp_dir("kasetto-mp-settings");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            r#"{"model": "sonnet", "enabledPlugins": {"mine@x": true}}"#,
        )
        .unwrap();
        let source = serde_json::json!({"source": "github", "repo": "cursor/plugins"});
        let id = plugin_id("pstack", "cursor-plugins");

        assert_eq!(declared_source_hash(&path, "cursor-plugins").unwrap(), None);
        write_plugins(
            &path,
            "cursor-plugins",
            &source,
            true,
            std::slice::from_ref(&id),
        )
        .unwrap();
        assert!(plugin_enabled(&path, &id));
        assert_eq!(
            declared_source_hash(&path, "cursor-plugins").unwrap(),
            Some(source_hash(&source))
        );

        remove_plugin(&path, &id).unwrap();
        assert!(remove_marketplace(&path, "cursor-plugins", &source_hash(&source)).unwrap());
        let left: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            left,
            serde_json::json!({"model": "sonnet", "enabledPlugins": {"mine@x": true}})
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_marketplace_with_another_source_is_never_removed() {
        let dir = temp_dir("kasetto-mp-foreign");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let theirs = serde_json::json!({"source": "github", "repo": "fork/plugins"});
        write_plugins(&path, "cursor-plugins", &theirs, true, &[]).unwrap();
        let ours = serde_json::json!({"source": "github", "repo": "cursor/plugins"});
        assert_ne!(
            declared_source_hash(&path, "cursor-plugins").unwrap(),
            Some(source_hash(&ours))
        );
        assert!(!remove_marketplace(&path, "cursor-plugins", &source_hash(&ours)).unwrap());
        assert!(fs::read_to_string(&path).unwrap().contains("fork/plugins"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn declaring_keeps_fields_the_tool_added() {
        let dir = temp_dir("kasetto-mp-keep");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let source = serde_json::json!({"source": "github", "repo": "cursor/plugins"});
        fs::write(
            &path,
            r#"{"extraKnownMarketplaces": {"cursor-plugins": {"source": {"source": "github", "repo": "cursor/plugins"}, "autoUpdate": true}}}"#,
        )
        .unwrap();
        write_plugins(&path, "cursor-plugins", &source, true, &[]).unwrap();
        let v: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v[MARKETPLACES]["cursor-plugins"]["autoUpdate"], true);
        let _ = fs::remove_dir_all(&dir);
    }
}
