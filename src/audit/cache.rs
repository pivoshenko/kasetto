//! Module that contains the on-disk TTL cache for audit lookups and the cached
//! GET every JSON provider goes through.
//!
//! Upstream verdicts are regenerated over time, so unlike the source cache this
//! one expires: each entry is trusted for 24h. A "no data" answer (skills.sh
//! 404, OSV with no advisories, deps.dev 404) is cached too, so an unaudited asset is not
//! re-probed on every run. `KASETTO_NO_CACHE` bypasses reads and writes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_cache, hash_str, http_client, now_unix};

/// Refresh a cached lookup at most once per 24h.
const TTL_SECS: u64 = 24 * 60 * 60;

/// Subdirectory of the kasetto cache holding every audit namespace.
const SUBDIR: &str = "audits";

/// Per-request budget for one audit lookup. A verdict is advisory and a failed
/// lookup degrades to a warning, so it gets less patience than a source fetch.
pub(super) const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    checked_at: u64,
    value: T,
}

fn disabled() -> bool {
    std::env::var_os("KASETTO_NO_CACHE").is_some_and(|v| !v.is_empty())
}

fn path(namespace: &str, key: &str) -> Option<PathBuf> {
    Some(
        dirs_kasetto_cache()
            .ok()?
            .join(SUBDIR)
            .join(namespace)
            .join(format!("{}.json", hash_str(key))),
    )
}

/// A fresh cached value for `key`, or `None` on a miss, an expired entry, or an
/// unreadable file.
pub(super) fn lookup<T: DeserializeOwned>(namespace: &str, key: &str) -> Option<T> {
    if disabled() {
        return None;
    }
    let text = fs::read_to_string(path(namespace, key)?).ok()?;
    let entry: Entry<T> = serde_json::from_str(&text).ok()?;
    if now_unix().saturating_sub(entry.checked_at) >= TTL_SECS {
        return None;
    }
    Some(entry.value)
}

/// Best-effort store: a write failure only means the next run fetches again.
pub(super) fn store<T: Serialize>(namespace: &str, key: &str, value: &T) {
    if disabled() {
        return;
    }
    let Some(path) = path(namespace, key) else {
        return;
    };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let entry = Entry {
        checked_at: now_unix(),
        value,
    };
    if let Ok(text) = serde_json::to_string(&entry) {
        // Two processes auditing the same key must not share a tmp file, or one
        // renames the other's half-written entry into place
        let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// Remove every cached audit lookup under `cache_root`; a missing directory is
/// already clean.
pub(super) fn clear_in(cache_root: &Path) -> io::Result<()> {
    match fs::remove_dir_all(cache_root.join(SUBDIR)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// GET `url` through the cache, keeping `project(raw)` rather than the raw
/// body so a provider can store only what it needs. `Ok(None)` is the
/// provider's `404` (not indexed, unknown package), cached like any other
/// answer. `label` names the provider in error messages.
pub(super) fn cached_get<R, T>(
    label: &str,
    namespace: &str,
    url: &str,
    refresh: bool,
    project: impl FnOnce(R) -> T,
) -> Result<Option<T>>
where
    R: DeserializeOwned,
    T: Serialize + DeserializeOwned,
{
    if !refresh {
        if let Some(hit) = lookup::<Option<T>>(namespace, url) {
            return Ok(hit);
        }
    }
    let resp = http_client()?
        .get(url)
        .timeout(LOOKUP_TIMEOUT)
        .send()
        .map_err(|e| err(format!("{label} request failed: {e}")))?;
    let status = resp.status();
    let found = if status.as_u16() == 404 {
        None
    } else if !status.is_success() {
        return Err(err(format!("{label} returned HTTP {status}")));
    } else {
        let text = resp
            .text()
            .map_err(|e| err(format!("failed to read {label} response: {e}")))?;
        let raw = serde_json::from_str::<R>(&text)
            .map_err(|e| err(format!("failed to parse {label} response: {e}")))?;
        Some(project(raw))
    };
    store(namespace, url, &found);
    Ok(found)
}

/// Percent-encode one URL path segment, so `@scope/name`, `github.com/owner/repo`,
/// or a skill name with a space travels as a single segment.
pub(super) fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;

    #[test]
    fn path_segments_are_percent_encoded() {
        assert_eq!(encode_segment("@acme/server"), "%40acme%2Fserver");
        assert_eq!(encode_segment("github.com/a/b"), "github.com%2Fa%2Fb");
        assert_eq!(encode_segment("1.0.0+build"), "1.0.0%2Bbuild");
        assert_eq!(encode_segment("my skill#1"), "my%20skill%231");
    }

    #[test]
    fn clearing_removes_only_the_audit_subdir_and_tolerates_absence() {
        let root = temp_dir("audit-cache-clear");
        fs::create_dir_all(root.join(SUBDIR).join("osv")).unwrap();
        fs::create_dir_all(root.join("sources")).unwrap();
        fs::write(root.join(SUBDIR).join("osv").join("x.json"), "{}").unwrap();

        clear_in(&root).unwrap();
        assert!(!root.join(SUBDIR).exists());
        assert!(root.join("sources").is_dir());

        clear_in(&root).unwrap();
        let _ = fs::remove_dir_all(&root);
    }
}
