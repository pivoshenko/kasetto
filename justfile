format-rs:
  cargo fmt

format-next:
  cd landing && pnpm format

format: format-rs format-next

lint-rs:
  cargo clippy -- -D warnings

lint-next:
  cd landing && pnpm lint

lint: lint-rs lint-next

test:
  cargo test

update:
  cargo update

build-rs:
  cargo build --release

build-next:
  cd landing && pnpm build

build-docs:
  cd docs && mkdocs build -f mkdocs.yml

build: build-rs build-next

serve-docs:
  cd docs && mkdocs serve -f mkdocs.yml

serve-landing:
  cd landing && pnpm dev

changelog:
  git-cliff --output CHANGELOG.md

check: format lint test build
