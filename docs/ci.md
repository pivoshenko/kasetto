# CI & Automation

**When you need this:** You want to validate `kasetto.yaml` in CI, keep team environments reproducible, or integrate Kasetto into scripts.

**What you'll learn:**

- Recommended flags (`--dry-run`, `--json`, `--plain`, `--quiet`)
- Exit-code expectations
- A GitHub Actions example

## Recommended Commands

### Validate Without Writing

Use `--dry-run` to check that sources resolve and that the plan matches expectations without touching disk:

```bash
kst sync --dry-run
```

### JSON Output for CI Logs

Use `--json` for structured logs:

```bash
kst sync --dry-run --json
```

### Avoid TUIs in Non-Interactive Environments

Kasetto will avoid interactive UIs when it detects non-interactive output, but you can force it:

- Set `NO_TUI=1`
- Or use `--plain` / `--quiet` as needed

```bash
NO_TUI=1 kst sync --dry-run --plain
```

## Exit Codes

Kasetto is designed to keep going when individual skills are missing/broken in a source, but failures that prevent reading sources/configs are treated as errors.

If you're depending on strict enforcement in CI, pair `--dry-run` with `--json` and enforce policy in the CI step based on the report.

## GitHub Actions Example

This validates a repo's `kasetto.yaml` (project scope) without writing changes:

```yaml
name: kasetto

on:
  pull_request:
  push:
    branches: [main]

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install kasetto
        run: curl -fsSL https://raw.githubusercontent.com/pivoshenko/kasetto/main/scripts/install.sh | sh

      - name: Validate kasetto.yaml
        env:
          NO_TUI: "1"
          # Add tokens if you pull from private sources:
          # GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: kst sync --project --dry-run --json
```
