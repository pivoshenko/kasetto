mod copy;
mod dirs;
mod hash;
mod http;
mod settings;

pub(crate) use copy::copy_dir;
pub(crate) use dirs::{dirs_home, dirs_kasetto_cache, dirs_kasetto_config, dirs_kasetto_data};
pub(crate) use hash::{hash_dir, hash_file, hash_str};
pub(crate) use http::http_client;
pub(crate) use settings::SettingsFile;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{err, Result};
use crate::model::extend::{extract_extends, merge_yaml};
use crate::model::{Config, Scope, SkillTarget, SkillsField};
use crate::source::{
    auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint, rewrite_browse_to_raw_url,
};

const MAX_EXTENDS_DEPTH: u8 = 8;

/// Where a config came from — used to resolve relative `extends` paths and
/// to detect cycles.
struct ConfigOrigin {
    /// Canonical identifier (absolute path or full URL) for cycle detection.
    canonical_id: String,
    /// Directory used to resolve relative `extends` references appearing in
    /// this config. `None` for HTTP origins (relative extends are an error).
    base_dir: Option<PathBuf>,
    /// Human-readable label for error messages.
    label: String,
}

pub(crate) fn load_config_any(config_path: &str) -> Result<(Config, PathBuf, String)> {
    let mut visited = HashSet::new();
    let (merged, origin) = load_config_recursive(config_path, None, &mut visited, 0)?;
    let cfg: Config = serde_yaml::from_value(merged)
        .map_err(|e| err(format!("failed to parse config {}: {e}", origin.label)))?;
    let cfg_dir = match origin.base_dir {
        Some(dir) => dir,
        None => std::env::current_dir()
            .map_err(|e| err(format!("failed to get current directory: {e}")))?,
    };
    Ok((cfg, cfg_dir, origin.label))
}

fn load_config_recursive(
    config_ref: &str,
    parent_base_dir: Option<&Path>,
    visited: &mut HashSet<String>,
    depth: u8,
) -> Result<(serde_yaml::Value, ConfigOrigin)> {
    if depth > MAX_EXTENDS_DEPTH {
        return Err(err(format!(
            "extends depth limit exceeded ({MAX_EXTENDS_DEPTH}) at {config_ref}"
        )));
    }

    let (text, origin) = fetch_config_text(config_ref, parent_base_dir)?;
    if !visited.insert(origin.canonical_id.clone()) {
        return Err(err(format!(
            "circular extends detected involving {}",
            origin.label
        )));
    }

    let mut value: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|e| err(format!("failed to parse config {}: {e}", origin.label)))?;
    let parents = extract_extends(&mut value);

    let mut merged: serde_yaml::Value = serde_yaml::Value::Mapping(Default::default());
    for parent_ref in &parents {
        let mut parent_visited = visited.clone();
        let (parent_value, _parent_origin) = load_config_recursive(
            parent_ref,
            origin.base_dir.as_deref(),
            &mut parent_visited,
            depth + 1,
        )
        .map_err(|e| {
            err(format!(
                "failed to load extended config '{parent_ref}' (extended from {}): {e}",
                origin.label
            ))
        })?;
        merged = merge_yaml(merged, parent_value);
    }
    let final_value = merge_yaml(merged, value);

    visited.remove(&origin.canonical_id);
    Ok((final_value, origin))
}

fn fetch_config_text(
    config_ref: &str,
    parent_base_dir: Option<&Path>,
) -> Result<(String, ConfigOrigin)> {
    if config_ref.starts_with("http://") || config_ref.starts_with("https://") {
        let fetch_url = match rewrite_browse_to_raw_url(config_ref) {
            Some(rewritten) => rewritten,
            None => config_ref.to_string(),
        };
        let auth = auth_for_request_url(&fetch_url);
        let request = auth.apply(http_client()?.get(&fetch_url));
        let response = request
            .send()
            .map_err(|e| err(format!("failed to fetch remote config: {config_ref}: {e}")))?;
        let status = response.status().as_u16();
        let text = response.text().map_err(|e| {
            err(format!(
                "failed to read remote config body for {config_ref}: {e}"
            ))
        })?;
        if !(200..300).contains(&status) {
            return Err(err(format!(
                "remote config returned HTTP {status} for {config_ref}{}",
                http_fetch_auth_hint(config_ref, status)
            )));
        }
        if text.trim_start().starts_with("<!DOCTYPE") || text.trim_start().starts_with("<html") {
            return Err(err(format!(
                "remote config at {config_ref} returned a login/HTML page instead of YAML - {}",
                auth_env_inline_help(config_ref)
            )));
        }
        return Ok((
            text,
            ConfigOrigin {
                canonical_id: fetch_url.clone(),
                base_dir: None,
                label: config_ref.to_string(),
            },
        ));
    }

    let path = PathBuf::from(config_ref);
    let resolved = if path.is_absolute() {
        path
    } else if let Some(base) = parent_base_dir {
        base.join(path)
    } else {
        path
    };
    let cfg_abs = fs::canonicalize(&resolved).map_err(|_| {
        err(format!(
            "config not found: {} (resolved to {})",
            config_ref,
            resolved.display()
        ))
    })?;
    let cfg_text = fs::read_to_string(&cfg_abs)?;
    let cfg_dir = cfg_abs
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| err("invalid config path"))?;
    let label = cfg_abs.to_string_lossy().to_string();
    Ok((
        cfg_text,
        ConfigOrigin {
            canonical_id: label.clone(),
            base_dir: Some(cfg_dir),
            label,
        },
    ))
}

pub(crate) type TargetSelection = (Vec<(String, PathBuf)>, Vec<BrokenSkill>);

pub(crate) fn select_targets(
    sf: &SkillsField,
    available: &HashMap<String, PathBuf>,
    source_root: &Path,
) -> Result<TargetSelection> {
    let mut out = Vec::new();
    let mut broken = Vec::new();
    match sf {
        SkillsField::Wildcard(s) if s == "*" => {
            for (k, v) in available {
                out.push((k.clone(), v.clone()));
            }
        }
        SkillsField::List(items) => {
            for it in items {
                match it {
                    SkillTarget::Name(name) => {
                        if let Some(p) = available.get(name) {
                            out.push((name.clone(), p.clone()));
                        } else {
                            broken.push(BrokenSkill {
                                name: name.clone(),
                                reason: format!("skill not found: {name}"),
                            });
                        }
                    }
                    SkillTarget::Obj { name, path } => {
                        if let Some(path) = path {
                            let base = PathBuf::from(path);
                            let base = if base.is_absolute() {
                                base
                            } else {
                                source_root.join(base)
                            };
                            let d = base.join(name);
                            if d.join("SKILL.md").exists() {
                                out.push((name.clone(), d));
                                continue;
                            }
                            broken.push(BrokenSkill {
                                name: name.clone(),
                                reason: format!(
                                    "skill not found at `{}`",
                                    base.join(name).display()
                                ),
                            });
                            continue;
                        }
                        if let Some(p) = available.get(name) {
                            out.push((name.clone(), p.clone()));
                        } else {
                            broken.push(BrokenSkill {
                                name: name.clone(),
                                reason: format!("skill not found: {name}"),
                            });
                        }
                    }
                }
            }
        }
        _ => return Err(err("invalid skills field")),
    }
    Ok((out, broken))
}

#[derive(Debug)]
pub(crate) struct BrokenSkill {
    pub name: String,
    pub reason: String,
}

pub(crate) fn resolve_path(base: &Path, raw: &str) -> PathBuf {
    let p = if raw.contains('~') {
        PathBuf::from(
            raw.replace(
                '~',
                &dirs_home()
                    .unwrap_or_else(|_| PathBuf::from("~"))
                    .to_string_lossy(),
            ),
        )
    } else {
        PathBuf::from(raw)
    };
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

/// Returns one skills path per configured agent, respecting scope.
/// Falls back to explicit `destination` if set.
pub(crate) fn resolve_destinations(
    base: &Path,
    cfg: &Config,
    scope: Scope,
) -> Result<Vec<PathBuf>> {
    if let Some(destination) = cfg.destination.as_deref() {
        return Ok(vec![resolve_path(base, destination)]);
    }
    let agents = cfg.agents();
    if agents.is_empty() {
        return Err(err(
            "config must define either destination or a supported agent preset",
        ));
    }
    match scope {
        Scope::Project => Ok(agents.iter().map(|a| a.project_path(base)).collect()),
        Scope::Global => {
            let home = dirs_home()?;
            Ok(agents.iter().map(|a| a.global_path(&home)).collect())
        }
    }
}

/// Returns one MCP settings path per configured agent, respecting scope.
pub(crate) fn resolve_mcp_settings_targets(
    cfg: &Config,
    scope: Scope,
    project_root: &Path,
) -> Result<Vec<crate::model::McpSettingsTarget>> {
    let agents = cfg.agents();
    if agents.is_empty() {
        return Ok(vec![]);
    }
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    match scope {
        Scope::Project => {
            for a in agents {
                let t = a.mcp_project_target(project_root);
                if seen.insert(t.path.clone()) {
                    out.push(t);
                }
            }
        }
        Scope::Global => {
            let home = dirs_home()?;
            let kasetto_config = dirs_kasetto_config()?;
            for a in agents {
                let t = a.mcp_settings_target(&home, &kasetto_config);
                if seen.insert(t.path.clone()) {
                    out.push(t);
                }
            }
        }
    }
    Ok(out)
}

/// Returns one commands directory per configured agent (filtering unsupported), deduped.
pub(crate) fn resolve_command_targets(
    cfg: &Config,
    scope: Scope,
    project_root: &Path,
) -> Result<Vec<crate::model::CommandTarget>> {
    let agents = cfg.agents();
    if agents.is_empty() {
        return Ok(vec![]);
    }
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    match scope {
        Scope::Project => {
            for a in agents {
                if let Some(t) = a.commands_project_path(project_root) {
                    if seen.insert(t.path.clone()) {
                        out.push(t);
                    }
                }
            }
        }
        Scope::Global => {
            let home = dirs_home()?;
            for a in agents {
                if let Some(t) = a.commands_global_path(&home) {
                    if seen.insert(t.path.clone()) {
                        out.push(t);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Root that lock-file `destination` paths are stored relative to, so the
/// committed lock stays portable across machines and users.
/// Project scope → the project root; Global scope → the user's home directory.
pub(crate) fn scope_root(scope: Scope, project_root: &Path) -> Result<PathBuf> {
    match scope {
        Scope::Project => Ok(project_root.to_path_buf()),
        Scope::Global => dirs_home(),
    }
}

/// Make an absolute install path portable by storing it relative to `root`.
/// Paths outside `root` (e.g. a custom absolute `destination`) are kept as-is.
pub(crate) fn relativize_dest(abs: &Path, root: &Path) -> String {
    match abs.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().to_string(),
        Err(_) => abs.to_string_lossy().to_string(),
    }
}

/// Inverse of [`relativize_dest`]: resolve a stored `destination` back to an
/// absolute path. Already-absolute values (legacy locks, out-of-root paths)
/// are returned unchanged.
pub(crate) fn resolve_dest(stored: &str, root: &Path) -> PathBuf {
    let p = PathBuf::from(stored);
    if p.is_absolute() {
        p
    } else {
        root.join(p)
    }
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(crate) fn now_iso() -> String {
    format!("{}", now_unix())
}

#[cfg(test)]
pub(crate) fn temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Agent, AgentField, Config, GitPin, McpEntry, McpsField, SkillTarget, SkillsField,
    };
    use std::path::Path;

    #[test]
    fn select_targets_reports_missing_skill() {
        let mut available = HashMap::new();
        available.insert("present".to_string(), PathBuf::from("/tmp/present"));
        let sf = SkillsField::List(vec![
            SkillTarget::Name("present".to_string()),
            SkillTarget::Name("missing".to_string()),
        ]);

        let (targets, broken) = select_targets(&sf, &available, Path::new("/tmp")).expect("select");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "present");
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].name, "missing");
        assert!(broken[0].reason.contains("skill not found"));
    }

    #[test]
    fn select_targets_prefers_explicit_path_override() {
        let root = temp_dir("kasetto-targets");
        let nested = root.join("skills-repo");
        let skill_dir = nested.join("custom-skill");
        fs::create_dir_all(&skill_dir).expect("create dirs");
        fs::write(skill_dir.join("SKILL.md"), "# Custom\n\nDesc\n").expect("write skill");

        let mut available = HashMap::new();
        available.insert(
            "custom-skill".to_string(),
            PathBuf::from("/tmp/wrong-location"),
        );
        let sf = SkillsField::List(vec![SkillTarget::Obj {
            name: "custom-skill".to_string(),
            path: Some(nested.to_string_lossy().to_string()),
        }]);

        let (targets, broken) = select_targets(&sf, &available, Path::new("/tmp")).expect("select");
        assert!(broken.is_empty());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "custom-skill");
        assert_eq!(targets[0].1, skill_dir);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn select_targets_resolves_relative_path_against_source_root() {
        let root = temp_dir("kasetto-targets-rel");
        let skill_dir = root.join("skills/productivity/grill-me");
        fs::create_dir_all(&skill_dir).expect("create dirs");
        fs::write(skill_dir.join("SKILL.md"), "# Grill\n\nDesc\n").expect("write skill");

        let available = HashMap::new();
        let sf = SkillsField::List(vec![SkillTarget::Obj {
            name: "grill-me".to_string(),
            path: Some("skills/productivity".to_string()),
        }]);

        let (targets, broken) = select_targets(&sf, &available, &root).expect("select");
        assert!(broken.is_empty());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "grill-me");
        assert_eq!(targets[0].1, skill_dir);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn agent_paths_cover_supported_presets() {
        let home = Path::new("/tmp/kasetto-home");

        assert_eq!(Agent::Codex.global_path(home), home.join(".codex/skills"));
        assert_eq!(
            Agent::Amp.global_path(home),
            home.join(".config/agents/skills")
        );
        assert_eq!(
            Agent::Windsurf.global_path(home),
            home.join(".codeium/windsurf/skills")
        );
        assert_eq!(Agent::Trae.global_path(home), home.join(".trae/skills"));
    }

    #[test]
    fn config_agent_parses_hyphenated_names() {
        let hyphenated: Config =
            serde_yaml::from_str("agent: kiro-cli\nskills: []\n").expect("parse config");
        assert_eq!(hyphenated.agent, Some(AgentField::One(Agent::KiroCli)));

        let claude_code: Config =
            serde_yaml::from_str("agent: claude-code\nskills: []\n").expect("parse config");
        assert_eq!(claude_code.agent, Some(AgentField::One(Agent::ClaudeCode)));
    }

    #[test]
    fn config_agent_parses_multi_agent_list() {
        let multi: Config =
            serde_yaml::from_str("agent:\n  - claude-code\n  - cursor\nskills: []\n")
                .expect("parse config");
        assert_eq!(
            multi.agent,
            Some(AgentField::Many(vec![Agent::ClaudeCode, Agent::Cursor]))
        );
        assert_eq!(multi.agents(), vec![Agent::ClaudeCode, Agent::Cursor]);
    }

    #[test]
    fn settings_file_load_creates_empty_for_missing_file() {
        let dir = temp_dir("kasetto-sf-missing");
        let path = dir.join("nonexistent.json");
        let sf = SettingsFile::load(&path).expect("load");
        assert_eq!(sf.data, serde_json::json!({}));
    }

    #[test]
    fn settings_file_load_parses_existing_json() {
        let dir = temp_dir("kasetto-sf-parse");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"mcpServers":{}}"#).unwrap();

        let sf = SettingsFile::load(&path).expect("load");
        assert!(sf.data["mcpServers"].is_object());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_file_save_creates_parent_dirs() {
        let dir = temp_dir("kasetto-sf-save");
        let nested = dir.join("deep").join("path").join("settings.json");

        let mut sf = SettingsFile::load(&nested).expect("load");
        sf.data["key"] = serde_json::json!("value");
        sf.save().expect("save");

        let text = fs::read_to_string(&nested).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["key"], "value");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_file_load_rejects_invalid_json() {
        let dir = temp_dir("kasetto-sf-invalid");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        fs::write(&path, "not valid json {{{").unwrap();

        let result = SettingsFile::load(&path);
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_parses_ref_field() {
        let yaml = r#"
agent: cursor
skills:
  - source: https://github.com/example/pack
    ref: v2.0
    skills: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.skills[0].git_ref.as_deref(), Some("v2.0"));
        assert!(cfg.skills[0].branch.is_none());
    }

    #[test]
    fn config_parses_branch_field() {
        let yaml = r#"
agent: cursor
skills:
  - source: https://github.com/example/pack
    branch: develop
    skills: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.skills[0].branch.as_deref(), Some("develop"));
        assert!(cfg.skills[0].git_ref.is_none());
    }

    #[test]
    fn config_ref_and_branch_both_parse() {
        let yaml = r#"
agent: cursor
skills:
  - source: https://github.com/example/pack
    ref: v3.0
    branch: develop
    skills: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.skills[0].git_ref.as_deref(), Some("v3.0"));
        assert_eq!(cfg.skills[0].branch.as_deref(), Some("develop"));
        assert!(
            matches!(cfg.skills[0].git_pin(), GitPin::Ref(r) if r == "v3.0"),
            "ref should win when both ref and branch are set"
        );
    }

    #[test]
    fn config_parses_sub_dir_field() {
        let yaml = r#"
agent: cursor
skills:
  - source: https://github.com/example/pack
    sub-dir: plugins/swift-apple-expert
    skills: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(
            cfg.skills[0].sub_dir.as_deref(),
            Some("plugins/swift-apple-expert")
        );
    }

    #[test]
    fn config_parses_sub_dir_alias() {
        let yaml = r#"
agent: cursor
skills:
  - source: https://github.com/example/pack
    sub_dir: plugins/swift-apple-expert
    skills: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(
            cfg.skills[0].sub_dir.as_deref(),
            Some("plugins/swift-apple-expert")
        );
    }

    #[test]
    fn git_pin_priority_ref_over_branch() {
        let spec = crate::model::SourceSpec {
            source: "https://github.com/x/y".into(),
            branch: Some("dev".into()),
            git_ref: Some("v1.0".into()),
            sub_dir: None,
            skills: SkillsField::Wildcard("*".into()),
        };
        assert!(
            matches!(spec.git_pin(), GitPin::Ref(r) if r == "v1.0"),
            "ref should take priority over branch"
        );
    }

    #[test]
    fn git_pin_branch_when_no_ref() {
        let spec = crate::model::SourceSpec {
            source: "https://github.com/x/y".into(),
            branch: Some("dev".into()),
            git_ref: None,
            sub_dir: None,
            skills: SkillsField::Wildcard("*".into()),
        };
        assert!(
            matches!(spec.git_pin(), GitPin::Branch(b) if b == "dev"),
            "expected branch pin"
        );
    }

    #[test]
    fn git_pin_default_when_neither() {
        let spec = crate::model::SourceSpec {
            source: "https://github.com/x/y".into(),
            branch: None,
            git_ref: None,
            sub_dir: None,
            skills: SkillsField::Wildcard("*".into()),
        };
        assert!(matches!(spec.git_pin(), GitPin::Default));
    }

    #[test]
    fn config_mcps_parses_ref_field() {
        let yaml = r#"
agent: cursor
skills: []
mcps:
  - source: https://github.com/example/mcp-pack
    ref: v1.5
    mcps: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.mcps[0].git_ref.as_deref(), Some("v1.5"));
        let as_spec = cfg.mcps[0].as_source_spec();
        assert_eq!(as_spec.git_ref.as_deref(), Some("v1.5"));
    }

    #[test]
    fn config_mcps_parses_wildcard() {
        let yaml = r#"
agent: cursor
skills: []
mcps:
  - source: https://github.com/acme/mcp-pack
    mcps: "*"
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert!(matches!(cfg.mcps[0].mcps, McpsField::Wildcard(_)));
    }

    #[test]
    fn config_mcps_parses_plain_strings() {
        let yaml = r#"
agent: cursor
skills: []
mcps:
  - source: https://github.com/acme/monorepo
    ref: v1.4.0
    mcps:
      - github
      - linear
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        let McpsField::List(ref entries) = cfg.mcps[0].mcps else {
            panic!("expected List");
        };
        assert_eq!(entries.len(), 2);
        assert!(matches!(&entries[0], McpEntry::Name(n) if n == "github"));
        assert!(matches!(&entries[1], McpEntry::Name(n) if n == "linear"));
    }

    #[test]
    fn config_mcps_parses_objects() {
        let yaml = r#"
agent: cursor
skills: []
mcps:
  - source: https://github.com/acme/monorepo
    mcps:
      - name: my-server
        path: tools
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        let McpsField::List(ref entries) = cfg.mcps[0].mcps else {
            panic!("expected List");
        };
        assert_eq!(entries.len(), 1);
        assert!(
            matches!(&entries[0], McpEntry::Obj { name, path: Some(p) } if name == "my-server" && p == "tools")
        );
    }

    #[test]
    fn load_config_any_resolves_extends_relative_to_parent() {
        let root = temp_dir("kasetto-extends-rel");
        fs::create_dir_all(&root).unwrap();
        let base = root.join("base.yaml");
        let child = root.join("child.yaml");
        fs::write(
            &base,
            "agent: cursor\nscope: global\nskills:\n  - source: https://x/a\n    ref: v1\n    skills: \"*\"\n",
        )
        .unwrap();
        fs::write(
            &child,
            "extends: ./base.yaml\nscope: project\nskills:\n  - source: https://x/b\n    skills: \"*\"\n",
        )
        .unwrap();

        let (cfg, _, _) = load_config_any(child.to_str().unwrap()).expect("load");
        assert_eq!(cfg.scope, Some(crate::model::Scope::Project));
        assert_eq!(cfg.skills.len(), 2);
        assert!(cfg
            .skills
            .iter()
            .any(|s| s.source == "https://x/a" && s.git_ref.as_deref() == Some("v1")));
        assert!(cfg.skills.iter().any(|s| s.source == "https://x/b"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_config_any_chains_extends() {
        let root = temp_dir("kasetto-extends-chain");
        fs::create_dir_all(&root).unwrap();
        let a = root.join("a.yaml");
        let b = root.join("b.yaml");
        let c = root.join("c.yaml");
        fs::write(&a, "agent: cursor\nscope: global\nskills: []\n").unwrap();
        fs::write(&b, "extends: ./a.yaml\nskills: []\n").unwrap();
        fs::write(&c, "extends: ./b.yaml\nscope: project\nskills: []\n").unwrap();

        let (cfg, _, _) = load_config_any(c.to_str().unwrap()).expect("load");
        assert_eq!(cfg.scope, Some(crate::model::Scope::Project));
        assert_eq!(cfg.agents().len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_config_any_detects_cycles() {
        let root = temp_dir("kasetto-extends-cycle");
        fs::create_dir_all(&root).unwrap();
        let a = root.join("a.yaml");
        let b = root.join("b.yaml");
        fs::write(&a, "extends: ./b.yaml\nskills: []\n").unwrap();
        fs::write(&b, "extends: ./a.yaml\nskills: []\n").unwrap();

        let result = load_config_any(a.to_str().unwrap());
        assert!(result.is_err(), "expected cycle error");
        let msg = format!("{}", result.err().unwrap());
        assert!(msg.contains("circular"), "got: {msg}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_config_any_overrides_same_identity_in_extends() {
        let root = temp_dir("kasetto-extends-override");
        fs::create_dir_all(&root).unwrap();
        let base = root.join("base.yaml");
        let child = root.join("child.yaml");
        fs::write(
            &base,
            "agent: cursor\nskills:\n  - source: https://x/a\n    ref: v1\n    skills: \"*\"\n",
        )
        .unwrap();
        fs::write(
            &child,
            "extends: ./base.yaml\nskills:\n  - source: https://x/a\n    ref: v1\n    skills:\n      - one\n",
        )
        .unwrap();

        let (cfg, _, _) = load_config_any(child.to_str().unwrap()).expect("load");
        assert_eq!(cfg.skills.len(), 1);
        assert!(matches!(&cfg.skills[0].skills, SkillsField::List(items) if items.len() == 1));

        let _ = fs::remove_dir_all(&root);
    }
}
