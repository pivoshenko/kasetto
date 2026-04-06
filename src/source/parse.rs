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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repo_url_github() {
        match parse_repo_url("https://github.com/openai/skills").expect("parse") {
            RepoUrl::GitHub { host, owner, repo } => {
                assert_eq!(host, "github.com");
                assert_eq!(owner, "openai");
                assert_eq!(repo, "skills");
            }
            _ => panic!("expected GitHub variant"),
        }
    }

    #[test]
    fn parse_repo_url_github_enterprise_two_segment_path() {
        match parse_repo_url("https://ghe.example.com/acme/skill-pack").expect("parse") {
            RepoUrl::GitHub { host, owner, repo } => {
                assert_eq!(host, "ghe.example.com");
                assert_eq!(owner, "acme");
                assert_eq!(repo, "skill-pack");
            }
            _ => panic!("expected GitHub variant"),
        }
    }

    #[test]
    fn parse_repo_url_github_trims_git_and_trailing_slash() {
        match parse_repo_url("https://github.com/pivoshenko/kasetto.git/").expect("parse") {
            RepoUrl::GitHub { host, owner, repo } => {
                assert_eq!(host, "github.com");
                assert_eq!(owner, "pivoshenko");
                assert_eq!(repo, "kasetto");
            }
            _ => panic!("expected GitHub variant"),
        }
    }

    #[test]
    fn parse_repo_url_gitlab() {
        match parse_repo_url("https://gitlab.example.com/group/subgroup/repo").expect("parse") {
            RepoUrl::GitLab { host, project_path } => {
                assert_eq!(host, "gitlab.example.com");
                assert_eq!(project_path, "group/subgroup/repo");
            }
            _ => panic!("expected GitLab variant"),
        }
    }

    #[test]
    fn parse_repo_url_gitlab_com_two_segments() {
        match parse_repo_url("https://gitlab.com/group/project").expect("parse") {
            RepoUrl::GitLab { host, project_path } => {
                assert_eq!(host, "gitlab.com");
                assert_eq!(project_path, "group/project");
            }
            _ => panic!("expected GitLab variant"),
        }
    }

    #[test]
    fn parse_repo_url_bitbucket_cloud() {
        match parse_repo_url("https://bitbucket.org/workspace/skill-repo").expect("parse") {
            RepoUrl::Bitbucket {
                workspace,
                repo_slug,
            } => {
                assert_eq!(workspace, "workspace");
                assert_eq!(repo_slug, "skill-repo");
            }
            _ => panic!("expected Bitbucket variant"),
        }
    }

    #[test]
    fn parse_repo_url_codeberg() {
        match parse_repo_url("https://codeberg.org/someone/skills").expect("parse") {
            RepoUrl::Gitea { host, owner, repo } => {
                assert_eq!(host, "codeberg.org");
                assert_eq!(owner, "someone");
                assert_eq!(repo, "skills");
            }
            _ => panic!("expected Gitea (Codeberg) variant"),
        }
    }
}
