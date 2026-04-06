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
- Syncs skills from **GitHub, GitLab, Bitbucket, Codeberg/Gitea** repos or **local directories** - including **GitHub Enterprise** and **self-hosted GitLab**.
- **21 built-in agent presets**: Claude Code, Cursor, Codex, Windsurf, Copilot, Gemini CLI, and [many more](#supported-agents).
- **MCP server management**: declare MCP servers in the same config and Kasetto merges them into each agent's native settings file.
- Tracks every install in a local manifest - knows what changed and why.
- `--dry-run`, `--json`, and `--verbose` flags for scripting and CI.
- Ships as a single binary - install as `kasetto`, run as `kst`.

## Why Kasetto

There are good tools in this space already - [Vercel Skills](https://github.com/vercel-labs/skills) installs skills from a curated catalog, and [Claude Plugins](https://claude.com/plugins) offer runtime integrations. Both work well for one-off installs, but neither gives you a declarative, version-controlled config.

Kasetto is a **community-first** project that solves a different problem: **declarative, reproducible skill management across machines and agents.**

- **Team consistency** - commit a YAML file, everyone gets the same skills.
- **Multi-source** - pull from GitHub, GitLab, Bitbucket, Codeberg/Gitea repos and local folders in one config.
- **Agent-agnostic** - one config field switches between 21 agent environments.
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

## Getting Started

**1. Sync from a remote config or a local file:**

```bash
# pull a shared team config from a URL
kst sync --config https://example.com/team-skills.yaml

# or use a local file
kst sync --config kasetto.yaml
```

That's it. Kasetto pulls the skills and installs them into the right agent directory. The next time you run `sync`, only what changed gets updated.

**2. See what's installed:**

```bash
kst list      # interactive browser with vim-style navigation
kst doctor    # version, paths, last sync status
```

## Commands

### `kst init`

Generates a starter `kasetto.yaml` in the current directory.

```bash
kst init [--force]
```

| Flag      | What it does                                        |
| --------- | --------------------------------------------------- |
| `--force` | Overwrite an existing `kasetto.yaml` without asking |

### `kst sync`

Reads the config, discovers skills, and makes the local destination match.

```bash
kst sync [--config <path-or-url>] [--dry-run] [--quiet] [--json] [--plain] [--verbose] [--project | --global]
```

| Flag        | What it does                                                 |
| ----------- | ------------------------------------------------------------ |
| `--config`  | Path or HTTPS URL to a YAML config (default: `kasetto.yaml`) |
| `--dry-run` | Preview what would change without writing anything           |
| `--quiet`   | Suppress non-error output                                    |
| `--json`    | Print the sync report as JSON                                |
| `--plain`   | Disable colors and spinner animations                        |
| `--verbose` | Show per-skill action details                                |
| `--project` | Install into the current project directory                   |
| `--global`  | Install globally (default)                                   |

Missing skills are reported as broken but won't stop the rest of the run. The exit code is non-zero only for source-level failures.

### `kst list`

Shows skills and MCP servers from the lock file(s). **Without** `--project` or `--global`, both scopes are merged so you can tell global and project installs apart (scope is shown per row / in JSON).

```bash
kst list [--json] [--quiet] [--plain] [--project | --global]
```

In a terminal (and without `--plain`), this opens an interactive browser — Skills and MCPs tabs with detail panes. Navigate with `j`/`k`, switch tabs with Tab or `h`/`l`, scroll with `PgUp`/`PgDn`, jump with `gg`/`G`. Use `--plain`, set `NO_TUI=1`, or pipe stdout for a plain text listing.

### `kst doctor`

Prints local diagnostics: version, lock file path, installation paths, last sync time, and any failed skills from the latest run.

```bash
kst doctor [--json] [--quiet] [--plain] [--project | --global]
```

### `kst clean`

Removes all tracked skills and MCP configs for the given scope.

```bash
kst clean [--dry-run] [--json] [--quiet] [--plain] [--project | --global]
```

| Flag        | What it does                                               |
| ----------- | ---------------------------------------------------------- |
| `--dry-run` | Preview what would be removed (prints paths and MCP packs) |
| `--json`    | Print output as JSON                                       |
| `--quiet`   | Suppress non-error output                                  |
| `--plain`   | Disable colors and banner-style header                     |
| `--project` | Clean project-scoped assets                                |
| `--global`  | Clean globally-scoped assets (default)                     |

### `kst self update`

Checks GitHub for the latest release and replaces the current binary in-place.

```bash
kst self update [--json]
```

### `kst self uninstall`

Removes installed skills, MCP configs, Kasetto data, and the binary.

```bash
kst self uninstall [--yes]
```

### `kst completions`

Generates shell completion scripts.

```bash
kst completions <shell>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`.

## Configuration

Kasetto looks for `kasetto.yaml` in the current directory by default. Point it at a specific file or URL with `--config`, or run `kst init` to generate a starter.

```yaml
# Choose an agent preset (single or multiple)...
agent: codex
# agent:
#   - claude-code
#   - cursor

# ...or set an explicit path (overrides agent)
# destination: ./my-skills

# Install scope: "global" (default) or "project"
# scope: project

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

  # Pin to a git tag or commit
  - source: https://github.com/acme/stable-skills
    ref: v1.2.0
    skills:
      - name: custom-skill
        path: tools/skills

# MCP servers (optional)
mcps:
  - source: https://github.com/org/mcp-pack
  - source: https://github.com/org/monorepo
    path: mcps/my-server/pack.json
```

| Key               | Required | Description                                                         |
| ----------------- | -------- | ------------------------------------------------------------------- |
| `agent`           | no       | One or more [supported agent presets](#supported-agents)            |
| `destination`     | no       | Explicit install path - overrides `agent` if both are set           |
| `scope`           | no       | `"global"` (default) or `"project"` - where to install              |
| `skills`          | **yes**  | List of skill sources                                               |
| `skills[].source` | **yes**  | Git host URL or local path                                          |
| `skills[].branch` | no       | Branch for remote sources (default: `main`, falls back to `master`) |
| `skills[].ref`    | no       | Git tag, commit SHA, or ref (takes priority over `branch`)          |
| `skills[].skills` | **yes**  | `"*"` for all, or a list of names / `{ name, path }` objects        |
| `mcps`            | no       | List of MCP server sources                                          |
| `mcps[].source`   | **yes**  | Git host URL or local path containing MCP config                    |
| `mcps[].branch`   | no       | Branch for remote sources                                           |
| `mcps[].ref`      | no       | Git tag, commit SHA, or ref                                         |
| `mcps[].path`     | no       | Explicit path to MCP JSON file within the source                    |

## Supported agents

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

## Private repos & enterprise

Set an environment variable and private sources just work — no login command, no credentials file:

| Host                        | Environment variable                                |
| --------------------------- | --------------------------------------------------- |
| GitHub / GitHub Enterprise  | `GITHUB_TOKEN` or `GH_TOKEN`                        |
| GitLab / GitLab self-hosted | `GITLAB_TOKEN` or `CI_JOB_TOKEN`                    |
| Bitbucket Cloud             | `BITBUCKET_EMAIL` + `BITBUCKET_TOKEN`               |
| Codeberg / Gitea / Forgejo  | `GITEA_TOKEN`, `CODEBERG_TOKEN`, or `FORGEJO_TOKEN` |

Kasetto auto-detects GitHub Enterprise for any hostname with an `owner/repo` path, and GitLab self-hosted when the hostname starts with `gitlab.`.

```yaml
skills:
  # GitHub Enterprise
  - source: https://ghe.example.com/acme/skill-pack
    skills: "*"

  # Self-hosted GitLab (nested groups supported)
  - source: https://gitlab.example.com/team/ai/skills
    skills:
      - code-reviewer
```

The same tokens apply when you fetch a remote config via `--config https://...`.

## Roadmap

- Agents management
- Hooks management
- Your idea? [Open an issue](https://github.com/pivoshenko/kasetto/issues)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
