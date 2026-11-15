#!/usr/bin/env bash
# Fetch GCC's own tests (gcc/testsuite/gcc.c-torture/{execute,compile} for
# `cargo xtask gcc-torture`; g++.dg, c-c++-common and g++.old-deja for
# `cargo xtask gxx-dg`) at a pinned GCC release tag (spec/harness.md "GCC
# testsuites"). The tests are GPL (GCC's testsuite
# license) and are NEVER vendored into kiln: the runner reads them from this
# checkout, outside the tracked tree.
#
# Shallow, blob-filtered, sparse checkout into <checkout>/.oss/gcc-tests
# (gitignored; about 160 MB with the C++ suites). Idempotent: a checkout
# already at the tag is left alone, and its sparse set is refreshed when
# PATHS changed, so a later suite is one more entry in PATHS. Prints the
# checkout path on success.
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
# The suites the runners read. The torture runner needs these two
# directories (the `.x` files beside the tests are read, never run) and the
# few files of other suites that torture tests #include by relative path
# (`grep -rh '#include "\.\./' gcc.c-torture` lists them). The g++.dg
# runner (`cargo xtask gxx-dg`) reads g++.dg, c-c++-common and g++.old-deja,
# and the gcc.dg / gcc.target files the tests it runs include by relative
# path (those of directories with their own .exp driver, which it does not
# run, are left out).
PATHS=(/gcc/testsuite/gcc.c-torture/execute/ /gcc/testsuite/gcc.c-torture/compile/
  /gcc/testsuite/gcc.dg/20030309-1.c /gcc/testsuite/gcc.dg/pr98304-1.c
  /gcc/testsuite/gcc.dg/tree-ssa/pr105777.c /gcc/testsuite/gcc.dg/tree-ssa/pr109938.c
  /gcc/testsuite/gcc.dg/tree-ssa/pr109986.c /gcc/testsuite/gcc.dg/tree-ssa/pr30314.c
  /gcc/testsuite/gcc.target/powerpc/ppc-switch-2.c
  /gcc/testsuite/g++.dg/ /gcc/testsuite/c-c++-common/ /gcc/testsuite/g++.old-deja/
  /gcc/testsuite/gcc.dg/pr104992.c /gcc/testsuite/gcc.dg/range.h
  /gcc/testsuite/gcc.dg/tg-tests.h /gcc/testsuite/gcc.dg/tree-ssa/pr91680.c
  /gcc/testsuite/gcc.dg/c23-stdarg-9.c /gcc/testsuite/gcc.dg/builtin-object-size-common.h
  /gcc/testsuite/gcc.dg/cpp/c11-elifdef-1.c /gcc/testsuite/gcc.dg/cpp/c23-elifdef-1.c
  /gcc/testsuite/gcc.target/i386/bmi2-check.h)
ws="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${1:-$ws/.oss/gcc-tests}"

if [ -d "$dir/.git" ] && [ -d "$dir/gcc/testsuite" ]; then
  head="$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)"
  want="$(git -C "$dir" rev-parse "refs/tags/$TAG^{commit}" 2>/dev/null || true)"
  if [ -n "$head" ] && [ "$head" = "$want" ]; then
    # Only when the set changed: re-applying it stats every file of the
    # checkout, minutes on the dev host's shared file system.
    have="$(git -C "$dir" sparse-checkout list 2>/dev/null || true)"
    if [ "$have" != "$(printf '%s\n' "${PATHS[@]}")" ]; then
      git -C "$dir" sparse-checkout set --no-cone "${PATHS[@]}" >&2
    fi
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
