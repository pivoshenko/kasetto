use std::fs;
use std::io::IsTerminal;

use sha2::{Digest, Sha256};

use crate::banner::print_banner_or_plain;
use crate::colors::{ACCENT, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::http_client;
use crate::profile::list_color_enabled;
use crate::ui::{animations_enabled, print_json, with_spinner, SYM_OK};

const GITHUB_REPO: &str = "pivoshenko/kasetto";

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
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

    if !as_json && std::io::stdout().is_terminal() {
        print_banner_or_plain(!color);
        println!();
    }

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
                "{SUCCESS}{SYM_OK}{RESET} Already on the latest version {ACCENT}{current_version}{RESET}"
            );
        } else {
            println!("{SYM_OK} Already on the latest version {current_version}");
        }
        return Ok(());
    }

    let target = current_target();
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(&target))
        .ok_or_else(|| err(format!("no release asset found for target: {target}")))?;

    let current_exe = std::env::current_exe()
        .map_err(|e| err(format!("failed to locate current executable: {e}")))?;

    let update_label = if color {
        format!(
            "Updating {}{}{} {}{}{} {}{}{}",
            ACCENT, current_version, RESET, SECONDARY, "→", RESET, ACCENT, latest_version, RESET,
        )
    } else {
        format!("Updating {} -> {}", current_version, latest_version)
    };

    let checksums_asset = release.assets.iter().find(|a| a.name == "checksums.txt");

    with_spinner(animate, !color, &update_label, || {
        self_replace(
            &asset.browser_download_url,
            &asset.name,
            checksums_asset.map(|a| a.browser_download_url.as_str()),
            &current_exe,
        )
    })?;

    let output = UpdateOutput {
        current_version: current_version.to_string(),
        latest_version: latest_version.to_string(),
        status: "updated".to_string(),
    };

    if as_json {
        print_json(&output)?;
    } else if color {
        println!("\n{SUCCESS}{SYM_OK}{RESET} Updated to {ACCENT}{latest_version}{RESET}");
    } else {
        println!("\n{SYM_OK} Updated to {latest_version}");
    }

    Ok(())
}

fn fetch_latest_release() -> Result<Release> {
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

fn is_newer(current: &str, latest: &str) -> bool {
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

fn self_replace(
    url: &str,
    asset_name: &str,
    checksums_url: Option<&str>,
    exe_path: &std::path::Path,
) -> Result<()> {
    let body = http_client()?
        .get(url)
        .send()?
        .error_for_status()
        .map_err(|e| err(format!("failed to download update: {e}")))?
        .bytes()?;

    // Verify SHA256 checksum when checksums.txt is available in the release
    if let Some(checksums_url) = checksums_url {
        let checksums_text = http_client()?
            .get(checksums_url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
            .map_err(|e| err(format!("failed to download checksums.txt: {e}")))?;

        verify_checksum(&body, asset_name, &checksums_text)?;
    }

    let gz = flate2::read::GzDecoder::new(body.as_ref());
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
