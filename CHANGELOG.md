# Changelog

All notable changes to kiln are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and kiln adheres to [Semantic Versioning](https://semver.org/). What a
version number promises is in [docs/STABILITY.md](docs/STABILITY.md).

## [Unreleased]

Planned as 1.1.0.

### Added

- C mode: `kiln` (or `-x c`) compiles C at `-std=c89`, `c99`, `c11`,
  `c17`, `c23` and the `gnu` dialects (default `gnu17`). C lexing (no
  C++-only punctuators, user-defined literals or alternative tokens),
  C keywords, clang's C predefined macros and feature-test answers
  (`__has_c_attribute`; `__has_cpp_attribute` is C++ only), and C
  linkage (unmangled names, `fn.var` static locals), so C and C++ objects
  link together. C-only semantics are still in progress
  ([docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md#c-mode)) (WP-1275).

## [1.0.1] - 2026-11-11

A bug-fix release. Nothing in it changes what [docs/STABILITY.md](docs/STABILITY.md)
promises.

### Fixed

- Reflection mangling (the ABI note in
  [docs/STABILITY.md](docs/STABILITY.md#covered-stable-within-a-major-version)): two
  different dependent reflected template-ids could get the same symbol
  (`^^AT<T>` and `^^TT<T*>`, and `^^F<T,int>` and `^^F<T,long>` in gcc
  mode). They no longer collide (WP-1250). GCC 16.1's spelling is used
  wherever it cannot collide. Three of its spellings lose information,
  and GCC itself fails to assemble the resulting overload pairs: a
  dependent function template-id without its arguments, a qualified
  variable template-id without its qualifier, and an alias template-id
  whose aliased type does not pin down the alias's arguments. kiln keeps
  its own spelling in those three cases, so the names differ from GCC's
  there; spec/abi.md §8.14 lists them. A reflected alias whose parameter
  appears only in a non-deduced context now gets GCC's `ta` form when
  its argument is dependent, so it links with g++-16 (WP-1260). A value-dependent reflection is no longer evaluated before
  instantiation (WP-1260).
- A consteval-only reference in a template is checked again in each
  instantiation. A dependent initializer such as
  `constexpr bool b = m == members[0];` in a `template for` body is no
  longer rejected (WP-1263, P3032R2).
- A failed `consteval` block in a lambda body leaves the lambda valid
  (WP-1257). A failed `static_assert` in a lambda body no longer stops
  evaluation, so a second refusal is reported too (WP-1247).
- Diagnostics:
  - When a reflection query triggers an instantiation error, the
    diagnostic names the consteval call frames (WP-1248). Those frames
    still show under copy elision and nesting, and spec names keep their
    template arguments (WP-1258).
  - Text the program provides (an uncaught exception's `what()`,
    attribute messages, reflected names) is escaped as clang escapes it
    (WP-1259).
  - An uncaught exception's `what()` keeps the characters read before a
    fault, and its length cap does not split a UTF-8 sequence (WP-1251).
  - An invalid escape in an unevaluated string (a `static_assert`
    message, `[[deprecated]]`, `[[nodiscard]]`, `extern "…"`, asm) no
    longer causes follow-on errors. The construct is dropped, as in
    clang 22. Escape diagnostics quote the whole escape and use clang 22's
    source ranges (WP-1268).
  - Column widths and the escaping of unprintable characters use LLVM
    22's Unicode 15.1 tables, so an unassigned code point is shown as
    `<U+XXXX>` as clang shows it (WP-1268).
- Invalid UTF-8 in an ordinary string or character literal is the
  `-Winvalid-source-encoding` warning, not an error, as in clang
  (WP-1267).
- Under `-fkiln-source-path-style=gcc`, header paths are spelled as
  g++-16 spells them (the `-I` directory exactly as given, `"../x.h"`
  relative to the includer). This is the name that `source_location` and
  `std::meta::source_location_of` report (WP-1262).
- C++20 modules: the experimental declaration image
  (`-fkiln-bmi-image`) now covers imports, partitions and
  using-declarations (WP-1068).

### Testing and tooling (not user-visible)

- The harness can run GCC 16.1 as the compiler under test
  (`--compiler $KILN_GCC`). It runs clean over the reflection and
  contracts tests (WP-1249), and every harness command maps its flags for
  GCC (WP-1261, WP-1266). Test compiles pass `-fno-crash-diagnostics`
  (WP-1264).
- The clang oracle retries once when a compile dies on a signal
  (WP-1254).
- The sweep workflow's `oss` job fails when it produces no report
  (WP-1241). It runs as one `oss-matrix` job over the `sweep = true` rows
  of `projects.toml` (WP-1265).
- `release.yml` uploads the dist archives from the workspace, and can be
  started by hand as a dry run (WP-1253).
- Spec notes with pinning tests: `type_of(reflect_constant(^^int))`
  (WP-1255), and `reflect_constant` of a function reflection's value,
  which instantiates nothing (WP-1252).

## [1.0.0] - 2026-11-10

The first release. This section summarises what kiln 1.0 does. Measured
results are in [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md), and the
limits are in [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md).

### Language

- C++98 through C++23, and C++26 as in the Croydon DIS draft: 260 of the
  261 rows of [docs/STANDARD_SUPPORT.md](docs/STANDARD_SUPPORT.md) are
  done. The one partial row is P2786 (trivial relocatability), which was
  removed from C++26 at Kona and is not in the DIS.
- C++26 static reflection (P2996, `<meta>`), on by default at
  `-std=c++26` (`-fno-reflection` turns it off). The `<meta>` header is
  GCC 16.1's libstdc++ one, selected with `--gcc-toolchain=`; libstdc++
  14 and libc++ 22 do not ship it. When `#include <meta>` is not found at
  C++26, a note names the GCC installation (or libc++) kiln selected, and
  a GCC 16 found through `g++-16` or `g++` on `PATH`.
- C++26 contracts (P2900): `pre`, `post`, `contract_assert`, the four
  evaluation semantics (`-fcontract-evaluation-semantic=`), and a
  replaceable violation handler.
- C++26 erroneous behaviour (P2795): at `-std=c++26` an uninitialized
  automatic variable (up to 4096 bytes) is zero-filled at its
  declaration. clang 22 does not do this. `[[indeterminate]]` on the
  variable, or `-ftrivial-auto-var-init=uninitialized`, opts out.
- C++20 modules, header units and `import std;`, in one invocation or in
  clang's two-step build: `--precompile`, `-fmodule-output`,
  `-fmodule-file=`, `-fprebuilt-module-path=`. kiln's module files are
  its own format. The declaration image (`-fkiln-bmi-image`, which builds
  an imported module from a serialized image instead of replaying it) is
  experimental and off by default; it does not yet cover `import std;`.
- Dependency scanning: `kiln-scan-deps` (also installed as
  `clang-scan-deps-22` and `clang-scan-deps`) writes P1689r5, so CMake
  3.28+ with Ninja builds C++20 module targets unchanged.
- Coroutines, exceptions, RTTI, `thread_local`, and clang's and GCC's
  common extensions and builtins.

### Constant evaluation (the Oven)

- All constant evaluation (`constexpr`, `consteval`, `static_assert`,
  template arguments, reflection) runs in the Oven, kiln's bytecode
  evaluator. It has exact per-evaluation budgets for steps, depth and
  live memory (`-fconstexpr-steps=`, `-fconstexpr-depth=`,
  `-fconstexpr-memory=`), and memory freed during an evaluation is
  reclaimed during that evaluation.
- On kiln's benchmark corpus, compared with clang 19: geometric mean
  0.21x the time and 0.17x the peak memory
  ([docs/status/perf.md](docs/status/perf.md)).
- Reports: `-fconstexpr-report=` (NDJSON per evaluation),
  `-fkiln-instantiation-report`, `-fkiln-memory-report`.

### Standard libraries and compatibility

- libc++ 22.1.8 test suite at `-std=c++26`: 99.4%. libc++ 19.1.7: 98.3%.
  Every public libstdc++ 14 header compiles at `-std=c++20` and
  `-std=c++26`.
- 47 open-source projects built with their own build systems and test
  suites, including all of LLVM, Boost, folly, Abseil, DuckDB and CMake:
  46 at parity with clang 19 or better on every test; the Boost
  metaprogramming libraries match clang 22.
- clang-compatible command line and predefined macros. CMake and
  autotools work unchanged. kiln reproduces clang 22 by default and
  clang 19 with `-fclang-compat=19`.

### Code generation and tools

- Writes LLVM IR as text and drives LLVM's tools. It never links against
  LLVM. Supports LLVM 22 (preferred) and LLVM 19, found on its own
  (`--llvm-dir=` and `KILN_LLVM_DIR` name one); `--version` and `-v`
  print the LLVM in use.
- Itanium C++ ABI as clang implements it, on Linux aarch64 and x86_64.
- `-O0` to `-O3`, `-Os`, `-Oz`; LTO (`-flto`, `-flto=thin`);
  floating-point model flags; AArch64 branch protection.
- DWARF debug information (`-g` and its levels, `-gdwarf-N`) that gdb and
  lldb read as they read clang's.
- Sanitizers: address, hwaddress, thread, memory, leak, fuzzer, and
  undefined behaviour.
- Coverage: `--coverage` (gcov) and `-fprofile-instr-generate`
  counters. Coverage mapping and PGO are not in 1.0.

### Robustness and distribution

- An internal compiler error prints a report and writes a reproducer
  (`.ii` and `.sh`) instead of a panic
  ([docs/REPORTING-BUGS.md](docs/REPORTING-BUGS.md)). kiln runs out of
  stack only as a diagnostic.
- Every crate is `#![forbid(unsafe_code)]`. The MSRV is Rust 1.95.
- Release archives from `cargo xtask dist`: relocatable,
  `kiln-<version>-<triple>.tar.xz` with a stripped binary, plus a
  `-symbols` archive and `SHA256SUMS`.

### Fixed in the final release candidate

These landed after the first 1.0 release candidate was measured.

- x86 builtins: every x86 builtin that clang lowers to generic IR is now
  lowered (AVX and AVX2 extract, insert and `perm2x128`, F16C, the FMA
  family and more). The builtins kiln declares but cannot lower went
  from 595 to 23 (`maskmovq`, Key Locker, the AMX internals), and those
  23 answer `__has_builtin` with 0. A call of an x86 builtin sets
  `min-legal-vector-width`, which fixes an LLVM backend crash at
  `-march=skylake-avx512 -O2`. OpenCV's AVX2 dispatch TUs now compile
  (WP-1242).
- Reflection: `extract` of a function now instantiates and odr-uses it
  (WP-1244).
- Constant evaluation: an uncaught exception in a constant evaluation
  prints its `what()` whole, up to 64 KiB, after which it ends with
  "… [N more characters]" (WP-1245).
- Reflection: `^^` accepts operator, literal-operator, conversion and
  dependent template-ids, such as `^^ns::operator==<int>` (WP-1246).
- Reflection: `members_of` leaves out members whose type is an undeduced
  placeholder, and `type_of` on one throws, as in GCC.
  `display_string_of` prints requires-clauses as GCC 16.1 does
  (WP-1243).

## Before 1.0

Before 1.0 kiln was developed in the open on `main`, at version 0.0.1,
with no tagged releases. The history is in the git log, the milestones
in [docs/ROADMAP.md](docs/ROADMAP.md), and each work package's record in
[docs/BACKLOG.md](docs/BACKLOG.md) and `spec/landed/`.

[1.0.1]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.1
[1.0.0]: https://github.com/christopherfretz/kiln/releases/tag/v1.0.0
