//! Module that contains the on-disk TTL cache for audit lookups.
//!
//! Upstream verdicts are regenerated over time, so unlike the source cache this
//! one expires: each entry is trusted for 24h. A "no data" answer (skills.sh
//! 404, OSV with no advisories, deps.dev 404) is cached too, so an unaudited asset is not
//! re-probed on every run. `KASETTO_NO_CACHE` bypasses reads and writes.

use std::fs;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::fsops::{dirs_kasetto_cache, hash_str, now_unix};

/// Refresh a cached lookup at most once per 24h.
const TTL_SECS: u64 = 24 * 60 * 60;

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
            .join("audits")
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
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}
