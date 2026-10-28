# AGENTS.md

Kasetto is a declarative AI agent environment manager written in Rust: a user declares skills, slash
commands, MCP servers, and instruction files in a `kasetto.yaml`, and `kst sync` pulls them from git
forges, transforms them into each agent's native format, installs them, and records exactly what was
installed in `kasetto.lock`. The repo is one Rust crate at the root shipping two binaries (`kasetto`
and its short alias `kst`, both calling `kasetto::run()`) plus the documentation site at `site/`.

## Never

- never hand-edit the generated config blocks in `README.md`, the docs, or the site hero.
  `kasetto.example.yaml` is the single source of truth; run `just generate-config-docs`
- never bump the version by hand. Releases are `workflow_dispatch`-only and git-cliff resolves the
  version, bumps `Cargo.toml`, tags, and publishes
- never let a resolved secret value reach the lock, the source cache, or the stage dir. Injection
  happens on the in-memory config and is written **only** to the agent destination; the lock hashes
  the placeholder source
- never change the lock schema without bumping `LOCK_VERSION` in `model/types.rs`
- never put machine- or run-specific data in `kasetto.lock`. It is committed and portable, so install
  paths are stored **relative to the scope root**
- never read `destination:` outside the skills path. It redirects skills only; commands, instructions,
  and MCPs go through `resolve_command_targets`, `resolve_instruction_targets`, and
  `resolve_mcp_settings_targets`, which always use the agent presets
- never make `fsops/config_edit.rs` round-trip serde. It edits `kasetto.yaml` at the raw-line level
  because `add`/`remove` must preserve the user's comments and key order byte-for-byte. Keep it that way
- never store a raw IP or a stable per-machine id in the site counters. `site/lib/stats.ts` hashes IP +
  user agent with the counted period inside the digest, so it rotates with that period. A distinct
  count needs a digest stable across the period it reports: unioning daily HyperLogLogs over a week
  counts one machine seven times, which is why the weekly figure has its own week-stamped key
- never echo an installed MCP definition in an audit verdict. `audit/mcp.rs` reads it back from the agent
  config with secrets already resolved, so a summary names the rule and at most a host or package name,
  never an env value, header, URL path or query, or shell script
- never cache a mutable ref. `fsops/cache.rs` caches extracted trees **only** for immutable refs (explicit
  tag or SHA); a branch or default ref can change upstream without the URL changing. Its `.complete`
  marker is written last and kept beside `tree/`, never inside it, so it cannot leak into hashed content

## Silent failure modes

- a new `site/content/docs/*.mdx` file is invisible in the site nav unless it is also added to the
  ordered `pages` array in `site/content/docs/meta.json`
- a site color lives twice: as a CSS token in `site/app/styles/tokens.css` and as raw hex in
  `site/lib/palette.ts`, which the next/og icon, Apple icon, and OG image renderers read because they
  cannot resolve CSS variables. Change one without the other and the favicons drift from the page
- the brand mark has one source, `site/lib/mark.ts`; the nav, favicons, and OG card render from it at
  runtime. `assets/*.svg`, `public/logo.svg`, and `public/favicon.svg` are hand-made exports, not built
  from it, and still carry the previous mark: change the mark and they stay stale until regenerated
- a `Callout` import in an MDX file shadows the site component mapped in `mdx-components.tsx`, so the
  docs render fumadocs' unstyled callout instead

## Commands

`just` is the task runner; every recipe is split `-rs` (crate) / `-site` (Next.js app), with the bare
name running both - `just --list`, or the recipe table in `CONTRIBUTING.md`, for the full set. `just
check` is lint + test + build over both halves, and CI runs exactly those recipes, so a green `just
check` locally means a green CI. The Rust suite is hermetic - no test issues a network request, so a
failure is a real failure, not a sandbox artifact.

```bash
cargo test resolve_config_path                 # by test-name substring
cargo test --lib commands::sync::skills        # by module path
cargo test -- --nocapture                      # keep stdout
```

Generation recipes, run after editing their inputs: `just generate-config-docs` (regenerates the example
config in `README.md`, the docs, and the site hero from `kasetto.example.yaml`; `node
scripts/sync-config-example.mjs --check` exits non-zero on drift) and `just generate-changelog`
(regenerates `CHANGELOG.md` with git-cliff and `cliff.toml`).

## Conventions

- every module starts with a `//!` doc comment beginning "Module that contains ..." or, for a
  directory module, "Package that contains ..."
- `unsafe_code` is forbidden and `clippy::all` is denied at the crate level; `lint-rs` runs Clippy with
  `-D warnings`, so a warning is a build failure
- tests are inline `#[cfg(test)] mod tests` blocks next to the code they cover - there is no `tests/`
  directory. `fsops::temp_dir` is the shared helper for filesystem tests
- the crate is a single private module tree: everything is `pub(crate)` / `pub(super)`, and `lib.rs`
  exports only `run` and `Result`
- `run()` returns `ExitCode`, not `Result`, so a failure prints the CLI's own `error:` line instead of
  Rust's `Debug` rendering
- `commands::Outcome` separates "could not do the job" (an `Err`) from "did the job, found problems,
  should still exit non-zero" (`Outcome::Failure`) - a broken asset in `sync`, a failed check in `doctor`
- `colors.rs` is the only file holding hex values; it defines the semantic roles and call sites use those
  names. There is deliberately no "foreground" constant - body text inherits from the terminal. Color is
  gated on `color_stdout_enabled()`; `NO_COLOR` and piping drop it
- commits and branches follow `CONTRIBUTING.md`: Conventional Commits with a module/command scope
- behavioral changes come with tests; interface changes update `site/content/docs/` alongside the code

## Cross-Cutting Changes

**Adding an agent** starts in `model/agent.rs` - one enum variant, and with it that agent's
skill/command/instruction/MCP paths and formats for both global and project scope. The roster is then
mirrored by hand and every copy has to move together, and a half-updated roster still compiles: the
README's agent table, `site/app/components/agents-grid.tsx`, its logo in `site/lib/agent-icons.ts`
(a missing entry falls back to a monogram), and the per-agent tables in
`site/content/docs/` (`agents.mdx`, `slash-commands.mdx`, `how-sync-works.mdx`). A new MCP settings shape
additionally needs an arm in `src/mcps/`.

**Adding a subcommand** touches the `Commands` enum in `cli.rs`, a new module under `commands/` declared
in `commands/mod.rs`, a dispatch arm plus a `should_suppress_notice` arm in `app.rs`, an `Outcome`
choice, the README's Commands table, and `site/content/docs/commands.mdx`.

**Adding anything for one asset kind** usually needs the same treatment in the other three. Skills,
commands, MCPs, and instructions are parallel concepts, each with its own config list, its own
discovery convention in a source repo, and its own per-agent destination and format.

## Architecture

**Sync.** The four asset kinds meet in `commands/sync/`, one submodule per kind plus a shared `mod.rs`
holding `SyncContext`, `SyncMut`, and the shared orphan-pruning pass `remove_stale`. Discovery
conventions live in `source/mod.rs`: skills are directories with a `SKILL.md`, commands are
`commands/**/*.md` with nesting namespaced by `:` (`commands/git/commit.md` -> `git:commit`),
instructions are `instructions/**/*.{md,mdc}` with the same namespacing, MCPs are `mcps/<name>.json`.

**Config and scope.** Scope resolution is CLI override > config `scope:` > `Global`, and
`fsops::resolve_destinations` turns (config, scope) into the concrete skills dirs, an explicit
`destination:` winning outright. With `--config` omitted, `lib.rs::resolve_config_path` tries
`$KASETTO_CONFIG`, `./kasetto.yaml`, a `source:` key in the XDG preferences file `kasetto/config.yaml`,
then the global `kasetto/kasetto.yaml`. `model/extend.rs` implements `extends:` as a YAML-level merge
before deserialization: scalars replace, asset lists merge by identity tuple.

**Lock file and runtime state.** Deliberately two files: `kasetto.lock` (`lock.rs`) is the committed,
portable record, and `state.rs` holds machine-local runtime state (last run, latest `doctor` report) in
the cache dir, keyed by lock path, and is safe to delete. Lock location follows scope: `./kasetto.lock`
for project, `$XDG_DATA_HOME/kasetto/` for global. The `--locked` / `--frozen` / `--update` triad is the
main branching axis in sync: `--locked` forbids all network access and errors if the lock cannot satisfy
the config, `--update` re-resolves moving refs (optionally narrowed via `update_only`). They are mutually
exclusive and rejected up front.

**Sources and caching.** `source/` fetches and extracts a repo tarball for a given ref. `source/hosts.rs`
classifies hosts and `source/auth.rs` maps them to env-var tokens; there is no login command or stored
credential by design. Cache population in `fsops/cache.rs` is atomic via a private tmp dir renamed into
place under `$XDG_CACHE_HOME/kasetto/sources/`.

**Transforms and secrets.** `instructions/` is the layer with a twist: some agents take a directory of
files, others an aggregate file (`CLAUDE.md`, `AGENTS.md`) that many instructions share, and aggregate
targets are written as managed `<!-- kasetto:instruction:ID ... -->` blocks so user hand-edits outside
the block survive a sync. `secrets/` resolves `${kst_...}` and tagged `${kst:<source>:<ref>}` placeholders
at sync time from env vars, a `credentials.yaml`, or an external secret manager, by shelling out to the
user's existing CLI session.

**Security audit.** `audit/` backs both `kst audit` and the post-install gate in `commands/sync/gate.rs`
(off unless `--audit [level]` or `audit.fail-on`; `--no-audit` wins). `audit::AuditThreshold` is the one
threshold type the CLI flags and `audit.fail-on` share, so `safe` is never a threshold. Targets come from
the lock: skills are looked up on skills.sh (`audit/skills_sh.rs`, `github.com` sources only, repo-level
verdicts), and each MCP server's definition is read back from the agent config and judged by the offline
rules in `audit/mcp.rs`; for the npm/PyPI package it launches, deps.dev (`audit/deps_dev.rs`) first
resolves the version a floating launch runs, then OSV (`audit/osv.rs`) is queried for that version and
deps.dev for deprecation and the source repo's Scorecard. Lookups run on a dedicated I/O pool, deduped
per package and per repo within a run, through a 24h TTL cache in `audit/cache.rs` that `kst clean`
clears. An asset that cannot be judged is unverified: it warns and never fails the gate unless
`fail-on-unverified` (config or flag) opts into failing closed. The gate runs before the sync report is
rendered, so `sync --json` carries its verdict under `audit` and `doctor` sees a failed gate. It is the
one network call `--locked` still makes, because the user opted into it.

**Update check and site counters.** `update_notifier.rs` asks `kasetto.dev/latest` (not the GitHub
API) for the newest version once per 24h, which is also the project's only active-install metric.
`self_update.rs` keeps its own `fetch_latest_release` against GitHub because it needs the release
asset list - the two are deliberately separate, so never collapse them. `KASETTO_NO_UPDATE_CHECK`
opts out. On the site, `site/lib/stats.ts` holds every counter (Upstash Redis via `KV_REST_API_*` or
`UPSTASH_REDIS_REST_*`), `/install` and `/install.ps1` count script downloads through
`site/lib/install-script.ts`, `/latest` counts CLI pings, and `/stats` reads them back. All of it
no-ops when no credentials are present, so previews and local dev still work.
