# First Steps with Kasetto

Once you've [installed Kasetto](./installation.md), run `kst` to make sure it's available:

```console
$ kst
An extremely fast AI skills manager

Usage: kst <COMMAND>

...
```

If there's no `kasetto.yaml` in the current directory, Kasetto greets you with an interactive home screen. Navigate with ++j++ / ++k++ or arrow keys, press ++enter++ to run the selected command, or use shortcut keys:

| Key                | Action                         |
| ------------------ | ------------------------------ |
| ++i++              | Init                           |
| ++s++              | Sync (prompts for config path) |
| ++l++              | List                           |
| ++d++              | Doctor                         |
| ++c++              | Clean                          |
| ++u++              | Self update                    |
| ++q++ / ++escape++ | Quit                           |

Set `NO_TUI=1` if you'd rather skip the interactive screen and get plain text hints.

## Creating a Config

Run `kst init` to generate a starter config:

```console
$ kst init
```

Or create a `kasetto.yaml` by hand:

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

## Syncing Skills

Run `kst sync` and Kasetto does the rest:

```console
$ kst sync
Syncing skills from 1 source...
  ✓ code-reviewer (installed)
  ✓ design-system (installed)
Synced 2 skills in 1.2s
```

Kasetto pulls the skills and installs them into the right agent directory. Next time you run `sync`, only what changed gets updated.

## Syncing from a Remote Config

Got a shared team config? Just pass it as a URL:

```console
$ kst sync --config https://example.com/team-skills.yaml
```

## Previewing Changes

Not ready to commit? Use `--dry-run` to see what would happen first:

```console
$ kst sync --dry-run
Would install: code-reviewer, design-system
Would remove: old-skill
```

## MCP Servers

Kasetto can also manage MCP server configs. Add an `mcps` section to your config:

```yaml
agent: claude-code

skills:
  - source: https://github.com/org/skill-pack
    skills: "*"

mcps:
  - source: https://github.com/org/mcp-pack
```

Kasetto merges them into each agent's native settings file during sync — nothing extra to do.

## Exploring What's Installed

Want to see what's installed? Open the browser:

```console
$ kst list
```

Navigate with ++j++ / ++k++, scroll with ++page-up++ / ++page-down++, jump with ++g++ ++g++ / ++shift+g++.
Set `NO_TUI=1` or pipe the output to get plain text instead.

Want to check your local setup:

```console
$ kst doctor
```

Doctor shows your version, lock file location, install paths, last sync time, and any skills that failed.

## Using JSON Output

Every command has a `--json` flag for scripting or CI:

```console
$ kst sync --json
$ kst list --json
$ kst doctor --json
$ kst clean --json
```

## Next Steps

See the [configuration reference](./configuration.md) for the full config schema, or browse the
[commands reference](./commands.md) for all available flags.
