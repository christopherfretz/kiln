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
| `--oracle <path>` | Oracle compiler for `run`/`pp` comparisons. Default `clang++-19`. |
| `--no-oracle` | Skip all oracle comparisons. |
| `--filter <substr>` | Keep only tests whose name (repo-relative path) contains `<substr>`. |
| `--update` | Rewrite existing pp `.expected` goldens with the actual output. |
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
| `expect: compile-ok \| compile-fail \| run \| pp` | Test kind. Default `compile-ok`. |
| `std: <std>` | `c++98 c++03 c++11 c++14 c++17 c++20 c++23 c++26` (C++), `c89 c99 c11 c17 c23` (C). Default `c++26` / `c17`. Passed as `-std=<std>`. |
| `args: <flags>` | Extra compiler flags, split shell-style (quotes, backslash). It may be repeated, and the flags accumulate. |
| `diag: <line>[:<col>] <sev> "<substring>"` | Expected diagnostic. `<sev>` is one of `error fatal warning note remark`. If the column is omitted, any column matches. |
| `strict-diags` | The `diag:` list is exhaustive for diagnostics located in the test file. |
| `stdout: <text>` | `run` only. Each line contributes its value, trimmed, plus `\n`. Repeated lines concatenate in order. |
| `stdout-file: <path>` | `run` only. The expected stdout is the file's exact contents. The path is relative to the test's directory. It cannot be combined with `stdout:`. |
| `exit: <code>` | `run` only. Expected exit code. Default 0. |
| `oracle: clang \| none` | `run`/`pp` only. Whether to compare with the oracle. Defaults to `clang` for `run` and `pp`. |
| `oven: steps<=N bytes<=M` | kiln-specific evaluation budget. Either bound may be omitted. See "kiln-specific tests". |
| `skip[(kiln\|clang)]: <reason>` | Do not run. A qualified marker applies only to that compiler kind. |
| `xfail[(kiln\|clang)]: <reason>` | Expected to FAIL. Qualified as for `skip`. |

`expect`, `std`, `stdout-file`, `exit`, `oracle`, `oven` and each
`skip`/`xfail` qualifier may appear at most once. If both a qualified marker
and an unqualified marker apply, the qualified one's reason is reported.

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

`.c` files get `-x c`, and `.h` test files get `-x c++`.

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
| `pp` | `-E -P` | exit 0; the diags match; the output equals `<test>.expected` after normalization (if that file exists); the oracle agrees (below) |

The `run` binary runs with cwd = its scratch dir, an empty stdin, and no
arguments. A `run-args:` directive does not exist yet.

### Oracle

For `run`, the oracle compiles the test with the same flags into
`oracle.out`, which is then run. Its stdout must be byte-identical to the test
binary's, and so must its exit status.

For `pp`, the oracle's `-E -P` output must equal the compiler's after
normalization.

The oracle is run even when the compiler under test is itself clang. This
keeps the harness's own code paths exercised. If the oracle compile fails, the
test is an **ERROR**: the test is wrong, not the compiler.

### pp normalization

Runs of spaces and tabs collapse to one space, lines are trimmed, and blank
lines are dropped. The function is `compare::pp_equivalent`.

TODO(kiln_lex): replace this with a token-wise comparison, re-lexing both
outputs, once `kiln_lex` exists. VALIDATION.md asks for that.

### Goldens

A pp golden is `<test file name>.expected`, for example
`function-like.cpp.expected`, stored next to the test. `--update` rewrites an
existing golden with the raw actual output when it differs, and reports
"updated N .expected file(s)". It never creates a golden. To opt a test into a
golden, create an empty file and run `--update`, then review the diff.

### kiln-specific tests

A test with an `oven:` directive is kiln-specific. When the compiler is not
kiln, the test is a SKIP with the reason "not supported by this compiler
(kiln-specific `oven:` directive)".

When the compiler is kiln, the test is currently also a SKIP, because the
counters protocol between kiln and the harness is not defined yet. See the
open questions.

## Outcomes

The outcomes are evaluated in this order:

1. The file cannot be read, or its directives are invalid: **ERROR**.
2. A `skip` marker applies: **SKIP**.
3. The test is kiln-specific and the compiler is not kiln: **SKIP**.
4. The test runs. A broken test (missing `stdout-file`, oracle rejection, I/O
   error) is **ERROR**, even when an `xfail` marker applies.
5. Otherwise, a failure is **FAIL**, or **XFAIL** if an `xfail` marker
   applies. Success is **PASS**, or **XPASS** if an `xfail` marker applies.
   An XPASS counts as a failure, so the stale marker gets removed.

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

## Tests backing this spec

- `xtask/src/test/{directives,diag_parse,compare,process}.rs`: unit tests for
  the directive parser, the diagnostic parser and matcher, normalization and
  diffs, and process timeouts.
- `xtask/src/test/tests.rs`:
  - the seed corpus passes with clang, and every directive kind appears in it;
  - `xtask/fixtures/basic/` (one fixture per outcome, with the expected status
    as the file-name prefix);
  - `xtask/fixtures/oracle/` (oracle disagreement, run with `--oracle
    clang++-22`; skipped if that compiler is absent);
  - `--filter`;
  - `--update`.

## Open questions

- **oven counters protocol.** kiln needs to report constexpr steps and bytes
  per evaluation, for example with `-fconstexpr-report=<file>` in a format the
  harness parses. Until that exists, `oven:` tests are skipped on kiln too.
- **`run-args:` / `run-stdin:`.** Programs cannot receive argv or stdin yet.
  See `tests/conformance/basic.start.main/argv.cpp`.
- **`xtask ci`** does not run `xtask test` yet. Add it once kiln compiles the
  seed corpus, possibly with `--compiler clang++-19` as an interim self-check.
