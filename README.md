# sukiro

Fast binary CLI to sync AI skills from local paths and GitHub repositories.

## Why

`sukiro` is built for high-speed, deterministic skill syncing:
- local + GitHub sources
- wildcard or explicit skill selection
- hash-based install/update detection
- stale skill cleanup
- dry-run support
- state + run reports

## Install

```bash
go build -o sukiro ./cmd/sukiro
```

## Usage

```bash
./sukiro --config skills.config.yaml
./sukiro --config skills.config.yaml --dry-run
./sukiro --config skills.config.yaml --json
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

## MVP notes

Current MVP focuses on `sync` behavior parity and performance-oriented implementation.
Additional commands (`list`, `doctor`, `prune`) are planned.
