#!/usr/bin/env bash
# The PCH snapshot part of the release gate (WP-1404; docs/VALIDATION.md
# "Release candidates"). The snapshot PCH is kiln's default since 1.2.0,
# so every release candidate re-runs the two sweeps that made it the
# default (docs/status/pch-snapshot-gates.md):
#
#   1. the whole corpus on the default path with the synthetic prefix:
#      cargo xtask test --compiler <kiln++> --pch-diff snapshot
#        --pch-diff-require-restore --pch-diff-synthetic
#      (pass: 0 failed, every replay on tests/pch-snapshot-replay-allow.txt);
#   2. the libc++ pair: cargo xtask libcxx --compiler <kiln++> --no-fail
#      -j 4 over six directories, without and with --pch-diff snapshot,
#      with --results; pass: every test has the same outcome in both.
#
# Usage: scripts/pch-release-gate.sh <kiln++> <out-dir> [corpus|libcxx|all]
#
# <kiln++> is the release candidate's binary (a `harness` or `release`
# build). <out-dir> gets the logs and the two results files; it must not
# exist yet. The corpus sweep needs about 50 GB of scratch, so it does not
# start with less than 90 GB free on the file system of the cargo target
# directory (override the limit with KILN_PCH_GATE_MIN_FREE_GB; it exists
# for a larger or smaller corpus, not to skip the check). Exit status: 0
# pass, 1 a sweep failed or the pair differs, 2 usage or precondition.
set -euo pipefail

usage() { echo "usage: $0 <kiln++> <out-dir> [corpus|libcxx|all]" >&2; exit 2; }
[[ $# -ge 2 && $# -le 3 ]] || usage
kiln=$1
out=$2
what=${3:-all}
case "$what" in corpus|libcxx|all) ;; *) usage ;; esac
[[ -x "$kiln" ]] || { echo "pch-release-gate: '$kiln' is not an executable" >&2; exit 2; }
[[ -e "$out" ]] && { echo "pch-release-gate: '$out' exists" >&2; exit 2; }
kiln=$(cd "$(dirname "$kiln")" && pwd)/$(basename "$kiln")
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
mkdir -p "$out"
out=$(cd "$out" && pwd)

dirs=(std/utilities std/containers std/algorithms std/ranges std/strings std/language.support)
rc=0

if [[ $what != libcxx ]]; then
  min=${KILN_PCH_GATE_MIN_FREE_GB:-90}
  tgt=${CARGO_TARGET_DIR:-$root/target}
  mkdir -p "$tgt"
  free=$(df -Pk "$tgt" | awk 'NR == 2 { print int($4 / 1048576) }')
  if (( free < min )); then
    echo "pch-release-gate: ${free} GB free under $tgt, the corpus sweep needs ${min} GB" >&2
    exit 2
  fi
  echo "== corpus: --pch-diff snapshot --pch-diff-require-restore --pch-diff-synthetic (${free} GB free)"
  set +e
  cargo xtask test --compiler "$kiln" --pch-diff snapshot --pch-diff-require-restore \
    --pch-diff-synthetic > "$out/corpus.log" 2>&1
  status=$?
  set -e
  grep -E '^(test result:|pch-diff snapshot)' "$out/corpus.log" || true
  if (( status != 0 )) || ! grep -q '^test result: ok\.' "$out/corpus.log"; then
    echo "pch-release-gate: corpus sweep FAILED (exit $status; $out/corpus.log)"
    rc=1
  fi
fi

if [[ $what != corpus ]]; then
  for side in plain snapshot; do
    extra=()
    [[ $side == snapshot ]] && extra=(--pch-diff snapshot)
    echo "== libc++: ${side}"
    set +e
    cargo xtask libcxx --compiler "$kiln" --no-fail -j 4 "${extra[@]}" \
      --results "$out/libcxx-$side.tsv" "${dirs[@]}" > "$out/libcxx-$side.log" 2>&1
    status=$?
    set -e
    if (( status != 0 )); then
      echo "pch-release-gate: libc++ $side run exited $status ($out/libcxx-$side.log)"
      rc=1
    fi
    grep -E '^pch-diff snapshot' "$out/libcxx-$side.log" || true
  done
  if [[ -s $out/libcxx-plain.tsv && -s $out/libcxx-snapshot.tsv ]]; then
    # Outcome and test of each line, header dropped; any difference fails.
    cut -f1,2 "$out/libcxx-plain.tsv" | grep -v '^#' | sort -k2 > "$out/libcxx-plain.outcomes"
    cut -f1,2 "$out/libcxx-snapshot.tsv" | grep -v '^#' | sort -k2 > "$out/libcxx-snapshot.outcomes"
    n=$(wc -l < "$out/libcxx-plain.outcomes")
    if diff "$out/libcxx-plain.outcomes" "$out/libcxx-snapshot.outcomes" > "$out/libcxx.diff"; then
      echo "libc++ pair: $n tests, 0 differences"
    else
      echo "libc++ pair: $n tests, $(grep -c '^[<>]' "$out/libcxx.diff") differing lines ($out/libcxx.diff)"
      rc=1
    fi
  else
    echo "pch-release-gate: a libc++ results file is missing"
    rc=1
  fi
fi

if (( rc == 0 )); then echo "pch-release-gate: pass"; else echo "pch-release-gate: FAIL"; fi
exit "$rc"
