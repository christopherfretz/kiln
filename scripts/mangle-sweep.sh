#!/usr/bin/env bash
# Mangling parity sweep against clang++-19 (spec/abi.md "Mangling sweep",
# docs/VALIDATION.md). Each TU (default: tests/mangle-sweep/*.cpp) is
# compiled to LLVM IR by kiln++ and by clang++-19 with the same flags,
# once per standard library; every `_Z` symbol either IR defines,
# declares or references must appear in both. Prints each mismatch as
#   <stdlib> <tu> kiln-only|clang-only <symbol>
# then one summary line, and exits 1 when any mismatch was found
# (2 when a compiler failed on a TU).
#
#   scripts/mangle-sweep.sh [--stdlib libc++|libstdc++|both] [--keep DIR] [TU...]
#
# KILN (default: $CARGO_TARGET_DIR/debug/kiln++) and CLANG (default:
# clang++-19) name the compilers.
set -u
here=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
stdlibs="libc++ libstdc++"
keep=""
tus=()
while [ $# -gt 0 ]; do
  case "$1" in
    --stdlib) shift; [ "$1" = both ] && stdlibs="libc++ libstdc++" || stdlibs="$1" ;;
    --keep) shift; keep="$1" ;;
    -h|--help) sed -n '2,15p' "$0"; exit 0 ;;
    *) tus+=("$1") ;;
  esac
  shift
done
[ ${#tus[@]} -eq 0 ] && tus=("$here"/tests/mangle-sweep/*.cpp)
kiln=${KILN:-${CARGO_TARGET_DIR:-$here/target}/debug/kiln++}
clang=${CLANG:-clang++-19}
work=${keep:-$(mktemp -d "${TMPDIR:-/tmp}/kiln-mangle-sweep.XXXXXX")}
mkdir -p "$work"
# clang 19 leaves __cpp_explicit_this_parameter undefined and kiln defines
# it (as clang 22 does); libstdc++ picks other overloads on it, so it is
# undefined for both.
flags=(-std=c++26 -O0 -S -emit-llvm -fno-exceptions -w -U__cpp_explicit_this_parameter)
# clang runs the always-inliner even at -O0, which would hide the names of
# `always_inline` functions (libstdc++ `make_format_args`) that kiln
# emits; the sweep compares the IR before any LLVM pass.
clang_flags=(-Xclang -disable-llvm-passes)
# The `$_N` closure names of lambdas with internal linkage are numbered in
# each compiler's own order and have no ABI: the number is dropped.
syms() {
  grep -oE '@"?_Z[A-Za-z0-9_.$]+' "$1" | sed -e 's/^@"\{0,1\}//' -e 's/\$_[0-9][0-9]*/$_N/g' | sort -u
}
total=0
failed=0
for lib in $stdlibs; do
  for tu in "${tus[@]}"; do
    base=$(basename "$tu" .cpp)
    k="$work/$base.$lib.kiln.ll"
    c="$work/$base.$lib.clang.ll"
    if ! "$clang" "${flags[@]}" "${clang_flags[@]}" -stdlib="$lib" -o "$c" "$tu" 2>"$c.err"; then
      echo "$lib $base clang-error (see $c.err)"; failed=1; continue
    fi
    if ! "$kiln" "${flags[@]}" -stdlib="$lib" -resource-dir "$here/resource" -o "$k" "$tu" 2>"$k.err"; then
      echo "$lib $base kiln-error (see $k.err)"; failed=1; continue
    fi
    syms "$k" > "$k.syms"
    syms "$c" > "$c.syms"
    while read -r s; do echo "$lib $base kiln-only $s"; total=$((total + 1)); done \
      < <(comm -23 "$k.syms" "$c.syms")
    while read -r s; do echo "$lib $base clang-only $s"; total=$((total + 1)); done \
      < <(comm -13 "$k.syms" "$c.syms")
  done
done
echo "mangle-sweep: $total mismatch(es) over ${#tus[@]} TU(s) x [$stdlibs]"
[ -z "$keep" ] && rm -r "$work"
[ $failed -ne 0 ] && exit 2
[ $total -ne 0 ] && exit 1
exit 0
