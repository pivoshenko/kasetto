use clap::{CommandFactory, Parser};
use clap_complete::generate;
use std::path::Path;

use crate::cli::{Cli, Commands, SyncArgs};
use crate::error::Result;

const DEFAULT_CONFIG: &str = "skills.config.yaml";

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let program_name = current_program_name();
    match resolve_command(cli, Path::new(DEFAULT_CONFIG).exists()) {
        StartupMode::Command(command) => match command {
            Commands::Sync { sync } => crate::commands::sync::run(
                &sync.config.unwrap_or_else(|| DEFAULT_CONFIG.into()),
                sync.dry_run,
                sync.quiet,
                sync.json,
                sync.plain,
                sync.verbose,
            ),
            Commands::List { json } => crate::commands::list::run(json),
            Commands::Doctor { json } => crate::commands::doctor::run(json),
            Commands::SelfUpdate { json } => crate::commands::self_update::run(json),
            Commands::Init { force } => crate::commands::init::run(force),
            Commands::Completions { shell } => {
                let mut cmd = Cli::command();
                let bin = program_name;
                generate(shell, &mut cmd, bin, &mut std::io::stdout());
                Ok(())
            }
        },
        StartupMode::Home => crate::home::run(&program_name, DEFAULT_CONFIG),
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
        let cli = Cli {
            sync: SyncArgs::default(),
            command: None,
        };
        assert!(matches!(resolve_command(cli, false), StartupMode::Home));
    }

    #[test]
    fn resolves_to_default_sync_without_command_and_with_default_config() {
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
