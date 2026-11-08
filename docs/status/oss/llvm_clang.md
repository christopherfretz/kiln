# clang and lld with kiln++ (WP-801)

LLVM **19.1.7** (tag `llvmorg-19.1.7`, the `llvm/` tree with
`LLVM_ENABLE_PROJECTS=clang;lld`) built with kiln++ as
`CMAKE_CXX_COMPILER` through LLVM's own CMake build: the LLVM libraries
and tools, the AArch64 and X86 backends, clang, lld, `libclang-cpp.so`
and both projects' unit tests. Then `check-clang` and `check-lld` run
(lit plus the unit tests). The control is a clang++-19 build of the same
tree. Both use the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md) and the `llvm_clang` project row. Host:
aarch64 Linux, libstdc++ 14, C++17 (the project row), Release with
assertions on, `-O3`, no debug info, default `-fclang-compat`. Date:
2026-11-07/08.

**Result: builds; check-lld at parity, check-clang at parity after
one more miscompile fix.** kiln++ compiles all 3565 TUs (2712 at the
first run, 526 failing) and links all 157 targets. Run 6's `check-lld`
matches the control: 2193 passed and the same environmental failure.
Run 6's `check-clang` has 8 failures against the control's 6. Six are
the control's own environmental failures. The other two are
`ClangAnalysisFlowSensitiveTests` shards that cluster 13 crashed. That
cluster is fixed after run 6 (`4f7b87c3e`). With its object recompiled and the
binary relinked, the binary passes all 671 of its tests under the cap. That
gives back the 32 tests the two crashed shards hid: 35197 + 32 = 35229, the
control's count. The WP met 13 clusters and one link failure. Ten were fixed on `wp/801`; the
other four were fixed on main during the WP. No xfail test was added.
kiln's peak TU RSS is 648 MB against clang's 1106 MB, and no TU uses
more than 0.81× clang's memory. Summed TU CPU time is 1.00× clang's,
and five TUs over 1 s are above 1.5× clang's CPU (recorded below).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh llvm_clang --compiler clang
ulimit -v 3000000                              # caps the test phase too (below)
scripts/oss/oss-build.sh llvm_clang
scripts/oss/oss-report.py llvm_clang
```

These run the following in `.oss/build/llvm_clang/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600` to each compile and link; the row sets no memory-cap
override.

```sh
cmake --fresh -S .oss/src/llvm_clang/llvm -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_TARGETS_TO_BUILD="AArch64;X86" -DLLVM_ENABLE_PROJECTS="clang;lld" \
  -DLLVM_ENABLE_ASSERTIONS=ON -DLLVM_INCLUDE_TESTS=ON \
  -DLLVM_INCLUDE_BENCHMARKS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_ENABLE_ZSTD=OFF -DLLVM_ENABLE_LIBXML2=OFF \
  -DLLVM_LIT_ARGS="-sv -j 4"
nice -n 10 make clean
nice -n 10 make -k all clang-test-depends ClangUnitTests \
  lld-test-depends LLDUnitTests                      # shared jobserver
timeout 18000 nice -n 10 sh -c 'make check-clang; make check-lld'
```

The wrapper caps only the compiler. The test phase runs the built
binaries directly, so the final kiln run (run 6) was started from a
shell with `ulimit -v 3000000`, which every test binary inherits. Unit
test binaries were run one at a time when they were checked by hand.
This rule came from an incident during the WP: a kiln-built
`ClangAnalysisFlowSensitiveTests`, run by hand without a cap, grew past
10 GB in 15 s and the host's OOM killer fired. That binary was cluster
11's miscompile (below).

`report.json`'s `tests` block is `check-lld`'s lit summary, the last
one in the log; `check-clang`'s figures below are read from `test.log`.

## Results

TUs are the build-phase C++ compiles, counted once each; CMake probes
are excluded. Wall is the whole `oss-build.sh` run (configure, build
and both checks). Peak TU RSS is the wrapper's `ru_maxrss` for the
compiler plus the backend it waited for.

| Build | TUs ok / failed | Links | check-clang (lit + unit) | check-lld | Wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| clang++-19 (control) | 3565 / 0 | 157 / 157 | 35229 passed, 6 failed ‡, 2734 unsupported, 8 skipped, 26 xfail (38003 discovered; lit 2483 s) | 2193 passed, 1 failed ‡, 767 unsupported (2961; 241 s) | 7951 s | 1106 MB (`clang/unittests/Tooling/SourceCodeTest.cpp`) |
| kiln++ run 1 (`ca211f477`) | 2712 / 526 | 107 / 107 | not reached | not reached | 5452 s | 419 MB (`llvm/lib/Passes/PassBuilder.cpp`) |
| kiln++ run 2 (`b2d83d369`) | stopped by hand after clusters 2-5 were fixed | | | | | |
| kiln++ run 3 (`fa7c9c34b`) | 3563 / 2 | 154 / 155 (`libclang-cpp.so`: response file) | not reached (`BasicTests`, `RangeSetTest` did not compile) | 2193 passed, 1 failed ‡, 767 unsupported (2961; 299 s) | 8403 s § | 720 MB (`clang/unittests/Tooling/SourceCodeTest.cpp`) |
| kiln++ run 3b (run 3's tree, clusters 7-8 recompiled) | 3565 / 0 | as run 3 | 34887 passed, 49 failed (6 ‡; clusters 9-12), 2734 unsupported, 8 skipped, 26 xfail (37704 discovered; 2711 s) | as run 3 | | as run 3 |
| kiln++ run 6 (`7805ab79b`, under `ulimit -v 3000000`) | 3565 / 0 | 157 / 157 | 35197 passed, 8 failed (6 ‡; 2 `ClangAnalysisFlowSensitiveTests` shards, cluster 13), 2734 unsupported, 8 skipped, 26 xfail (37973 discovered; lit 3436 s) | 2193 passed, 1 failed ‡, 767 unsupported (2961; 424 s) | 13857 s § | 648 MB (`clang/unittests/Tooling/SourceCodeTest.cpp`) |

‡ The control's seven failures are environmental and are expected
under kiln too: six check-clang tests that need a case-insensitive file
system or a header map with one (`APINotes/case-for-private-apinotes-file.c`,
`Lexer/case-insensitive-include-absolute.c`,
`Lexer/case-insensitive-include.c`,
`Lexer/case-insensitive-system-include.c`,
`Modules/inferred-framework-case.m`,
`Preprocessor/nonportable-include-with-hmap.c`), and lld's
`ELF/file-access.s`, which expects a permission error that root does
not get.

§ Runs 3 and 6 shared the host's jobserver pool with other agents'
builds, so their wall is not a compiler figure; the summed TU times below
are. Run 6's lit phases alone took 3436 s and 424 s, against the
control's 2483 s and 241 s for the same tests.

lit counts a gtest shard that crashes as one failed test, not as the
tests it holds. So run 3b discovered 299 fewer tests than the control and
run 6 discovered 30 fewer: run 6's two crashed shards hid 32 tests.

Top-5 peak TU RSS:

| Build | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| clang++-19 | `SourceCodeTest.cpp` 1105 MB | `SemaExpr.cpp` 981 MB | `CodeGenModule.cpp` 952 MB | `PassBuilder.cpp` 931 MB | `RangeSetTest.cpp` 925 MB |
| kiln++ run 1 | `PassBuilder.cpp` 419 MB | `ELFDumper.cpp` 400 MB | `X86ISelLowering.cpp` 381 MB | `AttrImpl.cpp` 343 MB | `BackendUtil.cpp` 341 MB |
| kiln++ run 3 | `SourceCodeTest.cpp` 720 MB | `ASTImporterTest.cpp` 622 MB | `SemaExpr.cpp` 571 MB | `Registry.cpp` 567 MB | `ASTMatchersTraversalTest.cpp` 536 MB |
| kiln++ run 6 | `SourceCodeTest.cpp` 647 MB | `ASTImporterTest.cpp` 621 MB | `Registry.cpp` 566 MB | `ASTMatchersTraversalTest.cpp` 536 MB | `SemaExpr.cpp` 507 MB |

## Compile time and memory

Run 6 against the control, over the 3541 TUs both compiled from the
same path. Sources generated into each build tree have different paths
and are left out. Summed TU wall is 1.075× clang's (12038 s vs 11198 s),
and 1.065× over the 2820 TUs that take clang 1 s or more. Summed TU CPU
time is 1.002× clang's. 251 TUs are over 1.5× clang's wall, 96 of them at 1 s or
more in clang. The run shared the host's jobserver pool with other
agents' builds, and CPU time puts most of them back under 1.5×. 51 TUs are
over 1.5× clang's CPU time, and five of them take clang 1 s or more:

| TU | run 6 kiln / clang CPU (s) | ratio | RSS kiln / clang (MB) |
|---|---|---|---|
| `clang/unittests/Lex/HeaderMapTest.cpp` | 2.12 / 1.23 | 1.72x | 69 / 195 |
| `clang/unittests/Format/FormatTestTextProto.cpp` | 2.59 / 1.54 | 1.68x | 73 / 206 |
| `clang/unittests/Basic/CharInfoTest.cpp` | 1.83 / 1.12 | 1.64x | 80 / 173 |
| `llvm/lib/Target/X86/Disassembler/X86Disassembler.cpp` | 3.83 / 2.51 | 1.53x | 182 / 355 |
| `clang/unittests/Basic/DarwinSDKInfoTest.cpp` | 1.97 / 1.31 | 1.50x | 72 / 206 |

These are recorded, not fixed. No TU is over 1.5× clang's RSS. The
largest kiln/clang RSS ratio is 0.81 (`clang/unittests/Format/FormatTest.cpp`),
and kiln's peak TU (648 MB) is below clang's (1106 MB).

The exact command lines of the five, for project tag `llvmorg-19.1.7`
(`$S`, `$B` and `$CXX` as described below the run 3 table). Each line gives
run 6's wall time, CPU time and peak RSS for both compilers:

- `clang/unittests/Lex/HeaderMapTest.cpp`, in `$B/tools/clang/unittests/Lex` (run 6: kiln 3.654 s wall, 2.119 s CPU, 69 MB; control: clang 1.433 s wall, 1.234 s CPU, 195 MB):

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/unittests/Lex" "-I$S/clang/unittests/Lex" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" "-I$S/third-party/unittest/googletest/include" "-I$S/third-party/unittest/googlemock/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -Wno-variadic-macros -Wno-gnu-zero-variadic-macro-arguments -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -Wno-suggest-override -MD -MT tools/clang/unittests/Lex/CMakeFiles/LexTests.dir/HeaderMapTest.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/clang/unittests/Lex/HeaderMapTest.cpp"
  ```

- `clang/unittests/Format/FormatTestTextProto.cpp`, in `$B/tools/clang/unittests/Format` (run 6: kiln 3.781 s wall, 2.592 s CPU, 73 MB; control: clang 1.671 s wall, 1.544 s CPU, 206 MB):

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/unittests/Format" "-I$S/clang/unittests/Format" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" "-I$S/third-party/unittest/googletest/include" "-I$S/third-party/unittest/googlemock/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -Wno-variadic-macros -Wno-gnu-zero-variadic-macro-arguments -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -Wno-suggest-override -MD -MT tools/clang/unittests/Format/CMakeFiles/FormatTests.dir/FormatTestTextProto.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/clang/unittests/Format/FormatTestTextProto.cpp"
  ```

- `clang/unittests/Basic/CharInfoTest.cpp`, in `$B/tools/clang/unittests/Basic` (run 6: kiln 3.074 s wall, 1.834 s CPU, 80 MB; control: clang 1.247 s wall, 1.119 s CPU, 173 MB):

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/unittests/Basic" "-I$S/clang/unittests/Basic" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" "-I$S/third-party/unittest/googletest/include" "-I$S/third-party/unittest/googlemock/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -Wno-variadic-macros -Wno-gnu-zero-variadic-macro-arguments -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -Wno-suggest-override -MD -MT tools/clang/unittests/Basic/CMakeFiles/BasicTests.dir/CharInfoTest.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/clang/unittests/Basic/CharInfoTest.cpp"
  ```

- `llvm/lib/Target/X86/Disassembler/X86Disassembler.cpp`, in `$B/lib/Target/X86/Disassembler` (run 6: kiln 4.588 s wall, 3.829 s CPU, 182 MB; control: clang 2.757 s wall, 2.51 s CPU, 355 MB):

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/Target/X86/Disassembler" "-I$S/llvm/lib/Target/X86/Disassembler" "-I$S/llvm/lib/Target/X86" "-I$B/lib/Target/X86" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fvisibility=hidden -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/Target/X86/Disassembler/CMakeFiles/LLVMX86Disassembler.dir/X86Disassembler.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/Target/X86/Disassembler/X86Disassembler.cpp"
  ```

- `clang/unittests/Basic/DarwinSDKInfoTest.cpp`, in `$B/tools/clang/unittests/Basic` (run 6: kiln 3.428 s wall, 1.967 s CPU, 72 MB; control: clang 1.466 s wall, 1.308 s CPU, 206 MB):

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/unittests/Basic" "-I$S/clang/unittests/Basic" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" "-I$S/third-party/unittest/googletest/include" "-I$S/third-party/unittest/googlemock/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -Wno-variadic-macros -Wno-gnu-zero-variadic-macro-arguments -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -Wno-suggest-override -MD -MT tools/clang/unittests/Basic/CMakeFiles/BasicTests.dir/DarwinSDKInfoTest.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/clang/unittests/Basic/DarwinSDKInfoTest.cpp"
  ```


Run 3 (3539 TUs compiled by both from the same path) gave the same picture: summed TU wall
1.03× clang's (11510 s vs 11181 s), 1.015× over the TUs at 1 s or more,
and no TU's kiln/clang RSS ratio above 0.81. It listed 266 TUs over
1.5× clang's time, 54 of them at 1 s or more in clang. Those runs share
the host with other agents' builds, so a single TU's wall in the log is
noisy. The outliers with the largest absolute gap were recompiled one at
a time on an idle jobserver slot, both compilers back to back, with the
build's own flags. Isolated, all but one fall under 1.5×:

| TU | run 3 kiln / clang (s) | isolated kiln / clang (s) | ratio | isolated RSS kiln / clang (MB) |
|---|---|---|---|---|
| `clang/lib/AST/DeclCXX.cpp` | 8.46 / 3.79 | 4.72 / 3.96 | 1.19x | 127 / 318 |
| `clang/lib/Sema/SemaARM.cpp` | 12.27 / 7.03 | 8.24 / 6.93 | 1.19x | 223 / 479 |
| `llvm/lib/MC/MCExpr.cpp` | 3.77 / 1.52 | 1.59 / 1.28 | 1.24x | 72 / 191 |
| `llvm/lib/MC/MCFragment.cpp` | 2.84 / 1.21 | 1.21 / 1.07 | 1.13x | 66 / 191 |
| `llvm/tools/llvm-config/llvm-config.cpp` | 3.08 / 1.12 | 1.74 / 1.32 | 1.32x | 74 / 155 |
| `llvm/lib/Target/AArch64/AArch64MIPeepholeOpt.cpp` | 3.68 / 2.21 | 3.19 / 2.63 | 1.21x | 120 / 304 |
| `llvm/lib/TextAPI/RecordsSlice.cpp` | 3.00 / 1.38 | 1.69 / 1.23 | 1.37x | 74 / 177 |
| `llvm/lib/TextAPI/PackedVersion.cpp` | 0.75 / 0.51 | 0.87 / 0.50 | 1.74x | 64 / 142 |
| `llvm/lib/MC/MCParser/DarwinAsmParser.cpp` | 1.69 / 1.50 | 1.89 / 1.53 | 1.24x | 71 / 206 |

`PackedVersion.cpp` stays over 1.5× in isolation (0.87 s vs 0.50 s).
It is recorded, not fixed. kiln's RSS is below clang's for every one of
them.

The exact command lines follow, for project tag `llvmorg-19.1.7`. `$S` is
the source checkout (`.oss/src/llvm_clang`), `$B` the build tree
(`.oss/build/llvm_clang/<kiln|clang>`), and `$CXX` either kiln++ (the
harness profile's `kiln++`) or `clang++-19`. The two commands differ
only in `$CXX`. Each was run from the directory named, as
`/usr/bin/time -v`, with `-o` and `-MF` pointing at scratch files.

- `clang/lib/AST/DeclCXX.cpp`, in `$B/tools/clang/lib/AST`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/lib/AST" "-I$S/clang/lib/AST" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT tools/clang/lib/AST/CMakeFiles/obj.clangAST.dir/DeclCXX.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/clang/lib/AST/DeclCXX.cpp"
  ```

- `clang/lib/Sema/SemaARM.cpp`, in `$B/tools/clang/lib/Sema`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/clang/lib/Sema" "-I$S/clang/lib/Sema" "-I$S/clang/include" "-I$B/tools/clang/include" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -fno-common -Woverloaded-virtual -Wno-nested-anon-types -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT tools/clang/lib/Sema/CMakeFiles/obj.clangSema.dir/SemaARM.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/clang/lib/Sema/SemaARM.cpp"
  ```

- `llvm/lib/MC/MCExpr.cpp`, in `$B/lib/MC`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/MC" "-I$S/llvm/lib/MC" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/MC/CMakeFiles/LLVMMC.dir/MCExpr.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/MC/MCExpr.cpp"
  ```

- `llvm/lib/MC/MCFragment.cpp`, in `$B/lib/MC`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/MC" "-I$S/llvm/lib/MC" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/MC/CMakeFiles/LLVMMC.dir/MCFragment.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/MC/MCFragment.cpp"
  ```

- `llvm/tools/llvm-config/llvm-config.cpp`, in `$B/tools/llvm-config`:

  ```sh
  "$CXX" "-DCMAKE_CFG_INTDIR="Release"" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/tools/llvm-config" "-I$S/llvm/tools/llvm-config" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT tools/llvm-config/CMakeFiles/llvm-config.dir/llvm-config.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/tools/llvm-config/llvm-config.cpp"
  ```

- `llvm/lib/Target/AArch64/AArch64MIPeepholeOpt.cpp`, in `$B/lib/Target/AArch64`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/Target/AArch64" "-I$S/llvm/lib/Target/AArch64" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fvisibility=hidden -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/Target/AArch64/CMakeFiles/LLVMAArch64CodeGen.dir/AArch64MIPeepholeOpt.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/Target/AArch64/AArch64MIPeepholeOpt.cpp"
  ```

- `llvm/lib/TextAPI/RecordsSlice.cpp`, in `$B/lib/TextAPI`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/TextAPI" "-I$S/llvm/lib/TextAPI" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/TextAPI/CMakeFiles/LLVMTextAPI.dir/RecordsSlice.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/TextAPI/RecordsSlice.cpp"
  ```

- `llvm/lib/TextAPI/PackedVersion.cpp`, in `$B/lib/TextAPI`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/TextAPI" "-I$S/llvm/lib/TextAPI" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/TextAPI/CMakeFiles/LLVMTextAPI.dir/PackedVersion.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/TextAPI/PackedVersion.cpp"
  ```

- `llvm/lib/MC/MCParser/DarwinAsmParser.cpp`, in `$B/lib/MC/MCParser`:

  ```sh
  "$CXX" -DGTEST_HAS_RTTI=0 -D_DEBUG -D_GLIBCXX_ASSERTIONS -D_GNU_SOURCE -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS "-I$B/lib/MC/MCParser" "-I$S/llvm/lib/MC/MCParser" "-I$B/include" "-I$S/llvm/include" -fPIC -fno-semantic-interposition -fvisibility-inlines-hidden -Werror=date-time -Werror=unguarded-availability-new -Wall -Wextra -Wno-unused-parameter -Wwrite-strings -Wcast-qual -Wmissing-field-initializers -pedantic -Wno-long-long -Wc++98-compat-extra-semi -Wimplicit-fallthrough -Wcovered-switch-default -Wno-noexcept-type -Wnon-virtual-dtor -Wdelete-non-virtual-dtor -Wsuggest-override -Wstring-conversion -Wmisleading-indentation -Wctad-maybe-unsupported -ffunction-sections -fdata-sections -O3 -DNDEBUG -std=c++17 -fno-exceptions -funwind-tables -fno-rtti -UNDEBUG -MD -MT lib/MC/MCParser/CMakeFiles/LLVMMCParser.dir/DarwinAsmParser.cpp.o -MF /tmp/x.d -o /tmp/x.o -c "$S/llvm/lib/MC/MCParser/DarwinAsmParser.cpp"
  ```

## Clusters

Failed TUs and kiln-only test failures, in the order the runs met them.
Every fix has a test under `tests/conformance/` that also passes under
`--compiler clang++-19` (the two `-ir` tests compare against the clang 22
oracle and declare `requires: clang>=22`). Hashes are on `wp/801` unless
marked. Miscompiles were bisected by swapping kiln's objects for the
control's: whole static libraries from `.oss/runs/clang/lib`, then single
objects, then the reduced source.

| # | Signature (first diagnostic, or failing tests) | TUs | Status | Test |
|---|---|---|---|---|
| 1 | `cannot cast 'CXXParenListInitExpr' to its private base class 'TrailingObjects<…>'` (425 TUs; `CGFunctionInfo` 81) (`friend class TrailingObjects;` did not find the injected-class-name in a base, `llvm/Support/TrailingObjects.h`) | 506 | fixed, `7e93804a4` | `dcl.type.elab/friend-class-found-in-base-class-scope-run.cpp` |
| 2 | `incompatible operand types ('clang::FileEntryRef' and 'FileEntry *')` (the [expr.cond]/6 built-in candidates, `PPDirectives.cpp`, `PPMacroExpansion.cpp`) | 2 | fixed on main by WP-787 `0bfbb7d0e`; `3ee08704a`'s tests kept | `expr.cond/p6-builtin-candidates-convert-class-arm-run.cpp`, `p6-builtin-candidates-ambiguous.cpp` |
| 3 | `expected namespace name` (`using namespace index;` hidden by `::index` from `<strings.h>`, `clang/lib/Index`) | 3 | fixed, `5b26a3342` | `namespace.udir/p1-function-does-not-hide-namespace-run.cpp` |
| 4 | `no matching function for call to 'operator new'` (placement argument `ASTContext`-derived pointer, `Decl.cpp`, `DeclCXX.cpp`, `DeclObjC.cpp`) | 3 | fixed, `43a885b35` | `expr.new/p19-placement-arg-derived-to-base-pointer-run.cpp` |
| 5 | `'getObj' following the 'template' keyword does not refer to a template` (`this->Base::getObj<T>()`, `lld/ELF/InputFiles.h:245`) | 12 | fixed, `b2d83d369` | `temp.names/p3-qualified-member-template-id-dependent-args-run.cpp` |
| 6 | cluster 2's first fix also ran the /6 step for arms of one class differing only in cv (`c ? DB() << x : DB()` became `bool`, `clang/lib/Parse/Parser.cpp`) | 1 (run 2) | fixed, `fa7c9c34b`; main's WP-787 code has the same guard | `expr.cond/p4-cv-differing-class-arms-not-builtin-run.cpp` |
| 7 | `out-of-line definition of 'DiagnosticsTestHelper' does not match any declaration in namespace 'clang'` (a qualified definition of a hidden friend, `unittests/Basic/DiagnosticTest.cpp`) | 1 | fixed, `3eaedfa44`, `41d9f7ff4` | `dcl.meaning/qualified-definition-of-hidden-friend-run.cpp` |
| 8 | `cannot create a non-constant pointer to member function` (`&Self::f<Args>` lost its qualifier on instantiation, `unittests/StaticAnalyzer/RangeSetTest.cpp`) | 1 | fixed, `2c6fdeb4f` | `expr.unary.op/p3-member-template-specialization-pointer-to-member-in-template-run.cpp` |
| 9 | check-clang (miscompile): 19 `InterfaceStubs/*` tests (`return {a, b};` kept NRVO) | 0 TUs, 19 tests | fixed on main by WP-799 `f1bcb5676` | WP-799's |
| 10 | check-clang (miscompile): `Index/skip-parsed-bodies/compile_commands.json`, `Tooling/auto-detect-from-source.cpp`, `Tooling/clang-check-mac-libcxx-abspath.cpp`, `-relpath.cpp` ("no compilation database": `static int D = S;` with `volatile S` dropped as unused, so the anchor no longer pulled `JSONCompilationDatabase.o` out of `libclangTooling.a`) | 0 TUs, 4 tests | fixed, `237dc4050` | `basic.start.dynamic/unused-static-volatile-read-init-ir.cpp` |
| 11 | check-clang (miscompile): 11 `ClangAnalysisFlowSensitiveTests` shards, an `llvm::any_cast` assertion in `ConstantPropagationTest`, and unbounded memory growth without assertions (lambdas of `checkDataflow<(anonymous)::ConstantPropagationAnalysis>` emitted `linkonce_odr`, so two TUs' different lambdas merged) | 0 TUs, 11 shards | fixed, `7805ab79b` | `basic.link/local-entity-of-internal-specialization-ir.cpp` |
| 12 | check-clang (miscompile): 9 `ToolingTests` `InterpolateTest.*` (a garbage `CommandLine.back()` and a double free; bisected to `InterpolatingCompilationDatabase.cpp.o` in `libclangTooling.a`) | 0 TUs, 9 tests | fixed on main before the WP's last merge: the object compiled by run 3's kiln++ fails, the one compiled after the merge passes | none added (main's) |
| 13 | check-clang run 6 (miscompile): 2 `ClangAnalysisFlowSensitiveTests` shards, `ChromiumCheckModelTest.*` threw `std::bad_alloc` under the `ulimit -v` cap. In a function template, `FileContentMappings FileContents = {{"check.h", H1}, {"othercheck.h", H2}}` was instantiated as `{{...}}`, which gave one pair whose second `std::string` was built from two unrelated pointers by the iterator-pair constructor. Bisected to `ChromiumCheckModelTest.cpp.o` by a per-TU swap. | 0 TUs, 2 shards | fixed after run 6, `4f7b87c3e`; the TU recompiled and the binary relinked pass 671/671 under the cap | `dcl.init.list/initializer-list-ctor-in-template-keeps-list-run.cpp` |
| link | `libclang-cpp.so`: the link line's `@rsp` response file was not expanded | 1 link | fixed on main by WP-799 `9c2ded71e` | WP-799's |

No xfail test was added.

## Disk

Measured with `du -sh` after run 6.

- `.oss/`: 16 GB in total.
  - The kiln build tree `.oss/build/llvm_clang/kiln`: 11 GB.
  - The source checkout `.oss/src/llvm_clang`: 2.2 GB.
  - `.oss/runs`: 2.1 GB. This holds each run's `compile.jsonl`, `build.log`, `test.log` and `report.json`, plus the control's static libraries and generated `.inc` files, which the object swaps and the `.inc` diff used.
  - The control's build tree was deleted after its libraries were saved.
- The cargo target directory: 11 GB.

## What remains

- Cluster 4's fix makes the derived-to-base placement match, but such a
  match is not ranked against a `void *` candidate by [over.ics.rank]
  (spec/sema-overload.md, operator new). LLVM did not need the ranking.
- Cluster 5's fix covers `this->Base::g<T>()`; the parse of
  `(*this).Base::g<T>()` is a separate gap (spec/sema-templates.md).
- `PackedVersion.cpp` compiles 1.74× slower than clang in isolation
  (above); recorded, not fixed.
- kiln warns about `gsl::Owner` and `gsl::Pointer` as unknown attributes
  in clang's headers; the build is unaffected (the attribute work is
  WP-805's).
- A lambda's `$_N` closure name in an internal-linkage function is
  numbered in source order, not in clang's first-mangled order
  (docs/status/oss/llvm_full.md, What remains); no test here depended
  on it.
- A dependent `decltype` return type is mangled `u7unknown` when it calls
  a member through a non-dependent pointer. For example,
  `-> decltype(P::Get(m).M(v, l->stream()))` with `L *l` gives
  `_ZN2MBIiE1fI3PolEEu7unknownRKS0_RKiP1L`, where clang writes
  `DTcldtclsrT_3Getfp_E1Mfp0_clptfp1_6streamEE`. Without `l->stream()`
  the mangling matches clang's. gtest's two `MatcherBase::MatchAndExplainImpl`
  overloads are written this way (`gtest-matchers.h:333`). The symbol is
  weak, so kiln objects linked with clang objects would not share it.
  Within one compiler the two overloads cannot both be viable, so the build
  is unaffected. The fallback is in `crates/kiln_abi/src/mangle.rs`
  (`u7unknown`).
- A closure type in a default member initializer
  (`std::function<…> SolverFactory = [] {…}`) is mangled with the Itanium
  ABI's data-member prefix (`13SolverFactoryMUlvE_`). clang 19 omits the
  prefix (`UlvE_`). The symbols are internal here.
- Variants not run: Debug, assertions off, C++20, libc++, the other
  backends, and the other LLVM projects (lldb, mlir, flang, the
  runtimes).
