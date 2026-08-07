# Changelog

All notable changes to this project will be documented in this file.

## [3.6.2] - 2026-08-04

### Bug fixes

- **agents**: Point the codex preset at .agents/skills
- **cli**: Count every asset kind in lock and uninstall summaries
- **sync**: Name every failed asset, not just skills
- **cli**: Correct stale claims in output and module docs
- **cli**: Describe all four asset kinds in help and diagnostics
- **cli**: Keep the arrow glyph in output, docs, and comments
- **cli**: Normalize terminal output and help text to plain ASCII

### Documentation

- Use plain ASCII punctuation in README, docs, and site copy

## [3.6.1] - 2026-08-04

### Bug fixes

- **cli**: Align short about line with the actual scope
- **site**: Add titles to svg assets for accessibility
- **site**: Derive agents-grid preset count from list length

### Miscellaneous

- Add editorconfig

### Build

- **deps**: Override sharp to >=0.35.0 for libvips advisory
- **deps**: Update dependencies
- **deps**: Update dependencies

### Release

- V3.6.1

## [3.6.0] - 2026-07-17

### Features

- Add zcode agent preset

### Build

- Bump crossbeam-epoch to 0.9.20 for RUSTSEC-2026-0204

### Release

- V3.6.0

## [3.5.0] - 2026-06-28

### Bug fixes

- **deps**: Bump quinn-proto to 0.11.15 for RUSTSEC-2026-0185
- **lock**: Record every skill destination in the lock rebuild
- **sync**: Track every skill destination to stop multi-agent data loss
- **secrets**: Scope --update rotation per file, not per source

### Documentation

- **readme**: List secret managers and broaden positioning
- **lock**: Note the comma-in-path limitation of the destination CSV
- **secrets**: Drop stale provider count in tagged-form intro
- **site**: Tighten feature-tab copy
- **secrets**: Document #json-key for gcp and az
- **secrets**: Document aws, gcp, az, pass, and keychain sources
- **secrets**: Flag that rotation requires --update
- **secrets**: Document the KeePass (kp) secret source
- **secrets**: Lowercase ${kst_…} grammar and env/crd source tags
- **secrets**: Document op and vault external secret managers
- **secrets**: Document ${KST_…} secret injection across README and site

### Features

- **doctor**: Report untracked entries in managed install paths
- **secrets**: Support #json-key extraction for gcp and az
- **site**: Add secret-sources cards to the homepage
- **secrets**: Add aws, gcp, az, pass, and keychain sources
- **secrets**: Harden injection per design review
- **secrets**: Resolve ${kst:kp:…} via the KeePassXC CLI
- **secrets**: Lowercase sentinel and explicit env/crd source tags
- **secrets**: Resolve ${KST:op:…} and ${KST:vault:…} via the op/vault CLIs
- **secrets**: Inject ${KST_…} placeholders into MCP configs at sync time

### Refactor

- Dedup path-set helpers and share MCP JSON object parsing
- **secrets**: Derive vault_path_field from split_field
- **site**: Drop the secret-cards header bar
- **site**: Drop redundant CLI label from secret cards, move above example
- **sync**: Extract classify_mcp_file from sync_mcps
- **sync**: Bundle per-skill install args into SkillJob

### Release

- V3.5.0

### Style

- Normalize unicode ellipsis to ASCII ... across docs, comments, and CLI help
- Collapse single-line use import in sync/skills

## [3.4.0] - 2026-06-21

### Bug fixes

- **commands**: Resolve commands from source_root, not cleanup_dir
- **cache**: Fall back to direct extract when the cache dir is unwritable
- **mcps**: Resolve MCP files from source_root, not cleanup_dir

### Documentation

- Correct inaccuracies found in a full docs audit
- Document source cache, sparse extraction, parallel fetch

### Features

- **site**: Add security headers, Analytics and Speed Insights

### Bench

- Add hyperfine cold-sync benchmark

### Perf

- Sparse-extract sub-dir sources and stream archive bodies
- Download source archives in parallel during skill sync
- Cache extracted source trees for immutable refs

### Release

- V3.4.0

## [3.3.1] - 2026-06-18

### Bug fixes

- **instructions**: Honor sub-dir, prune targetless reconfig, normalize extension names

### Release

- V3.3.1

## [3.3.0] - 2026-06-18

### Bug fixes

- **site**: Align demo status column for longer instruction slugs
- **site**: Override js-yaml and dompurify to patch transitive vulns

### Documentation

- Document instructions as a first-class asset kind
- Showcase instructions in readme demo and hero terminal
- Use brew tap/trust/install for Homebrew instructions

### Features

- **rules**: Distribute agent rules across all agents

### Refactor

- Rename rules asset kind to instructions

### Release

- V3.3.0

## [3.2.0] - 2026-06-12

### Bug fixes

- **fsops**: Preserve file permissions in copy_file
- **update-notifier**: Propagate cache serialization errors
- **fsops**: Make wildcard skill selection deterministic
- **fsops**: Expand only a leading tilde in resolve_path
- **commands**: Match remove by sub-dir from deep browse URLs
- **sync**: Refetch when the configured ref or branch changes
- **sync**: Defer stale-removal when any source failed
- **lock**: Stamp current schema version when saving
- **site**: Move pnpm onlyBuiltDependencies to workspace file
- **tui**: Drop amber tint from banner subtitle
- **tui**: Paint banner Japanese subtitle in amber Side B
- Update keywords
- **site**: Allow site directory in vercel deployment
- **sync**: Resolve skill path relative to source root
- **docs**: Fix uv venv build command for Vercel
- **docs**: Pin Python 3.12 for Vercel build
- **landing**: Shorten etymology line
- **landing**: Mention mcp sources in etymology line
- **landing**: Version badge inherits text color
- **landing**: Version badge same amber as star count
- **landing**: Use accent purple for version badge
- **landing**: Vertically align version and star count in action label
- **landing**: Show version after stars, match label font size
- **landing**: Softer warm off-white light mode palette
- **landing**: Add type=button; fix biome suppression for theme script
- **landing**: Prevent theme icon flash on light-mode; trim trailing blank line
- **mcps**: Stale mcp/ hint, extension check, init template, mcp/-rename warning
- **mcps**: Report broken entries individually; reject non-'*' wildcards
- **tests**: Serialize env-var-touching tests with a mutex to prevent races
- **tests**: Fix without-token test assertions and add env var cleanup
- **github**: Url-encode git refs in API tarball endpoint
- **tests**: Update github archive URL assertions to match API endpoint
- **github**: Use API endpoint for private repo archive downloads
- **github**: Url-encode git refs in API tarball endpoint
- **github**: Use API endpoint for private repo archive downloads
- **config**: Clean up rebase conflict resolution
- **sync**: Harden skill discovery
- **sync**: Follow symlinked directories in skill copy
- **sync**: Discover root-level skill packs
- **lint**: Use sort_by_key instead of sort_by for clippy 1.95 compatibility
- **ci**: Add git-cliff binary to PATH after installation
- **ci**: Install git-cliff binary for release workflow
- **install**: Match archive naming to release artifacts
- Resolve clippy collapsible-if warnings
- **ci**: Tag even when version already matches Cargo.toml
- **branding**: Center ASCII logo within border
- Correct Japanese branding to スキル

### CI/CD

- Fix action versions and test recipe failures
- Drop hashFiles guard; move .no-tests sentinel handling into justfile
- Flatten to one job per language
- Bump action versions to latest major
- Standardize workflow to per-language parallel pipelines on ubuntu-24.04-arm
- Rename Deploy Site workflow to Site
- Drop manual site deploy workflow and rename labels workflow
- **site**: Pin postcss>=8.5.10 to clear GHSA-qx2v-qp2m-jg93
- **site**: AllowBuilds true for esbuild and sharp
- **site**: Allowlist build scripts via pnpm-workspace.yaml
- Point pnpm/action-setup at site/package.json
- **site**: Declare packageManager and node engine in site package.json
- Split workflow into rs and next jobs with audit
- Run landing deploy from repo root; rootDirectory handled by Vercel project settings
- Add manual landing deploy workflow
- Simplify docs deploy — let Vercel build from docs/ via vercel.json
- Use venv instead of --system to avoid permission denied on runner
- Add --break-system-packages for uv on ubuntu-latest
- Update job names
- Update job names
- Move VERCEL_TOKEN to env block to prevent argv leak (bug_007)
- Add manual docs deploy workflow; disable Vercel auto-deploy
- Move site_dir outside docs_dir to fix mkdocs nested build error
- Set docs_dir to current directory in mkdocs.yml
- Correct custom_dir path in mkdocs.yml
- Use uv pip install --system for mkdocs build on Vercel
- Scope lint and build steps to Rust-only recipes
- Extend list of commands
- Add .gitignore
- **labels**: Add workflow_dispatch trigger
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

- Refresh CLAUDE.md for current justfile + CI shape
- **assets**: Tighten social preview centering
- **assets**: Drop terminal chrome from social preview, center wordmark
- **assets**: Restore ascii wordmark in social preview, use jetbrains mono
- **assets**: Redesign social preview with clean wordmark
- **readme**: Color inline status words (updated/added/removed) in demo svg
- **readme**: Bust camo cache for demo svg
- **readme**: Reflow demo svg into two columns for shorter full-width display
- **readme**: Scale demo svg to 50% width, centered
- **readme**: Shrink demo svg further (376h, 12px font, 11px rows)
- **readme**: Compress demo svg vertically, restore full width
- **readme**: Shrink demo svg to 720px and center
- **readme**: Split about-the-name into its own paragraph
- **readme**: Drop resolving spinner, move name blurb below demo
- **readme**: Compress demo svg vertically
- **readme**: Add resolving spinner, rename to demo.svg, span full width
- **readme**: Fix mock totals and hoist the demo above why-kasetto
- **readme**: Swap static mock for animated svg
- **readme**: Mirror the site's sync output mock
- **site**: Mirror sync -v run-length grouping in hero terminal
- **site**: Align hero terminal with new uv-style sync output
- Align with cargo + uv CLI-only direction
- Sync CLAUDE.md with justfile and ci updates
- **github**: Simplify pull request template
- **ci**: Document required secrets at top of workflow files
- Document the lockfile contract
- Use namespaced command names in config examples
- **site**: Rework feature cards, hero, and agent order
- Refine copy, terminology, and accuracy
- **site**: Surface slash commands as third asset kind
- **site**: Move backlinklog badge under footer social icons
- **site**: Add backlinklog badge to footer
- Reorder readme badges
- Add backlinklog badge and tweak rust badge color
- Describe github workflows in CLAUDE.md
- Capitalize Skills in tagline
- Streamline README install and getting started
- Simplify README for landing-page shape
- Remove demo tapes
- **demo**: Add hero demo tape and screen-recording script
- Link readme logo to kasetto.dev
- Document the site subproject in claude.md
- Expand example config with real-world skill packs
- Remove roadmap section
- Add real-world config example link
- Remove --yes and MCP confirmation gate references
- **readme**: Reformat tables; update etymology wording
- Mention MCP sources in name etymology
- Add missing mcps: field to Quick Start MCP example (bug_017)
- **contributing**: Add branch naming and expand commit message standards
- Add Mermaid sync-flow diagram page; fix MkDocs setup
- Update all prose docs for new mcps[] schema
- **config**: Document browser URL auto-rewriting for --config
- **sync**: Document root skill and sub-dir discovery
- **roadmap**: Add library crate extraction
- Soften dark theme text colors to match landing page palette
- Use full banner logo in header and link to landing page
- Replace landing page with quick start as docs index
- **roadmap**: Add audit command and smart URL rewriting
- Update startup routing diagram after root-level sync flag removal
- Update Vercel config
- Unify install URLs and fix doc alignment
- Extend "Why Kasetto" section
- Add landing page
- Update sections
- Update notes
- Update descriptions
- Add icons into cards
- Add authentication, skill format, multi-agent, and git host docs
- Update documentation to match current codebase
- Add usage descriptions for each commit prefix in contribution guidelines
- Add Vercel deployment config and rename Manifest-backed to Traceable
- Add mkdocs-material documentation site
- Cleanup
- **readme**: Fix license badge and clarify Vercel Skills comparison
- **readme**: Humanize copy and reorder sections
- Align rust badge styling with license badge
- Make rust stable badge green
- Reorder badges in requested order
- Align short description with Rust-first positioning
- Add flat-style badges to README

### Features

- **commands**: Guard add --locked + split add/remove into separate demo screens
- **commands**: Cargo/uv-style flags on add/remove/lock + two-scene demo
- **commands**: Add cargo/uv-style add, remove, and lock subcommands
- **site**: Show latest release next to stars and refresh og image
- **site**: Replace fumadocs callout with kasetto-styled component
- **ui**: Add eprint_warn and eprint_error stderr prefixes
- **sync**: Plumb -v / -vv verbosity levels through sync
- **ui**: Add tip: prefix helper and use in empty kst list
- **cli**: Add --color flag and deprecate --plain
- **sync**: Uv-style past-tense summary with elapsed timing
- **cli**: Count-action -q and -v for verbosity granularity
- **config**: Prefer local kasetto.yaml over preferences source
- **site**: Expand opengraph and twitter metadata
- **sync**: Make kasetto.lock authoritative with --update and --locked
- **lock**: Make kasetto.lock portable and commit-friendly
- **site**: Add light theme with toggle
- **site**: Add single-source config example sync
- **sync**: Add slash commands as third asset kind
- **tui**: Tint banner subtitle amber and drop star animation
- **site**: Align OG image with brand title and description
- **site**: Refine link preview metadata and OG image typography
- **site**: Add personal blog link to footer socials
- **site**: Show favicon mark in top nav
- **cli**: Notify when a new version is released
- **site**: Show repo name and star count in topnav
- **site**: Scroll cue, hero rebalance, unify warm accent to rust
- **site**: Proper multi-column footer, polished navbar, wider hero demo
- **site**: Typewriter command and per-row spinner in hero demo
- **site**: Wire docs search, a11y polish, animated kst demo
- **site**: Polish homepage hero and docs styling
- **site**: Merge landing and docs into unified next.js site
- **config**: Add extends for config inheritance
- **sync**: Remove MCP confirmation prompt
- **sync**: Add --force flag and surface skipped MCPs
- **landing**: Add name etymology under tagline
- **landing**: Show latest release version next to GitHub row
- **landing**: Add dark/light theme toggle; refresh code block and grid bg
- **cli**: Add config source management commands (#26)
- **search**: Add SkillsMP marketplace search command
- **config**: Support remote preset configs
- **config**: Support reusable skill presets
- **config**: Rewrite browser URLs to raw content for --config
- **sync**: Support skills source sub-dir selection
- **security**: Add self-update checksum verification and MCP approval gate
- **config**: Add KASETTO_CONFIG env var, preferences file, and rename global config
- **init**: Add --global config scaffold option
- **config**: Add global default config fallback
- Add init, clean, uninstall, completions commands and MCP support
- Add Windows builds and Scoop distribution
- Add shell completions and auto version bumping
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

- **site**: Reformat chip strip after fragment removal
- **github**: Remove issue templates
- **site**: Rename package to kasetto 1.0.0 and bump biome
- **justfile**: Rename site recipes to next and add install/audit
- **tests**: Silence field-reassign and useless-vec clippy lints
- **site**: Bump biome schema to 2.4.14
- **brand**: Recolor logo Japanese subtitle to amber
- Update name of demo scripts
- **brand**: Align visual identity with cassette DNA
- **gitignore**: Ignore macos .ds_store files and local .claude/ config
- Remove stale example
- **site**: Disable vercel auto-deploy on push to main
- **ci**: Rename vercel project secret variable
- **docs**: Remove mkdocs and finish migration to fumadocs
- **justfile**: Add update-next, update-docs, split update into sub-recipes
- Rebrand description and bump version to 2.0.1
- **deps**: Bump unicode-width from 0.1.14 to 0.2.2
- **deps**: Bump actions/download-artifact from 4 to 8
- **deps**: Bump actions/upload-artifact from 4 to 7
- Update version to 1.0.0 and improve spinner feedback
- Update chore files
- **deps**: Bump actions/checkout from 4 to 6
- Remove Go migration leftovers and keep Rust-only docs
- Productionize repo with CI/CD, Homebrew formula, and governance docs

### Refactor

- **sync**: Add skill_key helper for lock keys
- **fsops**: Rename now_iso to now_unix_str
- **mcps**: Drop dead quote-trim on codex env values
- **site**: Align palette and hero terminal mock with CLI
- **site**: Drop theme toggle, commit to dark-only
- **cli**: Unify section header grammar across commands
- **src**: Consolidate sync cleanup, split fsops, drop dead code
- **cli**: Reduce palette to 7 semantic roles, standardize verb headers
- **init**: Tighten next-steps prose
- **clean**: Adopt sync's glyph + summary pattern
- **list**: Short source URLs and unify with sync formatting
- **sync**: Run-length-group source URL on per-row labels
- **sync**: Drop per-row green verb, color glyph only
- **doctor**: Bold-aligned panels and grouped sections
- **colors**: Drop popil truecolor for operational ANSI 16-color
- **output**: Drop ✓/✗ symbols for uv-style past-tense lines
- **output**: Adopt uv prefixes for errors and field labels
- **sync**: Drop status-chip backgrounds for uv prefix glyphs
- **colors**: Centralize color-stdout gating in ui
- **colors**: Adopt popil palette as kasetto's color anchor
- **cli**: Show banner only on help and init
- **list**: Replace interactive browser with uv-style tables
- **cli**: Drop interactive home screen for bare invocation
- Apply formatter to sync modules
- Tighten clippy lints, drop dead code, document internals
- **site**: Sort imports
- **list**: Inline tab counts and drop separate header row
- **site**: Reorder footer socials and use globe icon
- **list**: Extract draw_tab_content to dedupe tab arms
- **list**: Split run into load and print helpers
- **clean**: Split run into removal and report helpers
- **tui**: Drop unused crossterm SUCCESS and WARNING consts
- **tui**: Migrate palette to brand RGB and add Side B accent
- **sync**: Remove --force flag
- **mcps**: Align mcps[] schema with skills[] — drop path: field
- **cli**: Remove root-level sync flags, always show TUI on bare kst
- Run linters
- Run linters
- Update flags
- Align Home and List TUIs
- Rename config example
- **commands**: Split sync list doctor into modules
- Rename project from sukiru to kasetto
- Rename project, binary, and branding from sukiro to sukiru
- Migrate sukiro core from Go to Rust
- Rename project and CLI to sukiro

### Testing

- Share one race-free temp_dir helper across modules
- **config**: Add unit tests for config resolution priority logic
- **core**: Expand coverage for fsops profile and ui

### Build

- Drop crossterm dependency and TUI scaffolding
- **deps**: Update cargo lockfile
- **deps**: Add next-themes
- **deps**: Update dependencies
- Update dev dependencies
- Update dev dependencies
- Update dependencies

### Perf

- **sync**: Hash each skill destination once per run
- Drop redundant clones and allocations
- **tui**: Redraw home and list browser on events only
- Use mimalloc, fat LTO, streaming I/O, and SQLite tuning

### Release

- V3.2.0
- V3.1.0
- V3.0.0
- V2.12.0
- V2.11.0
- V2.10.0
- V2.9.1
- V2.9.0
- V2.8.1
- V2.7.0
- V2.6.1
- V2.6.0
- V2.5.1
- V2.5.0
- V2.4.0
- V2.3.0
- V2.2.0
- V2.1.0
- V2.0.1
- V2.0.0
- V1.2.1
- V1.2.0
- V1.1.0

### Revert

- **readme**: Drop two-column demo layout, restore single column
- Roll back to ab457a1 (before config presets and search features)

### Style

- **site**: Polish demo snippet, footer, and docs background
- **site**: Swap Side B accent from rust to amber
- **landing**: Apply formatter line wrap
- Run formatters
- Run formatters
- Run formatters
- Run formatters
- **ci**: Capitalize step names in workflows

