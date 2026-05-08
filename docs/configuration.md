# Configuration

When `--config` is omitted, Kasetto looks for config in this order:

1. `$KASETTO_CONFIG` env var
2. `source:` key in `$XDG_CONFIG_HOME/kasetto/config.yaml`
3. `./kasetto.yaml`
4. `$XDG_CONFIG_HOME/kasetto/kasetto.yaml` (or `~/.config/kasetto/kasetto.yaml`)

Point it at a specific file or URL with `--config`, or run `kst init` for local `./kasetto.yaml` (`kst init --global` writes the global config file).
To persist a remote URL as your default, add a `source:` key to `~/.config/kasetto/config.yaml`.

## Example

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

  # Pin to a specific git tag
  - source: https://github.com/acme/monorepo
    ref: v1.2.0
    skills:
      - name: custom-skill
        path: tools/skills

  # Limit discovery to a nested directory inside the source
  - source: https://github.com/acme/agents
    sub-dir: plugins/swift-apple-expert
    skills: "*"

# MCP servers (optional)
mcps:
  # Discover all MCP files in the repo
  - source: https://github.com/org/mcp-pack
    mcps: "*"

  # Pick specific files from a monorepo (resolved from mcps/ dir)
  - source: https://github.com/org/monorepo
    ref: v1.4.0
    mcps:
      - github        # → mcps/github.json
      - linear        # → mcps/linear.json

  # Custom directory via { name, path }
  - source: https://github.com/org/other
    mcps:
      - name: my-server
        path: tools   # → tools/my-server.json

# Hooks (optional)
hooks:
  pre_sync:
    - ./scripts/validate-skills.sh
  post_sync:
    - echo "Synced $KASETTO_INSTALLED skills"
```

## Reference

### Top-Level Fields

| Key           | Required | Description                                                         |
| ------------- | -------- | ------------------------------------------------------------------- |
| `agent`       | no       | One or more [supported agent presets](./agents.md) - string or list |
| `destination` | no       | Explicit install path - overrides `agent` if both are set           |
| `scope`       | no       | `"global"` (default) or `"project"` - where to install              |
| `skills`      | **yes**  | List of skill sources                                               |
| `mcps`        | no       | List of MCP server sources                                          |
| `hooks`       | no       | Optional `pre_sync` and `post_sync` hook commands                   |

### Skill Source Fields

| Key       | Required | Description                                                                                    |
| --------- | -------- | ---------------------------------------------------------------------------------------------- |
| `source`  | **yes**  | Git host URL or local path (GitHub, GitLab, Bitbucket, Codeberg/Gitea)                         |
| `branch`  | no       | Branch for remote sources (default: `main`, falls back to `master`)                            |
| `ref`     | no       | Git tag, commit SHA, or ref - takes priority over `branch`                                     |
| `sub-dir` | no       | Relative subdirectory within the source used as the discovery root (`sub_dir` alias supported) |
| `skills`  | **yes**  | `"*"` for all, or a list of names / `{ name, path }` objects                                   |

### Skill Entry Fields

Each entry in the `skills` list can be a string (the skill name) or an object:

| Key    | Required | Description                                                                                   |
| ------ | -------- | --------------------------------------------------------------------------------------------- |
| `name` | **yes**  | Name of the skill directory to install                                                        |
| `path` | no       | Parent directory containing `<name>/SKILL.md`, resolved relative to the source root (or `sub-dir` if set). Absolute paths are honored as-is. |

| Form                                | Resolves to                                |
| ----------------------------------- | ------------------------------------------ |
| `- code-reviewer`                   | discovered (root or `skills/`)             |
| `- { name: x }`                     | discovered (root or `skills/`)             |
| `- { name: x, path: dir }`          | `dir/x/SKILL.md`                           |
| `- { name: x, path: nested/dir }`   | `nested/dir/x/SKILL.md`                    |

### MCP Source Fields

| Key      | Required | Description                                                          |
| -------- | -------- | -------------------------------------------------------------------- |
| `source` | **yes**  | Git host URL or local path containing MCP server config              |
| `branch` | no       | Branch for remote sources (default: `main`, falls back to `master`)  |
| `ref`    | no       | Git tag, commit SHA, or ref - takes priority over `branch`           |
| `mcps`   | **yes**  | `"*"` to discover all, or a list of names / `{ name, path }` objects |

When `mcps: "*"`, Kasetto auto-discovers MCP config files in this order:

1. `.mcp.json` at the source root
2. `mcp.json` at the source root
3. Any `.json` file inside the `mcps/` subdirectory

### MCP Entry Fields

Each entry in the `mcps` list can be a plain string (name) or an object — mirrors skill entries:

| Form                              | Resolves to             |
| --------------------------------- | ----------------------- |
| `- github`                        | `mcps/github.json`      |
| `- github.json`                   | `mcps/github.json`      |
| `- { name: x }`                   | `mcps/x.json`           |
| `- { name: x, path: dir }`        | `dir/x.json`            |
| `- { name: x, path: nested/dir }` | `nested/dir/x.json`     |

Paths are resolved relative to the source root (or `sub-dir` if set); absolute paths are honored as-is. `.json` is appended automatically when the name has no extension.


MCP config files must contain a `mcpServers` object with server definitions. Servers are merged
into each agent's native settings file (e.g., `.claude.json` for Claude Code, `.cursor/mcp.json`
for Cursor). See [how sync works](./how-sync-works.md) for merge behavior details.

## Hooks

Define shell commands that run before or after `kst sync`. Hooks can be placed in your **local**
`kasetto.yaml` or in the **global** config at `~/.config/kasetto/kasetto.yaml`.

If both the local and global config define hooks, **local takes priority** and the global hooks
are ignored entirely.

### Example

```yaml
hooks:
  pre_sync:
    - echo "Starting sync..."
    - ./scripts/validate-skills.sh
  post_sync:
    - echo "Synced $KASETTO_INSTALLED skills, updated $KASETTO_UPDATED"
    - curl -X POST https://hooks.slack.com/... -d '{"text":"Kasetto sync complete"}'
```

### Reference

| Key               | Description                                                                   |
| ----------------- | ----------------------------------------------------------------------------- |
| `hooks`           | Object containing `pre_sync` and/or `post_sync` arrays                        |
| `hooks.pre_sync`  | Shell commands run before sync via `sh -c`. Any failure aborts the sync.      |
| `hooks.post_sync` | Shell commands run after sync completes. Receives report via `KASETTO_*` env vars. |

Hooks run in the order listed. Use `--no-hooks` to skip all hooks for a single run.

`post_sync` hooks receive the following environment variables:

| Variable              | Example value        |
| --------------------- | -------------------- |
| `KASETTO_RUN_ID`      | `1746700000`         |
| `KASETTO_CONFIG`      | `kasetto.yaml`       |
| `KASETTO_DESTINATION` | `/home/user/.codex/skills` |
| `KASETTO_DRY_RUN`     | `0` or `1`           |
| `KASETTO_INSTALLED`   | `2`                  |
| `KASETTO_UPDATED`     | `1`                  |
| `KASETTO_REMOVED`     | `0`                  |
| `KASETTO_UNCHANGED`   | `5`                  |
| `KASETTO_BROKEN`      | `0`                  |
| `KASETTO_FAILED`      | `0`                  |

## Remote Configs

Kasetto can fetch configs from any HTTPS URL:

```bash
kst sync --config https://example.com/team-skills.yaml
```

Great for sharing a single config across a team without checking it into every repository.

### Real-world example

[pivoshenko/pivoshenko.ai](https://github.com/pivoshenko/pivoshenko.ai) is a public config that pulls skills from several community packs for Claude Code and OpenCode:

```bash
kst sync --config https://github.com/pivoshenko/pivoshenko.ai/blob/main/kasetto.yaml
```

Kasetto recognises browser URLs from GitHub, GitLab, and Gitea / Codeberg / Forgejo, and auto-rewrites them to the matching raw-content endpoint. You can paste any of these directly:

- `https://github.com/owner/repo/blob/main/kasetto.yaml`
- `https://gitlab.com/group/repo/-/blob/main/kasetto.yaml`
- `https://codeberg.org/owner/repo/src/branch/main/kasetto.yaml`

Kasetto prints a short `note: rewrote browser URL to raw content: ...` line so you can see what was fetched. Authentication is resolved against the rewritten host, so the same tokens that work for raw URLs apply here too.

If the URL points to a private repo, Kasetto uses the same token-based authentication as skill sources. See [authentication](./authentication.md) for the full list of supported environment variables.

## Multiple Agents

The `agent` field accepts a single value or a list. With a list, Kasetto installs skills to every agent's directory and merges MCP servers into every agent's settings file:

```yaml
agent:
  - claude-code
  - cursor
  - codex

skills:
  - source: https://github.com/org/skill-pack
    skills: "*"
```

Handy when you juggle multiple agents and want them all to share the same skill set.

## Agent vs Destination

If you set both, `destination` wins. Use `agent` for convenience with [supported presets](./agents.md), or `destination` when you need full control over the install path.

!!! tip

    Use `destination` when targeting an agent that isn't in the supported list.

## Scope: Global Vs Project

By default, skills are installed globally into the agent's home-directory path. Add `scope: project` to your config, or pass `--project` on the command line, to install into the current project directory instead.

The `--project` / `--global` flags always override whatever `scope` is set in the config file.

## Environment Variables

These environment variables affect Kasetto's output behavior:

| Variable   | Effect                                                                    |
| ---------- | ------------------------------------------------------------------------- |
| `NO_TUI`   | Disables interactive screens (home menu, list browser). Set to any value. |
| `NO_COLOR` | Disables colored output. Set to any value.                                |
