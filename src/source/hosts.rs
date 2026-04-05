//! HTTP(S) URL host classification for skill sources and remote config.

pub(crate) fn extract_host(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    Some(without_scheme.split('/').next()?.to_string())
}

pub(crate) fn is_gitlab_host(host: &str) -> bool {
    host == "gitlab.com" || host.ends_with(".gitlab.com") || host.starts_with("gitlab.")
}

pub(crate) fn is_bitbucket_host(host: &str) -> bool {
    host == "bitbucket.org" || host == "www.bitbucket.org"
}

/// Hosts that serve Gitea-style `/{owner}/{repo}/archive/{ref}.tar.gz` (Codeberg, Gitea, Forgejo).
pub(crate) fn is_gitea_style_host(host: &str) -> bool {
    matches!(
        host,
        "codeberg.org"
            | "www.codeberg.org"
            | "gitea.com"
            | "www.gitea.com"
            | "forgejo.org"
            | "www.forgejo.org"
    )
}
