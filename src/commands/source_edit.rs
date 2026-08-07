//! Shared plumbing for the config-editing commands (`add` / `remove`).

use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::model::Scope;

/// Resolve a local, writable config path. Remote configs (HTTP/S) cannot be
/// edited in place, so they are rejected with a hint to pass a local `--config`.
pub(super) fn resolve_local_config_path(config_override: Option<&str>) -> Result<PathBuf> {
    let raw = match config_override {
        Some(c) => c.to_string(),
        None => crate::default_config_path(),
    };
    if raw.contains("://") {
        return Err(err(format!(
            "cannot edit remote config `{raw}`; pass a local file with --config <path>"
        )));
    }
    Ok(PathBuf::from(raw))
}

/// Strip a trailing `@<ref>` shorthand off a positional source, cargo/uv-style
/// (e.g. `github.com/org/repo@v1.0`). The split only fires when the `@` lives in
/// the path tail (after the last `/`), so SSH-style `git@github.com:user/repo`
/// and userinfo URLs like `https://user@host/repo` round-trip unchanged.
pub(super) fn split_at_ref(source: &str) -> (String, Option<String>) {
    let Some(last_slash) = source.rfind('/') else {
        return (source.to_string(), None);
    };
    let tail = &source[last_slash..];
    let Some(at_rel) = tail.rfind('@') else {
        return (source.to_string(), None);
    };
    let at = last_slash + at_rel;
    let left = &source[..at];
    let right = &source[at + 1..];
    if right.is_empty() || left.is_empty() {
        return (source.to_string(), None);
    }
    (left.to_string(), Some(right.to_string()))
}

/// Run a plain sync against the freshly edited config so installs and the lock
/// catch up with the change. `add` installs the new source; `remove` prunes the
/// orphaned assets via sync's existing stale-cleanup pass.
pub(super) fn sync_after(
    path: &Path,
    scope: Option<Scope>,
    quiet: u8,
    plain: bool,
    locked: bool,
) -> Result<()> {
    let config_path = path.to_string_lossy();
    crate::commands::sync::run(&crate::commands::sync::SyncOptions {
        config_path: &config_path,
        dry_run: false,
        quiet: quiet > 0,
        as_json: false,
        plain,
        verbose: 0,
        scope_override: scope,
        update: false,
        update_only: Vec::new(),
        locked,
        allow_missing_secrets: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_at_ref_https_tag() {
        let (s, r) = split_at_ref("https://github.com/org/repo@v1.2.0");
        assert_eq!(s, "https://github.com/org/repo");
        assert_eq!(r.as_deref(), Some("v1.2.0"));
    }

    #[test]
    fn split_at_ref_userinfo_url_round_trips() {
        let (s, r) = split_at_ref("https://user@host.example/repo");
        assert_eq!(s, "https://user@host.example/repo");
        assert!(r.is_none());
    }

    #[test]
    fn split_at_ref_ssh_round_trips() {
        let (s, r) = split_at_ref("git@github.com:org/repo");
        assert_eq!(s, "git@github.com:org/repo");
        assert!(r.is_none());
    }

    #[test]
    fn split_at_ref_userinfo_and_ref() {
        let (s, r) = split_at_ref("https://user@host.example/repo@main");
        assert_eq!(s, "https://user@host.example/repo");
        assert_eq!(r.as_deref(), Some("main"));
    }

    #[test]
    fn split_at_ref_local_path_no_at() {
        let (s, r) = split_at_ref("./local/pack");
        assert_eq!(s, "./local/pack");
        assert!(r.is_none());
    }

    #[test]
    fn split_at_ref_trailing_at_ignored() {
        let (s, r) = split_at_ref("https://github.com/org/repo@");
        assert_eq!(s, "https://github.com/org/repo@");
        assert!(r.is_none());
    }
}
