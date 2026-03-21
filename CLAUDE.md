# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
just fmt        # Format code with cargo fmt
just lint       # Run clippy with warnings-as-errors
just test       # Run all tests
just build      # Build release binary
just check      # Run fmt + lint + test + build

cargo test <test_name>  # Run a single test by name
```

## Architecture

Kasetto is a Rust CLI tool (~3,600 lines) that syncs AI skill packs across 40+ agent environments (Claude Code, Cursor, Windsurf, Copilot, etc.). The entry point is `src/main.rs`, which sets a global `mimalloc` allocator and delegates to `lib::run()`.

**Data flow**: CLI parsing → `app.rs` router → command handler → `fsops.rs` → SQLite manifest

### Key modules

- **`app.rs`** — Startup router: decides between home TUI (no args, no config), explicit command, or default config sync
- **`cli.rs`** — `clap`-based argument parsing; all commands/flags defined here
- **`commands/`** — One file per command: `sync.rs` (main logic), `list.rs`, `doctor.rs`, `self_update.rs`
- **`fsops.rs`** (~695 lines) — All file system and network operations: config loading (local YAML or HTTPS URL), GitHub repo fetching via tar.gz API, SHA256 directory hashing, SQLite manifest read/write, path resolution
- **`model.rs`** — Core types: `Config` (YAML config), `Agent` enum (40+ presets with platform paths), `SkillEntry` (tracked skill with hash/timestamp), `State` (in-memory manifest), `Report`/`Summary` (sync results)
- **`profile.rs`** — Parses `SKILL.md` frontmatter and extracts skill metadata; falls back through: YAML frontmatter → heading → first body line
- **`home.rs`** (~554 lines) — Interactive TUI when no config exists; vim-style navigation (j/k, gg/G, PgUp/PgDn)
- **`list.rs`** (~759 lines) — Skill browser TUI with multi-column layout and JSON export mode
- **`ui.rs`** — Spinner animations and status chips; suppressed in `--quiet`/`--json`/`--plain` modes

### Config format (`skills.config.yaml`)

```yaml
agent: claude-code        # OR destination: ./path  (agent maps to platform-specific path)
skills:
  - source: https://github.com/org/repo
    branch: main          # optional, falls back main→master
    skills: "*"           # or list of { name, path } objects
  - source: ~/local/path
    skills:
      - skill-name
```

### Sync logic

1. Load YAML config (local file or HTTPS)
2. For each source: fetch GitHub repo (tar.gz) or read local directory
3. Discover skills via `SKILL.md` presence
4. SHA256-hash each skill directory; compare against SQLite manifest
5. Install/update changed skills, remove deleted ones
6. Persist new state to SQLite

## Commit convention

Follows [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`, `build:`, `perf:`, `style:`, `design:`, `revert:`

Example: `feat(sync): add support for private GitHub repos`
