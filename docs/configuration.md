# Configuration

Pass a config via `--config` or let Kasetto pick up `kasetto.yaml` in the current directory.

## Example

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

## Reference

### Top-level fields

| Key | Required | Description |
| --- | --- | --- |
| `agent` | no | One of the [supported agent presets](./agents.md) |
| `destination` | no | Explicit install path — overrides `agent` if both are set |
| `skills` | **yes** | List of skill sources |

### Skill source fields

| Key | Required | Description |
| --- | --- | --- |
| `source` | **yes** | GitHub URL or local path |
| `branch` | no | Branch for remote sources (default: `main`, falls back to `master`) |
| `skills` | **yes** | `"*"` for all, or a list of names / `{ name, path }` objects |

### Skill entry fields

Each entry in the `skills` list can be a string (the skill name) or an object:

| Key | Required | Description |
| --- | --- | --- |
| `name` | **yes** | Name of the skill directory to install |
| `path` | no | Custom subdirectory within the source to look for the skill |

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
