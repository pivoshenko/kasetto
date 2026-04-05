//! Skill pack sources: local paths, remote archives, discovery.

mod auth;
mod hosts;
mod parse;
mod remote;

pub(crate) use auth::{auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint};
pub(crate) use remote::rewrite_gitlab_raw_url;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::resolve_path;
use crate::model::{GitPin, SourceSpec};

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

        let available = discover(stage)?;
        Ok(MaterializedSource {
            source_revision,
            available,
            cleanup_dir: Some(stage.to_path_buf()),
        })
    } else {
        let root = resolve_path(cfg_dir, &src.source);
        let available = discover(&root)?;
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
    let mut out = HashMap::new();
    discover_skills_in_subdir(root, &mut out)?;
    discover_skills_in_subdir(&root.join("skills"), &mut out)?;
    Ok(out)
}

fn discover_skills_in_subdir(base: &Path, out: &mut HashMap<String, PathBuf>) -> Result<()> {
    if !base.exists() {
        return Ok(());
    }
    for e in fs::read_dir(base)? {
        let e = e?;
        if !e.file_type()?.is_dir() {
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
