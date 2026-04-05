#!/usr/bin/env bash
set -euo pipefail

# Compute the next semantic version from conventional commits using git-cliff,
# bump Cargo.toml, generate the changelog, then commit and tag.
#
# Usage:
#   ./scripts/release.sh              # auto-detect bump from commits
#   ./scripts/release.sh minor        # force a minor bump
#   ./scripts/release.sh 2.1.0        # use an explicit version

BUMP="${1:-}"

# Resolve next version

if [[ -z "$BUMP" ]]; then
    NEXT=$(git-cliff --bumped-version)
elif [[ "$BUMP" =~ ^(major|minor|patch)$ ]]; then
    NEXT=$(git-cliff --bumped-version --bump "$BUMP")
elif [[ "$BUMP" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    NEXT="v$BUMP"
else
    echo "error: argument must be major, minor, patch, or a semver (e.g. 2.1.0)" >&2
    exit 1
fi

VERSION="${NEXT#v}"
TAG="v${VERSION}"

echo "releasing ${TAG}"

# Check for clean working tree

if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "error: working tree is dirty — commit or stash changes first" >&2
    exit 1
fi

# Bump Cargo.toml

if [[ "$(uname)" == "Darwin" ]]; then
    sed -i '' "s/^version = .*/version = \"${VERSION}\"/" Cargo.toml
else
    sed -i "s/^version = .*/version = \"${VERSION}\"/" Cargo.toml
fi

cargo update --workspace 2>/dev/null

# Generate changelog

git-cliff --tag "$TAG" --output CHANGELOG.md

# Commit and tag

git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "release: ${TAG}"
git tag -a "$TAG" -m "release: ${TAG}"

echo "done — run 'git push origin main --tags' when ready"
