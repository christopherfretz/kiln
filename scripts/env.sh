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
# Build output: ONE shared target dir for every checkout/worktree, inside the
# workspace (operator rule: keep build state in the workspace, not in the
# container's overlay — per-worktree dirs in /var/tmp once grew to 37 GB).
# Sharing means unchanged crates are compiled once for all worktrees; cargo
# serializes concurrent builds with its lock. Override with CARGO_TARGET_DIR.
_TGT="${KILN_BUILD_ROOT:-$_MAIN/.build}/target"
case "$_MAIN" in /Volumes/AgentWork/kiln-helper/kiln) _TGT="/Volumes/AgentWork/kiln-helper/.build/target";; esac
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$_TGT}"
unset _TGT
unset _WS _MAIN _cand
