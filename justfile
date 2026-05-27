format-rs:
    cargo fmt

format-site:
    cd site && pnpm format

format: format-rs format-site

lint-rs:
    cargo clippy --all-targets -- -D warnings

lint-site:
    cd site && pnpm lint

lint: lint-rs lint-site

test:
    cargo test

update-rs:
    cargo update

update-site:
    cd site && pnpm update

update: update-rs update-site

build-rs:
    cargo build --release

build-site:
    cd site && pnpm build

build: build-rs build-site

serve-site:
    cd site && pnpm dev

changelog:
    git-cliff --output CHANGELOG.md

# regenerate README + docs config block from kasetto.example.yaml
sync-config:
    node scripts/sync-config-example.mjs
    cd site && pnpm exec biome format --write app/components/feature-tabs.tsx

demo-vhs:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v vhs >/dev/null || { echo "vhs not installed — https://github.com/charmbracelet/vhs" >&2; exit 1; }
    command -v kst >/dev/null || { echo "kst not on PATH — run 'cargo install --path .' first" >&2; exit 1; }
    command -v bat >/dev/null || { echo "bat not installed — brew install bat" >&2; exit 1; }
    stage=$(mktemp -d)
    trap 'rm -rf "$stage"' EXIT
    curl -fsSL https://raw.githubusercontent.com/pivoshenko/pivoshenko.ai/main/kasetto.yaml -o "$stage/kasetto.yaml"
    cp assets/scripts/demo.tape "$stage/demo.tape"
    ( cd "$stage" && vhs demo.tape )
    mv "$stage/demo.gif" assets/demo.gif
    echo "rendered → assets/demo.gif"

demo-fish:
    fish assets/scripts/demo.fish

check: format lint test build
