//! Environment-based credentials and HTTP auth hints for remote sources.

use reqwest::blocking::RequestBuilder;

use super::hosts::{extract_host, is_bitbucket_host, is_gitea_style_host, is_gitlab_host};

/// Optional custom headers plus HTTP Basic credentials (Bitbucket Cloud).
pub(crate) struct UrlRequestAuth {
    pub headers: Vec<(String, String)>,
    pub basic: Option<(String, String)>,
}

impl UrlRequestAuth {
    pub(crate) fn apply(&self, mut request: RequestBuilder) -> RequestBuilder {
        if let Some((user, pass)) = &self.basic {
            request = request.basic_auth(user, Some(pass));
        }
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }
        request
    }

    fn headers_only(headers: Vec<(String, String)>) -> Self {
        Self {
            headers,
            basic: None,
        }
    }

    fn basic_only(basic: Option<(String, String)>) -> Self {
        Self {
            headers: Vec::new(),
            basic,
        }
    }

    pub(super) fn for_github_archive() -> Self {
        Self::headers_only(github_auth_headers())
    }

    pub(super) fn for_gitlab_archive() -> Self {
        Self::headers_only(gitlab_auth_headers())
    }

    pub(super) fn for_bitbucket_archive() -> Self {
        Self::basic_only(bitbucket_basic_credentials())
    }

    pub(super) fn for_gitea_archive() -> Self {
        Self::headers_only(gitea_auth_headers())
    }
}

pub(crate) fn auth_env_inline_help(url: &str) -> String {
    match extract_host(url) {
        Some(h) if is_gitlab_host(&h) => {
            "set GITLAB_TOKEN (or CI_JOB_TOKEN in GitLab CI) for private GitLab.".into()
        }
        Some(h) if is_bitbucket_host(&h) => {
            "set BITBUCKET_EMAIL and BITBUCKET_TOKEN (Atlassian API token with repository read), \
             or BITBUCKET_USERNAME and BITBUCKET_APP_PASSWORD for Bitbucket Cloud."
                .into()
        }
        Some(h) if is_gitea_style_host(&h) => {
            "set CODEBERG_TOKEN, GITEA_TOKEN, or FORGEJO_TOKEN for private Codeberg (or other Gitea/Forgejo) repositories."
                .into()
        }
        Some(_) => "set GITHUB_TOKEN or GH_TOKEN for private GitHub or GitHub Enterprise.".into(),
        None => "set GITHUB_TOKEN, GH_TOKEN, GITLAB_TOKEN, Bitbucket credentials (see docs), or CODEBERG_TOKEN / GITEA_TOKEN for private repositories.".into(),
    }
}

/// Extra context for HTTP failures when fetching remote config or repo archives (issue #11).
pub(crate) fn http_fetch_auth_hint(url: &str, status: u16) -> String {
    match status {
        401 | 403 => format!(" - {}", auth_env_inline_help(url)),
        404 => format!(
            " - if the repo or file is private, {}",
            auth_env_inline_help(url)
        ),
        _ => String::new(),
    }
}

/// Auth for fetching a remote resource over HTTPS (config file or archive).
pub(crate) fn auth_for_request_url(url: &str) -> UrlRequestAuth {
    let Some(host) = extract_host(url) else {
        return UrlRequestAuth {
            headers: Vec::new(),
            basic: None,
        };
    };
    if is_gitlab_host(&host) {
        return UrlRequestAuth::headers_only(gitlab_auth_headers());
    }
    if is_bitbucket_host(&host) {
        return UrlRequestAuth::basic_only(bitbucket_basic_credentials());
    }
    if is_gitea_style_host(&host) {
        return UrlRequestAuth::headers_only(gitea_auth_headers());
    }
    UrlRequestAuth::headers_only(github_auth_headers())
}

fn bitbucket_basic_credentials() -> Option<(String, String)> {
    if let (Ok(email), Ok(token)) = (
        std::env::var("BITBUCKET_EMAIL"),
        std::env::var("BITBUCKET_TOKEN"),
    ) {
        return Some((email, token));
    }
    if let (Ok(user), Ok(pass)) = (
        std::env::var("BITBUCKET_USERNAME"),
        std::env::var("BITBUCKET_APP_PASSWORD"),
    ) {
        return Some((user, pass));
    }
    None
}

fn first_env_var(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| std::env::var(k).ok())
}

fn gitea_auth_headers() -> Vec<(String, String)> {
    match first_env_var(&["GITEA_TOKEN", "CODEBERG_TOKEN", "FORGEJO_TOKEN"]) {
        Some(token) => vec![("Authorization".to_string(), format!("token {token}"))],
        None => Vec::new(),
    }
}

fn gitlab_auth_headers() -> Vec<(String, String)> {
    if let Ok(token) = std::env::var("GITLAB_TOKEN") {
        vec![("PRIVATE-TOKEN".to_string(), token)]
    } else if let Ok(token) = std::env::var("CI_JOB_TOKEN") {
        vec![("JOB-TOKEN".to_string(), token)]
    } else {
        Vec::new()
    }
}

fn github_auth_headers() -> Vec<(String, String)> {
    match first_env_var(&["GITHUB_TOKEN", "GH_TOKEN"]) {
        Some(token) => vec![("Authorization".to_string(), format!("Bearer {token}"))],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_fetch_auth_hint_mentions_github_token_for_github_host() {
        let h = http_fetch_auth_hint("https://github.com/org/private", 403);
        assert!(h.contains("GITHUB_TOKEN") || h.contains("GH_TOKEN"), "{h}");
    }

    #[test]
    fn http_fetch_auth_hint_mentions_gitlab_token_for_gitlab_host() {
        let h = http_fetch_auth_hint("https://gitlab.com/group/proj", 401);
        assert!(h.contains("GITLAB_TOKEN"), "{h}");
    }

    #[test]
    fn http_fetch_auth_hint_mentions_bitbucket_env_for_bitbucket_host() {
        let h = http_fetch_auth_hint("https://bitbucket.org/ws/r", 403);
        assert!(
            h.contains("BITBUCKET_EMAIL") || h.contains("BITBUCKET_USERNAME"),
            "{h}"
        );
    }

    #[test]
    fn http_fetch_auth_hint_mentions_gitea_token_for_codeberg_host() {
        let h = http_fetch_auth_hint("https://codeberg.org/u/r", 401);
        assert!(
            h.contains("CODEBERG_TOKEN")
                || h.contains("GITEA_TOKEN")
                || h.contains("FORGEJO_TOKEN"),
            "{h}"
        );
    }
}
