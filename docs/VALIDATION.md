# Validation

Kiln's correctness is established by layered, automated evidence. The rule
for every change: new behavior ships with the test that would fail without it,
and perf-sensitive behavior ships with a recorded baseline.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit tests | `crates/*/src`, `crates/*/tests` | Module contracts in `spec/` |
| C | `tests/c/<area>/` (`hello`, `lex`, `predefined`, `linkage`, `interop`; spec/harness.md "C tests"); a default root | C mode (WP-1275): C lexing and keywords, the C predefines and feature queries vs `clang-22 -x c`, C linkage, C/C++ interop, `run` at every C standard |
| Conformance | `tests/conformance/<section>/` | Standard behavior by section ([lex], [cpp], [expr.const], ...) |
| Stress | `tests/stress/` | Deep nesting, huge packs, the stack guard, runaway-allocation regressions, under `mem<=N` budgets |
| Differential | `tests/differential/` | Same result as clang (-E tokens, program stdout, AST/layout facts) |
| Debugger | `tests/debuginfo/` (`expect: debugger`, spec/harness.md "Debugger tests"); a default root, SKIP without gdb/lldb-19, ERROR under `KILN_REQUIRE_DEBUGGERS=1` (GitHub CI) | A `-O0 -g` binary debugs under gdb and lldb-19 as clang's does: breakpoints, backtraces, variables, stepping |
| Goldens | alongside the above, `*.expected` | AST dumps, diagnostics, IR shapes (FileCheck-style) |
| libc++ suite | `libcxx/test` of llvmorg-19.1.7 (`scripts/fetch-libcxx-tests.sh [<tag>]`), driven by `cargo xtask libcxx` (`spec/libcxx.md`); manual/periodic gate, not part of `ci`; M3 gate status in `docs/status/libcxx-m3.md`, whole-suite status and the 10% sample procedure in `docs/status/libcxx.md` (M3b) | The real world |
| Accepts-invalid | clang's own C++ negative tests (`clang/test/{CXX,SemaCXX}` of llvmorg-22.1.8, `-verify` files; `scripts/fetch-clang-tests.sh`), driven by `cargo xtask clang-neg` (`spec/clang-neg.md`); informational, not part of `ci`; status in `docs/status/accepts-invalid.md` | kiln rejects what clang rejects: no file where clang expects an error passes silently through kiln (accept vs reject only, no wording) |
| GCC C torture | GCC 16.1's `gcc.c-torture` (`execute`, `execute/ieee`, `execute/builtins`, `compile`; `scripts/fetch-gcc-tests.sh`, gitignored `.oss/gcc-tests`, never vendored), driven by `cargo xtask gcc-torture` (spec/harness.md "GCC testsuites"); informational, not part of `ci`; status in `docs/status/c-mode.md` | kiln's C mode compiles and runs GCC's C tests at `-O0` and `-O2` as GCC 16.1 and clang-19 do; each case is pass, compile-reject, crash, wrong or timeout beside the oracles' result |
| GCC C++ testsuite | GCC 16.1's `g++.dg` (with `g++.dg/torture`), `c-c++-common` and `g++.old-deja` (same sparse checkout, `scripts/fetch-gcc-tests.sh`, never vendored), driven by `cargo xtask gxx-dg` (spec/harness.md "The g++.dg runner"); informational, not part of `ci`; status in `docs/status/gxx-dg.md` | kiln++ meets each test's `dg-error` expectations (and the errors its `dg-message` and `dg-regexp` regexps name) by line and severity (never by message) at the standards GCC's `g++-dg.exp` picks, compiles valid tests and runs `dg-do run` ones; each case is pass, rejects-valid, accept-invalid, wrong-line, wrong, crash or timeout (or expect-unknown, counted apart, where the file does not decide), with GCC 16.1 and clang on the cases kiln fails |
| libstdc++ headers | every public header of the system libstdc++ (`/usr/include/c++/14`) as a one-line TU, driven by `cargo xtask libstdcxx` (`spec/libstdcxx.md`); informational, not part of `ci`; status in `docs/status/libstdcxx.md` | The other real world (GCC's library) |
| Feature table | `docs/STANDARD_SUPPORT.md`, the standard support table (one section per standard, C++98/03 to C++26, reflection and kiln extensions), checked by `cargo xtask features` (`spec/harness.md` "Feature table"); manual/M4-gate check, not part of `ci` | Every core-language feature marked `done` has passing tests and clang 22's `__cpp_*` value at its standard; the summary matches the sections |
| Static gaps | `docs/status/static-gaps.md`, every source site that reports a known-incomplete language corner ("not supported yet", "not yet implemented", mangling, codegen fallbacks, Oven traps, reflection), checked by `cargo xtask static-gaps` in `ci --step lints` (spec/harness.md "CI", step 0). Each site carries a `// kiln-gap: GAP-NNN` marker naming its entry, so the check ignores line shifts; it fails on an unmarked site, an entry with no site left, an entry whose sites cell is stale, and an entry whose BACKLOG row is not open | No language gap exists that the inventory and a BACKLOG row do not name; a fixed gap leaves the list |
| Local paths | every tracked text file, checked in `ci --step lints` (`xtask/src/local_paths.rs`, spec/harness.md "CI", step 0) | No committed file holds an absolute path of a developer's machine (`/Volumes/`, `/Users/`, `/home/<user>/`, `/root/`, `/tmp/claude`); records and docs use paths relative to the checkout (bench records write `target/release/kiln++`, spec/bench.md "Recording") |
| Perf | `bench/` + `bench/baselines/*.json` | No wall-clock or memory regression; vs-clang ratios |
| Reflection perf | `bench/reflect/` (shapes S1-S6 from `cargo xtask gen-reflect`; named explicitly), `cargo xtask bench --scaling <shape>` (spec/bench.md "Reflection shapes", "Scaling sweeps"); manual/nightly on the dev host, not part of `ci` | The §3.8 budgets of docs/plans/m6-reflection.md: `peak_arena` independent of N, linear wall time and RSS, kiln vs GCC 16.1 |
| External corpora | `cargo xtask test --external <dir>` / `cargo xtask bench --external <dir>` on directories outside the repo (spec/harness.md "External corpora"); manual, never in `ci` | Never crash: crash reproducers and large downstream code compile or fail cleanly |
| OSS builds | real open-source projects (`scripts/oss/projects.toml`) built through their own build systems and test suites with kiln as the C++ compiler (`scripts/oss/`, spec/landed/harness/WP-666.md); manual, needs the network, never in `ci`; status in `docs/status/oss.md` | Real code bases build and pass their tests at parity with clang-19 (ROADMAP M8) |

**What runs where.** Merge-queue CI (`cargo xtask ci`, both jobs) runs the
unit tests, the corpus in clang and kiln mode, and the `oven:` counter
budgets the corpus carries, and every `oracle: gcc` comparison against
GCC 16.1, the reflection oracle: the jobs run in the `gcc:16.1.0-trixie`
image with `KILN_GCC=/usr/local/bin/g++` and `KILN_REQUIRE_GCC=1`, so a
missing GCC fails CI instead of skipping (WP-502; spec/harness.md "GCC
oracle"). The same jobs install gdb and lldb-19 and set
`KILN_REQUIRE_DEBUGGERS=1`, so the debugger tests (`tests/debuginfo`)
run in CI and a missing debugger fails it (WP-816). Likewise they
install compiler-rt for clang 19 and 22 and set
`KILN_REQUIRE_SANITIZER_RUNTIME=1`, so the ASan/UBSan run tests
(`requires: sanitizer-runtime`) run on both architectures; the
`unit codegen` job runs the Itanium mangling differential
(`mangle_vs_clang`, 39 files, about 1,200 clang symbols per triple) as
its own command with its counts in the log; and one `kiln-22` shard per
lane runs the CMake C++20 modules end-to-end
(`scripts/oss/selftest.sh --cmake-modules`) with CMake and ninja
(WP-1034; "GitHub Actions" below). On the dev host, where `scripts/env.sh` sets `KILN_GCC`,
`cargo xtask ci` and `cargo xtask test` run the same comparisons. Off the queue, by hand or nightly on the dev
host: `cargo xtask bench` (including `bench/reflect` and the vs-GCC table
via `--compiler $KILN_GCC`), `--scaling` sweeps, `--external` corpora,
`cargo xtask libcxx`, `libstdcxx`, `clang-neg`, `gcc-torture`, `gxx-dg` and `features`.

**Build directory.** `source scripts/env.sh` before `cargo` picks the
toolchain, the jobserver and the target directory: one per checkout,
`<repo>/.build/target-<checkout basename>` (or the superproject's
`.build/` when kiln is a submodule), so git worktrees never share one.
`KILN_TARGET_ROOT=<dir>` moves them to `<dir>/target-<checkout
basename>` (opt-in; unset, the default applies), and `CARGO_TARGET_DIR`
overrides both.

**OSS builds** (ROADMAP M8). `scripts/oss/oss-build.sh <project>
[--compiler kiln|clang]` clones a project of `scripts/oss/projects.toml`
at its pinned tag into `.oss/src/`, configures it with CMake ("Unix
Makefiles", so `make` draws from the box's shared jobserver) with the
logging wrapper `scripts/oss/kiln-cxx` as `CMAKE_CXX_COMPILER`, builds with
`make -k`, runs its test suite, and writes
`.oss/build/<project>/<compiler>/report.json`; the wrapper caps every
compiler invocation (3 GB address space, 600 s) and logs one JSON line per
invocation to `compile.jsonl`. The same harness with `--compiler clang`
is the control build. `scripts/oss/oss-report.py --update
docs/status/oss.md` turns the two reports into the per-project table and
the kiln failure clusters (normalized diagnostic signature, count,
representative TU, reproduction command), which are reduced into
`tests/conformance` cases and work packages. Not in `ci` (it needs the
network and hours of CPU); run by hand or by agents; `.oss/` is
gitignored and only the regenerated status page reaches the repo.
`scripts/oss/selftest.sh` checks the scripts themselves; its
`--cmake-modules` mode runs only the CMake modules end-to-end (a
three-module Ninja build with kiln++ as `CMAKE_CXX_COMPILER`, then an
edit and rebuild), failing rather than skipping without CMake 3.28,
ninja or a built kiln++, which is how GitHub CI runs it.

## Conformance test format

A test is a single `.cpp` (or `.c`/`.h` for pp tests) with a directive
header. Directives are comments starting with `//@`. Example:

```cpp
//@ std: c++26
//@ expect: compile-fail
//@ diag: 6:15 error "static assertion failed"
//@ diag: 6:20 note "evaluates to"
constexpr int f(int x) { return x * 2; }
static_assert(f(2) == 5);
```

Directives (the full contract, including matching and outcome rules, is
[spec/harness.md](../spec/harness.md)):
- `expect: compile-ok | compile-fail | run | pp | ast | layout | vtables`
  (default `compile-ok`).
  `ast` compares the AST dump (spec/ast-dump.md) with the required golden
  `<test>.expected`. Under clang the JSON dump is normalized by
  `cargo xtask clang-ast`, which also writes new goldens. `ast-filter:
  <substr>` dumps only the matching declarations. `layout` and `vtables`
  have no golden: kiln's `-fdump-record-layouts` / `-fdump-vtable-layouts`
  must agree with clang's dump (see "Differential testing" below).
- `std: c++98..c++26 | c89..c23` (default `c++26`, `c17` for `.c`),
  `args: <extra flags>` (which may use the placeholders `%{repo_root}`,
  `%{resource_dir}` and `%{test_dir}`)
- `diag: <line>[:<col>] <error|fatal|warning|note|remark> "<substring>"`.
  Every listed diagnostic must appear; `strict-diags` makes the list exhaustive.
  "In order" was left open here and is now settled. Each primary diagnostic
  and its notes form a group. Groups match in any order, because clang can
  report a later line first (the preprocessor runs ahead of the parser). Notes
  must stay in order within their group. Only diagnostics located in the test
  file are considered. The column is optional.
- `stdout: <text>` / `stdout-file: <path>` / `exit: <code>` for `run`, and
  the program's inputs: `run-args: <args>`, `run-stdin: <text>` /
  `run-stdin-file: <path>`, `env: NAME=value`
- `oracle: clang | none` for `run`/`pp`/`ast` tests: also build with clang
  and require identical output (`clang` is the default for `run` and `pp`;
  for `ast`, clang's normalized dump must equal the golden)
- `pp-compare: tokens | text` and `pp-linemarkers` for `pp` tests: how the
  output is compared (token-wise by default) and whether line markers are
  kept (`-E` instead of `-E -P`) and compared
- `oven: [<line>[:<col>]] steps<=N bytes<=M ...` asserts a constexpr budget,
  on the whole TU or on the evaluations at one line of the test. It is
  kiln-specific and skipped elsewhere. kiln reports its counters with
  `-fconstexpr-report=<path>` (NDJSON, one line per evaluation and a total;
  the protocol is in spec/oven.md "Reporting protocol").
- `mem<=N` lowers the address-space cap of every process the test runs to
  N MiB (see "Resource limits" below).
- `skip: <reason>` / `xfail: <reason>` (tracked, must shrink over time).
  `skip(kiln):`, `skip(clang):`, `xfail(kiln):` and `xfail(clang):` apply to
  one compiler only. This lets the corpus be validated against clang while it
  still tracks kiln's gaps.
- `requires: target-<arch>` / `requires: clang>=X[.Y[.Z]]` skip a test whose
  precondition does not hold: the test's effective target (the last
  `--target` in its `args`, else the host) must be `<arch>`, and every clang
  the test runs (the compiler under test, and the oracle when the test
  compares with one) must be at least that version. `requires: host-sve`
  skips a `run` test unless the harness host's CPU has SVE (WP-810).

The directive header must come first; a `//@` line after it is an error.
A `.h` file is a test only if it has directives, and `Inputs/` directories
hold auxiliary files, never tests. A pp golden is `<test file>.expected`, for
example `foo.cpp.expected`. It is compared token-wise (or as normalized text
under `pp-compare: text`) and refreshed by `cargo xtask test --update`.

**Resource limits.** Every compiler, oracle and test-binary process runs
under an address-space cap (`RLIMIT_AS`): 8192 MiB by default
(`--mem-limit <MiB>`, `0` disables it), lowered per test by `mem<=N`. kiln
invocations get kiln's stack reservation (1 GiB by default) on top, so a
budget measures what the compile allocates. A process that runs out of
memory under the cap makes the test **KILLED**, a failure that `xfail`
cannot mask, counted in the summary line. A runaway compile (the AST
dumper cycle of 2026-10-31 OOM-killed the whole container twice) thus dies
alone. `cargo xtask bench` caps its measured compiles the same way. The
contract is spec/harness.md "Resource limits".

**libc++ memory exclusions.** The libc++ runner makes a test UNSUPPORTED
when `--mem-limit` is below what its run step needs (`MEM_UNSUPPORTED` in
`xtask/src/libcxx/run.rs`, spec/libcxx.md). Measured 2026-11-03 (WP-491;
per-process peaks from the runner's `gnu-time` probe, `scripts/memlog.sh`
alongside: no OOM event in the cgroup during either gate run):

| scope | tests | `MEM_UNSUPPORTED` rows | largest process | peak |
|---|---:|---:|---|---:|
| `--gate m3 -j 2` (llvmorg-19.1.7) | 642 | 0 | `mem.poly.allocator.mem/construct_types.pass.cpp` (kiln compile) | 76 MiB RSS |
| `--gate m4 -j 2` (llvmorg-22.1.8) | 2661 | 0 | `variant.visit/visit_return_type.pass.cpp` (kiln compile) | 690 MiB RSS |
| `stringstream.members/gcount.pass.cpp` (outside both gates) | 1 | 1 | its run step | 6660 MiB address space, 5122 MiB RSS |

The gcount run is not measured directly: it exceeds the 3 GiB
per-process budget of the shared host. A scaled copy (payload `2^k - 2`
bytes for `INT_MAX - 1 = 2^31 - 2`, then the test's two 1-byte writes),
built by kiln and by clang against libc++ 19 and 22, peaks at exactly
3.25 x 2^k bytes of live heap for every k from 20 to 29 (VmPeak 4 MiB
above that, VmHWM 2.5 x 2^k): the payload plus a `stringbuf` buffer
regrown from 0.75 x 2^k to 1.5 x 2^k bytes. At k = 31 that is 6656 MiB of
heap, 6660 MiB of address space (what `RLIMIT_AS` caps), and 5122 MiB
RSS, which matches the 5122 MiB WP-369 measured for the real run.

`cargo xtask ci` is the merge gate. After fmt, clippy and the unit tests
(each cargo step rerun once if it fails with rustc's `error[E0463]`, a
crate artifact that vanished mid-build) it runs the corpus twice, and fails
on any FAIL, XPASS, ERROR or KILLED in either run:

- `cargo xtask test --compiler clang++-22`: the corpus validates itself, so a
  test that clang 22 does not pass is caught before it is used to judge kiln
  (skipped with a note when clang 22 is absent, and under `--quick`; GitHub
  CI installs it);
- `cargo xtask test` with the kiln just built: kiln must pass everything it
  is not marked `xfail(kiln)`/`skip(kiln)` for, so a kiln regression in the
  corpus fails `ci`. This run is never skipped.

`cargo xtask ci --quick` drops the clang self-check for local iteration;
merges use the full form. The contract is spec/harness.md "CI". An oracle
that hangs or crashes makes the test an ERROR, never a kiln FAIL.

A test that hits a known oracle defect carries `skip(clang): <the defect>`,
so the self-check stays green and kiln keeps the coverage; a `run` test also
needs `oracle: none`, since its oracle compile is not skipped in kiln mode.
A timeout kills the compile's whole process group (the clang driver's `cc1`
included). The defects found so far, each with the test that records it:

- clang 19.1.0 to 19.1.2 answer `__has_builtin` with 0 for four builtins
  they implement (`__can_pass_in_regs`, `__reference_binds_to_temporary`,
  `__reference_constructs_from_temporary`,
  `__reference_converts_from_temporary`; llvm/llvm-project#111477, fixed in
  19.1.3). kiln answers 1, as 19.1.3 and later do. The probes live apart in
  `templates/builtins/has-builtin-{probe,staged}-gh111477.cpp`, which carry
  `requires: clang>=19.1.3`, so an older oracle skips them and the rest of
  the probe still runs. The kiln_pp `features` differential test excuses the
  same four answers from such a clang (`oracle_defect` in
  `crates/kiln_pp/tests/features.rs`).
- Ubuntu's clang 19.1.1 answers `__is_target_environment(gnut64)` with 1 on
  a `-gnu` target; Debian's clang and kiln answer 0. The `features` test
  excuses that one answer from an `Ubuntu clang version 19.` oracle.
- clang 19.1.7 never terminates on a use of an explicitly specialized member
  class template (`template<> template<class U> struct X<int>::M`) whose
  specialization has a default member initializer, and crashes when that
  initializer is dependent. kiln is not changed to match.
  `temp.expl.spec/member-class-template-spec-nsdmi.cpp`.
- clang 19 rejects a parenthesized aggregate initialization `B(1)` of
  `struct B { int i; S arr[2]; }` (`S` with a default constructor) when it
  is instantiated in a template (`array initializer must be an initializer
  list`), though it accepts the same code outside a template, as clang 22
  does in both ([dcl.init.general]/16.6.2.2 value-initializes `arr`). kiln
  accepts it. `dcl.init.general/paren-aggregate-array-of-class.cpp`
  carries `requires: clang>=22` (WP-316).
- clang 19 and 22 resolve `class B<T>::In::Deep` (a nested tag named
  through a dependent specialization, in another template) to the primary
  template's `Deep`: they accept a `B<int>` without `In` and a `B<char>`
  whose `Deep` is a union without a diagnostic, and crash in codegen
  (`FindInstantiatedDecl`) when the selected specialization's `Deep` is
  used. g++ 14 and kiln instantiate the specialization's member and reject
  the invalid ones (WP-1054).
  `temp.inst/member-elaborated-qualifier-nested{,-run}.cpp`.
- clang 19 rejects an importer's use of an exported class template's static
  data member defined out of class in the module interface (`declaration
  of 'x' in the global module follows declaration in module m`); clang 22
  and kiln accept it. `module.interface/export-member-template-var-run.cpp`
  carries `requires: clang>=22` (WP-1159).
- clang 22 crashes (`Sema::SubstBaseSpecifiers`) on a base-specifier pack
  expansion naming a generic lambda's own pack (`struct L :
  decltype(xs)...`) in a local class of the lambda inside a function
  template; clang 19 and kiln accept it.
  `temp.variadic/p5-base-pack-generic-lambda-local-class-run.cpp` carries
  `requires: clang<22` (WP-1733).
- clang 19 sometimes recurses without bound in `AutoType::Profile` (a
  segfault) on a placeholder type-constraint whose argument names an
  earlier *constrained* placeholder parameter (`template <any auto R,
  of<f(R)> auto O>`): profiling the reference to `R` looks up the
  unconstrained `auto` ([temp.over.link]/6), and when that lands in the
  folding-set bucket of `O`'s type, which depends on addresses, it
  profiles `O` again. A few percent of compiles of one such parameter
  crash; 400 of them crash nearly always. clang 22 and kiln are fine.
  `temp.deduct.type/constrained-placeholder-nttp-prior-param-constrained-ref-run.cpp`
  carries `requires: clang>=22`; its sibling without `-constrained-ref`
  keeps the shape with an unconstrained `R` for both oracles (WP-1254).
  Any clang oracle compile that dies of a signal is retried once
  (spec/harness.md "Oracle crash retry"), so a rare crash on some other
  test leaves an advisory instead of costing a CI run.

`cargo xtask libcxx` is deliberately outside `ci`. It runs libc++'s own
tests (a pinned upstream release, fetched by `scripts/fetch-libcxx-tests.sh`)
with kiln. It reimplements the subset of lit that the gates need: suffix
classification, `REQUIRES`/`UNSUPPORTED`/`XFAIL` with lit's boolean grammar
over a static feature set (plus the `locale.*` features, detected from
`locale -a`), `ADDITIONAL_COMPILE_FLAGS`, and `ALLOW_RETRIES`. Long tests
(`REQUIRES: long_tests`, or listed in `xtask/src/libcxx/slow_tests.txt`) run
under `--run-timeout-long` (240 s) instead of `--timeout` (30 s).
`scripts/provision-locales.sh` (Debian, as root; idempotent) generates the
seven locales libc++'s tests ask for (`en_US.UTF-8`, `fr_FR.UTF-8`,
`ja_JP.UTF-8`, `ru_RU.UTF-8`, `zh_CN.UTF-8`, `cs_CZ.ISO8859-2`,
`fr_CA.ISO8859-1`); without it about 150 localization, iostreams, `<regex>`,
`<format>` and `<chrono>` tests are UNSUPPORTED. Run it once per container
before a libc++ run (`spec/libcxx.md` "Feature set"). `--gate m3` is
the M3 exit gate. A run's `--report` regenerates the per-directory table and
the error-cluster table of a status page (`docs/status/libcxx-m3.md`). Its
Run table reports two memory numbers: **peak RSS**, the largest
single-process peak over every step of every test (the test binaries' runs
included, so a `large_tests` test that allocates gigabytes sets it), and
**kiln compile peak RSS**, the largest peak of a kiln++ compile step alone,
which is the one that tracks kiln's memory (`spec/libcxx.md` "Report"). It is
run by hand or periodically, and the pass rate is tracked there. The
contract is `spec/libcxx.md`. It builds kiln++ with the optimized `harness`
cargo profile by default (`--profile debug|harness|release`; `spec/libcxx.md`
"Build profiles"), and the rows of `docs/status/libcxx.md` and
`docs/status/libcxx-m3.md` come from `harness` runs from WP-319 on; each row
names its profile (earlier rows used a debug kiln++).

`cargo xtask libstdcxx` is the libstdc++ consumption track, also outside
`ci`. It compiles every public header of the system libstdc++ (the
extension-less files directly under `/usr/include/c++/<newest>`, so 109
headers for GCC 14; `bits/`, `ext/`, `tr1/`, ... are reached only through
them) as a one-line TU `#include <header>` with kiln
`-std=<std> -stdlib=libstdc++ -fsyntax-only`, and with `clang++-19` on the
same TU as the reference. Each compile runs under the harness's memory and
time limits and takes a jobserver token (default `-j 2`). A header is
`ok`, `fail` (error count and first error), `crash`, `timeout` or `killed`,
with wall time and peak RSS for both compilers; a kiln failure on a header
clang++-19 also rejects is counted apart, not as a kiln bug. `--report
<path>` splices the run's region (summary line, run table, headers table,
generated error clusters) into a status page between
`<!-- libstdcxx:<std>:begin/end -->` markers, so one page holds a region
per standard plus hand-written text; `docs/status/libstdcxx.md` is that
page, with the hand-written cluster triage and its corpus repros
(`libstdcxx-*.cpp` under `tests/conformance/`). The exit status is non-zero
only for a tool error (kiln did not build, a compile could not be run, the
report could not be written), never for header failures. The contract is
`spec/libstdcxx.md`.

`cargo xtask clang-neg` measures what kiln wrongly accepts, also outside
`ci` (WP-912). It reads clang's own negative tests from a checkout
(`scripts/fetch-clang-tests.sh`, gitignored `.oss/clang-tests`; never
vendored). It turns each `%clang_cc1 ... -verify` RUN line into one kiln++
`-fsyntax-only -verify` case per standard, mapping the cc1 flags to driver
flags and skipping, by counted reason, other targets, unimplemented
language modes (`-fms-*`, `-fobjc*`, `-fblocks`, modules, ...),
multi-step runs and unmet `REQUIRES`. The test is copied with every
`expected-error` text made unmatchable, so kiln's own clang-faithful
verifier lists each active expected error (after `#if`, prefixes, `@`
locations and counts) and each error kiln emitted, with file and line. A
case is ACCEPTS-INVALID when clang expects an error and kiln reports none.
An expected-error line with no kiln error is MISSED-ERROR. Errors in an
`expected-no-diagnostics` file are REJECTS-VALID. Crashes, timeouts and
OOM are counted apart. Wording is never compared. `--report` splices the
generated region (totals, per-directory rates, top 50 accepts-invalid
files, missed-error clusters, skips) into `docs/status/accepts-invalid.md`,
which also carries the hand-written triage and its xfail repros. The unit
tests use inline text, so `ci` needs no checkout. The contract is
`spec/clang-neg.md`.

`cargo xtask gcc-torture` measures kiln's C mode on GCC's own C tests,
also outside `ci` (WP-1277). It reads `gcc.c-torture` from a sparse
checkout of the GCC 16.1 release (`scripts/fetch-gcc-tests.sh`, gitignored
`.oss/gcc-tests`; GPL, never vendored), honours the tests' DejaGnu
metadata (`dg-do`, `dg-options`, `dg-additional-options`, `dg-skip-if`,
`dg-require-effective-target`, the xfail forms, `dg-timeout-factor`) and
the `.x` files of the c-torture-execute suites, and compiles, links and
runs each test at each option set (default `-O0` and `-O2`, `-std=gnu23`)
through kiln, GCC 16.1 and clang-19. Each compiler's result per case is
pass, compile-reject, crash, wrong (the program aborted), timeout or
accept-invalid; flags a compiler rejects are dropped for it and listed.
It writes a per-case TSV beside the oracles' results and a summary
(classes, pass rates by suite and option set, kiln against the oracles,
failure clusters ranked by count, skips); `--report` splices the summary
into `docs/status/c-mode.md`, which carries the hand-written triage and
the `tests/c/` xfail repros. The pieces are suite-independent, so a
g++.dg runner reuses them. The exit status is non-zero only for a tool
error. The contract is spec/harness.md "GCC testsuites".

`cargo xtask gxx-dg` does the same for GCC's C++ tests (WP-1443):
`g++.dg` (its directories with their own `.exp` driver, such as
`modules`, `lto` or `gomp`, are counted, not run), `g++.dg/torture` at
`-O0` and `-O2`, `c-c++-common` compiled as C++ and `g++.old-deja`, each
test at the standards GCC 16's `g++-dg.exp` picks for it (C++20, C++98 and
C++26 for a test that names none). Errors are matched by line and severity
against the test's `dg-error`, `dg-warning` and `dg-message` lines; the
message text is GCC's wording and is never compared. GCC 16.1 and clang
run the cases kiln does not pass, which splits rejects-valid and
accepts-invalid by what clang does. `dg-final` scans are GCC-internal and
counted as not applicable. `--report` splices the summary into
`docs/status/gxx-dg.md`, which carries the hand-written triage.

`cargo xtask features` is outside `ci` as well. It checks the language
feature table, `docs/STANDARD_SUPPORT.md`, which has one section per standard
(C++98/03 to C++26, C++26 reflection, DR-level papers, kiln extensions) and one
row per core-language feature with its status, its corpus tests and its
`__cpp_*` macro value in kiln and in clang 22 at that standard. It fails a `done` row that names no test, whose tests do
not all pass under kiln, or whose macro kiln defines differently from clang 22.
It also fails any row whose kiln macro column is stale, and a summary table
(the per-standard counts at the top) that disagrees with the sections;
`--summary` prints the counts. Run it by hand, and at
the M4 gate. The format and checks are in `spec/harness.md` "Feature table".

Each test file names the standard section it covers in its directory path,
and `spec/` entries list the tests that back them, so coverage of a spec is
a query, not a guess.

## GitHub Actions

Both oracles are gates (the two-oracle policy, below): `KILN_ORACLE=19
cargo xtask ci --quick --oracle-only` must be green too, and before a merge that touches
an expectation `cargo xtask test --compiler clang++-19 <dirs>` checks the
corpus against clang 19 as well.

`.github/workflows/ci.yml` runs the same gate as `cargo xtask ci` (full
form, clang self-check included) plus `KILN_ORACLE=19 cargo xtask ci --quick
--oracle-only` (only the oracle-dependent `cargo test` targets and the kiln
corpus; spec/harness.md "CI") on every push to `main`, on every pull request
and every merge-queue group (`merge_group`). A newer push to the same ref
cancels the older run (`concurrency: ci-<ref>`).

**Job graph** (WP-692). The gate is fanned out over parallel jobs, each a
slice of `cargo xtask ci` (`--step`, `--shard`, `--unit-group`;
spec/harness.md "CI"). `ci.yml` calls the reusable workflow
`.github/workflows/ci-lane.yml` once per architecture (callers `arm` and
`x86_64`, so the jobs show as `arm / kiln-22 2/6`), and each lane runs:

- `lints`: `ci --step lints` (the in-process lints: diag-split, Landed
  files and the static-gap inventory; then fmt, clippy);
- `unit <group>`, one per unit group (`xtask`, `codegen`, `rest`): `ci
  --step unit --unit-group <group>`, the `cargo test --workspace` split by
  package so no job runs more than about two minutes of tests (the
  `xtask` group alone is the seed-corpus tests). `unit codegen` also runs
  the `ORACLE_TESTS` again under `KILN_ORACLE=19`, where they are already
  built;
- `self-check K/2`: `ci --step self-check --shard K/2`, the corpus under
  clang;
- `kiln-22 K/6` and `kiln-19 K/6`: `ci --step kiln --shard K/6`, and the
  same under `KILN_ORACLE=19 ... --oracle-only`;
- `guard` (aarch64 lane only, WP-1901): the misbehaviour checks
  (spec/harness.md "Misbehaviour checks"). On a pull request `cargo
  xtask guard risky --base HEAD^1 --head HEAD` annotates the changes
  that run code at build time or change CI (a new or changed dependency
  in a `Cargo.toml` or `Cargo.lock`, a new `build.rs` or proc-macro
  crate, `.github/`, `.cargo/`) as `::warning::` lines and a job
  summary, without failing: kiln's own PRs make such changes on purpose,
  and a check every such PR overrides teaches reviewers to ignore it;
  the annotation puts the change in front of the reviewer instead.
  Then, after `cargo build -p kiln_driver -p xtask` (so cargo's own
  writes to `$CARGO_HOME` are not traced), `cargo xtask guard trace --
  target/debug/xtask test --compiler target/debug/kiln++ <slice>` runs
  a corpus slice (run tests, links and a link failure, module units;
  108 tests) under strace and fails on any socket a process creates
  (no family is allowed: nothing in the slice opens one) and on any
  write outside the target directory, `/dev/null` and `/proc/self`
  (the run's `$TMPDIR` is a fresh directory in the target directory, so
  a compiler's temporary files stay inside). The network namespace
  route (`unshare -n`) is not open to an unprivileged container, so
  strace is the only witness.

The shard count `S = 6` is one constant: the anchored matrix list
`&kiln-shards` in `ci-lane.yml`, which both kiln runs reuse; every job
passes `K/${{ strategy.job-total }}`, so the list length is `N`.
Sharding is round-robin over the sorted test list (test `i` runs in shard
`K` iff `i % N == K - 1`), which spreads every directory, slow or fast,
over all shards. The fan-in jobs `check` (needs `arm`) and `check-x86_64`
(needs `x86_64`) run `if: always()` and pass only when the lane's result
is `success`, so a failed, cancelled or skipped job fails them; they are
the status checks branch rules name. That is 18 jobs per architecture
(19 on aarch64 with `guard`) plus the fan-in, 39 per run: on a plan with a 60-job concurrency limit two
overlapping runs queue each other's jobs.

**Job timeouts** (WP-1600). Every job has a `timeout-minutes` of about
three times its usual duration, so a hung runner fails its job instead of
holding the merge queue for GitHub's default six hours (on 2026-11-17
`arm / kiln-19 4/6` sat 34 minutes without a log line until it was
cancelled by hand). The data: the 30 latest successful `merge_group` and
the 30 latest successful `pull_request` runs of `ci.yml` (2026-11-16 to
11-17; 2280 jobs, each from its start, so time spent waiting for a runner
is not counted), in minutes. A matrix job gets the timeout of its slowest
leg; the rule is three times that leg's median rounded up to 5 minutes,
and at least twice the longest run seen (10 minutes at least for a job
that checks out, 5 for the fan-in jobs). The 80 successful runs before
those (to 2026-11-11) stay under the same maxima (11.3 for `kiln-22`).

| job (`ci-lane.yml`, both lanes) | slowest leg | median | maximum | timeout |
|---|---|---:|---:|---:|
| `lints` | x86_64 | 2.2 | 3.0 | 10 |
| `unit <group>` | arm `rest` | 15.6 | 18.1 | 50 |
| `self-check K/2` | arm 2/2 | 3.0 | 3.8 | 10 |
| `kiln-22 K/6` | arm 1/6 | 8.3 | 11.0 (x86_64 1/6) | 25 |
| `kiln-19 K/6` | arm 6/6 | 7.2 | 10.1 (x86_64 6/6) | 25 |
| `check`, `check-x86_64` (`ci.yml`) | | 0.1 | 0.1 | 5 |
| `guard` (arm only, WP-1901) | arm | (no run yet) | | 25 |

`guard` had no hosted run when it landed; it is a kiln build plus a
traced slice of 108 tests, shorter than a `kiln-22` shard, so it got
`kiln-22`'s 25 until it has data.

**Token permissions** (WP-1901). Every workflow sets a read-only default
token, `permissions: contents: read` at top level. Caches and artifacts
use the runner's own token, and the merge queue needs only the checks,
so no job of `ci.yml` or `ci-lane.yml` needs more; `release.yml`'s
`release` job (the draft release) has `contents: write` of its own. A
job that needs more gets its own `permissions:` block and a row in
`ci_tests::WRITE_JOBS` (`ci_tests::workflows_default_to_a_read_only_token`).

The callers `arm` and `x86_64` cannot carry a timeout (a job that calls a
reusable workflow takes none); their jobs do. The other workflows' jobs
that had none got one by the same rule: `release.yml` `dist` 15 (median
4.7, maximum 6.2 over 12 legs) and `draft release` 10 (0.2); `sweep.yml`
`libcxx summary` and `gxx-dg summary` 10 (under 0.5), `oss summary` 10 (no
successful run yet; a summary like the other two). The sweep's long jobs
already had theirs (`build` 90, `libcxx` 240, `oss` 355, `gxx-dg` 150).
WP-1862 made `release.yml` `dist` a PGO build (`dist --pgo --check`:
an instrumented build, the training, the final build), about three
times the plain build, so its timeout became 45 (three times 15). The
sweep's new opt-in `pgo` job (two release builds, the instrumented one
and the bench of the held-out set) got 120, the `build` job's 90 sized
for a `dist --pgo` and some margin.
`ci_tests::every_workflow_job_has_a_timeout` keeps every job of every
workflow timed.

What a timeout does: the job ends as cancelled with "exceeded the maximum
execution time", its lane's result is not `success`, so `check` (or
`check-x86_64`) fails. In a pull-request run that is an ordinary red
check: re-run the failed jobs. In a merge-group run the required check
failed, so GitHub removes the pull request from the merge queue (the
groups queued behind it are rebuilt without it); re-running that
merge-group run does not put it back. The way back in: read the timed-out
job's log (no output for minutes is the runner, a test that loops is the
change), then add the pull request to the queue again (`gh pr merge <n>
--merge --auto`, or "Merge when ready"), which starts a new merge group.
If the durations grow (a new corpus directory, a slower runner image),
remeasure and raise the timeouts with the table.

**Cache policy.** `Swatinem/rust-cache` keeps `~/.cargo` and the
dependency builds under one `shared-key` per architecture (`ci-arm`,
`ci-x86_64`), so every job of a lane restores the same entry (about 12 MB,
a couple of seconds). Only `unit rest` saves it (`save-if`), with
`cache-on-failure`; an exact hit is not saved again, so the entry changes
only with `Cargo.lock`, the toolchain or the environment. Workspace
crates are deliberately not cached (`cache-workspace-crates` stays off):
a fresh checkout gives every source a new mtime, so cargo rebuilds them
anyway; restoring commit times instead would be unsound, because a
pull request's commit can be older than the cached build and cargo would
then reuse a stale artifact. Each job therefore rebuilds the workspace
(about a minute), in parallel with every other job.

The job runs in a `gcc:16.1.0-trixie` container on the `ubuntu-24.04-arm`
runner: the same aarch64 Debian 13 with Debian's clang 22.1.8 and 19.1.7,
GCC 14 and libc++ 19 as the development container, plus GCC 16.1 in
`/usr/local` (the image is `debian:trixie` by way of `buildpack-deps`).
GCC 16.1 is only the `oracle: gcc` reference (`KILN_GCC`): clang and
kiln both still detect Debian's GCC 14 under `/usr/lib/gcc/<triple>/14`
for libstdc++, because neither looks in `/usr/local`. Its
`/usr/local/bin/gcc` is `cc` on `PATH` (Rust links with it), and the
image's `ld.so.conf` puts its libstdc++.so.6 first for every process; both
are compatible with what Debian's GCC 14 builds. The first action step
checks the GCC version and compiles a `<meta>` TU with it
(docs/status/perf-history.md "WP-502: GCC 16.1 in CI" has the cost). The packages are
installed in the first step (`clang-19 llvm-19 lld-19 libc++-19-dev
libc++abi-19-dev clang-22 llvm-22 lld-22`, plus `build-essential procps
time`). `libc++-22-dev` conflicts with `libc++-19-dev`, so libc++ 22 is
an extracted package tree under `.toolchain/libcxx-22-root` that only the
M4 libc++ gate uses (spec/libcxx.md "Toolchain"); the corpus does not
need it. The environment is pinned because the
differential tests depend on the host as well as on clang. They check
search paths, `__has_builtin`/feature tables, target defaults and the
host's `va_list` against goldens recorded with that clang on that host.
**Oracle-version assumption:** the reference oracles are Debian's clang
22.1.8 and 19.1.7, and kiln models clang 22.1 and clang 19.1.3 and later. An older 19.1 (Ubuntu
24.04 ships 19.1.1) is tolerated only through the oracle defects listed
above ("The defects found so far"): the corpus self-check and the kiln run pass
with it (the GH111477 probes skip), but it is not what CI runs.
`KILN_REQUIRE_LLVM=1` turns a missing LLVM tool into a failure instead of a
skip.

**Debuggers** (WP-816). The corpus jobs (`self-check`, `kiln-22`,
`kiln-19`) and `unit xtask` pass `debuggers: "true"` to the setup
action, which then also installs `gdb lldb-19` (Debian trixie's gdb 16.3
and lldb 19.1.7; a few seconds of apt per job, cheaper than a dedicated
job that would rebuild kiln). Those jobs run their container with
`--cap-add=SYS_PTRACE`. The lane env sets `KILN_REQUIRE_DEBUGGERS=1`, so
an `expect: debugger` test whose debugger is missing is an ERROR there,
never a SKIP; `ci_tests::workflow_installs_the_debuggers` checks that
every corpus job installs them. Each debugger session gets at least 60 s
(spec/harness.md "Debugger tests").

**Capability tests that must not skip in CI** (WP-1034). What runs
where, per lane (`arm` and `x86_64` alike):

- *Sanitizer runtimes*: the corpus jobs pass `sanitizer-runtimes:
  "true"`, which installs `libclang-rt-19-dev libclang-rt-22-dev`
  (compiler-rt for both clangs, about 30 MB each), and the lane env sets
  `KILN_REQUIRE_SANITIZER_RUNTIME=1`, so a `requires: sanitizer-runtime`
  test (the ASan/UBSan `run` tests, `tests/conformance/driver.sanitize`)
  whose clang finds no runtime is an ERROR. They run in the shard that
  owns them in `self-check`, `kiln-22` and `kiln-19`.
- *CMake modules end-to-end*: `kiln-22 1/6` passes `cmake: "true"`
  (Debian trixie's CMake 3.31 and ninja 1.12) and, after its corpus
  step, runs `scripts/oss/selftest.sh --cmake-modules` against the
  `target/debug/kiln++` that step built. It is only in one shard: it
  needs no oracle, and a second run would repeat the same build.
- *Mangling differential*: `unit codegen` runs `cargo test -p
  kiln_codegen --test mangle_vs_clang -- --nocapture` after the rest of
  `kiln_codegen`'s tests, printing per triple the files and clang
  symbols checked; `KILN_REQUIRE_LLVM=1` makes a missing clang++-19 a
  failure. Locally, `cargo xtask ci` (and `--quick`) runs the same
  command and `cargo xtask test tests/mangle` runs it after the corpus
  (spec/harness.md "The mangling differential").

`ci_tests::workflow_installs_the_sanitizer_runtimes` and
`workflow_runs_the_cmake_modules_e2e` keep the workflow honest.

The `x86_64` lane runs the same jobs in the same container on the
x86-64 `ubuntu-24.04` runner (WP-228), so the host-dependent paths
(default target, GCC detection, `run` tests compiled for and run on
x86-64) are exercised too. Tests whose expectations are aarch64-specific
pin `//@ args: --target=aarch64-unknown-linux-gnu` (compile-only kinds) or
carry `//@ requires: target-aarch64` (`run` tests, which cannot cross-run;
spec/harness.md). The `-v` search-list test compares with clang++-19 live
rather than with the reference container's goldens. `check` is the
required status check; whether `check-x86_64` is too is a branch-rule
decision. Every job checks out (the image has git) and runs the
setup-only composite action `.github/actions/kiln-ci` (toolchain
packages, the GCC check, Rust 1.95.0, the cache), so the setup is
written once.
`ext.c11.atomic/alignment-warning-x86_64-run.cpp` is the x86-64 twin of an
aarch64-only `run` test (WP-239).

Before WP-692 the whole gate ran serially in one job per architecture,
18 minutes on aarch64 and 26 on x86-64; the fanned-out times and the
per-job floor are in docs/status/perf-history.md "WP-692: CI fan-out". Locally,
`cargo xtask ci` is the same gate. Run
it before pushing `main`, because Actions only reports after the push, and
the local run is still the merge gate (docs/AGENTS.md rule 3).

### Tier-0 (outside pull requests)

A pull request from a fork runs untrusted code, so it does not get the
full CI straight away. A pipeline outside this repository screens it: a
mechanical prefilter, then **tier-0**, then a static triage, then the full
CI (`ci.yml`). The repository's "Require approval for all external
contributors" holds every workflow run of such a pull request; the pipeline
approves only the tier-0 run for a head sha and reads only its conclusion
(the contract is in spec/harness.md "CI", "Tier-0").

**What it runs** (WP-1902, `.github/workflows/tier0.yml`): two parallel
jobs on `ubuntu-24.04` (x86_64 only), in the `gcc:16.1.0-trixie`
container with the `kiln-ci` setup action, like `ci-lane.yml`'s jobs:

- `tier0 lints`: `cargo xtask ci --step lints` (the step-0 lints, then
  `cargo fmt --all --check` and clippy with `-D warnings`), the same as
  the full CI's `lints` job;
- `tier0 unit`: `cargo test --workspace --lib --exclude xtask`, the
  library unit tests of every crate. The full `unit` step is about 25
  runner-minutes on x86_64 (`unit rest` alone 20, nearly all of it the
  crates' integration tests in `tests/*.rs`, which compile and run C++;
  `unit xtask` 4, mostly the seed-corpus run), too slow for a screen. The
  library tests are about two of those minutes (`kiln_driver`'s own 2,
  every other crate's a few seconds) and build no test binary per
  integration file. No corpus run either: that waits for the triage.

**Cost.** A hosted pull-request run of 2026-11-21 (x86_64): container
and setup about 1 minute per job; `lints` 1.5 minutes; the full CI's `unit
rest` built every test target in 1.9 minutes from the `ci-x86_64` cache,
and the library tests ran about 2.1 minutes of it. Expected wall time is
therefore about 4 minutes for `tier0 lints` and 5 to 6 for `tier0 unit`,
in parallel, so 5 to 6 minutes per run and about 10 runner-minutes, well
inside the 10 to 15 minute target. Locally (24-core aarch64 container,
shared 8-token jobserver, cold target directory): `ci --step lints` 73 s
(clippy 39 s), building the library tests 45 s, running them 100 s
(`kiln_driver` 81 s). The
timeouts (15 and 25) are about three times the expected durations, as in
"Job timeouts". Tier-0 restores the `ci-x86_64` cache (a pull request can
read the default branch's entries) and never saves one.

**Why a separate file.** The pipeline matches the run to approve by its
workflow file path, so `.github/workflows/tier0.yml` must keep its name
and hold only tier-0; folding it into `ci.yml` would make the approval
start the whole fan-out. It triggers only on `pull_request`, with
`permissions: contents: read`, no secret, no `pull_request_target` or
`workflow_run`, no artifact and a timeout on every job
(`tier0_tests` in xtask checks each, and that a mutated copy fails).
Concurrency is one group per pull request (`tier0-<number>`, cancel in
progress), never one shared group: GitHub keeps one pending run per group
and cancels older pending ones, which would silently drop pull requests.
Rate limiting is the pipeline's job.

**Same-repo skip.** Both jobs carry `if:
github.event.pull_request.head.repo.full_name != github.repository`: a
pull request from a branch of this repository (the maintainers') is not
screened and gets the full CI, which runs everything tier-0 does, so its
tier-0 run shows two skipped jobs and costs nothing. The test is on the
head repository, not the author, so every fork's pull request runs it.

What only a real fork pull request proves: that the approval and the run
happen, that the `if:` admits the fork, that the cache restores across
the fork boundary, and the actual wall time. A same-repo pull request
only shows that the file parses (its jobs are skipped).

### x86_64 sweep

The development container is aarch64 and cannot run x86-64 code. It can
compile for `--target=x86_64-linux-gnu` against libstdc++ 14 when Debian's
`libstdc++-14-dev-amd64-cross` and `libc6-dev-amd64-cross` are installed
(the driver finds their `gcc-cross` installation as clang does,
spec/driver.md "Toolchain"), which is how x86-64-only header paths are
checked locally (WP-1002). The
heavy suites (libc++, the OSS corpus) run on x86-64 only through
`.github/workflows/sweep.yml` (WP-961) on the `ubuntu-24.04` runners, in
the same `gcc:16.1.0-trixie` container as the gate. It is never a merge
gate: it has no `pull_request` or `merge_group` trigger and is not a
required check (`ci_tests::sweep_never_gates` checks the triggers). Its
jobs:
- `build`: `cargo xtask dist` once, `dist --pgo` for a `sweep/rc-*`
  push (the shipped kind, WP-1862); every other job installs that kiln++
  into `/opt/kiln`, so what runs is the release archive's binary. The
  dist's `build-info.txt` (plain or PGO, and the profile) and a `commit`
  file are artifact `kiln-build`, which the release gate's `import-x86`
  checks.
- `libcxx K/6`: the libc++ 22.1.8 suite (`std` + `libcxx`, 9247 tests) as
  the aarch64 M5 run takes it (docs/status/libcxx.md "M5"), with libc++ 22
  extracted into `.toolchain/libcxx-22-root` and the lit locales
  generated, split by `cargo xtask libcxx --shard K/6`; each shard
  uploads its `--results` file and its `--report` page.
- `libcxx summary`: `scripts/libcxx-results.py` merges the shards into the
  run's summary page (counts, pass rate, failing tests).
- `oss matrix`: the oss jobs' two lists, as JSON outputs, the one place
  they are spelled (spec/landed/harness/WP-1265.md): the projects are the
  `sweep = true` rows of `scripts/oss/projects.toml` (`projects.py list
  --sweep --json`; add a project to the sweep by setting the key on its
  row), the compilers kiln and the clang 19 control on a push or a
  dispatch with `control`, else kiln alone.
- `oss <project> <compiler>`: `scripts/oss/oss-build.sh` for each project
  and compiler of `oss matrix` (ten projects), on the same runner, trees
  on the runner's `/mnt`; each uploads its
  `report.json`, `compile.jsonl`, `configure.log` and `test.log`. The
  job fails only when `oss-build.sh` wrote no `report.json`: the project
  was not measured (a failed clone or download, retried once, or a
  runner failure; spec/landed/harness/WP-1241.md). A build or test
  failure is in the report and the job stays green.
- `oss summary`: `scripts/oss/oss-report.py --expect` over all of them,
  its expected projects and compilers read from `oss matrix`'s outputs,
  as the matrix's are (`ci_tests::sweep_never_gates` checks both).
  It opens with how many of the matrix's project/compiler pairs reported
  and lists each one without a report as "no report (infrastructure)",
  never leaving it out. A release candidate's sweep needs every pair
  reported.
- `gxx-dg K/4` (opt-in, WP-1473): GCC 16.1's C++ testsuite
  (spec/harness.md "The g++.dg runner", docs/status/gxx-dg.md "x86-64")
  through the `build` job's kiln++ (`--compiler /opt/kiln/bin/kiln++`),
  with the image's GCC 16.1 (`KILN_GCC`, whose libstdc++ kiln and clang
  build against) and the `kiln-ci` action's clang 22 as oracles, at the
  runner's default limits and `-j 4`, split by `cargo xtask gxx-dg --shard
  K/4` (about 14,200 cases each); each shard uploads its TSV, summary and
  log as artifact `gxx-dg-K`.
- `gxx-dg summary`: `scripts/gxx-dg-compare.py merge --expect 4` joins the
  shard TSVs into `gxx-dg-x86_64.tsv` (a missing shard fails the job
  rather than shrinking the suite) and `summary` writes the headline,
  classes and buckets; artifact `gxx-dg-summary`. Compare it with an
  aarch64 run at the same commit by `scripts/gxx-dg-compare.py compare`.
- `pgo <arch>` (opt-in, WP-1862): the PGO measurement on both runner
  architectures (spec/harness.md "PGO build", measuring it): a plain
  `cargo xtask build` copied aside, `cargo xtask build --pgo`, then
  `cargo xtask bench --compiler <PGO> --compare-with <plain>` over the
  held-out set; artifact `pgo-<arch>` (results JSON, bench output, build
  log and times). A failed entry fails it, except one over its
  `budget-time:` (a development-host budget that both binaries can
  break on a slower runner). It runs for a dispatch with the `pgo` input or a push
  of a `sweep/pgo-<name>` branch, which runs it and nothing else (the
  `build`, libc++ and OSS conditions exclude `sweep/pgo-` refs).

The g++.dg jobs run only when asked: a dispatch with the `gxx` input
(default false), or a push of a `sweep/gxx-<name>` branch, which runs
`build` and the g++.dg jobs and none of the libc++ and OSS jobs (their
conditions exclude `sweep/gxx-` refs). Any other `sweep/` push, and a
dispatch with the default inputs, runs exactly what it ran before
(`ci_tests::sweep_never_gates` checks the conditions).

Run it before a release (docs/STABILITY.md), and after a change to ABI
lowering, calling conventions or the driver's target defaults: push the
release candidate's commit to a `sweep/<name>` branch (`git push origin
<sha>:refs/heads/sweep/rc-<version>`), or start it from the Actions tab
(workflow_dispatch, which can skip the libc++ part, the OSS part or the
clang control, and add the g++.dg jobs). A `sweep/gxx-<name>` branch runs
the g++.dg suite alone. A newer push of the same branch cancels the older run.
Wall time is about the slowest OSS job's: gRPC, about 2 h 20 min under
either compiler, inside the 355-minute job limit. Compare the libc++ summary with
the aarch64 numbers (`scripts/libcxx-results.py --baseline <aarch64
results>` marks the x86_64-only failures), record the run in
docs/status/x86_64-sweep.md, and delete the branch. The first run and its
triage are in that page.

## Differential testing against clang

`clang-22` is the oracle for everything that is not a kiln-specific feature,
and `clang-19` is the second oracle.

The oracle release is selected by `KILN_ORACLE` (`22`, the default since
WP-355, or `19`, the release the M3 gate and its numbers stay on). It names `clang++-N`, `opt-N`, `llc-N`, `FileCheck-N` and
`/usr/lib/llvm-N` in one place (`xtask/src/oracle.rs`); `xtask test` and
`ci` use `clang++-N` as the oracle and self-check compiler and run kiln++
with `KILN_LLVM_DIR=/usr/lib/llvm-N`, `ir_vs_clang` runs `clang++-N`/`opt-N`,
and `xtask libcxx --tag` picks a libc++ test checkout
(`.toolchain/libcxx-<tag>`, one per release). Contract: spec/harness.md
"Oracle toolchain".

**Two-oracle policy (WP-355).** The corpus is green under both releases.
Goldens, `//@ diag:` wording and the IR-diff baseline are clang 22's. An
expectation the two releases disagree on is pinned with `requires:`:
`clang>=22` when clang 22 is the contract (a C++26 feature clang 19 lacks,
reworded diagnostics such as `'__size_t' (aka 'unsigned long')` or
`(lambda)` in instantiation notes, re-sugared AST dumps), with `xfail(kiln)`
and a reason where kiln lacks the feature; `clang<22` when kiln has not
followed clang 22 yet (the residual list in
docs/status/wp-184-oracle-switch.md, each a follow-up). A kiln-only test
pinned to one release runs kiln at that release's compatibility level under
either oracle, so kiln's clang-22 behaviour stays tested in the 19 run and
vice versa. Behaviour changes between the releases (not wording) cite the
standard in the test header. The M3 libc++ gate stays on 19; M4+ gates run
on libc++ 22.

**Clang compatibility level.** kiln reproduces one clang release's
predefined macros and `__has_builtin`/`__has_feature`/`__has_extension`/
`__has_attribute`/`__has_cpp_attribute` answers, chosen by
`-fclang-compat=<major>`: 19 (kiln++'s own default) or 22 (spec/pp.md "clang
compatibility level"; WP-263). The test runner passes the oracle's major
version, so `KILN_ORACLE=22` (or `--oracle clang++-22`) compares kiln at
level 22 with clang 22 and headers take the same `#if` branches under both. The
`__cpp_*` feature-test macros are not part of the level: they follow what
kiln implements (docs/STANDARD_SUPPORT.md). Builtins that only clang 22
has are tested by `requires: clang>=22` tests (WP-355), checked against
clang++-22 by the self-check.
- `-E`: token-wise comparison after both outputs are re-lexed with
  `kiln_lex`. Identifiers and literals compare by spelling, punctuators by
  kind (a digraph equals its primary spelling), and whitespace is ignored.
  Tests run with `-E -P` by default; a `pp-linemarkers` test keeps the line
  markers and compares the include enter/leave events and every token's
  presumed file and line. Printer tests that care about spacing opt into
  `pp-compare: text`. The rules are in spec/harness.md "pp comparison".
  The preprocessor corpus is `tests/differential/pp/`:
  - one generated test per header (`cargo xtask gen-pp-corpus`): every
    libc++ 19 public header including `experimental/`, the C headers libc++
    wraps, and the top-level `/usr/include/*.h` headers it does not. This is
    the M1 exit criterion: `kiln++ -E` must agree with `clang -E` on all of
    them;
  - `torture/`: hand-written tests of the hard corners (hide sets,
    placemarkers, `__VA_OPT__`, stringizing, `#include` of macro-expanded
    names, `__COUNTER__`/`__LINE__`, `#line`, `_Pragma`, include guards and
    `#pragma once`, `#embed`, UCNs, raw strings, digraphs, line splices,
    files without a final newline, line markers), with goldens. Their intent
    follows clang's `clang/test/Preprocessor/`, written independently.

  `cargo xtask test tests/differential/pp --report-parity
  docs/status/pp-parity.md` refreshes the parity table on
  `docs/status/pp-parity.md`.
- `run`: stdout and exit code must match a clang-built binary.
- layout: `kiln++ -fdump-record-layouts` vs clang's for every class in the
  corpus; mangled names vs clang's for every external symbol.
  `expect: layout` and `expect: vtables` tests make this a corpus feature:
  the harness runs kiln with `-fsyntax-only -fdump-record-layouts` (or
  `-fdump-vtable-layouts`) and clang with `-fdump-record-layouts-complete`
  (or `-fdump-vtable-layouts` while emitting IR, because clang dumps only
  the vtables it emits, so every dynamic class needs a key function defined
  in the test), splits both with `kiln_abi::dump_blocks`, and compares record
  layouts block by block in order and vtables as a set of blocks (clang's
  `Original map` sections and repeated construction vtables dropped). The
  first differing block is reported with both texts. Without an oracle the
  tests are a SKIP. The `kiln_driver` differentials
  (`tests/record_layouts.rs`, `tests/vtable_layouts.rs`) share the same
  splitter.
- constexpr: a generator (`cargo xtask fuzz-constexpr`) produces random
  well-formed constexpr programs that print their results at runtime *and*
  `static_assert` them; both compilers must agree.

### Mangling sweep

`scripts/mangle-sweep.sh` checks symbol parity with clang++-19 over real
standard-library code. Each TU in `tests/mangle-sweep/` instantiates one
area of the library (algorithm, format, functional, iterator, memory,
optional, ranges). The script compiles it to LLVM IR with kiln++ and with
clang++-19, once against libc++ and once against libstdc++, runs `opt-19
-passes=always-inline` over both IRs, and compares the `_Z` symbols each
defines, declares or references. It prints one line per mismatch
(`<stdlib> <tu> kiln-only|clang-only <symbol>`) and a summary, and exits
1 on any mismatch (2 when a compiler failed).

```sh
source scripts/env.sh && cargo build -p kiln_driver
scripts/mangle-sweep.sh                       # both libraries, every TU
scripts/mangle-sweep.sh --stdlib libc++ --keep /tmp/sw tests/mangle-sweep/format.cpp
```

The sweep is run by hand, not in CI: the clang-only symbols that remain
are calls clang 19 emits and kiln does not (spec/abi.md §8.29), so the
expected output is not empty. A change to the mangler, to template
instantiation or to attribute inheritance should not add lines. Add a
TU when a library area that real code uses is not covered. Every
mismatch fixed gets a reduced test under `tests/mangle/` (or
`tests/codegen/` when it is about emission).

### GCC 16.1 header sweep

Real reflection code needs GCC 16.1's libstdc++ (its `<meta>`). CI has
no GCC 16 tree, so this sweep is run by hand. `KILN_GCC16_ROOT` is the root
of an unpacked GCC 16.1 image: headers under
`$KILN_GCC16_ROOT/usr/local/include/c++/16.1.0`, the installation under
`$KILN_GCC16_ROOT/usr/local/lib/gcc/<triple>/16.1.0` (with `crtbegin.o`).
The driver finds it with clang's `--gcc-toolchain=` (spec/driver.md
"Toolchain"); reflection, on by default at C++26 since WP-920 (the
`-freflection` below is now a no-op kept for history), predefines
`__cpp_impl_reflection`, which `<meta>` keys on.

```sh
GCC16=--gcc-toolchain=$KILN_GCC16_ROOT/usr/local
# the probe: one TU including every standard header that reflection-heavy
# code commonly uses (<algorithm> ... <vector>, <cxxabi.h>, <meta>)
kiln++ $GCC16 -std=c++26 -freflection -fsyntax-only probe.cpp
# the same with a main that instantiates (std::meta::is_type(^^int),
# std::vector<int>{1}.size()), and a TU that instantiates std::format,
# unordered_map, optional<string>, variant<int, string>, std::sort,
# set<int> and names std::thread, under -fsyntax-only and under
kiln++ $GCC16 -std=c++26 -freflection -S -emit-llvm tu.cpp -o tu.ll
llvm-as-19 tu.ll -o /dev/null && llvm-as-22 tu.ll -o /dev/null
# every public header alone, against clang++-19 on the same headers
cargo xtask libstdcxx --gcc-toolchain $KILN_GCC16_ROOT/usr/local
```

All must be free of errors (`-ferror-limit=0` to count them); record the
wall time and peak RSS (`/usr/bin/time -f '%e s %M KB'`) when a change
moves them. The WP-525 baseline and result are in
`spec/landed/driver/WP-525.md`.

### Header-differential mode (real `<meta>`)

kiln's reflection tests include `tests/support/kiln_meta.h`, kiln's own
declarations of `std::meta`. Real code includes libstdc++
16.1's `<meta>`, whose library code (`access_context`, `exception`,
`define_static_array`, the `vector<info>` and `string_view` results)
calls kiln's intrinsics in ways kiln's header does not. The
header-differential mode reruns the same tests through the real header
(spec/harness.md "Header-differential mode"):

```sh
source scripts/env.sh   # KILN_GCC = g++-16 (its prefix is the toolchain)
cargo xtask test --meta-header -j 4 tests/conformance tests/stress
```

Every reflection test that includes `kiln_meta.h` runs, unless it uses a
kiln extension, compares the header's declarations (`ast`, `ir`), or
opts out with `meta-header: skip <reason>`; the skip reasons are printed
with `-v`. The mode must pass (no FAIL or ERROR) before a change to
`<meta>` intrinsics, `kiln_meta.h` or the driver's libstdc++ handling
lands, and for a release candidate. It is not in `cargo xtask ci`: the
`oracle: gcc` tests that name `--gcc-toolchain=%{gcc_toolchain}` already
compile against the real header in CI, where GCC 16.1 is the image's.

Separately, `crates/kiln_oven/src/meta/gcc16_meta_names.txt` lists every
`consteval` function of the real `<meta>` as `intrinsic` (a kiln
`meta_family!` row) or `library` (GCC's header defines it over the
intrinsics); `cargo test -p kiln_oven` checks that every intrinsic has a
row, and with `KILN_GCC16_ROOT` set, that the list equals the header's.
A new GCC release's `<meta>` starts there.

### Debug-info sweep

`-g` must never break a program kiln compiles (WP-910). The debug-info
sweep compiles every positive test (`compile-ok`, `run`, `ir`, `layout`,
`vtables`, `debugger`) to an object with and without `-g` and runs
`llvm-dwarfdump --verify` on the `-g` object (spec/harness.md
"Debug-info sweep"). A test that compiles without `-g` and fails with it
is a FAIL, and so is one whose compile without `-g` crashes (a panic, an
internal compiler error, a signal: WP-1563); an IR test is compiled with
its own arguments minus `-disable-llvm-passes`. On `tests/codegen` the
sweep went from 645 passed, 25 failed, 19 skipped to 668 passed, 0
failed, 21 skipped (the two new skips name LLVM: clang crashes on the
same test, or `llvm-dwarfdump` rejects clang's object too):

```sh
source scripts/env.sh
cargo xtask test --debug-sweep tests/conformance/expr.reflect 'tests/conformance/meta.reflection*'
cargo xtask test --debug-sweep tests/conformance      # the whole corpus
```

Run it by hand before a change to `kiln_codegen::debuginfo`, to the
printer's metadata, or to anything that can reach debug information
from new AST (reflection, templates), and for a release candidate. It is
not in `cargo xtask ci`: on the shared 24-core host (load about 18) the
reflection directories take 83 s and the whole corpus 5 min 25 s (5192
tests: 2539 swept and clean, 2431 not swept by kind, 76 that do not
compile to an object without `-g`, the rest skipped by `requires`; peak
RSS 253 MB). Every breakage it finds gets a reduced `-g` run test under
`tests/conformance` (spec/landed/codegen/WP-910.md). On 2026-11-14
(WP-1424, load about 35) the whole corpus took 17 min: 6548 tests, 3091
swept and clean, 2964 not swept by kind, 243 that do not compile to an
object without `-g`, the rest skipped by `requires`.

What else checks `-g` IR: every `tests/codegen/debug_*.cpp` case goes
through `opt-N -passes=verify` in `ir_vs_clang` (kiln's IR always, the
oracle's unless a deviation lists it as invalid), and a `run` test with
`//@ args: -g` hands its text to the backend, which parses it. The
printer defines every identified struct that any printed metadata
constant names (spec/llvm.md §2.1), so a type reached only through
debug information cannot be left undefined again; before WP-1424 one
reached only from a `#dbg_declare` record was (a class-type template
argument of a local's class, `libcxx/test/std/utilities/function.objects/
func.not_fn/not_fn.nttp.pass.cpp` with `-g`), and no corpus test had
that shape.

## Performance tracking

`cargo xtask bench` runs each entry in `bench/` and records wall-clock
(median of N), max RSS of the child (via GNU time's `%M`, since safe Rust
cannot call `getrusage` for a child), and, for kiln, the Oven's own constexpr
counters from its `-fconstexpr-report` total, into `bench/results/<date>.json`
(git-ignored local history); `bench/baselines/` holds the committed reference,
one per entry and compiler kind. A run fails if any entry regresses beyond its
threshold (default 5% time, 2% memory, 0% steps, 2% constexpr bytes; an entry
can set its own with `threshold-*:` directives, and `--threshold-*` options
override both). Kiln entries of `frontend`, `oven`, `overload` and
`reflect` are also judged on kiln's accounted memory, the
`-fkiln-memory-report` total from one extra unmeasured compile, at 2%: it
is deterministic where peak RSS moves 2-6% between identical runs, it
sees kiln alone where an `obj` compile's largest process is `llc`, and it
counts the Oven's evaluation memory by its exact peak (`oven.heap-peak`),
so memory reclaimed during an evaluation still shows (WP-1303, WP-1315;
spec/bench.md "Regression policy"). The counters are deterministic, so they catch evaluator
regressions that host noise would hide in the timings, and `oven:` budgets
on bench entries are enforced too, as is a `budget-time:` directive: an
absolute ceiling on kiln's fastest run that holds even under `--record`, so a
re-recording cannot absorb a regression past it (`bench/pp/headers.cpp`
carries one at 1.5x its recorded kiln time, WP-487). The time rule compares the fastest current run
with the baseline median so host load does not read as a regression, and a
baseline taken with a different compiler version, host or entry source is
advisory only, except that kiln is always compared with its own past:
two kiln versions count as the same compiler (WP-1315). The same corpus is run through clang to produce the
comparison table in `docs/status/perf.md`. The `libcxx-slice` entry
(`bench/libcxx-slice/slice.list`, selected only when named) times kiln
against clang 22 on ~50 heavy libc++ 22 tests at `-fsyntax-only`, with the
flags the libc++ runner gives each test, and reports per-test and aggregate
ratios (time median, geometric mean, p90 and max; memory median and p90)
with the box load. Its oracle must be clang 20 or later: libc++ 22 rejects
older clangs under the runner's `-Werror`, so a `--compare-with clang++-19`
(or a clang 19 `--compiler`) would fail every test, and `xtask bench` refuses
it with an error (exit 2) before measuring anything (WP-487). It is record-only, because only the ratio measured in one
window means anything on this shared host, and its section of `perf.md` is
re-recorded at each milestone. The reflection shapes (`bench/reflect/`) are
measured the same way when named, and `--scaling <shape>` checks their
growth from N to 4N (4N/N above 4.6x on wall or RSS fails; a growing
`peak_arena` fails on the shapes whose evaluations do not depend on N);
both are off-queue, because a ratio of wall times needs a quiet host. The
`pch` group (`cargo xtask bench bench/pch`, selected only when named, about
3 minutes) measures kiln's precompiled headers against clang 19's on four
header sets: no PCH, PCH build and PCH load on the default path (the
snapshot), and replay (`-fno-kiln-pch-snapshot`). It reports the
path kiln's consumer took and judges kiln on memory only against
`bench/baselines/pch.json`. `--record` regenerates the numbers in
`docs/status/pch.md`. It runs by hand after a PCH change and before a
release, not in CI. The contract is `spec/bench.md`.

`-fmason-metadata`'s kind 2 (the definition hash) has a cost limit the
harness does not record: at most 3% of the front end's callgrind
instructions with the flag off, measured on `bench/frontend` and small
`libcxx-slice` units under `ulimit -v 3000000` (the table and the method
are in spec/codegen.md §5.5 "Cost", WP-1586). It is re-measured by hand
whenever kind 2's hash changes.

## Release candidates

Every tag is preceded by the release gate: the release archive, both
halves of the merge gate, the precompiled-header sweep, gcc.c-torture,
the libc++ suite and GCC's C++ testsuite on aarch64 and x86-64, every
open-source project row, and the downstream build, each compared with
the previous release. The procedure from the freeze to the tag is
[RELEASING.md](RELEASING.md); `cargo xtask release-gate` runs the gate
and writes its record (contract: [../spec/harness.md](../spec/harness.md)
"Release gate"). The PCH snapshot sweep, one of its steps, is
`scripts/pch-release-gate.sh <kiln++> <out-dir>`; it needs about 50 GB
of scratch and refuses to start with less than 90 GB free (exit 2); the
last results are in `docs/status/pch-snapshot-gates.md`.

### Patch releases

A patch release (`X.Y.Z`, `Z > 0`) is cut from a maintenance branch, not
from `main`:

1. `release/X.Y` branches from the last tag of that line (`vX.Y.0`, or
   the previous patch tag) and stays on the remote.
2. Each fix is a commit on that branch with its test. A fix that is
   already on `main` is cherry-picked (`git cherry-pick -x`); a fix that
   is new goes onto the branch first.
3. The branch has no pull request, so both halves of the merge gate run
   on it as steps of the release gate (`merge-gate`: `cargo xtask ci`;
   `oracle19`: `KILN_ORACLE=19 cargo xtask ci --oracle-only --step
   kiln`).
4. The release gate ([RELEASING.md](RELEASING.md)) runs on the branch
   head, and `cargo xtask release-gate record` writes its results into
   `docs/status/release-X.Y.Z.md`. The tag `vX.Y.Z` is made on the
   branch.
5. After the tag the branch is merged forward into `main` through one
   pull request (a merge commit; in a conflict over the version or the
   CHANGELOG's unreleased section `main`'s side is kept and the patch
   release's CHANGELOG section is inserted). The tag is then an ancestor
   of `main`, so the next minor release contains every released fix, and
   no commit on `main` is rewritten.
