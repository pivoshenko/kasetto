# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

`just check` runs format + lint + test + build for both the Rust crate and the Next.js site under `site/`. Per-target recipes are split:

```bash
just check          # full validation (rs + site)
just format         # format-rs + format-next
just lint           # lint-rs + lint-next
just test           # cargo test
just build          # build-rs + build-next
cargo test <name>   # run a single Rust test
just dev-next       # local Next.js dev server
```

The Rust project forbids `unsafe` code and warns on `dbg!` and `todo!` (see `[lints]` in `Cargo.toml`).

## Architecture

Kasetto is a single-binary CLI tool that syncs AI agent skills from GitHub repos or local directories into 21 agent environments. Two binaries (`kasetto` and `kst`) share the same code.

### Startup Routing (`app.rs`)

```
CLI args → match cli.command
  ├─ Explicit subcommand → run that command
  └─ None → banner + `--help` (exit 0) — cargo/uv style
```

### Module Layout

- **`commands/`** - Each subcommand: `sync/` (split into `skills.rs` + `mcps.rs` + `commands.rs`), `list`, `doctor`, `init`, `clean`, `self_update`, `uninstall`, `completions`
- **`model/`** - Core types: `Agent` enum (21 presets with install paths), `Config` (YAML deserialization), `Scope` (Global/Project), `SkillEntry`, `CommandEntry`, `CommandFormat`, `Report`, `Summary`. `extend.rs` holds the YAML-level `extends` merge: scalars replace; `skills`/`mcps`/`commands` merge by `(source, ref|branch, sub-dir)` identity
- **`source/`** - Remote handling: URL parsing (`parse.rs`), archive download/extraction (`remote.rs`), auth token resolution (`auth.rs`), git host URL rewriting (`hosts.rs`)
- **`fsops/`** - File operations: config loading from file/HTTP (`mod.rs`, `load_config_any` recursing through `extends`), path resolution, SHA256 hashing (`hash.rs`), recursive copy (`copy.rs`), XDG dirs (`dirs.rs`), HTTP client (`http.rs`), settings file I/O (`settings.rs`)
- **`mcps/`** - MCP server management: pack discovery (`pack.rs`), format-aware merging (`merge.rs`), Codex TOML handling (`codex.rs`). Supports 4 formats: McpServers JSON, VsCode servers JSON, OpenCode JSON, Codex TOML
- **`prompts/`** - Slash-command (user-defined prompt template) handling: frontmatter parsing (`parse.rs`), per-agent transforms (`transform.rs`), entry point `apply_command`. Supports 5 output formats: MarkdownFrontmatter, MarkdownPlain, PromptMd, PromptFile (Continue), GeminiToml
- **`lock.rs`** - Portable manifest persistence (`kasetto.lock`, schema `version: 2`): tracks installed skills + command/MCP assets only. Deterministic and commit-friendly — `destination` paths are stored relative to the scope root (project root for Project, home for Global) and resolved back to absolute at read time via `fsops::resolve_dest`/`relativize_dest`. No timestamps or run-specific data. **The lock is authoritative** (issue #33): a plain `kasetto sync` installs exactly what the lock pins and performs zero network fetches when on-disk destinations already match. The per-source `needs_fetch` gate in `commands/sync/skills.rs` re-hashes every destination *before* downloading and skips `materialize_source` entirely when all destinations match the locked hash; a missing/tampered secondary destination is repaired by copying from a verified-good local destination rather than fetching. `--update`/`-u` (optionally `--update <name>...`) is the only path that re-resolves moving refs and rewrites locked hashes; for a `skills: "*"` wildcard source, plain sync holds to the locked set (derived from lock entries where `entry.source == src.source`) and only `--update` re-resolves the wildcard via download + `select_targets`. `--locked`/`--frozen` never fetches and errors when the lock cannot satisfy the config (config-named skill absent, or source entirely absent); `--locked --update` is rejected as contradictory. Skill content hashing (`fsops::hash_dir`) normalizes path separators to `/` so digests are OS-invariant — existing locks therefore show one round of `updated` on the next plain sync after upgrading.
- **`state.rs`** - Machine-local runtime state kept *out* of the committed lock (mirrors how `uv` keeps state in its cache dir, separate from `uv.lock`). Holds `last_run`, the latest sync `Report` JSON (for `doctor` failures), and per-skill install timestamps (for `list`'s "updated N ago"). Stored as JSON under `$XDG_CACHE_HOME/kasetto/runtime/<hash-of-lock-path>.json`; safe to delete, regenerated on next sync
- **`banner.rs`** - ASCII brand banner with static color overlay. Only rendered on bare `kst` (welcome) and `kst init`. All other subcommands are banner-less — operational output should not repeat the signature
- **`update_notifier.rs`** - Background "new version available" notice. Fires a detached thread from `app::run` to refresh `$XDG_CACHE_HOME/kasetto/update-check.json` (24h TTL), then prints one yellow line at end of run. Reuses `is_newer`/`fetch_latest_release` from `commands::self_update`. Suppressed for `--json`/`--color never`/`--quiet`, `completions`, `self update`, and non-TTY stdout

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

**Color palette** (`colors.rs`): Operational output and clap help use the basic ANSI 16-color palette (`\x1b[32m` green, `\x1b[33m` yellow, `\x1b[36m` cyan, `\x1b[2m` dim, etc.) so hues inherit from the user's terminal theme — same approach as `cargo` and `uv`. Semantic roles map to ANSI: `SUCCESS` (green), `ERROR` (red), `ATTENTION` / `WARNING` (yellow), `INFO` (cyan, for `tip:`), `SECONDARY` (dim), `ACCENT` (bold no-color), `ACCENT_WARM` (bold cyan for the spinner). `NO_COLOR` / `--color never` gate emission via `ui::color_stdout_enabled()`. `CLICOLOR_FORCE` (set by `--color always`) overrides TTY detection. `clap_styles()` matches cargo — bold green headers, bold (uncolored) literals, default placeholders. **The brand banner (`banner.rs`) is the only surface that uses 24-bit truecolor**: popil lavender `#a89bb5` for the logo and popil yellow `#d4a85a` for the subtitle, kept as private constants in `banner.rs`.

**Shared helpers** (`ui.rs`): `SPINNER_FRAMES` (braille animation), `SYM_OK`/`SYM_FAIL` (✓/✗), `with_spinner()` (threaded progress animation), `print_json()`, `print_field()` / `print_label()` (dim secondary labels, uv-style), `eprint_fail()` (red `error:` prefix), `action_glyph()` (uv-style ` + ` / ` ~ ` / ` - ` / ` = ` / ` ! ` for per-asset sync rows), `color_stdout_enabled()`. All commands consume these rather than emitting inline ANSI.

**Output styling (uv-aligned)**: operational paths (`sync`/`list`/`doctor`/`clean`) follow uv discipline — terminal-default body text, bold-colored lead verbs in summaries (`Installed N items in Xms`, `Updated N items`, `Removed N items`, `Audited N items` for `--locked` no-ops), `warning:` / `error:` prefixed lines to stderr for non-fatal/fatal counts. `kst list` prints aligned tables (`NAME / SCOPE / SOURCE`, plus `UPDATED` for skills); per-action `--verbose` rows use the prefix-glyph format above.

### Key Patterns

- **Scope as first-class concept**: Global (`~/.agent/skills/`) vs Project (`./.agent/skills/`), with scope-scoped lock files. Resolution: CLI flag → config field → default Global. See `model::resolve_scope()`.
- **Agent as exhaustive enum**: `model::Agent` with serde aliases, maps to install paths and MCP settings targets. Adding an agent = add enum variant + path mappings.
- **Skill discovery by convention**: Skills found in `root/` or `root/skills/` by directory listing (no manifest needed). Each skill dir must contain a `SKILL.md`.
- **Output modes**: Most commands support `--color <auto|always|never>` (default `auto`), `--json` (structured), `-q`/`--quiet` (count action — repeat for stricter silence), and `-v`/`--verbose` (count action — `-v`/`-vv`/`-vvv`). `--plain` is preserved as a hidden deprecated alias for `--color never` and emits a stderr warning when used. Check `animations_enabled()` and the `as_json`/`plain`/`quiet` flags inside commands; resolve flags at the `app.rs` boundary via `OutputArgs::resolve_plain()` / `SyncArgs::resolve_plain()`.

## GitHub Workflows (`.github/workflows/`)

All four workflows expose `workflow_dispatch` so they can be triggered manually with `gh workflow run <name>.yaml --ref main`.

- **`ci.yaml`** — runs on push to `main` and on every PR. Two parallel `ubuntu-latest` jobs: `ci-rs` (`just lint-rs` → `just audit-rs` → `just test` → `just build-rs`) and `ci-next` (pnpm install → `pnpm lint` → `pnpm audit` → `pnpm build` in `site/`).
- **`release.yaml`** — manual dispatch only. Optional `version` input; otherwise `git-cliff --bumped-version` derives the next version from conventional commits. Pipeline: `tag` (bump `Cargo.toml` + `Cargo.lock`, regenerate `CHANGELOG.md` via `git-cliff`, commit as `release: vX.Y.Z`, tag, push) → `build` (matrix across 6 targets: linux/macos/windows × x86_64/aarch64; cross-compiles aarch64 linux with `gcc-aarch64-linux-gnu`) → `release` (sha256 `checksums.txt`, GitHub Release with `--latest --strip header` changelog body) → `publish-crate` (`cargo publish`, needs `CARGO_REGISTRY_TOKEN`) + `update-homebrew` (regenerates `Formula/kasetto.rb` in `pivoshenko/homebrew-tap`, needs `HOMEBREW_TAP_TOKEN`) + `update-scoop` (regenerates `kasetto.json` in `pivoshenko/scoop-bucket`, needs `SCOOP_BUCKET_TOKEN`).
- **`site.yaml`** — manual dispatch only. Runs `npx vercel deploy --prod --yes` against the Vercel project. Needs `VERCEL_TOKEN`, `VERCEL_ORG_ID`, `VERCEL_SITE_PROJECT_ID`. Not coupled to `release.yaml` — site ships independently of the CLI.
- **`labels.yaml`** — auto-syncs GitHub labels via `crazy-max/ghaction-github-labeler` whenever `.github/labels.yaml` or the workflow itself changes on `main`. Needs `GH_TOKEN`.
