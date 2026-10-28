# Validation

Kiln's correctness is established by layered, automated evidence. The rule
for every change: new behavior ships with the test that would fail without it,
and perf-sensitive behavior ships with a recorded baseline.

## Layers

| Layer | Where | What it proves |
|---|---|---|
| Unit tests | `crates/*/src`, `crates/*/tests` | Module contracts in `spec/` |
| Conformance | `tests/conformance/<section>/` | Standard behavior by section ([lex], [cpp], [expr.const], ...) |
| Differential | `tests/differential/` | Same result as clang (-E tokens, program stdout, AST/layout facts) |
| Goldens | alongside the above, `*.expected` | AST dumps, diagnostics, IR shapes (FileCheck-style) |
| libc++ suite | external, driven by `cargo xtask libcxx` | The real world |
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
- `expect: compile-ok | compile-fail | run | pp | ast` (default `compile-ok`).
  `ast` compares the AST dump (spec/ast-dump.md) with the required golden
  `<test>.expected`. Under clang the JSON dump is normalized by
  `cargo xtask clang-ast`, which also writes new goldens. `ast-filter:
  <substr>` dumps only the matching declarations.
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
- `skip: <reason>` / `xfail: <reason>` (tracked, must shrink over time).
  `skip(kiln):`, `skip(clang):`, `xfail(kiln):` and `xfail(clang):` apply to
  one compiler only. This lets the corpus be validated against clang while it
  still tracks kiln's gaps.

The directive header must come first; a `//@` line after it is an error.
A `.h` file is a test only if it has directives, and `Inputs/` directories
hold auxiliary files, never tests. A pp golden is `<test file>.expected`, for
example `foo.cpp.expected`. It is compared token-wise (or as normalized text
under `pp-compare: text`) and refreshed by `cargo xtask test --update`.

The corpus validates itself: `cargo xtask ci` runs `cargo xtask test
--compiler clang++-19` after the unit tests (skipped with a note when clang 19
is absent; GitHub CI installs it), so a test that clang 19 does not pass is
caught before it is used to judge kiln. An oracle that hangs or crashes makes
the test an ERROR, never a kiln FAIL.

Each test file names the standard section it covers in its directory path,
and `spec/` entries list the tests that back them, so coverage of a spec is
a query, not a guess.

## Differential testing against clang

`clang-19` is the oracle for everything that is not a kiln-specific feature.
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
    the M1 exit criterion: `kiln -E` must agree with `clang -E` on all of
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
- layout: `kiln -fdump-record-layouts` vs clang's for every class in the
  corpus; mangled names vs clang's for every external symbol.
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
