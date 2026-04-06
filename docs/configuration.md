# Configuration

Pass a config via `--config` or let Kasetto pick up `kasetto.yaml` in the current directory.
You can also run `kst init` to generate a starter config.

## Example

```yaml
# Choose an agent preset...
agent: codex

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
| `agent` | no | One of the [supported agent presets](./agents.md) |
| `destination` | no | Explicit install path — overrides `agent` if both are set |
| `scope` | no | `"global"` (default) or `"project"` — where to install |
| `skills` | **yes** | List of skill sources |
| `mcps` | no | List of MCP server sources |

### Skill source fields

| Key | Required | Description |
| --- | --- | --- |
| `source` | **yes** | GitHub URL or local path |
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
| `source` | **yes** | GitHub URL or local path containing MCP server config |
| `branch` | no | Branch for remote sources (default: `main`, falls back to `master`) |
| `ref` | no | Git tag, commit SHA, or ref — takes priority over `branch` |
| `path` | no | Explicit path to the MCP JSON file within the source (skips auto-discovery) |

Kasetto discovers MCP config files automatically (looking for files like `pack.json`), but you can
use the `path` field to point at a specific file when the source contains multiple configs or uses a
non-standard layout.

MCP servers are merged into each agent's native settings file (e.g., `.claude.json` for Claude Code,
`.cursor/mcp.json` for Cursor). The `clean` command removes all managed MCP entries.

## Remote configs

Kasetto can fetch configs from any HTTPS URL:

```console
$ kst sync --config https://example.com/team-skills.yaml
```

This is useful for sharing a single config across a team without checking it into every repository.

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
