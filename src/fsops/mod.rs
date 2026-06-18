mod config;
mod config_edit;
mod copy;
mod dirs;
mod hash;
mod http;
mod settings;

pub(crate) use config::load_config_any;
pub(crate) use config_edit::{
    insert_item, item_exists, remove_item, remove_names, Pin, RemoveOutcome, Section, Selector,
    SourceItem,
};
pub(crate) use copy::copy_dir;
pub(crate) use dirs::{dirs_home, dirs_kasetto_cache, dirs_kasetto_config, dirs_kasetto_data};
pub(crate) use hash::{hash_dir, hash_file, hash_str};
pub(crate) use http::http_client;
pub(crate) use settings::SettingsFile;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{err, Result};
use crate::model::{Config, Scope, SkillTarget, SkillsField};

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
            // HashMap iteration order is random; sort so install order, labels,
            // and --json output are stable across runs.
            out.sort_by(|a, b| a.0.cmp(&b.0));
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
    // Expand only a leading `~` (home prefix); a `~` elsewhere in the path is
    // an ordinary character (e.g. `./backup~old`) and must not be rewritten.
    let p = match raw
        .strip_prefix("~/")
        .or(if raw == "~" { Some("") } else { None })
    {
        Some(rest) => match dirs_home() {
            Ok(home) => home.join(rest),
            Err(_) => PathBuf::from(raw),
        },
        None => PathBuf::from(raw),
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

/// Returns one rules destination per configured agent (filtering unsupported), deduped.
pub(crate) fn resolve_rule_targets(
    cfg: &Config,
    scope: Scope,
    project_root: &Path,
) -> Result<Vec<crate::model::RuleTarget>> {
    let agents = cfg.agents();
    if agents.is_empty() {
        return Ok(vec![]);
    }
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    match scope {
        Scope::Project => {
            for a in agents {
                if let Some(t) = a.rules_project_path(project_root) {
                    if seen.insert(t.path.clone()) {
                        out.push(t);
                    }
                }
            }
        }
        Scope::Global => {
            let home = dirs_home()?;
            for a in agents {
                if let Some(t) = a.rules_global_path(&home) {
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

pub(crate) fn now_unix_str() -> String {
    now_unix().to_string()
}

/// Unique scratch directory for tests. Parallel test threads can observe the
/// same nanosecond, so a process-wide counter keeps concurrently created dirs
/// distinct; the nanos guard against stale dirs from a crashed earlier run.
#[cfg(test)]
pub(crate) fn temp_dir(prefix: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{seq}-{nonce}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Agent, AgentField, Config, GitPin, McpEntry, McpsField, SkillTarget, SkillsField,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn resolve_path_expands_only_leading_tilde() {
        let base = Path::new("/base");
        let home = dirs_home().expect("home");
        assert_eq!(resolve_path(base, "~/skills"), home.join("skills"));
        assert_eq!(resolve_path(base, "~"), home);
        // A `~` that is not the home prefix is an ordinary path character.
        assert_eq!(
            resolve_path(base, "backup~old/skills"),
            Path::new("/base/backup~old/skills")
        );
    }

    #[test]
    fn select_targets_wildcard_is_sorted() {
        let mut available = HashMap::new();
        for name in ["zeta", "alpha", "mid"] {
            available.insert(name.to_string(), PathBuf::from(format!("/tmp/{name}")));
        }
        let sf = SkillsField::Wildcard("*".into());

        let (targets, _) = select_targets(&sf, &available, Path::new("/tmp")).expect("select");
        let names: Vec<&str> = targets.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["alpha", "mid", "zeta"]);
    }

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
}
