#!/usr/bin/env bash
# EXPERIMENTAL (WP-1383): the PCH snapshot differential over the
# bench/pch prefixes (docs/status/pch-snapshot-prototype.md).
#
#   scripts/pch-snapshot-diff.sh <kiln> <out-dir> [case...]
#
# Each case under scripts/pch-snapshot-diff/<case>/ is a prefix header
# (`prefix.h`, built into a PCH with -fkiln-pch-snapshot=write), a main
# file (`main.cpp`) and the compile's flags (`flags`, one per line;
# `@GCC16@` is $KILN_GCC16, else .toolchain/gcc-16.1.0-image/usr/local in
# the checkout or beside it, as scripts/env.sh finds .toolchain). The main file is
# compiled three ways, `plain` (-include prefix.h, the judge), `replay`
# (-include-pch) and `snapshot` (-include-pch -fkiln-pch-snapshot=load),
# and snapshot is compared with replay (and replay with plain) on: the
# exit status, the diagnostics, -fdump-ast of the main file's
# declarations, the IR at -O0 and with -g, and the -MD list. Every
# output stays in <out-dir>/<case>/ for the classification.
set -u
if [ $# -lt 2 ]; then
  echo "usage: $0 <kiln> <out-dir> [case...]" >&2
  exit 2
fi
kiln=$(realpath "$1")
out=$2
shift 2
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
main=$(cd "$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)/.." && pwd)
case "$main" in */.git/modules*) main=${main%%/.git/modules*} ;; esac
gcc16=${KILN_GCC16:-}
for cand in "$root/.toolchain" "$main/.toolchain" "$main/../.toolchain"; do
  if [ -z "$gcc16" ] && [ -d "$cand/gcc-16.1.0-image/usr/local" ]; then
    gcc16=$cand/gcc-16.1.0-image/usr/local
  fi
done
cases=("$@")
if [ ${#cases[@]} -eq 0 ]; then
  cases=(gnu mid lcx meta diag)
fi
status=0
for c in "${cases[@]}"; do
  src=$here/pch-snapshot-diff/$c
  d=$out/$c
  mkdir -p "$d"
  cp "$src/prefix.h" "$src/main.cpp" "$d/"
  flags=()
  while IFS= read -r f; do
    [ -n "$f" ] && flags+=("${f//@GCC16@/$gcc16}")
  done <"$src/flags"
  (
    cd "$d" || exit 1
    "$kiln" "${flags[@]}" -fkiln-pch-snapshot=write -v -x c++-header prefix.h \
      -o prefix.pch >produce.out 2>produce.err
    echo $? >produce.rc
    for v in plain replay snapshot; do
      case $v in
        plain) pre=(-include prefix.h) ;;
        replay) pre=(-include-pch prefix.pch) ;;
        snapshot) pre=(-include-pch prefix.pch -fkiln-pch-snapshot=load) ;;
      esac
      rm -f $v.ll $v.g.ll $v.d
      env KILN_SNAP_TIMING=$([ $v = snapshot ] && echo 1 || echo 0) \
        "$kiln" "${flags[@]}" "${pre[@]}" -S -emit-llvm main.cpp -o $v.ll \
        -MD -MF $v.d -MT main.o 2>$v.err
      echo $? >$v.rc
      "$kiln" "${flags[@]}" "${pre[@]}" -g -S -emit-llvm main.cpp -o $v.g.ll 2>/dev/null
      "$kiln" "${flags[@]}" "${pre[@]}" -fsyntax-only -fdump-ast=locs main.cpp \
        >$v.ast 2>/dev/null
      grep -v -e '^snapshot-timing: ' $v.err >$v.diag
    done
  )
  verdict() {
    local a=$1 b=$2 what=$3 diffs=()
    for x in rc diag ast ll g.ll d; do
      # A failed compile writes no IR on either side.
      if [ -e "$d/$a.$x" ] || [ -e "$d/$b.$x" ]; then
        cmp -s "$d/$a.$x" "$d/$b.$x" || diffs+=("$x")
      fi
    done
    if [ ${#diffs[@]} -eq 0 ]; then
      echo "$c: $what: same"
    else
      echo "$c: $what: DIFFERENT: ${diffs[*]}"
      [ "$a" = snapshot ] && status=1
    fi
  }
  echo "$c: producer exit $(cat "$d/produce.rc"); $(grep -h "snapshot \(written\|not written\)" "$d/produce.err")"
  if grep -q '^snapshot-timing: restore ' "$d/snapshot.err"; then
    echo "$c: snapshot restored"
  else
    echo "$c: snapshot NOT restored (replayed)"
    status=1
  fi
  verdict snapshot replay "snapshot vs replay"
  verdict replay plain "replay vs plain (reference)"
done
exit $status
