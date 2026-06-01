<p align="center">
  <a href="https://www.kasetto.dev/"><img alt="Kasetto logo" src="assets/logo.svg" width="450" /></a>
</p>

<p align="center">
  <a href="https://github.com/pivoshenko/kasetto/actions/workflows/ci.yaml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/pivoshenko/kasetto/ci.yaml?style=flat-square&logo=github&logoColor=white&label=CI&color=0A6847"></a>
  <a href="https://github.com/pivoshenko/kasetto/releases"><img alt="Release" src="https://img.shields.io/github/v/release/pivoshenko/kasetto?style=flat-square&logo=github&logoColor=white&color=4856CD&label=Release"></a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-Stable-F74C00?style=flat-square&logo=rust&logoColor=white">
  <a href="https://github.com/pivoshenko/kasetto/blob/main/LICENSE-MIT"><img alt="License" src="https://img.shields.io/badge/License-MIT%20%7C%20Apache--2.0-0A6847?style=flat-square&logo=opensourceinitiative&logoColor=white"></a>
  <a href="https://stand-with-ukraine.pp.ua"><img alt="Stand with Ukraine" src="https://img.shields.io/badge/Stand_With-Ukraine-FFD700?style=flat-square&labelColor=0057B7"></a>
</p>

<p align="center">
  <a href="https://backlinklog.com/listing/kasetto.dev?utm_source=backlinklog&utm_medium=badge"><img alt="Listed on BacklinkLog" src="https://backlinklog.com/badge/kasetto.dev.svg" width="160" height="40"></a>
</p>

<p align="center">
  A declarative AI agent environment manager, written in Rust.
</p>

<img alt="kasetto sync output" src="assets/demo.svg" width="100%" />

**About the name**

Name comes from the Japanese word **カセット** (*kasetto*) - cassette. Think of Skills, MCPs, commands as cassettes you plug in, swap out, and share across machines.

## Why Kasetto

There are good tools in this space already - [Vercel Skills](https://github.com/vercel-labs/skills) installs skills from a curated catalog, and [Claude Plugins](https://claude.com/plugins) offer runtime integrations. Both work well for one-off installs, but neither gives you a declarative, version-controlled config.

Kasetto is a **community-first** project that solves a different problem: **declarative, reproducible skill management across machines and agents.**

- **Declarative** — one YAML file, your whole setup: skills, commands, MCPs, and agents. Apply globally or scope to a project; configs compose with `extends`, so org, team, and project stay in sync.
- **Enterprise & private repositories** — pull from anywhere: GitHub, GitLab, Bitbucket, Codeberg, Gitea, and self-hosted instances, public or private. Onboard a new engineer with one command; everyone gets the same environment, zero drift.
- **Multi-agent** — write once, ship everywhere. Claude Code, Cursor, Codex, Windsurf, Copilot, Gemini CLI, and [many more](#supported-agents) — one sync keeps them all current.
- **Skills, Commands & MCPs** — all three asset kinds, one source: skills, commands, and MCPs. Everything is transformed into each agent's native format, and auto-merged. Distribute rules, tools, and prompts as easily as sharing a repository link.
- **Speed** — instant by design. Built in Rust, it hashes content and diffs a lock file so only what changed gets touched — full syncs finish in seconds.
- **Universal** — one static binary for macOS, Linux, and Windows. Install as `kasetto`, run as `kst`. CI-friendly with `--json` output and real exit codes.

> Inspired by [cargo](https://github.com/rust-lang/cargo) and [uv](https://github.com/astral-sh/uv) — the same lock-first, declarative, CLI-only ergonomics, applied to AI agent skills.

## Install

### Standalone Installer

**macOS and Linux:**

```bash
curl -fsSL kasetto.dev/install | sh
```

**Windows:**

```powershell
powershell -ExecutionPolicy Bypass -c "irm kasetto.dev/install.ps1 | iex"
```

### Homebrew

```bash
brew install pivoshenko/tap/kasetto
```

### Scoop (Windows)

```bash
scoop bucket add kasetto https://github.com/pivoshenko/scoop-bucket
scoop install kasetto
```

### Cargo

```bash
cargo install kasetto
```

## Getting Started

**1. Scaffold a config:**

```bash
kst init            # creates ./kasetto.yaml in the current directory
kst init --global   # or a global one at ~/.config/kasetto/kasetto.yaml
```

Edit the generated `kasetto.yaml` — pick an `agent`, add a `skills:` source, and you're ready to sync.

**2. Sync skills into your agents:**

```bash
# uses ./kasetto.yaml in the current directory
kst sync

# or point at a shared team config over HTTPS
kst sync --config https://example.com/team-skills.yaml
```

Want bare `kst sync` to always pull from a remote URL? Persist it in `~/.config/kasetto/config.yaml`:

```yaml
source: https://github.com/pivoshenko/pivoshenko.ai/blob/main/kasetto.yaml
```

After that, `kst sync` resolves the URL automatically — no `--config` flag needed.

That's it. Kasetto pulls the skills, installs them into the right agent directory, and records exactly what it installed in `kasetto.lock`. Commit `kasetto.yaml` and `kasetto.lock` together (like `Cargo.lock` or `package-lock.json`) and every teammate gets identical versions. A plain `kst sync` honors the lock without re-resolving moving refs; `kst sync --update` rolls versions forward; `kst sync --locked` enforces the lock in CI.

See [pivoshenko/pivoshenko.ai](https://github.com/pivoshenko/pivoshenko.ai) for a community example pulling skills from multiple packs for Claude Code and OpenCode.

**3. See what's installed:**

```bash
kst list                    # table of installed skills, MCPs, commands
kst list --type skills      # filter to one asset kind
kst doctor                  # version, paths, last sync status
```

## Commands

One-line synopsis below. Full flags and examples in the [commands reference](https://kasetto.dev/docs/commands).

- **`kst init`** — generate a starter `kasetto.yaml` (local or `--global`).
- **`kst sync`** — read config, install skills + MCPs into agent dirs honoring `kasetto.lock`; `--update` rolls pins forward, `--locked`/`--frozen` enforce the lock without fetching.
- **`kst list`** — print a uv-style table of installed skills, MCPs, and commands from the lock file; `--type skills|mcps|commands` filters; `--json` for scripting.
- **`kst doctor`** — local diagnostics: version, paths, last sync status, broken skills.
- **`kst clean`** — remove tracked skills and MCP configs for the given scope.
- **`kst self update`** — fetch latest release, verify SHA256, replace binary in place.
- **`kst self uninstall`** — remove installed assets, data, and the binary.
- **`kst completions <shell>`** — emit shell completion script (`bash`/`zsh`/`fish`/`powershell`).

Most commands accept `--json`, `--color <auto|always|never>`, `-q`/`--quiet` (repeat for stricter silence), and `--project | --global`. `--plain` is still accepted as a deprecated alias for `--color never`.

## Configuration

When `--config` is omitted, Kasetto looks for config in this order:

1. `$KASETTO_CONFIG` env var
2. `./kasetto.yaml`
3. `source:` key in `$XDG_CONFIG_HOME/kasetto/config.yaml`
4. `$XDG_CONFIG_HOME/kasetto/kasetto.yaml` (or `~/.config/kasetto/kasetto.yaml`)

Run `kst init` to scaffold a local config, or `kst init --global` for the global one.

<!-- kasetto-config:start -->
```yaml
# Option A: preset destination by agent (see README for supported agent values)
agent:
  - codex
  - claude-code

# Option B: manual destination (takes precedence if both are set)
# destination: ./.agents/skills

skills:
  # "*" syncs every skill in the source — each is a directory with a SKILL.md,
  # discovered in the source root or its skills/ subdirectory
  - source: https://github.com/vercel-labs/next-skills
    # ref: v1.0.0   # pin to a tag or commit; omit to track the default branch
    skills: "*"

  # or list skills by name
  - source: https://github.com/anthropics/skills
    skills:
      - doc-coauthoring
      - frontend-design
      - pptx

  # sub-dir: resolve the named skills under this path, e.g. skills/productivity/grill-me/
  - source: https://github.com/mattpocock/skills
    sub-dir: skills/productivity
    skills:
      - grill-me
      - caveman

  # path: a skill in a non-standard location → <path>/<name>/, here skills/engineering/improve-codebase-architecture/
  - source: https://github.com/mattpocock/skills
    skills:
      - name: improve-codebase-architecture
        path: skills/engineering

commands:
  # names resolve to commands/<name>.md in the source (nested dirs namespace, e.g. git:commit)
  - source: https://github.com/gsd-build/get-shit-done
    commands:
      - gsd:explore
      - gsd:fast

mcps:
  # names resolve to mcps/<name>.json in the source
  - source: https://github.com/pivoshenko/pivoshenko.ai
    branch: main   # track a specific branch (use ref: to pin a tag or commit)
    mcps:
      - github
      - vercel
      - kaggle
```
<!-- kasetto-config:end -->

Full key reference, merge rules, and `extends:` inheritance live in the [configuration docs](https://kasetto.dev/docs/configuration).

## Supported Agents

Set the `agent` field and Kasetto figures out where to put things.

<details>
<summary>Full list of supported agents</summary>

<br />

| Agent          | Config value     | Install path                    |
| -------------- | ---------------- | ------------------------------- |
| Amp            | `amp`            | `~/.config/agents/skills/`      |
| Antigravity    | `antigravity`    | `~/.gemini/antigravity/skills/` |
| Augment        | `augment`        | `~/.augment/skills/`            |
| Claude Code    | `claude-code`    | `~/.claude/skills/`             |
| Cline          | `cline`          | `~/.agents/skills/`             |
| Codex          | `codex`          | `~/.codex/skills/`              |
| Continue       | `continue`       | `~/.continue/skills/`           |
| Cursor         | `cursor`         | `~/.cursor/skills/`             |
| Gemini CLI     | `gemini-cli`     | `~/.gemini/skills/`             |
| GitHub Copilot | `github-copilot` | `~/.copilot/skills/`            |
| Goose          | `goose`          | `~/.config/goose/skills/`       |
| Junie          | `junie`          | `~/.junie/skills/`              |
| Kiro CLI       | `kiro-cli`       | `~/.kiro/skills/`               |
| OpenClaw       | `openclaw`       | `~/.openclaw/skills/`           |
| OpenCode       | `opencode`       | `~/.config/opencode/skills/`    |
| OpenHands      | `openhands`      | `~/.openhands/skills/`          |
| Replit         | `replit`         | `~/.config/agents/skills/`      |
| Roo Code       | `roo`            | `~/.roo/skills/`                |
| Trae           | `trae`           | `~/.trae/skills/`               |
| Warp           | `warp`           | `~/.agents/skills/`             |
| Windsurf       | `windsurf`       | `~/.codeium/windsurf/skills/`   |

</details>

Don't see your agent? Use the `destination` field to point at any path.

## Private Repositories & Enterprise

Private GitHub, GitLab, Bitbucket, Codeberg, Gitea, and self-hosted instances work via env-var tokens (`GITHUB_TOKEN`, `GITLAB_TOKEN`, `BITBUCKET_TOKEN`, `GITEA_TOKEN`, etc.) — no login command, no credentials file. The same tokens apply to remote `--config` URLs.

Full host table and auth resolution rules in the [authentication docs](https://kasetto.dev/docs/authentication).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
