# Security Model

**When you need this:** You want to know what Kasetto can modify on disk, how it handles credentials, and whether syncing from URLs is safe.

**What you'll learn:**

- What Kasetto touches (and what it avoids)
- How tokens are provided (env vars only)
- How remote config fetching is authenticated

## What Kasetto Changes On Disk

During `kst sync`, Kasetto may:

- **Install/update skills** by copying skill directories into the chosen destination path
- **Remove skills** that are no longer in your config (for the selected scope)
- **Merge MCP servers** into agent-native settings files (additive merge; never overwrite existing servers)
- **Write the lock file** for the selected scope

Kasetto is designed around a "tracked-only" principle:

- **Skills**: fully managed at their install paths for entries tracked in the lock
- **MCP servers**: only server entries that Kasetto installed (tracked in the lock) are removed during cleanup

See [How Sync Works](./how-sync-works.md) for details.

## What Kasetto Does Not Do

- It does not run skill code.
- It does not overwrite existing MCP server entries.
- It does not require (or write) a credentials file.

## Credentials And Tokens

Kasetto reads tokens from environment variables (per host).

Examples:

- GitHub / GitHub Enterprise: `GITHUB_TOKEN` or `GH_TOKEN`
- GitLab / self-hosted GitLab: `GITLAB_TOKEN` or `CI_JOB_TOKEN`
- Bitbucket Cloud: `BITBUCKET_EMAIL` + `BITBUCKET_TOKEN` (or app password variants)

See [Authentication](./authentication.md) for the full list and host detection rules.

## Remote Config Fetching (`--config https://...`)

When you pass a URL to `--config`, Kasetto:

- Fetches the YAML over HTTPS
- Applies the same host-based token selection rules as skill/MCP sources

This means a private config hosted on a git provider can be accessed by setting the appropriate token env var for that host.

## Practical Recommendations

- Prefer pinning remote sources to immutable refs (`ref: v1.2.3` or a commit SHA) for stable rollouts.
- In CI, use `--dry-run` (and ideally `--json`) to validate without writing changes.
