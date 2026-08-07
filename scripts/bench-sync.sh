#!/usr/bin/env bash
#
# Cold-sync benchmark for kasetto, driven by hyperfine.
#
# Quantifies the two network/IO wins of the parallel-fetch + source-cache work:
#
#   1. Parallel vs serial source download
#      Same binary, cache disabled, identical work — only the rayon thread count
#      differs (RAYON_NUM_THREADS=1 reproduces the old sequential fetch). Isolates
#      the latency-overlap from downloading independent sources concurrently.
#
#   2. Cold vs warm source cache
#      Immutable-ref (SHA-pinned) sources, install dir + lock wiped before every
#      run so each is a fresh "cold sync". Cold disables the cache (re-downloads
#      every run); warm reuses the on-disk extracted-tree cache (zero network).
#
# Every timed run is a true cold sync: the lock and install directory are removed
# in hyperfine's --prepare, so kasetto must re-resolve and re-materialize.
#
# Sample results (2 default sources, 8 runs, warm network):
#   Scenario 1  parallel fetch  1.25× faster than serial      (2 sources)
#               ... grows with source count: ~1.71× at 4 sources (max-vs-sum
#               of independent download latencies).
#   Scenario 2  warm cache      2.48× faster than cold         (no network at all)
#
# Usage:
#   scripts/bench-sync.sh                 # build (if needed) + run, default sources
#   KASETTO_BIN=/path/to/kasetto scripts/bench-sync.sh
#   BENCH_RUNS=10 scripts/bench-sync.sh
#
# Requires: hyperfine, network access to the configured sources.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${KASETTO_BIN:-$repo_root/target/release/kasetto}"
runs="${BENCH_RUNS:-8}"

# Sources are real public skill repos. The cache scenario pins each to an
# immutable commit SHA (only immutable refs are cacheable); override to taste.
# Format per line: "<url> <sha>".
sources="${BENCH_SOURCES:-\
https://github.com/obra/superpowers 896224c4b1879920ab573417e68fd51d2ccc9072
https://github.com/anthropics/skills 57546260929473d4e0d1c1bb75297be2fdfa1949}"

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "error: hyperfine is not installed (https://github.com/sharkdp/hyperfine)" >&2
  exit 1
fi
if [[ ! -x "$bin" ]]; then
  echo "==> building release binary"
  (cd "$repo_root" && cargo build --release)
fi

work="$(mktemp -d)"
cache="$work/cache"
dest="$work/.claude"
lock="$work/kasetto.lock"
trap 'rm -rf "$work"' EXIT
mkdir -p "$cache"

# Generate the two configs: branch-tracked (moving) and SHA-pinned (immutable).
{
  echo "agent: claude-code"
  echo "scope: project"
  echo "skills:"
  while read -r url _sha; do
    [[ -z "$url" ]] && continue
    echo "  - source: $url"
    echo "    skills: \"*\""
  done <<<"$sources"
} >"$work/branch.yaml"

{
  echo "agent: claude-code"
  echo "scope: project"
  echo "skills:"
  while read -r url sha; do
    [[ -z "$url" ]] && continue
    echo "  - source: $url"
    echo "    ref: $sha"
    echo "    skills: \"*\""
  done <<<"$sources"
} >"$work/pinned.yaml"

n_sources="$(grep -c 'source:' "$work/branch.yaml" || true)"
cd "$work"

reset_install="rm -rf '$dest' '$lock'"

echo
echo "════════════════════════════════════════════════════════════════════"
echo " kasetto cold-sync benchmark — $n_sources sources, $runs runs each"
echo " binary: $bin"
echo "════════════════════════════════════════════════════════════════════"

echo
echo "── Scenario 1: parallel vs serial source download (cache off) ──"
hyperfine \
  --warmup 1 --runs "$runs" \
  --prepare "$reset_install" \
  --command-name "serial fetch (1 thread)" \
    "env RAYON_NUM_THREADS=1 KASETTO_NO_CACHE=1 XDG_CACHE_HOME='$cache' '$bin' sync --config branch.yaml --color never -q" \
  --command-name "parallel fetch (all threads)" \
    "env KASETTO_NO_CACHE=1 XDG_CACHE_HOME='$cache' '$bin' sync --config branch.yaml --color never -q" \
  --export-markdown "$work/scenario1.md" || true

echo
echo "── Scenario 2: cold vs warm source cache (SHA-pinned) ──"
# Pre-warm the cache once so the 'warm' command starts hot; --prepare wipes only
# the install + lock (never the cache), so every warm run still re-materializes.
rm -rf "$dest" "$lock"
env XDG_CACHE_HOME="$cache" "$bin" sync --config pinned.yaml --color never -q || true
hyperfine \
  --warmup 1 --runs "$runs" \
  --prepare "$reset_install" \
  --command-name "cold (no cache, re-downloads)" \
    "env KASETTO_NO_CACHE=1 XDG_CACHE_HOME='$cache' '$bin' sync --config pinned.yaml --color never -q" \
  --command-name "warm (cached, no network)" \
    "env XDG_CACHE_HOME='$cache' '$bin' sync --config pinned.yaml --color never -q" \
  --export-markdown "$work/scenario2.md" || true

echo
echo "Markdown summaries written under: $work (removed on exit)"
