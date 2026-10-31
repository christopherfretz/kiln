#!/usr/bin/env bash
# Fetch libc++'s test suite (libcxx/test + libcxx/utils) at the tag matching
# the installed libc++ headers, for `cargo xtask libcxx` (spec/libcxx.md).
#
# Shallow, blob-filtered, sparse checkout into <main checkout>/.toolchain/
# llvm-project (gitignored, so it survives container restarts and never
# enters the repo). Idempotent: a checkout already at the tag is left alone.
# Prints the checkout path on success.
#
# Usage: scripts/fetch-libcxx-tests.sh [<dir>]
#   LLVM_TAG (default llvmorg-19.1.7) and LLVM_REPO override the source.
set -euo pipefail

TAG="${LLVM_TAG:-llvmorg-19.1.7}"
REPO="${LLVM_REPO:-https://github.com/llvm/llvm-project.git}"

ws="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# From a worktree, use the main checkout's .toolchain (shared, durable).
# The main checkout is the common git dir's work tree: core.worktree when
# kiln is a submodule (its git dir lives under <super>/.git/modules/kiln),
# else the common dir's parent.
common="$(git -C "$ws" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || echo "$ws/.git")"
wt="$(git config --file "$common/config" core.worktree 2>/dev/null || true)"
if [ -n "$wt" ]; then
  case "$wt" in /*) main="$wt";; *) main="$common/$wt";; esac
else
  main="$common/.."
fi
main="$(cd "$main" 2>/dev/null && pwd || echo "$ws")"
[ -d "$main/xtask" ] || main="$ws"
dir="${1:-$main/.toolchain/llvm-project}"

have_tag() {
  [ -d "$dir/.git" ] || return 1
  [ -d "$dir/libcxx/test/std" ] || return 1
  [ -f "$dir/libcxx/utils/libcxx/test/format.py" ] || return 1
  local head want
  head="$(git -C "$dir" rev-parse HEAD 2>/dev/null)" || return 1
  want="$(git -C "$dir" rev-parse "refs/tags/$TAG^{commit}" 2>/dev/null)" || return 1
  [ "$head" = "$want" ]
}

if have_tag; then
  echo "$dir"
  exit 0
fi

if [ -e "$dir" ]; then
  echo "fetch-libcxx-tests: $dir exists but is not a checkout of $TAG; remove it and rerun" >&2
  exit 1
fi

mkdir -p "$(dirname "$dir")"
tmp="$dir.partial-$$"
trap 'rm -rf "$tmp"' EXIT
git clone --quiet --depth 1 --filter=blob:none --no-checkout --branch "$TAG" "$REPO" "$tmp" >&2
git -C "$tmp" sparse-checkout set --no-cone /libcxx/test/ /libcxx/utils/ >&2
git -C "$tmp" checkout --quiet "$TAG" >&2
# Keep the tag ref so the idempotence check can resolve it.
git -C "$tmp" tag -f "$TAG" HEAD >/dev/null
mv "$tmp" "$dir"
trap - EXIT
echo "$dir"
