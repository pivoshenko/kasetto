# Commands

## `kst init`

Generates a starter config file — local `./kasetto.yaml` by default, or global config with `--global`.

```bash
kst init [OPTIONS]
```

### Options

| Flag       | Description                                                                       |
| ---------- | --------------------------------------------------------------------------------- |
| `--global` | Write `$XDG_CONFIG_HOME/kasetto/kasetto.yaml` (or `~/.config/kasetto/kasetto.yaml`) |
| `--force`  | Overwrite an existing config file without prompting                               |

## `kst sync`

Reads your config, fetches any remote skills, and brings your local install up to date.

```bash
kst sync [OPTIONS]
```

### Options

| Flag                     | Description                                                  |
| ------------------------ | ------------------------------------------------------------ |
| `--config <path-or-url>` | Path or HTTPS URL to a YAML config (default order: `$KASETTO_CONFIG`, `source:` in `config.yaml`, `./kasetto.yaml`, `$XDG_CONFIG_HOME/kasetto/kasetto.yaml`) |
| `--dry-run`              | Preview what would change without writing anything           |
| `--quiet`                | Suppress non-error output                                    |
| `--json`                 | Print the sync report as JSON                                |
| `--plain`                | Disable colors and spinner animations                        |
| `--verbose`              | Show per-skill action details                                |
| `--yes`                  | Skip confirmation prompt for new MCP servers                 |
| `--project`              | Install into the current project directory                   |
| `--global`               | Install globally (default)                                   |

Missing skills are reported as broken but won't stop the rest of the run. The exit code is non-zero only for source-level failures.

!!! tip

    `--dry-run` is great in CI — verify your config without touching anything on disk.

## `kst list`

Shows everything Kasetto has installed (skills and MCP servers from the lock file).

```bash
kst list [OPTIONS]
```

### Options

| Flag        | Description                                                              |
| ----------- | ------------------------------------------------------------------------ |
| `--json`    | Output as JSON instead of the interactive browser                        |
| `--quiet`   | Do not print anything (unless `--json` is set)                           |
| `--plain`   | Disable colors; use plain text output instead of the full-screen browser |
| `--project` | Only read the project lock (`./kasetto.lock`) in the current directory   |
| `--global`  | Only read the global lock (under XDG data)                               |

With **no** `--project` or `--global`, Kasetto merges **both** scopes so you can see global and project installs together. Each skill row includes a **scope** field (in JSON and in the browser detail pane). JSON also includes `"merged_scopes": true` in that mode; MCP entries are objects with `name`, `scope`, `pack_file`, and `source`.

In a terminal (and without `--plain`), this opens an interactive browser — **Skills** and **MCPs** tabs, each with a detail pane. Navigate with ++j++ / ++k++, scroll with
++page-up++ / ++page-down++, jump with ++g++ ++g++ / ++shift+g++. Switch tabs with ++tab++ or ++h++ / ++l++.

!!! note

    Set `NO_TUI=1` or pipe stdout to force a non-browser text listing. For a **local** plain listing without disabling all TUIs, use `--plain`.

## `kst doctor`

Prints a local health check: your version, lock file location, install paths, last sync time, and any skills that failed.

```bash
kst doctor [OPTIONS]
```

### Options

| Flag        | Description                                |
| ----------- | ------------------------------------------ |
| `--json`    | Output as JSON                             |
| `--quiet`   | Do not print anything (unless `--json`)    |
| `--plain`   | Disable colors and the banner-style header |
| `--project` | Show project-scoped diagnostics            |
| `--global`  | Show globally-scoped diagnostics (default) |

## `kst clean`

Removes everything Kasetto installed for the given scope — skills, MCP configs, and lock file entries.

```bash
kst clean [OPTIONS]
```

### Options

| Flag        | Description                                                     |
| ----------- | --------------------------------------------------------------- |
| `--dry-run` | Preview what would be removed (lists skill paths and MCP packs) |
| `--json`    | Print output as JSON                                            |
| `--quiet`   | Suppress non-error output                                       |
| `--plain`   | Disable colors and banner-style header                          |
| `--project` | Clean project-scoped assets                                     |
| `--global`  | Clean globally-scoped assets (default)                          |

## `kst self`

Manage Kasetto itself — update to a new version or remove it completely.

### `kst self update`

Fetches the latest release from GitHub, verifies the SHA256 checksum against `checksums.txt` from the same release, and swaps out the binary in-place.

```bash
kst self update [OPTIONS]
```

#### Options

| Flag     | Description    |
| -------- | -------------- |
| `--json` | Output as JSON |

!!! note

    Self-update only works when Kasetto was installed via the standalone installer.
    For Homebrew or Cargo installs, use their own upgrade commands.

### `kst self uninstall`

A full teardown: removes installed skills and MCP configs, clears Kasetto's data directories, and deletes the binary.

```bash
kst self uninstall [OPTIONS]
```

#### Options

| Flag    | Description                                                    |
| ------- | -------------------------------------------------------------- |
| `--yes` | Skip the confirmation prompt (required in non-interactive use) |

## `kst completions`

Generates completion scripts for your shell.

```bash
kst completions <SHELL>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`.

!!! tip

    Example for Fish: `kst completions fish > ~/.config/fish/completions/kst.fish`
