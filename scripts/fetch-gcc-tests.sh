#!/usr/bin/env bash
# Fetch GCC's own C tests (gcc/testsuite/gcc.c-torture/{execute,compile})
# at a pinned GCC release tag, for `cargo xtask gcc-torture`
# (spec/harness.md "GCC testsuites"). The tests are GPL (GCC's testsuite
# license) and are NEVER vendored into kiln: the runner reads them from this
# checkout, outside the tracked tree.
#
# Shallow, blob-filtered, sparse checkout into <checkout>/.oss/gcc-tests
# (gitignored; about 15 MB). Idempotent: a checkout already at the tag is
# left alone, and its sparse set is refreshed, so a later suite (g++.dg,
# g++.old-deja for correctness mining) is one more entry in PATHS. Prints
# the checkout path on success.
#
# Usage: scripts/fetch-gcc-tests.sh [<tag>] [<dir>]
#   <tag> is a releases/gcc-* tag (default releases/gcc-16.1.0, the GCC
#   oracle's release); a first argument not starting with releases/ is
#   <dir>. GCC_REPO overrides the source (default the GitHub mirror, which
#   carries the release tags).
set -euo pipefail

TAG="releases/gcc-16.1.0"
case "${1:-}" in
  releases/gcc-*) TAG="$1"; shift;;
esac
case "$TAG" in
  *..*|*' '*) echo "fetch-gcc-tests: bad tag '$TAG'" >&2; exit 2;;
esac
REPO="${GCC_REPO:-https://github.com/gcc-mirror/gcc.git}"
# The suites the runners read. The torture runner needs only these two
# directories (the `.x` files beside the tests are read, never run).
PATHS=(/gcc/testsuite/gcc.c-torture/execute/ /gcc/testsuite/gcc.c-torture/compile/)
ws="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${1:-$ws/.oss/gcc-tests}"

if [ -d "$dir/.git" ] && [ -d "$dir/gcc/testsuite" ]; then
  head="$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)"
  want="$(git -C "$dir" rev-parse "refs/tags/$TAG^{commit}" 2>/dev/null || true)"
  if [ -n "$head" ] && [ "$head" = "$want" ]; then
    git -C "$dir" sparse-checkout set --no-cone "${PATHS[@]}" >&2
    echo "$dir"
    exit 0
  fi
fi
if [ -e "$dir" ]; then
  echo "fetch-gcc-tests: $dir exists but is not a checkout of $TAG; remove it and rerun" >&2
  exit 1
fi

mkdir -p "$(dirname "$dir")"
tmp="$dir.partial-$$"
trap 'rm -rf "$tmp"' EXIT
git clone --quiet --depth 1 --filter=blob:none --no-checkout --branch "$TAG" "$REPO" "$tmp" >&2
git -C "$tmp" sparse-checkout set --no-cone "${PATHS[@]}" >&2
git -C "$tmp" checkout --quiet "$TAG" >&2
git -C "$tmp" tag -f "$TAG" HEAD >/dev/null
mv "$tmp" "$dir"
trap - EXIT
echo "$dir"
