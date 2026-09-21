default:
    @just --list

install: install-rs install-site

install-rs:
    cargo fetch

install-site:
    cd site && pnpm install

format: format-rs format-site

format-rs:
    cargo fmt

format-site:
    cd site && pnpm format

lint: lint-rs lint-site

lint-rs:
    cargo clippy --all-targets -- -D warnings

lint-site:
    cd site && pnpm lint

test: test-rs test-site

test-rs:
    cargo test

test-site:
    @echo "no Next.js tests"

check: lint test build

update: update-rs update-site

update-rs:
    cargo update

update-site:
    cd site && pnpm update

build: build-rs build-site

build-rs:
    cargo build --release

build-site:
    cd site && pnpm build

run-dev-server:
    cd site && pnpm dev

run-prod-server:
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
