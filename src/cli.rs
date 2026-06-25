use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
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
    #[arg(short = 'q', long, action = ArgAction::Count, global = false)]
    #[arg(help = "suppress non-error output (repeat for stricter silence)")]
    pub quiet: u8,
    #[arg(long, value_name = "WHEN", default_value_t = ColorMode::Auto)]
    #[arg(help = "when to emit colors: auto, always, never")]
    pub color: ColorMode,
    #[arg(long, hide = true)]
    #[arg(help = "[deprecated] alias for --color never")]
    pub plain: bool,
}

impl OutputArgs {
    pub(crate) fn is_quiet(&self) -> bool {
        self.quiet > 0
    }

    /// Resolve color flags to an effective `plain` boolean.
    /// Sets `CLICOLOR_FORCE=1` for `--color always` and prints a one-line
    /// deprecation warning to stderr when the legacy `--plain` flag is used.
    pub(crate) fn resolve_plain(&self) -> bool {
        resolve_plain(self.plain, self.color)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub(crate) enum ListKind {
    #[default]
    All,
    Skills,
    Mcps,
    Commands,
    Instructions,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub(crate) enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl std::fmt::Display for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ColorMode::Auto => "auto",
            ColorMode::Always => "always",
            ColorMode::Never => "never",
        })
    }
}

#[derive(Args, Clone, Debug, Default)]
pub(crate) struct SyncArgs {
    #[arg(long)]
    #[arg(
        help = "config path or HTTP(S) URL",
        long_help = "Configuration location. Supports:\n- local file path\n- HTTP(S) URL to a YAML config file\n\nWhen omitted, kasetto checks defaults in this order:\n1) $KASETTO_CONFIG env var\n2) ./kasetto.yaml\n3) source: key in $XDG_CONFIG_HOME/kasetto/config.yaml\n4) $XDG_CONFIG_HOME/kasetto/kasetto.yaml (or ~/.config/kasetto/kasetto.yaml)"
    )]
    pub config: Option<String>,
    #[arg(long)]
    #[arg(help = "preview actions without changing files")]
    pub dry_run: bool,
    #[arg(short = 'q', long, action = ArgAction::Count)]
    #[arg(help = "suppress non-error output (repeat for stricter silence)")]
    pub quiet: u8,
    #[arg(long)]
    #[arg(help = "print final report as JSON")]
    pub json: bool,
    #[arg(long, value_name = "WHEN", default_value_t = ColorMode::Auto)]
    #[arg(help = "when to emit colors: auto, always, never")]
    pub color: ColorMode,
    #[arg(long, hide = true)]
    #[arg(help = "[deprecated] alias for --color never")]
    pub plain: bool,
    #[arg(short = 'v', long, action = ArgAction::Count)]
    #[arg(help = "increase output detail (-v, -vv, -vvv)")]
    pub verbose: u8,
    #[arg(long, short = 'u', num_args = 0.., value_name = "NAME")]
    #[arg(
        help = "re-resolve branch/default sources and rewrite locked hashes",
        long_help = "Re-resolve moving refs (branches/default HEAD) and rewrite the locked hash + revision.\n\nWith no value (--update), updates every source. Pass one or more skill names (--update foo bar) to re-resolve only the sources providing those skills; all other sources are honored from the lock.\n\nUpdating a skill from a multi-skill source re-resolves that whole source."
    )]
    pub update: Option<Vec<String>>,
    #[arg(long, visible_alias = "frozen")]
    #[arg(help = "fail if the lock cannot satisfy the config; never fetch (CI-friendly)")]
    pub locked: bool,
    #[arg(long)]
    #[arg(
        help = "warn instead of failing when a ${KST_…} secret can't be resolved",
        long_help = "By default an unresolved `${KST_NAME}` placeholder in an MCP config marks that entry broken and exits non-zero. With this flag, kasetto warns and writes the literal placeholder instead."
    )]
    pub allow_missing_secrets: bool,
    #[command(flatten)]
    pub scope: ScopeArgs,
}

impl SyncArgs {
    /// `true` when `--update` was passed (with or without names).
    pub(crate) fn update_active(&self) -> bool {
        self.update.is_some()
    }

    /// The explicit skill names passed to `--update <name>...` (empty for `--update` alone).
    pub(crate) fn update_only(&self) -> Vec<String> {
        self.update.clone().unwrap_or_default()
    }

    pub(crate) fn is_quiet(&self) -> bool {
        self.quiet > 0
    }

    pub(crate) fn verbosity(&self) -> u8 {
        self.verbose
    }

    pub(crate) fn resolve_plain(&self) -> bool {
        resolve_plain(self.plain, self.color)
    }
}

/// Apply color-flag side effects (CLICOLOR_FORCE for `always`, deprecation
/// warning for the legacy `--plain`) and return the effective `plain` value.
fn resolve_plain(plain_flag: bool, color: ColorMode) -> bool {
    if plain_flag {
        crate::ui::eprint_warn(
            "--plain is deprecated; use --color never instead",
            color == ColorMode::Never,
        );
    }
    match color {
        ColorMode::Always => {
            std::env::set_var("CLICOLOR_FORCE", "1");
            plain_flag
        }
        ColorMode::Never => true,
        ColorMode::Auto => plain_flag,
    }
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
            "kasetto sync --update",
            "kasetto sync --locked",
            "kasetto sync --dry-run --verbose",
            "kasetto sync --config https://example.com/kasetto.yaml",
        )
    )]
    Sync {
        #[command(flatten)]
        sync: SyncArgs,
    },
    #[command(
        about = "Add a source to the config and sync it",
        long_about = "Append a skill/MCP/command/instruction source to your local kasetto.yaml (preserving comments), then run a sync to install it.\n\nUse the kind-tagged flags --skill / --mcp / --command / --instruction (each repeatable) to name entries; a single add can touch several lists at once when a repo ships more than one kind. A lone `*` value (e.g. --skill \"*\") is a wildcard. With no kind flags, the source is added as `skills: \"*\"`.\n\nThe source may be a repo URL or a deep blob/tree browse URL — the latter is decomposed into source + ref/branch + sub-dir (and the skill name for a SKILL.md link). Explicit --ref / --branch / --sub-dir override the derived pieces.\n\nThe source is fetched once up front to verify it resolves (skip with --no-verify). Use --no-sync to edit the config without installing.",
        after_help = crate::cli_examples!(
            "kasetto add https://github.com/example/skill-pack",
            "kasetto add https://github.com/example/pack@v2.0",
            "kasetto add https://github.com/example/pack --skill alpha --skill beta",
            "kasetto add https://github.com/example/pack --skill find --mcp github --command review",
            "kasetto add https://github.com/org/repo/blob/main/skills/personal/edit-article/SKILL.md",
            "kasetto add https://github.com/example/pack --dry-run",
            "kasetto add https://github.com/example/pack --ref v2.0 --no-sync",
        )
    )]
    Add {
        #[arg(value_name = "SOURCE")]
        #[arg(help = "repo URL, deep blob/tree URL, local path, or SOURCE@REF shorthand")]
        source: String,
        #[arg(long = "skill", value_name = "NAME")]
        #[arg(help = "skill name to add (repeatable; \"*\" for all)")]
        skill: Vec<String>,
        #[arg(long = "mcp", value_name = "NAME")]
        #[arg(help = "MCP name to add (repeatable; \"*\" for all)")]
        mcp: Vec<String>,
        #[arg(long = "command", value_name = "NAME")]
        #[arg(help = "command name to add (repeatable; \"*\" for all)")]
        command: Vec<String>,
        #[arg(long = "instruction", value_name = "NAME")]
        #[arg(help = "instruction name to add (repeatable; \"*\" for all)")]
        instruction: Vec<String>,
        #[arg(long = "ref", value_name = "REF", conflicts_with = "branch")]
        #[arg(help = "pin to a git tag, commit SHA, or ref")]
        git_ref: Option<String>,
        #[arg(long, value_name = "BRANCH", conflicts_with = "git_ref")]
        #[arg(help = "track a specific branch")]
        branch: Option<String>,
        #[arg(long = "sub-dir", value_name = "DIR")]
        #[arg(help = "subdirectory inside the source to use as the root")]
        sub_dir: Option<String>,
        #[arg(long, value_name = "PATH")]
        #[arg(help = "config file to edit (default: ./kasetto.yaml)")]
        config: Option<String>,
        #[arg(long = "no-verify")]
        #[arg(help = "skip the upfront fetch that validates the source")]
        no_verify: bool,
        #[arg(long = "no-sync")]
        #[arg(help = "edit the config without installing")]
        no_sync: bool,
        #[arg(long)]
        #[arg(help = "preview edits without writing the config or syncing")]
        dry_run: bool,
        #[arg(long, visible_alias = "frozen")]
        #[arg(help = "during the follow-up sync, never fetch; honor the lock")]
        locked: bool,
        #[arg(long)]
        #[arg(help = "print the edit summary as JSON")]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    #[command(
        visible_alias = "rm",
        about = "Remove a source or named entries from the config and prune them",
        long_about = "Delete entries from your local kasetto.yaml (preserving comments), then run a sync so the now-unconfigured assets are removed from disk and the lock.\n\nMirrors `add`: the kind-tagged flags --skill / --mcp / --command / --instruction (each repeatable) name entries to subtract from a list; when the last name goes, the whole entry is dropped. A lone `*` value (e.g. --mcp \"*\") drops that kind's whole entry. With no kind flags, the source is removed from every list it appears in.\n\nThe source may be a repo URL or a deep blob/tree browse URL. When multiple entries share a source URL, pass --ref or --branch to pick one. Use --no-sync to edit the config without pruning.",
        after_help = crate::cli_examples!(
            "kasetto remove https://github.com/example/skill-pack",
            "kasetto remove https://github.com/example/pack@v2.0",
            "kasetto remove https://github.com/example/pack --skill find-skills",
            "kasetto remove https://github.com/example/repo --mcp github --command review",
            "kasetto remove https://github.com/example/pack --mcp \"*\"",
            "kasetto remove https://github.com/example/pack --dry-run",
            "kasetto rm ./local/pack --no-sync",
        )
    )]
    Remove {
        #[arg(value_name = "SOURCE")]
        #[arg(help = "repo URL, deep blob/tree URL, local path, or SOURCE@REF shorthand")]
        source: String,
        #[arg(long = "skill", value_name = "NAME")]
        #[arg(help = "skill name to remove (repeatable; \"*\" drops the whole entry)")]
        skill: Vec<String>,
        #[arg(long = "mcp", value_name = "NAME")]
        #[arg(help = "MCP name to remove (repeatable; \"*\" drops the whole entry)")]
        mcp: Vec<String>,
        #[arg(long = "command", value_name = "NAME")]
        #[arg(help = "command name to remove (repeatable; \"*\" drops the whole entry)")]
        command: Vec<String>,
        #[arg(long = "instruction", value_name = "NAME")]
        #[arg(help = "instruction name to remove (repeatable; \"*\" drops the whole entry)")]
        instruction: Vec<String>,
        #[arg(long = "ref", value_name = "REF", conflicts_with = "branch")]
        #[arg(help = "disambiguate by pinned ref")]
        git_ref: Option<String>,
        #[arg(long, value_name = "BRANCH", conflicts_with = "git_ref")]
        #[arg(help = "disambiguate by tracked branch")]
        branch: Option<String>,
        #[arg(long = "sub-dir", value_name = "PATH")]
        #[arg(help = "disambiguate by sub-dir (or pass a deep blob/tree URL)")]
        sub_dir: Option<String>,
        #[arg(long, value_name = "PATH")]
        #[arg(help = "config file to edit (default: ./kasetto.yaml)")]
        config: Option<String>,
        #[arg(long = "no-sync")]
        #[arg(help = "edit the config without pruning installed assets")]
        no_sync: bool,
        #[arg(long)]
        #[arg(help = "preview edits without writing the config or pruning")]
        dry_run: bool,
        #[arg(long, visible_alias = "frozen")]
        #[arg(help = "during the follow-up sync, never fetch; honor the lock")]
        locked: bool,
        #[arg(long)]
        #[arg(help = "print the edit summary as JSON")]
        json: bool,
        #[command(flatten)]
        output: OutputArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    #[command(
        about = "Resolve the config and pin it into kasetto.lock",
        long_about = "Re-resolve every source (re-resolving moving refs like `--update`) and write kasetto.lock, without installing to destinations.\n\nSkills are hashed from the source tree — identical to the hash a later sync computes at the destination — so the lock is immediately usable with `sync --locked`. MCP/command entries get refreshed revision pins; their content hash fills in on the next sync.",
        after_help = crate::cli_examples!(
            "kasetto lock",
            "kasetto lock --check",
            "kasetto lock --upgrade-package alpha --upgrade-package beta",
            "kasetto lock --project",
            "kasetto lock --config https://example.com/kasetto.yaml",
        )
    )]
    Lock {
        #[arg(long, value_name = "PATH")]
        #[arg(help = "config path or HTTP(S) URL")]
        config: Option<String>,
        #[arg(long)]
        #[arg(help = "print the result as JSON")]
        json: bool,
        #[arg(long, visible_aliases = ["locked", "frozen"])]
        #[arg(help = "verify the lock matches the config without writing (exit 1 on drift)")]
        check: bool,
        #[arg(
            long = "upgrade-package",
            short = 'P',
            value_name = "NAME",
            num_args = 1..,
        )]
        #[arg(
            help = "only re-resolve sources providing these skills",
            long_help = "Restrict the re-resolve to sources whose skill list (per the existing lock) overlaps NAME. Every other source's lock entries are carried over unchanged. Mirrors `sync --update <name>...`."
        )]
        upgrade_package: Vec<String>,
        #[command(flatten)]
        output: OutputArgs,
        #[command(flatten)]
        scope: ScopeArgs,
    },
    #[command(
        about = "List installed skills, MCPs, commands, and instructions",
        long_about = "Read installed assets from the lock file and print them as plain tables.\n\nFilter the output with `--type skills|mcps|commands|instructions|all` (default: all). Use --json for scripting.",
        after_help = crate::cli_examples!(
            "kasetto list",
            "kasetto list --type skills",
            "kasetto list --json",
        )
    )]
    List {
        #[arg(long)]
        #[arg(help = "print installed assets as JSON")]
        json: bool,
        #[arg(long = "type", value_enum, default_value_t = ListKind::All)]
        #[arg(help = "limit output to one asset kind")]
        kind: ListKind,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_sync(args: &[&str]) -> SyncArgs {
        let cli = Cli::try_parse_from(args).expect("parse");
        match cli.command {
            Some(Commands::Sync { sync }) => sync,
            _ => panic!("expected sync command"),
        }
    }

    #[test]
    fn update_absent_is_inactive() {
        let sync = parse_sync(&["kasetto", "sync"]);
        assert!(!sync.update_active());
        assert!(sync.update_only().is_empty());
    }

    #[test]
    fn update_flag_alone_is_active_with_no_names() {
        let sync = parse_sync(&["kasetto", "sync", "--update"]);
        assert!(sync.update_active());
        assert!(sync.update_only().is_empty());
    }

    #[test]
    fn update_with_names_is_active_and_selective() {
        let sync = parse_sync(&["kasetto", "sync", "--update", "foo", "bar"]);
        assert!(sync.update_active());
        assert_eq!(
            sync.update_only(),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn frozen_aliases_locked() {
        let sync = parse_sync(&["kasetto", "sync", "--frozen"]);
        assert!(sync.locked);
    }
}
