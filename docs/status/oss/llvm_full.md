# All of LLVM with kiln++ (WP-782)

LLVM **19.1.7** (tag `llvmorg-19.1.7`, `llvm/` only) built with kiln++ as
`CMAKE_CXX_COMPILER` through LLVM's own CMake build: every library, the
AArch64 and X86 backends, every tool, the unit tests and `check-llvm`
(lit plus the unit tests). The control is a clang++-19 build of the same
tree. Both use the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md). Host: aarch64 Linux, libstdc++ 14, C++17
(the project row), Release with assertions on, `-O3`, no debug info,
default `-fclang-compat`. Date: 2026-11-07.

**Result: builds; check-llvm at parity after three miscompile fixes.**
kiln++ compiles all 2780 TUs (153 at the first run) and links all 165
targets. Run 9's `check-llvm` has 7 failures against the control's 9.
Six are the control's own environmental failures. The seventh,
cluster 17, is fixed after run 9 (`8b18c72fa`) and verified under lit with a rebuilt `llvm-cov`. kiln's
summed TU time is 0.94× clang's and its peak TU RSS 0.63× clang's.

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh llvm_full --compiler clang
scripts/oss/oss-build.sh llvm_full
scripts/oss/oss-report.py llvm_full
```

These run the following in `.oss/build/llvm_full/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`; the row sets no memory-cap override.

```sh
cmake --fresh -S .oss/src/llvm_full/llvm -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_TARGETS_TO_BUILD="AArch64;X86" -DLLVM_ENABLE_PROJECTS= \
  -DLLVM_ENABLE_ASSERTIONS=ON -DLLVM_INCLUDE_TESTS=ON \
  -DLLVM_INCLUDE_BENCHMARKS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_ENABLE_ZSTD=OFF -DLLVM_ENABLE_LIBXML2=OFF \
  -DLLVM_LIT_ARGS="-sv -j 4"
nice -n 10 make clean
nice -n 10 make -k all llvm-test-depends UnitTests   # shared jobserver
timeout 10800 nice -n 10 make check-llvm
```

The project row adds `test_timeout = 10800` and a `lit_summary` reader
for lit's result block (`scripts/oss/projects.py`).

## Results

TUs are the build-phase C++ compiles, counted once each; CMake probes
are excluded. Wall is the whole `oss-build.sh` run (configure, build and
`check-llvm`). Peak TU RSS is the wrapper's `ru_maxrss` for the compiler
plus the backend it waited for.

| Build | TUs ok / failed | Links | check-llvm (lit + unit) | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| clang++-19 (control) | 2780 / 0 | 165 / 165 | 38432 passed, 9 failed ‡, 22121 unsupported, 78 skipped, 65 xfail (60705 discovered) | 3675 s (lit 2029 s) | 932 MB (`lib/Passes/PassBuilder.cpp`) |
| kiln++ run 1 (branch base, `291515e4a`) | 153 / 3 | 3 / 3 | not reached | 277 s | 108 MB (`lib/Demangle/ItaniumDemangle.cpp`) |
| kiln++ run 2 | 734 / 27 | 49 / 49 | not reached | 621 s | 312 MB |
| kiln++ run 3 | 1485 / 12 | 89 / 89 | not reached | 819 s | 409 MB |
| kiln++ run 5 (WP-775/779 merged) | 2375 / 5 | 127 / 127 | not reached | 2351 s | 602 MB |
| kiln++ run 6 | 2592 / 6 | 133 / 133 | not reached | 1199 s | 601 MB |
| kiln++ run 7 | 2780 / 0 | 164 / 166 | not reached (2 links) | 1929 s | 602 MB |
| kiln++ run 8 (`ba625d48d`) | 2780 / 0 | 165 / 165 | 60605 passed, 100 failed (clusters 15, 16 and 6 environmental) | 3334 s | 602 MB (`unittests/Frontend/OpenMPDecompositionTest.cpp`) |
| kiln++ run 9 (`5fa29f705`) | 2780 / 0 | 165 / 165 | 38434 passed, 7 failed (6 ‡ and cluster 17), 22121 unsupported, 78 skipped, 65 xfail (60705 discovered) | 5101 s (lit 1419 s) § | 602 MB (`unittests/Frontend/OpenMPDecompositionTest.cpp`) |

‡ The control's nine failures are environmental and are expected under
kiln too (run 9 hit six of them; the socket tests fail by port race): three Support permission tests that root does not fail
(`FileSystemTest.CreateDir`, `FileSystemTest.RespectUmask`,
`raw_ostreamTest.filePermissions`), four `raw_socket_streamTest` tests
(the container's socket sandbox) and two `llvm-lipo` executable tests.

§ Run 9's wall is longer than run 8's (3334 s) although its summed TU
compile time is lower than the control's (below). It ran while other
agents' builds shared the host's jobserver pool, so its wall is not a
compiler figure.

**Compile time and memory (run 9 vs control, 2752 TUs compiled by
both).** Summed TU wall is 0.94× clang's (6268 s vs 6657 s). 19 TUs
take over 1.5× clang's time, and all but two of them take under 1 s in
clang. Of the 2225 TUs at 1 s or more, Σ is 0.93×. The outliers over
1.5× are `tools/llvm-objdump/WasmDump.cpp` (2.18 s vs 1.12 s, 1.94×) and
`lib/Analysis/Analysis.cpp` (1.66 s vs 1.07 s, 1.55×). They are
recorded, not fixed. The longest kiln TU took 22.8 s. Peak TU RSS is
602 MB against clang's 955 MB. No TU's kiln/clang RSS ratio exceeds
0.85, so there is no RSS finding.

The control build tree was deleted once these numbers were recorded
(4.8 GB); its `compile.jsonl`, logs and the 112 clang-built static
libraries (247 MB) were kept under `.oss/runs/clang/` for per-library
object swaps.

## Clusters

Failed TUs grouped by their first diagnostic, in the order the runs met
them (`make -k` reaches more of the tree each run). Each WP-782 fix has
a test that also passes under `--compiler clang++-19`. WP-775 and
WP-779 landed while this ran, fixed several of the same clusters first
and were merged before run 5; their fixes are listed by their main
commit. Hashes are on `wp/782` unless marked.

| # | Signature (first diagnostic) | TUs | Status | Test |
|---|---|---|---|---|
| 1 | `type 'int' cannot be used prior to '::'` (the [temp.deduct.partial]/12 probe, YAMLParser.cpp) | 1, which blocked the build | fixed on main by WP-779 `07ff6b9d2` (structural walk); WP-782's SFINAE fix was dropped in the merge | `temp.func.order/p12-undeduced-probe-is-sfinae-run.cpp` (WP-775) |
| 2 | `reference to type 'const OptSpecifier[1]' could not bind to … 'const OptSpecifier[0]'` (a dependent braced default argument, Option/ArgList.h:80) | 4 | fixed, `27917d31d` | `dcl.fct.default/member-dependent-array-reference-braced.cpp` |
| 3 | `no viable conversion from 'uint16_t' to 'RecordPrefix'` (`T(args)` with a dependent argument, SimpleTypeSerializer.cpp) | 1 | fixed on main by WP-775 `3c51ce368`; the placeholder (`auto(x)`, CTAD) half is `4725d6128` | `temp.dep.expr/p3-placeholder-functional-cast-type-dependent-run.cpp` |
| 4 | `kiln: not yet implemented: explicit specialization` (a member class through a typedef, CGSCCPassManager.h:173) | 4 | fixed on main by WP-775 `931b9a02a` | `temp.expl.spec/member-class-via-typedef-run.cpp` (WP-775) |
| 5 | `reference to overloaded function could not be resolved` (an out-of-line member template's parameter `size` hidden by members, AArch64InstPrinter.cpp:1609) | 1 | fixed, `7d37fd9d6` | `temp.local/p7-member-template-param-not-hidden-out-of-line.cpp` |
| 6 | `type 'llvm::RegionNode *' cannot be used prior to '::'` (GraphTraits.h:80) | 3 | fixed on main by WP-775 `ba8d190b7`, `e77145dff` | WP-775's |
| 7 | `member access into incomplete type 'llvm::vfs::FileSystem'` (IntrusiveRefCntPtr.h:163, from an unused default argument) | 3 | fixed on main by WP-775 `7c7e5a8f6` | WP-775's |
| 8 | `'llvm::Function::isTargetIntrinsic' is not a member of class 'BasicTTIImplBase<…>'` (LLVMTargetMachine.cpp) | 1 | fixed on main by WP-775 `a2a5e4967` | WP-775's |
| 9 | `no matching member function for call to 'members_if'` (RDFLiveness.cpp:343) | 1 | fixed on main by WP-775 `e2ee75f57` | WP-775's |
| 10 | `'…ReciprocalEstimate::Disabled' is not a member of class 'const TargetLowering'` (`d.E::e` through an unscoped enum of a base, DAGCombiner.cpp) | 1 | fixed, `8d189965d` | `expr.ref/qualified-enumerator-of-base-class-run.cpp` |
| 11 | `no type named 'Lazy' in 'llvm::GenericDomTreeUpdater<…>'` (vexing parse through an uninstantiated member enum, CloningTest.cpp) | 1 | fixed, `f1539ee81` | `dcl.ambig.res/member-enum-of-base-specialization-enumerator-run.cpp` |
| 12 | `non-constant-expression cannot be narrowed from type '(unnamed enum …)' to 'int'` (an enum-to-promotion-type conversion, X86FixupVectorConstants.cpp) | 1 | fixed, `795564a02` | `dcl.init.list/enum-promotion-not-narrowing-run.cpp`, `enum-conversion-still-narrowing.cpp` |
| 13 | `use of undeclared identifier '__builtin_thread_pointer'` (llvm-exegesis BenchmarkRunner.cpp) | 1 | fixed, `c7dca9f9e` | `ext.gnu.builtins/thread-pointer-run.cpp`, `thread-pointer-not-constant.cpp`, `codegen/builtins_thread_pointer.cpp` |
| 14 | link: undefined `llvm.thread.pointer.p0` (the LLVM 20 overloaded name sent to the LLVM 19 backend) | 2 links | fixed, `ba625d48d` (keyed on `llvm_major`) | `codegen/builtins_thread_pointer.cpp` under `KILN_ORACLE=19` |
| 15 | check-llvm (miscompile): `unencodable update target version` / `MachObjectWriter.cpp` `llvm_unreachable` in every Mach-O `llc`/`llvm-lto`/`dsymutil` test (an unnamed `struct` in `using VersionInfo = struct {...}` treated as anonymous, so its default member initializers were dropped) | 0 TUs, 88 tests | fixed, `d0755b82f` | `class.union.anon/alias-of-unnamed-class-is-not-anonymous-run.cpp` |
| 16 | check-llvm (miscompile): `dyn_cast on a non-existent value` in `ScalarEvolution::computeLoopDisposition` (4 `IndVarSimplify` and 2 `LoopUnrollAndJam` tests under `-verify-scev`; `const SCEV *const &` bound to `this` lost its +32 base adjustment) | 0 TUs, 6 tests | fixed, `5fa29f705` | `dcl.init.ref/const-pointer-ref-to-derived-pointer-run.cpp` |
| 17 | check-llvm (miscompile): `malformed coverage data: the value of ULEB128 is too big` (`llvm-cov` on 32-bit Mach-O; a `#pragma pack(1)` class template instantiated unpacked, `CovMapFunctionRecordV1<uint32_t>` 24 bytes, not 20) | 0 TUs, 1 test | fixed, `8b18c72fa` | `cpp.pragma/pack-template-instantiation-run.cpp` |

Run 4 was stopped by hand; its two `killed by signal 15` records are not
a cluster. All test paths are under `tests/conformance/` unless they
start with `codegen/`.

## What remains

- Cluster 17 was fixed after run 9. It was verified by recompiling
  `CoverageMappingReader.cpp` in run 9's tree with the fixed kiln++,
  relinking `llvm-cov` and rerunning `tools/llvm-cov/binary-formats.c`
  under lit, which passed. A full run 10 was not made.
- Miscompiles were bisected by swapping kiln's objects for the
  control's: whole libraries, then single objects, then single
  functions (`llvm-extract` and `llvm-link` over `-O0` IR).
- A lambda's `$_N` closure name (one with no mangling number) is
  numbered differently from clang's (`$_1` against
  `$_0` for `GroupByComplexity`'s lambdas in `ScalarEvolution.cpp`).
  These are internal names, so there is no ABI effect, but object
  diffs against clang are noisier (`crates/kiln_abi/src/mangle.rs:2066`,
  `AnonIds`).
- Perf outliers over 1.5× clang: `WasmDump.cpp` and `Analysis.cpp`
  (above).
- Variants not run: Debug, assertions off, C++20, libc++, and the other
  backends.
