use clap::Parser;
use std::path::Path;

use crate::cli::{Cli, Commands, SelfAction, SyncArgs};
use crate::error::Result;
use crate::DEFAULT_CONFIG_FILENAME;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let program_name = current_program_name();
    match resolve_command(cli, Path::new(DEFAULT_CONFIG_FILENAME).exists()) {
        StartupMode::Command(command) => match command {
            Commands::Init { force } => crate::commands::init::run(force),
            Commands::Sync { sync } => {
                let config = sync
                    .config
                    .unwrap_or_else(|| DEFAULT_CONFIG_FILENAME.into());
                crate::commands::sync::run(&crate::commands::sync::SyncOptions {
                    config_path: &config,
                    dry_run: sync.dry_run,
                    quiet: sync.quiet,
                    as_json: sync.json,
                    plain: sync.plain,
                    verbose: sync.verbose,
                    scope_override: sync.scope.scope_override(),
                    show_banner: true,
                })
            }
            Commands::List { json, scope } => {
                crate::commands::list::run(json, scope.scope_override())
            }
            Commands::Doctor { json, scope } => {
                crate::commands::doctor::run(json, scope.scope_override(), &program_name)
            }
            Commands::Clean {
                dry_run,
                json,
                scope,
            } => crate::commands::clean::run(dry_run, json, false, scope.scope_override()),
            Commands::ManageSelf { action } => match action {
                SelfAction::Update { json } => crate::commands::self_update::run(json),
                SelfAction::Uninstall { yes } => crate::commands::uninstall::run(yes),
            },
            Commands::Completions { shell } => {
                crate::commands::completions::run(shell, &program_name)
            }
        },
        StartupMode::Home => crate::home::run(&program_name, DEFAULT_CONFIG_FILENAME),
    }
}

enum StartupMode {
    Command(Commands),
    Home,
}

fn resolve_command(cli: Cli, default_config_exists: bool) -> StartupMode {
    match (cli.command, cli.sync) {
        (Some(command), _) => StartupMode::Command(command),
        (None, sync) if sync.is_present() => StartupMode::Command(Commands::Sync { sync }),
        (None, _) if default_config_exists => StartupMode::Command(Commands::Sync {
            sync: SyncArgs::default(),
        }),
        (None, _) => StartupMode::Home,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_to_home_without_command_and_without_default_config() {
        // When no default config exists, should go to Home
        let cli = Cli {
            sync: SyncArgs::default(),
            command: None,
        };
        match resolve_command(cli, false) {
            StartupMode::Home => {}
            _ => panic!("expected Home startup mode"),
        }
    }

    #[test]
    fn resolves_to_default_sync_with_default_config() {
        let cli = Cli {
            sync: SyncArgs::default(),
            command: None,
        };
        match resolve_command(cli, true) {
            StartupMode::Command(Commands::Sync { sync }) => {
                assert_eq!(sync.config, None);
                assert!(!sync.dry_run);
                assert!(!sync.quiet);
                assert!(!sync.json);
                assert!(!sync.plain);
                assert!(!sync.verbose);
            }
            _ => panic!("expected default sync command"),
        }
    }

    #[test]
    fn resolves_root_sync_flags_to_sync_command() {
        let cli = Cli {
            sync: SyncArgs {
                config: Some("remote.yaml".into()),
                dry_run: true,
                quiet: false,
                json: false,
                plain: false,
                verbose: true,
                scope: Default::default(),
            },
            command: None,
        };
        match resolve_command(cli, false) {
            StartupMode::Command(Commands::Sync { sync }) => {
                assert_eq!(sync.config.as_deref(), Some("remote.yaml"));
                assert!(sync.dry_run);
                assert!(sync.verbose);
            }
            _ => panic!("expected root sync flags to resolve to sync"),
        }
    }
}
