# How Sync Works

This page explains what `kst sync` does under the hood - how skills are installed, how MCP
configs are merged, what gets overwritten, and what stays safe.

## Sync Flow

```
1. Load config (kasetto.yaml or --config URL)
2. Load lock file (kasetto.lock)
3. Sync skills
   a. For each skill source: materialize (download if remote)
   b. Discover skills in source directory
   c. For each skill: hash → compare to lock → copy if changed
   d. Remove skills no longer in config
4. Sync MCP servers
   a. For each MCP source: materialize (download if remote)
   b. Discover MCP pack files in source
   c. For each pack: hash → compare to lock → merge into agent settings if changed
   d. Remove MCP entries no longer in config
5. Save lock file and report (unless --dry-run)
```

## Skills: Copy and Replace

Skills are plain directories containing a `SKILL.md` file (see
[writing skills](./writing-skills.md)). On each sync, Kasetto:

- **Discovers** skills by scanning the source root and `skills/` subdirectory for directories
  that contain a `SKILL.md` file.
- **Hashes** the source skill directory.
- **Compares** the hash to the lock file. If unchanged, the skill is skipped.
- **Copies** the entire directory to the destination, replacing any previous version.
- **Removes** skill directories that are no longer listed in the config.

Skills are fully replaced on update - there is no partial merge.

## MCP Servers: Discovery and Additive Merge

Kasetto discovers MCP pack files in the source automatically:

1. `.mcp.json` at the source root
2. `mcp.json` at the source root
3. Any `.json` file inside a `mcp/` subdirectory

Each file must contain a `mcpServers` JSON object. The `path` config field bypasses this discovery
and points directly at a specific file.

Server entries are then merged into each agent's native settings file (e.g., `.claude.json`,
`.cursor/mcp.json`). The merge follows two rules:

1. **New entries are added.** If the settings file doesn't have a server with that name, it's
   inserted.
2. **Existing entries are never overwritten.** If a server name already exists - whether added by
   Kasetto or by hand - Kasetto leaves it untouched.

This means:

- **Manual edits are safe.** If you add API keys, environment variables, or tweak server settings
  after a sync, those changes survive future syncs.
- **Re-sync is idempotent.** Running `sync` twice with the same config produces the same result.
- **First write wins.** If two sources define a server with the same name, only the first one is
  installed.

### Supported Config Formats

Kasetto auto-detects and writes to four formats depending on the agent:

| Format               | Used by                                          | Target file example                |
| -------------------- | ------------------------------------------------ | ---------------------------------- |
| McpServers JSON      | Claude Code, Cursor, Gemini CLI, Roo, and others | `.claude.json`, `.cursor/mcp.json` |
| VS Code servers JSON | GitHub Copilot                                   | `.vscode/mcp.json`                 |
| OpenCode JSON        | OpenCode                                         | `.config/opencode/opencode.json`   |
| Codex TOML           | Codex                                            | `.codex/config.toml`               |

All formats follow the same additive-merge rules.

## Change Detection

Kasetto uses SHA-256 hashes to detect changes:

- **Skills:** The entire skill directory is hashed. If the hash matches the lock file, the skill is
  skipped.
- **MCP packs:** The pack file is hashed. If the hash matches **and** all server names are still
  present in the target settings, the pack is skipped.

This makes re-sync fast - unchanged sources require no file I/O beyond reading the lock.

## Lock File

Kasetto tracks what it installed in a YAML lock file called `kasetto.lock`. The location depends
on the scope:

| Scope   | Location                                                                               |
| ------- | -------------------------------------------------------------------------------------- |
| Global  | `$XDG_DATA_HOME/kasetto/kasetto.lock` (default: `~/.local/share/kasetto/kasetto.lock`) |
| Project | `./kasetto.lock` in the project root                                                   |

The lock file is how Kasetto knows what to remove when a source is deleted from the config or when
you run `kst clean`. You generally don't need to edit it by hand.

**Skills** are tracked by a composite key (`source::skill-name`) with their destination path and
hash.

**MCP servers** are tracked as assets with the server names they installed (e.g.,
`destination: "git-tools,airflow"`). Only these tracked names are removed during cleanup.

!!! important

    Kasetto never touches entries it didn't install. Manually added servers, skills from other
    tools, or entries from a different scope are always preserved.

## Removal Behavior

### Removing a Source from Config

When you remove a skill or MCP source from `kasetto.yaml` and re-sync:

- **Skills:** The skill directory is deleted from disk.
- **MCPs:** The specific server entries that Kasetto installed are removed from the agent's settings
  file. The file itself is preserved.

### `kst clean`

Removes everything tracked in the lock file for the given scope:

- Deletes all tracked skill directories.
- Removes all tracked MCP server entries from every agent's settings file.
- Clears the lock file.

User-added entries outside the lock are never affected.

## Edge Cases

**Conflicting server names across sources.** If source A and source B both define a server named
`"my-server"`, source A wins (processed first). If you later remove source A, `"my-server"` is
removed - even though source B also wanted it. Re-sync will then install source B's version.

**Renamed servers in upstream.** If an upstream MCP pack renames a server from `"old"` to `"new"`,
Kasetto sees this as: remove `"old"` (no longer in pack) + add `"new"` (not yet in settings). The
old entry is cleaned up, and the new one is added.

**Corrupted settings file.** If an agent's settings file is malformed JSON/TOML, the merge for that
file fails and is reported as an error. The lock file is not updated for that pack, so the next sync
retries the merge.
