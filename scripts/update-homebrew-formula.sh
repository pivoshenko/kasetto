#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <version> <sha256>"
  echo "Example: $0 v0.1.1 abcdef..."
  exit 1
fi

VERSION="$1"
SHA="$2"
FORMULA="Formula/kasetto.rb"

sed -i "s#url \"https://github.com/pivoshenko/kasetto/archive/refs/tags/v[^"]*\.tar\.gz\"#url \"https://github.com/pivoshenko/kasetto/archive/refs/tags/${VERSION}.tar.gz\"#" "$FORMULA"
sed -i "s#sha256 \"[^"]*\"#sha256 \"${SHA}\"#" "$FORMULA"

echo "Updated ${FORMULA} -> ${VERSION}"
