# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Kasetto is a declarative AI agent environment manager: a Rust CLI that syncs four asset kinds -
**skills**, **slash-commands**, **MCP servers**, and **instructions** (`CLAUDE.md` / `AGENTS.md` /
`.cursor/rules` / ...) - from git repos or local dirs into 22 agent environments, driven by a
`kasetto.yaml` config and pinned by a `kasetto.lock`. Modeled on cargo/uv ergonomics.

Two things live here:

- **`/` (Rust crate `kasetto`)** - the CLI. Two binaries from one lib: `kasetto` (default-run) and
  `kst` (`src/bin/kst.rs`); both call `kasetto::run`.
- **`site/`** - Next.js 15 App Router app serving both the marketing landing and the Fumadocs docs
  (`kasetto.dev` + `docs.kasetto.dev`). Independent pnpm project, not a cargo workspace member.

## Commands

Everything goes through `just` (CI drives the same recipes). Recipes are split `-rs` / `-next`.

```bash
just check           # format + lint + test + build, both targets - the pre-PR gate
just lint-rs         # cargo clippy --all-targets -- -D warnings
just test-rs         # cargo test (skipped if a .no-tests sentinel file exists)
just build-rs        # cargo build --release
just lint-next       # cd site && pnpm lint (biome, --write)
just build-next      # cd site && pnpm build
just dev-next        # local Next.js dev server
just audit           # cargo-audit + pnpm audit (installs cargo-audit if missing)
just sync-config     # regenerate README/docs/hero from kasetto.example.yaml
just bench           # cold-sync benchmark (needs hyperfine + network)

cargo test <name>                     # single test by substring
cargo test --lib model::agent::tests  # one module's tests
cargo run -- sync --dry-run           # exercise the CLI locally
```

`just test-next` is a deliberate no-op (`echo "no Next.js tests"`); the site has no test suite.

**`kasetto.example.yaml` is the single source of truth for the example config.** It is copied into
`README.md` (between `<!-- kasetto-config:start/end -->`), the docs, and the homepage hero by
`scripts/sync-config-example.mjs`. After editing it run `just sync-config`; `node
scripts/sync-config-example.mjs --check` exits non-zero on drift.

## Rust architecture

`src/lib.rs` owns default-config resolution and re-exports `run` + `Result`. `src/app.rs` parses
clap args and dispatches; with no subcommand it prints the banner + `--help` and exits 0 (cargo/uv
style). Errors are a boxed `Box<dyn Error + Send + Sync>` (`error.rs`), no error enum.

### Module map

| Module | Responsibility |
| --- | --- |
| `cli.rs` | clap `Cli`/`Commands` plus the flattened `OutputArgs` / `ScopeArgs` / `SyncArgs` groups |
| `commands/` | one module per subcommand; `sync/` splits into `skills` / `mcps` / `commands` / `instructions` |
| `model/` | `Agent` enum + install-path registry (`agent.rs`), config schema (`config.rs`), lock/report types (`types.rs`), `extends` merge (`extend.rs`) |
| `source/` | URL parsing (`parse.rs`), archive download + sparse extraction (`remote.rs`), env-token auth (`auth.rs`), git-host rewriting (`hosts.rs`) |
| `fsops/` | config load incl. HTTP + `extends` (`config.rs`), comment-preserving YAML edits (`config_edit.rs`), extracted-tree cache (`cache.rs`), XDG dirs, SHA256, copy, settings I/O |
| `secrets/` | `${kst...}` placeholder scanning (`template.rs`) and resolution backends (`source.rs`) |
| `mcps/` | format-aware merge into agent settings (`merge.rs`), Codex TOML (`codex.rs`) |
| `prompts/` / `instructions/` | per-agent transforms for commands and instruction files; both parse via the shared `frontmatter.rs` |
| `lock.rs` / `state.rs` | committed `kasetto.lock` vs machine-local runtime state |
| `ui.rs` / `colors.rs` / `banner.rs` | all terminal rendering |
| `update_notifier.rs` | background "new version available" check |

### Core concepts

- **Scope** (`model::resolve_scope`): `Project` or `Global`, resolved CLI flag -> config field ->
  default `Global`. It picks install paths *and* the lock location: `<project root>/kasetto.lock`
  for Project, `$XDG_DATA_HOME/kasetto/kasetto.lock` for Global.
- **Agent as exhaustive enum** (`model/agent.rs`, 22 variants + `AGENT_PRESETS`): each variant maps
  to skill dirs, command dirs, instruction destinations, and MCP settings targets, per scope.
  Adding an agent = new variant + entries in every path table + the `AGENT_PRESETS` array + the
  README agent table.
- **Per-agent output formats**: `McpSettingsFormat` (5: McpServers, VsCodeServers, OpenCode,
  CodexToml, ZCode), `CommandFormat` (5: MarkdownFrontmatter, MarkdownPlain, PromptMd, PromptFile,
  GeminiToml), `InstructionFormat` (3: AggregateMarkdown, CursorMdc, PlainMarkdownDir). All three
  enums live in `model/mod.rs` alongside their `*Target` structs.
- **Aggregate vs per-file instructions**: `AggregateMarkdown` merges many instructions into one
  shared file (`CLAUDE.md`, `AGENTS.md`, ...) using managed `<!-- kasetto:instruction:ID -->`
  comment blocks so hand edits and other instructions survive; the other two formats write one file
  per instruction into a rules directory. The lock's `destination` token encodes which
  (`agg:<rel>` = strip the block on teardown, `file:<rel>` = delete the file).
- **The lock is authoritative.** A plain `sync` installs exactly what `kasetto.lock` pins and does
  zero network I/O when on-disk hashes already match (`needs_fetch` in `commands/sync/skills.rs`
  re-hashes destinations *before* deciding to download). `--update`/`-u` is the only path that
  re-resolves moving refs and rewrites hashes; `--locked`/`--frozen` never fetches and errors if
  the lock cannot satisfy the config. `LOCK_VERSION` is 3. Lock paths are stored relative to the
  scope root and contain no timestamps, so it is portable and commit-friendly.
- **`state.rs` holds everything machine-local** (last run, latest report JSON, per-skill install
  timestamps) under `$XDG_CACHE_HOME/kasetto/runtime/<hash>.json`, deliberately out of the lock -
  same split as `uv.lock` vs uv's cache. Safe to delete.
- **Source cache** (`fsops/cache.rs`): only immutable `ref:` sources are cached, under
  `$XDG_CACHE_HOME/kasetto/sources/<sha256(key)>/tree/` with a sibling `.complete` marker written
  last (extract to `.tmp-*`, then rename). Moving branches are never cached. `KASETTO_NO_CACHE`
  opts out.
- **Secrets are in-memory only.** `${kst_<name>}` (chain form: env var as written, then uppercased,
  then `credentials.yaml`) or `${kst:<tag>:<ref>}` (tagged: `env`, `crd`, `op`, `vault`,
  `kp`/`keepass`, `aws`, `gcp`, `az`, `pass`, `keychain`). Only the lowercase `kst` sentinel is
  claimed - `${VAR}` and `${KST_...}` pass through untouched. Injection happens on the MCP merge
  path after parsing, so the lock hashes the *placeholder* file and resolved values never reach
  `kasetto.lock`, the source cache, or a stage dir. `Secret` is a redacting newtype.
- **Comment-preserving config edits**: `add`/`remove` rewrite `kasetto.yaml` line-surgically via
  `fsops/config_edit.rs`, never a serde round-trip, so user comments and key order survive
  byte-for-byte. Then they delegate to `sync`.
- **Config resolution** when `--config` is omitted (`lib.rs::resolve_config_path`): `$KASETTO_CONFIG`
  -> `./kasetto.yaml` -> `source:` key in `$XDG_CONFIG_HOME/kasetto/config.yaml` ->
  `$XDG_CONFIG_HOME/kasetto/kasetto.yaml` -> `./kasetto.yaml` fallback. `extends:` in a config is
  merged before deserialization (scalars replace; asset lists merge by source identity).
- **Perf shape**: sources that need fetching are materialized in parallel with `rayon`, then results
  are processed sequentially in config order so output, lock writes, and last-writer-wins
  destination semantics stay deterministic. `remote.rs` streams into the gzip decoder and
  sparse-extracts only entries under `sub-dir`.

### Output conventions

`colors.rs` defines 8 semantic 24-bit roles - `ACCENT` (bold, no color), `ATTENTION` (amber),
`SUCCESS` (green), `ERROR` (red), `INFO` (cyan), `BRAND` (violet), `SECONDARY` (grey), `INFRA`
(dim) - and hex values live *only* there. There is deliberately no foreground constant; body text
inherits the terminal. Commands must render through `ui.rs` helpers (`action_glyph`,
`print_section_header`, `print_source_header`, `print_status_leaf`, `print_tree_leaf`,
`print_sync_chips`, `with_spinner`, `eprint_fail`/`eprint_warn`) rather than emitting inline ANSI.
The banner is only shown on bare `kst` and `kst init`.

Sync/clean tree rows are `├─ {glyph} {label} {name} {detail}` (`print_status_leaf`) - icon, then
the status word in a fixed `STATUS_LABEL_W` column, then the name, then dim version metadata.
Every column left of the name is constant width, so names always start at the same offset. `list`
uses `print_tree_leaf` instead, a single-column `├─ {name}` row - the asset id and nothing else;
the source it came from is already the group header above it.

Section headers all go through `print_section_header(label, count_unit, lead_blank, plain)` - amber
uppercase, count one space after the label. `lead_blank` is the blank line *above* the header:
pass `false` for the first section a command prints so no command opens on a blank line, `true`
afterwards. `doctor` passes `true` throughout because its head line always precedes the panels.
Every helper takes `plain` (never `color`), and plain mode must differ from colored mode only in
ANSI - never in wording, casing, or column position.

`print_tip` emits its own leading blank line - a tip is an aside about the run, not part of the
report above it. Never hand-roll a `println!()` before one.

Trailing tails (`✓ healthy` on the doctor head, `writable` on a dir row, the item count on a source
header) sit **one space after their label** - never right-aligned to a column. Labels here are
paths and source URLs that range from `/tmp/pk` to a long monorepo URL, so any reserved column
strands the short rows' tails mid-terminal. Fixed-width columns are only for the closed
vocabularies: `STATUS_LABEL_W` in tree rows, and `print_doctor_kv`'s key column.

Most commands accept `--json`, `--color <auto|always|never>`, `-q`/`--quiet` (repeatable), and
`--project`/`--global`; only `sync` has `-v`/`--verbose`. `--plain` is a hidden deprecated alias
for `--color never`. Flags are resolved at the `app.rs` boundary via `OutputArgs::resolve_plain()`
/ `SyncArgs::resolve_plain()`, which also mirrors the choice into `CLICOLOR_FORCE`/`NO_COLOR` so
surfaces that never see the flag (the top-level error handler, `lock`) agree with it. `NO_COLOR`
and `CLICOLOR_FORCE` are honored.

**One exit path.** `app::run` returns `ExitCode` and is the only place the process decides its
status; no command calls `process::exit`. Commands that can complete *and* report a problem return
`commands::Outcome` (`sync` when anything is broken or failed, `doctor` when a check fails,
`lock --check` on drift, `add`/`remove` by propagating their follow-up sync's verdict); the rest
return `Result<()>` and are wrapped by `app::ok`. An `Err` is rendered once by `eprint_error` -
never let one reach `main`, or Rust's `Debug` formatter prints `Error: Custom { .. }`.

### Env vars the CLI reads

`KASETTO_CONFIG`, `KASETTO_CACHE_DIR`, `KASETTO_NO_CACHE`, `NO_COLOR`, `CLICOLOR_FORCE`,
XDG (`XDG_CONFIG_HOME` / `XDG_DATA_HOME` / `XDG_CACHE_HOME`, `HOME`, `APPDATA`),
`KST_KEEPASS_PASSWORD`, and source auth tokens (`GITHUB_TOKEN`/`GH_TOKEN`, `GITLAB_TOKEN`,
`CI_JOB_TOKEN`, `BITBUCKET_*`, `CODEBERG_TOKEN`/`GITEA_TOKEN`/`FORGEJO_TOKEN`).

## Site (`site/`)

Next.js 15 + React 19, Tailwind 3 with the Fumadocs preset, Biome (not ESLint/Prettier) for both
lint and format, pnpm 11 / Node >= 22.

- `app/page.tsx` is the marketing landing; `app/docs/[[...slug]]/page.tsx` renders MDX from
  `content/docs/*.mdx` (sidebar order comes from `content/docs/meta.json`).
- Raw-Markdown and LLM routes: `app/docs-md/[[...slug]]/route.ts` (reached via the
  `/docs/:path*.md` rewrite in `next.config.mjs`), `app/llms.txt`, `app/llms-full.txt`.
- `app/install/route.ts` and `app/install.ps1/route.ts` serve the installer scripts behind
  `kasetto.dev/install`.
- `next.config.mjs` also holds security headers and host-gated 308 redirects from
  `docs.kasetto.dev/<slug>` to `kasetto.dev/docs/<slug>` (add new slugs to `DOC_SLUGS`).
- ` ```mermaid ` fences become live `<Mermaid>` JSX via the `remarkMermaid` plugin in
  `source.config.ts`, bypassing Shiki.
- Design tokens live only in `:root` in `app/globals.css`; dark-only, no theme toggle.
- Vercel auto-deploy on `main` is disabled (`site/vercel.json`); the site ships via the manual
  `site.yaml` workflow.

## Conventions

- **Module docs**: every `.rs` file opens with a `//!` comment. `lib.rs` and each `mod.rs` start
  `Package that contains ...`; every other file starts `Module that contains ...`. One summary
  sentence, extra detail on following `//!` lines.
- **Tests are inline `#[cfg(test)] mod tests`** at the bottom of each module - there is no `tests/`
  directory. `assert_cmd`/`predicates`/`tempfile` are available as dev-deps.
- `unsafe_code` is forbidden; `clippy::all` is denied, `perf` warns, and `dbg!`/`todo!` warn
  (`[lints]` in `Cargo.toml`).
- Rust indents 4, everything else 2 (`.editorconfig`); max line length 120.
- **Conventional Commits** (`<type>(<scope>): <subject>`, imperative, lowercase, no trailing
  period) - `CHANGELOG.md` is generated by `git-cliff` from them, so commit messages are
  user-facing. Branches are `<type>/<kebab-description>`. Full type table in `CONTRIBUTING.md`.
- Feature or messaging changes must land in `README.md`, `site/content/docs/`, and the code
  together.
- The repo ships four local Claude skills under `.claude/skills/` (brand guidelines, CLI style,
  docs style, site style). Consult them before writing CLI output, README/docs prose, or site
  markup. `.claude/` is gitignored.

## CI and release (`.github/workflows/`)

All four workflows expose `workflow_dispatch` (`gh workflow run <name>.yaml --ref main`).

- **`ci.yaml`** - push to `main` + every PR. Two parallel jobs on `ubuntu-24.04-arm`: `ci-rs`
  (install -> lint -> audit -> test -> build) and `ci-next` (same shape). Every step is a `just`
  recipe, so reproducing CI locally is `just check` plus `just audit`.
- **`release.yaml`** - manual only. `tag` (git-cliff derives the version unless the `version` input
  overrides it - pass that one bare, `3.7.0` not `v3.7.0`, because only the auto-detect path strips
  the `v` and the workflow prepends it; bumps `Cargo.toml`/`Cargo.lock`, regenerates
  `CHANGELOG.md`, commits `release: vX.Y.Z`, tags, pushes)
  -> `build` (6 targets: linux/macos/windows x x86_64/aarch64) -> `release` (checksums + GitHub
  Release) -> `publish-crate` + `update-homebrew` (`pivoshenko/homebrew-tap`) + `update-scoop`
  (`pivoshenko/scoop-bucket`). Never bump the version by hand.
- **`site.yaml`** - manual only, `npx vercel deploy --prod --yes`. Decoupled from the CLI release.
- **`labels.yaml`** - syncs GitHub labels from `.github/labels.yaml`.
