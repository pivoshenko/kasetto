# First steps with Kasetto

After [installing Kasetto](./installation.md), you can check that it is available by running the
`kst` command:

```console
$ kst
An extremely fast AI skills manager

Usage: kst <COMMAND>

...
```

If no `kasetto.yaml` exists in the current directory, Kasetto opens an interactive home screen with
a menu of available commands. Navigate with ++j++ / ++k++ or arrow keys, press ++enter++ to run the
selected command, or use shortcut keys:

| Key | Action |
| --- | --- |
| ++i++ | Init |
| ++s++ | Sync (prompts for config path) |
| ++l++ | List |
| ++d++ | Doctor |
| ++c++ | Clean |
| ++u++ | Self update |
| ++q++ / ++escape++ | Quit |

Set `NO_TUI=1` to disable the interactive home screen and get plain text hints instead.

## Creating a config

Generate a starter config with `kst init`:

```console
$ kst init
```

Or create a `kasetto.yaml` manually:

```yaml
agent: claude-code

skills:
  - source: https://github.com/org/skill-pack
    branch: main
    skills:
      - code-reviewer
      - name: design-system
```

!!! tip

    Use the `agent` field to target any of the [supported agents](./agents.md), or use the
    `destination` field for a custom install path.

## Syncing skills

Run `kst sync` to install the declared skills:

```console
$ kst sync
Syncing skills from 1 source...
  ✓ code-reviewer (installed)
  ✓ design-system (installed)
Synced 2 skills in 1.2s
```

Kasetto reads the config, pulls the skills, and installs them into the right agent directory.
Next time you run `sync`, only changed skills are updated.

## Syncing from a remote config

Kasetto can fetch configs from any HTTPS URL — useful for sharing a single config across a team:

```console
$ kst sync --config https://example.com/team-skills.yaml
```

## Previewing changes

Use `--dry-run` to preview what would change without writing anything:

```console
$ kst sync --dry-run
Would install: code-reviewer, design-system
Would remove: old-skill
```

## MCP servers

Kasetto can also manage MCP server configs. Add an `mcps` section to your config:

```yaml
agent: claude-code

skills:
  - source: https://github.com/org/skill-pack
    skills: "*"

mcps:
  - source: https://github.com/org/mcp-pack
```

MCP servers are automatically merged into each agent's native settings file during `kst sync`.

## Exploring what's installed

Browse installed skills interactively:

```console
$ kst list
```

Navigate with ++j++ / ++k++, scroll with ++page-up++ / ++page-down++, jump with ++g++ ++g++ / ++shift+g++.
Set `NO_TUI=1` or pipe the output to get plain text instead.

Check your local setup:

```console
$ kst doctor
```

This prints version, lock file path, installation paths, last sync time, and any failed skills.

## Using JSON output

All commands support `--json` for scripting and CI:

```console
$ kst sync --json
$ kst list --json
$ kst doctor --json
$ kst clean --json
```

## Next steps

See the [configuration reference](./configuration.md) for the full config schema, or browse the
[commands reference](./commands.md) for all available flags.
