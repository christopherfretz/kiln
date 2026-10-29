# Harness: `cargo xtask test`

Status: implemented (WP-003). Code: `xtask/src/test/`. The directive format is
introduced in `docs/VALIDATION.md`; this file is the normative contract for the
runner. It is the contract a re-implementation of the runner must satisfy, and
the contract a test author can rely on.

## Command line

```
cargo xtask test [options] [paths...]
```

| Option | Meaning |
|---|---|
| `--compiler <path>` | Compiler under test. Default: `cargo build -p kiln_driver`, then the `kiln` binary in the target dir (`$CARGO_TARGET_DIR`, else `target_directory` from `cargo metadata`), `debug` profile. |
| `--oracle <path>` | Oracle compiler for `run`/`pp`/`ast`/`layout`/`vtables` comparisons. Default `clang++-19`. |
| `--no-oracle` | Skip all oracle comparisons (`layout` and `vtables` tests, which have no other reference, are then a SKIP). |
| `--filter <substr>` | Keep only tests whose name (repo-relative path) contains `<substr>`. |
| `--update` | Rewrite existing pp and ast `.expected` goldens with the actual output. |
| `--pp-compare tokens\|text` | Override every pp test's `pp-compare:` directive (see "pp comparison"). |
| `--report-parity <file>` | After the run, write a parity table into `<file>` (see "Parity report"). A relative path is relative to the repo root. |
| `-j, --jobs <N>` | Parallel tests. Default `min(available CPUs, 8)`, capped at the test count. |
| `--timeout <secs>` | Run-time limit for test binaries (float seconds, default 10). Compiles have a fixed 120 s limit. |
| `-v, --verbose` | Print every result and every command that was run. |
| `-h, --help` | Usage. |

`--flag=value` is accepted for every option that takes a value.

`paths` are files or directories (default: `tests/conformance` and
`tests/differential`, whichever exist).

Exit status: **0** when there is no FAIL, XPASS or ERROR (XFAIL and SKIP are
allowed); **1** when there is at least one; **2** for a usage error, an
infrastructure error (kiln build failed, compiler not runnable, scratch dir not
creatable), or an empty test selection.

## Discovery

- Directories are walked recursively. Files and directories whose names start
  with `.` are ignored, and so are directories named `Inputs` (the lit
  convention for auxiliary files).
- `.cpp` and `.c` files are tests. A `.h` file is a test only if its header
  contains a `//@` directive; otherwise it is an include file.
- A file named explicitly on the command line is always a test.
- The test list is sorted by name and de-duplicated. A test's name is its path
  relative to the repo root (or absolute, if it lies outside the root).

## Directives

The directive header is the leading run of lines that are blank or start with
`//`. Directives are the lines in it that start with `//@`, written as
`//@ key: value`. A `//@` line after the header is an ERROR, because it would
otherwise be silently ignored. Unknown keys, malformed values and conflicting
directives are also ERRORs.

| Directive | Semantics |
|---|---|
| `expect: compile-ok \| compile-fail \| run \| pp \| ast \| layout \| vtables` | Test kind. Default `compile-ok`. |
| `std: <std>` | `c++98 c++03 c++11 c++14 c++17 c++20 c++23 c++26` (C++), `c89 c99 c11 c17 c23` (C). Default `c++26` / `c17`. Passed as `-std=<std>`. |
| `args: <flags>` | Extra compiler flags, split shell-style (quotes, backslash). It may be repeated, and the flags accumulate. Placeholders are substituted in each flag (below). |
| `diag: <line>[:<col>] <sev> "<substring>"` | Expected diagnostic. `<sev>` is one of `error fatal warning note remark`. If the column is omitted, any column matches. |
| `strict-diags` | The `diag:` list is exhaustive for diagnostics located in the test file. |
| `stdout: <text>` | `run` only. Each line contributes its value, trimmed, plus `\n`. Repeated lines concatenate in order. |
| `stdout-file: <path>` | `run` only. The expected stdout is the file's exact contents. The path is relative to the test's directory. It cannot be combined with `stdout:`. |
| `exit: <code>` | `run` only. Expected exit code. Default 0. |
| `run-args: <args>` | `run` only. Program arguments, split shell-style as for `args:`. It may be repeated, and the arguments accumulate. |
| `run-stdin: <text>` | `run` only. Each line contributes its value, trimmed, plus `\n`, to the program's stdin, as for `stdout:`. Repeated lines concatenate in order. |
| `run-stdin-file: <path>` | `run` only. The program's stdin is the file's exact contents. The path is relative to the test's directory (use `Inputs/`, which discovery skips). It cannot be combined with `run-stdin:`. A missing file is an ERROR. |
| `env: <NAME>=<value>` | `run` only. Sets an environment variable for the program (the inherited environment is otherwise kept). `NAME` matches `[A-Za-z_][A-Za-z0-9_]*`; the value is the rest of the line, trimmed, and may be empty or contain `=`. It may be repeated, once per name. |
| `oracle: clang \| none` | `run`/`pp`/`ast` only. Whether to compare with the oracle. Defaults to `clang` for `run` and `pp`, `none` for `ast` (the golden is the contract). `layout` and `vtables` tests are always compared with the oracle (they have no golden), so the directive is an error there. |
| `ast-filter: <substr>` | `ast` only. Dump only the declarations whose qualified name contains `<substr>` (no whitespace): kiln gets `-fdump-ast-filter=<substr>`. |
| `pp-compare: tokens \| text` | `pp` only. How outputs are compared (see "pp comparison"). Default `tokens`. |
| `pp-linemarkers` | `pp` only. Preprocess with `-E` instead of `-E -P`, so the output keeps line markers, and compare them (see "pp comparison"). |
| `oven: [<line>[:<col>]] <counter><=N ...` | kiln-specific constexpr budget; `<counter>` is `steps`, `bytes`, `peak_bytes`, `calls`, `allocs`, `peak_arena`, `frozen_bytes` or `evaluations`. Not for `pp`. See "kiln-specific tests". |
| `skip[(kiln\|clang)]: <reason>` | Do not run. A qualified marker applies only to that compiler kind. |
| `xfail[(kiln\|clang)]: <reason>` | Expected to FAIL. Qualified as for `skip`. |

`expect`, `std`, `stdout-file`, `run-stdin-file`, `exit`, `oracle`,
`pp-compare`, `pp-linemarkers`, `ast-filter`, and each `skip`/`xfail` qualifier may appear
at most once; `oven` at most once per
location (and once without one). If both a qualified marker
and an unqualified marker apply, the qualified one's reason is reported.

**Placeholders.** An `args:` flag may contain `%{name}`, replaced when the
command is built:

| Placeholder | Value |
|---|---|
| `%{repo_root}` | The repo root (absolute). |
| `%{resource_dir}` | `<repo_root>/resource`, kiln's resource directory (the directory that contains `include/`, which is what `-resource-dir` expects). |
| `%{test_dir}` | The directory containing the test (absolute). |

An unknown name or a `%{` without a closing `}` is an ERROR at parse time.
There is no escape for a literal `%{`. Values are absolute paths, so a
`pp-linemarkers` test whose golden must not depend on the checkout location
should name include directories repo-relative instead (cwd is the repo root).

The compiler kind is detected from the first line of `<compiler> --version`.
It is `kiln` if that line starts with `kiln version`, `clang` if it contains
`clang version`, and `other` otherwise.

### Diagnostics

The compiler's stderr is parsed as clang-format diagnostics:

- `file:line:col: sev: msg`, `file:line: sev: msg`, or location-less
  (`prog: error: msg` / `error: msg`).
- Indented lines (source snippets, carets, fix-its) are ignored.
- `fatal error` parses as severity `fatal`.

Diagnostics are grouped: a group is one primary (any non-note) followed by its
notes. The `diag:` directives are grouped the same way, and a `note` directive
must follow a primary directive.

Matching works as follows:

- Only diagnostics located in the test file count. Diagnostics in headers, in
  the predefines buffer, or without a location are ignored, except that a
  compile-fail needs one error in the test file (see below).
- A directive matches a diagnostic when the line is equal, the column is equal
  (if one was given), the severity matches, and the message contains the
  substring. The severity `error` also accepts `fatal`.
- Each directive group is matched to a distinct diagnostic group (maximum
  bipartite matching). **Groups are order-insensitive.** clang can emit a
  diagnostic for a later line first; for example, the preprocessor runs ahead
  of the parser, so a `#warning` on line 9 is reported before a
  `static_assert` failure on line 5.
- Within a group, the note directives must match an in-order subsequence of
  the group's notes. Under `strict-diags`, they must match the notes exactly.
- Under `strict-diags`, every diagnostic group whose primary is located in the
  test file must be matched.

## Test kinds

The compiler runs with cwd = repo root and receives the test as its
repo-relative name:

```
<compiler> [-x c | -x c++ (for .h)] -std=<std> <mode flags> <args...> <name> [-o <out>]
```

`.c` files get `-x c`, and `.h` test files get `-x c++`. A kiln-specific test
compiled by kiln also gets `-fconstexpr-report=<scratch>/constexpr-report.jsonl`
last (see "kiln-specific tests").

Any compile ends as a **FAIL** ("compiler crashed" / "compiler hung") if:

- the compile is killed by a signal or times out, or
- stderr contains `panicked at` or `PLEASE submit a bug report`.

This holds whatever the expectation is. An internal compiler error never
satisfies `compile-fail`.

| Kind | Mode | Pass condition |
|---|---|---|
| `compile-ok` | `-fsyntax-only` | exit 0, and the diags match |
| `compile-fail` | `-fsyntax-only` | non-zero exit, at least one `error`/`fatal` located in the test file, and the diags match |
| `run` | compile and link to `<scratch>/test.out` | the compile succeeds and the diags match; the binary exits normally within `--timeout`; stdout equals the expectation exactly (if one is given); the exit code equals `exit:` (default 0); the oracle agrees (below) |
| `pp` | `-E -P` (`-E` with `pp-linemarkers`) | exit 0; the diags match; the output is equivalent to `<test>.expected` (if that file exists); the oracle agrees (below). Equivalence is defined in "pp comparison". |
| `ast` | kiln: `-fsyntax-only -fdump-ast [-fdump-ast-filter=<substr>]`; clang: `-fsyntax-only -Xclang -ast-dump=json`, normalized | exit 0; the diags match; the dump equals `<test>.expected` up to trailing whitespace on each line and trailing blank lines; the oracle agrees (below). See "ast tests". |
| `layout` | kiln: `-fsyntax-only -fdump-record-layouts`; clang: `-fsyntax-only -Xclang -fdump-record-layouts-complete` | exit 0; the diags match; the dump has at least one record layout; the oracle's dump has the same blocks in the same order. See "Layout and vtable tests". |
| `vtables` | kiln: `-fsyntax-only -fdump-vtable-layouts`; clang: `-S -emit-llvm -o <scratch>/test.ll -Xclang -fdump-vtable-layouts` | exit 0; the diags match; the dump has at least one vtable block; the oracle's dump has the same set of blocks. See "Layout and vtable tests". |

The `run` binary runs with cwd = its scratch dir, the `run-args:` arguments,
the inherited environment plus `env:`, and the `run-stdin:` /
`run-stdin-file:` bytes on stdin (empty without them). The oracle's binary
gets the same.

### Oracle

For `run`, the oracle compiles the test with the same flags into
`oracle.out`, which is then run. Its stdout must be byte-identical to the test
binary's, and so must its exit status. If the oracle's binary is killed by a
signal or exceeds `--timeout`, the test is an **ERROR**: the reference itself
is broken, so there is nothing to compare with.

For `pp`, the oracle preprocesses with the same flags, and its output must be
equivalent to the compiler's (see "pp comparison").

For `ast` (only with `oracle: clang`), the oracle's normalized dump must
equal the golden. It is checked when the compiler is kiln, or when the
oracle is not the compiler under test (a clang compiler's own dump is
already compared with the golden).

For `layout` and `vtables`, the oracle's dump is the reference; see "Layout
and vtable tests".

The oracle is run even when the compiler under test is itself clang. This
keeps the harness's own code paths exercised. If the oracle compile fails or
hangs (exceeds the compile timeout), the test is an **ERROR**: the test is
wrong, not the compiler.

### pp comparison

Two preprocessed outputs are compared in one of two modes, chosen by the
`--pp-compare` option, else the test's `pp-compare:` directive, else
`tokens`. The mismatch report names the mode.

**`tokens`** (the default; `xtask/src/test/pp_tokens.rs`). Both outputs are
re-lexed with `kiln_lex` under the test's standard (`c++NN` as written; C
standards use the C++98 rules until `kiln_lex` has a C mode) and compared as
sequences of preprocessing tokens:

- Whitespace, comments and line breaks are not compared, except as token
  boundaries. `a+b` and `a + b` are equal; `++` and `+ +` are not.
- Punctuators compare by kind, so a digraph equals its primary spelling
  (`<:` equals `[`, `%:%:` equals `##`).
- Every other token compares by category (identifier, pp-number, character
  literal, string literal, raw string literal, header name, other character,
  invalid) and spelling. The spelling is taken after removing line splices
  (and trigraphs, where the standard has them), and is otherwise exact: a
  UCN and the character it names are different spellings, as are `0x1F` and
  `0X1f`.
- Without `pp-linemarkers` (`-P`), nothing else is compared.
- With `pp-linemarkers`, lines of the form `# N "file" flags` (and
  `#line N "file"`) are parsed instead of compared as tokens. Every token
  gets a presumed location (file, line) and the system-header flags (3, 4) in
  effect, and those are compared too. A marker that enters (flag 1) or
  leaves (flag 2) a file is itself an item, compared by line, file and
  flags, except markers that enter or leave a pseudo-file (`<built-in>`,
  `<command line>`), which differ between compilers. Markers without flag
  1 or 2 only move the presumed location, so the choice between a marker and
  blank lines is not compared. File names are normalized by collapsing
  `//`, `/./` and `dir/..`.

On a mismatch the report gives the index of the first differing item and
the item counts, three items of context, then the differing items, `-` for
the expected side (the golden or the oracle) and `+` for the actual one, as
`line:col  spelling  (category)`; in marker mode each token also shows
` [file:line]` (plus ` sys` or ` sys extern-C`), and a marker item shows as
`# N file flags  (line marker)`. The
items stop where the two sides agree again (8 equal items within 64 of
each side), or after 10. The report ends with the physical output line of
the first difference on each side.

**`text`** (`compare::pp_diff`). Runs of spaces and tabs collapse to one
space, lines are trimmed, and blank lines are dropped; the results must be
equal, and the report is a line diff. Use it for a test whose point is the
exact layout of the output (the printer's spacing), not its tokens.

### Goldens

A pp or ast golden is `<test file name>.expected`, for example
`function-like.cpp.expected`, stored next to the test. `--update` rewrites an
existing golden with the raw actual output when the comparison above says it
differs (so a golden is not churned by spacing changes), and reports
"updated N .expected file(s)". It never creates a golden. To opt a test into a
golden, create an empty file and run `--update`, then review the diff.
(For ast tests the golden is required, so this is also how one is created;
`cargo xtask clang-ast <test> > <test>.expected` does the same.)

### ast tests

An `ast` test compares the AST dump (`spec/ast-dump.md`) with its golden
`<test>.expected`, which is required: a missing golden is an **ERROR** whose
message points to `cargo xtask clang-ast <test>`, which prints the golden as
clang sees it. Under kiln the dump is kiln's stdout. Under clang, clang's JSON
dump is converted in-process by the `xtask clang-ast` normalizer
(`spec/ast-dump.md` §6; constexpr values come from a second, text-dump run),
so the AST corpus is checked against clang today. A dump the normalizer
cannot convert (a construct it does not map) is an **ERROR** naming the
construct: the test cannot be checked against clang. With another compiler
(`other`), `ast` tests are a SKIP. A dump that differs from the golden is a
**FAIL** with a line diff (`-` golden, `+` actual); under `--update` the
golden is rewritten instead.

### Layout and vtable tests

`layout` and `vtables` tests check the Itanium C++ ABI dumps
(`spec/abi.md` §7) against the oracle's. There is no golden: clang's dump is
the reference, produced from the same source and flags (`std:`, `args:`) in
the modes of the table above. kiln's plain `-fdump-record-layouts` already
lays out every complete record, like clang's `-complete` variant; clang only
dumps the vtables it emits, so its side compiles to LLVM IR, and every
dynamic class in a `vtables` test needs a key function defined in the test.

Both dumps are split into blocks by `kiln_abi::dump_blocks`, the splitter the
`kiln_driver` differential tests use:

- **layout**: a block starts at each `*** Dumping AST Record Layout` marker.
  clang's builtin records (`__NSConstantString_tag`, `__va_list_tag`,
  `std::__va_list`) are dropped. The blocks must be equal, one by one and in
  order. A mismatch is a **FAIL** that names the first differing block (or
  the first one only one side has) with both texts and a line diff
  (`-oracle +actual`), plus the two block counts.
- **vtables**: a block starts at each `Vtable for `, `Construction vtable
  for `, `VTable indices for `, `Thunks for ` or `Virtual base offset
  offsets for ` line. clang's `Original map` sections are dropped, trailing
  blank lines are trimmed, and the blocks are compared as a set, because
  clang prints them in emission order and repeats construction vtables. A
  mismatch is a **FAIL** listing, for each pair of blocks with the same head
  line, a line diff, then the blocks only the oracle has and the blocks only
  the compiler has (at most 4 of each, then a count).

The compiler under test must dump at least one block (a test that pins
nothing is a **FAIL**), and so must the oracle: an empty oracle dump is an
**ERROR** (for `vtables`, usually a class without a defined key function). A
failing compile is a **FAIL** under the compiler and an **ERROR** under the
oracle. The comparison runs, as for `ast`, when the compiler is kiln or the
oracle is not the compiler under test; in the clang self-check the dump must
still be produced and non-empty. Without an oracle (`--no-oracle`) these
tests are a **SKIP**, and with another compiler (`other`) too. `oven:`
budgets apply as usual (kiln's `-fsyntax-only` compile is the one checked).

### kiln-specific tests

A test with an `oven:` directive is kiln-specific. When the compiler is not
kiln, the test is a SKIP with the reason "not supported by this compiler
(kiln-specific `oven:` directive)".

When the compiler is kiln, its compile gets
`-fconstexpr-report=<scratch>/constexpr-report.jsonl` (a stale file is
removed first), and the report is read in the format of `spec/oven.md`
"Reporting protocol". Tests without `oven:` do not get the flag. Budgets are
checked only after the test kind's own checks pass, so a test that fails to
compile fails for that reason. Then:

- No readable report, or a malformed one: **FAIL** (kiln broke the protocol).
- `oven: <counter><=N ...` without a location bounds the report's `total`.
  Its `peak_bytes` and `peak_arena` are the maximum over the evaluations,
  and `evaluations` their count.
- `oven: <line>[:<col>] <counter><=N ...` bounds the evaluations whose `loc`
  is in the test file at that line (and column, if given), combined as for
  the total (sums; maximum for `peak_bytes` and `peak_arena`; count for `evaluations`). If no
  evaluation matches, the test FAILs: a budget on nothing is a stale budget.
- Each bound is inclusive: a counter above it is a **FAIL**, naming the
  location, the counter, its value and the budget.

The report's total is listed with the commands under `-v`.

### Parity report

`--report-parity <file>` writes, after the run, a markdown summary between
the lines `<!-- parity:begin -->` and `<!-- parity:end -->` in `<file>`:
the compiler, the oracle, the pp comparison mode, the run date, and one
table row per directory of tests (the test's parent directory) plus a total,
with columns tests, pass, fail, xfail, xpass, skip, error and pass rate
(pass / (tests - skip)). Text outside the markers is kept, so the file can
carry hand-written context. A file without the markers gets them appended; a
missing file is created; a file with only one of the markers is an
infrastructure error (exit 2). The report is written whatever the outcome.
`docs/status/pp-parity.md` is the preprocessor parity page:

```
cargo xtask test tests/differential/pp --report-parity docs/status/pp-parity.md
```

## Generated pp corpus

`cargo xtask gen-pp-corpus` (`xtask/src/gen_pp.rs`) writes one `pp` test
per header to `tests/differential/pp/`, each `#include`-ing the header with
`-stdlib=libc++ -resource-dir %{resource_dir}`, oracle clang, and no golden:

| Directory | Headers |
|---|---|
| `libcxx/` | libc++'s public headers (files in its include dir without `.h`, plus `cxxabi.h`) |
| `libcxx/experimental/` | libc++'s public `experimental/` headers |
| `libcxx-c/` | the C headers libc++ wraps (its `.h` files) |
| `sysroot/` | top-level `<sysroot>/*.h` headers that libc++ does not wrap |

Excluded, with the reason printed by the generator: names starting with `__`
(internal), `ext/` (deprecated GNU extensions), `libcxx.imp` and
`module.modulemap` (not headers), and sysroot headers that cannot be
included on their own (`gnumake.h`, `regexp.h`).

Options: `--std <std>` (default `c++26`; any other standard goes to sibling
files named `<header>.<std>.cpp`), `--libcxx <dir>` (default
`/usr/lib/llvm-19/include/c++/v1`), `--sysroot <dir>` (default
`/usr/include`), `--out <dir>`, and `--check` (write nothing; exit 1 if any
file would change). The generator writes only changed files, removes stale
generated files (those starting with its banner) in the directories it owns,
and prints per-directory counts. Hand-written tests live in
`tests/differential/pp/torture/`, which the generator does not touch.

## Outcomes

The outcomes are evaluated in this order:

1. The file cannot be read, or its directives are invalid: **ERROR**.
2. A `skip` marker applies: **SKIP**.
3. The test is kiln-specific and the compiler is not kiln: **SKIP**.
4. The test runs. A broken test (missing `stdout-file` or `run-stdin-file`,
   oracle rejection, oracle hang or crash, I/O error) is **ERROR**, even when
   an `xfail` marker applies.
5. Otherwise, a failure is **FAIL**, or **XFAIL** if an `xfail` marker
   applies. Success is **PASS**, or **XPASS** if an `xfail` marker applies.
   An XPASS counts as a failure, so the stale marker gets removed.

Step 3 also covers `ast`, `layout` and `vtables` tests under an `other`
compiler, and `layout` and `vtables` tests without an oracle: they are a
**SKIP** with the reason.

## Output

Output goes to stdout:

- One header line: `running N tests with <compiler> (<kind>), oracle: <oracle|none>, J jobs`.
- One block per result, in name order. A result line is
  `<STATUS> <name> [(<reason>)]`. With `-v`, the line ends with the elapsed time `[<secs>s]`.
- PASS results are printed only with `-v`. SKIP, XFAIL and the failure
  statuses are always printed.
- A failure result is followed by indented details (unified line diffs with
  `-expected +actual`, or `-oracle +actual`) and the commands that were run.
- Last comes the summary:

```
test result: ok|FAILED. P passed; F failed; X xfail; XP xpass; S skipped; E errors; T tests in S.SSs
```

Scratch files live in `<target>/xtask-test/run-<pid>-<nanos>/t<index>/`. Each
test's directory is removed when the test finishes, and the run directory is
removed at the end of the run.

## CI

`cargo xtask ci` runs `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings` and `cargo test --workspace`, then, as a corpus
self-check, `cargo xtask test --compiler clang++-19`: the seed corpus must
pass with the reference compiler, or the corpus is wrong. When `clang++-19`
is not on PATH (`--version` cannot be run), that step is skipped with a note
on stderr and `ci` still succeeds. The end-to-end tests in
`xtask/src/test/tests.rs` and `xtask/src/bench/tests.rs` skip themselves with
a note the same way. GitHub CI (`.github/workflows/ci.yml`, ubuntu-24.04)
installs `clang-19 libc++-19-dev libc++abi-19-dev` and runs `cargo xtask ci`,
so neither is skipped there.

## Tests backing this spec

- `xtask/src/test/{directives,diag_parse,compare,pp_tokens,report}.rs` and
  `xtask/src/util/process.rs`: unit tests for the directive parser and
  placeholders, the diagnostic parser and matcher, normalization and diffs,
  token-wise pp comparison (token boundaries, digraphs, `<::`, line markers,
  mismatch reports), the parity report, and process timeouts.
- `xtask/src/gen_pp.rs`: the generator's options, and idempotent generation
  over a fake header tree.
- `xtask/src/test/tests.rs`:
  - the seed corpus (`tests/conformance`, `tests/differential` except the
    generated pp corpus, plus `tests/differential/pp/torture`) passes with
    clang, and every directive kind appears in it;
  - `xtask/fixtures/basic/` (one fixture per outcome, with the expected status
    as the file-name prefix), including program arguments, stdin and
    environment, the oracle's binary hanging or crashing, token-wise and text
    pp comparison, `pp-linemarkers` goldens, and placeholders;
  - `xtask/fixtures/oven/`: `oven:` budgets, run against
    `xtask/fixtures/fake-kiln/kiln`, a shell script that identifies as kiln,
    compiles with clang and writes the canned report `<test>.oven.jsonl`
    (`@SRC@` replaced by the source path) to the `-fconstexpr-report=` path;
  - `xtask/fixtures/ast/clang/` (`expect: ast` under clang: golden match
    and mismatch, `ast-filter:`, diags, a compile error, a missing golden,
    a construct the normalizer rejects) and `xtask/fixtures/ast/kiln/` (the
    same under the fake kiln, which prints the canned dump `<test>.ast`
    with `@FILTER@` replaced by the `-fdump-ast-filter=` value, plus the
    `oracle: clang` comparison and `--no-oracle`);
  - `xtask/fixtures/dumps/kiln/` (`expect: layout` and `expect: vtables`
    under the fake kiln, which prints the canned dump `<test>.layout` or
    `<test>.vtables`, against clang: a match, a differing block, an xfail,
    an empty dump, a missing diag, an oracle that emits no vtable, and
    `--no-oracle`) and `xtask/fixtures/dumps/clang/` (the clang self-check:
    a layout with diags, a vtable with a thunk, a vtable clang does not
    emit, a compile error);
  - `xtask/fixtures/oracle/` (oracle disagreement, run with `--oracle
    clang++-22`; skipped if that compiler is absent);
  - `--filter`;
  - `--update` (pp and ast goldens).
- `kiln_abi::dump_blocks` unit tests: block splitting, builtin records,
  `Original map` and duplicate construction vtables.
- `tests/conformance/class.derived/layout-*.cpp` and
  `tests/conformance/class.virtual/{layout,vtable}-*.cpp`: the modes on the
  conformance corpus.
- `xtask/src/clang_ast/tests.rs`: the clang-ast normalizer (the mapping
  table covers M2, unknown kinds and keys are errors, the spec example,
  values, strings, the side channel), over `xtask/fixtures/ast/normalize/`.

## Open questions

- **`xtask ci` with kiln.** The corpus self-check uses clang++-19. Once kiln
  compiles the seed corpus, `ci` should also run `xtask test` with kiln.
- **Located `oven:` budgets in headers.** A location names a line of the test
  file; there is no way yet to bound an evaluation located in an included
  file, other than through the total.
