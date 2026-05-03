//! Skill pack sources: local paths, remote archives, discovery.

mod auth;
mod hosts;
mod parse;
mod remote;

pub(crate) use auth::{auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint};
pub(crate) use remote::rewrite_browse_to_raw_url;

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::resolve_path;
use crate::model::{GitPin, SourceSpec};

fn repo_name_hint(parsed: &parse::RepoUrl) -> String {
    match parsed {
        parse::RepoUrl::GitHub { repo, .. } => repo.clone(),
        parse::RepoUrl::GitLab { project_path, .. } => project_path
            .split('/')
            .next_back()
            .unwrap_or(project_path)
            .to_string(),
        parse::RepoUrl::Bitbucket { repo_slug, .. } => repo_slug.clone(),
        parse::RepoUrl::Gitea { repo, .. } => repo.clone(),
    }
}

fn resolve_source_root(base_root: &Path, sub_dir: Option<&str>) -> Result<PathBuf> {
    let Some(sub_dir) = sub_dir else {
        return Ok(base_root.to_path_buf());
    };

    let trimmed = sub_dir.trim();
    if trimmed.is_empty() {
        return Err(err("skills source `sub-dir` cannot be empty"));
    }

    let rel = Path::new(trimmed);
    if rel.is_absolute() {
        return Err(err("skills source `sub-dir` must be relative"));
    }
    if rel
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
    {
        return Err(err(
            "skills source `sub-dir` must not escape the source root",
        ));
    }

    let resolved = base_root.join(rel);
    if !resolved.exists() {
        return Err(err(format!(
            "skills source sub-dir not found: {}",
            resolved.display()
        )));
    }
    if !resolved.is_dir() {
        return Err(err(format!(
            "skills source sub-dir is not a directory: {}",
            resolved.display()
        )));
    }
    Ok(resolved)
}

pub(crate) fn materialize_source(
    src: &SourceSpec,
    cfg_dir: &Path,
    stage: &Path,
) -> Result<MaterializedSource> {
    if src.source.contains("://") {
        let parsed = parse::parse_repo_url(&src.source)?;
        let pin = src.git_pin();

        let source_revision = match &pin {
            GitPin::Ref(r) => {
                let (url, auth) = remote::remote_repo_archive_ref(&parsed, r);
                remote::download_extract(&url, &auth, stage, &src.source)?;
                format!("ref:{r}")
            }
            GitPin::Branch(b) => {
                let (url, auth) = remote::remote_repo_archive_branch(&parsed, b);
                remote::download_extract(&url, &auth, stage, &src.source)?;
                format!("branch:{b}")
            }
            GitPin::Default => {
                let (url, auth) = remote::remote_repo_archive_branch(&parsed, "main");
                remote::download_extract(&url, &auth, stage, &src.source).or_else(|_| {
                    let (url, auth) = remote::remote_repo_archive_branch(&parsed, "master");
                    remote::download_extract(&url, &auth, stage, &src.source).map_err(|e2| {
                        err(format!("{e2} (also tried branch `master` after `main`)"))
                    })
                })?;
                "branch:main".into()
            }
        };

        let source_root = resolve_source_root(stage, src.sub_dir.as_deref())?;
        let hint = src
            .sub_dir
            .as_deref()
            .and_then(|sub_dir| Path::new(sub_dir).file_name())
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(|name| name.to_string())
            .unwrap_or_else(|| repo_name_hint(&parsed));
        let available = discover_with_root_name(&source_root, Some(hint.as_str()))?;
        Ok(MaterializedSource {
            source_revision,
            available,
            cleanup_dir: Some(stage.to_path_buf()),
        })
    } else {
        let root = resolve_path(cfg_dir, &src.source);
        let source_root = resolve_source_root(&root, src.sub_dir.as_deref())?;
        let available = discover(&source_root)?;
        Ok(MaterializedSource {
            source_revision: "local".into(),
            available,
            cleanup_dir: None,
        })
    }
}

pub(crate) struct MaterializedSource {
    pub source_revision: String,
    pub available: HashMap<String, PathBuf>,
    pub cleanup_dir: Option<PathBuf>,
}

pub(crate) fn discover(root: &Path) -> Result<HashMap<String, PathBuf>> {
    let root_name = root.file_name().and_then(|name| name.to_str());
    discover_with_root_name(root, root_name)
}

fn discover_with_root_name(
    root: &Path,
    root_name: Option<&str>,
) -> Result<HashMap<String, PathBuf>> {
    let mut out = HashMap::new();
    let root_skill_name = if root.join("SKILL.md").is_file() {
        if let Some(name) = root_name.filter(|name| !name.is_empty()) {
            out.insert(name.to_string(), root.to_path_buf());
            Some(name.to_string())
        } else {
            None
        }
    } else {
        None
    };
    discover_skills_in_subdir(root, &mut out)?;
    discover_skills_in_subdir(&root.join("skills"), &mut out)?;
    if let Some(ref name) = root_skill_name {
        if out.get(name).map(|p| p != root) == Some(true) {
            eprintln!("warning: subdirectory skill `{name}` shadows root-level SKILL.md");
        }
    }
    Ok(out)
}

fn discover_skills_in_subdir(base: &Path, out: &mut HashMap<String, PathBuf>) -> Result<()> {
    if !base.exists() {
        return Ok(());
    }
    for e in fs::read_dir(base)? {
        let e = e?;
        if !e.path().is_dir() {
            continue;
        }
        let d = e.path();
        if d.join("SKILL.md").is_file() {
            out.insert(e.file_name().to_string_lossy().to_string(), d);
        }
    }
    Ok(())
}

pub(crate) fn discover_mcps(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();

    // Check well-known root-level MCP files (.mcp.json is the Claude Code convention).
    for name in [".mcp.json", "mcp.json"] {
        let p = root.join(name);
        if p.is_file() {
            out.push(p);
        }
    }

    // Check the mcp/ subdirectory for additional pack JSON files.
    let mcp_dir = root.join("mcp");
    if mcp_dir.exists() {
        for e in fs::read_dir(mcp_dir)? {
            let e = e?;
            let path = e.path();
            if e.file_type()?.is_file()
                && path.extension().map(|ext| ext == "json").unwrap_or(false)
            {
                out.push(path);
            }
        }
    }

    Ok(out)
}

/// Resolve a single MCP file by explicit path within a repo root.
pub(crate) fn resolve_mcp_path(root: &Path, rel_path: &str) -> Result<Vec<PathBuf>> {
    let target = root.join(rel_path);
    if target.is_file() {
        Ok(vec![target])
    } else {
        Err(err(format!(
            "MCP path not found: {} (resolved to {})",
            rel_path,
            target.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SkillsField, SourceSpec};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn local_materialize_does_not_set_cleanup_dir() {
        let root = temp_dir("kasetto-local-src");
        let skill_dir = root.join("demo-skill");
        fs::create_dir_all(&skill_dir).expect("create dirs");
        fs::write(skill_dir.join("SKILL.md"), "# Demo\n\nDesc\n").expect("write skill");

        let src = SourceSpec {
            source: root.to_string_lossy().to_string(),
            branch: None,
            git_ref: None,
            sub_dir: None,
            skills: SkillsField::Wildcard("*".to_string()),
        };
        let stage = temp_dir("kasetto-stage");
        let materialized =
            materialize_source(&src, Path::new("/"), &stage).expect("materialize local");

        assert!(materialized.cleanup_dir.is_none());
        assert!(materialized.available.contains_key("demo-skill"));
        assert!(root.exists());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&stage);
    }

    #[test]
    fn local_materialize_supports_sub_dir() {
        let root = temp_dir("kasetto-local-subdir-src");
        let nested = root.join("plugins/swift-apple-expert");
        fs::create_dir_all(&nested).expect("create dirs");
        fs::write(nested.join("SKILL.md"), "# Nested\n\nDesc\n").expect("write skill");

        let src = SourceSpec {
            source: root.to_string_lossy().to_string(),
            branch: None,
            git_ref: None,
            sub_dir: Some("plugins/swift-apple-expert".to_string()),
            skills: SkillsField::Wildcard("*".to_string()),
        };

        let stage = temp_dir("kasetto-stage-subdir");
        let materialized =
            materialize_source(&src, Path::new("/"), &stage).expect("materialize local subdir");

        assert!(materialized.available.contains_key("swift-apple-expert"));
        assert_eq!(
            materialized.available.get("swift-apple-expert").unwrap(),
            &nested
        );

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&stage);
    }

    #[test]
    fn discover_supports_root_level_skill_with_hint() {
        let root = temp_dir("kasetto-root-skill");
        fs::create_dir_all(&root).expect("create dirs");
        fs::write(root.join("SKILL.md"), "# Root\n\nDesc\n").expect("write skill");

        let available =
            discover_with_root_name(&root, Some("raycast-script-creator")).expect("discover");
        assert!(available.contains_key("raycast-script-creator"));
        assert_eq!(available.get("raycast-script-creator").unwrap(), &root);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_uses_local_directory_name_for_root_level_skill() {
        let root = temp_dir("kasetto-root-skill-local");
        fs::create_dir_all(&root).expect("create dirs");
        fs::write(root.join("SKILL.md"), "# Root\n\nDesc\n").expect("write skill");

        let available = discover(&root).expect("discover");
        let root_name = root.file_name().unwrap().to_string_lossy().to_string();
        assert!(available.contains_key(&root_name));
        assert_eq!(available.get(&root_name).unwrap(), &root);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_mcps_finds_root_dot_mcp_json() {
        let root = temp_dir("kasetto-mcp-root");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"tool":{"command":"x"}}}"#,
        )
        .unwrap();

        let mcps = discover_mcps(&root).unwrap();
        assert_eq!(mcps.len(), 1);
        assert!(mcps[0].ends_with(".mcp.json"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_mcps_finds_root_mcp_json() {
        let root = temp_dir("kasetto-mcp-root2");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("mcp.json"),
            r#"{"mcpServers":{"tool":{"command":"x"}}}"#,
        )
        .unwrap();

        let mcps = discover_mcps(&root).unwrap();
        assert_eq!(mcps.len(), 1);
        assert!(mcps[0].ends_with("mcp.json"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_mcps_finds_mcp_subdir_and_root() {
        let root = temp_dir("kasetto-mcp-both");
        let mcp_dir = root.join("mcp");
        fs::create_dir_all(&mcp_dir).unwrap();
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"a":{"command":"x"}}}"#,
        )
        .unwrap();
        fs::write(
            mcp_dir.join("extra.json"),
            r#"{"mcpServers":{"b":{"command":"y"}}}"#,
        )
        .unwrap();

        let mcps = discover_mcps(&root).unwrap();
        assert_eq!(mcps.len(), 2);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_mcps_returns_empty_when_nothing() {
        let root = temp_dir("kasetto-mcp-empty");
        fs::create_dir_all(&root).unwrap();

        let mcps = discover_mcps(&root).unwrap();
        assert!(mcps.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_mcp_path_finds_explicit_file() {
        let root = temp_dir("kasetto-mcp-path");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"tool":{"command":"x"}}}"#,
        )
        .unwrap();

        let mcps = resolve_mcp_path(&root, ".mcp.json").unwrap();
        assert_eq!(mcps.len(), 1);
        assert!(mcps[0].ends_with(".mcp.json"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_mcp_path_nested() {
        let root = temp_dir("kasetto-mcp-nested");
        let nested = root.join("configs");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("custom.json"),
            r#"{"mcpServers":{"tool":{"command":"x"}}}"#,
        )
        .unwrap();

        let mcps = resolve_mcp_path(&root, "configs/custom.json").unwrap();
        assert_eq!(mcps.len(), 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_mcp_path_errors_on_missing() {
        let root = temp_dir("kasetto-mcp-missing");
        fs::create_dir_all(&root).unwrap();

        let result = resolve_mcp_path(&root, "nonexistent.json");
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&root);
    }
}
