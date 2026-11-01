# Validation

Kiln's correctness is established by layered, automated evidence. The rule
for every change: new behavior ships with the test that would fail without it,
and perf-sensitive behavior ships with a recorded baseline.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit tests | `crates/*/src`, `crates/*/tests` | Module contracts in `spec/` |
| Conformance | `tests/conformance/<section>/` | Standard behavior by section ([lex], [cpp], [expr.const], ...) |
| Stress | `tests/stress/` | Deep nesting, huge packs, the stack guard, runaway-allocation regressions, under `mem<=N` budgets |
| Differential | `tests/differential/` | Same result as clang (-E tokens, program stdout, AST/layout facts) |
| Goldens | alongside the above, `*.expected` | AST dumps, diagnostics, IR shapes (FileCheck-style) |
| libc++ suite | `libcxx/test` of llvmorg-19.1.7 (`scripts/fetch-libcxx-tests.sh [<tag>]`), driven by `cargo xtask libcxx` (`spec/libcxx.md`); manual/periodic gate, not part of `ci`; M3 gate status in `docs/status/libcxx-m3.md`, whole-suite status and the 10% sample procedure in `docs/status/libcxx.md` (M3b) | The real world |
| libstdc++ headers | every public header of the system libstdc++ (`/usr/include/c++/14`) as a one-line TU, driven by `cargo xtask libstdcxx` (`spec/libstdcxx.md`); informational, not part of `ci`; status in `docs/status/libstdcxx.md` | The other real world (GCC's library) |
| Feature table | `docs/status/cxx-features.md`, checked by `cargo xtask features` (`spec/harness.md` "Feature table"); manual/M4-gate check, not part of `ci` | Every C++23/C++26 paper marked `done` has passing tests and clang 22's `__cpp_*` value |
| Perf | `bench/` + `bench/baselines/*.json` | No wall-clock or memory regression; vs-clang ratios |

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
  compares with one) must be at least that version.

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

`cargo xtask ci` is the merge gate. After fmt, clippy and the unit tests
(each cargo step rerun once if it fails with rustc's `error[E0463]`, a
crate artifact that vanished mid-build) it runs the corpus twice, and fails
on any FAIL, XPASS, ERROR or KILLED in either run:

- `cargo xtask test --compiler clang++-19`: the corpus validates itself, so a
  test that clang 19 does not pass is caught before it is used to judge kiln
  (skipped with a note when clang 19 is absent, and under `--quick`; GitHub
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

`cargo xtask libcxx` is deliberately outside `ci`. It runs libc++'s own
tests (a pinned upstream release, fetched by `scripts/fetch-libcxx-tests.sh`)
with kiln. It reimplements the subset of lit that the gates need: suffix
classification, `REQUIRES`/`UNSUPPORTED`/`XFAIL` with lit's boolean grammar
over a static feature set, and `ADDITIONAL_COMPILE_FLAGS`. `--gate m3` is
the M3 exit gate. A run's `--report` regenerates the per-directory table and
the error-cluster table of a status page (`docs/status/libcxx-m3.md`). It is
run by hand or periodically, and the pass rate is tracked there. The
contract is `spec/libcxx.md`.

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

`cargo xtask features` is outside `ci` as well. It checks the language
feature table, `docs/status/cxx-features.md`, which has one row per C++23 and
C++26 paper with its status, its corpus tests and its `__cpp_*` macro value in
kiln and in clang 22. It fails a `done` row that names no test, whose tests do
not all pass under kiln, or whose macro kiln defines differently from clang 22.
It also fails any row whose kiln macro column is stale. Run it by hand, and at
the M4 gate. The format and checks are in `spec/harness.md` "Feature table".

Each test file names the standard section it covers in its directory path,
and `spec/` entries list the tests that back them, so coverage of a spec is
a query, not a guess.

## GitHub Actions

`.github/workflows/ci.yml` runs the same `cargo xtask ci` (full form, clang
self-check included) on every push to `main`, on every pull request and every
merge-queue group (`merge_group`). A newer push to the same ref cancels the
older run (`concurrency: ci-<ref>`).

The job runs in a `debian:trixie` container on the `ubuntu-24.04-arm`
runner: the same aarch64 Debian 13 with Debian's clang 19.1.7, GCC 14 and
libc++ 19 as the development container. The packages are installed in the
first step (`clang-19 llvm-19 lld-19 libc++-19-dev libc++abi-19-dev`,
plus `build-essential procps time`). The environment is pinned because the
differential tests depend on the host as well as on clang. They check
search paths, `__has_builtin`/feature tables, target defaults and the
host's `va_list` against goldens recorded with that clang on that host.
**Oracle-version assumption:** the reference oracle is Debian's clang
19.1.7, and kiln models clang 19.1.3 and later. An older 19.1 (Ubuntu
24.04 ships 19.1.1) is tolerated only through the oracle defects listed
above ("The defects found so far"): the corpus self-check and the kiln run pass
with it (the GH111477 probes skip), but it is not what CI runs.
`KILN_REQUIRE_LLVM=1` turns a missing LLVM tool into a failure instead of a
skip.

A second job, `check-x86_64`, runs the same steps in the same container on
the x86-64 `ubuntu-24.04` runner (WP-228), so the host-dependent paths
(default target, GCC detection, `run` tests compiled for and run on
x86-64) are exercised too. Tests whose expectations are aarch64-specific
pin `//@ args: --target=aarch64-unknown-linux-gnu` (compile-only kinds) or
carry `//@ requires: target-aarch64` (`run` tests, which cannot cross-run;
spec/harness.md). The `-v` search-list test compares with clang++-19 live
rather than with the reference container's goldens. `check` is the
required status check; whether `check-x86_64` is too is a branch-rule
decision. The two jobs stay separate jobs (a matrix would rename the
required check to `check (…)`). Each installs git, checks out, and runs
the shared composite action `.github/actions/kiln-ci` (toolchain packages,
Rust 1.99.1, the cache, `cargo xtask ci`), so the steps are written once.
`ext.c11.atomic/alignment-warning-x86_64-run.cpp` is the x86-64 twin of an
aarch64-only `run` test (WP-239).

`Swatinem/rust-cache` keeps `~/.cargo` and the dependency builds, and
saves them even when the run fails. A warm run takes about 6 minutes:
17 s of packages, about 1 minute of workspace build, 2.5 minutes of
`cargo test`, and then the two corpus runs (46 s clang, 74 s kiln). A cold
run adds a minute or two. Locally, `cargo xtask ci` is the same gate. Run
it before pushing `main`, because Actions only reports after the push, and
the local run is still the merge gate (docs/AGENTS.md rule 3).

## Differential testing against clang

`clang-19` is the oracle for everything that is not a kiln-specific feature.

The oracle release is selected by `KILN_ORACLE` (`19`, the default and the
release the M3 gate and its numbers stay on, or `22` for the M4+ switch,
WP-184). It names `clang++-N`, `opt-N`, `llc-N`, `FileCheck-N` and
`/usr/lib/llvm-N` in one place (`xtask/src/oracle.rs`); `xtask test` and
`ci` use `clang++-N` as the oracle and self-check compiler and run kiln++
with `KILN_LLVM_DIR=/usr/lib/llvm-N`, `ir_vs_clang` runs `clang++-N`/`opt-N`,
and `xtask libcxx --tag` picks a libc++ test checkout
(`.toolchain/libcxx-<tag>`, one per release). Until steps B–E of
docs/status/wp-184-oracle-switch.md land, the corpus is baselined on 19 and
`KILN_ORACLE=22` runs report FAILs (never mapping-table ERRORs). Contract:
spec/harness.md "Oracle toolchain".
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

## Performance tracking

`cargo xtask bench` runs each entry in `bench/` and records wall-clock
(median of N), max RSS of the child (via GNU time's `%M`, since safe Rust
cannot call `getrusage` for a child), and, for kiln, the Oven's own constexpr
counters from its `-fconstexpr-report` total, into `bench/results/<date>.json`
(git-ignored local history); `bench/baselines/` holds the committed reference,
one per entry and compiler kind. A run fails if any entry regresses beyond its
threshold (default 5% time, 2% memory, 0% steps, 2% constexpr bytes; an entry
can set its own with `threshold-*:` directives, and `--threshold-*` options
override both). The counters are deterministic, so they catch evaluator
regressions that host noise would hide in the timings, and `oven:` budgets
on bench entries are enforced too. The time rule compares the fastest current run
with the baseline median so host load does not read as a regression, and a
baseline taken with a different compiler version, host or entry source is
advisory only. The same corpus is run through clang to produce the
comparison table in `docs/status/perf.md`. The contract is `spec/bench.md`.

## Release candidates

An RC is a tag on `main` where all of these are green in CI on a clean
container: fmt, clippy (deny warnings), unit, conformance (no `xfail`
regressions), differential, libc++ pass-rate not lower than the previous RC,
bench within thresholds. The supervisor agent signs off in `docs/status/`.
