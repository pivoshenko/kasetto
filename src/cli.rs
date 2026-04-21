use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;

use crate::model::Scope;

#[derive(Parser)]
#[command(
    name = "kasetto",
    version,
    color = clap::ColorChoice::Always,
    styles = crate::colors::clap_styles(),
    about = "sync and maintain local AI skill packs",
    long_about = "A declarative AI agent environment manager, written in Rust.",
    after_help = crate::cli_examples!(
        "kasetto",
        "kasetto sync --config https://example.com/kasetto.yaml --verbose",
        "kasetto init",
        "kasetto list",
        "kasetto doctor",
    )
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Shared scope flags available on commands that operate on installed assets.
#[derive(Args, Clone, Debug, Default)]
pub(crate) struct ScopeArgs {
    #[arg(long, conflicts_with = "global")]
    #[arg(help = "install into the current project")]
    pub project: bool,
    #[arg(long, conflicts_with = "project")]
    #[arg(help = "install globally (default)")]
    pub global: bool,
}

impl ScopeArgs {
    /// CLI scope override (if any). Returns `None` when neither flag is set.
    pub(crate) fn scope_override(&self) -> Option<Scope> {
        if self.project {
            Some(Scope::Project)
        } else if self.global {
            Some(Scope::Global)
        } else {
            None
        }
    }
}

/// Shared output flags for commands that print to the terminal (matches `sync` where applicable).
#[derive(Args, Clone, Debug, Default)]
pub(crate) struct OutputArgs {
    #[arg(long)]
    #[arg(help = "suppress non-error output")]
    pub quiet: bool,
    #[arg(long)]
    #[arg(help = "disable colors and animations")]
    pub plain: bool,
}

#[derive(Args, Clone, Debug, Default)]
pub(crate) struct SyncArgs {
    #[arg(long)]
    #[arg(
        help = "config path or HTTP(S) URL",
        long_help = "Configuration location. Supports:\n- local file path\n- HTTP(S) URL to a YAML config file\n\nWhen omitted, kasetto checks defaults in this order:\n1) $KASETTO_CONFIG env var\n2) source: key in $XDG_CONFIG_HOME/kasetto/config.yaml\n3) ./kasetto.yaml\n4) $XDG_CONFIG_HOME/kasetto/kasetto.yaml (or ~/.config/kasetto/kasetto.yaml)"
    )]
    pub config: Option<String>,
    #[arg(long)]
    #[arg(help = "preview actions without changing files")]
    pub dry_run: bool,
    #[arg(long)]
    #[arg(help = "suppress non-error output")]
    pub quiet: bool,
    #[arg(long)]
    #[arg(help = "print final report as JSON")]
    pub json: bool,
    #[arg(long)]
    #[arg(help = "disable colors and animations")]
    pub plain: bool,
    #[arg(long)]
    #[arg(help = "print per-skill action list")]
    pub verbose: bool,
    #[arg(long)]
    #[arg(help = "skip confirmation prompt for new MCP servers")]
    pub yes: bool,
    #[command(flatten)]
    pub scope: ScopeArgs,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    #[command(
        about = "Create a starter config file",
        long_about = "Writes a commented template you can edit before running sync.\n\nBy default, writes ./kasetto.yaml. With --global, writes $XDG_CONFIG_HOME/kasetto/kasetto.yaml (or ~/.config/kasetto/kasetto.yaml).\n\nIf the target file already exists, you are prompted to overwrite (TTY) unless `--force` is set.",
        after_help = crate::cli_examples!(
            "kasetto init",
            "kasetto init --global",
            "kasetto init --force",
        )
    )]
    Init {
        #[arg(short, long)]
        #[arg(help = "overwrite an existing config file without prompting")]
        force: bool,
        #[arg(long)]
        #[arg(help = "write global config to $XDG_CONFIG_HOME/kasetto/kasetto.yaml")]
        global: bool,
    },
    #[command(
        about = "Sync skills from configured sources",
        long_about = "Read configuration, discover requested skills and MCPs, then install/update/remove local copies so destination matches config.\n\nUse --dry-run to preview changes without modifying files.",
        after_help = crate::cli_examples!(
            "kasetto sync",
            "kasetto sync --dry-run --verbose",
            "kasetto sync --config https://example.com/kasetto.yaml",
        )
    )]
    Sync {
        #[command(flatten)]
        sync: SyncArgs,
    },
    #[command(
        about = "List installed skills and MCPs",
        long_about = "Read installed skills and MCPs from the lock file.\n\nIn interactive terminals, kasetto opens a navigable browser with tabs for Skills and MCPs. Use --json for scripting.",
        after_help = crate::cli_examples!("kasetto list", "kasetto list --json",)
    )]
    List {
        #[arg(long)]
        #[arg(help = "print installed assets as JSON")]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    #[command(
        about = "Run local diagnostics",
        long_about = "Inspect local kasetto setup, including version, manifest path, active installation paths, MCP servers, and failed skill installs from the latest sync report.",
        after_help = crate::cli_examples!("kasetto doctor", "kasetto doctor --json",)
    )]
    Doctor {
        #[arg(long)]
        #[arg(help = "print diagnostic output as JSON")]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },

    #[command(
        about = "Remove installed skills and MCPs",
        long_about = "Remove all installed skills and MCP server configurations, resetting the lock file.",
        after_help = crate::cli_examples!("kasetto clean", "kasetto clean --dry-run",)
    )]
    Clean {
        #[arg(long)]
        #[arg(help = "preview what would be removed")]
        dry_run: bool,
        #[arg(long)]
        #[arg(help = "print output as JSON")]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    #[command(
        name = "self",
        about = "Manage this kasetto installation",
        long_about = "Update the running binary from GitHub releases, or uninstall kasetto and remove local config and data.",
        after_help = crate::cli_examples!(
            "kasetto self update",
            "kasetto self update --json",
            "kasetto self uninstall",
            "kasetto self uninstall --yes",
        )
    )]
    ManageSelf {
        #[command(subcommand)]
        action: SelfAction,
    },
    #[command(
        about = "Generate shell completions",
        long_about = "Generate shell completion scripts for kasetto.\n\nThe output is written to stdout so it can be sourced directly or redirected to a file.",
        after_help = crate::cli_examples!(
            "kasetto completions bash",
            "kasetto completions zsh",
            "kasetto completions fish",
            "kasetto completions powershell",
        )
    )]
    Completions {
        #[arg(help = "target shell")]
        shell: Shell,
    },
}

#[derive(Subcommand)]
pub(crate) enum SelfAction {
    #[command(
        about = "Update kasetto to the latest release",
        long_about = "Check GitHub for the latest kasetto release. If a newer version is available, download the matching binary and replace the current executable in-place.",
        after_help = crate::cli_examples!("kasetto self update", "kasetto self update --json",)
    )]
    Update {
        #[arg(long)]
        #[arg(help = "print update output as JSON")]
        json: bool,
    },
    #[command(
        about = "Completely uninstall kasetto",
        long_about = "Remove all installed assets, $XDG_CONFIG_HOME/kasetto/, $XDG_DATA_HOME/kasetto/, and the kasetto binary itself.",
        after_help = crate::cli_examples!("kasetto self uninstall", "kasetto self uninstall --yes",)
    )]
    Uninstall {
        #[arg(long)]
        #[arg(help = "skip confirmation prompt")]
        yes: bool,
    },
}
