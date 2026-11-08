#!/usr/bin/env bash
# grpc-tests.sh <list> <src>: run gRPC's test binaries (WP-811).
#
# gRPC's CMake build registers no ctest tests (upstream drives them with
# tools/run_tests), so the grpc row of projects.toml builds the binaries
# named in <list> (scripts/oss/grpc-tests.txt) and this script runs them
# from the build directory it is started in: at most 4 at once, each under
# `timeout 300`, with the gRPC source tree <src> as the working directory
# (the tests open src/core/tsi/test_creds/... relative to it). One line per
# test, `PASS name`, `FAIL name (rc N)` or `MISSING name` (not built), then
# a ctest-style `N% tests passed, F tests failed out of T` line, which is
# what report.json reads (spec/landed/harness/WP-666.md). A missing binary
# counts as a failure. Each test's output is in test-out/<name>.log.
#
# The end2end tests take their ports from gRPC's port server
# (tools/run_tests/python_utils/port_server.py, Python stdlib only) on
# localhost:32766; this script starts it first and stops it at exit.
set -u
LIST="$1" SRC="$(cd "$2" && pwd)" BIN="$(pwd)"
OUT="$BIN/test-out"
mkdir -p "$OUT" && find "$OUT" -mindepth 1 -delete
PORT_SERVER=32766
python3 "$SRC/tools/run_tests/python_utils/port_server.py" -p "$PORT_SERVER" \
  -l "$OUT/port_server.log" > /dev/null 2>&1 &
PS_PID=$!
trap 'kill "$PS_PID" 2> /dev/null' EXIT
for _ in $(seq 50); do
  python3 -c "import urllib.request as u; u.urlopen('http://localhost:$PORT_SERVER/version_number', timeout=1)" \
    2> /dev/null && break
  sleep 0.2
done
names=$(sed -e 's/#.*//' "$LIST" | tr -s ' \t' '\n\n' | grep -v '^$')
run_one() {
  n="$1"
  if [ ! -x "$BIN/$n" ]; then echo "MISSING $n"; return; fi
  (cd "$SRC" && timeout -s KILL 300 "$BIN/$n" > "$OUT/$n.log" 2>&1)
  rc=$?
  if [ "$rc" = 0 ]; then echo "PASS $n"; else echo "FAIL $n (rc $rc)"; fi
}
export -f run_one
export BIN SRC OUT
printf '%s\n' $names | xargs -P 4 -I{} bash -c 'run_one "$@"' _ {} | sort -k2 > "$OUT/summary.txt"
cat "$OUT/summary.txt"
t=$(wc -l < "$OUT/summary.txt")
p=$(grep -c '^PASS ' "$OUT/summary.txt")
f=$((t - p))
pct=0; [ "$t" -gt 0 ] && pct=$((100 * p / t))
echo
echo "$pct% tests passed, $f tests failed out of $t"
[ "$f" = 0 ]
