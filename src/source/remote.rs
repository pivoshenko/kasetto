//! Remote archive and tarball download (GitHub, GitLab, Bitbucket, Gitea).

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::http_client;

use super::auth::{auth_env_inline_help, http_fetch_auth_hint, UrlRequestAuth};
use super::parse::RepoUrl;
use crate::model::GitPin;

/// Build archive URL for a branch name (uses `refs/heads/` prefix for GitHub).
pub(super) fn remote_repo_archive_branch(
    parsed: &RepoUrl,
    branch: &str,
) -> (String, UrlRequestAuth) {
    match parsed {
        RepoUrl::GitHub { host, owner, repo } => {
            let auth = UrlRequestAuth::for_github_archive();
            // GitHub's web archive endpoint doesn't support token auth for private repos.
            // The API endpoint (api.github.com) does and works for public repos too.
            let url = if host == "github.com" && !auth.headers.is_empty() {
                format!(
                    "https://api.{host}/repos/{owner}/{repo}/tarball/{}",
                    encode_github_ref(branch)
                )
            } else {
                format!("https://{host}/{owner}/{repo}/archive/refs/heads/{branch}.tar.gz")
            };
            (url, auth)
        }
        _ => remote_repo_archive_ref(parsed, branch),
    }
}

/// Build archive URL for a generic git ref (tag, SHA, branch).
/// Uses the short form that works for any ref type on all hosts.
pub(super) fn remote_repo_archive_ref(parsed: &RepoUrl, git_ref: &str) -> (String, UrlRequestAuth) {
    match parsed {
        RepoUrl::GitHub { host, owner, repo } => {
            let auth = UrlRequestAuth::for_github_archive();
            let url = if host == "github.com" && !auth.headers.is_empty() {
                format!(
                    "https://api.{host}/repos/{owner}/{repo}/tarball/{}",
                    encode_github_ref(git_ref)
                )
            } else {
                format!("https://{host}/{owner}/{repo}/archive/{git_ref}.tar.gz")
            };
            (url, auth)
        }
        RepoUrl::GitLab { host, project_path } => (
            gitlab_project_archive_url(host, project_path, git_ref),
            UrlRequestAuth::for_gitlab_archive(),
        ),
        RepoUrl::Bitbucket {
            workspace,
            repo_slug,
        } => (
            bitbucket_archive_tarball_url(workspace, repo_slug, git_ref),
            UrlRequestAuth::for_bitbucket_archive(),
        ),
        RepoUrl::Gitea { host, owner, repo } => (
            gitea_archive_tarball_url(host, owner, repo, git_ref),
            UrlRequestAuth::for_gitea_archive(),
        ),
    }
}

/// GitLab API path encoding: `/` → `%2F`.
fn encode_gitlab_path(path: &str) -> String {
    path.replace('/', "%2F")
}

/// GitHub API ref encoding: `/` → `%2F` so that refs like `feature/foo`
/// are treated as a single path segment in the tarball URL.
fn encode_github_ref(git_ref: &str) -> String {
    git_ref.replace('/', "%2F")
}

fn gitlab_project_archive_url(host: &str, project_path: &str, branch: &str) -> String {
    let encoded = encode_gitlab_path(project_path);
    format!("https://{host}/api/v4/projects/{encoded}/repository/archive.tar.gz?sha={branch}")
}

fn gitlab_repository_file_raw_url(
    host: &str,
    project: &str,
    file_path: &str,
    git_ref: &str,
) -> String {
    format!(
        "https://{host}/api/v4/projects/{}/repository/files/{}/raw?ref={git_ref}",
        encode_gitlab_path(project),
        encode_gitlab_path(file_path),
    )
}

/// Bitbucket Cloud source archive (see Atlassian KB: `.../get/{branch}.tar.gz`).
fn bitbucket_archive_tarball_url(workspace: &str, repo_slug: &str, branch: &str) -> String {
    format!("https://bitbucket.org/{workspace}/{repo_slug}/get/{branch}.tar.gz")
}

fn gitea_archive_tarball_url(host: &str, owner: &str, repo: &str, branch: &str) -> String {
    format!("https://{host}/{owner}/{repo}/archive/{branch}.tar.gz")
}

/// Rewrite browser-style URLs (e.g. `/blob/`, `/src/branch/`) to the raw-content
/// equivalent so users can paste a URL straight from their browser into
/// `--config` or skill sources.
pub(crate) fn rewrite_browse_to_raw_url(url: &str) -> Option<String> {
    let (cleaned, query) = match url.split_once('?') {
        Some((c, q)) => (c, Some(q)),
        None => (url, None),
    };
    let scheme_len = if cleaned.starts_with("https://") {
        "https://".len()
    } else if cleaned.starts_with("http://") {
        "http://".len()
    } else {
        return None;
    };
    let scheme = &cleaned[..scheme_len];
    let without_scheme = &cleaned[scheme_len..];
    let (host, rest) = without_scheme.split_once('/')?;

    if host == "github.com" {
        if let Some(rewritten) = rewrite_github_blob(rest) {
            return Some(rewritten);
        }
        return None;
    }

    if super::hosts::is_gitea_style_host(host) {
        if let Some(rewritten) = rewrite_gitea_src(scheme, host, rest, query) {
            return Some(rewritten);
        }
        return None;
    }

    rewrite_gitlab_raw_url(host, rest)
}

fn rewrite_github_blob(rest: &str) -> Option<String> {
    let parts: Vec<&str> = rest.splitn(5, '/').collect();
    if parts.len() < 5 {
        return None;
    }
    let (owner, repo, marker, git_ref, file_path) =
        (parts[0], parts[1], parts[2], parts[3], parts[4]);
    if !matches!(marker, "blob" | "raw") {
        return None;
    }
    if owner.is_empty() || repo.is_empty() || git_ref.is_empty() || file_path.is_empty() {
        return None;
    }
    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{git_ref}/{file_path}"
    ))
}

fn rewrite_gitea_src(scheme: &str, host: &str, rest: &str, query: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = rest.splitn(6, '/').collect();
    if parts.len() < 6 {
        return None;
    }
    let (owner, repo, src, kind, git_ref, file_path) =
        (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]);
    if src != "src" {
        return None;
    }
    if !matches!(kind, "branch" | "commit" | "tag") {
        return None;
    }
    if owner.is_empty() || repo.is_empty() || git_ref.is_empty() || file_path.is_empty() {
        return None;
    }
    let mut out = format!("{scheme}{host}/{owner}/{repo}/raw/{kind}/{git_ref}/{file_path}");
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    Some(out)
}

fn rewrite_gitlab_raw_url(host: &str, rest: &str) -> Option<String> {
    for marker in ["/-/raw/", "/-/blob/"] {
        if let Some(idx) = rest.find(marker) {
            let project = &rest[..idx];
            let after = &rest[idx + marker.len()..];
            let (ref_name, file_path) = after.split_once('/')?;
            return Some(gitlab_repository_file_raw_url(
                host, project, file_path, ref_name,
            ));
        }
    }

    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 3 {
        return None;
    }
    let file_start = parts.iter().position(|p| p.contains('.'))?;
    if file_start < 2 {
        return None;
    }
    let project = parts[..file_start].join("/");
    let file_path = parts[file_start..].join("/");
    Some(gitlab_repository_file_raw_url(
        host, &project, &file_path, "main",
    ))
}

pub(super) fn download_extract(
    fetch_url: &str,
    auth: &UrlRequestAuth,
    dst: &Path,
    user_source: &str,
) -> Result<String> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    let request = http_client()?.get(fetch_url);
    let request = auth.apply(request);
    let response = request
        .send()
        .map_err(|e| err(format!("failed to reach {user_source}: {e}")))?;
    let status = response.status();
    let status_u16 = status.as_u16();
    let body = response
        .bytes()
        .map_err(|e| err(format!("failed to read archive for {user_source}: {e}")))?;
    if !status.is_success() {
        return Err(err(format!(
            "failed to download {user_source} (HTTP {status_u16}){}",
            http_fetch_auth_hint(user_source, status_u16)
        )));
    }
    if body.starts_with(b"<") || body.starts_with(b"<!") {
        return Err(err(format!(
            "failed to download {user_source}: server returned HTML instead of a .tar.gz - {}",
            auth_env_inline_help(user_source)
        )));
    }
    let gz = flate2::read::GzDecoder::new(body.as_ref());
    let mut archive = tar::Archive::new(gz);
    let mut archive_root = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let p = entry.path()?;
        let parts: Vec<_> = p.components().collect();
        if archive_root.is_none() {
            archive_root = parts
                .first()
                .map(|component| component.as_os_str().to_string_lossy().to_string());
        }
        if parts.len() < 2 {
            continue;
        }
        let rel = parts
            .iter()
            .skip(1)
            .map(|c| c.as_os_str())
            .collect::<PathBuf>();
        if rel.components().any(|c| c == Component::ParentDir) {
            return Err(err("unsafe archive path"));
        }
        let target = dst.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(target)?;
    }
    Ok(archive_root.unwrap_or_else(|| "unknown".to_string()))
}

fn archive_root_name(parsed: &RepoUrl, sha: &str) -> String {
    match parsed {
        RepoUrl::GitHub { owner, repo, .. } => format!("{owner}-{repo}-{sha}"),
        RepoUrl::Gitea { owner, repo, .. } => format!("{owner}-{repo}-{sha}"),
        RepoUrl::Bitbucket {
            workspace,
            repo_slug,
            ..
        } => {
            format!("{workspace}-{repo_slug}-{sha}")
        }
        RepoUrl::GitLab { project_path, .. } => {
            let slug = project_path.replace('/', "-");
            format!("{slug}-{sha}")
        }
    }
}

fn fetch_json_field(url: &str, auth: &UrlRequestAuth, field_path: &[&str]) -> Option<String> {
    let client = http_client().ok()?;
    let request = client.get(url);
    let request = auth.apply(request);
    let response = request.send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let text = response.text().ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut current = &value;
    for field in field_path {
        current = current.get(field)?;
    }
    current.as_str().map(String::from)
}

fn resolve_branch_sha(parsed: &RepoUrl, branch: &str) -> Option<String> {
    match parsed {
        RepoUrl::GitHub { host, owner, repo } => {
            let api_host = if host == "github.com" {
                "api.github.com"
            } else {
                host
            };
            let url = format!("https://{api_host}/repos/{owner}/{repo}/branches/{branch}");
            let auth = UrlRequestAuth::for_github_archive();
            fetch_json_field(&url, &auth, &["commit", "sha"])
        }
        RepoUrl::GitLab { host, project_path } => {
            let encoded = project_path.replace('/', "%2F");
            let url =
                format!("https://{host}/api/v4/projects/{encoded}/repository/branches/{branch}");
            let auth = UrlRequestAuth::for_gitlab_archive();
            fetch_json_field(&url, &auth, &["commit", "id"])
        }
        RepoUrl::Bitbucket {
            workspace,
            repo_slug,
        } => {
            let url = format!(
                "https://api.bitbucket.org/2.0/repositories/{workspace}/{repo_slug}/refs/branches/{branch}"
            );
            let auth = UrlRequestAuth::for_bitbucket_archive();
            fetch_json_field(&url, &auth, &["target", "hash"])
        }
        RepoUrl::Gitea { host, owner, repo } => {
            let url = format!("https://{host}/api/v1/repos/{owner}/{repo}/branches/{branch}");
            let auth = UrlRequestAuth::for_gitea_archive();
            fetch_json_field(&url, &auth, &["commit", "id"])
        }
    }
}

fn resolve_ref_sha(parsed: &RepoUrl, git_ref: &str) -> Option<String> {
    match parsed {
        RepoUrl::GitHub { host, owner, repo } => {
            let api_host = if host == "github.com" {
                "api.github.com"
            } else {
                host
            };
            let url = format!("https://{api_host}/repos/{owner}/{repo}/git/ref/{git_ref}");
            let auth = UrlRequestAuth::for_github_archive();
            fetch_json_field(&url, &auth, &["object", "sha"])
        }
        RepoUrl::GitLab { host, project_path } => {
            let encoded = project_path.replace('/', "%2F");
            let url =
                format!("https://{host}/api/v4/projects/{encoded}/repository/commits/{git_ref}");
            let auth = UrlRequestAuth::for_gitlab_archive();
            fetch_json_field(&url, &auth, &["id"])
        }
        RepoUrl::Bitbucket {
            workspace,
            repo_slug,
        } => {
            let url = format!(
                "https://api.bitbucket.org/2.0/repositories/{workspace}/{repo_slug}/commit/{git_ref}"
            );
            let auth = UrlRequestAuth::for_bitbucket_archive();
            fetch_json_field(&url, &auth, &["hash"])
        }
        RepoUrl::Gitea { host, owner, repo } => {
            let url = format!("https://{host}/api/v1/repos/{owner}/{repo}/git/commits/{git_ref}");
            let auth = UrlRequestAuth::for_gitea_archive();
            fetch_json_field(&url, &auth, &["sha"])
        }
    }
}

/// Resolve the full revision string for a remote ref without downloading the archive.
/// Returns `branch:{name}@{archive_root}` or `ref:{ref}@{archive_root}`.
/// Returns `None` if the provider can't be reached or the ref doesn't exist (caller falls back to archive download).
pub(super) fn resolve_remote_revision(parsed: &RepoUrl, pin: &GitPin) -> Option<String> {
    let (label, sha) = match pin {
        GitPin::Ref(r) => {
            let sha = resolve_ref_sha(parsed, r)?;
            (format!("ref:{r}"), sha)
        }
        GitPin::Branch(b) => {
            let sha = resolve_branch_sha(parsed, b)?;
            (format!("branch:{b}"), sha)
        }
        GitPin::Default => {
            if let Some(sha) = resolve_branch_sha(parsed, "main") {
                ("branch:main".to_string(), sha)
            } else {
                let sha = resolve_branch_sha(parsed, "master")?;
                ("branch:master".to_string(), sha)
            }
        }
    };
    let root = archive_root_name(parsed, &sha);
    Some(format!("{label}@{root}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn archive_root_name_github() {
        let parsed = RepoUrl::GitHub {
            host: "github.com".into(),
            owner: "openai".into(),
            repo: "skills".into(),
        };
        assert_eq!(archive_root_name(&parsed, "abc123"), "openai-skills-abc123");
    }

    #[test]
    fn archive_root_name_gitea() {
        let parsed = RepoUrl::Gitea {
            host: "codeberg.org".into(),
            owner: "user".into(),
            repo: "my-skills".into(),
        };
        assert_eq!(
            archive_root_name(&parsed, "def456"),
            "user-my-skills-def456"
        );
    }

    #[test]
    fn archive_root_name_bitbucket() {
        let parsed = RepoUrl::Bitbucket {
            workspace: "acme".into(),
            repo_slug: "skill-packs".into(),
        };
        assert_eq!(
            archive_root_name(&parsed, "789ghi"),
            "acme-skill-packs-789ghi"
        );
    }

    #[test]
    fn archive_root_name_gitlab() {
        let parsed = RepoUrl::GitLab {
            host: "gitlab.com".into(),
            project_path: "group/subgroup/repo".into(),
        };
        assert_eq!(
            archive_root_name(&parsed, "jkl012"),
            "group-subgroup-repo-jkl012"
        );
    }

    #[test]
    fn github_branch_archive_uses_refs_heads_prefix_without_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("GH_TOKEN");
        let parsed = RepoUrl::GitHub {
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
        };
        // No token set → falls back to web archive URL.
        let (url, _) = remote_repo_archive_branch(&parsed, "main");
        assert_eq!(url, "https://github.com/o/r/archive/refs/heads/main.tar.gz");
    }

    #[test]
    fn github_branch_archive_uses_api_endpoint_with_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("GITHUB_TOKEN", "test-token");
        let parsed = RepoUrl::GitHub {
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
        };
        let (url, _) = remote_repo_archive_branch(&parsed, "main");
        std::env::remove_var("GITHUB_TOKEN");
        assert_eq!(url, "https://api.github.com/repos/o/r/tarball/main");
    }

    #[test]
    fn github_branch_archive_encodes_slash_in_ref_with_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("GITHUB_TOKEN", "test-token");
        let parsed = RepoUrl::GitHub {
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
        };
        let (url, _) = remote_repo_archive_branch(&parsed, "feature/foo");
        std::env::remove_var("GITHUB_TOKEN");
        assert_eq!(
            url,
            "https://api.github.com/repos/o/r/tarball/feature%2Ffoo"
        );
    }

    #[test]
    fn github_ref_archive_uses_short_form_without_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("GITHUB_TOKEN");
        std::env::remove_var("GH_TOKEN");
        let parsed = RepoUrl::GitHub {
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
        };
        let (url, _) = remote_repo_archive_ref(&parsed, "v2.0");
        assert_eq!(url, "https://github.com/o/r/archive/v2.0.tar.gz");
        let (url, _) = remote_repo_archive_ref(&parsed, "abc123def");
        assert_eq!(url, "https://github.com/o/r/archive/abc123def.tar.gz");
    }

    #[test]
    fn github_ref_archive_encodes_slash_in_ref_with_token() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("GITHUB_TOKEN", "test-token");
        let parsed = RepoUrl::GitHub {
            host: "github.com".into(),
            owner: "o".into(),
            repo: "r".into(),
        };
        let (url, _) = remote_repo_archive_ref(&parsed, "refs/tags/release/1.2");
        std::env::remove_var("GITHUB_TOKEN");
        assert_eq!(
            url,
            "https://api.github.com/repos/o/r/tarball/refs%2Ftags%2Frelease%2F1.2"
        );
    }

    #[test]
    fn bitbucket_archive_urls_use_bitbucket_org() {
        let u = bitbucket_archive_tarball_url("ws", "myrepo", "main");
        assert_eq!(u, "https://bitbucket.org/ws/myrepo/get/main.tar.gz");
    }

    #[test]
    fn gitea_archive_urls_match_web_download() {
        let u = gitea_archive_tarball_url("codeberg.org", "a", "b", "main");
        assert_eq!(u, "https://codeberg.org/a/b/archive/main.tar.gz");
    }

    #[test]
    fn rewrite_github_blob_url_to_raw() {
        let out = rewrite_browse_to_raw_url(
            "https://github.com/pivoshenko/kasetto/blob/main/kasetto.yml",
        )
        .expect("rewritten");
        assert_eq!(
            out,
            "https://raw.githubusercontent.com/pivoshenko/kasetto/main/kasetto.yml"
        );
    }

    #[test]
    fn rewrite_github_blob_url_with_nested_path() {
        let out = rewrite_browse_to_raw_url(
            "https://github.com/owner/repo/blob/v1.2.3/configs/kasetto.yml",
        )
        .expect("rewritten");
        assert_eq!(
            out,
            "https://raw.githubusercontent.com/owner/repo/v1.2.3/configs/kasetto.yml"
        );
    }

    #[test]
    fn rewrite_github_raw_url_to_raw_alias() {
        let out = rewrite_browse_to_raw_url("https://github.com/owner/repo/raw/main/kasetto.yml")
            .expect("rewritten");
        assert_eq!(
            out,
            "https://raw.githubusercontent.com/owner/repo/main/kasetto.yml"
        );
    }

    #[test]
    fn rewrite_github_repo_root_returns_none() {
        assert!(rewrite_browse_to_raw_url("https://github.com/owner/repo").is_none());
    }

    #[test]
    fn rewrite_gitea_src_branch_to_raw() {
        let out = rewrite_browse_to_raw_url(
            "https://codeberg.org/owner/repo/src/branch/main/kasetto.yml",
        )
        .expect("rewritten");
        assert_eq!(
            out,
            "https://codeberg.org/owner/repo/raw/branch/main/kasetto.yml"
        );
    }

    #[test]
    fn rewrite_gitea_src_tag_to_raw() {
        let out = rewrite_browse_to_raw_url(
            "https://codeberg.org/owner/repo/src/tag/v1.0.0/configs/kasetto.yml",
        )
        .expect("rewritten");
        assert_eq!(
            out,
            "https://codeberg.org/owner/repo/raw/tag/v1.0.0/configs/kasetto.yml"
        );
    }

    #[test]
    fn rewrite_gitlab_blob_url_uses_api_raw_endpoint() {
        let out =
            rewrite_browse_to_raw_url("https://gitlab.com/group/sub/repo/-/blob/main/kasetto.yml")
                .expect("rewritten");
        assert_eq!(
            out,
            "https://gitlab.com/api/v4/projects/group%2Fsub%2Frepo/repository/files/kasetto.yml/raw?ref=main"
        );
    }

    #[test]
    fn rewrite_skips_unrecognized_url() {
        assert!(rewrite_browse_to_raw_url("https://example.com/some/path").is_none());
    }

    #[test]
    fn rewrite_skips_non_http_scheme() {
        assert!(rewrite_browse_to_raw_url("git@github.com:owner/repo.git").is_none());
    }
}
