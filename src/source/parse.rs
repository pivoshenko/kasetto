//! Repository URL parsing into a structured [`RepoUrl`].

use crate::error::{err, Result};

use super::hosts::{is_bitbucket_host, is_gitea_style_host, is_gitlab_host};

#[derive(Debug, Clone)]
pub(crate) enum RepoUrl {
    GitHub {
        host: String,
        owner: String,
        repo: String,
    },
    GitLab {
        host: String,
        project_path: String,
    },
    /// Bitbucket Cloud (`bitbucket.org`).
    Bitbucket {
        workspace: String,
        repo_slug: String,
    },
    /// Gitea / Forgejo - including Codeberg (`codeberg.org`).
    Gitea {
        host: String,
        owner: String,
        repo: String,
    },
}

pub(crate) fn parse_repo_url(url: &str) -> Result<RepoUrl> {
    let cleaned = url.trim_end_matches('/').trim_end_matches(".git");
    let without_scheme = cleaned
        .strip_prefix("https://")
        .or_else(|| cleaned.strip_prefix("http://"))
        .ok_or_else(|| err("unsupported URL scheme"))?;

    let parts: Vec<_> = without_scheme.splitn(2, '/').collect();
    if parts.len() < 2 || parts[1].is_empty() {
        return Err(err("unsupported repository URL"));
    }

    let host = parts[0];
    let path = parts[1];

    if is_gitlab_host(host) {
        return Ok(RepoUrl::GitLab {
            host: host.to_string(),
            project_path: path.to_string(),
        });
    }

    if is_bitbucket_host(host) {
        let segments = path_segments(path);
        if segments.len() != 2 {
            return Err(err(
                "invalid Bitbucket URL: expected https://bitbucket.org/workspace/repo",
            ));
        }
        return Ok(RepoUrl::Bitbucket {
            workspace: segments[0].to_string(),
            repo_slug: segments[1].to_string(),
        });
    }

    let segments = path_segments(path);
    if segments.len() < 2 {
        return Err(err(
            "unsupported repository URL: expected at least owner/repo",
        ));
    }

    if host == "github.com" {
        if segments.len() != 2 {
            return Err(err(
                "invalid GitHub URL: expected https://github.com/owner/repo",
            ));
        }
        return Ok(RepoUrl::GitHub {
            host: host.to_string(),
            owner: segments[0].to_string(),
            repo: segments[1].to_string(),
        });
    }

    if is_gitea_style_host(host) {
        if segments.len() != 2 {
            return Err(err(
                "invalid URL: expected https://host/owner/repo (Gitea / Codeberg style)",
            ));
        }
        return Ok(RepoUrl::Gitea {
            host: host.to_string(),
            owner: segments[0].to_string(),
            repo: segments[1].to_string(),
        });
    }

    if segments.len() >= 3 {
        return Ok(RepoUrl::GitLab {
            host: host.to_string(),
            project_path: path.to_string(),
        });
    }

    Ok(RepoUrl::GitHub {
        host: host.to_string(),
        owner: segments[0].to_string(),
        repo: segments[1].to_string(),
    })
}

fn path_segments(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// A repo-browse URL decomposed into the pieces `add` needs: the repo root, the
/// pinned ref, the sub-directory, and (when the URL points at a `SKILL.md`) the
/// skill name. Lets a user paste the exact URL they're looking at.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct BrowseDerived {
    pub source: String,
    pub branch: Option<String>,
    pub git_ref: Option<String>,
    pub sub_dir: Option<String>,
    pub skill_name: Option<String>,
}

/// Decompose a GitHub/Gitea/GitLab `blob`/`tree` browse URL. Returns `None` for
/// plain repo URLs and local paths (the caller uses the source verbatim).
///
/// - `…/{owner}/{repo}/blob/{ref}/{path}/SKILL.md` → sub-dir = parent of the
///   skill dir, skill_name = the skill dir's name.
/// - `…/{owner}/{repo}/tree/{ref}/{path}` → sub-dir = `{path}`, no name.
/// - GitLab uses a `/-/` separator before `blob`/`tree`.
/// - A 40-hex `{ref}` is stored as `git_ref` (pinned); anything else as `branch`
///   (so `--update` keeps tracking it). Explicit flags override either.
pub(crate) fn derive_browse_url(url: &str) -> Option<BrowseDerived> {
    let scheme = if url.starts_with("https://") {
        "https://"
    } else if url.starts_with("http://") {
        "http://"
    } else {
        return None;
    };
    let without = url.trim_end_matches('/').strip_prefix(scheme)?;
    let segs: Vec<&str> = without.split('/').filter(|s| !s.is_empty()).collect();

    // Need at least host/owner/repo/<marker>/<ref>.
    let marker = segs.iter().position(|s| *s == "blob" || *s == "tree")?;
    if marker < 3 || marker + 1 >= segs.len() {
        return None;
    }

    // Repo path is everything before the marker, dropping a trailing GitLab `-`.
    let mut repo_end = marker;
    if segs[repo_end - 1] == "-" {
        repo_end -= 1;
    }
    if repo_end < 3 {
        return None;
    }
    let host_and_repo = segs[..repo_end].join("/");
    let source = format!("{scheme}{host_and_repo}");

    // TODO: branches that contain `/` (`feature/foo`, `release/2026-Q1`) are
    // genuinely ambiguous from URL syntax alone — `…/tree/feature/foo/skills/a`
    // could be ref `feature` + subdir `foo/skills/a` *or* ref `feature/foo` +
    // subdir `skills/a`. We take the first segment as the ref, which matches
    // the most common case (single-segment branches, tags, SHAs). For a proper
    // fix, probe the host's branches API for the longest existing prefix.
    // Today's failure modes: with verify on (the default), `add` errors at
    // fetch time; with `--no-verify`, a wrong entry can be written — override
    // with explicit `--branch` + `--sub-dir` in that case.
    let git_ref_seg = segs[marker + 1];
    let rest: Vec<&str> = segs[marker + 2..].to_vec();

    let (sub_dir, skill_name) = if rest.last() == Some(&"SKILL.md") {
        let skill_segs = &rest[..rest.len() - 1];
        match skill_segs.split_last() {
            Some((name, parent)) => {
                let sub = (!parent.is_empty()).then(|| parent.join("/"));
                (sub, Some((*name).to_string()))
            }
            None => (None, None),
        }
    } else {
        let sub = (!rest.is_empty()).then(|| rest.join("/"));
        (sub, None)
    };

    let is_sha = git_ref_seg.len() == 40 && git_ref_seg.bytes().all(|b| b.is_ascii_hexdigit());
    let (branch, git_ref) = if is_sha {
        (None, Some(git_ref_seg.to_string()))
    } else {
        (Some(git_ref_seg.to_string()), None)
    };

    Some(BrowseDerived {
        source,
        branch,
        git_ref,
        sub_dir,
        skill_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repo_url_github() {
        let url = parse_repo_url("https://github.com/openai/skills").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::GitHub { host, owner, repo }
                    if host == "github.com" && owner == "openai" && repo == "skills"
            ),
            "expected GitHub URL"
        );
    }

    #[test]
    fn parse_repo_url_github_enterprise_two_segment_path() {
        let url = parse_repo_url("https://ghe.example.com/acme/skill-pack").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::GitHub { host, owner, repo }
                    if host == "ghe.example.com" && owner == "acme" && repo == "skill-pack"
            ),
            "expected GitHub Enterprise-style URL"
        );
    }

    #[test]
    fn parse_repo_url_github_trims_git_and_trailing_slash() {
        let url = parse_repo_url("https://github.com/pivoshenko/kasetto.git/").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::GitHub { host, owner, repo }
                    if host == "github.com" && owner == "pivoshenko" && repo == "kasetto"
            ),
            "expected trimmed GitHub URL"
        );
    }

    #[test]
    fn parse_repo_url_gitlab() {
        let url = parse_repo_url("https://gitlab.example.com/group/subgroup/repo").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::GitLab { host, project_path }
                    if host == "gitlab.example.com" && project_path == "group/subgroup/repo"
            ),
            "expected GitLab URL"
        );
    }

    #[test]
    fn parse_repo_url_gitlab_com_two_segments() {
        let url = parse_repo_url("https://gitlab.com/group/project").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::GitLab { host, project_path }
                    if host == "gitlab.com" && project_path == "group/project"
            ),
            "expected gitlab.com URL"
        );
    }

    #[test]
    fn parse_repo_url_bitbucket_cloud() {
        let url = parse_repo_url("https://bitbucket.org/workspace/skill-repo").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::Bitbucket { workspace, repo_slug }
                    if workspace == "workspace" && repo_slug == "skill-repo"
            ),
            "expected Bitbucket URL"
        );
    }

    #[test]
    fn derive_blob_skill_md_splits_subdir_and_name() {
        let d = derive_browse_url(
            "https://github.com/mattpocock/skills/blob/main/skills/personal/edit-article/SKILL.md",
        )
        .expect("derive");
        assert_eq!(d.source, "https://github.com/mattpocock/skills");
        assert_eq!(d.branch.as_deref(), Some("main"));
        assert_eq!(d.git_ref, None);
        assert_eq!(d.sub_dir.as_deref(), Some("skills/personal"));
        assert_eq!(d.skill_name.as_deref(), Some("edit-article"));
    }

    #[test]
    fn derive_tree_uses_path_as_subdir_no_name() {
        let d = derive_browse_url("https://github.com/mattpocock/skills/tree/main/skills/personal")
            .expect("derive");
        assert_eq!(d.source, "https://github.com/mattpocock/skills");
        assert_eq!(d.branch.as_deref(), Some("main"));
        assert_eq!(d.sub_dir.as_deref(), Some("skills/personal"));
        assert_eq!(d.skill_name, None);
    }

    #[test]
    fn derive_sha_ref_is_pinned_not_branch() {
        let sha = "a".repeat(40);
        let d =
            derive_browse_url(&format!("https://github.com/o/r/tree/{sha}/pack")).expect("derive");
        assert_eq!(d.git_ref.as_deref(), Some(sha.as_str()));
        assert_eq!(d.branch, None);
    }

    #[test]
    fn derive_gitlab_dash_separator() {
        let d = derive_browse_url("https://gitlab.com/group/proj/-/tree/main/skills/a")
            .expect("derive");
        assert_eq!(d.source, "https://gitlab.com/group/proj");
        assert_eq!(d.branch.as_deref(), Some("main"));
        assert_eq!(d.sub_dir.as_deref(), Some("skills/a"));
    }

    #[test]
    fn derive_plain_repo_url_is_none() {
        assert_eq!(derive_browse_url("https://github.com/owner/repo"), None);
        assert_eq!(derive_browse_url("./local/pack"), None);
    }

    #[test]
    fn parse_repo_url_codeberg() {
        let url = parse_repo_url("https://codeberg.org/someone/skills").expect("parse");
        assert!(
            matches!(
                url,
                RepoUrl::Gitea { host, owner, repo }
                    if host == "codeberg.org" && owner == "someone" && repo == "skills"
            ),
            "expected Codeberg (Gitea) URL"
        );
    }
}
