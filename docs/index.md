# First Steps with Kasetto

**When you need this:** You want a quick, predictable path to "working sync" and you're not sure what Kasetto will modify.

**What you'll learn:**

- How to create a config and run your first sync
- How to preview changes and avoid interactive UI
- Where to go next for the exact sync/merge guarantees

Once you've [installed Kasetto](./installation.md), run `kst` to make sure it's available:

```bash
kst
A declarative AI agent environment manager

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

```bash
kst init
```

Use `kst init --global` to create `$XDG_CONFIG_HOME/kasetto/kasetto.yaml` (or `~/.config/kasetto/kasetto.yaml`).

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

```bash
kst sync
Syncing skills from 1 source...
  ✓ code-reviewer (installed)
  ✓ design-system (installed)
Synced 2 skills in 1.2s
```

Kasetto pulls the skills and installs them into the right agent directory. Next time you run `sync`, only what changed gets updated.

If you want the "exact contract" for what gets copied/removed, read [How Sync Works](./how-sync-works.md).

## Syncing from a Remote Config

Got a shared team config? Just pass it as a URL:

```bash
kst sync --config https://example.com/team-skills.yaml
```

For private configs hosted on git providers, set the right env var token first (see [Authentication](./authentication.md#remote-configs)).

## Previewing Changes

Not ready to commit? Use `--dry-run` to see what would happen first:

```bash
kst sync --dry-run
Would install: code-reviewer, design-system
Would remove: old-skill
```

See [CI & automation](./ci.md) for recommended CI patterns.

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

If you want the exact merge rules and conflict behavior, see [How Sync Works](./how-sync-works.md#mcp-servers-discovery-and-additive-merge).

## Exploring What's Installed

Want to see what's installed? Open the browser:

```bash
kst list
```

Navigate with ++j++ / ++k++, switch tabs with ++tab++ or ++h++ / ++l++, scroll with ++page-up++ / ++page-down++, jump with ++g++ ++g++ / ++shift+g++.
With no `--project` or `--global`, **global and project** installs are shown together (each row is labeled by scope). Use `--plain`, set `NO_TUI=1`, or pipe stdout for plain text instead of the full-screen browser.

Want to check your local setup:

```bash
kst doctor
```

Doctor shows your version, lock file location, install paths, last sync time, and any skills that failed.

## Using JSON Output

`sync`, `list`, `doctor`, `clean`, and `self update` support `--json` for scripting or CI:

```bash
kst sync --json
kst list --json
kst doctor --json
kst clean --json
```

## Next Steps

See the [configuration reference](./configuration.md) for the full config schema, browse the
[commands reference](./commands.md), or grab a ready-to-use pattern from the [cookbook](./cookbook.md).
