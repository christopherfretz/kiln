#!/usr/bin/env bash
# Fetch clang's own C++ tests (clang/test/{CXX,SemaCXX,SemaTemplate,Parser})
# at a pinned llvm-project tag, for `cargo xtask clang-neg`
# (spec/clang-neg.md). The tests are Apache-2.0 WITH LLVM-exception and are
# never vendored into kiln: the runner reads them from this checkout.
#
# Shallow, blob-filtered, sparse checkout into <checkout>/.oss/clang-tests
# (gitignored; about 50 MB). Idempotent: a checkout already at the tag is
# left alone. Prints the checkout path on success.
#
# Usage: scripts/fetch-clang-tests.sh [<tag>] [<dir>]
#   <tag> is an llvmorg-* release tag (default llvmorg-22.1.8, the tag the
#   published numbers use); a first argument not starting with llvmorg- is
#   <dir>. LLVM_REPO overrides the source.
set -euo pipefail

TAG="llvmorg-22.1.8"
case "${1:-}" in
  llvmorg-*) TAG="$1"; shift;;
esac
case "$TAG" in
  */*|*..*) echo "fetch-clang-tests: bad tag '$TAG'" >&2; exit 2;;
esac
REPO="${LLVM_REPO:-https://github.com/llvm/llvm-project.git}"
# The four test directories, plus the few files outside them that tests
# there #include by relative path.
PATHS=(/clang/test/CXX/ /clang/test/SemaCXX/ /clang/test/SemaTemplate/ /clang/test/Parser/
  /clang/test/Sema/attr-callback.c /clang/test/Sema/attr-callback-broken.c
  /clang/test/CodeGen/X86/builtin_test_helpers.h)
ws="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${1:-$ws/.oss/clang-tests}"

if [ -d "$dir/.git" ] && [ -d "$dir/clang/test/CXX" ]; then
  head="$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)"
  want="$(git -C "$dir" rev-parse "refs/tags/$TAG^{commit}" 2>/dev/null || true)"
  if [ -n "$head" ] && [ "$head" = "$want" ]; then
    git -C "$dir" sparse-checkout set --no-cone "${PATHS[@]}" >&2
    echo "$dir"
    exit 0
  fi
fi
if [ -e "$dir" ]; then
  echo "fetch-clang-tests: $dir exists but is not a checkout of $TAG; remove it and rerun" >&2
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
