# FAQ

**When you need this:** You have a quick "what happens if…" question about syncing skills or MCP servers.

## Will Kasetto Overwrite My MCP Entries?

No. MCP merges are **additive** and existing server entries are **never overwritten**. See [How Sync Works](./how-sync-works.md#mcp-servers-discovery-and-additive-merge).

## What Happens When Two Sources Define The Same MCP Server Name?

**First write wins** based on config order. Later sources with the same server name are skipped. See [How Sync Works](./how-sync-works.md#edge-cases).

## Where Is The Lock File?

- **Global scope**: `$XDG_DATA_HOME/kasetto/kasetto.lock` (default: `~/.local/share/kasetto/kasetto.lock`)
- **Project scope**: `./kasetto.lock`

See [How Sync Works](./how-sync-works.md#lock-file).

## How Do I Uninstall Safely?

- To remove tracked installs for a scope: `kst clean`
- To remove everything including the binary: `kst self uninstall`

See `kst clean` and `kst self uninstall` in [Commands](./commands.md).

## Can I Use Multiple Agents?

Yes. Set `agent` to a list. See [Configuration](./configuration.md#multiple-agents).

## Does Kasetto Run Code From Skills?

No. Skills are copied as directories. Execution is up to the agent you load them into.

## Can I Pin Sources To A Known-Good Version?

Yes. Use `ref:` with a tag or commit SHA. See [Cookbook](./cookbook.md#mcp-packs-pinning-and-rollouts).

## What Does `--dry-run` Do?

It prints what would change without writing any files. Useful in CI. See [CI & automation](./ci.md).

## Why Didn't My MCP Servers Show Up?

Most common causes:

- The target agent settings file is malformed JSON/TOML
- The MCP file doesn't contain a top-level `mcpServers` object

See [How Sync Works](./how-sync-works.md#edge-cases) (corrupted settings file behavior).

## How Do Remote Configs (`--config https://...`) Authenticate?

Kasetto selects tokens by the URL hostname using the same rules as skill/MCP sources. See [Authentication](./authentication.md#remote-configs).
