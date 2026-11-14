#!/usr/bin/env bash
# The PCH snapshot differential over the bench/pch prefixes
# (docs/status/pch-snapshot-gates.md). It runs the default path: since
# WP-1404 a PCH build writes the snapshot and a consumer restores it with
# no PCH option at all.
#
#   scripts/pch-snapshot-diff.sh <kiln> <out-dir> [case...]
#
# Each case under scripts/pch-snapshot-diff/<case>/ is a prefix header
# (`prefix.h`, built into a PCH with the case's flags and no PCH option,
# as a user would), a main file (`main.cpp`, or `main.c` for a C case)
# and the compile's flags (`flags`, one per line; `@GCC16@` is
# $KILN_GCC16, else .toolchain/gcc-16.1.0-image/usr/local in the checkout
# or beside it, as scripts/env.sh finds .toolchain). The main file is
# compiled three ways, `plain` (-include prefix.h, the judge), `replay`
# (-include-pch -fno-kiln-pch-snapshot) and `snapshot` (-include-pch, the
# default), and snapshot is compared with replay (and replay with plain)
# on: the exit status, the diagnostics (snapshot vs replay: the
# consumer's only, pch-snapshot-diff/consumer-only.awk), -fdump-ast of
# the main file's declarations, the IR at -O0 and with -g, and the -MD
# list. Every output stays in <out-dir>/<case>/ for the classification.
#
# The producer's shape (WP-1404): it runs with KILN_SNAP_TIMING=1 and
# without -v (which would rule out the single pass), so its
# `snapshot-timing: producer single pass:` line says whether the one pass
# stood in for the header pass (`single`) or the header took two
# (`two`). A case's `expect-shape` (one word, `single` or `two`) is
# checked against it; `producer-env` (one VAR=value per line) is put in
# the producer's environment (`KILN_PCH_SNAPSHOT_PASSES=2` forces two).
#
# WP-1403, a consumer whose search path differs: a case may also have
# `consumer-flags` (one per line, put before `flags` in the three
# compiles of the main file, not in the producer's), a `files/` tree
# copied beside the prefix (the directories those flags name), and
# `expect-replay` (one line: the reason the snapshot must be refused
# for, as the -Wpch-snapshot-unused warning gives it; the snapshot then
# replays, and the warning is the one line its diagnostics may add).
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
  cases=(gnu mid lcx meta diag diag-once c17 gnu23 oven oven-limits meta-oven open-push-macro open-diag-push
    open-pack open-visibility open-once open-fp-pragma open-weak open-weak-alias
    open-extname attr-region search-extra search-shadow search-has-include mid-two-pass)
fi
status=0
for c in "${cases[@]}"; do
  src=$here/pch-snapshot-diff/$c
  d=$out/$c
  mkdir -p "$d"
  # A C case has `main.c` (its header is then `-x c-header`).
  if [ -e "$src/main.c" ]; then mainf=main.c hx=c-header; else mainf=main.cpp hx=c++-header; fi
  cp "$src/prefix.h" "$src/$mainf" "$d/"
  [ -d "$src/files" ] && cp -R "$src/files/." "$d/"
  flags=()
  while IFS= read -r f; do
    [ -n "$f" ] && flags+=("${f//@GCC16@/$gcc16}")
  done <"$src/flags"
  cflags=()
  if [ -e "$src/consumer-flags" ]; then
    while IFS= read -r f; do
      [ -n "$f" ] && cflags+=("${f//@GCC16@/$gcc16}")
    done <"$src/consumer-flags"
  fi
  expect=
  [ -e "$src/expect-replay" ] && expect=$(cat "$src/expect-replay")
  penv=()
  if [ -e "$src/producer-env" ]; then
    while IFS= read -r f; do
      [ -n "$f" ] && penv+=("$f")
    done <"$src/producer-env"
  fi
  (
    cd "$d" || exit 1
    env KILN_SNAP_TIMING=1 ${penv[@]+"${penv[@]}"} \
      "$kiln" "${flags[@]}" -x "$hx" prefix.h -o prefix.pch >produce.out 2>produce.err
    echo $? >produce.rc
    for v in plain replay snapshot; do
      case $v in
        plain) pre=(-include prefix.h) ;;
        replay) pre=(-include-pch prefix.pch -fno-kiln-pch-snapshot) ;;
        snapshot) pre=(-include-pch prefix.pch) ;;
      esac
      rm -f $v.ll $v.g.ll $v.d
      env KILN_SNAP_TIMING=$([ $v = snapshot ] && echo 1 || echo 0) \
        "$kiln" ${cflags[@]+"${cflags[@]}"} "${flags[@]}" "${pre[@]}" -S -emit-llvm "$mainf" -o $v.ll \
        -MD -MF $v.d -MT main.o 2>$v.err
      echo $? >$v.rc
      "$kiln" ${cflags[@]+"${cflags[@]}"} "${flags[@]}" "${pre[@]}" -g -S -emit-llvm "$mainf" -o $v.g.ll 2>/dev/null
      "$kiln" ${cflags[@]+"${cflags[@]}"} "${flags[@]}" "${pre[@]}" -fsyntax-only -fdump-ast=locs "$mainf" \
        >$v.ast 2>/dev/null
      # The warning of a refused snapshot (WP-1403) is checked below,
      # and the summary line it adds to.
      if [ -n "$expect" ]; then
        grep -v -e '^snapshot-timing: ' -e '\[-Wpch-snapshot-unused\]$' -e ' generated\.$' $v.err >$v.diag
      else
        grep -v -e '^snapshot-timing: ' $v.err >$v.diag
      fi
      grep -e '\[-Wpch-snapshot-unused\]$' $v.err >$v.unused
    done
    # The snapshot does not show the header's own diagnostics again
    # (clang's PCH policy); replay does. `replay.cdiag` is replay's
    # without them, which the snapshot must equal.
    n=$(sed -n 's/^snapshot-timing: the header reported \([0-9]*\) diagnostic.*/\1/p' snapshot.err)
    awk -v n="${n:-0}" -f "$here/pch-snapshot-diff/consumer-only.awk" replay.diag >replay.cdiag
  )
  verdict() {
    local a=$1 b=$2 what=$3 diffs=()
    for x in rc diag ast ll g.ll d; do
      # A failed compile writes no IR on either side.
      local bx=$x
      # Snapshot vs replay: consumer-only diagnostics.
      [ "$a" = snapshot ] && [ "$x" = diag ] && bx=cdiag
      if [ -e "$d/$a.$x" ] || [ -e "$d/$b.$bx" ]; then
        cmp -s "$d/$a.$x" "$d/$b.$bx" || diffs+=("$x")
      fi
    done
    if [ ${#diffs[@]} -eq 0 ]; then
      echo "$c: $what: same"
    else
      echo "$c: $what: DIFFERENT: ${diffs[*]}"
      [ "$a" = snapshot ] && status=1
    fi
  }
  wrote=$(sed -n 's/^snapshot-timing: note: \(written\|not written.*\)/snapshot \1/p' "$d/produce.err")
  shape=$(sed -n 's/^snapshot-timing: producer single pass: //p' "$d/produce.err")
  case $shape in
    "stands in"*) got=single ;;
    "") got=none ;;
    *) got=two ;;
  esac
  echo "$c: producer exit $(cat "$d/produce.rc"); ${wrote:-no snapshot note}; shape $got${shape:+ ($shape)}"
  if [ -e "$src/expect-shape" ] && [ "$(cat "$src/expect-shape")" != "$got" ]; then
    echo "$c: producer took shape $got, expected $(cat "$src/expect-shape")"
    status=1
  fi
  if [ "$got" = single ] && grep -q '^warning: \|: warning: ' "$d/produce.err"; then
    echo "$c: a single-pass producer printed a warning"
    status=1
  fi
  if [ -n "$expect" ]; then
    want="warning: precompiled header 'prefix.pch' was built with a different configuration ($expect); the header is processed again [-Wpch-snapshot-unused]"
    if grep -q '^snapshot-timing: restore ' "$d/snapshot.err"; then
      echo "$c: snapshot restored, expected a replay ($expect)"
      status=1
    elif [ "$(cat "$d/snapshot.unused")" = "$want" ]; then
      echo "$c: snapshot replayed as expected ($expect)"
    else
      echo "$c: snapshot replayed without the expected warning: $(cat "$d/snapshot.unused")"
      status=1
    fi
  elif grep -q '^snapshot-timing: restore ' "$d/snapshot.err"; then
    if [ ${#cflags[@]} -gt 0 ] && ! grep -q '^snapshot-timing: find .*lookups repeated' "$d/snapshot.err"; then
      echo "$c: snapshot restored without repeating its lookups"
      status=1
    fi
    echo "$c: snapshot restored"
  else
    echo "$c: snapshot NOT restored (replayed)"
    status=1
  fi
  verdict snapshot replay "snapshot vs replay"
  verdict replay plain "replay vs plain (reference)"
done
exit $status
