use clap::{CommandFactory, Parser};
use std::path::Path;
use std::time::Duration;

use crate::banner::print_banner;
use crate::cli::{Cli, Commands, SelfAction};
use crate::default_config_path;
use crate::error::Result;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let program_name = current_program_name();
    let default_config = default_config_path();

    let update_handle = crate::update_notifier::spawn_background_check();
    let suppress_notice = should_suppress_notice(&cli.command);

    // Wait briefly so the cache is fresh before commands like `doctor` read it
    // and before we render the end-of-run notice. Suppressed paths skip this so
    // scripted output stays fast.
    if !suppress_notice {
        crate::update_notifier::wait_for_check(update_handle, Duration::from_millis(800));
    }

    let result = match cli.command {
        Some(command) => match command {
            Commands::Init { force, global } => crate::commands::init::run(force, global),
            Commands::Sync { sync } => {
                let update = sync.update_active();
                let update_only = sync.update_only();
                let quiet = sync.is_quiet();
                let verbose = sync.verbosity();
                let plain = sync.resolve_plain();
                let config = sync.config.unwrap_or_else(|| default_config.clone());
                crate::commands::sync::run(&crate::commands::sync::SyncOptions {
                    config_path: &config,
                    dry_run: sync.dry_run,
                    quiet,
                    as_json: sync.json,
                    plain,
                    verbose,
                    scope_override: sync.scope.scope_override(),
                    update,
                    update_only,
                    locked: sync.locked,
                })
            }
            Commands::Add {
                source,
                skill,
                mcp,
                command,
                git_ref,
                branch,
                sub_dir,
                config,
                no_verify,
                no_sync,
                dry_run,
                locked,
                json,
                output,
                scope,
            } => crate::commands::add::run(&crate::commands::add::AddOptions {
                source: &source,
                skills: &skill,
                mcps: &mcp,
                commands: &command,
                git_ref: git_ref.as_deref(),
                branch: branch.as_deref(),
                sub_dir: sub_dir.as_deref(),
                config: config.as_deref(),
                scope_override: scope.scope_override(),
                no_verify,
                no_sync,
                dry_run,
                locked,
                as_json: json,
                quiet: output.quiet,
                plain: output.resolve_plain(),
            }),
            Commands::Remove {
                source,
                skill,
                mcp,
                command,
                git_ref,
                branch,
                sub_dir,
                config,
                no_sync,
                dry_run,
                locked,
                json,
                output,
                scope,
            } => crate::commands::remove::run(&crate::commands::remove::RemoveOptions {
                source: &source,
                skills: &skill,
                mcps: &mcp,
                commands: &command,
                git_ref: git_ref.as_deref(),
                branch: branch.as_deref(),
                sub_dir: sub_dir.as_deref(),
                config: config.as_deref(),
                scope_override: scope.scope_override(),
                no_sync,
                dry_run,
                locked,
                as_json: json,
                quiet: output.quiet,
                plain: output.resolve_plain(),
            }),
            Commands::Lock {
                config,
                json,
                check,
                upgrade_package,
                output,
                scope,
            } => {
                // Apply color side effects (CLICOLOR_FORCE for `--color always`);
                // `lock` renders via `color_stdout_enabled()`, not a plain flag.
                let _ = output.resolve_plain();
                crate::commands::lock::run(&crate::commands::lock::LockOptions {
                    config: config.as_deref(),
                    scope_override: scope.scope_override(),
                    as_json: json,
                    quiet: output.quiet,
                    check,
                    upgrade_only: upgrade_package,
                })
            }
            Commands::List {
                json,
                kind,
                output,
                scope,
            } => crate::commands::list::run(
                json,
                kind,
                output.resolve_plain(),
                output.is_quiet(),
                scope.scope_override(),
            ),
            Commands::Doctor {
                json,
                output,
                scope,
            } => crate::commands::doctor::run(
                json,
                output.resolve_plain(),
                output.is_quiet(),
                scope.scope_override(),
                &program_name,
            ),
            Commands::Clean {
                dry_run,
                json,
                output,
                scope,
            } => crate::commands::clean::run(
                dry_run,
                json,
                output.is_quiet(),
                output.resolve_plain(),
                scope.scope_override(),
            ),
            Commands::ManageSelf { action } => match action {
                SelfAction::Update { json } => crate::commands::self_update::run(json),
                SelfAction::Uninstall { yes } => crate::commands::uninstall::run(yes),
            },
            Commands::Completions { shell } => {
                crate::commands::completions::run(shell, &program_name)
            }
        },
        None => {
            print_banner();
            let _ = Cli::command().print_help();
            println!();
            Ok(())
        }
    };

    if result.is_ok() {
        crate::update_notifier::print_notice_if_available(suppress_notice);
    }
    result
}

/// Suppress the update notice for machine-readable / scripted output and for
/// commands that already print version info.
fn should_suppress_notice(command: &Option<Commands>) -> bool {
    match command {
        Some(Commands::Sync { sync }) => sync.json || sync.plain || sync.is_quiet(),
        Some(Commands::Add { json, output, .. }) => *json || output.plain || output.is_quiet(),
        Some(Commands::Remove { json, output, .. }) => *json || output.plain || output.is_quiet(),
        Some(Commands::Lock { json, output, .. }) => *json || output.plain || output.is_quiet(),
        Some(Commands::List { json, output, .. }) => *json || output.plain || output.is_quiet(),
        Some(Commands::Doctor { json, output, .. }) => *json || output.plain || output.is_quiet(),
        Some(Commands::Clean { json, output, .. }) => *json || output.plain || output.is_quiet(),
        Some(Commands::Completions { .. }) => true,
        Some(Commands::ManageSelf { .. }) => true,
        Some(Commands::Init { .. }) => false,
        None => false,
    }
}

fn current_program_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|arg| {
            Path::new(&arg)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "kasetto".to_string())
}
