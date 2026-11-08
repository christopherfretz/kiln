# OpenCV with kiln++ (WP-803)

This page covers OpenCV **4.12.0** (tag `4.12.0`). It was built with
kiln++ as `CMAKE_CXX_COMPILER` through OpenCV's own CMake build and CTest
suite. The control is a clang++-19 build of the same tree, and the
harness is M8's (`scripts/oss/`, spec/landed/harness/WP-666.md, WP-803).

The modules are `core`, `imgproc`, `imgcodecs`, `features2d`, `flann`,
`calib3d` and `ts` (the test framework; without it no tests are built).
OpenCV is dense in NEON intrinsics: the universal intrinsics
(`intrin_neon.hpp`) are the baseline, and the configure step compiles
check programs to decide which dispatch targets (`NEON_DOTPROD`,
`NEON_FP16`, `NEON_BF16`) get their own TUs. A compiler that fails such
a check silently drops those TUs, so the dispatch line of
`configure.log` is compared too: both builds have `NEON_DOTPROD
NEON_FP16 NEON_BF16`.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** All 635 TUs compile, all 19 targets link and all 7
tests pass, under both compilers.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh opencv --compiler clang
scripts/oss/oss-build.sh opencv
scripts/oss/oss-report.py --perf opencv
```

The configure flags are the `opencv` row of `scripts/oss/projects.toml`
(`-DBUILD_LIST=core,imgproc,features2d,flann,calib3d,ts`,
`-DBUILD_TESTS=ON`, `-DBUILD_PERF_TESTS=OFF`, and IPP, ITT, OpenCL,
FFmpeg, GTK, GStreamer, V4L, protobuf, ADE, Eigen and LAPACK off), with
`-DCMAKE_BUILD_TYPE=Release` and `-DCMAKE_CXX_STANDARD=17`. Then:

```sh
nice -n 10 make -k
unset OPENCV_TEST_DATA_PATH
export OPENCV_FOR_THREADS_NUM=4 \
  GTEST_FILTER=-$(grep -v '^#' scripts/oss/opencv-gtest-exclude.txt | paste -sd:)
ctest --output-on-failure -j 4 -E opencv_test_imgcodecs
```

There is no `opencv_extra` test data in the container. The 480 gtest
cases that fail under the clang-19 control without it are listed in
`scripts/oss/opencv-gtest-exclude.txt` and excluded for both compilers.
`opencv_test_imgcodecs` segfaults on its missing data under the control,
so it is excluded as a whole (its TUs are still compiled and linked).

## Results

Time is the wrapper's CPU time (user plus system) per TU. RSS is the
TU's `ru_maxrss`, which includes the backend.

| Build | TUs ok / failed | Links | Tests | Wall | Peak TU RSS |
|---|---|---|---|---|---|
| kiln, first (WP-803 base) | 120 / 39 (159 attempted) | 0 / 0 | 0/7 | 183 s | 82 MB |
| clang++-19 (control) | 635 / 0 | 19 / 19 | 7/7 | 1091 s | 356 MB (`modules/core/test/test_intrin.cpp`) |
| **kiln, final** | **635 / 0** | **19 / 19** | **7/7** | **997 s** | **272 MB** (`modules/core/test/test_intrin.cpp`) |

Over the 628 TUs both logs time (the seven `opencl_kernels_*.cpp` and
dispatch sources generated into the build directory have different
paths), kiln/clang has a geomean of **1.22x time** and **0.43x RSS**.
The CPU totals are 842 s and 702 s. The five heaviest TUs (by clang
time):

| TU | clang s | kiln s | time x | clang MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `modules/core/test/test_intrin_emulator.cpp` | 10.7 | 11.3 | 1.06 | 346 | 260 | 0.75 |
| `modules/core/test/test_intrin.cpp` | 9.6 | 12.0 | 1.25 | 356 | 272 | 0.76 |
| `modules/core/test/test_arithm.cpp` | 8.4 | 9.4 | 1.11 | 332 | 243 | 0.73 |
| `modules/imgproc/src/connectedcomponents.cpp` | 7.5 | 7.5 | 1.01 | 227 | 159 | 0.70 |
| `modules/calib3d/src/dls.cpp` | 7.4 | 2.4 | 0.32 | 256 | 81 | 0.32 |

No TU is over 1.2x on RSS. 260 of the 503 TUs that take clang at least
0.5 s are over 1.2x on time (P1 below).

## Clusters

Correctness. The first build's failures, by first diagnostic. Later rows
were exposed by fixing earlier ones. No test failed or hung once its TUs
compiled, so there is no miscompile cluster.

| # | Signature | TUs | Status | Repro |
|---|---|---|---|---|
| O1 | a GNU statement expression whose result is a class lvalue (`CV_Assert` and friends) | 26 | fixed on main, `6ad563ead` (WP-799); WP-803's own fix was dropped in the merge | `ext.gnu.stmt.expr/class-lvalue-result-run.cpp` |
| O2 | NEON `vld2`/`vst2`, `vshll_n`, `vrshrn_n`, `vqmovun`, `vpadal`, `vqneg`, `vqabs` not supported | 12 | fixed, `4fe89e875` (codegen); `vld2`/`vst2` are main's `b0b8aba45` since the merge | `target.neon/ld2-st2-widen-run.cpp`, `target.neon/ir-ld2-widen-19.cpp` |
| O3 | invalid UTF-8 inside a comment rejected | 1 | fixed, `2e5a0fd97` (pp) | `lex.phases/invalid-utf8-in-comment-run.cpp` |
| O4 | NEON `vld3`/`vld4`/`vst3`/`vst4`, `vcvt{n,a,m,p}`, `vhadd`/`vrhadd`/`vhsub`, `vq{r,}shr{u,}n_n`, `vqshl_n`, `vrshr_n`, `vtbl1`/`vtbl2` not supported | 21 (exposed by O1, O2) | fixed, `0510845a0` (codegen); `vld3`/`vld4`/`vst3`/`vst4` are main's `b0b8aba45` since the merge | `target.neon/ld34-cvt-qshrn-tbl-run.cpp`, `target.neon/ir-ld34-cvt-qshrn-tbl-19.cpp` |
| O5 | `'nlanes' is a private member of 'v_uint8x16'`: `friend struct VTraits<V>` not honoured while `VTraits<V>` is instantiated inside `V`'s definition | 89 | fixed, `1b4b3e24b` (sema) | `temp.friend/specialization-instantiated-in-befriending-class-run.cpp` |
| O6 | NEON `vaddlv` not supported | 1 | fixed, `e03b318c5` (codegen) | `target.neon/ld34-cvt-qshrn-tbl-run.cpp` |
| O7 | NEON `vshl`/`vrshl`/`vqshl`/`vqrshl` (register count) not supported | 9 | fixed, `846be5b3b` (codegen) | `target.neon/ld34-cvt-qshrn-tbl-run.cpp`, `target.neon/ir-ld34-cvt-qshrn-tbl-19.cpp` |
| O8 | `extraneous 'template<>' in declaration of variable 'indices'`: an explicitly specialized constructor's `template<>` head reached its body (flann `lsh_table.h`) | 2 | fixed, `41b170805` (sema) | `temp.expl.spec/special-member-spec-body-locals-run.cpp` |
| O9 | `'SetParam' is a private member of 'WithParamInterface<...>'`: gtest's friend class template declared in a namespace that encloses the unnamed namespace of the original | ~50 (534 errors) | fixed, `9f6742595` (sema) | `temp.friend/class-template-in-unnamed-namespace-run.cpp` |
| O10 | `no member named 'operator Matx<double, 3, 3>' in 'cv::Mat'`: an explicitly named conversion function template specialization (`mat.inl.hpp`) | 1 | fixed, `a977cbed6` (sema) | `temp.mem/conversion-template-named-explicitly-run.cpp` |
| O11 | NEON `vdotq_u32` not supported, so the `NEON_DOTPROD` configure check failed and `matmul.neon_dotprod.cpp`, `norm.neon_dotprod.cpp` were silently not built | 2 (no error in the build) | fixed, `5df911b46` (codegen) | `target.neon/dotprod-run.cpp`, `target.neon/ir-dotprod-19.cpp` |

The spec sentences are listed in `spec/landed/harness/WP-803.md`.

Performance (kept apart from correctness):

| # | Signature | TUs | Status |
|---|---|---|---|
| P1 | over 1.2x clang's CPU time, mostly small TUs (0.5 s to 2 s) that use under half of clang's RSS | 260 of 503 | filed, not profiled |

The largest is `modules/calib3d/src/five-point.cpp`. It is reproduced
by the following, run from `.oss/build/opencv/kiln/modules/calib3d`
(project opencv, tag 4.12.0):

```sh
B=$KILN/.oss/build/opencv/kiln; S=$KILN/.oss/src/opencv/modules
F="-DCVAPI_EXPORTS -D_USE_MATH_DEFINES -D__OPENCV_BUILD=1 -D__STDC_CONSTANT_MACROS \
  -D__STDC_FORMAT_MACROS -D__STDC_LIMIT_MACROS -I$S/calib3d/include -I$B/modules/calib3d \
  -I$S/core/include -I$S/flann/include -I$S/imgproc/include -I$S/features2d/include \
  -isystem $B -fsigned-char -W -Wall -pthread -ffunction-sections -fdata-sections \
  -fvisibility=hidden -fvisibility-inlines-hidden -O3 -DNDEBUG -std=c++17 -fPIC \
  -c $S/calib3d/src/five-point.cpp -o /dev/null"
kiln++ $F       # 7.9 s wall, 113 MB peak RSS
clang++-19 $F   # 3.9 s wall, 203 MB peak RSS
```

The objects have the same text size (109088 and 109144 bytes). With the
same flags `calib3d/src/dls.cpp` goes the other way: 2.6 s and 81 MB
under kiln, 7.7 s and 257 MB under clang, with text of 70762 and 69190
bytes.

## What remains

- P1.
- Invalid UTF-8 inside a string literal or another token is an error
  under kiln, but a warning under clang (`-Winvalid-utf8`, which kiln
  does not have). OpenCV does not hit it.
