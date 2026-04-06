# Installing Kasetto

## Installation methods

Install Kasetto with our standalone installers or your package manager of choice.

### Standalone installer

Kasetto provides a standalone installer to download and install the binary:

=== "macOS and Linux"

    Use `curl` to download the script and execute it with `sh`:

    ```console
    $ curl -fsSL https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.sh | sh
    ```

=== "Windows"

    Use `irm` to download the script and execute it with `iex`:

    ```pwsh-session
    PS> powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.ps1 | iex"
    ```

    Changing the [execution policy](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies) allows running a script from the internet.

!!! tip

    The installation script may be inspected before use:

    === "macOS and Linux"

        ```console
        $ curl -fsSL https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.sh | less
        ```

    === "Windows"

        ```pwsh-session
        PS> powershell -c "irm https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.ps1 | more"
        ```

    Alternatively, binaries can be downloaded directly from [GitHub Releases](#github-releases).

By default, the binary is placed in `~/.local/bin`. The following environment variables can customize the installation:

| Variable              | Description            | Default                                                      |
| --------------------- | ---------------------- | ------------------------------------------------------------ |
| `KASETTO_VERSION`     | Version tag to install | Latest release                                               |
| `KASETTO_INSTALL_DIR` | Installation directory | `~/.local/bin` (Unix) / `%USERPROFILE%\.local\bin` (Windows) |

### Homebrew

Kasetto is available via a Homebrew tap.

```console
$ brew install pivoshenko/tap/kasetto
```

### Scoop

Kasetto is available via a Scoop bucket (Windows).

```console
$ scoop bucket add kasetto https://github.com/pivoshenko/scoop-bucket
$ scoop install kasetto
```

### Cargo

Kasetto is available via [crates.io](https://crates.io).

```console
$ cargo install kasetto
```

!!! note

    This method builds Kasetto from source, which requires a compatible Rust toolchain.

### GitHub Releases

Kasetto release artifacts can be downloaded directly from
[GitHub Releases](https://github.com/pivoshenko/kasetto/releases).

Each release page includes binaries for all supported platforms.

### From source

Clone the repository and install with Cargo:

```console
$ git clone https://github.com/pivoshenko/kasetto && cd kasetto
$ cargo install --path .
```

## Upgrading Kasetto

When Kasetto is installed via the standalone installer, it can update itself on-demand:

```console
$ kst self update
```

When another installation method is used, use the package manager's upgrade method instead.
For example, with Cargo:

```console
$ cargo install kasetto
```

## Shell autocompletion

!!! tip

    You can run `echo $SHELL` to help determine your shell.

To enable shell autocompletion for Kasetto commands, run one of the following:

=== "Bash"

    ```bash
    echo 'eval "$(kst completions bash)"' >> ~/.bashrc
    ```

=== "Zsh"

    ```bash
    echo 'eval "$(kst completions zsh)"' >> ~/.zshrc
    ```

=== "fish"

    ```bash
    echo 'kst completions fish | source' > ~/.config/fish/completions/kst.fish
    ```

=== "PowerShell"

    ```powershell
    if (!(Test-Path -Path $PROFILE)) {
      New-Item -ItemType File -Path $PROFILE -Force
    }
    Add-Content -Path $PROFILE -Value '(& kst completions powershell) | Out-String | Invoke-Expression'
    ```

Then restart the shell or source the shell config file.

## Next steps

See the [quick start](./getting-started.md) or jump straight to the [configuration](./configuration.md)
reference to start using Kasetto.
