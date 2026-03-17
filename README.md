# kasetto

<p align="center">
  <img alt="Kasetto logo" src="assets/branding/logo.svg" width="520" />
</p>

<p align="center">
  <img alt="License" src="https://img.shields.io/github/license/pivoshenko/kasetto?style=flat-square&logo=opensourceinitiative&logoColor=white&color=0A6847">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-white?style=flat-square&logo=rust&logoColor=white&labelColor=0A6847&color=0A6847">
  <img alt="Release" src="https://img.shields.io/github/v/release/pivoshenko/kasetto?style=flat-square&logo=github&logoColor=white&color=4856CD&label=Release">
  <img alt="Support Ukraine" src="https://img.shields.io/badge/Support-Ukraine-FFC93C?style=flat-square&labelColor=07689F">
</p>

An extremely fast AI skills and workflow manager, written in Rust.

## Why

`kasetto` is built for high-speed, deterministic skill syncing:
- local + GitHub sources
- wildcard or explicit skill selection
- hash-based install/update detection
- stale skill cleanup
- dry-run support
- state + run reports

## Install

### From source

```bash
cargo build --release
./target/release/kasetto sync --config skills.config.yaml --dry-run
```

### Homebrew (tap)

```bash
brew tap pivoshenko/kasetto
brew install kasetto
```

## Usage

```bash
./kasetto sync --config skills.config.yaml
./kasetto sync --config skills.config.yaml --dry-run
./kasetto sync --config skills.config.yaml --json
```

## Session-start hooks (Claude/Cursor)

Install auto-sync hooks:

```bash
./kasetto install-hooks --config skills.config.yaml
```

This installs:
- `~/.kasetto/hooks/session-start.sh` (runner with lock + timeout + cache TTL)
- `~/.claude/hooks/session-start.sh`
- `~/.cursor/hooks/session-start.sh`

Defaults:
- timeout: 10s
- cache TTL: 300s

Override:

```bash
./kasetto install-hooks --config skills.config.yaml --timeout-seconds 15 --cache-ttl-seconds 120
```

## Config

```yaml
destination: ~/.openclaw/workspace/skills
skills:
  - source: ./skills
    skills: "*"

  - source: https://github.com/pivoshenko/pivoshenko.ai
    branch: main
    skills:
      - name: pivoshenko-brand-guidelines
      - name: skill-creator
```

## State and reports

- State: `~/.ai/bootstrap/state.json`
- Run reports: `~/.ai/bootstrap/runs/run-<timestamp>/report.json`

## CI/CD

- PR and main branch: fmt/clippy/test/build checks
- Tag push (`v*`): cross-platform binaries + checksums + GitHub Release

## MVP notes

Current MVP focuses on `sync` behavior parity and performance-oriented implementation.
Additional commands (`list`, `doctor`, `prune`, `tui`) are planned.
