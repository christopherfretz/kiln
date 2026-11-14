# libc++ driver: `cargo xtask libcxx`

Status: implemented (WP-081). Code: `xtask/src/libcxx/`. This is the contract
for running libc++'s own test suite (`libcxx/test` of LLVM `llvmorg-19.1.7`)
with kiln as the compiler, without lit or CMake. It reimplements the subset of
libc++'s lit format (`libcxx/utils/libcxx/test/format.py`) that the gates
need. It is a separate manual or periodic gate. It is **not** part of
`cargo xtask ci` (`spec/harness.md` "CI").

## Test sources

`scripts/fetch-libcxx-tests.sh [<tag>] [<dir>]` makes a shallow, sparse,
blobless checkout of an llvm-project release tag (paths `libcxx/test` and
`libcxx/utils`); `<tag>` is an `llvmorg-*` tag, default `$LLVM_TAG`, else
`llvmorg-19.1.7` (a first argument not starting with `llvmorg-` is
`<dir>`). The default location is `.toolchain/libcxx-<tag>` of the main
checkout (a worktree resolves to its main checkout through
`core.worktree`), so checkouts of two releases coexist: the M3 gate stays on
19 while the clang 22 oracle (spec/harness.md "Oracle toolchain") uses 22.
The script is idempotent: a checkout already at the tag is left alone, and
for the default tag a checkout at the older location
`.toolchain/llvm-project` is used as is. It prints the path. `LLVM_REPO`
overrides the remote.

`cargo xtask libcxx` uses the checkout of `--tag <tag>` (default the
`--gate`'s tag — `llvmorg-19.1.7` for `m3`, `llvmorg-22.1.8` for `m4` —
else `$KILN_LIBCXX_TAG`, else `llvmorg-19.1.7`; a tag not of the form
`llvmorg-<version>` is a usage error): `<repo>/.toolchain/libcxx-<tag>`,
then the main checkout's, and for `llvmorg-19.1.7` then
`.toolchain/llvm-project` of each, the first that exists.
`--libcxx-tests <dir>` overrides them. If
`<dir>/libcxx/test/support/test_macros.h` is missing, it fails with exit
status 2 and the one-line message `libc++ tests not found at <dir>; run
scripts/fetch-libcxx-tests.sh <tag>`. The report's tests line names the
checkout's `git describe` and, when it differs from the tag, `(expected
<tag>)`.

### Toolchain

The tag's major version N picks the release kiln++ imitates: when N is 19
or 22, kiln++ runs with `KILN_LLVM_DIR=/usr/lib/llvm-N` and, for N ≠ 19,
`-fclang-compat=N`; for another major, N is `KILN_ORACLE`'s
(spec/harness.md "Oracle toolchain"). The libc++ must be release N's:
- `/usr/lib/llvm-N/include/c++/v1` when it exists (the system's
  `libc++-N-dev`; `-stdlib=libc++` finds it), else
- an extracted package tree `.toolchain/libcxx-N-root` of this checkout or
  of the main checkout, holding `usr/lib/llvm-N/include/c++/v1` and
  `usr/lib/<multiarch>/libc++.a` (`dpkg-deb -x` of `libc++-N-dev`,
  `libc++abi-N-dev` and `libunwind-N-dev`; Debian's `libc++-22-dev`
  conflicts with `libc++-19-dev`, so the two cannot both be installed).
  Compiles then add `-nostdinc++ -isystem <tree include>`, and links
  `-L<tree lib> -Wl,--as-needed -Wl,-Bstatic -lc++experimental -lc++
  -lc++abi -Wl,-Bdynamic`, so no shared libc++ of the other release is
  picked up.

When neither exists the run fails with exit status 2 before compiling
anything, naming both places (`--list` does not need them).

### Cross targets

Three environment variables (`features.rs`; unit tests
`cross_target_environment`, `multiarch_directories`,
`target_and_executor_commands`, `exec_expansion`) run the suite for
another target, e.g. x86_64 binaries under qemu user emulation on an
AArch64 host. With none of them set, nothing changes: no command line,
library directory, feature or limit differs.

- **`KILN_LIBCXX_TARGET=<triple>`** (blank: unset) names a Linux target.
  A triple that names no Linux OS is rejected with exit status 2. It is
  the `target=` feature and the triple the thread-API features are
  derived from ("Feature set"); every compile and link of the run
  (including `.sh.cpp` commands through `%{compile_flags}` and
  `%{link_flags}`, the `_LIBCPP_VERSION` probe, clang mode and the
  `-fsyntax-only` compiles of `cargo xtask bench`'s libc++ slice) gets
  `--target=<triple>` right after `-stdlib=libc++` (compile) or
  `-pthread` (link). The target is *cross* when its Debian multiarch
  directory (`<arch>-linux-<env>` without the vendor, `<env>` defaulting
  to `gnu`, `i[3-6]86` → `i386`, `arm*` → `arm`) is not the host default
  target's; `KILN_LIBCXX_TARGET=<host triple>` only adds the flag.
  For a cross target:
  - the system libc++ (the host's) is never used: the libc++ is the
    package tree `.toolchain/libcxx-N-root` ("Toolchain") holding the
    target's `usr/lib/<multiarch>/libc++.a` (`dpkg-deb -x` of the target
    architecture's `libc++-N-dev`, `libc++abi-N-dev` and
    `libunwind-N-dev`). Without one the run fails with exit status 2
    before compiling anything, naming the directories it looked in;
  - the link's `-L` is not the tree's library directory but
    `<scratch>.libcxx/`, holding symbolic links to its three static
    archives only (`libc++.a`, `libc++abi.a`, `libc++experimental.a`;
    a missing one is the same exit-2 error). The driver's own trailing
    `-lc++`, outside the runner's `-Bstatic`, would otherwise open the
    tree's `libc++.so` linker script, whose `libc++.so.1` is in the
    runtime package `libc++1-N`, installed for the host only.
  - kiln++ must find the target's C runtime and libgcc itself (a cross
    GCC installation, spec/driver.md "Toolchain"); the harness passes no
    `--sysroot`.
- **`KILN_LIBCXX_EXEC="<words>"`** (split at whitespace, no shell
  quoting; blank: unset) runs every test binary: the run step is `<words>
  ./t.exe` and `%{exec}` in `.sh.cpp` commands is the words, each
  shell-quoted ("RUN lines"). Nothing else executes a built binary.
  The run step's `--mem-limit` and RSS measurement apply to the executor
  process (`--mem-limit 0` if the emulator reserves more address space).
- **`KILN_LIBCXX_TIMEOUT_SCALE=<n>`** (a number > 0, default 1)
  multiplies both run limits, `--timeout` and `--run-timeout-long`
  ("Timeouts and retries"); compile and link limits do not change.
  Compute-bound tests run 7 to 12 times slower under qemu user emulation,
  so an emulated run sets `KILN_LIBCXX_TIMEOUT_SCALE=10`.

Each variable that is set is written before `cargo xtask libcxx` in the
report's command line ("Report"), and the report's target is the
`target=` triple.

What stays the host's: `locale -a` and `zdump` (the `locale.*` and
`has-no-zdump` features), the `TMPDIR`, symlink and path-length
environment exclusions and the warning-flag probe (which compiles with
no `--target`; flag acceptance does not depend on it). They are right
for an executor that shares the host's file system, as qemu user
emulation does; a remote executor is out of scope. An emulated x86_64
run on AArch64 is:

```
KILN_LIBCXX_TARGET=x86_64-unknown-linux-gnu \
KILN_LIBCXX_EXEC="qemu-x86_64 -L /usr/x86_64-linux-gnu" \
KILN_LIBCXX_TIMEOUT_SCALE=10 cargo xtask libcxx --tag llvmorg-22.1.8 ...
```

## Command line

```
cargo xtask libcxx [options] [<dir>...]
```

| Option | Meaning |
|---|---|
| `<dir>` | Directories relative to `libcxx/test`, walked recursively. Each one is a row of the report. `Inputs/` directories are skipped, as lit skips them. With no `<dir>`, the default is `--gate m3`. |
| `--gate m3\|m4` | A gate's directories (below). With `<dir>`s, those are added after them. Any other gate is a usage error. |
| `--tag <tag>` | The release whose checkout to use (above). Default the gate's tag, else `$KILN_LIBCXX_TAG`, else `llvmorg-19.1.7`. |
| `--libcxx-tests <dir>` | The `llvm-project` checkout. |
| `--profile <p>` | The cargo profile the implicit build uses: `debug` (cargo's `dev`), `harness` (default) or `release` (see "Build profiles"). Anything else is a usage error. |
| `--compiler <path>` | kiln binary. Default: `cargo build --profile <p> -p kiln_driver`, then `<target>/<p>/kiln++`. A clang (by its `--version`) selects clang mode (see "Clang mode"). |
| `--filter <s>` | Keep only tests whose path (relative to `libcxx/test`) contains `<s>`. |
| `--list` | Print each selected test's kind and pre-run verdict (`run`, `run (XFAIL: …)`, `UNSUPPORTED (…)`). Nothing is compiled. |
| `-j, --jobs <n>` | Tests in parallel. Default 2. |
| `--timeout <secs>` | Test-binary run limit. Default 30. |
| `--run-timeout-long <secs>` | Test-binary run limit of a long test (see "Timeouts and retries"). Default 240. |
| `--compile-timeout <secs>` | Limit of each compile and link step. Default 120 s for `harness` and `release`, 300 s for `debug` (also with `--compiler`: the default follows `--profile`). |
| `--mem-limit <MiB>` | Address-space cap for every process (`spec/harness.md` "Resource limits"). kiln steps get kiln's stack reservation on top. Default 8192. `0` means no cap. |
| `--report <file.md>` | Write the generated region of a status page (see "Report"). |
| `--results <file.tsv>` | Write one line per test (see "Results file"). |
| `--shard <K/N>` | Run only shard K of N (1 ≤ K ≤ N; anything else is a usage error): of the sorted selection (after `--filter`), the tests whose 0-based index is K − 1 modulo N. Round robin, so the shards of one selection are disjoint, cover it, and get a similar mix of directories. The start line is preceded by `xtask libcxx: shard K/N: <n> of <total> tests`. The x86_64 sweep (`.github/workflows/sweep.yml`, WP-961) runs the full suite as 6 shards. |
| `--pch-diff replay\|snapshot` | Compile each pass test's leading `#include`s into a PCH and the test with `-include-pch` of it (see "PCH differential"). kiln only: with a clang `--compiler` it is a usage error, as is any other value. |
| `--keep` | Keep the scratch directories under `<target>/xtask-libcxx/` and the run's `TMPDIR` root `<temp>/kiln-tmp-<pid>-<nanos>/` (see "Commands"). |
| `--no-fail` | Exit 0 even when the run has failures. |
| `-v` | Print every result, with the tail of the failing step's stderr. |

**Profile in the output.** The start line names the profile and the
compile limit (`(harness, compile timeout 120 s, -j 4)`), the summary
line ends with `; harness kiln`, and the report has a `kiln build profile`
row. With `--compiler` the profile is the binary's directory name when that
is `debug`, `harness` or `release` (`release (--compiler)`), else
`--compiler`. The report's command adds `--profile <p>` when it is not
`harness` and `--compile-timeout` when given.

**Gates.** `m3` is `std/utilities/meta`, `std/utilities/utility`,
`std/containers/sequences/array`, `std/strings/string.view`,
`std/containers/views/views.span`, `std/numerics/bit`,
`std/language.support/support.limits` and
`std/language.support/support.initlist`, on `llvmorg-19.1.7`. The status
page is `docs/status/libcxx-m3.md`; the gate stays frozen on libc++ 19 so
its numbers remain comparable (WP-355). `m4` is `m3`'s directories followed
by the M4 exit headers' (docs/ROADMAP.md M4): `std/containers/sequences/vector`,
`std/strings/basic.string`, `std/utilities/memory`, `std/utilities/optional`,
`std/utilities/variant`, `std/utilities/tuple`, `std/algorithms`,
`std/ranges`, `std/utilities/format`, `std/utilities/expected`,
`std/containers/views/mdspan`, `std/input.output/iostream.format/print.fun`,
`std/containers/container.adaptors/flat.map`, on `llvmorg-22.1.8` with
libc++ 22 ("Toolchain"); its baseline is in `docs/status/libcxx.md`.

### PCH differential

`--pch-diff <mode>` (WP-1036; spec/harness.md "PCH differential" is the
corpus form) runs every test through a precompiled header of its own
prefix, so that the libc++ suite checks that `-include-pch` gives the
result of textual inclusion. It applies to a test when:

- its kind is a pass kind (`.pass.cpp`, `.compile.pass.cpp`,
  `.link.pass.cpp`); a `.verify.cpp` runs plainly, because `-verify`
  sees only the consumer's diagnostics (an image consumer does not
  diagnose the header again), and a fail kind's outcome does not show
  a difference;
- it has no `MODULE_DEPENDENCIES`, and no flag that starts with
  `-include` or `-x`;
- it opens with `#include` lines (`leading_includes`: blank lines and
  comments may come between them; the first other line ends them).

Every other test runs as without the option. For one that applies:

1. **The PCH.** The leading `#include`s are written, in order, to
   `pchdiff.h` in the test's scratch directory, and a copy of the test
   with those lines emptied (every other line keeps its number) to
   `src/<file>`. kiln builds the PCH with the base flags, `-iquote <test
   dir>` (for its quoted `#include`s), the test's compile flags (the
   consumer's order: a snapshot is used only when the command lines
   match) and the mode's producer flags (`replay`:
   `-fno-kiln-pch-snapshot`; `snapshot`: nothing, the default path since
   WP-1404, with `KILN_SNAP_TIMING=1`), under the same memory limit and compile
   timeout as a compile step. If that fails, the test fails at
   `compile`, with `the PCH of the leading #includes does not build`.
2. **The test** compiles the copy with `-include-pch <pch>`, the mode's
   consumer flags (`replay`: `-fno-kiln-pch-snapshot`; `snapshot`: none,
   with `KILN_SNAP_TIMING=1`) and
   `-iquote <test dir>`. Its outcome is judged as without the option.
   So a gate is green under `--pch-diff` exactly when it is green
   without it.

**The path tally.** After the summary line, `pch-diff <mode>: N tests
with a PCH` is printed, then one line per path a consumer takes, with
its count. The path is taken from the producer's `-v` note (`image (N
records)` or `replay: <why>`); in `replay` mode it is `replay`. In
`snapshot` mode it is the compile step's: `snapshot` when it restored,
else `replay: producer: <why>` or `replay: consumer: <why>` as in the
corpus (spec/harness.md "PCH differential"), numbers and quoted paths
reduced so like reasons count together. The comparison with a run
without a PCH is two runs with `--results` (WP-1395's sweep is in
docs/status/pch-snapshot-gates.md).

**Measured** (m3 gate, libc++ 19, `-j 4`, harness profile,
2026-11-11): `replay` gives 636 pass and 6 unsupported, as without the
option, and 575 tests get a PCH. The `snapshot` numbers are in
docs/status/pch-snapshot-gates.md.

## Build profiles

The implicit build is optimized by default: an unoptimized kiln++ is several
times slower on template-heavy tests, which pushed
`variant.visit/visit.pass.cpp` past the compile limit, and it makes every
large run several times longer (the M3 gate at `-j 4`: 303 s with a debug
kiln++, 48–63 s with `harness`). `debug` stays the
profile of `cargo build`, `cargo xtask test` and `ci`, so an implementer's
default build is unchanged.

`harness` (Cargo.toml) inherits `release` and sets `opt-level = 3`,
`debug = 1` (line tables: a panic's backtrace keeps file and line),
`lto = false`, `codegen-units = 16`, `incremental = true`,
`debug-assertions = false` and `overflow-checks = false`. `release` keeps
thin LTO and 4 codegen units: it runs 7–9% faster than `harness` (WP-326,
below), and the bench baselines are measured with it. `harness` stays the
suite's default for its cheaper incremental rebuilds, not for speed.

Debug assertions are off, as in the `release` binary users run. The
`debug_assert!`s in the compiler crates check internal invariants (stack
discipline in `kiln_sema::lambda`, operand counts in `kiln_oven::compile`,
the punctuator spelling in `kiln_pp`); no test expects one to fire, since a
fired assertion is a panic and so a FAIL, never a PASS. A harness run
therefore cannot pass a test that a debug run fails because of an assertion
it skips; it can only miss such a crash, and `--profile debug` remains the
way to look for one. No gate result depends on the choice (WP-319: the M3
gate gives the same 630 PASS, 6 UNSUPPORTED and 6 `.verify` FAILs, the
same tests, under `harness` as under `debug`).

Measured on the shared 24-core aarch64 box under load (load average 13–15,
shared jobserver), each build in a fresh target directory, `cargo build -p
kiln_driver --profile <p>` under `/usr/bin/time -f "%e %M"` (`%M` is the
largest rustc), and `kiln++ -c` of `variant.visit/visit.pass.cpp` and
`visit_return_type.pass.cpp` with the harness flags:

| profile | clean build | largest rustc | `visit.pass` | `visit_return_type.pass` |
|---|---:|---:|---:|---:|
| `debug` (`dev`) | 24 s | 1.08 GiB | 247 s | 465 s |
| `harness` | 58 s, 35 s | 1.23 GiB | 13.7 s | 20.9 s |
| `release` | 44 s, 61 s | 1.62 GiB | 16.6 s | 26.8 s |
| clang++-19 | | | 10.2 s | 16.7 s |

Where two build times are given, two clean builds were run; the spread
comes from the box's load, not from the profile. The compile times are on
main `93ee57ff`, where both tests compile cleanly (WP-303); they are best of
two, and the box was loaded, so they are higher than on a quiet box. kiln's
peak RSS is the same in every profile (0.48 and 0.73 GiB; clang 1.0 and
1.6 GiB). `harness` builds well within the 3-minute budget and compiles the
two tests faster than `release` did in that measurement (0.82x and 0.78x
of its time), which made it the default; WP-326 below found the opposite on
a later main. A debug kiln++ is 18–22x slower than `harness` on these
two tests: `visit_return_type.pass` exceeds even the 300 s `debug` limit,
which is sized for ordinary tests, not for these two.

WP-326 re-measured on main `2c31e9f33`. The WP-319 figures were best of
two, run one profile after the other under load 13–15; sequential passes
on this box differ from each other by up to 2.4x per test, which swamps a
20% effect. So each variant was built once in its own target directory,
and every command was then run with every binary in turn, three rounds with
the order rotated, taking each binary's minimum. The commands were the 48
`bench/libcxx-slice` tests at `-fsyntax-only`, the 7 `bench/frontend`
sources, and `-fsyntax-only` and `-c` of the two `variant.visit` tests.
Times are the geomean of the per-command minimum, relative to `harness`.
Variants other than `release` and `harness` are `release` with one
`--config` override:

| variant | clean build | largest rustc | binary | slice | frontend | `visit` |
|---|---:|---:|---:|---:|---:|---:|
| `harness` | 48 s | 1.38 GB | 73 MB | 1.000 | 1.000 | 1.000 |
| `release` (thin, 4 cgu) | 51 s | 1.57 GB | 82 MB | 0.907, 0.933 | 0.945, 0.912 | 0.911 |
| `codegen-units = 16` | 58 s | 1.55 GB | 84 MB | 0.923 | 0.916 | |
| `lto = false` | 49 s | 1.47 GB | 79 MB | 0.961 | 1.001 | |
| `lto = "fat"` | 119 s | 2.69 GB | 73 MB | 0.946 | 0.935 | |
| `codegen-units = 1` | 71 s | 1.50 GB | 75 MB | 0.929 | 0.879 | |

The load average was 8–15. Where `release` has two figures, it was measured
in two of these runs against different variants. Figures within about 3% of
each other are noise. The four LTO variants are equally fast, and LTO is
what makes the difference: `lto = false` with 4 units is no faster than
`harness`. Fat LTO and one codegen unit cost 1.4–2.3x the build time and,
for fat LTO, a 2.7 GB rustc for no measurable gain, so `release` is
unchanged. `panic = "abort"` was not tried, because it would change
behaviour: `kiln_driver` joins the compiler thread and turns a panic into
exit status 70 with a message, where abort would give SIGABRT.

## Classification

A file is a test according to its name, checked in `format.py`'s order.
`*.sh.<ext>` and `*.gen.<ext>` are checked first. A file whose name starts
with `.` is not a test.

| Suffix | Kind | Steps | Passes when |
|---|---|---|---|
| `.compile.pass.cpp` | compile.pass | `-fsyntax-only` | it compiles |
| `.compile.fail.cpp` | compile.fail | `-fsyntax-only`; with clang-verify directives, `%{verify}` | it fails to compile (a kiln crash is a FAIL, not a pass), first error recorded; with directives, as `.verify.cpp` (see ".compile.fail.cpp" below) |
| `.link.pass.cpp` | link.pass | `-c`, then link | both succeed |
| `.link.fail.cpp` | link.fail | `-c`, then link | the compile succeeds and the link fails |
| `.pass.cpp` | pass | `-c`, link, run | the binary exits 0 |
| `.verify.cpp` | verify | `%{verify}`: `-fsyntax-only -Wno-error -Xclang -verify -Xclang -verify-ignore-unexpected=note` (`run.rs` `VERIFY_FLAGS`) | it exits 0: every `expected-*` comment matched (spec/diag.md §11) |
| `.sh.<ext>` | sh | its `RUN:` commands (see "RUN lines") | every command exits 0; a test without `RUN:` lines is UNSUPPORTED (`driver: .sh test without RUN lines`) |
| `.gen.<ext>` | gen | — | UNSUPPORTED (`driver: generated tests`) |
| `.pass.mm`, `.compile.pass.mm` | objective-c++ | — | UNSUPPORTED (`driver: objective-c++`) |

### .compile.fail.cpp

libc++'s `format.py` runs a `.compile.fail.cpp` test as `! %{cxx} ...
-fsyntax-only`, so any failed compile passes, including one that fails
for an unrelated reason. The driver is stricter in two ways (WP-163):
- **With directives: `-verify`.** A test that has a clang-verify
  directive (`directives::has_verify_directives`) runs as a `.verify.cpp`
  test, with the same flags and outcome: its diagnostics must match. A
  directive is `expected-error`, `-warning`, `-note` or `-remark` (an
  optional `-re`, then `{{` on the same line), with the default prefix
  only.
- **Without directives: exit status, with the first error recorded.**
  - The test passes on a failed compile, as in lit, and records the
    compile's first `error:` line (`TestResult::first_error`) with the
    directories of its location dropped. A compile that fails without an
    `error:` line records `(no error line)`.
  - `-v` prints it under the result.
  - `--report` lists every such pass with its first error. An audit
    compares that line with the line the test means to reject.

No libc++ 19 or 22 `.compile.fail.cpp` test carries a directive: 219 and
195 tests, 0 with `expected-*` comments. So strict mode cannot change a
gate today, and there is no `--strict-compile-fail` switch; the rule is
always on. A directive-bearing test from a later release is checked from
the start.

## Directives

lit keywords are read from every line of the file, until a line with `END.`.
A keyword must not directly follow a letter, a digit or `_`.

| Keyword | Handling |
|---|---|
| `REQUIRES: e1, e2` | Every expression must be true, or the test is UNSUPPORTED (`REQUIRES: <the false ones>`). Repeated lines accumulate. |
| `UNSUPPORTED: e1, e2` | If any expression is true, the test is UNSUPPORTED (`UNSUPPORTED: <it>`). |
| `XFAIL: e1, e2` / `XFAIL: *` | If any expression is true, the test is expected to fail. |
| `ADDITIONAL_COMPILE_FLAGS: f…` | Whitespace-split. Appended to the compile line in file order. |
| `ADDITIONAL_COMPILE_FLAGS(<feature>): f…` | Appended after the plain flags when `<feature>` is exactly in the feature set, or is exactly `clang` (`DIALECT_FLAG_FEATURES`, see "Feature set"). |
| `FILE_DEPENDENCIES: a, b` | Each file, relative to the test's directory, is copied into the scratch directory. A missing file, a glob or a `%{…}` makes the test UNSUPPORTED (`driver: FILE_DEPENDENCIES`). |
| `MODULE_DEPENDENCIES:` | `std` and `std.compat` need no build step: kiln builds them in-process from libc++'s module sources (spec/modules.md §2). With a package tree ("Toolchain") kiln's own search would find the system libc++'s `std.cppm`, not the tree's, so the test gets `-fprebuilt-module-path=<prefix>/share/libc++/v1` for the tree's `<prefix>/include/c++/v1` when that directory holds `std.cppm` (kiln mode only; `Config::module_source_flags`). Any other module makes the test UNSUPPORTED (`driver: modules`). A `.sh.cpp` test gets `format.py`'s module build instead (see "RUN lines"; WP-984). |
| `ALLOW_RETRIES: N` | A failing run step is retried up to `N` more times (see "Timeouts and retries"). `N` must be an integer, and the keyword may appear once; otherwise the test is an ERROR (lit's `ParserKind.INTEGER`). |
| `RUN:` | Read only in `.sh.<ext>` tests, where they are the steps (see "RUN lines"); a line ending in `\` continues on the next `RUN:` line. Elsewhere `format.py` ignores them, and so does the driver. |

If a flag contains a `%{…}` substitution, the test is UNSUPPORTED (`driver:
substitution`). No substitutions are expanded.

A malformed expression, or an `ADDITIONAL_COMPILE_FLAGS(` without `):`,
makes the test an ERROR.

**`lit.local.cfg`.** Every `lit.local.cfg` from `libcxx/test` down to the
test's directory is read (WP-386 widened the subset; `LocalCfg` in
`directives.rs`). Understood statements, with `#` comments and blank
lines:
- `if "<feature>" [not] in config.available_features:` followed by
  `config.unsupported = True`: when the condition holds the test is
  UNSUPPORTED (`lit.local.cfg: [missing ]<feature>`);
- `config.available_features.add("<feature>")`: the feature holds for
  `REQUIRES:`/`UNSUPPORTED:`/`XFAIL:` of the tests below;
- `config.substitutions.append(("%{name}", <value>))`: a string-literal
  value defines `%{name}` for the `RUN:` lines below; any other value
  (`shlex.quote(sys.executable)`, the `%{python}` of the selftests) is
  recorded as unresolved, so only a test whose `RUN:` lines use it is
  UNSUPPORTED (`driver: substitution %{name}`);
- `import ...` and `<identifier> = <expression>` (Python locals, which can
  only matter through the statements above);
- `config.load_from_path(localConfig, lit_config)`, the idiom of the
  `libcxx/` directories that mirror `std/`: the `lit.local.cfg` of the
  same path under `std/` is applied as well (a missing one is opaque).

A file with any other statement makes its whole subtree UNSUPPORTED
(`driver: lit.local.cfg`). No `lit.local.cfg` exists under the M3 gate
directories. With this subset none of libc++ 19's `libcxx/` subtree is
opaque: of its 807 tests 487 run, 292 are configuration-UNSUPPORTED
(`REQUIRES:`/`UNSUPPORTED:`), 8 by a `lit.local.cfg` condition, 13 are
`.gen` tests, 4 need an unresolved substitution and 3 a module build
(`--list`; before WP-386 all 807 were `driver: lit.local.cfg`).

### RUN lines

A `.sh.<ext>` test's `RUN:` commands (`script.rs`, WP-386) run in order in
the test's scratch directory, which is `%T` (`t<index>`, see
"Commands"); the first that fails
decides the outcome. Substitutions are expanded as lit does: `%{name}`
until nothing changes (at most 10 rounds, lit's `recursiveExpansionLimit`; past that the test is an ERROR), then
`%s`, `%S`, `%p`, `%t` (`<scratch>/t`), `%T` and `%%` in one pass. The
built-in `%{name}`s are `format.py`'s: `%{cxx}` (the compiler),
`%{flags}` (empty), `%{compile_flags}` (the base flags and the test's
`ADDITIONAL_COMPILE_FLAGS`), `%{link_flags}`, `%{exec}` (empty: tests run
locally; `$KILN_LIBCXX_EXEC`'s words when set, see "Cross targets"), `%{build}` (`%{cxx} %s %{flags} %{compile_flags} %{link_flags}
-o %t.exe`), `%{run}` (`%{exec} %t.exe`) and `%{verify}` (`%{cxx} %s
%{flags} %{compile_flags}` with the `.verify.cpp` flags), plus the
resolved `lit.local.cfg` substitutions. Each expanded command runs as a
`bash` program with `set -o pipefail` and lit's `not` (`not() { !
"$@"; }`), so libc++'s idioms (`%{build}` then `%{run}`, output
redirected and `diff`ed, `echo %{compile_flags} | grep ...`) work. A
command that starts with `%{cxx}` and names a source (`%s`, `-c`,
`-fsyntax-only`) or uses `%{build}`/`%{verify}` is limited like a compile
step (a failure is FAIL (compile)); one with only objects like a link;
anything else like the run step, retried under `ALLOW_RETRIES:`. stdout
joins stderr in the failure detail. UNSUPPORTED instead: a `%{name}`
that is neither built in nor resolved (`driver: substitution %{name}`;
`%{python}`, the install-layout `%{lib-dir}`/`%{include-dir}`,
`%{gdb}`).

A `.sh.cpp` test with `MODULE_DEPENDENCIES:` gets `format.py`'s module
build (libc++ 19 lines 157-197, WP-984; `run.rs` `module_build`). Before
its `RUN:` lines, `std` and then `std.compat` (when named) are
precompiled with `%{cxx} %{flags} %{compile_flags}
-Wno-reserved-module-identifier -Wno-reserved-user-defined-literal
[-fmodule-file=std=%T/std.pcm] --precompile -o %T/<m>.pcm -c
<module-dir>/<m>.cppm`. These are compile steps, using the flags without
the module flags. `%{compile_flags}` of the `RUN:` lines then gains
`-fmodule-file=std.compat=%T/std.compat.pcm %T/std.compat.pcm` (when
named), then `-fmodule-file=std=%T/std.pcm %T/std.pcm`. `<module-dir>`
is `<prefix>/share/libc++/v1` for the `<prefix>/include/c++/v1` the tests
compile against (the package tree's, else the toolchain's libc++;
`Config::module_dir`). Without a `std.cppm` there, the test is
UNSUPPORTED (`driver: MODULE_DEPENDENCIES without libc++ module
sources`). The same build runs in clang mode. This covers libc++'s
`libcxx/selftest/modules/` (4 tests, all PASS under kiln and
clang++-22).

### Clang mode

`--compiler <clang>` (any compiler whose `--version` names clang, e.g.
`clang++-22`) runs the suite under clang as a self-check of the harness
(WP-386): the base flags drop kiln's own (`-resource-dir`,
`-fclang-compat`) and pin the tag's libc++ headers with `-nostdinc++
-isystem <include/c++/v1>` (the checkout's, else the LLVM installation's),
so clang compiles the same headers kiln does. The report's profile row
reads `clang (--compiler)` and the summary line ends `; clang`. A test
that passes under kiln and fails under clang is a clang difference or a
harness bug; the reverse is a kiln bug. The stratum A delta is in
docs/status/libcxx.md.

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

The feature set is the static list below plus what is derived from the
toolchain the run uses (**Target**, **Derived**, **Locales**). It is defined in
`xtask/src/libcxx/features.rs`. There is no compiler-identity
feature (`clang`, `gcc`, `clang-19`, `apple-clang`), so tests that XFAIL or
skip one particular compiler run normally under kiln. `kiln` itself is in
the set. `gcc-style-warnings` is in the set although its name says `gcc`:
lit derives it for every compiler that understands `-Wno-<group>` flags
(clang and GCC, not `cl.exe`), and libc++ 19 uses it only in
`ADDITIONAL_COMPILE_FLAGS(gcc-style-warnings): -Wno-...` lines, which kiln
accepts (`depr.numeric.limits.has.denorm/deprecated.verify.cpp` needs its
`-Wno-unused-value`).

`ADDITIONAL_COMPILE_FLAGS(clang):` flags apply to kiln (and in clang mode)
although `clang` is not a feature: kiln implements clang's dialect and
flags, so a flag libc++ adds for clang is one kiln needs too. libc++ 22
`simd_unary.pass.cpp` adds `-flax-vector-conversions=integer` against the
runner's `-flax-vector-conversions=none` (`vec_ext.h` `__negate` returns
the plain-`char` vector `!v` into an `unsigned char` mask, an error under
`none` in clang as in kiln); it passed in the WP-461 run only because
kiln did not accept `-flax-vector-conversions=` yet, so the runner
dropped the flag, and failed from WP-445 on. Only the exact name `clang`
counts: not `clang-<N>`, `apple-clang` or `clang-modules-build`, and
`REQUIRES:`, `UNSUPPORTED:` and `XFAIL:` never see it (WP-980,
`directives.rs` `clang_dialect_flags`).

`c++26`, `c++experimental`, `large_tests`, `long_tests`,
`optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`,
`std-at-least-c++03`, `std-at-least-c++11`, `std-at-least-c++14`,
`std-at-least-c++17`, `std-at-least-c++20`, `std-at-least-c++23`,
`std-at-least-c++26`, `-fsized-deallocation`, `gcc-style-warnings`, `has-fconstexpr-steps`, `verify-support`,
`can-create-symlinks`, `has-64-bit-atomics`, `has-unix-headers`, `kiln`.

What stays static, and why: the parameters (lit's `params.py` defaults;
libc++ 22's `std` parameter also adds `std-at-least-<s>` for every
standard `s` of `_allStandards` up to the selected one, so all seven at
`c++26`, which libc++ 19 never names);
compiler-flag probes, which are kiln's answers and change only with kiln;
snippet-compiling target probes (`has-unix-headers`, `has-64-bit-atomics`),
taken as clang 19 sees this machine so the set does not move with kiln.

**Target.** The target triple is kiln's default target,
`<host arch>-unknown-linux-gnu` (`features::target_triple`; the harness
passes no `--target`), so the same harness gives the right set on aarch64
and x86_64; `$KILN_LIBCXX_TARGET` replaces it ("Cross targets"). From it and the host (`features::target_features`), as lit
derives them: `target=<triple>` (`params.py` `target_triple`), `linux`
when the triple's OS is Linux (`__linux__` among the compiler's macros,
`features.py`) and `buildhost=<os>` (`sys.platform`, `linux` here). lit
has no `host=` feature and no bare architecture feature in libc++ 19 or
22's `features.py`, so the harness adds none.

**Derived.** `features::detect` reads the libc++ headers the run
compiles against: the system install of `KILN_ORACLE`
(`/usr/lib/llvm-N/include/c++/v1`, `spec/harness.md` "Oracle toolchain")
or the extracted `.toolchain/libcxx-N-root` ("Toolchain"), whichever the
run uses:
- the thread API, `libcpp-has-thread-api-pthread` or
  `libcpp-has-thread-api-external` (`features::thread_api_features`;
  `features.py` `macros`, libc++ 22's `true_false_macros`): none when
  `__config_site` disables threads; else the API it selects (libc++ 19:
  `_LIBCPP_HAS_THREAD_API_<X>` defined; libc++ ≥ 20: defined to `1`);
  else pthreads when the target OS is one `__config` maps to pthreads
  (Linux, the Apple and BSD systems, ...), as `__config` computes it.
  Debian's libc++ 19 and the extracted libc++ 22 both select nothing and
  get `libcpp-has-thread-api-pthread` from the Linux target;
- `__config_site` macros, as lit's `features.py` `macros` table maps
  them: `libcpp-abi-version=<n>` from `_LIBCPP_ABI_VERSION`;
  `libcpp-hardening-mode=<mode>` from `_LIBCPP_HARDENING_MODE_DEFAULT`
  (below); and `no-filesystem`, `no-localization`, `no-monotonic-clock`,
  `no-random-device`, `no-threads`, `no-tzdb`, `no-wide-characters`,
  `libcpp-has-no-unicode` and `libcpp-has-no-availability-markup` when
  libc++ 19's `_LIBCPP_HAS_NO_<X>` is defined or libc++ ≥ 20's
  `_LIBCPP_HAS_<X>` is defined to `0`. Debian's libc++ 19 gives exactly
  `libcpp-abi-version=1`, `libcpp-hardening-mode=none` and
  `libcpp-has-no-availability-markup`;
- `has-no-zdump` when `zdump --version` fails, as in `features.py` (the
  tzdata validation tests compare against `zdump`);
- the libc++ version, `_LIBCPP_VERSION` in `__config`
  (`features::libcxx_version`). lit has no feature for it; the harness
  uses it to gate the `TZDATA_Z_UNSUPPORTED` exclusion (below).

**Locales.** `locale.<name>` is in the set when `locale -a` lists the
locale, for the seven names libc++'s `features.py` knows: `cs_CZ.ISO8859-2`,
`en_US.UTF-8`, `fr_CA.ISO8859-1`, `fr_FR.UTF-8`, `ja_JP.UTF-8`,
`ru_RU.UTF-8` and `zh_CN.UTF-8` (`features::LOCALES`). Both sides are
compared after glibc's codeset normalization (`_nl_normalize_codeset`):
the codeset keeps only its letters, lowercased, and digits, with an `iso`
prefix when only digits remain. So `en_US.utf8` is `en_US.UTF-8` and
`cs_CZ.iso88592` is `cs_CZ.ISO8859-2`. A locale listed without a codeset
(`fr_CA`) does not count. lit instead asks `setlocale` in a probe program;
on glibc that accepts exactly the names `locale -a` lists, normalized the
same way. A run without `locale` on `PATH` has no `locale.*` feature.
`scripts/provision-locales.sh` generates the seven (Debian; as root): it
installs `locales` when `locale-gen` is missing, adds the entries to
`/etc/locale.gen` (the ISO-8859 ones under a name that carries the codeset,
`cs_CZ.ISO-8859-2 ISO-8859-2`) and runs `locale-gen`. It is idempotent and
exits 0 at once when all seven are present. `glibc-old-ru_RU-decimal-point`
(glibc older than 2.27) is never in the set.

Some features are deliberately missing:
- no `has-1024-bit-atomics`;
- no `add-latomic-workaround`;
- no `host-has-gdb`;
- `libcpp-hardening-mode=<mode>` is the installed library's default, not a
  parameter. lit adds the feature only when its `hardening_mode` parameter
  is set, and that parameter defaults to undefined. The set includes it
  anyway because it describes the library that is actually installed:
  Debian's `__config_site` defines `_LIBCPP_HARDENING_MODE_DEFAULT` as `2`,
  `_LIBCPP_HARDENING_MODE_NONE` (`1 << 1`; `1 << 2` is `fast`, `1 << 4`
  `extensive`, `1 << 3` `debug`). `.assert.pass.cpp` death tests are gated
  with `UNSUPPORTED: libcpp-hardening-mode=none` and are skipped, as lit
  would skip them on this library. They cannot pass here under any compiler
  (clang++-19 reports "child did not die" too). `REQUIRES:
  libcpp-hardening-mode=debug` tests stay UNSUPPORTED;
- no `-faligned-allocation` or other flag-probe features that kiln does
  not implement. `-fexperimental-library` is implemented (WP-461:
  `__has_feature(experimental_library)`, and `-lc++experimental` at the
  link; `libcxx/experimental/fexperimental-library.compile.pass.cpp`), but
  the feature list does not probe it: every compile already defines
  `_LIBCPP_ENABLE_EXPERIMENTAL`;
- no `no-*` features with Debian's libc++: everything libc++ can build is
  enabled (they are derived, above).

The target triple is fixed. The table must be revised on any other host.

### Environment exclusions

A few tests cannot pass on this host however they are compiled: built
with clang 19 they fail at the same assertion. They are UNSUPPORTED with
an `env:` reason, before any directive is read (`run::env_unsupported`,
also in `--list`), only while the condition holds:
- run as root (effective uid 0: the second field of `Uid:` in
  `/proc/self/status`, as in the agent containers), reason `env: running as
  root: perms::none does not deny access`: 23 filesystem tests that expect `permissions(dir,
  perms::none)` to make `dir/file` inaccessible get no `EACCES`, since
  root bypasses permission checks (`ROOT_UNSUPPORTED` in `run.rs`, e.g.
  `fs.op.status/status.pass.cpp`, `fs.op.is_block_file/is_block_file.pass.cpp`,
  `directory_entry.mods/replace_filename.pass.cpp`; WP-297).
- `RLIMIT_NPROC` does not limit root: run as root, reason `env: running
  as root: RLIMIT_NPROC does not limit root`, for
  `futures.async/thread_create_failure.pass.cpp`, which lowers the limit
  to make `std::async` fail and finds the thread created
  (`ROOT_NPROC_UNSUPPORTED`; clang 22 fails the same way, WP-461).
- a `--mem-limit` cap below what the test allocates, reason `env:
  --mem-limit <m> MiB is below the <need> MiB it needs: <why>`
  (`MEM_UNSUPPORTED`, path, MiB and why): `stringstream.members/gcount.pass.cpp`
  needs an `INT_MAX - 1` byte payload plus a `stringbuf` grown past
  `INT_MAX`, 2 + 1.5 + 3 GiB at the last regrowth: 6660 MiB of address
  space, measured on a scaled copy (docs/VALIDATION.md "libc++ memory
  exclusions"; the run itself exceeds the host's 3 GiB per-process
  budget), so 7168 MiB (WP-491; clang 22 fails under 3072 MiB too,
  WP-461). With `--mem-limit 0` (no cap) or one at
  least that large, the test runs.
- the test's `TMPDIR` on a host-shared bind mount (the mount holding it
  in `/proc/self/mountinfo` has type `fakeowner`, `fuse.grpcfuse` or
  `virtiofs`): a directory's hard-link count is not `2 + subdirectories`
  and `create_directory(p, attrs)` cannot copy the attributes
  (`HOST_FS_UNSUPPORTED`: `fs.op.hard_lk_ct/hard_link_count.pass.cpp`,
  `fs.op.create_directory/create_directory_with_attributes.pass.cpp`).
- the system tzdata (`/usr/share/zoneinfo/tzdata.zi`, first line
  `# version YYYYx`) at 2024b or later, with libc++ older than 20
  (`TZDATA_Z_UNSUPPORTED`: `time.zone.members/sys_info.zdump.pass.cpp`):
  `tzdata.zi` spells numeric abbreviations `%z`, which libc++ 19's tzdb
  parser keeps verbatim, so `sys_info::abbrev` differs from `zdump`'s.
  libc++ 20 expands `%z`, so the exclusion is dropped automatically when
  the headers' `_LIBCPP_VERSION` (**Derived**) is 200000 or more (an
  unreadable version counts as older).
- glibc's `en_US.UTF-8` groups digits by `3` only (`LC_ALL=en_US.UTF-8
  locale -k grouping` prints exactly `grouping=3`; glibc 2.41 in the
  agent containers), reason `env: glibc en_US.UTF-8 grouping is 3, the
  test expects 3;3`: the two `grouping.pass.cpp` tests of
  `locale.moneypunct.byname` and `locale.numpunct.byname` assert `"\3\3"`
  on every non-Windows, non-AIX target (`GROUPING_UNSUPPORTED`).

A test joins a list only after its clang 19 build (clang 22 for a libc++
22 test) is shown to fail at the same assertion under the same condition;
the lists are sorted (unit test `env_exclusions`).

## Timeouts and retries

A test binary runs under `--timeout` (default 30 s), or under
`--run-timeout-long` (default 240 s) when the test is long:
- a `REQUIRES:` expression names `long_tests` as a plain identifier. lit's
  `long_tests` parameter defaults to true, so `long_tests` is in the
  feature set and these tests run; lit gives them no other limit, so the
  driver's longer limit is what keeps them from timing out on a loaded
  host (`rand.dist.bern.geo/eval.pass.cpp`,
  `rand.dist.bern.negbin/eval.pass.cpp`,
  `rand.dist.samp.discrete/eval.pass.cpp`);
- or the test is listed in `xtask/src/libcxx/slow_tests.txt`: one path
  per line (relative to `libcxx/test`), then the reason, sorted, `#`
  comments. A test joins only after it is shown to time out under
  `--timeout` and pass under the long limit (today
  `ofstream.members/buffered_writes.pass.cpp`, slow on the host-shared
  scratch mount, `thread.semaphore/lost_wakeup.pass.cpp` and
  `charconv.msvc/test.pass.cpp`).

`--list` shows `(long: REQUIRES: long_tests)` or `(long:
slow_tests.txt)` after `run`, and `(ALLOW_RETRIES: N)` when the test has
it. Compile and link steps always use `--compile-timeout`.
`$KILN_LIBCXX_TIMEOUT_SCALE` multiplies both run limits ("Cross targets").

**`ALLOW_RETRIES: N`.** lit reruns a failing test up to `N` more times
and reports the last attempt. The driver reruns only the run step: the
compile and link steps are deterministic, so rerunning them reaches the
same binary. Any failing run (non-zero exit, a signal, TIMEOUT, KILLED) is
retried. A test that passes on a retry is a PASS, and `-v` shows `passed
on attempt k of n`. The run's wall time counts every attempt.

## Commands

Every step runs with the cwd set to the test's scratch directory,
`<target>/xtask-libcxx/run-<pid>-<nanos>/t<i>/`, and `TMPDIR` set to its
own `<temp>/kiln-tmp-<pid>-<nanos>/t<i>/`, `<temp>` being the system temp
directory (`std::env::temp_dir`). As lit's `lit-tmp-*` directory, it is short:
libc++ 22's `filesystem_error::what()` drops the last byte of a message of
256 bytes or more, so `fs.op.copy_file/copy_file.pass.cpp`, comparing two
`TMPDIR/<cwd name>-static_env.<n>/file1` paths in one message, needs each
under 103 bytes (unit test `short_scratch_paths`; WP-461). Both directories
are removed afterwards unless `--keep` is given. Compiles use the flags below, then the
`ADDITIONAL_COMPILE_FLAGS`, then the step flag, then the test path:

```
kiln++ -std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread \
     -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL \
     -fclang-compat=<N> -fno-crash-diagnostics \
     -I <llvm-project>/libcxx/test/support  <warning flags>  \
     -D_LIBCPP_HAS_NO_PRAGMA_SYSTEM_HEADER  <ADDITIONAL_COMPILE_FLAGS>  \
     (-fsyntax-only | -c -o t.o)  <test>
kiln t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi
./t.exe                     # cwd = scratch, --timeout, --mem-limit, no TZ
```

`$KILN_LIBCXX_TARGET` adds `--target=<triple>` to the compile and link
lines and `$KILN_LIBCXX_EXEC` puts its words before `./t.exe` ("Cross
targets").

The test binary runs without `TZ` in its environment, as under lit (whose
`config.environment` does not pass it on): `current_zone()` is then the
system's `/etc/localtime`, which the `current_zone.pass.cpp` tests compare
against.

This mirrors lit's `llvm-libc++-shared.cfg` at its defaults: the newest
standard, experimental features enabled, and `enable_warnings` (WP-418,
replacing WP-402's no-warnings choice). `<warning flags>` are libc++'s
`_warningFlags` (`utils/libcxx/test/params.py` lines 17-81 at the M4 tag:
`-Werror -Wall -Wextra -Wshadow ...` and the `-Wno-...` list), verbatim
in `run.rs` `WARNING_FLAGS`. As lit's `AddOptionalWarningFlag`, each one
is kept only if the compiler accepts it: once per run, an empty
translation unit is compiled with `-xc++ -fsyntax-only -Werror <flag>`
(kiln, like clang 22, rejects the GCC-only `-Wno-` names; since WP-445
kiln accepts `-flax-vector-conversions=none`, as clang). With them, the libc++ headers
are not system headers (`-D_LIBCPP_HAS_NO_PRAGMA_SYSTEM_HEADER`), so
kiln's warnings inside them count; a `.verify.cpp` compile undefines it
again and adds `-Wno-error` (`%{verify}`, `format.py` line 102). The
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
5. XFAIL, unless the test is in `run.rs` `XFAIL_IGNORED` (path and
   reason; empty since WP-418, when the driver took lit's `-Werror`: the
   `libcxx/selftest/*/werror.*` tests now XFAIL as under lit).
6. XFAIL as well, under kiln only (not in clang mode) and with libc++ 22
   or later headers, for a test in `run.rs` `CLANG_DEFECT_XFAIL` (path and
   reason): its `TEST_COMPILER_CLANG` branch asserts a clang defect kiln
   does not share (kiln defines `__clang__`). Today the two
   `*_comparable_with.compile.pass.cpp` tests that pin llvm.org/PR171438
   (WP-446; docs/status/libcxx-m5.md). Passing such a test is an XPASS:
   kiln has taken on the defect.
7. XFAIL as well, under kiln only (not in clang mode) and with libc++ 22
   headers (`_LIBCPP_VERSION` major 22, `M5_TRIAGED_LIBCXX`), for a test in
   `run.rs` `M5_TRIAGED` (path and reason, `XFAIL: m5: <reason>` in
   `--list`): the M5 triage ledger of class (b) `.verify` diagnostic gaps
   (a missing warning or a wording difference) that do not gate M5, each
   reason naming the gap and its WP or "no WP yet" (WP-461;
   docs/status/libcxx-m5.md "Triaged (b)"). An accepts-invalid (a
   missing *error*) is a correctness gap and is never ledgered. Passing
   such a test is an XPASS: the fix must remove the entry. The unit test
   `m5_triaged_list` checks the ledger is sorted, every path exists in the
   libc++ 22 checkout (when it is present), and no path is in two of
   `XFAIL_IGNORED`, `CLANG_DEFECT_XFAIL` and `M5_TRIAGED`.

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
It happens when kiln (or llc, during link) panics (kiln's `internal
compiler error:` banner, spec/driver.md "Internal compiler errors", or a
raw `panicked at` from an older build), prints LLVM's `PLEASE submit a bug
report` banner, or dies from a signal. kiln compiles get
`-fno-crash-diagnostics`, so a crash writes no reproducer. `XFAIL`
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
  command, compile and link flags, features, limits, wall time, peak RSS
  (the largest single-process peak, via GNU time, with its test; see
  **Peak RSS** below), and kiln compile peak RSS (the largest peak of a
  kiln++ compile step, with its test).
- **Peak RSS** is over every step of a test, the test binary's run
  included, so it does not by itself measure kiln; **kiln compile peak
  RSS** is over the compile steps alone (`TestResult::compile_rss_kb`;
  the `-fsyntax-only`, `-verify` or `-c` step, not the link or run), so
  it is kiln's own number. The `REQUIRES:
  large_tests` tests allocate gigabytes at run time:
  `stringstream.members/gcount.pass.cpp` (and the `tellp` tests of the same
  shape) builds a `std::string` of `INT_MAX - 1` characters and writes it
  into a `std::stringstream`, whose buffer grows past 4 GiB; its run step
  peaks at 5122 MiB, while kiln's compile of it peaks at about 70 MiB
  (WP-369; a clang++-19 build of a scaled-down payload peaks within 0.1%
  of kiln's build). Such a peak is the library test's own, not a breach
  of the per-process memory budget of kiln.
  `tests/conformance/stringbuf.virtuals/libcxx-intmax-payload-compile-mem.cpp`
  keeps the compile of that shape under `mem<=1024`.
- **Results by directory**: one row per `<dir>`, plus a total. Columns are
  total, pass, fail, xfail, xpass, unsupported (with the driver share),
  killed, timeout, error, and pass rate. Pass rate is PASS / (total −
  UNSUPPORTED).
- **Failures by stage**: compile, link, run and expected-fail per directory.
- **Dominant first errors per directory**: the three most common first-error
  clusters among each directory's FAIL tests.
- **Error clusters**: the top 40 clusters (see below).
- **Crashes**: one row per crash cluster, with its test count and up to
  three example tests.
- **KILLED, TIMEOUT, ERROR and XPASS**: each test, with its first message.
- **compile.fail tests passed on exit status**: the count, then (folded)
  each such test with its first error line (see ".compile.fail.cpp").
- **UNSUPPORTED reasons**: the count for each reason.

**Clusters.** For a FAIL (or a crash), each `error:` and `fatal error:` line
of the failing step's stderr gives a key:
1. Take the message after `error: `, dropping the location.
2. Replace each quoted span (`'…'` or `` `…' ``) with `'…'`.
3. Replace each run of digits with `N`.
4. Prefix the key with the stage (`compile: `, `link: `, `run: `).

A failing `.verify.cpp` step is keyed by its `-verify` report instead
(`cluster::verify_keys`): one key per listed entry,
`verify: <level> expected but not seen: <text>` or
`verify: <level> seen but not expected: <text>`, with the entry's text
normalized as above (the `File … Line …` location dropped). A verify
failure without such a report (a crash, a driver error) falls back to the
`error:` keys.

If a step has no `error:` line, the key falls back as follows:
- a link step uses its `undefined reference to …` line;
- a run step uses `run: assertion failed` for a failed `assert`;
- otherwise the key is the first stderr line, or the exit status.

An expected-fail test that succeeded gets the key `compile.fail: compiled
successfully` or `link.fail: linked successfully`. A crash's first key is
`crash: panicked at <file>: <normalized message>` (or `crash: <signal>`); for
kiln's banner, `<file>` comes from its `note: in compiler source <file>:L:C`
line and the message from its `internal compiler error:` line, which is
itself never an `error:` key. The `error:` keys printed before the crash
follow it.

Each cluster counts **first** (tests whose first key it is) and **any** (tests
that have it anywhere). Clusters are ranked by first, then any, then text.
Each cluster has an example test (preferring one where it is the first key)
and that test's raw message.

## Results file

`--results <file.tsv>` (WP-961) writes the run's results in selection
order, for merging shards or comparing two runs (the report's tables are
not mergeable). Line 1 is the header `# outcome<TAB>test<TAB>detail<TAB>first-key<TAB>wall-s`;
then one line per test with five tab-separated fields: the outcome tag
(`PASS`, `FAIL`, `XFAIL`, `XPASS`, `UNSUPPORTED`, `KILLED`, `TIMEOUT`,
`ERROR`), the path relative to `libcxx/test`, the detail (the failing
stage — `compile`, `link`, `run`, `expected-fail-but-succeeded` — for
FAIL, KILLED and TIMEOUT; the reason for UNSUPPORTED and ERROR), the first
cluster key ("Classification" in "Report"), and the wall time in seconds
with two decimals. An empty field is `-`; a tab or newline inside a field
becomes a space. `scripts/libcxx-results.py` merges such files (exit 2 if
a test is in two of them) into the counts per outcome for the whole
selection, `std/` and `libcxx/`, the pass rate, and the list of failing
tests, each marked against an optional `--baseline` run.

## Tests backing this spec

Unit tests are table-driven and live in `xtask/src/libcxx/*.rs`:
- `boolexpr.rs`: grammar, precedence, regex subset, parse errors.
- `directives.rs`: classification table, keywords, verdicts, `END.`,
  `lit.local.cfg`; `local_cfg_substitutions_and_features`,
  `local_cfg_mirror`, `run_lines` (WP-386); `clang_dialect_flags`
  (WP-980).
- `script.rs`: `expansion`, `checks`, `line_kinds`, `quoting` (WP-386);
  `exec_expansion` (`%{exec}`/`%{run}` with executor words, WP-1391).
- `cluster.rs`: normalization, message extraction, counting, crash keys.
- `run.rs`: verdict order, outcome rules; `env_exclusions` (the lists
  are sorted, the root reasons, the `--mem-limit` gate, `Uid:` parsing);
  `m5_triaged_list` (sorted, kiln only, libc++ 22 only, paths exist, no
  path on two XFAIL lists); `short_scratch_paths` (the deepest
  `copy_file` path stays under 103 bytes); `slow_tests_list`
  (`slow_tests.txt` is sorted, every entry has a reason, which tests are
  long); `xfail_ignored_list` (sorted, every entry has a reason); `clang_defect_xfail_list` (sorted, kiln only, libc++ 22+);
  `compile_fail_records_first_error_or_verifies` (WP-163: the first
  error with its directories dropped, `(no error line)`, a compile that
  succeeds, and a directive-bearing test run as verify);
  `allow_retries_reruns_the_binary` (a fake compiler whose test
  script fails twice, then passes: PASS with 2 retries, FAIL with 1; a
  timeout is retried under the given limit); `preparation_of_scripts`
  (`.sh.cpp` idioms) and its `ETXTBSY` regression loop (fake scripts are
  written by a child `cp`/`chmod`, never held open for writing by this
  process, while another thread forks); `clang_mode_flags`;
  `target_and_executor_commands` (with the cross-target variables unset
  the compile, link and run commands are byte-for-byte the earlier ones;
  set, where `--target` and the executor go; WP-1391).
- `report.rs`: counts, page rendering (with the `kiln build profile` row
  and the two peak-RSS rows, measured and unavailable), marker splicing.
- `mod.rs`: options, discovery, `lit.local.cfg` lookup;
  `shards_partition_the_selection` (`--shard` parsing; the shards are
  disjoint, cover the selection, round robin) and `results_file_lines`
  (the `--results` header, fields, `-` and tab replacement, WP-961);
  `profiles_and_compile_timeouts` (`--profile`, the per-profile compile
  limit, `--compile-timeout`, the profile named for a `--compiler` binary);
  `tag_selection`
  (`--tag`, `KILN_LIBCXX_TAG`, bad tags) and `checkouts_coexist` (two
  `.toolchain/libcxx-<tag>` checkouts side by side, the legacy
  `.toolchain/llvm-project` for the default tag only); `cross_target_tree`
  (a cross target's tree, the error naming where it looked, the
  static-archives directory; WP-1391).
- `features.rs`: the set is sorted and unique, with no compiler-identity
  feature and no static `locale.*`; `locale_names_normalize_like_glibc`;
  `locale_features_from_locale_a` (no locales, the provisioned seven, a
  bare `fr_CA`); `target_features_from_the_triple` (Linux triples with and
  without a vendor, a non-Linux one); `thread_api_from_config_site_and_target`
  (libc++ 19 and 22 spellings, no threads, an explicit API, a target
  `__config` maps to no API); `cross_target_environment`
  (`KILN_LIBCXX_TARGET`/`_EXEC`/`_TIMEOUT_SCALE` values) and
  `multiarch_directories` (Debian multiarch names, what is cross; WP-1391).
- `directives.rs`: `allow_retries_and_long_tests` (`ALLOW_RETRIES:` must
  be one integer; which `REQUIRES:` mark a test long); `verify_directives`
  (what counts as a clang-verify directive).

## Open questions

- **M5: real lit.** The full suite (M5) needs `RUN:` scripts, `%{…}`
  substitutions, `.sh.cpp` and `.gen.py`. Two candidate paths:
  - run upstream lit (`libcxx/utils/libcxx/test/format.py`) with kiln as
    `%{cxx}` through a kiln-flavoured `lit.site.cfg`, which needs Python
    and lit in the container;
  - grow this driver until it covers the suite.

  kiln has `-verify` (WP-143), so either path can run `%{verify}`. This
  driver's outcomes and report format should carry over, so that pass
  rates stay comparable.
- **`.verify.cpp` FAILs are diagnostic gaps.** Most of these tests check
  that a particular diagnostic text appears (`[[nodiscard]]`, deprecation
  warnings, `static_assert` messages, overload-resolution errors). A FAIL
  means kiln's diagnostic differs from clang 19's, not that the library is
  rejected. Whether kiln should match clang's diagnostic text for all of
  these, or only for the ones a test pins, is open.
- **Static features.** Apart from the derived features, the set is
  hand-maintained for one host (see "What stays static").
  lit derives it by probing the compiler (`features.py`). Probing kiln
  would make the set honest for flags kiln does not implement yet.
- **Compiler-identity XFAILs.** With no `clang` feature,
  `XFAIL: clang-19`-style annotations do not apply to kiln. A test that
  only clang fails therefore counts against kiln. That is intended, but it
  is worth revisiting once the pass rate is high.
