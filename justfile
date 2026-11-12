default:
    @just --list

install: install-rs install-site

install-rs:
    cargo fetch

install-site:
    cd site && pnpm install

format: format-rs format-site

format-rs:
    cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets
    cargo fmt --all

format-site:
    cd site && pnpm format

lint: lint-rs lint-site

lint-rs:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

lint-site:
    cd site && pnpm lint
    cd site && pnpm typecheck

audit: audit-rs audit-site

audit-rs:
    cargo audit

audit-site:
    cd site && pnpm audit

test: test-rs test-site

test-rs:
    @[ -f .no-tests ] && echo "skipping (.no-tests sentinel)" || cargo test --workspace

test-site:
    @cd site && ([ -f .no-tests ] && echo "skipping (.no-tests sentinel)" || { echo "no test command, add tests or restore .no-tests" >&2; exit 1; })

check: lint test build

update: update-rs update-site

update-rs:
    cargo update

update-site:
    cd site && pnpm update

build: build-rs build-site

build-rs:
    cargo build --release --workspace

build-site:
    cd site && pnpm build

run-dev-server:
    cd site && pnpm dev

run-prod-server: build-site
    cd site && pnpm start

generate-changelog:
    git-cliff --output CHANGELOG.md

generate-config-docs:
    node scripts/sync-config-example.mjs
    cd site && pnpm exec biome format --write app/components/feature-tabs.tsx

generate-social-preview:
    rsvg-convert -b '#1f1f1e' --page-width 1280 --page-height 640 --top 41 \
      -w 1280 -h 558 assets/preview_social_dark.svg -o assets/preview_social_dark.png

benchmark-sync:
    ./scripts/bench-sync.sh
