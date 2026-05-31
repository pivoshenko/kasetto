use std::fs;
use sha2::{Digest, Sha256};

use crate::colors::{ACCENT, ATTENTION, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::http_client;
use crate::profile::list_color_enabled;
use crate::ui::{
    animations_enabled, print_json, print_update_closer, relativize_home, with_spinner,
    with_spinner_transient,
};

pub(crate) const GITHUB_REPO: &str = "pivoshenko/kasetto";

#[derive(serde::Deserialize)]
pub(crate) struct Release {
    pub(crate) tag_name: String,
    pub(crate) assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
pub(crate) struct Asset {
    pub(crate) name: String,
    pub(crate) browser_download_url: String,
}

#[derive(serde::Serialize)]
struct UpdateOutput {
    current_version: String,
    latest_version: String,
    status: String,
}

pub(crate) fn run(as_json: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let color = list_color_enabled();
    let animate = animations_enabled(false, as_json, !color);


    let release = with_spinner(animate, !color, "Checking for updates", || {
        fetch_latest_release()
    })?;

    let latest_version = release.tag_name.trim_start_matches('v');

    if !is_newer(current_version, latest_version) {
        let output = UpdateOutput {
            current_version: current_version.to_string(),
            latest_version: latest_version.to_string(),
            status: "up_to_date".to_string(),
        };
        if as_json {
            print_json(&output)?;
        } else if color {
            println!(
                "{SUCCESS}✓{RESET} {SUCCESS}{ACCENT}Audited{RESET} kasetto {ACCENT}{current_version}{RESET} (already latest)"
            );
        } else {
            println!("Audited kasetto {current_version} (already latest)");
        }
        return Ok(());
    }

    // `✓ Update available  X → Y` per design.
    if !as_json {
        if color {
            println!(
                "{SUCCESS}✓{RESET} Update available  {ATTENTION}{current_version} → {latest_version}{RESET}"
            );
            println!();
        } else {
            println!("Update available  {current_version} → {latest_version}");
            println!();
        }
    }

    let target = current_target();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(&target))
        .ok_or_else(|| err(format!("no release asset found for target: {target}")))?;

    let current_exe = std::env::current_exe()
        .map_err(|e| err(format!("failed to locate current executable: {e}")))?;

    let checksums_asset = release.assets.iter().find(|a| a.name == "checksums.txt");

    // Phase 1: download archive bytes.
    let body = with_spinner_transient(
        animate,
        !color,
        format!("Downloading kasetto {latest_version}"),
        || download_archive(&asset.browser_download_url),
    )?;
    print_step_done(
        &format!("Downloaded kasetto {latest_version}"),
        color,
        as_json,
    );

    // Phase 2: verify checksum.
    if let Some(checksums_asset) = checksums_asset {
        with_spinner_transient(animate, !color, "Verifying signature", || {
            let checksums_text = http_client()?
                .get(&checksums_asset.browser_download_url)
                .send()
                .and_then(|r| r.error_for_status())
                .and_then(|r| r.text())
                .map_err(|e| err(format!("failed to download checksums.txt: {e}")))?;
            verify_checksum(&body, &asset.name, &checksums_text)
        })?;
        print_step_done("Signature verified", color, as_json);
    }

    // Phase 3: install.
    with_spinner_transient(animate, !color, "Installing", || {
        install_from_archive(&body, &current_exe)
    })?;
    let exe_display = relativize_home(&current_exe.to_string_lossy());
    print_step_done(&format!("Installed to {exe_display}"), color, as_json);

    let output = UpdateOutput {
        current_version: current_version.to_string(),
        latest_version: latest_version.to_string(),
        status: "updated".to_string(),
    };

    if as_json {
        print_json(&output)?;
    } else {
        println!();
        print_update_closer(latest_version, current_version, !color);
        if color {
            println!(
                "  {SECONDARY}Run{RESET} {ATTENTION}kasetto --version{RESET} {SECONDARY}to confirm{RESET}"
            );
        } else {
            println!("  Run kasetto --version to confirm");
        }
    }

    Ok(())
}

/// `✓ message` — emit a single completed-step line per the design's update
/// flow. Skipped under `--json` (the JSON summary is the only output then).
fn print_step_done(message: &str, color: bool, as_json: bool) {
    if as_json {
        return;
    }
    if color {
        println!("{SUCCESS}✓{RESET} {message}");
    } else {
        println!("{message}");
    }
}

pub(crate) fn fetch_latest_release() -> Result<Release> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let text = http_client()?
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| err(format!("failed to fetch latest release: {e}")))?
        .error_for_status()
        .map_err(|e| err(format!("GitHub API error: {e}")))?
        .text()
        .map_err(|e| err(format!("failed to read release response: {e}")))?;
    let release: Release = serde_json::from_str(&text)
        .map_err(|e| err(format!("failed to parse release response: {e}")))?;
    Ok(release)
}

fn current_target() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match (arch, os) {
        ("aarch64", "macos") => "aarch64-apple-darwin".to_string(),
        ("x86_64", "macos") => "x86_64-apple-darwin".to_string(),
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".to_string(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".to_string(),
        _ => format!("{arch}-unknown-{os}"),
    }
}

pub(crate) fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> (u64, u64, u64) {
        let parts: Vec<u64> = v.split('.').filter_map(|s| s.parse().ok()).collect();
        (
            parts.first().copied().unwrap_or(0),
            parts.get(1).copied().unwrap_or(0),
            parts.get(2).copied().unwrap_or(0),
        )
    };
    parse(latest) > parse(current)
}

/// Download the release archive bytes from `url`.
fn download_archive(url: &str) -> Result<Vec<u8>> {
    let body = http_client()?
        .get(url)
        .send()?
        .error_for_status()
        .map_err(|e| err(format!("failed to download update: {e}")))?
        .bytes()?;
    Ok(body.to_vec())
}

/// Extract the archive into a tmp dir, replace `exe_path` with the new binary,
/// and back up the old one (restored on failure).
fn install_from_archive(body: &[u8], exe_path: &std::path::Path) -> Result<()> {
    let gz = flate2::read::GzDecoder::new(body);
    let mut archive = tar::Archive::new(gz);

    let tmp_dir = std::env::temp_dir().join(format!("kasetto-update-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir)?;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.to_string_lossy().contains("..") {
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(err("unsafe archive path"));
        }
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if file_name == "kasetto" || file_name == "kst" {
            let target = tmp_dir.join(&file_name);
            entry.unpack(&target)?;
        }
    }

    let new_binary = tmp_dir.join("kasetto");
    if !new_binary.exists() {
        let _ = fs::remove_dir_all(&tmp_dir);
        return Err(err("kasetto binary not found in release archive"));
    }

    let backup = exe_path.with_extension("old");
    fs::rename(exe_path, &backup)
        .map_err(|e| err(format!("failed to back up current binary: {e}")))?;

    match fs::copy(&new_binary, exe_path) {
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(exe_path, fs::Permissions::from_mode(0o755))?;
            }
            let _ = fs::remove_file(&backup);
        }
        Err(e) => {
            let _ = fs::rename(&backup, exe_path);
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(err(format!("failed to replace binary: {e}")));
        }
    }

    let _ = fs::remove_dir_all(&tmp_dir);
    Ok(())
}

/// Verify that the SHA256 of `data` matches the expected hash for `asset_name`
/// found in the checksums text (one `<hash>  <filename>` per line).
fn verify_checksum(data: &[u8], asset_name: &str, checksums_text: &str) -> Result<()> {
    let expected = checksums_text
        .lines()
        .find(|line| line.ends_with(asset_name))
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| {
            err(format!(
                "checksum not found for {asset_name} in checksums.txt"
            ))
        })?;

    let actual = format!("{:x}", Sha256::digest(data));
    if actual != expected {
        return Err(err(format!(
            "checksum mismatch for {asset_name}: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_detects_patch_bump() {
        assert!(is_newer("1.0.0", "1.0.1"));
    }

    #[test]
    fn is_newer_detects_minor_bump() {
        assert!(is_newer("1.0.0", "1.1.0"));
    }

    #[test]
    fn is_newer_detects_major_bump() {
        assert!(is_newer("1.0.0", "2.0.0"));
    }

    #[test]
    fn is_newer_returns_false_for_same_version() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn is_newer_returns_false_for_older_version() {
        assert!(!is_newer("2.0.0", "1.0.0"));
    }

    #[test]
    fn current_target_returns_nonempty_string() {
        let target = current_target();
        assert!(!target.is_empty());
    }

    #[test]
    fn verify_checksum_passes_on_match() {
        let data = b"hello world";
        let hash = format!("{:x}", Sha256::digest(data));
        let checksums = format!("{hash}  kasetto-aarch64-apple-darwin.tar.gz\n");
        verify_checksum(data, "kasetto-aarch64-apple-darwin.tar.gz", &checksums).unwrap();
    }

    #[test]
    fn verify_checksum_fails_on_mismatch() {
        let data = b"hello world";
        let checksums = "0000000000000000000000000000000000000000000000000000000000000000  kasetto-aarch64-apple-darwin.tar.gz\n";
        let result = verify_checksum(data, "kasetto-aarch64-apple-darwin.tar.gz", checksums);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("checksum mismatch"));
    }

    #[test]
    fn verify_checksum_fails_when_asset_not_in_checksums() {
        let data = b"hello world";
        let checksums = "abcdef1234567890  kasetto-x86_64-unknown-linux-gnu.tar.gz\n";
        let result = verify_checksum(data, "kasetto-aarch64-apple-darwin.tar.gz", checksums);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("checksum not found"));
    }

    #[test]
    fn verify_checksum_handles_multiple_entries() {
        let data = b"binary content";
        let hash = format!("{:x}", Sha256::digest(data));
        let checksums = format!(
            "aaaa  kasetto-x86_64-unknown-linux-gnu.tar.gz\n{hash}  kasetto-aarch64-apple-darwin.tar.gz\nbbbb  kasetto-x86_64-apple-darwin.tar.gz\n"
        );
        verify_checksum(data, "kasetto-aarch64-apple-darwin.tar.gz", &checksums).unwrap();
    }
}
