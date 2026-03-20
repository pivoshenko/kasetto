# Changelog

All notable changes to this project will be documented in this file.

## [1.2.1] - 2026-03-20

### Documentation

- Add usage descriptions for each commit prefix in contribution guidelines

### Perf

- Use mimalloc, fat LTO, streaming I/O, and SQLite tuning

## [1.2.0] - 2026-03-20

### Features

- Add Windows builds and Scoop distribution

### Release

- V1.2.0

## [1.1.0] - 2026-03-20

### Bug fixes

- **ci**: Add git-cliff binary to PATH after installation
- **ci**: Install git-cliff binary for release workflow
- **install**: Match archive naming to release artifacts
- Resolve clippy collapsible-if warnings

### CI/CD

- **labels**: Add workflow_dispatch trigger

### Documentation

- Add Vercel deployment config and rename Manifest-backed to Traceable
- Add mkdocs-material documentation site
- Cleanup

### Features

- Add shell completions and auto version bumping

### Release

- V1.1.0

### Style

- **ci**: Capitalize step names in workflows

## [1.0.0] - 2026-03-19

### Bug fixes

- **ci**: Tag even when version already matches Cargo.toml
- **branding**: Center ASCII logo within border
- Correct Japanese branding to スキル

### CI/CD

- Combine tag and release into single workflow, fix formatting
- Add tag workflow for version releases
- Fix release workflow and enable CI on push
- **release**: Auto-generate changelog with git-cliff
- Disable automatic workflow triggers

### Design

- Update logo subtitle to KASETTO
- Update logo subtitle to SUKIRU
- Enlarge Japanese logo accent and tighten edge alignment
- Center branding and add colorful logo variants
- Add Japanese-first logo pack and wire branding

### Documentation

- **readme**: Fix license badge and clarify Vercel Skills comparison
- **readme**: Humanize copy and reorder sections
- Align rust badge styling with license badge
- Make rust stable badge green
- Reorder badges in requested order
- Align short description with Rust-first positioning
- Add flat-style badges to README

### Features

- Add self-update command, install scripts, icon, and rewrite README
- **branding**: Replace logo
- **branding**: Replace logo with ascii banner mark
- **cli**: Add interactive startup flow
- **cli**: Revamp sync/list ui and add doctor diagnostics
- **cli**: Add kst alias and themed banner
- Add premium animated sync UX with plain and verbose modes
- Support remote config URLs for sync and hooks
- Add sukiro startup banner with Japanese label
- Add session-start hook installer for Claude and Cursor
- Bootstrap skills-manager MVP with sync engine, standards files, and landing page

### Miscellaneous

- Update version to 1.0.0 and improve spinner feedback
- Update chore files
- **deps**: Bump actions/checkout from 4 to 6
- Remove Go migration leftovers and keep Rust-only docs
- Productionize repo with CI/CD, Homebrew formula, and governance docs

### Refactor

- **commands**: Split sync list doctor into modules
- Rename project from sukiru to kasetto
- Rename project, binary, and branding from sukiro to sukiru
- Migrate sukiro core from Go to Rust
- Rename project and CLI to sukiro

### Testing

- **core**: Expand coverage for fsops profile and ui

