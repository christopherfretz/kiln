#!/usr/bin/env bash
# selftest.sh: checks the OSS harness scripts (spec/landed/harness/WP-666.md
# "Tests"). Runs kiln-cxx on a hello world with clang++-19 and with kiln++
# (KILN_OSS_CXX, else $CARGO_TARGET_DIR/harness/kiln++, else debug/kiln++;
# skipped when none is built), checks the JSONL fields, and runs
# oss-report.py on the two-cluster fixture under fixtures/, plain, with
# --perf (WP-792) and with --update, and the libc++ mode (WP-694): the
# wrapper's `-stdlib=libc++` and oss-report.py --stdlib libc++, and a CMake
# target_precompile_headers project (WP-1023). Prints one PASS/FAIL line
# per check and exits 1 on any FAIL. With a kiln++, it also builds
# fixtures/modules with CMake and Ninja (WP-982, cmake_modules);
# `selftest.sh --cmake-modules` runs only that build, with cmake >= 3.28,
# ninja and a kiln++ required (GitHub CI, WP-1034).
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIX="$HERE/fixtures"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/kiln-oss-selftest.XXXXXX")"
cleanup() { find "$TMP" -mindepth 1 -delete; rmdir "$TMP"; }
trap cleanup EXIT
FAILS=0
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1"; FAILS=$((FAILS + 1)); fi; }

KILN_CXX="${KILN_OSS_CXX:-}"
[ -z "$KILN_CXX" ] && [ -x "${CARGO_TARGET_DIR:-/nonexistent}/harness/kiln++" ] && KILN_CXX="$CARGO_TARGET_DIR/harness/kiln++"
[ -z "$KILN_CXX" ] && [ -x "${CARGO_TARGET_DIR:-/nonexistent}/debug/kiln++" ] && KILN_CXX="$CARGO_TARGET_DIR/debug/kiln++"

# C++20 modules through CMake (WP-982, spec/modules.md §11.9): the
# fixtures/modules project with kiln++ as CMAKE_CXX_COMPILER and Ninja.
# CMake finds the scanner by the same alias mechanism as an installed tree
# (`clang-scan-deps-22` beside kiln++, a link to the kiln binary); the
# check refuses a system clang-scan-deps. Then an incremental rebuild:
# touching the interface recompiles it, the implementation unit and the
# consumer, but not the partition. WP-984: the project builds with libc++,
# and its `stdapp` does `import std;` through CXX_MODULE_STD (CMake builds
# libc++'s std.cppm with kiln, from the libc++.modules.json kiln prints;
# cmake >= 3.30); touching the interface or stdapp's source never
# rebuilds the std module. XDG_CACHE_HOME stays under $M.
# Skipped without cmake >= 3.28 or ninja, except under --cmake-modules,
# which runs only this check and FAILs then (and without a kiln++); GitHub
# CI runs it so (WP-1034).
cmake_modules() {
  local M="$TMP/modules" v
  v="$(cmake --version 2>/dev/null | sed -n 's/^cmake version \([0-9]*\)\.\([0-9]*\).*/\1 \2/p')"
  if ! command -v ninja > /dev/null || [ -z "$v" ] || [ "$(echo "$v" | awk '{print ($1 > 3 || ($1 == 3 && $2 >= 28))}')" != 1 ]; then
    # --cmake-modules (GitHub CI, WP-1034): the tools are required.
    if [ -n "$ONLY_CMAKE_MODULES" ]; then check "cmake modules: cmake >= 3.28 and ninja on PATH" false; return; fi
    echo "SKIP cmake modules: needs cmake >= 3.28 and ninja"; return
  fi
  echo "cmake modules: $(cmake --version | head -1), ninja $(ninja --version), kiln++ $KILN_CXX"
  mkdir -p "$M" && cp -R "$FIX/modules" "$M/src"
  local std_ok=0
  [ "$(echo "$v" | awk '{print ($1 > 3 || ($1 == 3 && $2 >= 30))}')" = 1 ] && std_ok=1
  XDG_CACHE_HOME="$M/xdg" cmake -S "$M/src" -B "$M/b" -G Ninja -DCMAKE_CXX_COMPILER="$KILN_CXX" -DCMAKE_CXX_FLAGS=-stdlib=libc++ > "$M/configure.log" 2>&1
  check "cmake modules: configure" "[ $? -eq 0 ]"
  local scanner; scanner="$(sed -n 's/^CMAKE_CXX_COMPILER_CLANG_SCAN_DEPS:FILEPATH=//p' "$M/b/CMakeCache.txt" 2>/dev/null)"
  check "cmake modules: the scanner is kiln's alias" "[ -n '$scanner' ] && [ \"\$(readlink -f '$scanner')\" = \"\$(readlink -f '$KILN_CXX')\" ]"
  XDG_CACHE_HOME="$M/xdg" ninja -C "$M/b" -v > "$M/build.log" 2>&1
  check "cmake modules: build" "[ $? -eq 0 ]"
  check "cmake modules: app prints hello 42" "[ \"\$('$M/b/app')\" = 'hello 42' ]"
  if [ "$std_ok" = 1 ]; then
    check "cmake modules: CMake found import std support" "! grep -q 'no import std' '$M/configure.log'"
    check "cmake modules: std.cppm built by kiln" "grep -F -- '-c ' '$M/build.log' | grep -qF 'libc++/v1/std.cppm'"
    check "cmake modules: stdapp prints import std 6" "[ \"\$('$M/b/stdapp')\" = 'import std 6' ]"
  else
    echo "SKIP cmake modules: import std needs cmake >= 3.30"
  fi
  sleep 1 && touch "$M/src/greet.cppm"
  XDG_CACHE_HOME="$M/xdg" ninja -C "$M/b" -v > "$M/rebuild.log" 2>&1
  check "cmake modules: rebuild after touching the interface" "[ $? -eq 0 ]"
  local o
  for o in greet.dir/greet.cppm.o greet.dir/greet-impl.cpp.o app.dir/main.cpp.o; do
    check "cmake modules: touch greet.cppm recompiles $o" "grep -qF -- '-o CMakeFiles/$o -c ' '$M/rebuild.log'"
  done
  check "cmake modules: touch greet.cppm leaves the partition" "! grep -qF -- '-o CMakeFiles/greet.dir/greet-part.cppm.o -c ' '$M/rebuild.log'"
  check "cmake modules: touch greet.cppm leaves the std module" "! grep -qF 'libc++/v1/std' '$M/rebuild.log'"
  if [ "$std_ok" = 1 ]; then
    sleep 1 && touch "$M/src/stdmain.cpp"
    XDG_CACHE_HOME="$M/xdg" ninja -C "$M/b" -v > "$M/rebuild-std.log" 2>&1
    check "cmake modules: rebuild after touching stdmain.cpp" "[ $? -eq 0 ] && grep -qF -- '-o CMakeFiles/stdapp.dir/stdmain.cpp.o -c ' '$M/rebuild-std.log'"
    check "cmake modules: touch stdmain.cpp leaves the std module" "! grep -qF 'libc++/v1/std' '$M/rebuild-std.log'"
  fi
  check "cmake modules: then nothing to do" "ninja -C '$M/b' | grep -q 'no work to do'"
}
ONLY_CMAKE_MODULES=""
if [ "${1:-}" = --cmake-modules ]; then
  ONLY_CMAKE_MODULES=1
  if [ -n "$KILN_CXX" ]; then cmake_modules; else check "cmake modules: a kiln++ (KILN_OSS_CXX)" false; fi
  if [ "$FAILS" -gt 0 ]; then
    for l in configure build rebuild rebuild-std; do
      [ -f "$TMP/modules/$l.log" ] && { echo "--- $l.log (tail)"; tail -n 40 "$TMP/modules/$l.log"; }
    done
  fi
  echo "selftest: $FAILS failed"
  [ "$FAILS" -eq 0 ]; exit
elif [ $# -gt 0 ]; then
  echo "usage: selftest.sh [--cmake-modules]" >&2; exit 2
fi

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
# Inline asm `!srcloc` operands are normalized; other metadata is not.
for n in C D E; do mkdir -p "$SD/out/$n"; cp "$SD/out/A/0.ll" "$SD/out/A/1.ll" "$SD/out/$n/"; done
printf 'call void asm sideeffect "nop", ""(), !srcloc !90\n!90 = !{i64 100, i64 113}\n' >> "$SD/out/C/0.ll"
printf 'call void asm sideeffect "nop", ""(), !srcloc !90\n!90 = !{i64 239, i64 252}\n' >> "$SD/out/D/0.ll"
printf 'call void asm sideeffect "nop", ""(), !srcloc !90\n!90 = !{i64 100, i64 113}\n!91 = !{i64 7}\n' >> "$SD/out/E/0.ll"
check "stage-diff: inline asm srcloc offsets normalized" "python3 '$HERE/stage-diff.py' compare --out '$SD/out' C D | grep -q '^stage-diff: 2 identical, 0 differ'"
check "stage-diff: other metadata differences reported" "! python3 '$HERE/stage-diff.py' compare --out '$SD/out' C E >/dev/null && grep -q '^+!91' '$SD/out/diffs/0.diff'"

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

# Nested `[ requires ]`, conditional properties, name skip, subset and
# FORCE over a library whose CMakeLists.txt adds test/ itself (WP-812).
J2="$TMP/jam2"; mkdir -p "$J2/libs/dm2/test"; cp -r "$JB/cm" "$J2/cm"
printf 'cmake_minimum_required(VERSION 3.20)\nproject(Boost NONE)\nset(BUILD_TESTING ON)\nlist(APPEND CMAKE_MODULE_PATH "${CMAKE_SOURCE_DIR}/cm")\nadd_subdirectory(libs/dm2)\n' > "$J2/CMakeLists.txt"
printf 'add_library(boost_dm2 INTERFACE)\nadd_library(Boost::dm2 ALIAS boost_dm2)\nadd_subdirectory(test)\n' > "$J2/libs/dm2/CMakeLists.txt"
: > "$J2/libs/dm2/test/CMakeLists.txt"
printf '  [ run a.cpp : : : [ requires x y ] <define>T=1 <toolset>intel:<debug-symbols>off <target-os>linux:<define>L=1 : a_1 ]\n  [ run a.cpp : : : $(SEL) : a_select ]\n  [ run b.cpp ]\n  [ run c.cpp ]\n  [ run d.cpp ]\n  [ run lanczos.cpp ]\n  [ run e.cpp inst//inst : : : : e_i ]\n' > "$J2/libs/dm2/test/Jamfile"
mkdir -p "$J2/libs/dm2/test/inst"; for f in a b c d lanczos e inst/x; do : > "$J2/libs/dm2/test/$f.cpp"; done
j2() { cmake -S "$J2" -B "$J2/b$1" -G "Unix Makefiles" -DCMAKE_PROJECT_Boost_INCLUDE="$HERE/shims/boost-jamfile-tests.cmake" \
  -DKILN_BOOST_JAMFILE_LIBRARIES=dm2 "${@:2}" 2>&1 | sed -n 's/^-- T //p'; }
j2 1 -DKILN_BOOST_JAMFILE_FORCE_dm2=ON '-DKILN_BOOST_JAMFILE_SKIP_dm2=_select$' '-DKILN_BOOST_JAMFILE_SUBSET_dm2=3|lanczos' > "$J2/got.txt"
printf '%s\n' 'run dm2-a_1 [a.cpp] [Boost::dm2] [T=1;L=1]' 'run dm2-d [d.cpp] [Boost::dm2] []' 'run dm2-lanczos [lanczos.cpp] [Boost::dm2] []' > "$J2/want.txt"
check "boost shim: nested requires, conditionals, skip, subset, force" "diff -u '$J2/want.txt' '$J2/got.txt'"
check "boost shim: a Jam library target links a static library from its glob" \
  "[ \"\$(j2 3 -DKILN_BOOST_JAMFILE_FORCE_dm2=ON '-DKILN_BOOST_JAMFILE_SKIP_dm2=^[^e]' '-DKILN_BOOST_JAMFILE_LIBS_dm2=inst//inst=inst/*.cpp')\" = 'run dm2-e_i [e.cpp] [kiln_dm2_inst__inst;Boost::dm2] []' ]"
check "boost shim: a library with CMake tests is skipped without FORCE" "[ -z \"\$(j2 2)\" ]"

# A deterministic subset of a `tests` target's dependencies (WP-812).
J3="$TMP/sub"; mkdir -p "$J3"
printf 'cmake_minimum_required(VERSION 3.20)\nproject(Boost NONE)\nadd_custom_target(tests)\nforeach(t g_e g_a g_d other g_c g_b)\n  add_custom_target(${t})\n  add_dependencies(tests ${t})\nendforeach()\n' > "$J3/CMakeLists.txt"
cmake -S "$J3" -B "$J3/b" -G "Unix Makefiles" -DCMAKE_PROJECT_Boost_INCLUDE="$HERE/shims/boost-jamfile-tests.cmake" \
  '-DKILN_BOOST_SUBSET_TARGETS=^g_|3|_e$' > "$J3/conf.txt" 2>&1
printf '%s\n' g_b g_c > "$J3/want.txt"
check "boost shim: target subset (every N-th plus keep regex)" "diff -u '$J3/want.txt' '$J3/b/kiln-subset-excluded.txt' && grep -q 'kiln: subset /^g_/: 3 of 5 test targets kept' '$J3/conf.txt' && make -s -C '$J3/b' -n kiln_tests > /dev/null"

printf '#include <cstdio>\nint main() { std::puts("hello"); }\n' > "$TMP/hello.cpp"
printf 'int main() { return x; }\n' > "$TMP/bad.cpp"

# field <jsonl> <line-index> <python expr over r>
field() { python3 -c "import json,sys; r=[json.loads(l) for l in open(sys.argv[1])][int(sys.argv[2])]; sys.exit(0 if ($3) else 1)" "$1" "$2"; }


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
# libc++ mode (WP-694): `-stdlib=libc++` first, and the binary runs on libc++.
stdlib_one() { # <label> <compiler or empty>
  local label="$1" cxx="$2" log="$TMP/$1-libcxx.jsonl"
  (cd "$TMP" && KILN_OSS_CXX="$cxx" KILN_OSS_LOG="$log" KILN_OSS_STDLIB=libc++ "$HERE/kiln-cxx" -O1 hello.cpp -o "hello-$label-libcxx" >/dev/null 2>&1)
  check "$label: libc++ hello builds and runs" "[ \"\$('$TMP/hello-$label-libcxx')\" = hello ]"
  check "$label: libc++ argv starts with -fno-crash-diagnostics -stdlib=libc++" "field '$log' 0 'r[\"kind\"]==\"ok\" and r[\"argv\"][1:3]==[\"-fno-crash-diagnostics\",\"-stdlib=libc++\"]'"
  check "$label: libc++ binary needs libc++.so" "readelf -d '$TMP/hello-$label-libcxx' | grep -q 'libc++.so' && ! readelf -d '$TMP/hello-$label-libcxx' | grep -q 'libstdc++'"
}
run_one clang ""
check "clang: default compiler is clang++-19" "field '$TMP/clang.jsonl' 0 'r[\"argv\"][0]==\"clang++-19\"'"
stdlib_one clang ""
(cd "$TMP" && KILN_OSS_STDLIB=bogus "$HERE/kiln-cxx" --version >/dev/null 2>&1)
check "a bad KILN_OSS_STDLIB exits 2" "[ $? -eq 2 ]"
if [ -n "$KILN_CXX" ]; then
  echo "kiln++: $KILN_CXX"
  run_one kiln "$KILN_CXX"
  stdlib_one kiln "$KILN_CXX"
  (cd "$TMP" && KILN_OSS_CXX="$KILN_CXX" KILN_OSS_LOG="$TMP/oom.jsonl" KILN_OSS_VMEM_KB=30000 "$HERE/kiln-cxx" -c hello.cpp -o o.o >/dev/null 2>&1)
  check "kiln: 30 MB address-space cap is kind oom" "field '$TMP/oom.jsonl' 0 'r[\"kind\"]==\"oom\" and r[\"rc\"]!=0 and r[\"diag\"]!=\"\"'"
  cmake_modules
else
  echo "SKIP kiln: no kiln++ (set KILN_OSS_CXX or build --profile harness)"
fi
# CMake target_precompile_headers (WP-1023): CMake's Clang PCH flags
# (`-Xclang -emit-pch`, `-Xclang -include-pch`, `-Xclang -include`) over a
# prefix header with no include guard, two TUs, built and run; the build is
# timed with PCH on and with -DCMAKE_DISABLE_PRECOMPILE_HEADERS=ON plus
# `-include pch.hpp` (the same TUs, textually; PCH is
# for correctness, not speed, in kiln 1.0: TIME lines are informational).
PC="$TMP/pch"
mkdir -p "$PC"
printf 'cmake_minimum_required(VERSION 3.20)\nproject(pchdemo CXX)\nset(CMAKE_CXX_STANDARD 20)\nadd_executable(app main.cpp other.cpp)\ntarget_precompile_headers(app PRIVATE pch.hpp)\n' > "$PC/CMakeLists.txt"
printf '#include <algorithm>\n#include <map>\n#include <string>\n#include <vector>\nstruct Unguarded { int v; };\ninline int twice(int x) { return 2 * x; }\n#define PCH_GREETING "hello pch"\n' > "$PC/pch.hpp"
printf '#include <cstdio>\nint other();\nint main() { std::vector<int> v{3, 1, 2}; std::sort(v.begin(), v.end()); Unguarded u{twice(v[2])}; std::printf("%%s %%d\\n", PCH_GREETING, u.v + other()); }\n' > "$PC/main.cpp"
printf 'int other() { std::map<std::string, int> m{{"a", 1}}; return m["a"] + twice(0); }\n' > "$PC/other.cpp"
pch_one() { # <label> <compiler or empty>
  local label="$1" cxx="$2" b
  for mode in on off; do
    b="$PC/b-$label-$mode"
    local extra=()
    [ "$mode" = off ] && extra=(-DCMAKE_DISABLE_PRECOMPILE_HEADERS=ON "-DCMAKE_CXX_FLAGS=-include $PC/pch.hpp")
    KILN_OSS_CXX="$cxx" KILN_OSS_LOG="$TMP/pch-$label.jsonl" cmake -S "$PC" -B "$b" -G "Unix Makefiles" -DCMAKE_CXX_COMPILER="$HERE/kiln-cxx" "${extra[@]}" > "$b.configure.log" 2>&1
    local t0 t1
    t0=$(date +%s.%N)
    KILN_OSS_CXX="$cxx" KILN_OSS_LOG="$TMP/pch-$label.jsonl" make -C "$b" VERBOSE=1 > "$b.build.log" 2>&1
    t1=$(date +%s.%N)
    echo "TIME $label PCH $mode: $(python3 -c "print(f'{$t1 - $t0:.2f}')")s"
  done
  check "$label: CMake PCH build runs" "[ \"\$('$PC/b-$label-on/app')\" = 'hello pch 7' ]"
  check "$label: CMake PCH flags reached the compiler" "grep -q -- '-Xclang -emit-pch' '$PC/b-$label-on.build.log' && grep -q -- '-Xclang -include-pch' '$PC/b-$label-on.build.log' && [ -s '$PC/b-$label-on/CMakeFiles/app.dir/cmake_pch.hxx.pch' ]"
  check "$label: CMake build without PCH runs" "[ \"\$('$PC/b-$label-off/app')\" = 'hello pch 7' ] && ! grep -q -- '-include-pch' '$PC/b-$label-off.build.log'"
}
pch_one clang ""
if [ -n "$KILN_CXX" ]; then
  pch_one kiln "$KILN_CXX"
fi

(cd "$TMP" && KILN_OSS_LOG="$TMP/to.jsonl" KILN_OSS_TIMEOUT=0.01 "$HERE/kiln-cxx" -O2 -c hello.cpp -o t.o >/dev/null 2>&1)
check "timeout: rc 124 and kind timeout" "[ $? -eq 124 ] && field '$TMP/to.jsonl' 0 'r[\"kind\"]==\"timeout\"'"

# A lit summary (LLVM check-*) gives `tests` (WP-782).
printf 'Failed Tests (1):\n  LLVM :: a.ll\n\nTotal Discovered Tests: 10\n  Unsupported      : 2 (20.00%%)\n  Passed           : 6 (60.00%%)\n  Expectedly Failed: 1 (10.00%%)\n  Failed           : 1 (10.00%%)\nmake: *** [check-llvm] Error 1\n' > "$TMP/lit.log"
check "lit summary counts" "python3 -c 'import sys; sys.path.insert(0, sys.argv[1]); import projects; s, c = projects.test_summary(sys.argv[2]); sys.exit(0 if c == {\"passed\": 9, \"failed\": 1, \"total\": 10} else 1)' '$HERE' '$TMP/lit.log'"

# meson test's and automake's summaries give `tests` (WP-920).
printf 'Summary of Failures:\n\n 3/4 a FAIL 0.01s\n\nOk:                3   \nExpected Fail:     0   \nFail:              1   \nUnexpected Pass:   0   \nSkipped:           1   \nTimeout:           0   \n\nFull log written to x\n' > "$TMP/meson.log"
check "meson test summary counts" "python3 -c 'import sys; sys.path.insert(0, sys.argv[1]); import projects; s, c = projects.test_summary(sys.argv[2]); sys.exit(0 if c == {\"passed\": 4, \"failed\": 1, \"total\": 5} else 1)' '$HERE' '$TMP/meson.log'"
printf '# TOTAL: 3\n# PASS:  2\n# SKIP:  0\n# XFAIL: 0\n# FAIL:  1\n# XPASS: 0\n# ERROR: 0\nmake: x\n# TOTAL: 2\n# PASS:  1\n# SKIP:  1\n# XFAIL: 0\n# FAIL:  0\n# XPASS: 0\n# ERROR: 0\n' > "$TMP/automake.log"
check "automake summaries summed" "python3 -c 'import sys; sys.path.insert(0, sys.argv[1]); import projects; s, c = projects.test_summary(sys.argv[2]); sys.exit(0 if c == {\"passed\": 4, \"failed\": 1, \"total\": 5} else 1)' '$HERE' '$TMP/automake.log'"
printf '[a]\nurl="u"\ntag="t"\nbuild="autotools"\ncxx_std=17\ntest="true"\nwave=1\narchive=true\n' > "$TMP/ar.toml"
check "an archive row needs its sha256" "python3 -c 'import sys; sys.path.insert(0, sys.argv[1]); import projects; projects.load(sys.argv[2])' '$HERE' '$TMP/ar.toml' > '$TMP/ar.out' 2>&1; grep -q 'needs the tarball' '$TMP/ar.out'"
# kiln-cxx's meson --version shim (WP-920): only for a compiler whose
# banner lacks `clang`, and only with KILN_OSS_VERSION_SHIM.
printf '#!/bin/sh\nfor a; do :; done\ncase "$a" in --version) echo "kiln version 9.9.9";; -dumpversion) echo 22.1.8;; esac\n' > "$TMP/fakecxx" && chmod +x "$TMP/fakecxx"
check "version shim: a clang line first, then the banner" "[ \"\$(KILN_OSS_VERSION_SHIM=1 KILN_OSS_CXX='$TMP/fakecxx' '$HERE/kiln-cxx' --version | tr '\\n' '|')\" = 'clang-compatible 22.1.8 (kiln-cxx Meson shim, WP-920)|kiln version 9.9.9|' ]"
check "version shim: off without the variable" "[ \"\$(KILN_OSS_CXX='$TMP/fakecxx' '$HERE/kiln-cxx' --version)\" = 'kiln version 9.9.9' ]"
check "version shim: clang's banner untouched" "! KILN_OSS_VERSION_SHIM=1 KILN_OSS_CXX=clang++-19 '$HERE/kiln-cxx' --version | grep -q 'Meson shim'"

# oss-report.py on the fixture: two clusters, the 3x reject first.
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" demo > "$TMP/report.md"
check "report matches fixtures/demo.expected.md" "diff -u '$FIX/demo.expected.md' '$TMP/report.md'"
cp "$FIX/status.md" "$TMP/status.md"
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --update "$TMP/status.md" demo >/dev/null 2>&1
check "--update rewrites the section" "grep -q 'Failure clusters (kiln): 2.' '$TMP/status.md' && ! grep -q 'stale text' '$TMP/status.md' && grep -q 'trailing text' '$TMP/status.md'"
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --perf demo > "$TMP/perf.md"
check "--perf: geomean and the slow TU" "grep -q 'geomean kiln/clang-19 time \\*\\*2.00x\\*\\*, RSS \\*\\*1.50x\\*\\*' '$TMP/perf.md' && grep -q 'TUs over 1.2x on time or RSS: 1 of the 1' '$TMP/perf.md'"
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --perf --top 0 demo > "$TMP/perf0.md"
check "--perf --top 0: no TU rows, same totals" "grep -q 'geomean kiln/clang-19 time' '$TMP/perf0.md' && ! grep -q 'e.cc' '$TMP/perf0.md' && grep -q 'e.cc' '$TMP/perf.md'"
check "--update regenerates the projects table" "grep -q '| \`demo\` | 1 | \`v1.0.0\` | blocked (2 clusters) |' '$TMP/status.md' && grep -q '| \`other\` | 2 | \`v2.0.0\` | tests at parity |' '$TMP/status.md'"

# --stdlib libc++ reads the -libcxx builds into their own block and the
# projects table's libc++ column, keeping the libstdc++ status (WP-694).
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --stdlib libc++ demo > "$TMP/libcxx.md"
check "--stdlib libc++: the libc++ section" "grep -q '^### demo (v1.0.0), libc++\$' '$TMP/libcxx.md' && grep -q '| tests passed / total | 3 / 4 | 4 / 4 |' '$TMP/libcxx.md'"
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --stdlib libc++ --update "$TMP/status.md" demo >/dev/null 2>&1
check "--stdlib libc++ --update: libc++ column and block" "grep -q '| \`demo\` | 1 | \`v1.0.0\` | blocked (2 clusters) | blocked (tests 3/4 vs clang 4/4) |' '$TMP/status.md' && grep -q '| \`other\` | 2 | \`v2.0.0\` | tests at parity | not run |' '$TMP/status.md' && grep -q '<!-- oss:demo:libcxx -->' '$TMP/status.md' && grep -q 'Failure clusters (kiln): 2.' '$TMP/status.md'"
printf '# page\n' > "$TMP/page.md"
python3 "$HERE/oss-report.py" --root "$FIX/oss" --toml "$FIX/projects.toml" --stdlib libc++ --update "$TMP/page.md" demo >/dev/null 2>&1
check "--update adds no projects table to a page without one" "grep -q '<!-- oss:demo:libcxx -->' '$TMP/page.md' && ! grep -q 'oss:projects' '$TMP/page.md'"

echo "selftest: $FAILS failed"
[ "$FAILS" -eq 0 ]
