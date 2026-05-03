use clap::Parser;
use std::path::Path;

use crate::cli::{Cli, Commands, SelfAction};
use crate::default_config_path;
use crate::error::Result;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let program_name = current_program_name();
    let default_config = default_config_path();
    match cli.command {
        Some(command) => match command {
            Commands::Init { force, global } => crate::commands::init::run(force, global),
            Commands::Sync { sync } => {
                let config = sync.config.unwrap_or_else(|| default_config.clone());
                crate::commands::sync::run(&crate::commands::sync::SyncOptions {
                    config_path: &config,
                    dry_run: sync.dry_run,
                    quiet: sync.quiet,
                    as_json: sync.json,
                    plain: sync.plain,
                    verbose: sync.verbose,
                    yes: sync.yes,
                    scope_override: sync.scope.scope_override(),
                    show_banner: true,
                })
            }
            Commands::List {
                json,
                output,
                scope,
            } => {
                crate::commands::list::run(json, output.plain, output.quiet, scope.scope_override())
            }
            Commands::Doctor {
                json,
                output,
                scope,
            } => crate::commands::doctor::run(
                json,
                output.plain,
                output.quiet,
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
                output.quiet,
                output.plain,
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
        None => crate::home::run(&program_name, &default_config),
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
