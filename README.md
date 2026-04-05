<p align="center">
  <img alt="Kasetto logo" src="assets/logo.svg" width="450" />
</p>

<p align="center">
  <a href="https://github.com/pivoshenko/kasetto/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/pivoshenko/kasetto/ci.yml?style=flat-square&logo=github&logoColor=white&label=CI&color=0A6847"></a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-Stable-0A6847?style=flat-square&logo=rust&logoColor=white">
  <a href="https://github.com/pivoshenko/kasetto/releases"><img alt="Release" src="https://img.shields.io/github/v/release/pivoshenko/kasetto?style=flat-square&logo=github&logoColor=white&color=4856CD&label=Release"></a>
  <a href="https://github.com/pivoshenko/kasetto/blob/main/LICENSE-MIT"><img alt="License" src="https://img.shields.io/badge/License-MIT%20%7C%20Apache--2.0-0A6847?style=flat-square&logo=opensourceinitiative&logoColor=white"></a>
  <a href="https://stand-with-ukraine.pp.ua"><img alt="Stand with Ukraine" src="https://img.shields.io/badge/Stand_With-Ukraine-FFD700?style=flat-square&labelColor=0057B7"></a>
</p>

<p align="center">
  An extremely fast AI skills manager, written in Rust.
</p>

Name comes from the Japanese word **カセット** (*kasetto*) - cassette. Think of skill sources as cassettes you plug in, swap out, and share across machines.

## Highlights

- **Declarative** - one YAML config describes your entire skill setup. Version it, share it, bootstrap a whole team in seconds.
- Syncs skills from **GitHub repos** or **local directories** into any agent environment.
- **35+ built-in agent presets**: Claude Code, Cursor, Codex, Windsurf, Copilot, Gemini CLI, and [many more](#supported-agents).
- Tracks every install in a local SQLite manifest - knows what changed and why.
- `--dry-run`, `--json`, and `--verbose` flags for scripting and CI.
- Ships as a single binary - install as `kasetto`, run as `kst`.

## Why Kasetto

There are good tools in this space already - [Vercel Skills](https://github.com/vercel-labs/skills) installs skills from a curated catalog, and [Claude Plugins](https://claude.com/plugins) offer runtime integrations. Both work well for one-off installs, but neither gives you a declarative, version-controlled config.

Kasetto is a **community-first** project that solves a different problem: **declarative, reproducible skill management across machines and agents.**

- **Team consistency** - commit a YAML file, everyone gets the same skills.
- **Multi-source** - pull from multiple GitHub repos and local folders in one config.
- **Agent-agnostic** - one config field switches between 35+ agent environments.
- **Traceable** - every install is tracked, diffable, and inspectable.
- **CI-friendly** - `--json` output and non-zero exit codes for automation.

> Inspired by [uv](https://github.com/astral-sh/uv) - what uv did for Python packages, Kasetto aims to do for AI skills.

## Install

### Standalone installer

**macOS and Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.sh | sh
```

**Windows:**

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.ps1 | iex"
```

By default the binary is placed in `~/.local/bin`. You can override this with environment variables:

| Variable              | Description            | Default                                                      |
| --------------------- | ---------------------- | ------------------------------------------------------------ |
| `KASETTO_VERSION`     | Version tag to install | Latest release                                               |
| `KASETTO_INSTALL_DIR` | Installation directory | `~/.local/bin` (Unix) / `%USERPROFILE%\.local\bin` (Windows) |

### Homebrew

```bash
brew install pivoshenko/tap/kasetto
```

### Cargo

```bash
cargo install kasetto
```

### From source

```bash
git clone https://github.com/pivoshenko/kasetto && cd kasetto
cargo install --path .
```

## Getting started

**1. Sync from a shared config or a local file:**

```bash
# from a remote URL (great for teams)
kst sync --config https://example.com/team-skills.yaml

# from a local file
kst sync --config kasetto.yaml
```

That's it. Kasetto reads the config, pulls the skills, and installs them into the right agent directory. Next time you run `sync`, only changed skills are updated.

**2. Explore what's installed:**

```bash
kst list      # interactive browser with vim-style navigation
kst doctor    # version, paths, last sync status
```

## Commands

### `kst sync`

Reads the config, discovers skills, and makes the local destination match.

```bash
kst sync [--config <path-or-url>] [--dry-run] [--quiet] [--json] [--plain] [--verbose]
```

| Flag        | What it does                                                       |
| ----------- | ------------------------------------------------------------------ |
| `--config`  | Path or HTTPS URL to a YAML config (default: `kasetto.yaml`) |
| `--dry-run` | Preview what would change without writing anything                 |
| `--quiet`   | Suppress non-error output                                          |
| `--json`    | Print the sync report as JSON                                      |
| `--plain`   | Disable colors and spinner animations                              |
| `--verbose` | Show per-skill action details                                      |

Missing skills are reported as broken but don't stop the run. The exit code is non-zero only for source-level failures.

### `kst list`

Shows everything currently tracked in the manifest.

```bash
kst list [--json]
```

In a terminal it opens an interactive browser - navigate with `j`/`k`, scroll with `PgUp`/`PgDn`, jump with `gg`/`G`. Set `NO_TUI=1` or pipe the output to get plain text instead.

### `kst doctor`

Prints local diagnostics: version, manifest DB path, installation paths, last sync time, and any failed skills from the latest run.

```bash
kst doctor [--json]
```

### `kst self update`

Checks GitHub for the latest release and replaces the current binary in-place.

```bash
kst self update [--json]
```

### `kst self uninstall`

Removes installed skills, local Kasetto config and data, and the binary.

```bash
kst self uninstall [--yes]
```

## Configuration

Pass a config via `--config` or let Kasetto pick up `kasetto.yaml` in the current directory.

```yaml
# Choose an agent preset...
agent: codex

# ...or set an explicit path (overrides agent)
# destination: ./my-skills

skills:
  # Pull specific skills from a GitHub repo
  - source: https://github.com/org/skill-pack
    branch: main
    skills:
      - code-reviewer
      - name: design-system

  # Sync everything from a local folder
  - source: ~/Development/my-skills
    skills: "*"

  # Override the subdirectory inside a repo
  - source: https://github.com/acme/monorepo
    skills:
      - name: custom-skill
        path: tools/skills
```

| Key               | Required | Description                                                         |
| ----------------- | -------- | ------------------------------------------------------------------- |
| `agent`           | no       | One of the [supported agent presets](#supported-agents)             |
| `destination`     | no       | Explicit install path - overrides `agent` if both are set           |
| `skills`          | **yes**  | List of skill sources                                               |
| `skills[].source` | **yes**  | GitHub URL or local path                                            |
| `skills[].branch` | no       | Branch for remote sources (default: `main`, falls back to `master`) |
| `skills[].skills` | **yes**  | `"*"` for all, or a list of names / `{ name, path }` objects        |

## Supported agents

Set the `agent` field and Kasetto handles the rest.

<details>
<summary>Full list of 35+ supported agents</summary>

<br />

| Agent          | Config value     | Install path                    |
| -------------- | ---------------- | ------------------------------- |
| AdaL           | `adal`           | `~/.adal/skills/`               |
| Amp            | `amp`            | `~/.config/agents/skills/`      |
| Antigravity    | `antigravity`    | `~/.gemini/antigravity/skills/` |
| Augment        | `augment`        | `~/.augment/skills/`            |
| Claude Code    | `claude-code`    | `~/.claude/skills/`             |
| Cline          | `cline`          | `~/.agents/skills/`             |
| CodeBuddy      | `codebuddy`      | `~/.codebuddy/skills/`          |
| Codex          | `codex`          | `~/.codex/skills/`              |
| Command Code   | `command-code`   | `~/.commandcode/skills/`        |
| Continue       | `continue`       | `~/.continue/skills/`           |
| Cortex Code    | `cortex`         | `~/.snowflake/cortex/skills/`   |
| Crush          | `crush`          | `~/.config/crush/skills/`       |
| Cursor         | `cursor`         | `~/.cursor/skills/`             |
| Deep Agents    | `deepagents`     | `~/.deepagents/agent/skills/`   |
| Droid          | `droid`          | `~/.factory/skills/`            |
| Gemini CLI     | `gemini-cli`     | `~/.gemini/skills/`             |
| GitHub Copilot | `github-copilot` | `~/.copilot/skills/`            |
| Goose          | `goose`          | `~/.config/goose/skills/`       |
| iFlow CLI      | `iflow-cli`      | `~/.iflow/skills/`              |
| Junie          | `junie`          | `~/.junie/skills/`              |
| Kilo Code      | `kilo`           | `~/.kilocode/skills/`           |
| Kimi Code CLI  | `kimi-cli`       | `~/.config/agents/skills/`      |
| Kiro CLI       | `kiro-cli`       | `~/.kiro/skills/`               |
| Kode           | `kode`           | `~/.kode/skills/`               |
| MCPJam         | `mcpjam`         | `~/.mcpjam/skills/`             |
| Mistral Vibe   | `mistral-vibe`   | `~/.vibe/skills/`               |
| Mux            | `mux`            | `~/.mux/skills/`                |
| Neovate        | `neovate`        | `~/.neovate/skills/`            |
| OpenClaw       | `openclaw`       | `~/.openclaw/skills/`           |
| OpenCode       | `opencode`       | `~/.config/opencode/skills/`    |
| OpenHands      | `openhands`      | `~/.openhands/skills/`          |
| Pi             | `pi`             | `~/.pi/agent/skills/`           |
| Pochi          | `pochi`          | `~/.pochi/skills/`              |
| Qoder          | `qoder`          | `~/.qoder/skills/`              |
| Qwen Code      | `qwen-code`      | `~/.qwen/skills/`               |
| Replit         | `replit`         | `~/.config/agents/skills/`      |
| Roo Code       | `roo`            | `~/.roo/skills/`                |
| Trae           | `trae`           | `~/.trae/skills/`               |
| Trae CN        | `trae-cn`        | `~/.trae-cn/skills/`            |
| Universal      | `universal`      | `~/.config/agents/skills/`      |
| Warp           | `warp`           | `~/.agents/skills/`             |
| Windsurf       | `windsurf`       | `~/.codeium/windsurf/skills/`   |
| Zencoder       | `zencoder`       | `~/.zencoder/skills/`           |

</details>

Need an agent that isn't listed? Use the `destination` field to point at any path.

## Roadmap

- MCP servers management
- Agents management
- Hooks management
- Your idea? [Open an issue](https://github.com/pivoshenko/kasetto/issues)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
