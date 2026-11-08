# LLVM self-host: the kiln-built clang builds LLVM again (WP-815)

M8 wave 4c, the bootstrap check with kiln as stage 0:

- **Stage 0** is kiln++ at `75d7f9612` (kiln main, harness profile).
- **Stage 1** is clang and lld 19.1.7 (tag `llvmorg-19.1.7`, commit
  `cd708029e0b2869e80abe31ddb175f7c35361f90`), built by kiln++ through
  the `llvm_clang` row (WP-801): Release with assertions, AArch64 and X86
  backends, `-O3`, C++17.
- **Stage 2** is the same tree, built again by stage 1's `bin/clang` and
  `bin/clang++` through the `llvm_selfhost` row
  (spec/landed/harness/WP-815.md). It uses the same configure flags,
  build targets and test command.

Host: aarch64 Linux, libstdc++ 14. Date: 2026-11-08.

**Result: parity, and no miscompile found.**

- **Builds.** Stage 1 and stage 2 compile all 3565 TUs and link all 157
  targets.
- **Stage 2 tests.** Stage 2's `check-clang` and `check-lld` match the
  clang++-19 control count for count: 35229 and 2193 passed, with the
  control's own seven environmental failures and no others.
- **Stage 1 tests.** Stage 1's tests match as well. Its one extra
  failure was a timing-dependent unit test, which passed on every rerun.
- **IR comparison.** The stage-2/stage-3 comparison is byte-identical
  after two documented normalizations (the ident string, and inline
  asm source offsets):
  - 200 TUs to `-S -emit-llvm -O2` and the same 200 to assembly, across
    the stage-1 clang, the stage-2 clang and Debian's clang 19.1.7;
  - a wider sample of 1000 TUs to IR (the 200 and 800 more), stage 2
    against Debian's clang.
- **Clusters.** No cluster was found, so no kiln fix and no xfail test
  were added.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # stage 0: $CARGO_TARGET_DIR/harness/kiln++
ulimit -v 3000000                              # every compile, link and test binary
scripts/oss/oss-build.sh llvm_clang            # stage 1 (+ its check-clang, check-lld)
scripts/oss/oss-build.sh llvm_selfhost         # stage 2 (+ its check-clang, check-lld)
# controls: the same two rows with --compiler clang (clang++-19 builds
# stage 1; that stage 1 builds a control stage 2), here with --no-test
scripts/oss/oss-build.sh llvm_clang --compiler clang --no-test
scripts/oss/oss-build.sh llvm_selfhost --compiler clang --no-test
scripts/oss/oss-report.py --perf llvm_clang llvm_selfhost
```

`oss-build.sh llvm_selfhost --which` prints the two compilers a run
will use. Stage 2 configures as `llvm_clang` does (see
[llvm_clang.md](llvm_clang.md)), with `-DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx`
wrapping `.oss/build/llvm_clang/kiln/bin/clang++`, and
`-DCMAKE_C_COMPILER=.oss/build/llvm_clang/kiln/bin/clang`. It builds
`all clang-test-depends ClangUnitTests lld-test-depends LLDUnitTests`
with `nice -n 10 make -k` on the shared jobserver, then runs
`make check-clang; make check-lld` (lit `-sv -j 4`). Both kiln runs
were started under `ulimit -v 3000000`, so every test binary inherits
the 3 GB cap that WP-801 introduced.

The stage-2/stage-3 comparison (`scripts/oss/stage-diff.py`) reruns
stage 1's own TU commands (from its `compile.jsonl`) with each compiler:

```sh
B=.oss/build
scripts/oss/stage-diff.py make --log $B/llvm_clang/kiln/compile.jsonl --sample 200 \
  --out .oss/stagediff s1=$B/llvm_clang/kiln/bin/clang++ c19=/usr/bin/clang++-19 \
  s2=$B/llvm_selfhost/kiln/bin/clang++
nice -n 10 make -k -f .oss/stagediff/Makefile        # shared jobserver
scripts/oss/stage-diff.py compare --out .oss/stagediff c19 s1 s2
# the same with --emit asm --out .oss/stagediff-asm; and
# --sample 1000 --out .oss/stagediff-1k c19=... s2=...
```

Each command is the TU's build command with `-c`, `-o` and the
dependency-file flags dropped, and `-S -emit-llvm -O2
-fdiscard-value-names` added. The `-O2` replaces nothing: it follows
the build's `-O3`, so it wins. `-fdiscard-value-names` is explicit
because the assertion-enabled stage-1 and stage-2 clangs keep value
names by default and Debian's release clang does not.

Two things are normalized, and nothing else.

1. **The ident string.** In IR, `!llvm.ident` reads
   `clang version 19.1.7 (https://github.com/llvm/llvm-project cd708029e…)`
   for stage 1 and stage 2, and `Debian clang version 19.1.7 (3+b1)` for
   Debian's clang. In assembly, the `.ident` string differs the same
   way. Both become `clang version`.
2. **Inline asm source offsets.** The operands of each metadata node
   that an inline asm's `!srcloc` names become `{srcloc}`. These
   operands are raw `SourceLocation` encodings, offsets into the
   SourceManager, which clang uses only to place diagnostics from the
   backend's assembler. They shift by the length difference of the
   predefines buffer. Debian's `__VERSION__` is
   `"Debian Clang 19.1.7 (3+b1)"` and its `__clang_version__` is
   `"19.1.7 (3+b1)"`. The upstream build's are
   `"Clang 19.1.7 (https://github.com/llvm/llvm-project cd708029e…)"`
   and `"19.1.7 (https://…)"`, 66 and 73 bytes longer.

   One of the 1000 TUs, `llvm/lib/Support/Valgrind.cpp`, has inline
   asm (from `valgrind.h`). Its two `!srcloc` nodes differed by exactly
   139 = 66 + 73. Stage 1's output for it is byte-identical to stage
   2's, because both carry the same version strings. This is the only
   TU in the sample with `!srcloc`.

## Results

TUs are build-phase C++ compiles. Build is from the end of configure to
the end of `make`. The runs shared the host's jobserver pool with other
agents' builds, so wall time is not a compiler figure. The summed TU CPU
time is. Peak TU RSS is the wrapper's `ru_maxrss` for one compile.

| Run | compiler | TUs ok | links | build | summed TU CPU / wall | peak TU RSS |
|---|---|---|---|---|---|---|
| stage 1 | kiln++ `75d7f9612` | 3565 / 3565 | 157 / 157 | 3790 s | 11344 s / 14121 s | 647 MB (`clang/unittests/Tooling/SourceCodeTest.cpp`) |
| stage 1 control | clang++-19 (Debian, no assertions) | 3565 / 3565 | 157 / 157 | 6237 s | 11613 s / 13601 s | 1108 MB (same TU) |
| stage 2 | the kiln-built stage-1 clang | 3565 / 3565 | 157 / 157 | 6615 s | 15779 s / 24211 s | 1161 MB (same TU) |
| stage 2 control | the clang-19-built stage-1 clang | 3565 / 3565 | 157 / 157 | 5375 s | 14924 s / 18799 s | 1189 MB (same TU) |

Whole `oss-build.sh` wall, including configure and both checks: stage 1
8839 s, stage 2 12722 s. The `/usr/bin/time -v` peak over every process
of the stage-1 run, tests included, was 1084 MB.

### check-clang and check-lld

| Run | check-clang (lit + unit) | check-lld |
|---|---|---|
| control (WP-801 clang++-19 build) | 35229 passed, 6 failed ‡, 2734 unsupported, 8 skipped, 26 xfail (38003 discovered; lit 2483 s) | 2193 passed, 1 failed ‡, 767 unsupported (2961; 241 s) |
| stage 1 (kiln++-built) | 35228 passed, 7 failed (6 ‡ + `DirectoryWatcherTest.AddFiles` †), 2734 unsupported, 8 skipped, 26 xfail (38003; lit 4244 s) | 2193 passed, 1 failed ‡, 767 unsupported (2961; 587 s) |
| stage 2 (built by stage 1) | 35229 passed, 6 failed ‡, 2734 unsupported, 8 skipped, 26 xfail (38003; lit 5090 s) | 2193 passed, 1 failed ‡, 767 unsupported (2961; 568 s) |

‡ The control's environmental failures (see
[llvm_clang.md](llvm_clang.md)):
- six check-clang tests that need a case-insensitive file system or a
  header map: `APINotes/case-for-private-apinotes-file.c`,
  `Lexer/case-insensitive-include-absolute.c`,
  `Lexer/case-insensitive-include.c`,
  `Lexer/case-insensitive-system-include.c`,
  `Modules/inferred-framework-case.m` and
  `Preprocessor/nonportable-include-with-hmap.c`;
- lld's `ELF/file-access.s`, which expects a permission error that root
  does not get.

Both stages fail exactly these.

† Classified as environmental: a timing flake.
`DirectoryWatcherTest.AddFiles` (DirectoryWatcherTest.cpp:264) waits a
bounded time for inotify events. It failed once, in a lit run at load
average 25. Afterwards, under the same `ulimit -v 3000000`, the
kiln-built binary
`tools/clang/unittests/DirectoryWatcher/DirectoryWatcherTests` passed
that test alone 10 times out of 10, and passed the whole binary (8/8).
Stage 2's run of the same test, built by stage 1, passed.

### Stage-2/stage-3 comparison

The sample is fixed by the stage-1 log: its `ok` TUs sorted by source
path, every (3565/N)-th. The 200-TU sample holds 121 `llvm/`, 70
`clang/`, 7 `lld/` and 2 generated TUs.

| Sample | output | compilers | identical | differ |
|---|---|---|---|---|
| 200 TUs | `-S -emit-llvm -O2` | Debian clang-19, stage 1, stage 2 | 200 | 0 |
| 200 TUs | `-S -O2` (AArch64 assembly) | Debian clang-19, stage 1, stage 2 | 200 | 0 |
| 1000 TUs (the 200 and 800 more) | `-S -emit-llvm -O2` | Debian clang-19, stage 2 | 1000 (999 before the `!srcloc` normalization) | 0 |

So stage 1 (compiled by kiln++) and stage 2 (compiled by stage 1)
produce the same IR and code as Debian's clang 19.1.7, which kiln
had no part in building. A kiln miscompile in a path these TUs exercise would make
stage 1's output differ. A miscompile that only corrupted code
generation in stage 2 would make stage 2's output differ.

## Compile time and memory (kiln's codegen quality)

Stage 2 and its control compile the same tree with two clang 19.1.7
builds of the same configuration. They differ only in the compiler
that built them: kiln++ or clang++-19, both at `-O3`. So their ratio is
a measure of kiln's generated code. Over the 3541 TUs both compiled
from the same path (`oss-report.py --perf llvm_selfhost`):

- Time: geomean kiln-built/clang-built CPU time is 1.06×, and summed it
  is 1.057× (15766 s against 14911 s).
- RSS: geomean 0.96×, with peak TU RSS of 1161 MB against 1189 MB.

The two runs did not overlap in time. Each shared the host with
different load (load average 18 to 28), so per-TU ratios in the log
are noisy: 1008 of the 3452 TUs that take at least 0.5 s are over
1.2×, and the largest are 2–3× on 1–7 s TUs. Seven TUs were therefore
timed again with both stage-1 compilers interleaved, 3 runs each, and
the minimum CPU time kept (below).

| TU | log kiln-built / clang-built CPU (s) | interleaved min kiln-built / clang-built CPU (s) | ratio | RSS (MB) |
|---|---|---|---|---|
| `clang/lib/Sema/SemaExpr.cpp` | 56.5 / 58.1 | 46.89 / 46.26 | 1.01x | 1030 / 1030 |
| `clang/lib/ASTMatchers/Dynamic/Registry.cpp` | 56.6 / 48.5 | 43.29 / 48.38 | 0.89x | 898 / 899 |
| `llvm/lib/Target/X86/X86ISelLowering.cpp` | 30.9 / 31.8 | 27.69 / 28.50 | 0.97x | 658 / 661 |
| `clang/lib/StaticAnalyzer/Core/Checker.cpp` | 6.7 / 3.0 | 2.93 / 2.94 | 1.00x | 296 / 297 |
| `llvm/lib/MCA/HardwareUnits/Scheduler.cpp` | 2.8 / 1.0 | 0.89 / 0.89 | 1.00x | 136 / 136 |
| `llvm/lib/Bitcode/Reader/ValueList.cpp` | 2.7 / 1.2 | 1.16 / 1.17 | 0.99x | 157 / 157 |
| `clang/lib/Tooling/Syntax/Mutations.cpp` | 3.8 / 1.7 | 1.87 / 1.87 | 1.00x | 219 / 219 |

Interleaved, the geomean is 0.98×. The log's 2–3× outliers all fall to
1.00×, and the heavy TUs scatter ±10% from run to run under the host's
load (Registry.cpp: 43–55 s kiln-built, 48–52 s clang-built). **kiln's
`-O3` code for clang runs as fast as clang-19's, within the noise of
this host.** No perf finding is recorded.

How the TUs were run (the commands, so the timing can be repeated): each
TU's stage-2 command from `.oss/build/llvm_selfhost/kiln/compile.jsonl`,
with `-MD`, `-MT`, `-MF` and `-o` dropped and `-o /dev/null` added. It
was run in the TU's `cwd` from that log as
`/usr/bin/time -f '%U %S %e %M' nice -n 10 $STAGE1/bin/clang++ <args>`,
under `ulimit -v 3000000`, where `$STAGE1` is
`.oss/build/llvm_clang/kiln` for the kiln-built compiler and
`.oss/build/llvm_clang/clang` for the clang-built one. The two compilers
alternated, three runs each per TU. For example, for `SemaExpr.cpp`,
in `$B/tools/clang/lib/Sema`, with `$S` the llvm-project checkout:

```sh
"$STAGE1/bin/clang++" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/lib/Sema" "-I$S/clang/lib/Sema" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -c "$S/clang/lib/Sema/SemaExpr.cpp" -o /dev/null
```

For comparison, the other figures:
- Debian's clang-19, which has no assertions, compiles the tree in
  11613 s of CPU. That makes the assertion-enabled stage-1 clangs about
  30% slower, whoever built them.
- kiln++ itself compiles it in 11344 s. Its geomean CPU over the 3541
  common TUs is 1.03× clang-19's, and its RSS is 0.40× clang-19's
  (`oss-report.py --perf llvm_clang`; peak TU 647 MB against 1108 MB).
  That agrees with WP-801's 1.00× CPU.

## Clusters

None. No stage-1 or stage-2 test failed beyond the control's
environmental failures and one timing flake. No sampled TU's IR or
assembly differs. So no kiln fix and no xfail test were needed.
