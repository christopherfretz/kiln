# source this before running cargo.
# Finds a workspace-local Rust toolchain if one exists (./.toolchain in this
# checkout, or ../.toolchain when kiln is a submodule of a wrapper repo);
# otherwise the system rustup is used. Works from git worktrees.
_WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
_MAIN="$(cd "$(git -C "$_WS" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || echo "$_WS/.git")/.." && pwd)"
# A submodule's common dir is <super>/.git/modules/kiln, so climb to the superproject.
case "$_MAIN" in */.git/modules*) _MAIN="${_MAIN%%/.git/modules*}";; esac
for _cand in "$_WS/.toolchain" "$_MAIN/.toolchain" "$_MAIN/../.toolchain" "${KILN_TOOLCHAIN:-}"; do
  if [ -n "$_cand" ] && [ -d "$_cand/cargo/bin" ]; then
    export RUSTUP_HOME="$_cand/rustup" CARGO_HOME="$_cand/cargo"
    export PATH="$CARGO_HOME/bin:$PATH"
    break
  fi
done
# Build output in fast local disk, one dir per checkout so worktrees do not
# thrash each other. Override with CARGO_TARGET_DIR.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/var/tmp/kiln-target/$(basename "$_WS")}"
unset _WS _MAIN _cand
