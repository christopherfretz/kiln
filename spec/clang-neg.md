# Accepts-invalid measurement: `cargo xtask clang-neg`

Status: implemented (WP-912). Code: `xtask/src/clang_neg/`. This is the
contract for measuring what kiln wrongly **accepts**. clang's own C++
negative tests (`-verify` files with `expected-error` comments) are run
through kiln. The command counts the files where clang expects an error and
kiln reports none, and the expected-error lines where kiln reports nothing.
It is about accept versus reject only. Diagnostic wording is never compared
(wording parity is post-1.0; docs/ROADMAP.md).

The command is informational. It is **not** part of `cargo xtask ci`
(`spec/harness.md` "CI"), and an accepts-invalid result does not fail it.
The exit status is 2 only when the tool itself fails (no test checkout, a
case that cannot be started at all). The published numbers live in
`docs/status/accepts-invalid.md` and the headline in docs/COMPATIBILITY.md
"Rejecting invalid code".

## Test source

The tests are read from an llvm-project checkout, never vendored. They are
Apache-2.0 WITH LLVM-exception. `scripts/fetch-clang-tests.sh [<tag>]`
makes a shallow, blob-filtered, sparse checkout of
`clang/test/{CXX,SemaCXX,SemaTemplate,Parser}` into `.oss/clang-tests`. It
is gitignored and about 50 MB. The default tag is `llvmorg-22.1.8`
(`CLANG_TAG` in `xtask/src/clang_neg/mod.rs`), and the status page names
the tag and commit of the run.

A test is a `.cpp` file under a selected directory. `Inputs/` directories
hold headers and are not walked.

## Command line

```
cargo xtask clang-neg [options] [<dir-or-file>...]
```

Paths are relative to `<tests>/clang/test`. The default is `CXX SemaCXX`.

| Option | Meaning |
|---|---|
| `--tests <dir>` | The llvm-project checkout (default `.oss/clang-tests` in the checkout running xtask). |
| `--profile <p>` | The cargo profile kiln++ is built with: `debug`, `harness` (default) or `release`. |
| `--compiler <path>` | A kiln++ binary instead of building one. |
| `--filter <glob>` | Keep only tests whose relative path matches (`*`, `?`; the libstdcxx matcher). |
| `--list` | Print every planned case, then the skip reasons with their counts. Nothing runs. |
| `-j, --jobs <n>` | Cases in parallel (default 4). Each case holds a jobserver token (`spec/harness.md` "Jobserver"). |
| `--timeout <secs>` | Per-case limit (default 60). |
| `--mem-limit <MiB>` | Address-space cap per case (default 2048), plus kiln's stack reservation (`spec/harness.md`). `0` disables it. |
| `--json <file>` | The JSON report (default `<target>/xtask-clang-neg/report.json`). |
| `--report <file.md>` | Rewrite the generated region of a status page (see "Report"). |
| `-v, --verbose` | Print every case. Otherwise only crashes, timeouts, skips, rejects-valid and a progress line every 500 cases are printed. |

## RUN lines

The `RUN:` lines of a file are read in order. A line ending in `\` is
joined with the next. Each command is planned on its own:

- A command without a `-verify` or `-verify=` token contributes nothing
  (FileCheck runs, `-ast-dump`, ...). A file with no `-verify` command is
  counted as a file but is not a case.
- `not` is stripped. `not --crash` is skipped.
- The tool must be `%clang_cc1`. `%clang` (driver) commands and any other
  tool are skipped.
- A pipe, redirection or compound command (`|`, `&&`, `;`, `>`) is skipped,
  and so is a command using `%t`, `%T` or `%{...}` (multi-step runs with
  `split-file` or a PCH).
- `%s` is the input. Any other input file, or an unknown `%`
  substitution, is skipped. A command without `%s` is skipped.
- `%std_cxx<a>-<b>` expands to one case per standard from `c++<a>` to
  `c++<b>` over 98, 11, 14, 17, 20, 23, 26. An open upper end runs to 26,
  as lit's expansion does.
- `-std=<s>` sets the standard. Without one the case uses `gnu++17`, cc1's
  default.
- `-verify` adds the prefix `expected`. `-verify=a,b` adds `a` and `b`.
  Duplicates are dropped and order is kept.

Each standard of a command is one **case**. Two cases of the same file
with the same standard, prefixes and mapped arguments (for example, RUN
lines that differ only in a dropped `-emit-llvm-only`) are run once; the
others count under the skip reason "duplicate configuration after flag
mapping".

### Flag mapping

cc1 flags become kiln++ driver flags:

| cc1 | kiln++ |
|---|---|
| `-triple <t>` (also `-triple=<t>`, `-target`) | `--target=<t>` when the arch is `x86_64` or `aarch64` (`arm64` is read as `aarch64`) and the OS is Linux. With an `unknown`/`pc`/`none`/`elf` OS it becomes `<arch>-unknown-linux-gnu`. `%itanium_abi_triple` (the host triple on a non-Windows host) adds no `--target`. Any other triple skips the case. |
| `-fcxx-exceptions` | exceptions on. Without it every case gets `-fno-exceptions`: cc1 enables C++ exceptions only with `-fcxx-exceptions`, while the driver (kiln's and clang's) enables them by default. |
| `-fexceptions`, `-fno-exceptions`, `-fno-cxx-exceptions` | dropped (they follow from the above). |
| `-fsyntax-only`, `-emit-llvm[-only]`, `-emit-obj`, `-S`, `-o <f>`, `-ferror-limit[=]<n>`, `-verify-ignore-unexpected[=...]`, `-fdump-record-layouts[-simple]`, diagnostic-format flags | dropped. |
| `-x c++` | dropped. Any other `-x` skips the case. |
| `-D`, `-U`, `-I`, `-isystem`, `-iquote`, `-idirafter`, `-include`, `-target-feature` with a separate value | passed, with `%S` replaced by the test's directory. |
| `-target-cpu <c>` | `-mcpu=<c>` |
| any other `-` flag (`-W...`, `-pedantic[-errors]`, `-fno-rtti`, `-fconstexpr-depth=`, ...) | passed as is. |

A flag the kiln driver then rejects (`kiln++: error: ...`) makes the case a
skip with reason `kiln: <driver message>`, counted after the run.

### Skips

Every skip is counted by reason, both as configurations (the number of
cases it stands for, more than one for a `%std_cxx` range) and as files.

- **cc1 flags kiln does not implement** (`SKIP_FLAGS` in
  `xtask/src/clang_neg/runline.rs`, matched as prefixes): `-fms-*`,
  `-fobjc*`, `-fopenmp*`, `-fcuda*`, `-fhip*`, `-fsycl*`, `-fopencl*`,
  `-cl-*`, `-fhlsl*`, `-fmodules*`, `-fmodule-*`, `-fprebuilt-module*`,
  `-fimplicit-module*`, `-emit-module*`, `-emit-pch`, `-include-pch`,
  `-fblocks`, `-fdelayed-template-parsing`, `-fborland-extensions`,
  `-fdeclspec`, `-fptrauth*`, `-fenable-matrix`, `-fzvector`, `-fzos*`,
  `-fasm-blocks`, `-fpascal-strings`, `-fclang-abi-compat*`,
  `-aux-triple`, `-ast-*`, `-analyze*`, `-fsanitize*`, `-E`,
  `-fapple-kext`, `-fgpu*`, `-fnative-half*`, and
  `-fexperimental-new-constant-interpreter`. The last one is clang's
  second constant evaluator, so those RUN lines are the same test again
  with that evaluator's prefixes.
- **Other targets**: see the triple row above. The reason names the arch
  (`-triple i686-*`) or the OS (`-triple *-apple`).
- **File requirements**: `REQUIRES:` terms are comma-separated and all
  must hold. A term may use `||`, `&&` and `!`. `UNSUPPORTED:` skips when
  any term holds, and `XFAIL: *` (or a term that holds) skips. The
  features that hold are `x86-registered-target`,
  `aarch64-registered-target`, `LP64`, `system-linux`, `shell` and
  `thread_support`. A `target=` term holds when it names Linux.
  Everything else (for example `asserts`) does not hold.
- **`expected-error` in an inactive `#if`** is not a skip. kiln's
  verifier sees only the directives in active code (below), so a
  directive for another configuration is never counted for this one.

## Classification

A case runs kiln++ on a copy of the test in a scratch directory, keeping
the test's file name:

```
kiln++ -fsyntax-only -ferror-limit=0 -std=<s> -verify=<prefixes> \
       -iquote <test's directory> <mapped flags> <copy>
```

`-iquote` keeps `#include "..."` of the test's neighbours working.

kiln's own `-verify` (spec/diag.md §11, clang-19 faithful) finds the
directives. It scans comments as its preprocessor lexes them, so it handles
inactive `#if` branches, `-verify=` prefixes, `@line`, `@+N`, `@-N`,
`@file:line`, `@*`, markers, `N`/`N+`/`N-M` counts and `-re` directives
exactly as in clang.

To keep wording out of the comparison, the copy differs from the test in
one way: in every `<prefix>-error` and `<prefix>-error-re` directive,
`KILN-NEG ` is inserted after the opening braces. Only bytes within a line
are inserted, so every line keeps its number. No diagnostic contains the
sentinel, so no `expected-error` can match. The verifier then lists:

- every active `expected-error` occurrence (a count of `2` is listed twice)
  under `'<prefix>-error' diagnostics expected but not seen`, with the file
  and line it expects;
- every error kiln emitted under `... seen but not expected`, with its file
  and line. At a macro location there is no file, and a frontend error
  shows as `(frontend)`.

From that report:

- **expected errors (E)**: the listed expected occurrences. A directive
  with a minimum count of 0 (`0-1`) is never listed.
- **kiln errors**: the seen errors, minus the verifier's own complaints
  about malformed directives ("invalid expected ...", "cannot find
  end/start ...", "file '...' could not be located in expected ...",
  marker errors). Each such case counts as a verify complaint.
- **MISSED-ERROR** (line level): an expected occurrence with no kiln error
  on the same line of the same file. An entry without a file matches any
  file, and `Line *` matches any line.
- **ACCEPTS-INVALID** (case level): E ≥ 1 and kiln reported no error. At
  file level, a file is accepts-invalid when any of its cases is.
- **REJECTS-VALID**: the file contains `<prefix>-no-diagnostics` for one
  of the case's prefixes, E = 0, and kiln reported an error.
- **CRASH**: a panic (`panicked at`), an LLVM crash banner, or death by a
  signal (or exit code ≥ 128). The panic location is the cluster key.
  **TIMEOUT** and **OOM** (`spec/harness.md` "Memory limit") are counted
  separately. None of the three is a verdict on accept versus reject.

A kiln error saying `not supported yet` or `not implemented yet` still
counts as an error. It is no verdict on the program, so the status page
counts these cases and lists the messages separately.

Known blind spots. Each makes the rate an under-count of what kiln
accepts:

- A directive written in an included header (`Inputs/*.h`) is not
  neutralized. It matches as usual when kiln's wording matches clang's, so
  that occurrence counts neither as expected nor as missed.
- An error on the right line but for the wrong reason counts as a hit.

## Report

The JSON report holds:

- the run information (tag, clang commit, kiln commit, command line, wall
  time);
- the totals and the per-directory totals;
- the skips by reason;
- every case: file, RUN index, standard, prefixes, mapped arguments,
  status, wall time, E, kiln error count, the missed lines with clang's
  expected text, and kiln's first error.

`--report <file.md>` rewrites the region between
`<!-- clang-neg:begin ... -->` and `<!-- clang-neg:end -->` of the page,
or appends it, and keeps the hand-written text around it. The region has:

- the run information, with wall time and the peak RSS of any one case;
- the totals table (accepts-invalid files and cases, missed-error lines,
  rejects-valid, crashes, timeouts, OOM);
- one row per directory: `CXX/<section>` for the standard-section tree
  (`CXX/dcl.decl`, `CXX/drs`, ...) and the top directory otherwise
  (`SemaCXX`);
- the top 50 accepts-invalid files, ranked by the E of their worst case;
- the top 40 missed-error clusters, keyed by clang's expected text
  normalized as in `spec/libcxx.md` "Clusters" (quoted spans become
  `'…'`, digit runs become `N`), ranked by files;
- every rejects-valid case with kiln's first error;
- every crash, timeout and OOM;
- kiln's "not supported yet" errors by message;
- the skip reasons.

The triage (the root-cause clusters and their xfail repros under
`tests/conformance/`) is hand-written text on the same page.

## Tests

The unit tests in `xtask/src/clang_neg/` (run by `cargo test -p xtask`,
so by `ci`) cover RUN-line joining, flag mapping, `%std_cxx` ranges, skip
reasons, requirement lines, the sentinel rewrite, report parsing,
classification, exit-status judging and the page's counts. They use inline
text, not the clang checkout, so `ci` needs no network or checkout.
