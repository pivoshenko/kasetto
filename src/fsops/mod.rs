mod copy;
mod dirs;
mod hash;
mod http;
mod settings;

pub(crate) use copy::copy_dir;
pub(crate) use dirs::{dirs_home, dirs_kasetto_config, dirs_kasetto_data};
pub(crate) use hash::{hash_dir, hash_file};
pub(crate) use http::http_client;
pub(crate) use settings::SettingsFile;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{err, Result};
use crate::model::{Config, Scope, SkillTarget, SkillsField};
use crate::source::{
    auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint, rewrite_gitlab_raw_url,
};

pub(crate) fn load_config_any(config_path: &str) -> Result<(Config, PathBuf, String)> {
    if config_path.starts_with("http://") || config_path.starts_with("https://") {
        let fetch_url =
            rewrite_gitlab_raw_url(config_path).unwrap_or_else(|| config_path.to_string());
        let auth = auth_for_request_url(config_path);
        let request = auth.apply(http_client()?.get(&fetch_url));
        let response = request
            .send()
            .map_err(|e| err(format!("failed to fetch remote config: {config_path}: {e}")))?;
        let status = response.status().as_u16();
        let text = response.text().map_err(|e| {
            err(format!(
                "failed to read remote config body for {config_path}: {e}"
            ))
        })?;
        if !(200..300).contains(&status) {
            return Err(err(format!(
                "remote config returned HTTP {status} for {config_path}{}",
                http_fetch_auth_hint(config_path, status)
            )));
        }
        if text.trim_start().starts_with("<!DOCTYPE") || text.trim_start().starts_with("<html") {
            return Err(err(format!(
                "remote config at {config_path} returned a login/HTML page instead of YAML — {}",
                auth_env_inline_help(config_path)
            )));
        }
        let cfg: Config = serde_yaml::from_str(&text)?;
        let cfg_dir = std::env::current_dir()
            .map_err(|e| err(format!("failed to get current directory: {e}")))?;
        return Ok((cfg, cfg_dir, config_path.to_string()));
    }

    let cfg_abs = fs::canonicalize(config_path)
        .map_err(|_| err(format!("config not found: {config_path}")))?;
    let cfg_text = fs::read_to_string(&cfg_abs)?;
    let cfg: Config = serde_yaml::from_str(&cfg_text)?;
    let cfg_dir = cfg_abs
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| err("invalid config path"))?;
    Ok((cfg, cfg_dir, cfg_abs.to_string_lossy().to_string()))
}

pub(crate) type TargetSelection = (Vec<(String, PathBuf)>, Vec<BrokenSkill>);

pub(crate) fn select_targets(
    sf: &SkillsField,
    available: &HashMap<String, PathBuf>,
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
                            let d = PathBuf::from(path).join(name);
                            if d.join("SKILL.md").exists() {
                                out.push((name.clone(), d));
                                continue;
                            }
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

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
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
    use crate::model::{Agent, AgentField, Config, GitPin, SkillTarget, SkillsField};
    use std::path::Path;

    #[test]
    fn select_targets_reports_missing_skill() {
        let mut available = HashMap::new();
        available.insert("present".to_string(), PathBuf::from("/tmp/present"));
        let sf = SkillsField::List(vec![
            SkillTarget::Name("present".to_string()),
            SkillTarget::Name("missing".to_string()),
        ]);

        let (targets, broken) = select_targets(&sf, &available).expect("select");
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

        let (targets, broken) = select_targets(&sf, &available).expect("select");
        assert!(broken.is_empty());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "custom-skill");
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
        match cfg.skills[0].git_pin() {
            GitPin::Ref(r) => assert_eq!(r, "v3.0"),
            _ => panic!("expected GitPin::Ref when both ref and branch are set"),
        }
    }

    #[test]
    fn git_pin_priority_ref_over_branch() {
        let spec = crate::model::SourceSpec {
            source: "https://github.com/x/y".into(),
            branch: Some("dev".into()),
            git_ref: Some("v1.0".into()),
            skills: SkillsField::Wildcard("*".into()),
        };
        match spec.git_pin() {
            GitPin::Ref(r) => assert_eq!(r, "v1.0"),
            _ => panic!("ref should take priority over branch"),
        }
    }

    #[test]
    fn git_pin_branch_when_no_ref() {
        let spec = crate::model::SourceSpec {
            source: "https://github.com/x/y".into(),
            branch: Some("dev".into()),
            git_ref: None,
            skills: SkillsField::Wildcard("*".into()),
        };
        match spec.git_pin() {
            GitPin::Branch(b) => assert_eq!(b, "dev"),
            _ => panic!("expected branch pin"),
        }
    }

    #[test]
    fn git_pin_default_when_neither() {
        let spec = crate::model::SourceSpec {
            source: "https://github.com/x/y".into(),
            branch: None,
            git_ref: None,
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
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.mcps[0].git_ref.as_deref(), Some("v1.5"));
        let as_spec = cfg.mcps[0].as_source_spec();
        assert_eq!(as_spec.git_ref.as_deref(), Some("v1.5"));
    }

    #[test]
    fn config_mcps_parses_path_field() {
        let yaml = r#"
agent: cursor
skills: []
mcps:
  - source: https://github.com/shepsci/kaggle-skill
    path: .mcp.json
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(cfg.mcps[0].path.as_deref(), Some(".mcp.json"));
    }

    #[test]
    fn config_mcps_path_defaults_to_none() {
        let yaml = r#"
agent: cursor
skills: []
mcps:
  - source: https://github.com/example/mcp-pack
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("parse");
        assert!(cfg.mcps[0].path.is_none());
    }
}
