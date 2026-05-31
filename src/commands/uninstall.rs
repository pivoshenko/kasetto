use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::{fs};

use crate::colors::{ACCENT, ATTENTION, RESET, SECONDARY, SUCCESS};
use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_config, dirs_kasetto_data};
use crate::lock::load_lock;
use crate::model::resolve_scope;
use crate::ui::print_uninstall_closer;

pub(crate) fn run(yes: bool) -> Result<()> {
    if !yes {
        if !io::stdin().is_terminal() {
            return Err(err(
                "pass --yes to confirm uninstall in non-interactive mode",
            ));
        }
        println!("{ATTENTION}This will remove kasetto, kst, and all installed assets.{RESET}");
        println!();
        print!("{ACCENT}Uninstall kasetto?{RESET} {SECONDARY}[y/N]{RESET} ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !matches!(input.trim(), "y" | "Y" | "yes") {
            println!("{SECONDARY}Cancelled.{RESET}");
            return Ok(());
        }
        println!();
    }

    // Snapshot what's about to be removed so we can report it after silent cleanup.
    let scope = resolve_scope(None, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let counts = match load_lock(scope, &project_root) {
        Ok(lock) => count_assets(&lock),
        Err(_) => UninstallCounts::default(),
    };

    let _ = crate::commands::clean::run(false, false, true, false, None);

    let config_removed = remove_dir_if_exists(dirs_kasetto_config().ok().as_deref())?;
    let data_removed = remove_dir_if_exists(dirs_kasetto_data().ok().as_deref())?;

    let exe = std::env::current_exe()
        .map_err(|e| err(format!("could not resolve binary path: {e}")))?;
    let install_dir = exe
        .parent()
        .ok_or_else(|| err("could not determine install directory"))?;
    let kst_removed = remove_file_if_exists(&install_dir.join("kst"))?;
    let binary_removed = remove_file_if_exists(&exe)?;

    // Summary checklist — only show categories that actually had something.
    println!();
    if counts.skills > 0 {
        print_check(&format!("{} skills removed", counts.skills));
    }
    if counts.mcps > 0 {
        print_check(&format!("{} MCP servers removed", counts.mcps));
    }
    if counts.command_dirs > 0 {
        print_check(&format!("{} directories unlinked", counts.command_dirs));
    }
    if config_removed || data_removed {
        print_check("Lock file removed");
    }
    if binary_removed || kst_removed {
        print_check("Binary removed");
    }

    println!();
    let color = crate::profile::list_color_enabled();
    print_uninstall_closer(env!("CARGO_PKG_VERSION"), !color);
    Ok(())
}

#[derive(Default)]
struct UninstallCounts {
    skills: usize,
    mcps: usize,
    command_dirs: usize,
}

fn count_assets(lock: &crate::lock::LockFile) -> UninstallCounts {
    let skills = lock.state().skills.len();
    let mcps = lock.list_installed_mcps().len();
    let command_dirs: HashSet<PathBuf> = lock
        .assets
        .values()
        .filter(|a| a.kind == "command")
        .flat_map(|a| a.destination.split(','))
        .filter(|s| !s.is_empty())
        .filter_map(|p| Path::new(p).parent().map(Path::to_path_buf))
        .collect();
    UninstallCounts {
        skills,
        mcps,
        command_dirs: command_dirs.len(),
    }
}

/// Green `✓` + dim label — the cassette confirmation-row pattern. Used for
/// the uninstall summary checklist where every row is housekeeping.
fn print_check(msg: &str) {
    let color = crate::profile::list_color_enabled();
    if color {
        println!("{SUCCESS}✓{RESET} {SECONDARY}{msg}{RESET}");
    } else {
        println!("✓ {msg}");
    }
}

fn remove_dir_if_exists(path: Option<&Path>) -> Result<bool> {
    let Some(p) = path else { return Ok(false) };
    if !p.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(p).map_err(|e| err(format!("failed to remove {}: {e}", p.display())))?;
    Ok(true)
}

fn remove_file_if_exists(path: &PathBuf) -> Result<bool> {
    if path.exists() || path.symlink_metadata().is_ok() {
        fs::remove_file(path)
            .map_err(|e| err(format!("failed to remove {}: {e}", path.display())))?;
        return Ok(true);
    }
    Ok(false)
}
