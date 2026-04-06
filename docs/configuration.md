# Configuration

Pass a config via `--config` or let Kasetto pick up `kasetto.yaml` in the current directory.
You can also run `kst init` to generate a starter config.

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

# MCP servers (optional)
mcps:
  - source: https://github.com/org/mcp-pack
  - source: https://github.com/org/monorepo
    path: mcps/my-server/pack.json
```

## Reference

### Top-level fields

| Key | Required | Description |
| --- | --- | --- |
| `agent` | no | One or more [supported agent presets](./agents.md) — string or list |
| `destination` | no | Explicit install path — overrides `agent` if both are set |
| `scope` | no | `"global"` (default) or `"project"` — where to install |
| `skills` | **yes** | List of skill sources |
| `mcps` | no | List of MCP server sources |

### Skill source fields

| Key | Required | Description |
| --- | --- | --- |
| `source` | **yes** | Git host URL or local path (GitHub, GitLab, Bitbucket, Codeberg/Gitea) |
| `branch` | no | Branch for remote sources (default: `main`, falls back to `master`) |
| `ref` | no | Git tag, commit SHA, or ref — takes priority over `branch` |
| `skills` | **yes** | `"*"` for all, or a list of names / `{ name, path }` objects |

### Skill entry fields

Each entry in the `skills` list can be a string (the skill name) or an object:

| Key | Required | Description |
| --- | --- | --- |
| `name` | **yes** | Name of the skill directory to install |
| `path` | no | Custom subdirectory within the source to look for the skill |

### MCP source fields

| Key | Required | Description |
| --- | --- | --- |
| `source` | **yes** | Git host URL or local path containing MCP server config |
| `branch` | no | Branch for remote sources (default: `main`, falls back to `master`) |
| `ref` | no | Git tag, commit SHA, or ref — takes priority over `branch` |
| `path` | no | Explicit path to the MCP JSON file within the source (skips auto-discovery) |

Kasetto discovers MCP config files automatically in this order:

1. `.mcp.json` at the source root
2. `mcp.json` at the source root
3. Any `.json` file inside a `mcp/` subdirectory

Use the `path` field to point at a specific file when the source contains multiple configs or uses
a non-standard layout.

MCP config files must contain a `mcpServers` object with server definitions. Servers are merged
into each agent's native settings file (e.g., `.claude.json` for Claude Code, `.cursor/mcp.json`
for Cursor). See [how sync works](./how-sync-works.md) for merge behavior details.

## Remote configs

Kasetto can fetch configs from any HTTPS URL:

```console
$ kst sync --config https://example.com/team-skills.yaml
```

This is useful for sharing a single config across a team without checking it into every repository.

If the remote config is hosted on a private repo, Kasetto applies the same token-based
authentication used for skill sources. See [authentication](./authentication.md) for the full list
of supported environment variables.

## Multiple agents

The `agent` field accepts a single value or a list. When a list is provided, skills are installed
to every agent's directory and MCP servers are merged into every agent's settings file:

```yaml
agent:
  - claude-code
  - cursor
  - codex

skills:
  - source: https://github.com/org/skill-pack
    skills: "*"
```

This is useful when you work with several agents and want them all to share the same skills.

## Agent vs destination

If both `agent` and `destination` are set, `destination` takes priority. Use `agent` for
convenience with [supported presets](./agents.md), or `destination` for full control over the
install path.

!!! tip

    Use `destination` when targeting an agent that isn't in the supported list.

## Scope: global vs project

By default, Kasetto installs skills globally (into the agent's home-directory path). Set
`scope: project` in the config or pass `--project` on the command line to install into the
current project directory instead.

The `--project` / `--global` CLI flags override the config file's `scope` field.
