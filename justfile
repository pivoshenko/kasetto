default:
    @just --list

install: install-package install-site

install-package:
    cargo fetch

install-site:
    cd site && pnpm install

format: format-package format-site

format-package:
    cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets
    cargo fmt --all

format-site:
    cd site && pnpm format

lint: lint-package lint-site

lint-package:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

lint-site:
    cd site && pnpm lint
    cd site && pnpm typecheck

audit: audit-package audit-site

audit-package:
    cargo audit

audit-site:
    cd site && pnpm audit

test: test-package test-site

test-package:
    @[ -f .no-tests ] && echo "skipping (.no-tests sentinel)" || cargo test --workspace

test-site:
    @cd site && ([ -f .no-tests ] && echo "skipping (.no-tests sentinel)" || { echo "no test command, add tests or restore .no-tests" >&2; exit 1; })

check: lint test build

update: update-package update-site

update-package:
    cargo update

update-site:
    cd site && pnpm update

build: build-package build-site

build-package:
    cargo build --release --workspace

build-site:
    cd site && pnpm build

run-dev-server:
    cd site && pnpm dev

run-prod-server: build-site
    cd site && pnpm start

generate-config-docs:
    node scripts/sync-config-example.mjs
    cd site && pnpm exec biome format --write app/components/feature-tabs.tsx

generate-social-preview:
    rsvg-convert -b '#1f1f1e' --page-width 1280 --page-height 640 --top 41 \
      -w 1280 -h 558 assets/preview_social_dark.svg -o assets/preview_social_dark.png

benchmark-sync:
    ./scripts/bench-sync.sh
