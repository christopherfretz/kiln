#!/usr/bin/env bash
# selftest.sh: checks the OSS harness scripts (spec/landed/harness/WP-666.md
# "Tests"). Runs kiln-cxx on a hello world with clang++-19 and with kiln++
# (KILN_OSS_CXX, else $CARGO_TARGET_DIR/harness/kiln++, else debug/kiln++;
# skipped when none is built), checks the JSONL fields, and runs
# oss-report.py on the two-cluster fixture under fixtures/, plain, with
# --perf (WP-792) and with --update. Prints one PASS/FAIL line per check and
# exits 1 on any FAIL.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX="$HERE/fixtures"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/kiln-oss-selftest.XXXXXX")"
cleanup() { find "$TMP" -mindepth 1 -delete; rmdir "$TMP"; }
trap cleanup EXIT
FAILS=0
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1"; FAILS=$((FAILS + 1)); fi; }

for f in projects.py oss-report.py kiln-cxx stage-diff.py; do
  check "py_compile $f" "python3 -c 'import py_compile,sys; py_compile.compile(sys.argv[1], cfile=sys.argv[2], doraise=True)' '$HERE/$f' '$TMP/$f.pyc' >/dev/null 2>&1"
done
check "projects.toml loads" "python3 '$HERE/projects.py' list >/dev/null"
check "oss-build.sh parses" "bash -n '$HERE/oss-build.sh'"
# `checkout`: defaults to the project name; rows sharing one must agree on
# url/tag/submodules (WP-787).
ck() { printf '[a]\nurl="u"\ntag="t"\nbuild="cmake"\ncxx_std=20\ntest="true"\nwave=4\ncheckout="s"\n[b]\nurl="u"\ntag="%s"\nbuild="cmake"\ncxx_std=20\ntest="true"\nwave=4\n%s\n' "$1" "$2" > "$TMP/ck.toml"; python3 -c "import sys; sys.path.insert(0, sys.argv[1]); import projects; d = projects.load(sys.argv[2]); print(d['a']['checkout'], d['b']['checkout'])" "$HERE" "$TMP/ck.toml" 2>&1; }
check "checkout defaults to the project name" "[ \"\$(ck t '')\" = 's b' ]"
check "a shared checkout is accepted" "[ \"\$(ck t 'checkout=\"s\"')\" = 's s' ]"
check "a shared checkout at another tag is refused" "case \"\$(ck t2 'checkout=\"s\"')\" in *\"shares checkout 's'\"*) true ;; *) false ;; esac"

# A bootstrap stage (WP-815): `stage_of` must name another unstaged cmake
# row, and oss-build.sh takes that row's built compilers (`--which`).
st() { printf '[a]\nurl="u"\ntag="t"\nbuild="cmake"\ncxx_std=20\ntest="true"\nwave=4\n%s\n[b]\nurl="u"\ntag="t"\nbuild="cmake"\ncxx_std=20\ntest="true"\nwave=4\nstage_of="%s"\n' "$1" "$2" > "$TMP/st.toml"; python3 -c "import sys; sys.path.insert(0, sys.argv[1]); import projects; d = projects.load(sys.argv[2]); print(d['b']['stage_of'], d['b']['stage_cxx'], d['b']['stage_cc'])" "$HERE" "$TMP/st.toml" 2>&1; }
check "stage_of names a row; stage_cxx/stage_cc default" "[ \"\$(st '' a)\" = 'a bin/clang++ bin/clang' ]"
check "stage_of of itself, an unknown row or a staged row is refused" "case \"\$(st '' b)\$(st '' z)\$(st 'stage_of=\"b\"' a)\" in *'stage_of must'*'stage_of must'*'stage_of must'*) true ;; *) false ;; esac"
SO="$TMP/stage-oss"; mkdir -p "$SO/build/llvm_clang/kiln/bin"
ln -s "$(command -v clang++-19)" "$SO/build/llvm_clang/kiln/bin/clang++"; ln -s "$(command -v clang-19)" "$SO/build/llvm_clang/kiln/bin/clang"
check "staged row: --which takes the stage row's compilers" "[ \"\$(KILN_OSS_ROOT='$SO' KILN_OSS_CXX=/nonexistent '$HERE/oss-build.sh' llvm_selfhost --which 2>/dev/null | tr '\n' ' ')\" = 'CXX=$SO/build/llvm_clang/kiln/bin/clang++ CC=$SO/build/llvm_clang/kiln/bin/clang ' ]"
KILN_OSS_ROOT="$SO" "$HERE/oss-build.sh" llvm_selfhost --compiler clang --which > /dev/null 2> "$TMP/stage.err"
check "staged row: an unbuilt stage exits 2 and says so" "[ $? -eq 2 ] && grep -q 'which is not built' '$TMP/stage.err'"
# stage-diff.py (WP-815): two TUs under two spellings of clang 19.
SD="$TMP/sd"; mkdir -p "$SD"
printf 'int f(int x) { return x * 3 + 1; }\n' > "$SD/a.cpp"; printf 'struct S { int v; int g() const { return v << 2; } };\nint h(S s) { return s.g(); }\n' > "$SD/b.cpp"
for s in a b; do printf '{"phase":"build","kind":"ok","source":"%s.cpp","cwd":"%s","argv":["kiln++","-O3","-MD","-MT","x","-MF","x.d","-c","%s.cpp","-o","%s.o"]}\n' "$s" "$SD" "$s" "$s"; done > "$SD/c.jsonl"
python3 "$HERE/stage-diff.py" make --log "$SD/c.jsonl" --sample 200 --out "$SD/out" "A=$(command -v clang++-19)" "B=$(readlink -f "$(command -v clang++-19)")" >/dev/null && make -s -k -f "$SD/out/Makefile" >/dev/null 2>&1
check "stage-diff: two TUs identical under two clang 19 spellings" "python3 '$HERE/stage-diff.py' compare --out '$SD/out' A B | grep -q '^stage-diff: 2 identical, 0 differ, 0 missing, of 2'"

# The Boost Jamfile shim's library rules, variants, defines and links
# (WP-804): a stub superproject whose boost_test() prints what it is given.
JB="$TMP/jam"; mkdir -p "$JB/cm" "$JB/libs/demo/test/suite"
printf 'cmake_minimum_required(VERSION 3.20)\nproject(Boost NONE)\nset(BUILD_TESTING ON)\nlist(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cm")\nadd_subdirectory(libs/demo)\n' > "$JB/CMakeLists.txt"
printf 'add_library(boost_demo INTERFACE)\nadd_library(Boost::demo ALIAS boost_demo)\nadd_library(Boost::demo_fw ALIAS boost_demo)\n' > "$JB/libs/demo/CMakeLists.txt"
printf 'function(boost_test)\n  cmake_parse_arguments(_ "" "PREFIX;NAME;TYPE" "SOURCES;LINK_LIBRARIES;COMPILE_DEFINITIONS;INCLUDE_DIRECTORIES" ${ARGN})\n  list(TRANSFORM __SOURCES REPLACE "^.*/test/" "")\n  message(STATUS "T ${__TYPE} ${__PREFIX}-${__NAME} [${__SOURCES}] [${__LINK_LIBRARIES}] [${__COMPILE_DEFINITIONS}]")\nendfunction()\n' > "$JB/cm/BoostTest.cmake"
printf 'run plain.cpp ;\n  [ my-run2 named.cpp ../build//boost_demo : named_t ]\n  [ my-self run-fail : suite : case-one ]\n  [ my-files ser : A : : [ requires x ] ]\n  [ my-files gone ]\n' > "$JB/libs/demo/test/Jamfile"
for f in plain named ser A suite/case-one; do : > "$JB/libs/demo/test/$f.cpp"; done
cmake -S "$JB" -B "$JB/b" -G "Unix Makefiles" -DCMAKE_PROJECT_Boost_INCLUDE="$HERE/shims/boost-jamfile-tests.cmake" \
  -DKILN_BOOST_JAMFILE_LIBRARIES=demo "-DKILN_BOOST_JAMFILE_RULES_demo=my-run2=run|{1}|0;my-self={0}|{2}|{1}/{2}.cpp;my-files=run|{0}|0,1" \
  "-DKILN_BOOST_JAMFILE_VARIANTS_demo=my-files|ARCH=a|_a;my-files|ARCH=b|_b" -DKILN_BOOST_JAMFILE_DEFINES_demo=PD \
  -DKILN_BOOST_JAMFILE_LINK_demo=Boost::other -DKILN_BOOST_JAMFILE_TARGET_demo=Boost::demo_fw 2>&1 | sed -n 's/^-- T //p' > "$JB/got.txt"
printf '%s\n' 'run demo-plain [plain.cpp] [Boost::demo_fw;Boost::other] [PD]' 'run demo-named_t [named.cpp] [Boost::demo_fw;Boost::other] [PD]' \
  'run-fail demo-case_one [suite/case-one.cpp] [Boost::demo_fw;Boost::other] [PD]' 'run demo-ser_a [ser.cpp;A.cpp] [Boost::demo_fw;Boost::other] [ARCH=a;PD]' \
  'run demo-ser_b [ser.cpp;A.cpp] [Boost::demo_fw;Boost::other] [ARCH=b;PD]' > "$JB/want.txt"
check "boost shim: library rules, variants, defines and links" "diff -u '$JB/want.txt' '$JB/got.txt'"

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
  check "$label: ok line" "field '$log' 0 'r[\"kind\"]==\"ok\" and r[\"rc\"]==0 and r[\"source\"]==\"hello.cpp\" and r[\"output\"]==\"hello-$label\" and r[\"phase\"]==\"build\" and r[\"cwd\"]==\"$TMP\" and r[\"max_rss_kb\"]>0 and r[\"wall_s\"]>0 and r[\"cpu_s\"]>=0 and r[\"diag\"]==\"\" and r[\"v\"]==1'"
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
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --perf demo > "$TMP/perf.md"
check "--perf: geomean and the slow TU" "grep -q 'geomean kiln/clang-19 time \\*\\*2.00x\\*\\*, RSS \\*\\*1.50x\\*\\*' '$TMP/perf.md' && grep -q 'TUs over 1.2x on time or RSS: 1 of the 1' '$TMP/perf.md'"
check "--update regenerates the projects table" "grep -q '| \`demo\` | 1 | \`v1.0.0\` | blocked (2 clusters) |' '$TMP/status.md' && grep -q '| \`other\` | 2 | \`v2.0.0\` | tests at parity |' '$TMP/status.md'"

echo "selftest: $FAILS failed"
[ "$FAILS" -eq 0 ]
