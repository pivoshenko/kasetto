# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

`just check` runs format + lint + test + build for both the Rust crate and the Next.js site under `site/`. Per-target recipes are split:

```bash
just check          # full validation (rs + site)
just format         # format-rs + format-site
just lint           # lint-rs + lint-site
just test           # cargo test
just build          # build-rs + build-site
cargo test <name>   # run a single Rust test
just serve-site     # local Next.js dev server
```

The Rust project forbids `unsafe` code and warns on `dbg!` and `todo!` (see `[lints]` in `Cargo.toml`).

## Architecture

Kasetto is a single-binary CLI tool that syncs AI agent skills from GitHub repos or local directories into 21 agent environments. Two binaries (`kasetto` and `kst`) share the same code.

### Startup Routing (`app.rs`)

```
CLI args → match cli.command
  ├─ Explicit subcommand → run that command
  └─ None → Home screen (interactive TUI menu)
```

### Module Layout

- **`commands/`** - Each subcommand: `sync/` (split into `skills.rs` + `mcps.rs` + `commands.rs`), `list`, `doctor`, `init`, `clean`, `self_update`, `uninstall`, `completions`
- **`model/`** - Core types: `Agent` enum (21 presets with install paths), `Config` (YAML deserialization), `Scope` (Global/Project), `SkillEntry`, `CommandEntry`, `CommandFormat`, `Report`, `Summary`. `extend.rs` holds the YAML-level `extends` merge: scalars replace; `skills`/`mcps`/`commands` merge by `(source, ref|branch, sub-dir)` identity
- **`source/`** - Remote handling: URL parsing (`parse.rs`), archive download/extraction (`remote.rs`), auth token resolution (`auth.rs`), git host URL rewriting (`hosts.rs`)
- **`fsops/`** - File operations: config loading from file/HTTP (`mod.rs`, `load_config_any` recursing through `extends`), path resolution, SHA256 hashing (`hash.rs`), recursive copy (`copy.rs`), XDG dirs (`dirs.rs`), HTTP client (`http.rs`), settings file I/O (`settings.rs`)
- **`mcps/`** - MCP server management: pack discovery (`pack.rs`), format-aware merging (`merge.rs`), Codex TOML handling (`codex.rs`). Supports 4 formats: McpServers JSON, VsCode servers JSON, OpenCode JSON, Codex TOML
- **`prompts/`** - Slash-command (user-defined prompt template) handling: frontmatter parsing (`parse.rs`), per-agent transforms (`transform.rs`), entry point `apply_command`. Supports 5 output formats: MarkdownFrontmatter, MarkdownPlain, PromptMd, PromptFile (Continue), GeminiToml
- **`lock.rs`** - Portable manifest persistence (`kasetto.lock`, schema `version: 2`): tracks installed skills + command/MCP assets only. Deterministic and commit-friendly — `destination` paths are stored relative to the scope root (project root for Project, home for Global) and resolved back to absolute at read time via `fsops::resolve_dest`/`relativize_dest`. No timestamps or run-specific data.
- **`state.rs`** - Machine-local runtime state kept *out* of the committed lock (mirrors how `uv` keeps state in its cache dir, separate from `uv.lock`). Holds `last_run`, the latest sync `Report` JSON (for `doctor` failures), and per-skill install timestamps (for `list`'s "updated N ago"). Stored as JSON under `$XDG_CACHE_HOME/kasetto/runtime/<hash-of-lock-path>.json`; safe to delete, regenerated on next sync
- **`home/`** - Interactive welcome screen with `prompt.rs` for sync arg input
- **`list/`** - Interactive TUI browser: `browse.rs` (event loop), `render.rs` (frame drawing), `session.rs` (state/guard), `tab.rs`, `types.rs`
- **`update_notifier.rs`** - Background "new version available" notice. Fires a detached thread from `app::run` to refresh `$XDG_CACHE_HOME/kasetto/update-check.json` (24h TTL), then prints one yellow line at end of run. Reuses `is_newer`/`fetch_latest_release` from `commands::self_update`. Suppressed for `--json`/`--plain`/`--quiet`, `completions`, `self update`, and non-TTY stdout

### Site (`site/`)

Next.js 15 App Router project that hosts both the marketing landing (`/`) and the Fumadocs-powered documentation (`/docs/*`). Single Vercel project serves `kasetto.dev` and `docs.kasetto.dev` (legacy subdomain — host-gated 308 redirects in `next.config.mjs` rewrite `docs.kasetto.dev/<slug>` to `kasetto.dev/docs/<slug>`).

- **`app/`** — App Router pages, shared `TopNav`, theme-less dark layout. `app/page.tsx` is the marketing homepage (tape-deck layout); `app/docs/[[...slug]]/page.tsx` renders MDX via Fumadocs.
- **`content/docs/*.mdx`** — Documentation source. Order in `meta.json`. Mermaid blocks become live `<Mermaid>` JSX via the `remarkMermaid` plugin in `source.config.ts` (bypasses Shiki).
- **`app/globals.css`** — Single source of design tokens in `:root`: palette (`--bg`/`--mauve`/`--rust`/...), type scale (`--fs-xs`..`--fs-2xl`), spacing (`--space-1`..`--space-18`), tracking, radius. Component styles reference tokens — no hardcoded color/font values outside the `:root` block. Fumadocs `--fd-*` tokens are bridged to the same palette in HSL.
- Dark-only: there is no theme toggle; `RootProvider theme={{ enabled: false }}` and `<html className="dark" data-theme="dark">`.

### Sync Data Flow

1. Load config from file or HTTP URL (with GitLab/GitHub/Gitea auth via env vars). If the YAML has `extends:`, the loader recursively fetches and merges parent configs before deserialization (cycle-detected, capped at depth 8).
2. Resolve scope (CLI flag → config field → default Global) and destination paths per agent
3. For each skill source: materialize (download if remote) → discover available skills → select targets → hash → copy → update lock state
4. For each command source: materialize → discover commands (`commands: "*"` auto-discovers the `commands/` directory; named/explicit entries otherwise) → parse frontmatter → transform each into the target agent's native format (`prompts::apply_command`, 5 output formats) → write to destination → update lock
5. For each MCP source: materialize → resolve files (`mcps: "*"` → auto-discover `.mcp.json` / `mcp.json` / `mcps/*.json`; `mcps: [names]` → `mcps/<name>.json`; `mcps: [{name, path}]` → `<path>/<name>.json`) → collect pending installs → merge into agent settings files → update lock
6. Save lock file and report (unless `--dry-run`)

### UI System

**Color palette** (`colors.rs`): Semantic ANSI constants - `ACCENT` (bold magenta), `SUCCESS` (green), `ERROR` (red), `WARNING` (yellow), `SECONDARY` (grey), `INFO` (cyan). The `term` submodule maps these to crossterm `Color` values for TUI rendering. `clap_styles()` applies the palette to CLI help text. The `cli_examples!` macro renders help footers.

**Shared helpers** (`ui.rs`): `SPINNER_FRAMES` (braille animation), `SYM_OK`/`SYM_FAIL` (✓/✗), `with_spinner()` (threaded progress animation), `print_json()`, `print_field()`, `print_label()`, `print_section_header()`, `print_name_list()`, `eprint_fail()`, `status_chip()`. All commands use these rather than direct `println!` with inline ANSI codes.

**TUI** (`tui.rs`): `TuiGuard` (RAII alternate screen), `draw_banner_or_fallback()` (shared banner rendering with small-terminal fallback). The TUI banner is static — both the home screen and list browser block on `event::read()` and redraw only on key press or resize via a dirty flag. The list browser (`list/`) uses a separate `TerminalGuard` that reserves rows inline rather than using alternate screen.

### Key Patterns

- **Scope as first-class concept**: Global (`~/.agent/skills/`) vs Project (`./.agent/skills/`), with scope-scoped lock files. Resolution: CLI flag → config field → default Global. See `model::resolve_scope()`.
- **Agent as exhaustive enum**: `model::Agent` with serde aliases, maps to install paths and MCP settings targets. Adding an agent = add enum variant + path mappings.
- **Skill discovery by convention**: Skills found in `root/` or `root/skills/` by directory listing (no manifest needed). Each skill dir must contain a `SKILL.md`.
- **Output modes**: Most commands support `--plain` (no ANSI), `--json` (structured), and default (colors + animations). Check `animations_enabled()` and the `as_json`/`plain`/`quiet` flags.
- **`ListItem` trait** in `list/render.rs`: Generic list pane rendering - both `InstalledSkill` and `AssetEntry` implement it, eliminating duplicate pane code.

## GitHub Workflows (`.github/workflows/`)

All four workflows expose `workflow_dispatch` so they can be triggered manually with `gh workflow run <name>.yaml --ref main`.

- **`ci.yaml`** — runs on push to `main` and on every PR. Single `ubuntu-latest` job: `just lint-rs` → `just test` → `just build-rs`. Does not touch the site.
- **`release.yaml`** — manual dispatch only. Optional `version` input; otherwise `git-cliff --bumped-version` derives the next version from conventional commits. Pipeline: `tag` (bump `Cargo.toml` + `Cargo.lock`, regenerate `CHANGELOG.md` via `git-cliff`, commit as `release: vX.Y.Z`, tag, push) → `build` (matrix across 6 targets: linux/macos/windows × x86_64/aarch64; cross-compiles aarch64 linux with `gcc-aarch64-linux-gnu`) → `release` (sha256 `checksums.txt`, GitHub Release with `--latest --strip header` changelog body) → `publish-crate` (`cargo publish`, needs `CARGO_REGISTRY_TOKEN`) + `update-homebrew` (regenerates `Formula/kasetto.rb` in `pivoshenko/homebrew-tap`, needs `HOMEBREW_TAP_TOKEN`) + `update-scoop` (regenerates `kasetto.json` in `pivoshenko/scoop-bucket`, needs `SCOOP_BUCKET_TOKEN`).
- **`site.yaml`** — manual dispatch only. Runs `npx vercel deploy --prod --yes` against the Vercel project. Needs `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_SITE_PROJECT_ID`. Not coupled to `release.yaml` — site ships independently of the CLI.
- **`labels.yaml`** — auto-syncs GitHub labels via `crazy-max/ghaction-github-labeler` whenever `.github/labels.yaml` or the workflow itself changes on `main`. Needs `GH_TOKEN`.
