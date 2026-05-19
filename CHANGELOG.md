# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Sync user-defined slash commands across supported agents

## [2.11.0] - 2026-05-19

### Features

- **tui**: Tint banner subtitle amber and drop star animation

### Perf

- **tui**: Redraw home and list browser on events only

## [2.10.0] - 2026-05-18

### Bug fixes

- **tui**: Drop amber tint from banner subtitle

### Documentation

- Capitalize Skills in tagline
- Streamline README install and getting started
- Simplify README for landing-page shape

### Features

- **site**: Align OG image with brand title and description
- **site**: Refine link preview metadata and OG image typography
- **site**: Add personal blog link to footer socials
- **site**: Show favicon mark in top nav

### Refactor

- **list**: Inline tab counts and drop separate header row
- **site**: Reorder footer socials and use globe icon

### Release

- V2.10.0

### Style

- **site**: Polish demo snippet, footer, and docs background

## [2.9.1] - 2026-05-12

### Bug fixes

- **tui**: Paint banner Japanese subtitle in amber Side B

### Documentation

- Remove demo tapes
- **demo**: Add hero demo tape and screen-recording script
- Link readme logo to kasetto.dev

### Miscellaneous

- **tests**: Silence field-reassign and useless-vec clippy lints
- **site**: Bump biome schema to 2.4.14
- **brand**: Recolor logo Japanese subtitle to amber
- Update name of demo scripts
- **brand**: Align visual identity with cassette DNA
- **gitignore**: Ignore macos .ds_store files and local .claude/ config

### Refactor

- **list**: Extract draw_tab_content to dedupe tab arms
- **list**: Split run into load and print helpers
- **clean**: Split run into removal and report helpers
- **tui**: Drop unused crossterm SUCCESS and WARNING consts
- **tui**: Migrate palette to brand RGB and add Side B accent

### Release

- V2.9.1

### Style

- **site**: Swap Side B accent from rust to amber

## [2.9.0] - 2026-05-10

### Features

- **cli**: Notify when a new version is released

### Release

- V2.9.0

## [2.8.1] - 2026-05-10

### Bug fixes

- Update keywords
- **site**: Allow site directory in vercel deployment

### Documentation

- Document the site subproject in claude.md

### Features

- **site**: Show repo name and star count in topnav
- **site**: Scroll cue, hero rebalance, unify warm accent to rust
- **site**: Proper multi-column footer, polished navbar, wider hero demo
- **site**: Typewriter command and per-row spinner in hero demo
- **site**: Wire docs search, a11y polish, animated kst demo
- **site**: Polish homepage hero and docs styling
- **site**: Merge landing and docs into unified next.js site

### Miscellaneous

- Remove stale example
- **site**: Disable vercel auto-deploy on push to main
- **ci**: Rename vercel project secret variable
- **docs**: Remove mkdocs and finish migration to fumadocs

### Release

- V2.8.1

## [2.7.0] - 2026-05-09

### Documentation

- Expand example config with real-world skill packs
- Remove roadmap section

### Features

- **config**: Add extends for config inheritance

### Release

- V2.7.0

## [2.6.1] - 2026-05-07

### Bug fixes

- **sync**: Resolve skill path relative to source root
- **docs**: Fix uv venv build command for Vercel
- **docs**: Pin Python 3.12 for Vercel build

### Documentation

- Add real-world config example link

### Release

- V2.6.1

## [2.6.0] - 2026-05-04

### Bug fixes

- **landing**: Shorten etymology line
- **landing**: Mention mcp sources in etymology line
- **landing**: Version badge inherits text color
- **landing**: Version badge same amber as star count
- **landing**: Use accent purple for version badge
- **landing**: Vertically align version and star count in action label
- **landing**: Show version after stars, match label font size
- **landing**: Softer warm off-white light mode palette

### Documentation

- Remove --yes and MCP confirmation gate references
- **readme**: Reformat tables; update etymology wording
- Mention MCP sources in name etymology

### Features

- **sync**: Remove MCP confirmation prompt
- **sync**: Add --force flag and surface skipped MCPs
- **landing**: Add name etymology under tagline
- **landing**: Show latest release version next to GitHub row

### Miscellaneous

- **justfile**: Add update-next, update-docs, split update into sub-recipes

### Refactor

- **sync**: Remove --force flag

### Build

- Update dev dependencies

### Release

- V2.6.0

### Style

- **landing**: Apply formatter line wrap

## [2.5.1] - 2026-05-04

### Bug fixes

- **landing**: Add type=button; fix biome suppression for theme script
- **landing**: Prevent theme icon flash on light-mode; trim trailing blank line
- **mcps**: Stale mcp/ hint, extension check, init template, mcp/-rename warning

### CI/CD

- Run landing deploy from repo root; rootDirectory handled by Vercel project settings
- Add manual landing deploy workflow
- Simplify docs deploy — let Vercel build from docs/ via vercel.json
- Use venv instead of --system to avoid permission denied on runner
- Add --break-system-packages for uv on ubuntu-latest
- Update job names
- Update job names
- Move VERCEL_TOKEN to env block to prevent argv leak (bug_007)

### Documentation

- Add missing mcps: field to Quick Start MCP example (bug_017)

### Release

- V2.5.1

## [2.5.0] - 2026-05-03

### Bug fixes

- **mcps**: Report broken entries individually; reject non-'*' wildcards
- **tests**: Serialize env-var-touching tests with a mutex to prevent races
- **tests**: Fix without-token test assertions and add env var cleanup
- **github**: Url-encode git refs in API tarball endpoint
- **tests**: Update github archive URL assertions to match API endpoint
- **github**: Use API endpoint for private repo archive downloads
- **github**: Url-encode git refs in API tarball endpoint
- **github**: Use API endpoint for private repo archive downloads
- **config**: Clean up rebase conflict resolution

### CI/CD

- Add manual docs deploy workflow; disable Vercel auto-deploy

### Documentation

- **contributing**: Add branch naming and expand commit message standards
- Add Mermaid sync-flow diagram page; fix MkDocs setup
- Update all prose docs for new mcps[] schema

### Features

- **landing**: Add dark/light theme toggle; refresh code block and grid bg
- **cli**: Add config source management commands (#26)
- **search**: Add SkillsMP marketplace search command
- **config**: Support remote preset configs
- **config**: Support reusable skill presets

### Refactor

- **mcps**: Align mcps[] schema with skills[] — drop path: field

### Release

- V2.5.0

### Revert

- Roll back to 1393a55 (before config presets and search features)

### Style

- Run formatters

## [2.4.0] - 2026-04-26

### Documentation

- **config**: Document browser URL auto-rewriting for --config

### Features

- **config**: Rewrite browser URLs to raw content for --config

### Release

- V2.4.0

## [2.3.0] - 2026-04-23

### Bug fixes

- **sync**: Harden skill discovery
- **sync**: Follow symlinked directories in skill copy
- **sync**: Discover root-level skill packs

### Documentation

- **sync**: Document root skill and sub-dir discovery
- **roadmap**: Add library crate extraction
- Soften dark theme text colors to match landing page palette
- Use full banner logo in header and link to landing page
- Replace landing page with quick start as docs index
- **roadmap**: Add audit command and smart URL rewriting

### Features

- **sync**: Support skills source sub-dir selection
- **security**: Add self-update checksum verification and MCP approval gate

### Release

- V2.3.0
- V2.2.0

## [2.1.0] - 2026-04-19

### Bug fixes

- **lint**: Use sort_by_key instead of sort_by for clippy 1.95 compatibility

### CI/CD

- Move site_dir outside docs_dir to fix mkdocs nested build error
- Set docs_dir to current directory in mkdocs.yml
- Correct custom_dir path in mkdocs.yml
- Use uv pip install --system for mkdocs build on Vercel
- Scope lint and build steps to Rust-only recipes
- Extend list of commands
- Add .gitignore

### Documentation

- Update startup routing diagram after root-level sync flag removal
- Update Vercel config
- Unify install URLs and fix doc alignment
- Extend "Why Kasetto" section
- Add landing page

### Features

- **config**: Add KASETTO_CONFIG env var, preferences file, and rename global config
- **init**: Add --global config scaffold option
- **config**: Add global default config fallback

### Refactor

- **cli**: Remove root-level sync flags, always show TUI on bare kst
- Run linters

### Testing

- **config**: Add unit tests for config resolution priority logic

### Release

- V2.1.0

### Style

- Run formatters

## [2.0.1] - 2026-04-06

### Miscellaneous

- Rebrand description and bump version to 2.0.1

### Release

- V2.0.1

## [2.0.0] - 2026-04-06

### Documentation

- Update sections
- Update notes
- Update descriptions
- Add icons into cards
- Add authentication, skill format, multi-agent, and git host docs
- Update documentation to match current codebase

### Features

- Add init, clean, uninstall, completions commands and MCP support

### Miscellaneous

- **deps**: Bump unicode-width from 0.1.14 to 0.2.2
- **deps**: Bump actions/download-artifact from 4 to 8
- **deps**: Bump actions/upload-artifact from 4 to 7

### Refactor

- Run linters
- Update flags
- Align Home and List TUIs
- Rename config example

### Build

- Update dev dependencies
- Update dependencies

### Release

- V2.0.0

### Style

- Run formatters
- Run formatters

## [1.2.1] - 2026-03-20

### Documentation

- Add usage descriptions for each commit prefix in contribution guidelines

### Perf

- Use mimalloc, fat LTO, streaming I/O, and SQLite tuning

### Release

- V1.2.1

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

