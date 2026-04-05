# Commands

## `kst sync`

Reads the config, discovers skills, and makes the local destination match.

```console
$ kst sync [OPTIONS]
```

### Options

| Flag | Description |
| --- | --- |
| `--config <path-or-url>` | Path or HTTPS URL to a YAML config (default: `kasetto.yaml`) |
| `--dry-run` | Preview what would change without writing anything |
| `--quiet` | Suppress non-error output |
| `--json` | Print the sync report as JSON |
| `--plain` | Disable colors and spinner animations |
| `--verbose` | Show per-skill action details |

Missing skills are reported as broken but don't stop the run. The exit code is non-zero only for
source-level failures.

!!! tip

    Use `--dry-run` in CI to verify configs without making changes.

## `kst list`

Shows everything currently tracked in the manifest.

```console
$ kst list [OPTIONS]
```

### Options

| Flag | Description |
| --- | --- |
| `--json` | Output as JSON instead of the interactive browser |

In a terminal it opens an interactive browser — navigate with ++j++ / ++k++, scroll with
++page-up++ / ++page-down++, jump with ++g++ ++g++ / ++shift+g++.

!!! note

    Set `NO_TUI=1` or pipe the output to get plain text instead of the interactive browser.

## `kst doctor`

Prints local diagnostics: version, manifest DB path, installation paths, last sync time, and any
failed skills from the latest run.

```console
$ kst doctor [OPTIONS]
```

### Options

| Flag | Description |
| --- | --- |
| `--json` | Output as JSON |

## `kst self`

Manage the running Kasetto installation (update or uninstall).

### `kst self update`

Checks GitHub for the latest release and replaces the current binary in-place.

```console
$ kst self update [OPTIONS]
```

#### Options

| Flag | Description |
| --- | --- |
| `--json` | Output as JSON |

!!! note

    Self-update is only available when Kasetto was installed via the standalone installer.
    When installed via Homebrew or Cargo, use their respective upgrade commands.

### `kst self uninstall`

Removes installed skills and MCP configs, deletes Kasetto config and data directories, and removes the binary.

```console
$ kst self uninstall [OPTIONS]
```

#### Options

| Flag | Description |
| --- | --- |
| `--yes` | Skip the confirmation prompt (required in non-interactive use) |
