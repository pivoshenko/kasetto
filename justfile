default:
    @just --list

install-rs:
    cargo fetch

install-next:
    cd site && pnpm install

install: install-rs install-next

format-rs:
    cargo fmt

format-next:
    cd site && pnpm format

format: format-rs format-next

lint-rs:
    cargo clippy --all-targets -- -D warnings

lint-next:
    cd site && pnpm lint

lint: lint-rs lint-next

test:
    cargo test

update-rs:
    cargo update

update-next:
    cd site && pnpm update

update: update-rs update-next

build-rs:
    cargo build --release

build-next:
    cd site && pnpm build

build: build-rs build-next

audit-rs:
    @command -v cargo-audit >/dev/null || cargo install --locked cargo-audit
    cargo audit

audit-next:
    cd site && pnpm audit

audit: audit-rs audit-next

dev-next:
    cd site && pnpm dev

start-next:
    cd site && pnpm start

changelog:
    git-cliff --output CHANGELOG.md

# regenerate README + docs config block from kasetto.example.yaml
sync-config:
    node scripts/sync-config-example.mjs
    cd site && pnpm exec biome format --write app/components/feature-tabs.tsx

check: format lint test build
