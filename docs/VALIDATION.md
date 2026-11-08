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
| Debugger | `tests/debuginfo/` (`expect: debugger`, spec/harness.md "Debugger tests"); a default root, SKIP without gdb/lldb-19, ERROR under `KILN_REQUIRE_DEBUGGERS=1` (GitHub CI) | A `-O0 -g` binary debugs under gdb and lldb-19 as clang's does: breakpoints, backtraces, variables, stepping |
| Goldens | alongside the above, `*.expected` | AST dumps, diagnostics, IR shapes (FileCheck-style) |
| libc++ suite | `libcxx/test` of llvmorg-19.1.7 (`scripts/fetch-libcxx-tests.sh [<tag>]`), driven by `cargo xtask libcxx` (`spec/libcxx.md`); manual/periodic gate, not part of `ci`; M3 gate status in `docs/status/libcxx-m3.md`, whole-suite status and the 10% sample procedure in `docs/status/libcxx.md` (M3b) | The real world |
| libstdc++ headers | every public header of the system libstdc++ (`/usr/include/c++/14`) as a one-line TU, driven by `cargo xtask libstdcxx` (`spec/libstdcxx.md`); informational, not part of `ci`; status in `docs/status/libstdcxx.md` | The other real world (GCC's library) |
| Feature table | `docs/STANDARD_SUPPORT.md`, the standard support table (one section per standard, C++98/03 to C++26, reflection and kiln extensions), checked by `cargo xtask features` (`spec/harness.md` "Feature table"); manual/M4-gate check, not part of `ci` | Every core-language feature marked `done` has passing tests and clang 22's `__cpp_*` value at its standard; the summary matches the sections |
| Perf | `bench/` + `bench/baselines/*.json` | No wall-clock or memory regression; vs-clang ratios |
| Reflection perf | `bench/reflect/` (shapes S1-S6 from `cargo xtask gen-reflect`; named explicitly), `cargo xtask bench --scaling <shape>` (spec/bench.md "Reflection shapes", "Scaling sweeps"); manual/nightly on the dev host, not part of `ci` | The §3.8 budgets of docs/plans/m6-reflection.md: `peak_arena` independent of N, linear wall time and RSS, kiln vs GCC 16.1 |
| External corpora | `cargo xtask test --external <dir>` / `cargo xtask bench --external <dir>` on directories outside the repo (spec/harness.md "External corpora"); manual, never in `ci` | Never crash: private crash reproducers and the reference workload compile or fail cleanly; counts only reach the repo |
| OSS builds | real open-source projects (`scripts/oss/projects.toml`) built through their own build systems and test suites with kiln as the C++ compiler (`scripts/oss/`, spec/landed/harness/WP-666.md); manual, needs the network, never in `ci`; status in `docs/status/oss.md` | Real code bases build and pass their tests at parity with clang-19 (ROADMAP M8) |

**What runs where.** Merge-queue CI (`cargo xtask ci`, both jobs) runs the
unit tests, the corpus in clang and kiln mode, and the `oven:` counter
budgets the corpus carries, and every `oracle: gcc` comparison against
GCC 16.1, the reflection oracle: the jobs run in the `gcc:16.1.0-trixie`
image with `KILN_GCC=/usr/local/bin/g++` and `KILN_REQUIRE_GCC=1`, so a
missing GCC fails CI instead of skipping (WP-502; spec/harness.md "GCC
oracle"). The same jobs install gdb and lldb-19 and set
`KILN_REQUIRE_DEBUGGERS=1`, so the debugger tests (`tests/debuginfo`)
run in CI and a missing debugger fails it (WP-816). On the dev host, where `scripts/env.sh` sets `KILN_GCC`,
`cargo xtask ci` and `cargo xtask test` run the same comparisons. Off the queue, by hand or nightly on the dev
host: `cargo xtask bench` (including `bench/reflect` and the vs-GCC table
via `--compiler $KILN_GCC`), `--scaling` sweeps, `--external` corpora,
`cargo xtask libcxx`, `libstdcxx` and `features`.

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
`scripts/oss/selftest.sh` checks the scripts themselves.

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

- `lints`: `ci --step lints` (`cargo xtask lint`, fmt, clippy);
- `unit <group>`, one per unit group (`xtask`, `codegen`, `rest`): `ci
  --step unit --unit-group <group>`, the `cargo test --workspace` split by
  package so no job runs more than about two minutes of tests (the
  `xtask` group alone is the seed-corpus tests). `unit codegen` also runs
  the `ORACLE_TESTS` again under `KILN_ORACLE=19`, where they are already
  built;
- `self-check K/2`: `ci --step self-check --shard K/2`, the corpus under
  clang;
- `kiln-22 K/6` and `kiln-19 K/6`: `ci --step kiln --shard K/6`, and the
  same under `KILN_ORACLE=19 ... --oracle-only`.

The shard count `S = 6` is one constant: the anchored matrix list
`&kiln-shards` in `ci-lane.yml`, which both kiln runs reuse; every job
passes `K/${{ strategy.job-total }}`, so the list length is `N`.
Sharding is round-robin over the sorted test list (test `i` runs in shard
`K` iff `i % N == K - 1`), which spreads every directory, slow or fast,
over all shards. The fan-in jobs `check` (needs `arm`) and `check-x86_64`
(needs `x86_64`) run `if: always()` and pass only when the lane's result
is `success`, so a failed, cancelled or skipped job fails them; they are
the status checks branch rules name. That is 18 jobs per architecture
plus the fan-in, 38 per run: on a plan with a 60-job concurrency limit two
overlapping runs queue each other's jobs.

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
(docs/status/perf.md "WP-502: GCC 16.1 in CI" has the cost). The packages are
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
per-job floor are in docs/status/perf.md "WP-692: CI fan-out". Locally,
`cargo xtask ci` is the same gate. Run
it before pushing `main`, because Actions only reports after the push, and
the local run is still the merge gate (docs/AGENTS.md rule 3).

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
TU when a library area the reference workload uses is not covered. Every
mismatch fixed gets a reduced test under `tests/mangle/` (or
`tests/codegen/` when it is about emission).

### GCC 16.1 header sweep

The reference workload needs GCC 16.1's libstdc++ (its `<meta>`). CI has
no GCC 16 tree, so this sweep is run by hand. `KILN_GCC16_ROOT` is the root
of an unpacked GCC 16.1 image: headers under
`$KILN_GCC16_ROOT/usr/local/include/c++/16.1.0`, the installation under
`$KILN_GCC16_ROOT/usr/local/lib/gcc/<triple>/16.1.0` (with `crtbegin.o`).
The driver finds it with clang's `--gcc-toolchain=` (spec/driver.md
"Toolchain"); `-freflection` predefines `__cpp_impl_reflection`, which
`<meta>` keys on.

```sh
GCC16=--gcc-toolchain=$KILN_GCC16_ROOT/usr/local
# the probe: one TU including every standard header the reference
# workload uses (<algorithm> ... <vector>, <cxxabi.h>, <meta>)
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
declarations of `std::meta`. The reference workload includes libstdc++
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
is a FAIL:

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
`tests/conformance` (spec/landed/codegen/WP-910.md).

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
on bench entries are enforced too, as is a `budget-time:` directive: an
absolute ceiling on kiln's fastest run that holds even under `--record`, so a
re-recording cannot absorb a regression past it (`bench/pp/headers.cpp`
carries one at 1.5x its recorded kiln time, WP-487). The time rule compares the fastest current run
with the baseline median so host load does not read as a regression, and a
baseline taken with a different compiler version, host or entry source is
advisory only. The same corpus is run through clang to produce the
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
contract is `spec/bench.md`.

## Release candidates

An RC is a tag on `main` where all of these are green in CI on a clean
container: fmt, clippy (deny warnings), unit, conformance (no `xfail`
regressions), differential, libc++ pass-rate not lower than the previous RC,
bench within thresholds. The supervisor agent signs off in `docs/status/`.
