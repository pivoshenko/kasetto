use std::io::{self, IsTerminal, Write};
use std::{fs, path::PathBuf};

use crate::colors::{ACCENT, ATTENTION, ERROR, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_config, dirs_kasetto_data};

pub(crate) fn run(yes: bool) -> Result<()> {
    if !yes {
        if !io::stdin().is_terminal() {
            return Err(err(
                "pass --yes to confirm uninstall in non-interactive mode",
            ));
        }
        println!("{ATTENTION}This will remove kasetto, kst, and all installed assets.{RESET}");
        println!();
        print!("{ACCENT}Uninstall kasetto?{RESET} [y/N] ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !matches!(input.trim(), "y" | "Y" | "yes") {
            println!("{SECONDARY}Cancelled.{RESET}");
            return Ok(());
        }
        println!();
    }

    println!("{ACCENT}Removing installed assets...{RESET}");
    if let Err(e) = crate::commands::clean::run(false, false, true, false, None) {
        eprintln!("{ERROR}\x1b[1merror:{RESET} clean failed: {e}");
    }

    // 2. Remove $XDG_CONFIG_HOME/kasetto/ (saved config, MCP stubs, …)
    if let Ok(kasetto_config) = dirs_kasetto_config() {
        if kasetto_config.exists() {
            println!("{ACCENT}Removing {}...{RESET}", kasetto_config.display());
            fs::remove_dir_all(&kasetto_config).map_err(|e| {
                err(format!(
                    "failed to remove {}: {e}",
                    kasetto_config.display()
                ))
            })?;
        }
    }

    // 3. Remove $XDG_DATA_HOME/kasetto/ (lock file, …)
    if let Ok(kasetto_data) = dirs_kasetto_data() {
        if kasetto_data.exists() {
            println!("{ACCENT}Removing {}...{RESET}", kasetto_data.display());
            fs::remove_dir_all(&kasetto_data)
                .map_err(|e| err(format!("failed to remove {}: {e}", kasetto_data.display())))?;
        }
    }

    // 4. Remove binary and kst symlink
    let exe =
        std::env::current_exe().map_err(|e| err(format!("could not resolve binary path: {e}")))?;
    let install_dir = exe
        .parent()
        .ok_or_else(|| err("could not determine install directory"))?;

    remove_file_if_exists(&install_dir.join("kst"))?;
    remove_file_if_exists(&exe)?;

    println!();
    println!("{SUCCESS}\x1b[1mUninstalled{RESET} kasetto");
    Ok(())
}

fn remove_file_if_exists(path: &PathBuf) -> Result<()> {
    if path.exists() || path.symlink_metadata().is_ok() {
        println!("{ACCENT}Removing {}...{RESET}", path.display());
        fs::remove_file(path)
            .map_err(|e| err(format!("failed to remove {}: {e}", path.display())))?;
    }
    Ok(())
}
