# Contributing to Kasetto

Thanks for contributing.

## Development setup

```bash
rustc --version
cargo --version

cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

## Project structure

- `src/main.rs`: CLI entrypoint + core MVP logic
- `Formula/kasetto.rb`: Homebrew formula
- `site/`: project landing page

## Commit style

Use conventional-ish prefixes:
- `feat:` new functionality
- `fix:` bug fixes
- `refactor:` behavior-preserving code changes
- `build:` tooling/dependency updates
- `docs:` documentation only
- `ci:` workflow changes

## Pull requests

Before opening a PR:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Include:
- what changed
- why
- any breaking behavior

## Release

Releases are tag-driven via GitHub Actions.
Tag format: `vX.Y.Z`
