use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, Parser, Subcommand};

fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
        .usage(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
        .literal(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
        .placeholder(AnsiColor::Cyan.on_default())
}

#[derive(Parser)]
#[command(
    name = "kasetto",
    version,
    color = clap::ColorChoice::Always,
    args_conflicts_with_subcommands = true,
    styles = cli_styles(),
    about = "sync and maintain local AI skill packs",
    long_about = "An extremely fast AI skills manager, written in Rust.",
    after_help = "\x1b[1;35mExamples:\x1b[0m\n  \x1b[90mkasetto\x1b[0m\n  \x1b[90mkasetto --config skills.config.yaml --dry-run\x1b[0m\n  \x1b[90mkasetto sync --config https://example.com/skills.config.yaml --verbose\x1b[0m\n  \x1b[90mkasetto list\x1b[0m\n  \x1b[90mkasetto list --json\x1b[0m\n  \x1b[90mkasetto doctor\x1b[0m\n  \x1b[90mkasetto doctor --json\x1b[0m"
)]
pub struct Cli {
    #[command(flatten)]
    pub sync: SyncArgs,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Args, Clone, Debug, Default)]
pub struct SyncArgs {
    #[arg(long)]
    #[arg(
        help = "config path or HTTP(S) URL",
        long_help = "Configuration location. Supports:\n- local file path (default: skills.config.yaml)\n- HTTP(S) URL to a YAML config file"
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
}

impl SyncArgs {
    pub fn is_present(&self) -> bool {
        self.config.is_some()
            || self.dry_run
            || self.quiet
            || self.json
            || self.plain
            || self.verbose
    }
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(
        about = "Sync skills from configured sources",
        long_about = "Read configuration, discover requested skills, then install/update/remove local copies so destination matches config.\n\nUse --dry-run to preview changes without modifying files.",
        after_help = "\x1b[1;35mExamples:\x1b[0m\n  \x1b[90mkasetto sync\x1b[0m\n  \x1b[90mkasetto sync --dry-run --verbose\x1b[0m\n  \x1b[90mkasetto sync --config https://example.com/skills.config.yaml\x1b[0m"
    )]
    Sync {
        #[command(flatten)]
        sync: SyncArgs,
    },
    #[command(
        about = "List installed skills from manifest database",
        long_about = "Read installed skills from the local manifest database.\n\nIn interactive terminals, kasetto opens a navigable skill browser with a banner, list pane, and detail pane. Use --json for scripting."
    )]
    List {
        #[arg(long)]
        #[arg(help = "print installed skills as JSON")]
        json: bool,
    },
    #[command(
        about = "Run local diagnostics",
        long_about = "Inspect local kasetto setup, including version, manifest path, active installation paths, and failed skill installs from the latest sync report.",
        after_help = "\x1b[1;35mExamples:\x1b[0m\n  \x1b[90mkasetto doctor\x1b[0m\n  \x1b[90mkasetto doctor --json\x1b[0m"
    )]
    Doctor {
        #[arg(long)]
        #[arg(help = "print diagnostic output as JSON")]
        json: bool,
    },
}
