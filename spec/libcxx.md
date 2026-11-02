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
| `--compiler <path>` | kiln binary. Default: `cargo build --profile <p> -p kiln_driver`, then `<target>/<p>/kiln++`. |
| `--filter <s>` | Keep only tests whose path (relative to `libcxx/test`) contains `<s>`. |
| `--list` | Print each selected test's kind and pre-run verdict (`run`, `run (XFAIL: …)`, `UNSUPPORTED (…)`). Nothing is compiled. |
| `-j, --jobs <n>` | Tests in parallel. Default 2. |
| `--timeout <secs>` | Test-binary run limit. Default 30. |
| `--run-timeout-long <secs>` | Test-binary run limit of a long test (see "Timeouts and retries"). Default 240. |
| `--compile-timeout <secs>` | Limit of each compile and link step. Default 120 s for `harness` and `release`, 300 s for `debug` (also with `--compiler`: the default follows `--profile`). |
| `--mem-limit <MiB>` | Address-space cap for every process (`spec/harness.md` "Resource limits"). kiln steps get kiln's stack reservation on top. Default 8192. `0` means no cap. |
| `--report <file.md>` | Write the generated region of a status page (see "Report"). |
| `--keep` | Keep the scratch directories under `<target>/xtask-libcxx/`. |
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
`debug-assertions = false` and `overflow-checks = false`. `release` itself is
not changed (the bench baselines are measured with it).

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
two tests faster than `release` does (0.82x and 0.78x of its time; the
reason the thin-LTO build is slower here has not been investigated), so it
is the default. A debug kiln++ is 18–22x slower than `harness` on these
two tests: `visit_return_type.pass` exceeds even the 300 s `debug` limit,
which is sized for ordinary tests, not for these two.

## Classification

A file is a test according to its name, checked in `format.py`'s order.
`*.sh.<ext>` and `*.gen.<ext>` are checked first. A file whose name starts
with `.` is not a test.

| Suffix | Kind | Steps | Passes when |
|---|---|---|---|
| `.compile.pass.cpp` | compile.pass | `-fsyntax-only` | it compiles |
| `.compile.fail.cpp` | compile.fail | `-fsyntax-only` | it fails to compile (a kiln crash is a FAIL, not a pass); no `-verify`, as in `format.py` (no such test carries `expected-*` comments) |
| `.link.pass.cpp` | link.pass | `-c`, then link | both succeed |
| `.link.fail.cpp` | link.fail | `-c`, then link | the compile succeeds and the link fails |
| `.pass.cpp` | pass | `-c`, link, run | the binary exits 0 |
| `.verify.cpp` | verify | `%{verify}`: `-fsyntax-only -Wno-error -Xclang -verify -Xclang -verify-ignore-unexpected=note` (`run.rs` `VERIFY_FLAGS`) | it exits 0: every `expected-*` comment matched (spec/diag.md §11) |
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
| `ALLOW_RETRIES: N` | A failing run step is retried up to `N` more times (see "Timeouts and retries"). `N` must be an integer, and the keyword may appear once; otherwise the test is an ERROR (lit's `ParserKind.INTEGER`). |
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

The feature set is static except for the `locale.*` features, which are
detected (below). It describes the configuration below and is defined in
`xtask/src/libcxx/features.rs`. There is no compiler-identity
feature (`clang`, `gcc`, `clang-19`, `apple-clang`), so tests that XFAIL or
skip one particular compiler run normally under kiln. `kiln` itself is in
the set. `gcc-style-warnings` is in the set although its name says `gcc`:
lit derives it for every compiler that understands `-Wno-<group>` flags
(clang and GCC, not `cl.exe`), and libc++ 19 uses it only in
`ADDITIONAL_COMPILE_FLAGS(gcc-style-warnings): -Wno-...` lines, which kiln
accepts (`depr.numeric.limits.has.denorm/deprecated.verify.cpp` needs its
`-Wno-unused-value`).

`c++26`, `c++experimental`, `large_tests`, `long_tests`,
`optimization=none`, `stdlib=libc++`, `stdlib=llvm-libc++`,
`-fsized-deallocation`, `gcc-style-warnings`, `has-fconstexpr-steps`, `verify-support`,
`libcpp-abi-version=1`, `libcpp-hardening-mode=none`,
`libcpp-has-no-availability-markup`, `libcpp-has-thread-api-pthread`,
`buildhost=linux`, `linux`, `can-create-symlinks`, `has-64-bit-atomics`,
`has-unix-headers`, `kiln`, `target=aarch64-unknown-linux-gnu`.

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
- only `libcpp-hardening-mode=none`, never `fast`, `extensive` or `debug`.
  lit adds the feature only when its `hardening_mode` parameter is set, and
  that parameter defaults to undefined. The set includes it anyway because
  it describes the library that is actually installed: Debian's
  `__config_site` defines `_LIBCPP_HARDENING_MODE_DEFAULT` as
  `_LIBCPP_HARDENING_MODE_NONE`. `.assert.pass.cpp` death tests are gated
  with `UNSUPPORTED: libcpp-hardening-mode=none` and are skipped, as lit
  would skip them on this library. They cannot pass here under any compiler
  (clang++-19 reports "child did not die" too). `REQUIRES:
  libcpp-hardening-mode=debug` tests stay UNSUPPORTED;
- no `-fexperimental-library`, `-faligned-allocation` or other flag-probe
  features that kiln does not implement;
- no `no-*` features: everything libc++ can build is enabled.

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
- the scratch directory on a host-shared bind mount (the mount holding it
  in `/proc/self/mountinfo` has type `fakeowner`, `fuse.grpcfuse` or
  `virtiofs`): a directory's hard-link count is not `2 + subdirectories`
  and `create_directory(p, attrs)` cannot copy the attributes
  (`HOST_FS_UNSUPPORTED`: `fs.op.hard_lk_ct/hard_link_count.pass.cpp`,
  `fs.op.create_directory/create_directory_with_attributes.pass.cpp`).
- an aarch64 host (`COMPILER_RT_UNSUPPORTED`: `numeric.ops.sat/mul_sat.pass.cpp`):
  LLVM lowers the signed 128-bit `__builtin_mul_overflow` to a `__muloti4`
  libcall that only compiler-rt's builtins define, and the GNU link line
  (clang's and kiln's) links libgcc, which has none; the link fails. x86_64
  expands the multiply inline.
- the system tzdata (`/usr/share/zoneinfo/tzdata.zi`, first line
  `# version YYYYx`) at 2024b or later (`TZDATA_Z_UNSUPPORTED`:
  `time.zone.members/sys_info.zdump.pass.cpp`): `tzdata.zi` spells numeric
  abbreviations `%z`, which libc++ 19's tzdb parser keeps verbatim, so
  `sys_info::abbrev` differs from `zdump`'s.
- glibc's `en_US.UTF-8` groups digits by `3` only (`LC_ALL=en_US.UTF-8
  locale -k grouping` prints exactly `grouping=3`; glibc 2.41 in the
  agent containers), reason `env: glibc en_US.UTF-8 grouping is 3, the
  test expects 3;3`: the two `grouping.pass.cpp` tests of
  `locale.moneypunct.byname` and `locale.numpunct.byname` assert `"\3\3"`
  on every non-Windows, non-AIX target (`GROUPING_UNSUPPORTED`).

A test joins a list only after its clang 19 build is shown to fail at the
same assertion under the same condition; the lists are sorted (unit
test `env_exclusions`).

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

**`ALLOW_RETRIES: N`.** lit reruns a failing test up to `N` more times
and reports the last attempt. The driver reruns only the run step: the
compile and link steps are deterministic, so rerunning them reaches the
same binary. Any failing run (non-zero exit, a signal, TIMEOUT, KILLED) is
retried. A test that passes on a retry is a PASS, and `-v` shows `passed
on attempt k of n`. The run's wall time counts every attempt.

## Commands

Every step runs with the cwd and `TMPDIR` set to the test's scratch directory,
`<target>/xtask-libcxx/run-<pid>-<nanos>/t<i>/`. The directory is removed
afterwards unless `--keep` is given. Compiles use the flags below, then the
`ADDITIONAL_COMPILE_FLAGS`, then the step flag, then the test path:

```
kiln++ -std=c++26 -stdlib=libc++ -resource-dir <repo>/resource -pthread \
     -ferror-limit=0 -D_LIBCPP_ENABLE_EXPERIMENTAL \
     -I <llvm-project>/libcxx/test/support  <ADDITIONAL_COMPILE_FLAGS>  \
     (-fsyntax-only | -c -o t.o)  <test>
kiln t.o -o t.exe -stdlib=libc++ -pthread -lc++experimental -lc++abi
./t.exe                     # cwd = scratch, --timeout, --mem-limit, no TZ
```

The test binary runs without `TZ` in its environment, as under lit (whose
`config.environment` does not pass it on): `current_zone()` is then the
system's `/etc/localtime`, which the `current_zone.pass.cpp` tests compare
against.

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
- **Crashes**: one row per crash cluster, with its test count and up to
  three example tests.
- **KILLED, TIMEOUT, ERROR and XPASS**: each test, with its first message.
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
- `run.rs`: verdict order, outcome rules; `env_exclusions` (the lists
  are sorted, the root reason, `Uid:` parsing); `slow_tests_list`
  (`slow_tests.txt` is sorted, every entry has a reason, which tests are
  long); `allow_retries_reruns_the_binary` (a fake compiler whose test
  script fails twice, then passes: PASS with 2 retries, FAIL with 1; a
  timeout is retried under the given limit).
- `report.rs`: counts, page rendering (with the `kiln build profile` row),
  marker splicing.
- `mod.rs`: options, discovery, `lit.local.cfg` lookup;
  `profiles_and_compile_timeouts` (`--profile`, the per-profile compile
  limit, `--compile-timeout`, the profile named for a `--compiler` binary);
  `tag_selection`
  (`--tag`, `KILN_LIBCXX_TAG`, bad tags) and `checkouts_coexist` (two
  `.toolchain/libcxx-<tag>` checkouts side by side, the legacy
  `.toolchain/llvm-project` for the default tag only).
- `features.rs`: the set is sorted and unique, with no compiler-identity
  feature and no static `locale.*`; `locale_names_normalize_like_glibc`;
  `locale_features_from_locale_a` (no locales, the provisioned seven, a
  bare `fr_CA`).
- `directives.rs`: `allow_retries_and_long_tests` (`ALLOW_RETRIES:` must
  be one integer; which `REQUIRES:` mark a test long).

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
- **Static features.** Apart from the locales, the feature set is
  hand-maintained for one host.
  lit derives it by probing the compiler (`features.py`). Probing kiln
  would make the set honest for flags kiln does not implement yet.
- **Compiler-identity XFAILs.** With no `clang` feature,
  `XFAIL: clang-19`-style annotations do not apply to kiln. A test that
  only clang fails therefore counts against kiln. That is intended, but it
  is worth revisiting once the pass rate is high.
