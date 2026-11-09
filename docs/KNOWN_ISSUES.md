# Known issues

This page lists what kiln does not do yet, or does differently, as of
1.0. Each item names the work package that tracks it in
[BACKLOG.md](BACKLOG.md), where one exists. How to use the parts that do
work is in the [user guide](USER_GUIDE.md). Measured results for each
library and project are in [COMPATIBILITY.md](COMPATIBILITY.md).

If you hit something not listed here, please
[report it](REPORTING-BUGS.md).

## Platforms

- **Linux on aarch64 and x86_64 only.** Any other `--target` (macOS,
  Windows, 32-bit, other architectures) is `unsupported target`. Support
  for other platforms is not planned for 1.0.
- **glibc 2.39 or newer** is needed to run the release binaries. Building
  them on an older-glibc image is [WP-943](BACKLOG.md).
- **x86 `-m<feature>` flags** (`-mavx2`, `-msse4.1`, `-mno-<feature>`, ...)
  are `unknown argument`. Projects that build SIMD variants with them
  (Bitcoin Core, OpenCV) configure that code out:
  [WP-1000](BACKLOG.md). `-march=` and `-mcpu=` work.
- **x86_64-only gaps** found by the first x86_64 sweep, in
  [status/x86_64-sweep.md](status/x86_64-sweep.md):
  - glibc's x86-64 `typedef __float128 _Float128` does not make the two
    the same type as it does under clang, so LLVM's `IRTests` does not
    build: [WP-1001](BACKLOG.md).
  - A narrow signed `++` bug exposed by signed `char` (2 libc++ tests);
    see the sweep page.
  - gRPC's full x86_64 results are pending: [WP-1002](BACKLOG.md).
- **LLVM 19 and 22 only.** LLVM 20 and 21 are not supported.
- **Identification.** `--version` prints `kiln version <v>` without the
  git commit ([WP-942](BACKLOG.md)). The `-v` output of a link step
  repeats the backing clang's "Debian clang version" line
  ([WP-976](BACKLOG.md)).

## C mode

kiln compiles C++ only. `kiln` can preprocess C (`-E` on `.c`, `.h`,
`.i`, or `-x c`), but compiling C is `not implemented yet`. `kiln++`
compiles a `.c` file as C++, with a warning, as `clang++` does. C support
is [WP-243](BACKLOG.md) and is not part of 1.0.

## Build systems

- **Meson** requires the word `clang` in `--version` output and rejects
  kiln. Use the wrapper in the
  [user guide](USER_GUIDE.md#using-kiln-from-a-build-system). kiln does
  not ship one. CMake and autotools work unchanged
  ([COMPATIBILITY.md](COMPATIBILITY.md#build-systems)).
- **`cargo install`**: kiln is not on crates.io, and
  `cargo install --path crates/kiln_driver` installs the `kiln` binary
  without the `kiln++` symlink or the resource directory. Use the release
  archive or `cargo xtask dist`
  ([STABILITY.md](STABILITY.md#cratesio-not-published)).

## Modules

Module files (`--precompile`, `-fmodule-output`) work. They are kiln's
own format, so see the [user guide](USER_GUIDE.md#modules). What is
missing (spec/modules.md §11.9):

- **Dependency scanning** (P1689 output, `-fmodules-reduced-bmi`), so
  CMake's C++20 module support (`CXX_SCAN_FOR_MODULES`, `FILE_SET
  CXX_MODULES`) does not work with kiln yet.
- **`-fmodule-header`** and header-unit module files. Header units work
  from their headers.
- **No `import std;` module-file cache.** `import std;` compiles libc++'s
  `std.cppm` in every importing translation unit unless you precompile it
  yourself (`--precompile std.cppm`, then `-fmodule-file=std=std.pcm`,
  and link `std.o`).
- **Module files hold sources, not a serialised AST**, so an importer
  re-parses the module's text. Importing libc++'s `std` module file costs
  about what compiling `std.cppm` in-process costs
  ([COMPATIBILITY.md](COMPATIBILITY.md#c20-modules)).
- In libc++ 22's suite, one test fails because `std.compat` imported over
  `std` does not keep the two global module fragments apart
  ([status/libcxx-m5.md](status/libcxx-m5.md)). WP-460 fixed it, but the
  full suite has not been re-run since.

## Sanitizers and coverage

- **Source-based coverage mapping.** `-fcoverage-mapping` (what
  `llvm-cov show` and `llvm-cov report` need) is rejected.
  `-fprofile-instr-generate` counters and `--coverage` work:
  [WP-1003](BACKLOG.md).
- **gcov filters** `-fprofile-filter-files` and `-fprofile-exclude-files`
  are rejected: [WP-1004](BACKLOG.md).
- **Profile-guided optimization is not part of 1.0.**
  `-fprofile-generate`, `-fcs-profile-generate`, `-fprofile-use`,
  `-fprofile-instr-use` and `-fprofile-sample-use` are rejected with an
  error that says so.
- **UBSan.** `-fsanitize=vptr` and `-fsanitize=function`
  ([WP-934](BACKLOG.md)), the nullability checks ([WP-935](BACKLOG.md))
  and `local-bounds` ([WP-936](BACKLOG.md)) are accepted but emit no
  check. Other small gaps: [WP-937](BACKLOG.md). The umbrella is
  [WP-852](BACKLOG.md).
- **`-fsanitize-ignorelist=`** files are checked for existence but not
  applied: [WP-857](BACKLOG.md).
- **`-ftrapv`** and `-fsanitize-merge=` are rejected:
  [WP-856](BACKLOG.md).
- Smaller differences from clang's sanitizer IR (value names, ASan module
  name, constructor details) are [WP-855](BACKLOG.md),
  [WP-858](BACKLOG.md), [WP-859](BACKLOG.md) and [WP-861](BACKLOG.md).
  They do not change what the sanitizers report.

## Flags and attributes

kiln rejects a clang flag it does not implement instead of ignoring it.
These flags are the exceptions: they are accepted and have no effect.

| Flag | What happens | WP |
|---|---|---|
| `-ftime-trace[=]`, `-ftime-trace-granularity=`, `-ftime-trace-verbose` | warning; no trace file is written | — |
| `-ffat-lto-objects` | objects are bitcode only, not fat | [WP-899](BACKLOG.md) |
| `-f[no-]vectorize`, `-f[no-]slp-vectorize`, `-fstrict-vtable-pointers` | accepted; LLVM's defaults apply | [WP-900](BACKLOG.md) (deferred) |
| `-fdiagnostics-show-template-tree` | accepted; diagnostics are printed without the tree | [WP-901](BACKLOG.md) (deferred) |
| `-F <dir>` | warning; framework directories do not exist on Linux | — |
| `-f[no-]stack-clash-protection`, `-f[no-]common`, `-fstack-usage`, `-m64`, `-pipe` and a few more | accepted silently; listed in spec/driver.md, "Codegen flags" | — |

`kiln++ --help` does not list every implemented flag yet.
spec/driver.md, "Options", is the complete list.

**Attributes.** Every attribute clang knows is in one of three classes
(`crates/kiln_sema/src/attr_triage.rs`, audit in
[status/silent-drops.md](status/silent-drops.md)):

- Class A attributes would change semantics, the ABI or layout. kiln does
  not implement a few of them, and those are a hard error,
  `'X' attribute is not supported by kiln yet`. That error is not
  silenced by `-w` or in system headers, and `__has_attribute` answers 0
  for them, so portable code takes its fallback path.
- Class B attributes steer only optimization, instrumentation or debug
  information. Class C attributes feed only diagnostics or analyses kiln
  does not run. Both are accepted. Most of class B is implemented
  (WP-813). Where it is not, the cost is a missed optimization or
  diagnostic, never wrong code.
- Semantic and argument checks on clang attributes (rejecting a misused
  `unavailable`, `carries_dependency`, ...) are incomplete:
  [WP-954](BACKLOG.md).

## Diagnostics and accepts-invalid

kiln is measured on clang 22.1.8's own negative tests
([status/accepts-invalid.md](status/accepts-invalid.md), run at
`321baa9de`):

- In 7.1% of test files (89 of 1250) where clang expects an error, kiln
  reports none in some configuration.
- 17.6% of `expected-error` lines (4930 of 28065) have no kiln error on
  them.
- 11.2% of `expected-no-diagnostics` configurations (47 of 418) get an
  error from kiln (rejects-valid).
- 0 crashes, timeouts or out-of-memory results in 2850 cases.

Most missed errors are declaration rules: types defined where the grammar
forbids them, members named after their class, union and static member
rules, redeclarations, and attribute checks. The clusters are
[WP-945](BACKLOG.md) to [WP-960](BACKLOG.md). Valid code is not affected.
Code that clang rejects may compile with kiln, so build with clang or GCC
in CI too if you rely on rejection.

Diagnostic wording follows clang's but is not always identical, and some
warnings clang has are missing. Wording is not covered by
[STABILITY.md](STABILITY.md).

## libc++ 22 test suite

kiln passes 99.2% of libc++ 22.1.8's suite (9247 tests, `-std=c++26`;
[status/libcxx-m5.md](status/libcxx-m5.md)). What was left at that run:

- **9 correctness failures**, all fixed since (WP-460: `trivial_abi` ×6,
  `enable_if`, the `std.compat` modules case; WP-491: `make_from_tuple`).
  The full suite has not been re-run since those fixes.
- **12 expected failures for missing or reworded warnings**, none of
  them wrong code:
  - `-Wnonnull` ×5 ([WP-433](BACKLOG.md))
  - `-Wthread-safety` ×2
  - deprecation warnings on `rel_ops` and the C++17 adaptor typedefs ×2
  - `alloc_align` power-of-two check ×1
  - constraint-note wording ×1
  - `std::hash<std::monostate>` wording ×1
- **1 long test** that times out under heavy host load and passes at its
  full timeout.

## Open-source projects with gaps

Of the 47 projects in [COMPATIBILITY.md](COMPATIBILITY.md#libraries-and-applications),
43 pass everything clang passes. The others:

- **simdjson**: 116 of 117 tests. The one failure is a negative compile
  test that needs `-Wuninitialized` under `-Werror`.
- **folly**: 3767 of 3773 in the full run. The 6 failures (`memcpy_test`)
  were fixed by WP-763 and pass in a targeted re-run.
- **Boost metaprogramming libraries**: 99.6% of clang's passing tests.
  Of the 23 TUs that fail, 13 also fail under clang 19.
- **folly::coro**: 583 of 618. All 33 kiln-only failures had one cause,
  fixed by WP-802. A partial re-run found two more compile failures,
  fixed by WP-933. The full re-run is [WP-992](BACKLOG.md).

## Crashes

kiln reports an internal compiler error instead of crashing silently, and
writes a reproducer ([REPORTING-BUGS.md](REPORTING-BUGS.md)). Known
internal-error sites being hardened into diagnostics are
[WP-971](BACKLOG.md) to [WP-974](BACKLOG.md).
