use std::fs;

use crate::banner::print_banner_or_plain;
use crate::colors::{ACCENT, ERROR, RESET, SUCCESS, WARNING};
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config};
use crate::lock::{load_lock, save_lock};
use crate::mcps::remove_mcp_server;
use crate::model::{all_mcp_project_targets, all_mcp_settings_targets, resolve_scope, Scope};
use crate::profile::list_color_enabled;
use crate::ui::{animations_enabled, print_json, SYM_OK};

#[derive(serde::Serialize)]
struct CleanOutput {
    skills_removed: usize,
    mcps_removed: usize,
    commands_removed: usize,
    dry_run: bool,
}

pub(crate) fn run(
    dry_run: bool,
    as_json: bool,
    quiet: bool,
    plain: bool,
    scope_override: Option<Scope>,
) -> Result<()> {
    let animate = animations_enabled(quiet, as_json, plain);
    if !as_json && !quiet {
        print_banner_or_plain(plain || !animate);
    }

    let scope = resolve_scope(scope_override, None);
    let project_root = std::env::current_dir().unwrap_or_default();
    let mut lock = load_lock(scope, &project_root)?;

    let state = lock.state();
    let mcp_assets = lock.list_tracked_asset_ids("mcp");
    let command_assets = lock.list_tracked_asset_ids("command");

    let skills_count = state.skills.len();
    let mcps_count = mcp_assets.len();
    let commands_count = command_assets.len();

    if !dry_run {
        for entry in state.skills.values() {
            let _ = fs::remove_dir_all(&entry.destination);
        }

        let mcp_targets = match scope {
            Scope::Project => all_mcp_project_targets(&project_root),
            Scope::Global => {
                let home = dirs_home()?;
                let kasetto_config = dirs_kasetto_config()?;
                all_mcp_settings_targets(&home, &kasetto_config)
            }
        };
        for (_id, servers_csv) in &mcp_assets {
            for server_name in servers_csv.split(',').filter(|s| !s.is_empty()) {
                for target in &mcp_targets {
                    if target.path.exists() {
                        let _ = remove_mcp_server(server_name, target);
                    }
                }
            }
        }

        for (_id, destination_csv) in &command_assets {
            for destination in destination_csv.split(',').filter(|s| !s.is_empty()) {
                let _ = fs::remove_file(destination);
            }
        }

        lock.clear_all();
        save_lock(&lock, scope, &project_root)?;
    }

    let output = CleanOutput {
        skills_removed: skills_count,
        mcps_removed: mcps_count,
        commands_removed: commands_count,
        dry_run,
    };

    if as_json {
        print_json(&output)?;
    } else if !quiet {
        let color = list_color_enabled() && !plain;
        let (label_color, prefix) = if dry_run {
            (WARNING, "Would remove")
        } else {
            (ERROR, "Removed")
        };
        println!();
        println!(
            "  {label_color}{prefix}{RESET}: {}",
            skills_count + mcps_count + commands_count
        );

        if dry_run {
            println!();
            if !state.skills.is_empty() {
                println!("  Skills:");
                for entry in state.skills.values() {
                    if color {
                        println!(
                            "    {ACCENT}skill{RESET}  {}  ({})",
                            entry.destination, entry.skill
                        );
                    } else {
                        println!("    skill  {}  ({})", entry.destination, entry.skill);
                    }
                }
            }
            let mcp_packs: Vec<_> = lock
                .assets
                .iter()
                .filter(|(_, a)| a.kind == "mcp")
                .collect();
            if !mcp_packs.is_empty() {
                println!("  MCP packs (server names merged from kasetto):");
                for (_, a) in mcp_packs {
                    let servers: String = a
                        .destination
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if color {
                        println!(
                            "    {ACCENT}mcp{RESET}    {}  (pack: {}, source: {})",
                            servers, a.name, a.source
                        );
                    } else {
                        println!(
                            "    mcp    {}  (pack: {}, source: {})",
                            servers, a.name, a.source
                        );
                    }
                }
            }

            let command_files: Vec<_> = lock
                .assets
                .iter()
                .filter(|(_, a)| a.kind == "command")
                .collect();
            if !command_files.is_empty() {
                println!("  OpenCode commands:");
                for (_, a) in command_files {
                    let destinations: String = a
                        .destination
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if color {
                        println!(
                            "    {ACCENT}command{RESET} {}  (source: {})",
                            destinations, a.source
                        );
                    } else {
                        println!("    command {}  (source: {})", destinations, a.source);
                    }
                }
            }
        }

        if !dry_run {
            println!();
            println!("{SUCCESS}{SYM_OK}{RESET} Lock file reset.");
        } else {
            println!();
            println!("Run without {ACCENT}--dry-run{RESET} to apply.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsops::temp_dir;
    use crate::lock::{load_lock, save_lock, LockFile};
    use std::sync::{Mutex, OnceLock};

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn clean_removes_tracked_command_assets_in_project_scope() {
        let _guard = cwd_lock().lock().expect("lock cwd");
        let original_cwd = std::env::current_dir().expect("current dir");

        let project_root = temp_dir("kasetto-clean-commands");
        let command_dir = project_root.join(".opencode/commands");
        let command_file = command_dir.join("review.md");
        fs::create_dir_all(&command_dir).expect("create command dir");
        fs::write(&command_file, "---\n---\nReview\n").expect("write command file");

        let mut lock = LockFile::default();
        lock.save_tracked_asset(
            "command",
            "command::local::review",
            "review.md",
            "h1",
            "local",
            &command_file.to_string_lossy(),
        );
        save_lock(&lock, Scope::Project, &project_root).expect("save lock");

        std::env::set_current_dir(&project_root).expect("set current dir");
        run(false, false, true, true, Some(Scope::Project)).expect("clean run");
        std::env::set_current_dir(original_cwd).expect("restore current dir");

        assert!(!command_file.exists());
        let cleaned = load_lock(Scope::Project, &project_root).expect("load cleaned lock");
        assert!(cleaned.assets.is_empty());

        let _ = fs::remove_dir_all(&project_root);
    }
}
