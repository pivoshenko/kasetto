# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
just check          # format + lint + test + build (full validation)
just format         # cargo fmt
just lint           # cargo clippy -- -D warnings
just test           # cargo test
just build          # cargo build --release
cargo test <name>   # run a single test by name
```

The project forbids `unsafe` code and warns on `dbg!` and `todo!` (see `[lints]` in `Cargo.toml`).

## Architecture

Kasetto is a single-binary CLI tool that syncs AI agent skills from GitHub repos or local directories into 21 agent environments. Two binaries (`kasetto` and `kst`) share the same code.

### Startup Routing (`app.rs`)

```
CLI args → match cli.command
  ├─ Explicit subcommand → run that command
  └─ None → Home screen (interactive TUI menu)
```

### Module Layout

- **`commands/`** - Each subcommand: `sync/` (split into `skills.rs` + `mcps.rs`), `list`, `doctor`, `init`, `clean`, `self_update`, `uninstall`, `completions`
- **`model/`** - Core types: `Agent` enum (21 presets with install paths), `Config` (YAML deserialization), `Scope` (Global/Project), `SkillEntry`, `Report`, `Summary`
- **`source/`** - Remote handling: URL parsing (`parse.rs`), archive download/extraction (`remote.rs`), auth token resolution (`auth.rs`), git host URL rewriting (`hosts.rs`)
- **`fsops/`** - File operations: config loading from file/HTTP (`mod.rs`), path resolution, SHA256 hashing (`hash.rs`), recursive copy (`copy.rs`), XDG dirs (`dirs.rs`), HTTP client (`http.rs`), settings file I/O (`settings.rs`)
- **`mcps/`** - MCP server management: pack discovery (`pack.rs`), format-aware merging (`merge.rs`), Codex TOML handling (`codex.rs`). Supports 4 formats: McpServers JSON, VsCode servers JSON, OpenCode JSON, Codex TOML
- **`lock.rs`** - Manifest persistence: tracks installed skills + MCP assets, stores latest sync report as JSON blob
- **`home/`** - Interactive welcome screen with `prompt.rs` for sync arg input
- **`list/`** - Interactive TUI browser: `browse.rs` (event loop), `render.rs` (frame drawing), `session.rs` (state/guard), `tab.rs`, `types.rs`

### Sync Data Flow

1. Load config from file or HTTP URL (with GitLab/GitHub/Gitea auth via env vars)
2. Resolve scope (CLI flag → config field → default Global) and destination paths per agent
3. For each skill source: materialize (download if remote) → discover available skills → select targets → hash → copy → update lock state
4. For each MCP source: materialize → discover/resolve packs → collect pending installs → prompt for confirmation if new servers found (unless `--yes`) → merge into agent settings files → update lock
5. Save lock file and report (unless `--dry-run`)

### UI System

**Color palette** (`colors.rs`): Semantic ANSI constants - `ACCENT` (bold magenta), `SUCCESS` (green), `ERROR` (red), `WARNING` (yellow), `SECONDARY` (grey), `INFO` (cyan). The `term` submodule maps these to crossterm `Color` values for TUI rendering. `clap_styles()` applies the palette to CLI help text. The `cli_examples!` macro renders help footers.

**Shared helpers** (`ui.rs`): `SPINNER_FRAMES` (braille animation), `SYM_OK`/`SYM_FAIL` (✓/✗), `with_spinner()` (threaded progress animation), `print_json()`, `print_field()`, `print_label()`, `print_section_header()`, `print_name_list()`, `eprint_fail()`, `status_chip()`. All commands use these rather than direct `println!` with inline ANSI codes.

**TUI** (`tui.rs`): `TuiGuard` (RAII alternate screen), `draw_banner_or_fallback()` (shared banner rendering with small-terminal fallback), `draw_stars()` (twinkling animation). The list browser (`list/`) uses a separate `TerminalGuard` that reserves rows inline rather than using alternate screen.

### Key Patterns

- **Scope as first-class concept**: Global (`~/.agent/skills/`) vs Project (`./.agent/skills/`), with scope-scoped lock files. Resolution: CLI flag → config field → default Global. See `model::resolve_scope()`.
- **Agent as exhaustive enum**: `model::Agent` with serde aliases, maps to install paths and MCP settings targets. Adding an agent = add enum variant + path mappings.
- **Skill discovery by convention**: Skills found in `root/` or `root/skills/` by directory listing (no manifest needed). Each skill dir must contain a `SKILL.md`.
- **Output modes**: Most commands support `--plain` (no ANSI), `--json` (structured), and default (colors + animations). Check `animations_enabled()` and the `as_json`/`plain`/`quiet` flags.
- **`ListItem` trait** in `list/render.rs`: Generic list pane rendering - both `InstalledSkill` and `AssetEntry` implement it, eliminating duplicate pane code.
