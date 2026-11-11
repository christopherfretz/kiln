#!/usr/bin/env bash
# oss-build.sh <project> [--compiler kiln|clang] [--stdlib libstdc++|libc++]
#        [--no-test] [--jobs N] [--install PREFIX] [--which]
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
# A row with `stage_of = "<row>"` (WP-815, a bootstrap stage) builds with
# compilers that row built: `.oss/build/<row>/<compiler>/<stage_cxx>` and
# `<stage_cc>` (default bin/clang++ and bin/clang), so `--compiler kiln`
# means "with the compiler kiln++ built" and `--compiler clang` "with the one
# clang-19 built" (with `--stdlib libc++`, the `<compiler>-libcxx` tree's).
# `--which` prints the resolved compilers and exits.
#
# `--stdlib libc++` (WP-694; default $KILN_OSS_STDLIB, else libstdc++)
# builds against the system libc++ 19 (the one clang++-19 uses): kiln-cxx
# puts `-stdlib=libc++` on every compile and link, and the build tree, the
# dependency installs and the report get a `-libcxx` suffix
# (.oss/build/<project>/<compiler>-libcxx/), so the libstdc++ baseline is
# kept.
#
# A row with `modules = true` (WP-1148, C++20 modules through CMake) is
# generated for Ninja, since CMake's Makefiles generator cannot scan, and
# CMake is told the scanner: kiln-scan-deps beside kiln++ (the kiln-cxx
# wrapper has no clang-scan-deps beside it, so CMake would otherwise take a
# system clang-scan-deps-22 for kiln), or clang-scan-deps-<N> beside the
# control clang++-<N>. The scanner is run directly, not through kiln-cxx,
# so `--stdlib libc++` also goes into CMAKE_CXX_FLAGS. Ninja 1.12 is not a
# jobserver client: it runs with --jobs, at most 4, as meson does. A row
# with `local = "<dir>"` builds the fixture scripts/oss/<dir> in place of a
# clone.
#
# Environment: KILN_OSS_CXX (the kiln++ to test; default
# $CARGO_TARGET_DIR/harness/kiln++; ignored by a staged row), KILN_OSS_CC
# (the kiln C driver for a `c = "kiln"` row; default `kiln` beside
# KILN_OSS_CXX; WP-1277), KILN_OSS_ROOT (default <kiln>/.oss).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KILN="$(cd "$HERE/../.." && pwd)"
OSS="${KILN_OSS_ROOT:-$KILN/.oss}"
WRAP="$HERE/kiln-cxx"
# meson and ninja (WP-920) come from a venv under the OSS root, never a
# system install: `python3 -m venv .oss/venv && .oss/venv/bin/pip install
# meson==1.12.1 ninja==1.13.2`.
[ -d "$OSS/venv/bin" ] && PATH="$OSS/venv/bin:$PATH"

usage() { sed -n '2,3p' "$0" | sed 's/^# //' >&2; exit 2; }
PROJECT="" COMPILER=kiln RUN_TEST=1 JOBS=4 INSTALL="" WHICH="" STDLIB="${KILN_OSS_STDLIB:-libstdc++}"
while [ $# -gt 0 ]; do
  case "$1" in
    --compiler) COMPILER="${2:-}"; shift 2 ;;
    --compiler=*) COMPILER="${1#*=}"; shift ;;
    --stdlib) STDLIB="${2:-}"; shift 2 ;;
    --stdlib=*) STDLIB="${1#*=}"; shift ;;
    --no-test) RUN_TEST=""; shift ;;
    --jobs) JOBS="${2:-}"; shift 2 ;;
    --jobs=*) JOBS="${1#*=}"; shift ;;
    --install) INSTALL="${2:-}"; RUN_TEST=""; shift 2 ;;
    --which) WHICH=1; shift ;;
    -h|--help) usage ;;
    -*) echo "oss-build: unknown option $1" >&2; usage ;;
    *) [ -z "$PROJECT" ] || usage; PROJECT="$1"; shift ;;
  esac
done
[ -n "$PROJECT" ] || usage
case "$COMPILER" in kiln|clang) ;; *) echo "oss-build: --compiler must be kiln or clang" >&2; exit 2 ;; esac
case "$STDLIB" in
  libstdc++) SFX="" ;;
  libc++) SFX="-libcxx" ;;
  *) echo "oss-build: --stdlib must be libstdc++ or libc++" >&2; exit 2 ;;
esac
export KILN_OSS_STDLIB="$STDLIB"
case "$JOBS" in ''|*[!0-9]*) echo "oss-build: --jobs takes a number" >&2; exit 2 ;; esac

CFG="$(python3 "$HERE/projects.py" shell "$PROJECT")" || exit 2
eval "$CFG"

if [ -n "$P_STAGE_OF" ]; then
  # A bootstrap stage: the compilers are another row's build artifacts.
  # With --stdlib libc++ they are the ones the libc++ build of that row built.
  stage="$OSS/build/$P_STAGE_OF/$COMPILER$SFX"
  CXX="$stage/$P_STAGE_CXX" P_C_COMPILER="$stage/$P_STAGE_CC"
  how="stage $P_STAGE_OF/$COMPILER$SFX"
  for t in "$CXX" "$P_C_COMPILER"; do
    [ -x "$t" ] || { echo "oss-build: $PROJECT builds with $t, which is not built: run 'oss-build.sh $P_STAGE_OF --compiler $COMPILER --stdlib $STDLIB' first" >&2; exit 2; }
  done
elif [ "$COMPILER" = kiln ]; then
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
# C (WP-1277). `c = "clang"` (the default): both builds run `c_compiler`
# directly and C TUs are not logged. `c = "kiln"`: both builds run kiln-cc,
# which logs each C TU (`lang`) and runs $KILN_OSS_CC: in the kiln build the
# kiln driver beside kiln++ (or $KILN_OSS_CC when set), in the control
# `c_compiler` (clang-19).
CC_REAL="$(command -v "$P_C_COMPILER" || echo "$P_C_COMPILER")"
if [ "$P_C" = kiln ]; then
  if [ "$COMPILER" = kiln ]; then
    CC_REAL="${KILN_OSS_CC:-$(dirname "$CXX")/kiln}"
    CC_REAL="$(command -v "$CC_REAL")" || { echo "oss-build: $PROJECT has c = \"kiln\" but there is no kiln driver beside $CXX: set KILN_OSS_CC" >&2; exit 2; }
  else
    [ -x "$CC_REAL" ] || { echo "oss-build: $P_C_COMPILER not on PATH" >&2; exit 2; }
  fi
  P_C_COMPILER="$HERE/kiln-cc"
  export KILN_OSS_CC="$CC_REAL"
fi
if [ -n "$WHICH" ]; then
  echo "CXX=$CXX"
  if [ "$P_C" = kiln ]; then echo "CC=$P_C_COMPILER"; echo "CC_REAL=$CC_REAL"; else echo "CC=$CC_REAL"; fi
  exit 0
fi
echo "oss-build: $PROJECT $P_TAG with $CXX (from $how), $STDLIB; C ($P_C) with $CC_REAL"
export KILN_OSS_CXX="$CXX"

# Dependencies: built with this compiler, installed once, found by cmake.
PREFIXES=()
for dep in "${P_DEPS[@]}"; do
  pre="$OSS/$dep-install-$COMPILER$SFX"
  if [ ! -f "$pre/.oss-installed" ]; then
    echo "oss-build: dependency $dep -> $pre"
    "$0" "$dep" --compiler "$COMPILER" --stdlib "$STDLIB" --jobs "$JOBS" --install "$pre" || { echo "oss-build: dependency $dep failed" >&2; exit 1; }
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

# One retry of a fetch (WP-1241): a single DNS or network blip must not
# cost a sweep job its report. `cmd || again cmd` runs a failed clone or
# download once more after KILN_OSS_FETCH_RETRY_S seconds (default 30);
# nothing else is retried.
again() {
  echo "oss-build: fetch failed, retrying once in ${KILN_OSS_FETCH_RETRY_S:-30} s" >&2
  sleep "${KILN_OSS_FETCH_RETRY_S:-30}"
  "$@"
}

SRC="$OSS/src/$P_CHECKOUT"
[ -z "$P_LOCAL" ] || SRC="$HERE/$P_LOCAL"
BUILD="$OSS/build/$PROJECT/$COMPILER$SFX${INSTALL:+-dep}"
mkdir -p "$OSS/src" "$BUILD"
export KILN_OSS_LOG="$BUILD/compile.jsonl"
# The wrapper's address-space cap: the project's `vmem_kb` (default
# 3000000, at most 6000000) unless the caller set KILN_OSS_VMEM_KB.
export KILN_OSS_VMEM_KB="${KILN_OSS_VMEM_KB:-$P_VMEM_KB}"
: > "$KILN_OSS_LOG"
rm -f "$BUILD/report.json" "$BUILD/configure.log" "$BUILD/build.log" "$BUILD/test.log"

if [ -n "$P_LOCAL" ]; then
  [ -d "$SRC" ] || { echo "oss-build: the local fixture $SRC does not exist" >&2; exit 2; }
elif [ -n "$P_ARCHIVE" ]; then
  # A release tarball (WP-920: autotools projects whose git tags carry no
  # generated `configure`; the harness has no autoreconf). `url` is the
  # tarball, `sha256` pins it, and $SRC/.oss-archive records the tag.
  if [ -f "$SRC/.oss-archive" ]; then
    have="$(cat "$SRC/.oss-archive")"
    [ "$have" = "$P_TAG" ] || echo "oss-build: warning: $SRC holds '$have', the toml pins '$P_TAG' (delete the checkout to refetch)" >&2
  else
    tarball="$OSS/src/$P_CHECKOUT.tarball"
    echo "oss-build: fetching $P_URL"
    curl_get() { curl -sSfL --retry 3 -o "$tarball" "$P_URL"; }
    curl_get || again curl_get || { echo "oss-build: download failed" >&2; exit 1; }
    echo "$P_SHA256  $tarball" | sha256sum -c --quiet - || { echo "oss-build: $P_URL does not match the toml's sha256" >&2; rm -f "$tarball"; exit 1; }
    mkdir -p "$SRC" && tar -xf "$tarball" -C "$SRC" --strip-components=1 || { echo "oss-build: cannot unpack $tarball" >&2; exit 1; }
    rm -f "$tarball"
    echo "$P_TAG" > "$SRC/.oss-archive"
  fi
elif [ -d "$SRC/.git" ]; then
  have="$(git -C "$SRC" describe --tags --exact-match 2>/dev/null || echo '?')"
  [ "$have" = "$P_TAG" ] || echo "oss-build: warning: $SRC is at '$have', the toml pins '$P_TAG' (delete the checkout to refetch)" >&2
else
  sub=()
  [ -n "$P_SUBMODULES" ] && sub=(--recurse-submodules --shallow-submodules)
  echo "oss-build: cloning $P_URL@$P_TAG"
  # A failed clone removes the directory it created, so the retry starts
  # clean; one that leaves $SRC (a submodule failed after the checkout) is
  # not retried, since the second clone would refuse the directory.
  clone() { git -c advice.detachedHead=false clone -q --depth 1 --branch "$P_TAG" "${sub[@]}" "$P_URL" "$SRC"; }
  clone || { [ ! -e "$SRC" ] && again clone; } || { echo "oss-build: clone failed" >&2; exit 1; }
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

# Modules (WP-1148): Ninja, the scanner, and the stdlib flag for it.
GEN=(-G "Unix Makefiles") MODFLAGS=()
if [ -n "$P_MODULES" ]; then
  [ "$P_BUILD" = cmake ] || { echo "oss-build: modules = true needs a cmake project" >&2; exit 2; }
  command -v ninja > /dev/null || { echo "oss-build: modules rows need ninja (apt-get install ninja-build)" >&2; exit 2; }
  if [ "$COMPILER" = kiln ] && [ -z "$P_STAGE_OF" ]; then
    SCAN="$(dirname "$CXX")/kiln-scan-deps"
  else
    b="$(basename "$CXX")"; SCAN="$(dirname "$CXX")/${b/clang++/clang-scan-deps}"
  fi
  [ -x "$SCAN" ] || { echo "oss-build: no scanner $SCAN beside $CXX" >&2; exit 2; }
  echo "oss-build: modules: Ninja $(ninja --version), scanner $SCAN"
  GEN=(-G Ninja)
  MODFLAGS=("-DCMAKE_CXX_COMPILER_CLANG_SCAN_DEPS=$SCAN")
  [ "$STDLIB" = libc++ ] && MODFLAGS+=("-DCMAKE_CXX_FLAGS=-stdlib=libc++")
  [ "$JOBS" -le 4 ] || JOBS=4
fi

T0=$(date +%s.%N)
BUILD_RC="" TEST_RC=""
export KILN_OSS_PHASE=configure
case "$P_BUILD" in
  cmake)
    nice -n 10 cmake --fresh -S "$SRCDIR" -B "$BUILD" "${GEN[@]}" \
      -DCMAKE_CXX_COMPILER="$WRAP" -DCMAKE_C_COMPILER="$P_C_COMPILER" \
      -DCMAKE_CXX_STANDARD="$P_CXX_STD" -DCMAKE_BUILD_TYPE=Release \
      "${MODFLAGS[@]}" "${P_CONFIGURE[@]}" > "$BUILD/configure.log" 2>&1
    CONF_RC=$? ;;
  meson)
    # meson's only Linux backend is ninja, which is not a jobserver client:
    # bound it to --jobs (at most 4) instead.
    [ "$JOBS" -le 4 ] || JOBS=4
    command -v meson > /dev/null && command -v ninja > /dev/null || { echo "oss-build: meson rows need meson and ninja: python3 -m venv $OSS/venv && $OSS/venv/bin/pip install meson==1.12.1 ninja==1.13.2" >&2; exit 2; }
    # kiln-cxx's --version shim (WP-920): meson needs `clang` in it.
    export KILN_OSS_VERSION_SHIM=1
    CXX="$WRAP" CC="$P_C_COMPILER" nice -n 10 meson setup --wipe "$BUILD/meson" "$SRCDIR" \
      --buildtype=release -Dcpp_std="c++$P_CXX_STD" "${P_CONFIGURE[@]}" > "$BUILD/configure.log" 2>&1
    CONF_RC=$? ;;
  make)
    # A copy of the source tree per build (WP-1277): an in-source Makefile
    # would otherwise let the kiln and clang builds share objects.
    mkdir -p "$BUILD/make" && find "$BUILD/make" -mindepth 1 -delete
    tar -C "$SRCDIR" --exclude=.git -cf - . | tar -C "$BUILD/make" -xf -
    CONF_RC=$? ;;
  autotools)
    # Out of source, from the tarball's generated configure (WP-920).
    mkdir -p "$BUILD/autotools" && find "$BUILD/autotools" -mindepth 1 -delete
    (cd "$BUILD/autotools" && nice -n 10 "$SRCDIR/configure" CXX="$WRAP" CC="$P_C_COMPILER" \
      CXXFLAGS="-O2 -std=c++$P_CXX_STD" "${P_CONFIGURE[@]}") > "$BUILD/configure.log" 2>&1
    CONF_RC=$? ;;
esac
echo "oss-build: configure rc=$CONF_RC ($BUILD/configure.log)"

if [ "$CONF_RC" = 0 ]; then
  export KILN_OSS_PHASE=build
  case "$P_BUILD" in
    cmake)
      if [ -n "$P_MODULES" ]; then
        nice -n 10 ninja -C "$BUILD" -t clean > /dev/null 2>&1
        nice -n 10 ninja -C "$BUILD" -k 0 -j "$JOBS" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      else
        nice -n 10 make -C "$BUILD" clean > /dev/null 2>&1
        nice -n 10 make -k "${JFLAG[@]}" -C "$BUILD" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      fi
      BUILD_RC=$? ;;
    meson)
      nice -n 10 meson compile -C "$BUILD/meson" -j "$JOBS" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      BUILD_RC=$? ;;
    make)
      # The project's own Makefile in the copy, told which compilers to use.
      nice -n 10 make -k "${JFLAG[@]}" -C "$BUILD/make" CXX="$WRAP" CC="$P_C_COMPILER" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      BUILD_RC=$? ;;
    autotools)
      nice -n 10 make -k "${JFLAG[@]}" -C "$BUILD/autotools" "${P_BUILD_TARGETS[@]}" > "$BUILD/build.log" 2>&1
      BUILD_RC=$? ;;
  esac
  echo "oss-build: build rc=$BUILD_RC ($BUILD/build.log)"
  if [ -n "$INSTALL" ]; then
    [ "$BUILD_RC" = 0 ] && nice -n 10 cmake --install "$BUILD" >> "$BUILD/build.log" 2>&1 && date -u > "$INSTALL/.oss-installed" || BUILD_RC=1
    echo "oss-build: install into $INSTALL rc=$BUILD_RC"
  fi
  if [ -n "$RUN_TEST" ]; then
    export KILN_OSS_PHASE=test
    tdir="$BUILD"; [ "$P_BUILD" = meson ] && tdir="$BUILD/meson"; [ "$P_BUILD" = make ] && tdir="$BUILD/make"; [ "$P_BUILD" = autotools ] && tdir="$BUILD/autotools"
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

python3 "$HERE/projects.py" report-json --project "$PROJECT" --compiler "$COMPILER" --stdlib "$STDLIB" \
  --compiler-path "$CXX" --c-compiler-path "$CC_REAL" --build-dir "$BUILD" --configure-rc "$CONF_RC" \
  ${BUILD_RC:+--build-rc "$BUILD_RC"} ${TEST_RC:+--test-rc "$TEST_RC"} --wall "$WALL"
[ -z "$INSTALL" ] || [ "$BUILD_RC" = 0 ]
