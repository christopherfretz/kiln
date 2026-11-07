#!/usr/bin/env bash
# oss-build.sh <project> [--compiler kiln|clang] [--no-test] [--jobs N]
#        [--install PREFIX]
#
# Builds one project of scripts/oss/projects.toml through its own build
# system with scripts/oss/kiln-cxx as the C++ compiler, runs its tests, and
# writes .oss/build/<project>/<compiler>/report.json (schema:
# spec/landed/harness/WP-666.md). Not part of `cargo xtask ci` (needs the
# network). Source scripts/env.sh first so `make` draws from the shared
# jobserver; `--jobs` only applies when there is none. The project's `deps`
# are first built and `cmake --install`ed with the same compiler into
# .oss/<dep>-install-<compiler>/ (`--install PREFIX`, which uses the dep's
# `install_configure` and skips its tests), unless already installed.
#
# Environment: KILN_OSS_CXX (the kiln++ to test; default
# $CARGO_TARGET_DIR/harness/kiln++), KILN_OSS_ROOT (default <kiln>/.oss).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KILN="$(cd "$HERE/../.." && pwd)"
OSS="${KILN_OSS_ROOT:-$KILN/.oss}"
WRAP="$HERE/kiln-cxx"

usage() { sed -n '2,3p' "$0" | sed 's/^# //' >&2; exit 2; }
PROJECT="" COMPILER=kiln RUN_TEST=1 JOBS=4 INSTALL=""
while [ $# -gt 0 ]; do
  case "$1" in
    --compiler) COMPILER="${2:-}"; shift 2 ;;
    --compiler=*) COMPILER="${1#*=}"; shift ;;
    --no-test) RUN_TEST=""; shift ;;
    --jobs) JOBS="${2:-}"; shift 2 ;;
    --jobs=*) JOBS="${1#*=}"; shift ;;
    --install) INSTALL="${2:-}"; RUN_TEST=""; shift 2 ;;
    -h|--help) usage ;;
    -*) echo "oss-build: unknown option $1" >&2; usage ;;
    *) [ -z "$PROJECT" ] || usage; PROJECT="$1"; shift ;;
  esac
done
[ -n "$PROJECT" ] || usage
case "$COMPILER" in kiln|clang) ;; *) echo "oss-build: --compiler must be kiln or clang" >&2; exit 2 ;; esac
case "$JOBS" in ''|*[!0-9]*) echo "oss-build: --jobs takes a number" >&2; exit 2 ;; esac

CFG="$(python3 "$HERE/projects.py" shell "$PROJECT")" || exit 2
eval "$CFG"

if [ "$COMPILER" = kiln ]; then
  if [ -n "${KILN_OSS_CXX:-}" ]; then
    CXX="$KILN_OSS_CXX"; how='$KILN_OSS_CXX'
  elif [ -n "${CARGO_TARGET_DIR:-}" ] && [ -x "$CARGO_TARGET_DIR/harness/kiln++" ]; then
    CXX="$CARGO_TARGET_DIR/harness/kiln++"; how='$CARGO_TARGET_DIR/harness'
  else
    echo "oss-build: no kiln++: set KILN_OSS_CXX or run 'cargo build --profile harness -p kiln_driver' after sourcing scripts/env.sh" >&2
    exit 2
  fi
  export KILN_RESOURCE_DIR="${KILN_RESOURCE_DIR:-$KILN/resource}"
else
  # The control compiler: clang++-19 unless the toml names another
  # (`control_cxx`, for a project clang 19 cannot build; WP-792).
  CXX="$(command -v "$P_CONTROL_CXX")" || { echo "oss-build: $P_CONTROL_CXX not on PATH" >&2; exit 2; }
  how='PATH'
fi
CXX="$(cd "$(dirname "$CXX")" && pwd)/$(basename "$CXX")"
echo "oss-build: $PROJECT $P_TAG with $CXX (from $how)"
export KILN_OSS_CXX="$CXX"

# Dependencies: built with this compiler, installed once, found by cmake.
PREFIXES=()
for dep in "${P_DEPS[@]}"; do
  pre="$OSS/$dep-install-$COMPILER"
  if [ ! -f "$pre/.oss-installed" ]; then
    echo "oss-build: dependency $dep -> $pre"
    "$0" "$dep" --compiler "$COMPILER" --jobs "$JOBS" --install "$pre" || { echo "oss-build: dependency $dep failed" >&2; exit 1; }
  fi
  PREFIXES+=("$pre")
done
if [ -n "$INSTALL" ]; then
  [ "$P_BUILD" = cmake ] || { echo "oss-build: --install needs a cmake project" >&2; exit 2; }
  P_CONFIGURE=("${P_INSTALL_CONFIGURE[@]}" "-DCMAKE_INSTALL_PREFIX=$INSTALL")
  P_BUILD_TARGETS=()
fi
# `@OSS@` in a configure flag is this directory (a project shim, WP-726),
# and in the test command (a test-exclusion list, WP-803).
P_CONFIGURE=("${P_CONFIGURE[@]//@OSS@/$KILN/scripts/oss}")
P_TEST="${P_TEST//@OSS@/$KILN/scripts/oss}"
[ ${#PREFIXES[@]} -eq 0 ] || P_CONFIGURE+=("-DCMAKE_PREFIX_PATH=$(IFS=';'; echo "${PREFIXES[*]}")")

SRC="$OSS/src/$PROJECT"
BUILD="$OSS/build/$PROJECT/$COMPILER${INSTALL:+-dep}"
mkdir -p "$OSS/src" "$BUILD"
export KILN_OSS_LOG="$BUILD/compile.jsonl"
# The wrapper's address-space cap: the project's `vmem_kb` (default
# 3000000, at most 6000000) unless the caller set KILN_OSS_VMEM_KB.
export KILN_OSS_VMEM_KB="${KILN_OSS_VMEM_KB:-$P_VMEM_KB}"
: > "$KILN_OSS_LOG"
rm -f "$BUILD/report.json" "$BUILD/configure.log" "$BUILD/build.log" "$BUILD/test.log"

if [ -d "$SRC/.git" ]; then
  have="$(git -C "$SRC" describe --tags --exact-match 2>/dev/null || echo '?')"
  [ "$have" = "$P_TAG" ] || echo "oss-build: warning: $SRC is at '$have', the toml pins '$P_TAG' (delete the checkout to refetch)" >&2
else
  sub=()
  [ -n "$P_SUBMODULES" ] && sub=(--recurse-submodules --shallow-submodules)
  echo "oss-build: cloning $P_URL@$P_TAG"
  git -c advice.detachedHead=false clone -q --depth 1 --branch "$P_TAG" "${sub[@]}" "$P_URL" "$SRC" || { echo "oss-build: clone failed" >&2; exit 1; }
fi
SRCDIR="$SRC${P_SOURCE_SUBDIR:+/$P_SOURCE_SUBDIR}"

# make: inherit the shared jobserver from MAKEFLAGS; -j only without one.
JFLAG=()
case "${MAKEFLAGS:-}" in
  *--jobserver-auth=fifo:*)
    fifo="${MAKEFLAGS##*--jobserver-auth=fifo:}"; fifo="${fifo%% *}"
    [ -p "$fifo" ] || JFLAG=(-j "$JOBS") ;;
  *) JFLAG=(-j "$JOBS") ;;
esac

T0=$(date +%s.%N)
BUILD_RC="" TEST_RC=""
export KILN_OSS_PHASE=configure
case "$P_BUILD" in
  cmake)
    nice -n 10 cmake --fresh -S "$SRCDIR" -B "$BUILD" -G "Unix Makefiles" \
      -DCMAKE_CXX_COMPILER="$WRAP" -DCMAKE_C_COMPILER="$P_C_COMPILER" \
      -DCMAKE_CXX_STANDARD="$P_CXX_STD" -DCMAKE_BUILD_TYPE=Release \
      "${P_CONFIGURE[@]}" > "$BUILD/configure.log" 2>&1
    CONF_RC=$? ;;
  meson)
    # meson's only Linux backend is ninja, which is not a jobserver client:
    # bound it to --jobs (at most 4) instead.
    [ "$JOBS" -le 4 ] || JOBS=4
    CXX="$WRAP" CC="$P_C_COMPILER" nice -n 10 meson setup --wipe "$BUILD/meson" "$SRCDIR" \
      --buildtype=release -Dcpp_std="c++$P_CXX_STD" "${P_CONFIGURE[@]}" > "$BUILD/configure.log" 2>&1
    CONF_RC=$? ;;
  make)
    CONF_RC=0 ;;
esac
echo "oss-build: configure rc=$CONF_RC ($BUILD/configure.log)"

if [ "$CONF_RC" = 0 ]; then
  export KILN_OSS_PHASE=build
  case "$P_BUILD" in
    cmake)
      nice -n 10 make -C "$BUILD" clean > /dev/null 2>&1
      nice -n 10 make -k "${JFLAG[@]}" -C "$BUILD" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      BUILD_RC=$? ;;
    meson)
      nice -n 10 meson compile -C "$BUILD/meson" -j "$JOBS" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      BUILD_RC=$? ;;
    make)
      # In-source: the project's own Makefile, told which compilers to use.
      nice -n 10 make -k "${JFLAG[@]}" -C "$SRCDIR" CXX="$WRAP" CC="$P_C_COMPILER" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      BUILD_RC=$? ;;
  esac
  echo "oss-build: build rc=$BUILD_RC ($BUILD/build.log)"
  if [ -n "$INSTALL" ]; then
    [ "$BUILD_RC" = 0 ] && nice -n 10 cmake --install "$BUILD" >> "$BUILD/build.log" 2>&1 && date -u > "$INSTALL/.oss-installed" || BUILD_RC=1
    echo "oss-build: install into $INSTALL rc=$BUILD_RC"
  fi
  if [ -n "$RUN_TEST" ]; then
    export KILN_OSS_PHASE=test
    tdir="$BUILD"; [ "$P_BUILD" = meson ] && tdir="$BUILD/meson"; [ "$P_BUILD" = make ] && tdir="$SRCDIR"
    # Test scratch is per build tree: suites that default to a fixed path
    # under /tmp (leveldb's /tmp/leveldbtest-<uid>) must not collide when the
    # kiln and clang builds, or two checkouts, run their tests at once.
    TTMP="$BUILD/test-tmp"; mkdir -p "$TTMP" && find "$TTMP" -mindepth 1 -delete
    (cd "$tdir" && TMPDIR="$TTMP" TEST_TMPDIR="$TTMP" timeout "$P_TEST_TIMEOUT" nice -n 10 bash -c "$P_TEST") > "$BUILD/test.log" 2>&1
    TEST_RC=$?
    echo "oss-build: test rc=$TEST_RC ($BUILD/test.log)"
  fi
fi
WALL=$(python3 -c "print($(date +%s.%N) - $T0)")

python3 "$HERE/projects.py" report-json --project "$PROJECT" --compiler "$COMPILER" \
  --compiler-path "$CXX" --build-dir "$BUILD" --configure-rc "$CONF_RC" \
  ${BUILD_RC:+--build-rc "$BUILD_RC"} ${TEST_RC:+--test-rc "$TEST_RC"} --wall "$WALL"
[ -z "$INSTALL" ] || [ "$BUILD_RC" = 0 ]
