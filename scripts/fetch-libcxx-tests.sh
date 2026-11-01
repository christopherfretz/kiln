#!/usr/bin/env bash
# Fetch libc++'s test suite (libcxx/test + libcxx/utils) at the tag matching
# the installed libc++ headers, for `cargo xtask libcxx` (spec/libcxx.md).
#
# Shallow, blob-filtered, sparse checkout into <main checkout>/.toolchain/
# libcxx-<tag> (gitignored, so it survives container restarts and never
# enters the repo), so checkouts of different releases coexist (the M3 gate
# stays on 19 while the clang 22 oracle uses 22; spec/harness.md "Oracle
# toolchain"). Idempotent: a checkout already at the tag is left alone. For
# the default tag, a checkout at the older location .toolchain/llvm-project
# is used as is. Prints the checkout path on success; `cargo xtask libcxx
# --tag <tag>` finds it.
#
# Usage: scripts/fetch-libcxx-tests.sh [<tag>] [<dir>]
#   <tag> is an llvmorg-* release tag (default: $LLVM_TAG, else
#   llvmorg-19.1.7); a first argument not starting with llvmorg- is <dir>.
#   LLVM_REPO overrides the source.
set -euo pipefail

DEFAULT_TAG="llvmorg-19.1.7"
TAG="${LLVM_TAG:-$DEFAULT_TAG}"
case "${1:-}" in
  llvmorg-*) TAG="$1"; shift;;
esac
case "$TAG" in
  llvmorg-?*) ;;
  *) echo "fetch-libcxx-tests: bad tag '$TAG' (want llvmorg-<version>)" >&2; exit 2;;
esac
case "$TAG" in
  */*|*..*) echo "fetch-libcxx-tests: bad tag '$TAG'" >&2; exit 2;;
esac
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
dir="${1:-$main/.toolchain/libcxx-$TAG}"

# have_tag <dir>: <dir> is a complete checkout at $TAG.
have_tag() {
  local d="$1"
  [ -d "$d/.git" ] || return 1
  [ -d "$d/libcxx/test/std" ] || return 1
  [ -f "$d/libcxx/utils/libcxx/test/format.py" ] || return 1
  local head want
  head="$(git -C "$d" rev-parse HEAD 2>/dev/null)" || return 1
  want="$(git -C "$d" rev-parse "refs/tags/$TAG^{commit}" 2>/dev/null)" || return 1
  [ "$head" = "$want" ]
}

if have_tag "$dir"; then
  echo "$dir"
  exit 0
fi
legacy="$main/.toolchain/llvm-project"
if [ -z "${1:-}" ] && [ "$TAG" = "$DEFAULT_TAG" ] && have_tag "$legacy"; then
  echo "$legacy"
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
