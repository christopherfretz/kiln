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
# Build output: inside the workspace (operator rule: keep build state in the
# workspace, not in the container's overlay), ONE target dir PER CHECKOUT
# (`.build/target-<checkout basename>`: the main checkout `kiln` gets
# `target-kiln`, the worktree `.worktrees/wp-058` gets `target-wp-058`).
# A target dir shared between worktrees is unsound: cargo hashes workspace
# members by their path relative to the workspace root, so two checkouts
# produce the same artifact names and overwrite each other's rlibs and
# uplifted binaries mid-build ("can't find crate", a kiln binary from
# another branch). Delete a worktree's dir when the worktree is removed.
# Override with CARGO_TARGET_DIR.
_TGT="${KILN_BUILD_ROOT:-$_MAIN/.build}/target-$(basename "$_WS")"
case "$_MAIN" in /Volumes/AgentWork/kiln-helper/kiln) _TGT="/Volumes/AgentWork/kiln-helper/.build/target-$(basename "$_WS")";; esac
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$_TGT}"
unset _TGT
unset _WS _MAIN _cand
