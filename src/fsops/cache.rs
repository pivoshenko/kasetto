//! On-disk cache of extracted source trees, keyed by the resolved archive URL.
//!
//! Only **immutable** refs (explicit tag/SHA `ref:`) are cached: a moving ref
//! (branch/default) can change upstream without the URL changing, so caching it
//! would serve stale content. An immutable ref's URL fully determines its bytes,
//! so a hit is always correct — no TTL, no revalidation, zero network.
//!
//! Layout (`$XDG_CACHE_HOME/kasetto/sources/<sha256(url)>/`):
//! - `tree/`       — the extracted repository root (what `materialize` reads)
//! - `.complete`   — written last; its presence marks a fully-populated entry
//!
//! The marker lives *beside* `tree/`, never inside it, so it can never leak into
//! a skill's hashed/copied content. Population is atomic: extract into a private
//! `.tmp-*` sibling, then rename into place — a crash mid-extract leaves an
//! orphan tmp dir, never a half-populated (yet `.complete`) entry. Concurrent
//! population of the same key (parallel fetch) is a benign race: the loser reuses
//! the winner's entry.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::dirs::dirs_kasetto_cache;
use super::hash::hash_str;
use crate::error::Result;

const COMPLETE_MARKER: &str = ".complete";
const TREE_SUBDIR: &str = "tree";

/// Caching is opt-out via `KASETTO_NO_CACHE` (set to any non-empty value).
fn disabled() -> bool {
    std::env::var_os("KASETTO_NO_CACHE").is_some_and(|v| !v.is_empty())
}

fn sources_root() -> Result<PathBuf> {
    Ok(dirs_kasetto_cache()?.join("sources"))
}

fn entry_dir(root: &Path, url: &str) -> PathBuf {
    root.join(hash_str(url))
}

/// A complete cached tree for `url`, or `None` on a miss (or when caching is off).
pub(crate) fn lookup(url: &str) -> Option<PathBuf> {
    if disabled() {
        return None;
    }
    let entry = entry_dir(&sources_root().ok()?, url);
    if entry.join(COMPLETE_MARKER).is_file() {
        let tree = entry.join(TREE_SUBDIR);
        if tree.is_dir() {
            return Some(tree);
        }
    }
    None
}

/// Populate the cache for `url` by extracting into a private tmp tree via
/// `extract`, then atomically promote it. Returns the cached `tree/` path.
///
/// `extract(tree_dir)` must materialize the source root at `tree_dir`. On any
/// promotion race the existing complete entry wins and is returned instead.
/// Returns `None` when caching is disabled (caller falls back to direct extract).
pub(crate) fn store<F>(url: &str, extract: F) -> Option<Result<PathBuf>>
where
    F: FnOnce(&Path) -> Result<()>,
{
    if disabled() {
        return None;
    }
    Some(store_inner(url, extract))
}

fn store_inner<F>(url: &str, extract: F) -> Result<PathBuf>
where
    F: FnOnce(&Path) -> Result<()>,
{
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let root = sources_root()?;
    std::fs::create_dir_all(&root)?;
    let final_dir = entry_dir(&root, url);

    let nonce = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = root.join(format!(
        ".tmp-{}-{}-{}",
        hash_str(url),
        std::process::id(),
        nonce
    ));
    // Clear any stale tmp from a crashed run before reusing the path.
    let _ = std::fs::remove_dir_all(&tmp);
    let tmp_tree = tmp.join(TREE_SUBDIR);
    std::fs::create_dir_all(&tmp_tree)?;

    if let Err(e) = extract(&tmp_tree) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    std::fs::write(tmp.join(COMPLETE_MARKER), b"")?;

    // Atomic promote. If another worker already populated this key, our rename
    // fails (target non-empty) — discard our tmp and reuse the winner's entry.
    match std::fs::rename(&tmp, &final_dir) {
        Ok(()) => Ok(final_dir.join(TREE_SUBDIR)),
        Err(_) if final_dir.join(COMPLETE_MARKER).is_file() => {
            let _ = std::fs::remove_dir_all(&tmp);
            Ok(final_dir.join(TREE_SUBDIR))
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use std::fs;

    /// Point the cache at a throwaway `XDG_CACHE_HOME` for the duration of a test.
    /// Tests touching the same env var must hold this lock.
    struct CacheEnv {
        _dir: PathBuf,
    }

    fn with_cache_env(dir: &Path) -> CacheEnv {
        std::env::set_var("XDG_CACHE_HOME", dir);
        std::env::remove_var("KASETTO_NO_CACHE");
        CacheEnv {
            _dir: dir.to_path_buf(),
        }
    }

    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn miss_then_store_then_hit() {
        let _g = ENV_LOCK.lock().unwrap();
        let home = temp_dir("kasetto-cache-home");
        fs::create_dir_all(&home).unwrap();
        let _env = with_cache_env(&home);

        let url = "https://example.com/o/r/archive/v1.0.tar.gz";
        assert!(lookup(url).is_none(), "cold lookup misses");

        let tree = store(url, |dir| {
            fs::write(dir.join("SKILL.md"), "# hi\n")?;
            Ok(())
        })
        .expect("caching enabled")
        .expect("store ok");
        assert!(tree.join("SKILL.md").is_file());

        let hit = lookup(url).expect("warm lookup hits");
        assert_eq!(hit, tree);
        assert!(hit.join("SKILL.md").is_file());

        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn disabled_via_env_returns_none() {
        let _g = ENV_LOCK.lock().unwrap();
        let home = temp_dir("kasetto-cache-off");
        fs::create_dir_all(&home).unwrap();
        std::env::set_var("XDG_CACHE_HOME", &home);
        std::env::set_var("KASETTO_NO_CACHE", "1");

        let url = "https://example.com/o/r/archive/v2.0.tar.gz";
        assert!(lookup(url).is_none());
        assert!(
            store(url, |_| Ok(())).is_none(),
            "store is a no-op when off"
        );

        std::env::remove_var("KASETTO_NO_CACHE");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn failed_extract_leaves_no_complete_entry() {
        let _g = ENV_LOCK.lock().unwrap();
        let home = temp_dir("kasetto-cache-fail");
        fs::create_dir_all(&home).unwrap();
        let _env = with_cache_env(&home);

        let url = "https://example.com/o/r/archive/v3.0.tar.gz";
        let res = store(url, |_| Err(crate::error::err("boom"))).expect("enabled");
        assert!(res.is_err());
        assert!(lookup(url).is_none(), "a failed populate must not cache");

        let _ = fs::remove_dir_all(&home);
    }
}
