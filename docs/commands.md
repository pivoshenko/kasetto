# Commands

## `kst init`

Generates a starter `kasetto.yaml` in the current directory.

```console
$ kst init [OPTIONS]
```

### Options

| Flag | Description |
| --- | --- |
| `--force` | Overwrite an existing `kasetto.yaml` without prompting |

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
| `--project` | Install into the current project directory |
| `--global` | Install globally (default) |

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
| `--project` | List project-scoped assets |
| `--global` | List globally-scoped assets (default) |

In a terminal it opens an interactive browser — navigate with ++j++ / ++k++, scroll with
++page-up++ / ++page-down++, jump with ++g++ ++g++ / ++shift+g++.

!!! note

    Set `NO_TUI=1` or pipe the output to get plain text instead of the interactive browser.

## `kst doctor`

Prints local diagnostics: version, lock file path, installation paths, last sync time, and any
failed skills from the latest run.

```console
$ kst doctor [OPTIONS]
```

### Options

| Flag | Description |
| --- | --- |
| `--json` | Output as JSON |
| `--project` | Show project-scoped diagnostics |
| `--global` | Show globally-scoped diagnostics (default) |

## `kst clean`

Removes all tracked skills and MCP configs for the given scope.

```console
$ kst clean [OPTIONS]
```

### Options

| Flag | Description |
| --- | --- |
| `--dry-run` | Preview what would be removed |
| `--json` | Print output as JSON |
| `--project` | Clean project-scoped assets |
| `--global` | Clean globally-scoped assets (default) |

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

## `kst completions`

Generates shell completion scripts.

```console
$ kst completions <SHELL>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`.

!!! tip

    Example for Fish: `kst completions fish > ~/.config/fish/completions/kst.fish`
