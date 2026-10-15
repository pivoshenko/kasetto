//! Module that contains the background "new version available" notice.
//!
//! On startup, spawn a detached thread that fetches the latest released version
//! and writes it to a cache file. On the next run, if the cache says a newer
//! version exists, print a single yellow line at the end of the command.
//!
//! The check goes to `kasetto.dev/latest` rather than the GitHub API, so the
//! once-per-24h request doubles as the project's only active-install signal.
//! `self_update` still resolves release assets straight from GitHub. Set
//! `KASETTO_NO_UPDATE_CHECK` to opt out: no request is made, and an existing
//! cache is ignored rather than resurfacing as a notice.
//!
//! Best-effort: any failure (offline, rate-limit, IO) is silent.

use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::colors::{ACCENT, ATTENTION, RESET, SECONDARY};
use crate::commands::self_update::is_newer;
use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_cache, http_client};
use crate::profile::list_color_enabled;

const CACHE_FILE: &str = "update-check.json";
const LATEST_URL: &str = "https://kasetto.dev/latest";
/// Refresh cache at most once per 24h (matches npm/brew defaults).
const TTL_SECS: u64 = 24 * 60 * 60;

#[derive(serde::Deserialize)]
struct LatestVersion {
    tag_name: String,
}

/// Fetch the latest released version from `kasetto.dev`.
///
/// Deliberately separate from [`crate::commands::self_update::fetch_latest_release`]:
/// that one needs GitHub's asset list to download a binary, this one needs only
/// a version string.
fn fetch_latest_version() -> Result<String> {
    let text = http_client()?
        .get(LATEST_URL)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| err(format!("failed to fetch latest version: {e}")))?
        .error_for_status()
        .map_err(|e| err(format!("update check error: {e}")))?
        .text()
        .map_err(|e| err(format!("failed to read update check response: {e}")))?;
    let latest: LatestVersion = serde_json::from_str(&text)
        .map_err(|e| err(format!("failed to parse update check response: {e}")))?;
    Ok(latest.tag_name)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct CacheEntry {
    pub(crate) checked_at: u64,
    pub(crate) latest_version: String,
}

/// Handle for a pending background check; main thread can wait briefly so the
/// cache is populated before the notice is rendered. Detached threads die when
/// the main thread exits, so without this fast commands never persist results.
pub(crate) struct UpdateCheckHandle {
    rx: mpsc::Receiver<()>,
}

fn update_check_disabled() -> bool {
    std::env::var("KASETTO_NO_UPDATE_CHECK").is_ok_and(|v| !v.is_empty() && v != "0")
}

fn cache_path() -> Option<PathBuf> {
    // Test override
    if let Ok(dir) = std::env::var("KASETTO_CACHE_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir).join(CACHE_FILE));
        }
    }
    dirs_kasetto_cache().ok().map(|d| d.join(CACHE_FILE))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_cache(path: &std::path::Path) -> Option<CacheEntry> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(path: &std::path::Path, entry: &CacheEntry) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn cache_is_fresh(entry: &CacheEntry, now: u64) -> bool {
    now.saturating_sub(entry.checked_at) < TTL_SECS
}

/// Spawn background thread to refresh the update-check cache.
///
/// Returns `None` when the cache is fresh (no work to do) or when the cache
/// path can't be resolved. Otherwise returns a handle so the main thread can
/// optionally [`wait_for_check`] before rendering the notice.
pub(crate) fn spawn_background_check() -> Option<UpdateCheckHandle> {
    if update_check_disabled() {
        return None;
    }
    let path = cache_path()?;
    let now = now_secs();
    if let Some(entry) = read_cache(&path) {
        if cache_is_fresh(&entry, now) {
            return None;
        }
    }

    let (tx, rx) = mpsc::channel();
    // Best-effort: this detached thread only refreshes a cosmetic update-check cache, so any
    // panic here is intentionally isolated to the thread and never aborts the real command
    std::thread::spawn(move || {
        if let Ok(tag) = fetch_latest_version() {
            let entry = CacheEntry {
                checked_at: now_secs(),
                latest_version: tag.trim_start_matches('v').to_string(),
            };
            let _ = write_cache(&path, &entry);
        }
        let _ = tx.send(());
    });
    Some(UpdateCheckHandle { rx })
}

/// Block up to `timeout` waiting for the background check to finish.
///
/// Detached threads are killed when `main` returns, so fast commands need this
/// to give the HTTP request a chance to complete and persist its result. On
/// timeout we silently move on; the cache will be refreshed on a later run.
pub(crate) fn wait_for_check(handle: Option<UpdateCheckHandle>, timeout: Duration) {
    if let Some(h) = handle {
        let _ = h.rx.recv_timeout(timeout);
    }
}

/// Read the cached "latest version" for diagnostic display (e.g. `doctor`).
///
/// Returns `None` when the check is opted out, so a cache left over from before
/// the opt-out cannot resurface as a notice or a `doctor` verdict.
pub(crate) fn read_cached_entry() -> Option<CacheEntry> {
    if update_check_disabled() {
        return None;
    }
    let path = cache_path()?;
    read_cache(&path)
}

/// Current Unix timestamp; exposed so callers can compute cache age.
pub(crate) fn now_unix_secs() -> u64 {
    now_secs()
}

/// If the cache reports a newer version, print one yellow line.
///
/// Suppressed when stdout is not a TTY, or when `suppress` is true (e.g.
/// `--plain`/`--quiet`/`--json`, or commands that emit machine-readable output).
pub(crate) fn print_notice_if_available(suppress: bool) {
    if suppress || update_check_disabled() {
        return;
    }
    if !std::io::stdout().is_terminal() {
        return;
    }
    let Some(path) = cache_path() else {
        return;
    };
    let Some(entry) = read_cache(&path) else {
        return;
    };
    let current = env!("CARGO_PKG_VERSION");
    if !is_newer(current, &entry.latest_version) {
        return;
    }
    let line = render_notice(current, &entry.latest_version, list_color_enabled());
    println!("{line}");
}

fn render_notice(current: &str, latest: &str, color: bool) -> String {
    let cmd = upgrade_command();
    if color {
        format!(
            "\n{ACCENT}{ATTENTION}New version available:{RESET} {ACCENT}{current}{RESET} {SECONDARY}→{RESET} {ACCENT}{latest}{RESET}  {SECONDARY}run `{cmd}`{RESET}"
        )
    } else {
        format!("\nNew version available: {current} -> {latest}  run `{cmd}`")
    }
}

/// Best-guess install method based on the path of the running executable.
#[derive(Debug, PartialEq, Eq)]
enum InstallMethod {
    Homebrew,
    Cargo,
    Installer,
}

fn detect_install_method() -> InstallMethod {
    let exe = std::env::current_exe().ok();
    let lossy = exe.as_ref().map(|p| p.to_string_lossy());
    let path = lossy.as_deref().unwrap_or("");
    classify_install_path(path)
}

fn classify_install_path(path: &str) -> InstallMethod {
    if path.contains("/Cellar/")
        || path.contains("/opt/homebrew/")
        || path.contains("/Homebrew/")
        || path.contains("/linuxbrew/")
    {
        InstallMethod::Homebrew
    } else if path.contains("/.cargo/bin") || path.contains("/cargo/bin") {
        InstallMethod::Cargo
    } else {
        InstallMethod::Installer
    }
}

fn upgrade_command() -> &'static str {
    match detect_install_method() {
        InstallMethod::Homebrew => "brew upgrade kasetto",
        InstallMethod::Cargo => "cargo install kasetto",
        InstallMethod::Installer => "kst self update",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;

    #[test]
    fn cache_round_trip() {
        let dir = temp_dir("kasetto-notifier-rt");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(CACHE_FILE);
        let entry = CacheEntry {
            checked_at: 1_700_000_000,
            latest_version: "9.9.9".into(),
        };
        write_cache(&path, &entry).unwrap();
        let back = read_cache(&path).unwrap();
        assert_eq!(back.checked_at, 1_700_000_000);
        assert_eq!(back.latest_version, "9.9.9");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn opt_out_env_var_skips_the_check() {
        std::env::set_var("KASETTO_NO_UPDATE_CHECK", "1");
        assert!(update_check_disabled());
        // no network: the opt-out short-circuits before the thread is spawned
        assert!(spawn_background_check().is_none());

        std::env::set_var("KASETTO_NO_UPDATE_CHECK", "0");
        assert!(!update_check_disabled());

        std::env::remove_var("KASETTO_NO_UPDATE_CHECK");
        assert!(!update_check_disabled());
    }

    #[test]
    fn opt_out_ignores_a_populated_cache() {
        let dir = temp_dir("kasetto-notifier-optout");
        fs::create_dir_all(&dir).unwrap();
        std::env::set_var("KASETTO_CACHE_DIR", &dir);
        let entry = CacheEntry {
            checked_at: now_secs(),
            latest_version: "99.0.0".into(),
        };
        write_cache(&dir.join(CACHE_FILE), &entry).unwrap();

        std::env::remove_var("KASETTO_NO_UPDATE_CHECK");
        assert!(read_cached_entry().is_some());

        std::env::set_var("KASETTO_NO_UPDATE_CHECK", "1");
        assert!(read_cached_entry().is_none());

        std::env::remove_var("KASETTO_NO_UPDATE_CHECK");
        std::env::remove_var("KASETTO_CACHE_DIR");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fresh_cache_within_ttl() {
        let entry = CacheEntry {
            checked_at: 1_000_000,
            latest_version: "1.0.0".into(),
        };
        assert!(cache_is_fresh(&entry, 1_000_000 + TTL_SECS - 1));
        assert!(!cache_is_fresh(&entry, 1_000_000 + TTL_SECS));
    }

    #[test]
    fn render_notice_includes_versions_when_plain() {
        let s = render_notice("1.0.0", "2.0.0", false);
        assert!(s.contains("1.0.0"));
        assert!(s.contains("2.0.0"));
        assert!(!s.contains("\x1b["));
    }

    #[test]
    fn render_notice_uses_ansi_when_color() {
        let s = render_notice("1.0.0", "2.0.0", true);
        assert!(s.contains("\x1b["));
        assert!(s.contains("New version available"));
    }

    #[test]
    fn classify_install_path_detects_homebrew() {
        assert_eq!(
            classify_install_path("/opt/homebrew/bin/kasetto"),
            InstallMethod::Homebrew
        );
        assert_eq!(
            classify_install_path("/usr/local/Cellar/kasetto/2.8.1/bin/kasetto"),
            InstallMethod::Homebrew
        );
        assert_eq!(
            classify_install_path("/home/linuxbrew/.linuxbrew/bin/kasetto"),
            InstallMethod::Homebrew
        );
    }

    #[test]
    fn classify_install_path_detects_cargo() {
        assert_eq!(
            classify_install_path("/home/me/.cargo/bin/kasetto"),
            InstallMethod::Cargo
        );
    }

    #[test]
    fn classify_install_path_falls_back_to_installer() {
        assert_eq!(
            classify_install_path("/usr/local/bin/kasetto"),
            InstallMethod::Installer
        );
        assert_eq!(
            classify_install_path("/home/me/.local/bin/kasetto"),
            InstallMethod::Installer
        );
    }

    #[test]
    fn missing_cache_returns_none() {
        let dir = temp_dir("kasetto-notifier-missing");
        let path = dir.join(CACHE_FILE);
        assert!(read_cache(&path).is_none());
    }
}
