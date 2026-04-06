# Installing Kasetto

## Installation Methods

Pick whichever method works for you — standalone installer, package manager, or straight from source.

### Standalone Installer

The quickest way to get started — downloads and installs the binary in one command:

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

The binary lands in `~/.local/bin` by default. Override it with environment variables:

| Variable              | Description            | Default                                                      |
| --------------------- | ---------------------- | ------------------------------------------------------------ |
| `KASETTO_VERSION`     | Version tag to install | Latest release                                               |
| `KASETTO_INSTALL_DIR` | Installation directory | `~/.local/bin` (Unix) / `%USERPROFILE%\.local\bin` (Windows) |

### Homebrew

Available via a Homebrew tap:

```console
$ brew install pivoshenko/tap/kasetto
```

### Scoop

Available via a Scoop bucket on Windows:

```console
$ scoop bucket add kasetto https://github.com/pivoshenko/scoop-bucket
$ scoop install kasetto
```

### Cargo

Available on [crates.io](https://crates.io):

```console
$ cargo install kasetto
```

!!! note

    This builds from source, so you'll need a compatible Rust toolchain.

### GitHub Releases

Prefer to grab a binary directly? Head to [GitHub Releases](https://github.com/pivoshenko/kasetto/releases) — every release includes binaries for all supported platforms.

### From Source

Clone and install with Cargo:

```console
$ git clone https://github.com/pivoshenko/kasetto && cd kasetto
$ cargo install --path .
```

## Upgrading

If you used the standalone installer, updating is a one-liner:

```console
$ kst self update
```

For Homebrew or Cargo installs, use the package manager's own upgrade command. For example, with Cargo:

```console
$ cargo install kasetto
```

## Shell Autocompletion

!!! tip

    You can run `echo $SHELL` to help determine your shell.

To get tab completions for `kst`, add one of these to your shell config:

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

Then restart your shell or source the config file.

## Next Steps

Check out the [quick start](./getting-started.md), or jump straight to the [configuration reference](./configuration.md) if you already know what you want.
