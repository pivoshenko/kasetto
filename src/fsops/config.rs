use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::model::extend::{extract_extends, merge_yaml};
use crate::model::Config;
use crate::source::{
    auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint, rewrite_browse_to_raw_url,
};

use super::http::http_client;

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
        let fetch_url =
            rewrite_browse_to_raw_url(config_ref).unwrap_or_else(|| config_ref.to_string());
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
                canonical_id: fetch_url,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::model::SkillsField;

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
