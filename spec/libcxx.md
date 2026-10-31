# libc++ driver: `cargo xtask libcxx`

Status: implemented (WP-081). Code: `xtask/src/libcxx/`. This is the contract
for running libc++'s own test suite (`libcxx/test` of LLVM `llvmorg-19.1.7`)
with kiln as the compiler, without lit or CMake. It reimplements the subset of
libc++'s lit format (`libcxx/utils/libcxx/test/format.py`) that the gates
need. It is a separate manual or periodic gate. It is **not** part of
`cargo xtask ci` (`spec/harness.md` "CI").

## Test sources

`scripts/fetch-libcxx-tests.sh [dir]` makes a shallow, sparse, blobless
checkout of `llvmorg-19.1.7` (paths `libcxx/test` and `libcxx/utils`). The
default location is `.toolchain/llvm-project` of the main checkout (a
worktree resolves to its main checkout through `core.worktree`). The script
is idempotent: a checkout already at the tag is left alone. It prints the
path. `LLVM_TAG` and `LLVM_REPO` override the tag and the remote.

`cargo xtask libcxx` looks for `<repo>/.toolchain/llvm-project` first, then
the main checkout's. `--libcxx-tests <dir>` overrides both. If
`<dir>/libcxx/test/support/test_macros.h` is missing, it fails with exit
status 2 and the one-line message `libc++ tests not found at <dir>; run
scripts/fetch-libcxx-tests.sh`.

## Command line

```
cargo xtask libcxx [options] [<dir>...]
```

| Option | Meaning |
|---|---|
| `<dir>` | Directories relative to `libcxx/test`, walked recursively. Each one is a row of the report. `Inputs/` directories are skipped, as lit skips them. With no `<dir>`, the default is `--gate m3`. |
| `--gate m3` | The M3 gate directories (below). With `<dir>`s, those are added after them. |
| `--libcxx-tests <dir>` | The `llvm-project` checkout. |
| `--compiler <path>` | kiln binary. Default: `cargo build -p kiln_driver` (debug). |
| `--filter <s>` | Keep only tests whose path (relative to `libcxx/test`) contains `<s>`. |
| `--list` | Print each selected test's kind and pre-run verdict (`run`, `run (XFAIL: …)`, `UNSUPPORTED (…)`). Nothing is compiled. |
| `-j, --jobs <n>` | Tests in parallel. Default 2. |
| `--timeout <secs>` | Test-binary run limit. Default 30. Compile and link steps have a fixed 120 s limit. |
| `--mem-limit <MiB>` | Address-space cap for every process (`spec/harness.md` "Resource limits"). kiln steps get kiln's stack reservation on top. Default 8192. `0` means no cap. |
| `--report <file.md>` | Write the generated region of a status page (see "Report"). |
| `--keep` | Keep the scratch directories under `<target>/xtask-libcxx/`. |
| `--no-fail` | Exit 0 even when the run has failures. |
| `-v` | Print every result, with the tail of the failing step's stderr. |

**Gates.** `m3` is `std/utilities/meta`, `std/utilities/utility`,
`std/containers/sequences/array`, `std/strings/string.view`,
`std/containers/views/views.span`, `std/numerics/bit`,
`std/language.support/support.limits` and
`std/language.support/support.initlist`. The status page is
`docs/status/libcxx-m3.md`.

## Classification

A file is a test according to its name, checked in `format.py`'s order.
`*.sh.<ext>` and `*.gen.<ext>` are checked first. A file whose name starts
with `.` is not a test.

| Suffix | Kind | Steps | Passes when |
|---|---|---|---|
| `.compile.pass.cpp` | compile.pass | `-fsyntax-only` | it compiles |
| `.compile.fail.cpp` | compile.fail | `-fsyntax-only` | it fails to compile (a kiln crash is a FAIL, not a pass) |
| `.link.pass.cpp` | link.pass | `-c`, then link | both succeed |
| `.link.fail.cpp` | link.fail | `-c`, then link | the compile succeeds and the link fails |
| `.pass.cpp` | pass | `-c`, link, run | the binary exits 0 |
| `.verify.cpp` | verify | — | UNSUPPORTED (`driver: -verify`): kiln has no `-verify` |
| `.sh.<ext>` | sh | — | UNSUPPORTED (`driver: RUN lines`) |
| `.gen.<ext>` | gen | — | UNSUPPORTED (`driver: generated tests`) |
| `.pass.mm`, `.compile.pass.mm` | objective-c++ | — | UNSUPPORTED (`driver: objective-c++`) |

## Directives

lit keywords are read from every line of the file, until a line with `END.`.
A keyword must not directly follow a letter, a digit or `_`.

| Keyword | Handling |
|---|---|
| `REQUIRES: e1, e2` | Every expression must be true, or the test is UNSUPPORTED (`REQUIRES: <the false ones>`). Repeated lines accumulate. |
| `UNSUPPORTED: e1, e2` | If any expression is true, the test is UNSUPPORTED (`UNSUPPORTED: <it>`). |
| `XFAIL: e1, e2` / `XFAIL: *` | If any expression is true, the test is expected to fail. |
| `ADDITIONAL_COMPILE_FLAGS: f…` | Whitespace-split. Appended to the compile line in file order. |
| `ADDITIONAL_COMPILE_FLAGS(<feature>): f…` | Appended after the plain flags when `<feature>` is exactly in the feature set. |
| `FILE_DEPENDENCIES: a, b` | Each file, relative to the test's directory, is copied into the scratch directory. A missing file, a glob or a `%{…}` makes the test UNSUPPORTED (`driver: FILE_DEPENDENCIES`). |
| `MODULE_DEPENDENCIES:` | UNSUPPORTED (`driver: modules`). |
| any `RUN:` | UNSUPPORTED (`driver: RUN lines`). Custom scripts are not interpreted. |

If a flag contains a `%{…}` substitution, the test is UNSUPPORTED (`driver:
substitution`). No substitutions are expanded.

A malformed expression, or an `ADDITIONAL_COMPILE_FLAGS(` without `):`,
makes the test an ERROR.

**`lit.local.cfg`.** Every `lit.local.cfg` from `libcxx/test` down to the
test's directory is read. Only one pattern is understood: a series of
`if "<feature>" [not] in config.available_features:` lines, each followed by
`config.unsupported = True`. `#` comments and blank lines are allowed. When
a condition holds, the test is UNSUPPORTED (`lit.local.cfg: [missing ]<feature>`).
A file with anything else makes its whole subtree UNSUPPORTED (`driver:
lit.local.cfg`). No `lit.local.cfg` exists under the M3 gate directories.

**Boolean grammar** (lit's `BooleanExpression`):

```
expr  := or
or    := and ('||' and)*
and   := not ('&&' not)*
not   := '!' not | '(' or ')' | 'true' | ident
ident := ( [-+=._a-zA-Z0-9]+ | '{{' regex '}}' )+
```

An identifier is true when it matches a whole feature name. Literal pieces
match exactly. `{{regex}}` pieces use a subset of regex: literal
characters, `\`-escapes of punctuation, `.`, `[…]` and `[^…]` with ranges,
`(…)` with `|`, and `*`, `+` and `?`. Anything else in a regex is a parse
error: `{n,m}`, anchors, `(?`, and letter escapes. That makes the test an
ERROR.

## Feature set

The feature set is static. It describes the configuration below and is
defined in `xtask/src/libcxx/features.rs`. There is no compiler-identity
feature (`clang`, `gcc`, `clang-19`, `apple-clang`), so tests that XFAIL or
skip one particular compiler run normally under kiln. `kiln` itself is in
the set.

`c++26`, `c++experimental`, `large_tests`, `long_tests`,
`optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`,
`-fsized-deallocation`, `has-fconstexpr-steps`, `libcpp-abi-version=1`,
`libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`,
`buildhost=linux`, `linux`, `can-create-symlinks`, `has-64-bit-atomics`,
`has-unix-headers`, `kiln`, `target=aarch64-unknown-linux-gnu`.

Some features are deliberately missing:
- no `locale.*`: only `C`, `C.UTF-8` and `POSIX` exist;
- no `has-1024-bit-atomics`;
- no `add-latomic-workaround`;
- no `host-has-gdb`;
- no `libcpp-hardening-mode=*`: the lit parameter is undefined, and the
  Debian default is `none`;
- no `-fexperimental-library`, `-faligned-allocation` or other flag-probe
  features that kiln does not implement;
- no `no-*` features: everything libc++ can build is enabled.

The target triple is fixed. The table must be revised on any other host.

## Commands

Every step runs with the cwd and `TMPDIR` set to the test's scratch directory,
`<target>/xtask-libcxx/run-<pid>-<nanos>/t<i>/`. The directory is removed
afterwards unless `--keep` is given. Compiles use the flags below, then the
`ADDITIONAL_COMPILE_FLAGS`, then the step flag, then the test path:

```
kiln -std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread \
     -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL \
     -I <llvm-project>/libcxx/test/support  <ADDITIONAL_COMPILE_FLAGS>  \
     (-fsyntax-only | -c -o t.o)  <test>
kiln t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi
./t.exe                     # cwd = scratch, --timeout, --mem-limit
```

This mirrors lit's `llvm-libc++-shared.cfg` at its defaults: the newest
standard, experimental features enabled, no warnings, and no `-Werror`. The
libc++ headers and runtime are the system's LLVM 19 ones. The `_LIBCPP_VERSION`
that kiln sees is recorded in the report.

## Outcomes

The verdict is reached in this order:
1. ERROR: the file is unreadable or a directive is malformed.
2. Configuration UNSUPPORTED: `lit.local.cfg`, then `REQUIRES:`, then
   `UNSUPPORTED:`.
3. Driver UNSUPPORTED: kind, RUN lines, modules, substitutions,
   FILE_DEPENDENCIES.
4. The steps.
5. XFAIL.

| Outcome | Meaning |
|---|---|
| PASS | The kind's expectation held. |
| FAIL (stage) | Stage `compile`, `link` or `run` failed, or `expected-fail-but-succeeded` (a compile.fail compiled, or a link.fail linked). |
| XFAIL | Expected to fail (`XFAIL:`), and it failed. |
| XPASS | Expected to fail, but it passed. |
| UNSUPPORTED (reason) | Not run. A reason starting with `driver:` is a driver gap. Any other reason is the configuration. |
| KILLED (stage) | A process hit the memory cap (`out_of_memory`, `spec/harness.md`). |
| TIMEOUT (stage) | A process ran past its limit. Its whole process group is killed. |
| ERROR (reason) | The test could not be evaluated, or a process could not be spawned. |

A compiler crash is a FAIL with a `crash:` cluster key, flagged as a crash.
It happens when kiln (or llc, during link) panics (`panicked at`), prints
LLVM's `PLEASE submit a bug report` banner, or dies from a signal. `XFAIL`
never hides KILLED, TIMEOUT or a crash. A test binary that aborts (a failed
`assert`) is an ordinary run FAIL.

**Exit status:**
- 1 if any test is FAIL, XPASS, KILLED, TIMEOUT or ERROR, unless `--no-fail`
  is given.
- 2 on a usage error or an infrastructure error (missing checkout, no tests
  selected, kiln build failure).
- 0 otherwise.

## Report

The console shows each non-PASS/UNSUPPORTED/XFAIL result as
`[k/n] OUTCOME: path — first cluster`, then the top 10 clusters and a summary
line.

`--report <file.md>` writes a generated region between `<!-- libcxx:begin -->`
and `<!-- libcxx:end -->`. Text outside the markers is preserved, which is
where hand-written analysis goes. A file without markers gets the region
appended. The region contains:
- **Run**: kiln version and commit, tests tag, `_LIBCPP_VERSION`, target,
  command, compile and link flags, features, limits, wall time, and peak RSS
  (the largest single-process peak, via GNU time, with its test).
- **Results by directory**: one row per `<dir>`, plus a total. Columns are
  total, pass, fail, xfail, xpass, unsupported (with the driver share),
  killed, timeout, error, and pass rate. Pass rate is PASS / (total −
  UNSUPPORTED).
- **Failures by stage**: compile, link, run and expected-fail per directory.
- **Dominant first errors per directory**: the three most common first-error
  clusters among each directory's FAIL tests.
- **Error clusters**: the top 40 clusters (see below).
- **Crashes, KILLED, TIMEOUT, ERROR and XPASS**: each test, with its first
  message.
- **UNSUPPORTED reasons**: the count for each reason.

**Clusters.** For a FAIL (or a crash), each `error:` and `fatal error:` line
of the failing step's stderr gives a key:
1. Take the message after `error: `, dropping the location.
2. Replace each quoted span (`'…'` or `` `…' ``) with `'…'`.
3. Replace each run of digits with `N`.
4. Prefix the key with the stage (`compile: `, `link: `, `run: `).

If a step has no `error:` line, the key falls back as follows:
- a link step uses its `undefined reference to …` line;
- a run step uses `run: assertion failed` for a failed `assert`;
- otherwise the key is the first stderr line, or the exit status.

An expected-fail test that succeeded gets the key `compile.fail: compiled
successfully` or `link.fail: linked successfully`. A crash's first key is
`crash: panicked at <file>: <normalized message>` (or `crash: <signal>`). The
`error:` keys printed before the crash follow it.

Each cluster counts **first** (tests whose first key it is) and **any** (tests
that have it anywhere). Clusters are ranked by first, then any, then text.
Each cluster has an example test (preferring one where it is the first key)
and that test's raw message.

## Tests backing this spec

Unit tests are table-driven and live in `xtask/src/libcxx/*.rs`:
- `boolexpr.rs`: grammar, precedence, regex subset, parse errors.
- `directives.rs`: classification table, keywords, verdicts, `END.`,
  `lit.local.cfg`.
- `cluster.rs`: normalization, message extraction, counting, crash keys.
- `run.rs`: verdict order, outcome rules.
- `report.rs`: counts, page rendering, marker splicing.
- `mod.rs`: options, discovery, `lit.local.cfg` lookup.
- `features.rs`: the set is sorted and unique, with no compiler-identity
  feature.

## Open questions

- **M5: real lit.** The full suite (M5) needs `RUN:` scripts, `%{…}`
  substitutions, `.sh.cpp`, `.gen.py` and `-verify`. Two candidate paths:
  - run upstream lit (`libcxx/utils/libcxx/test/format.py`) with kiln as
    `%{cxx}` through a kiln-flavoured `lit.site.cfg`, which needs Python
    and lit in the container;
  - grow this driver until it covers the suite.

  Either path also needs `-verify` (clang's `expected-*` comment checking)
  in kiln. Most `.verify.cpp` tests check `[[nodiscard]]` and
  `static_assert` diagnostics. This driver's outcomes and report format
  should carry over, so that pass rates stay comparable.
- **Static features.** The feature set is hand-maintained for one host.
  lit derives it by probing the compiler (`features.py`). Probing kiln
  would make the set honest for flags kiln does not implement yet.
- **Compiler-identity XFAILs.** With no `clang` feature,
  `XFAIL: clang-19`-style annotations do not apply to kiln. A test that
  only clang fails therefore counts against kiln. That is intended, but it
  is worth revisiting once the pass rate is high.
