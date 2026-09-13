# Contributing

- [Contributing](#contributing)
  - [Reporting Bugs](#reporting-bugs)
    - [How to Submit a Bug Report](#how-to-submit-a-bug-report)
  - [Suggesting Enhancements](#suggesting-enhancements)
    - [How to Submit an Enhancement](#how-to-submit-an-enhancement)
  - [Code Contributions](#code-contributions)
    - [Local Development](#local-development)
    - [CI/CD](#cicd)
    - [Branches](#branches)
    - [Commits](#commits)
    - [Pull Requests](#pull-requests)

Thank you for taking the time to contribute.

These guidelines are intended to make contributions consistent and easy to review across repositories. They are guidance, not hard instructions, and maintainers may adapt them when needed.

## Reporting Bugs

Before creating a bug report, search existing issues to avoid duplicates.

When opening a bug report, include enough context for someone else to reproduce the issue and understand the impact.

> [!NOTE]
> If you find a closed issue that looks similar, open a new issue and link the previous one.

### How to Submit a Bug Report

Open a bug report and provide the following:

- A clear, descriptive title
- Reproduction steps (minimal and reliable if possible)
- Current behavior and expected behavior
- Relevant environment details (for example OS, runtime, browser, framework versions)
- Logs, stack traces, screenshots, or recordings when useful

If the issue is intermittent, describe how often it happens and known triggers.
If the issue appeared after a change, mention the last known working version or commit if available.

## Suggesting Enhancements

Before submitting an enhancement, check whether a similar request already exists.

Enhancement requests can include new features, changes to existing behavior, usability improvements, or performance improvements.

### How to Submit an Enhancement

Open a feature request and provide the following:

- A clear problem statement
- The proposed solution
- Alternatives considered or current workarounds
- Expected impact (who benefits and how)

Concrete examples, API sketches, UI mockups, or references are helpful when relevant.

## Code Contributions

### Local Development

This project needs a Rust toolchain (`cargo`) for the CLI and a Node.js/pnpm toolchain for the `site/` app.
This project uses [`just`](https://github.com/casey/just) as its task runner. Run `just --list` for the full set; these are the ones you need day to day:

| Command | What it does |
| --- | --- |
| `install` | Runs `install-rs` and `install-site` |
| `install-rs` | Fetches the crate dependencies into the cargo cache |
| `install-site` | Installs the `site/` app's Node dependencies with pnpm |
| `format` | Runs `format-rs` and `format-site` |
| `format-rs` | Reformats the Rust sources in place |
| `format-site` | Reformats the `site/` sources in place |
| `lint` | Runs `lint-rs` and `lint-site` |
| `lint-rs` | Lints every Rust target with Clippy, failing on any warning |
| `lint-site` | Lints the `site/` sources |
| `test` | Runs `test-rs` and `test-site` |
| `test-rs` | Runs the Rust test suite, skipped with a message if a `.no-tests` sentinel file exists |
| `test-site` | No-op; the site has no test suite |
| `check` | Runs `lint`, `test`, and `build` |
| `update` | Runs `update-rs` and `update-site` |
| `update-rs` | Upgrades the crate lockfile to the newest compatible versions |
| `update-site` | Upgrades the `site/` app's Node dependencies |
| `build` | Runs `build-rs` and `build-site` |
| `build-rs` | Builds the optimized release binaries |
| `build-site` | Builds the production bundle for the `site/` app |
| `run-dev-server` | Serves the `site/` app locally in development mode |
| `run-prod-server` | Serves the built production `site/` app locally |
| `generate-changelog` | Regenerates `CHANGELOG.md` from the commit history with git-cliff |
| `generate-config-docs` | Regenerates the example config in the README and docs, then reformats the affected site file |
| `generate-social-preview` | Rasterizes the social preview SVG into a 1280x640 PNG via `rsvg-convert` |
| `benchmark-sync` | Runs the cold-sync benchmark script |

1. Fork the repository and create a branch for your change
2. Set up the project with `just install`, then make your change
3. Run `just check` and fix anything it reports before opening a pull request

> [!IMPORTANT]
> Behavioral code changes should include or update tests.

### CI/CD

Workflows live in `.github/workflows`:

| Workflow | Trigger | What it does |
| --- | --- | --- |
| CI | Push to `main`, pull requests, `workflow_dispatch` | Two parallel jobs, both on `ubuntu-24.04-arm`: `ci-rs` sets up the stable Rust toolchain with rustfmt and Clippy over a cached cargo registry, then installs, lints, tests and builds the crate; `ci-next` sets up pnpm with Node 24, then installs, lints, tests and builds the `site/` app |
| Release | `workflow_dispatch` (optional `version` input) | Six chained jobs: `tag` (`ubuntu-24.04-arm`) resolves the next version from the commit history with git-cliff or from the optional input, bumps the version in `Cargo.toml` and the lockfile, regenerates `CHANGELOG.md`, then commits and pushes `main` with the `v<version>` tag; `build` needs `tag` and cross-compiles release binaries for six targets (`x86_64` and `aarch64` for `unknown-linux-gnu` on `ubuntu-latest`, `apple-darwin` on `macos-latest`, `pc-windows-msvc` on `windows-latest`), packaging `kasetto` and `kst` into a per-target `.tar.gz` or `.zip`; `release` needs `tag` and `build` and publishes the GitHub Release with the generated notes, every archive and a `checksums.txt`; then `publish-crate`, `update-homebrew` and `update-scoop` (all `ubuntu-24.04-arm`, each needing `tag` and `release`) publish the crate to Crates.io, push a refreshed `Formula/kasetto.rb` to `pivoshenko/homebrew-tap`, and push a refreshed `kasetto.json` to `pivoshenko/scoop-bucket` |
| Site | `workflow_dispatch` | Single `deploy` job on `ubuntu-latest`; deploys the `site/` app to Vercel production using the `VERCEL_TOKEN`, `VERCEL_ORG_ID` and `VERCEL_SITE_PROJECT_ID` secrets |

CI must be green before a pull request is merged.

### Branches

Branch names follow the pattern `<type>/<short-description>` using the same type prefixes as commits.
The description should be lowercase kebab-case, brief, and specific enough to identify the change at a glance.

Examples:

```
feat/github-token-refresh
fix/private-repo-archive-auth
docs/update-sync-flow-diagram
refactor/mcps-schema-alignment
```

A branch covering multiple unrelated changes should be split. One concern per branch makes review and bisect much easier.

### Commits

Use clear, focused commits with descriptive messages.

This project follows [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

**Format**

```
<type>(<scope>): <subject>

[optional body]
```

- **type** - one of the prefixes from the table below
- **scope** - the module, command, or area being changed (e.g. `sync`, `mcps`, `github`, `landing`, `config`); omit when the change is truly cross-cutting
- **subject** - imperative mood, lowercase, no trailing period, 72 characters or fewer
- **body** - optional; use it to explain *why*, not *what*; wrap at 72 characters

**Type prefixes**

| Prefix     | When to use                                                                                             |
| ---------- | ------------------------------------------------------------------------------------------------------- |
| `feat`     | A new feature or user-facing capability                                                                 |
| `fix`      | A bug fix that corrects incorrect behavior                                                              |
| `docs`     | Changes to documentation only (README, comments, guides)                                                |
| `refactor` | Code restructuring that does not change external behavior (renaming, extracting functions, simplifying) |
| `test`     | Adding, updating, or fixing tests without changing production code                                      |
| `chore`    | Maintenance tasks that don't affect source code or tests (dependency bumps, config tweaks, .gitignore)  |
| `ci`       | Changes to CI/CD configuration and scripts (GitHub Actions, workflows, pipelines)                       |
| `build`    | Changes to the build system or external dependencies (Cargo.toml, build scripts, Makefile)              |
| `perf`     | A code change that improves performance without altering functionality                                  |
| `style`    | Formatting-only changes (whitespace, semicolons, linting) with no logic changes                         |
| `design`   | Changes to visual or UI design assets and layout                                                        |
| `revert`   | Reverts a previous commit (reference the reverted commit hash in the body)                              |

**Examples**

```
feat(sync): support skills source sub-directory selection
fix(github): url-encode git refs in API tarball endpoint
refactor(mcps): align mcps[] schema with skills[]
docs(config): document browser URL auto-rewriting for --config
```

### Pull Requests

- Fill out the pull request template completely
- Keep the pull request focused and scoped to one change set
- Ensure tests and checks pass before requesting review
- Update documentation when behavior or interfaces change
- Respond to review feedback and keep the branch up to date with the target branch

Maintainers may ask for changes, additional tests, or scope adjustments before merging.
