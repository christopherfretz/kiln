#!/usr/bin/env bash
# selftest.sh: checks the OSS harness scripts (spec/landed/harness/WP-666.md
# "Tests"). Runs kiln-cxx on a hello world with clang++-19 and with kiln++
# (KILN_OSS_CXX, else $CARGO_TARGET_DIR/harness/kiln++, else debug/kiln++;
# skipped when none is built), checks the JSONL fields, and runs
# oss-report.py on the two-cluster fixture under fixtures/, plain and with
# --update. Prints one PASS/FAIL line per check and exits 1 on any FAIL.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX="$HERE/fixtures"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/kiln-oss-selftest.XXXXXX")"
cleanup() { find "$TMP" -mindepth 1 -delete; rmdir "$TMP"; }
trap cleanup EXIT
FAILS=0
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1"; FAILS=$((FAILS + 1)); fi; }

for f in projects.py oss-report.py kiln-cxx; do
  check "py_compile $f" "python3 -c 'import py_compile,sys; py_compile.compile(sys.argv[1], cfile=sys.argv[2], doraise=True)' '$HERE/$f' '$TMP/$f.pyc' >/dev/null 2>&1"
done
check "projects.toml loads" "python3 '$HERE/projects.py' list >/dev/null"
check "oss-build.sh parses" "bash -n '$HERE/oss-build.sh'"

printf '#include <cstdio>\nint main() { std::puts("hello"); }\n' > "$TMP/hello.cpp"
printf 'int main() { return x; }\n' > "$TMP/bad.cpp"

# field <jsonl> <line-index> <python expr over r>
field() { python3 -c "import json,sys; r=[json.loads(l) for l in open(sys.argv[1])][int(sys.argv[2])]; sys.exit(0 if ($3) else 1)" "$1" "$2"; }

KILN_CXX="${KILN_OSS_CXX:-}"
[ -z "$KILN_CXX" ] && [ -x "${CARGO_TARGET_DIR:-/nonexistent}/harness/kiln++" ] && KILN_CXX="$CARGO_TARGET_DIR/harness/kiln++"
[ -z "$KILN_CXX" ] && [ -x "${CARGO_TARGET_DIR:-/nonexistent}/debug/kiln++" ] && KILN_CXX="$CARGO_TARGET_DIR/debug/kiln++"

run_one() { # <label> <compiler or empty>
  local label="$1" cxx="$2" log="$TMP/$1.jsonl"
  (cd "$TMP" && KILN_OSS_CXX="$cxx" KILN_OSS_LOG="$log" KILN_OSS_PHASE=build "$HERE/kiln-cxx" -O1 hello.cpp -o "hello-$label" >/dev/null 2>&1)
  check "$label: hello builds and runs" "[ \"\$('$TMP/hello-$label')\" = hello ]"
  check "$label: ok line" "field '$log' 0 'r[\"kind\"]==\"ok\" and r[\"rc\"]==0 and r[\"source\"]==\"hello.cpp\" and r[\"output\"]==\"hello-$label\" and r[\"phase\"]==\"build\" and r[\"cwd\"]==\"$TMP\" and r[\"max_rss_kb\"]>0 and r[\"wall_s\"]>0 and r[\"diag\"]==\"\" and r[\"v\"]==1'"
  (cd "$TMP" && KILN_OSS_CXX="$cxx" KILN_OSS_LOG="$log" "$HERE/kiln-cxx" -c bad.cpp -o bad.o 2>"$TMP/$label.err")
  check "$label: reject exits 1" "[ $? -eq 1 ]"
  check "$label: stderr passes through" "grep -q \"undeclared identifier 'x'\" '$TMP/$label.err'"
  check "$label: reject line" "field '$log' 1 'r[\"kind\"]==\"reject\" and r[\"rc\"]==1 and r[\"source\"]==\"bad.cpp\" and \"error: use of undeclared identifier\" in r[\"diag\"]'"
  (cd "$TMP" && KILN_OSS_CXX="$cxx" KILN_OSS_LOG="$log" "$HERE/kiln-cxx" --version >/dev/null 2>&1)
  check "$label: query line" "field '$log' 2 'r[\"kind\"]==\"ok\" and r[\"source\"]==\"none\"'"
}
run_one clang ""
check "clang: default compiler is clang++-19" "field '$TMP/clang.jsonl' 0 'r[\"argv\"][0]==\"clang++-19\"'"
if [ -n "$KILN_CXX" ]; then
  echo "kiln++: $KILN_CXX"
  run_one kiln "$KILN_CXX"
  (cd "$TMP" && KILN_OSS_CXX="$KILN_CXX" KILN_OSS_LOG="$TMP/oom.jsonl" KILN_OSS_VMEM_KB=30000 "$HERE/kiln-cxx" -c hello.cpp -o o.o >/dev/null 2>&1)
  check "kiln: 30 MB address-space cap is kind oom" "field '$TMP/oom.jsonl' 0 'r[\"kind\"]==\"oom\" and r[\"rc\"]!=0 and r[\"diag\"]!=\"\"'"
else
  echo "SKIP kiln: no kiln++ (set KILN_OSS_CXX or build --profile harness)"
fi
(cd "$TMP" && KILN_OSS_LOG="$TMP/to.jsonl" KILN_OSS_TIMEOUT=0.01 "$HERE/kiln-cxx" -O2 -c hello.cpp -o t.o >/dev/null 2>&1)
check "timeout: rc 124 and kind timeout" "[ $? -eq 124 ] && field '$TMP/to.jsonl' 0 'r[\"kind\"]==\"timeout\"'"

# A lit summary (LLVM check-*) gives `tests` (WP-782).
printf 'Failed Tests (1):\n  LLVM :: a.ll\n\nTotal Discovered Tests: 10\n  Unsupported      : 2 (20.00%%)\n  Passed           : 6 (60.00%%)\n  Expectedly Failed: 1 (10.00%%)\n  Failed           : 1 (10.00%%)\nmake: *** [check-llvm] Error 1\n' > "$TMP/lit.log"
check "lit summary counts" "python3 -c 'import sys; sys.path.insert(0, sys.argv[1]); import projects; s, c = projects.test_summary(sys.argv[2]); sys.exit(0 if c == {\"passed\": 9, \"failed\": 1, \"total\": 10} else 1)' '$HERE' '$TMP/lit.log'"

# oss-report.py on the fixture: two clusters, the 3x reject first.
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" demo > "$TMP/report.md"
check "report matches fixtures/demo.expected.md" "diff -u '$FIX/demo.expected.md' '$TMP/report.md'"
cp "$FIX/status.md" "$TMP/status.md"
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --update "$TMP/status.md" demo >/dev/null 2>&1
check "--update rewrites the section" "grep -q 'Failure clusters (kiln): 2.' '$TMP/status.md' && ! grep -q 'stale text' '$TMP/status.md' && grep -q 'trailing text' '$TMP/status.md'"
check "--update regenerates the projects table" "grep -q '| \`demo\` | 1 | \`v1.0.0\` | blocked (2 clusters) |' '$TMP/status.md' && grep -q '| \`other\` | 2 | \`v2.0.0\` | tests at parity |' '$TMP/status.md'"

echo "selftest: $FAILS failed"
[ "$FAILS" -eq 0 ]
