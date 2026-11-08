# Boost heavy libraries with kiln++ (WP-812)

Boost **1.90.0** (tag `boost-1.90.0`, superproject with submodules) is built
through its CMake superproject with `BUILD_TESTING=ON` and static libraries
for 6 heavy libraries: Asio, Beast, Cobalt, Graph, Geometry and Math. The
libraries they depend on (context, chrono, container, random, unit_test
and others) are built too, and their TUs are counted as "other". Only the
six libraries' tests are registered. Tests run under CTest. The control is
a clang++-19 build of the same tree. Both use the M8 harness
(`scripts/oss/`, spec/landed/harness/WP-666.md, WP-787.md, WP-804.md and
WP-812.md). Host: aarch64 Linux, libstdc++ 14, C++20 (the project row),
default `-fclang-compat`. Date: 2026-11-08.

**Result:** tests at parity. The final kiln build (`d9a4f127a`) compiles and links everything the clang-19 control does (1370 / 4 TUs, 652 / 4 links; the same 4 + 4 failures, all missing optional dependencies) and passes all 602 ctest tests, like the control. 6 clusters were fixed (30 TUs and 6 tests in the first build, plus 2 interim regressions). No miscompile was found. Kiln takes 1.90x clang's CPU time (geomean) at 0.47x its peak RSS; the main perf findings are header-search filesystem cost and Math's reverse-mode autodiff compile-fail tests (Performance, below).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh boost_heavy --compiler clang
scripts/oss/oss-build.sh boost_heavy
scripts/oss/oss-report.py --perf --top 10 --update docs/status/oss.md boost_heavy
```

These run the following in `.oss/build/boost_heavy/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`. The full `-D` list is the `configure` array of the
`boost_heavy` row in `scripts/oss/projects.toml`.

```sh
cmake --fresh -S .oss/src/boost -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTING=ON -DBUILD_SHARED_LIBS=OFF \
  -DBOOST_ENABLE_MPI=OFF -DBOOST_ENABLE_PYTHON=OFF \
  "-DBOOST_INCLUDE_LIBRARIES=asio;beast;cobalt;graph;geometry;math" \
  -DCMAKE_PROJECT_Boost_INCLUDE=scripts/oss/shims/boost-jamfile-tests.cmake \
  "-DKILN_BOOST_JAMFILE_LIBRARIES=asio;graph;math" \
  "-DKILN_BOOST_JAMFILE_SKIP_asio=_select$" \
  -DKILN_BOOST_JAMFILE_FORCE_math=ON \
  "-DKILN_BOOST_JAMFILE_SUBSET_math=8|constexpr|float128|lanczos|ccmath" \
  "-DKILN_BOOST_JAMFILE_LIBS_math=test_instances//test_instances=test_instances/*.cpp;compile_test_main=compile_test/main.cpp" \
  "-DKILN_BOOST_SUBSET_TARGETS=^boost_geometry_|4|" \
  -DKILN_BOOST_JAMFILE_...   # per-library includes, defines and links
nice -n 10 make -k kiln_tests         # draws from the shared jobserver
ctest -j 4 --output-on-failure --timeout 300 \
  --exclude-from-file kiln-subset-excluded.txt -E '<19 tests, below>'
```

**Test registration.** Beast, Cobalt and Geometry add their own CMake
tests. Asio, Graph and Math have only b2 tests in a superproject build, so
the shim registers them from their Jamfiles (spec/landed/harness/WP-812.md):

- Asio: 347 tests. Its `*_select` reruns are skipped: they differ from
  the plain tests only by a Jam variable's defines.
- Graph: 123 tests.
- Math: its CMake tests are only header compile tests, so its b2 tests are
  FORCEd. Its Jamfile links `test_instances` (explicit instantiations) and
  `compile_test_main` into many tests; the shim builds both from their
  globs.

**Subset rules.** Two suites are too large to build whole in this slot.
Both subsets are deterministic (sorted names, every N-th kept):

- Geometry: 100 of its 398 test executables (every 4th, by target name).
  The other 298 go to `kiln-subset-excluded.txt` for ctest.
- Math: 173 of its 1090 b2 test names (every 8th, plus every name that
  matches `constexpr|float128|lanczos|ccmath`, the constant-evaluation
  heavy ones).

That gives 621 ctest tests: Asio 253, Beast 9, Cobalt 3, Geometry 100,
Graph 97 and Math 159.

**Optional dependencies.** The configure found OpenSSL 3.5.7 (Asio and
Beast SSL tests) and ICU 76.1. These were off or absent:

- MPI and Python (row flags): no Boost.MPI or Python bindings.
- fftw3 (Math `cardinal_trigonometric`), mpreal / GMP / MPFR headers
  (Math `mpreal_concept_check`).
- LEDA and the Stanford GraphBase (Graph `leda_graph_cc`,
  `stanford_graph_cc`).
- Boost.Timer and Boost.Filesystem are not in the row's libraries, so the
  four Graph tests that link them fail to link.

**Excluded tests.** The row's ctest `-E` leaves out 19 tests. Each of them
also fails under the clang-19 control:

- Graph `transitive_closure_test` and `metric_tsp_approx` need
  Boost.Timer; `all_planar_input_files_test` and
  `parallel_edges_loops_test` need Boost.Filesystem (link failures).
- Graph `metis_test`, `weighted_matching_test`, `cycle_ratio_tests`,
  `random_matching_test`, `graphml_test`, `mas_test` and
  `stoer_wagner_test` need command-line arguments or input files that b2
  passes and the shim does not.
- Math `cardinal_trigonometric_test_3` needs fftw3.
- Geometry `algorithms_densify`, `algorithms_area_geo`,
  `algorithms_buffer_multi_polygon`, `algorithms_buffer_linestring_geo`,
  `algorithms_get_clusters`, `formulas_direct` and
  `strategies_segment_intersection_geo` fail numeric checks on aarch64.

The control's first run had no `-E` list (that run produced it): 586 of
621 passed, and the 35 others were these 19 plus the 16 Math `*_incl_test`
executables that did not link before the shim built `compile_test_main`.
With the list, the final control passes all 602.

**System Boost is not used.** The host has Boost 1.83 in
`/usr/include/boost`. A `-M` of Geometry's `test/algorithms/area/area.cpp`
lists the same 1795 in-tree Boost and system headers under clang++-19 and
kiln++, and none under `/usr/include/boost`. The only difference is the 21
compiler resource headers.

## Results

- **TUs** are the build-phase compiles. CMake probes are excluded.
- **Wall** is the whole `oss-build.sh` run (configure, build and test),
  taken from `report.json`.
- **Σ TU wall** and **Σ TU CPU** sum the wrapper's per-TU compile times.
  Links are excluded.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once, from an empty build directory, on a host shared with
other harness builds.

| Build | TUs ok / failed | Links ok / failed | Tests (ctest) | Wall | Σ TU wall | Σ TU CPU | Peak TU RSS |
|---|---|---|---|---|---|---|---|
| clang++-19, first | 1369 / 4 | 636 / 20 | 586/621 | 7756 s † | 8003 s | 4149 s | 1086 MB (`beast/test/beast/websocket/read3.cpp`) |
| clang++-19, final (control) | 1370 / 4 | 652 / 4 | 602/602 (19 excluded) | 4756 s | 5729 s | 3991 s | 1087 MB (`read3.cpp`) |
| kiln, first (`be67586de`) | 1332 / 34 | 629 / 20 | 580/621 | 13035 s | 15825 s | 6028 s | 862 MB (`read3.cpp`) |
| kiln, final (`d9a4f127a`) | 1370 / 4 | 652 / 4 | 602/602 (19 excluded) | 6515 s ‡ | 23477 s ‡ | 6793 s | 865 MB (`read3.cpp`) | |

† The first control's wall includes the clone. The other runs reused the
source.

‡ The final kiln build ran while the shared host was at load ~25 on 24
cores. Its wall-clock columns are inflated (Σ TU wall is 3.5x Σ TU CPU,
against 1.4x for the final control), so compare CPU.

The control's 4 failed TUs are `mpreal_concept_check`,
`cardinal_trigonometric_test`, `leda_graph_cc` and `stanford_graph_cc`
(missing optional dependencies); its 4 failed links are the Timer and
Filesystem tests. In the first builds, the 16 other failed links were the
Math `*_incl_test` executables (no `compile_test_main` yet).

### By library

First → final. Tests count ctest results (the first builds ran all 621;
the final builds ran 602).

Kiln, first → final, with the final clang-19 control for reference.
TUs and links are ok / failed; tests are passed / run.

| Library | TUs, kiln | Links, kiln | Tests, kiln | TUs / links / tests, control |
|---|---|---|---|---|
| asio | 345 / 2 → 347 / 0 | 339 / 0 → 341 / 0 | 251/253 → 253/253 | 347 / 0, 341 / 0, 253/253 |
| beast | 153 / 0 → 153 / 0 | 9 / 0 → 9 / 0 | 9/9 → 9/9 | 153 / 0, 9 / 0, 9/9 |
| cobalt | 30 / 26 → 63 / 0 | 0 / 0 → 4 / 0 | 0/3 → 3/3 | 63 / 0, 4 / 0, 3/3 |
| graph | 123 / 2 → 123 / 2 | 93 / 4 → 93 / 4 | 86/97 → 86/86 | 123 / 2, 93 / 4, 86/86 |
| geometry | 100 / 0 → 100 / 0 | 100 / 0 → 100 / 0 | 93/100 → 93/93 | 100 / 0, 100 / 0, 93/93 |
| math | 424 / 4 → 427 / 2 | 88 / 16 → 105 / 0 | 141/159 → 158/158 | 427 / 2, 105 / 0, 158/158 |
| other (dependencies) | 157 / 0 → 157 / 0 | — | — | 157 / 0 |

Tests that failed under kiln and passed under the control: 6 in the first
build (Asio `experimental_coro_exception` and `experimental_coro_simple_test`,
the 3 Cobalt tests, Math `sinhc_pi_test`; all "Not Run" because their TUs
failed), none in the final build. Math gained a TU and 17 links between the
first and final builds because the shim now builds `compile_test_main`.

### Performance

CPU time and peak RSS of the compiler plus the backend it waited for, over
the 1320 build-phase TUs that compiled with both (`oss-report.py --perf`):

- First build (`be67586de`): geomean kiln/clang time **1.64x**, RSS
  **0.47x**; total CPU 5700 s vs 3885 s.
- Final build (`d9a4f127a`): geomean time **1.90x**, RSS **0.47x**; total
  CPU 6519 s vs 3881 s; peak TU RSS 865 MB vs 1087 MB.
- 953 of the 1025 TUs that take clang at least 0.5 s are over 1.2x on time.
  None is over 1.2x on RSS: kiln's peak RSS is below clang's on all 1025.

Perf clusters (not fixed here; each re-runs from a `boost_heavy` build tree
made by `scripts/oss/oss-build.sh boost_heavy [--compiler clang] --no-test`
at tag `boost-1.90.0`; the exact argv of every TU is its record in that
tree's `compile.jsonl`, and `make VERBOSE=1 <target>` in the tree re-runs
it with kiln++ or clang++-19 and the row's flags, `-std=gnu++20 -O3
-DNDEBUG`, 157 `-I` directories):

1. **Header search costs ~4x clang's filesystem time.** Graph TUs are the
   worst ratio (3.5x to 5x). `libs/graph/test/lvalue_pmap.cpp` (target
   `graph-lvalue_pmap`, cwd the build root), re-run by hand at load ~23:
   kiln++ `-c` 66.7 s wall, 2.8 s user, 5.1 s sys, 155 MB; clang++-19
   `-c` 17.9 s wall, 1.6 s user, 1.1 s sys, 262 MB. With `-E` instead of
   `-c` (preprocessing only): kiln++ 67.0 s wall, 1.8 s user, 5.6 s sys;
   clang++-19 17.9 s wall, 0.9 s user, 1.4 s sys. So the whole kiln
   slowdown on this TU is in the preprocessor, mostly in system time and
   waiting on the bind-mounted source tree: with 157 include directories
   and ~1800 headers, kiln makes several times the file-system calls clang
   makes (inferred from sys time; strace is not available here). In the
   first build the same TU took 5.3 s CPU under kiln and 2.3 s under clang.
2. **Geometry heavy TUs: 1.3x to 3.0x CPU at 0.5x RSS** (Heaviest TUs,
   below). E.g. `libs/geometry/index/test/rtree/generated/b2d/rtree_qua_que_b2d.cpp`
   (target `boost_geometry_index_rtree_qua_que_b2d`, cwd
   `libs/geometry/index/test/rtree/generated/b2d`): kiln 67.2 s wall /
   36.5 s CPU / 426 MB, clang 22.7 s wall / 820 MB.
3. **Math reverse-mode autodiff compile-fail tests: ~3x CPU, and 3 exceed
   the 3 GB address-space limit after their first error.** 53 Math
   `compile-fail-math-test_reverse_mode_autodiff_*` tests compile at ctest
   time. Kiln rejects all 53 with the same first diagnostic as clang, so
   all pass, but they take 689 s CPU (3407 s wall at load ~25) against
   clang's 235 s (370 s); ctest's total went from 624 s (control) to
   3590 s. Three keep instantiating after the first error until
   `ulimit -v 3000000` stops them (kind `oom`, in both kiln builds):
   - `test_reverse_mode_autodiff_rising_factorial_compile_test_ET_ON_multiprecision_float_types.cpp`:
     kiln 91.0 s wall / 38.9 s CPU / 1831 MB at the limit, clang 7.4 s /
     4.3 s / 442 MB.
   - `test_reverse_mode_autodiff_ibeta_inv_compile_test_ET_ON_multiprecision_float_types.cpp`:
     kiln 64.5 s / 23.3 s / 1745 MB, clang 7.0 s / 5.0 s / 591 MB.
   - `test_reverse_mode_autodiff_ibetac_inv_compile_test_ET_ON_multiprecision_float_types.cpp`:
     kiln 62.0 s / 23.4 s / 1751 MB, clang 7.7 s / 5.4 s / 583 MB.

   All three live under `libs/math/test/test_reverse_mode_autodiff_special_functions_compile/`;
   re-run with `ctest -R '<file name without .cpp>' -V` in the build root.
   The first error is `factorials.hpp:149:14: no viable conversion from
   returned value of type 'int' to function return type 'rvar<…>'` under
   both compilers; clang is done 4 to 5 s of CPU later, kiln keeps
   instantiating.

### Heaviest TUs

The 10 build-phase TUs with the highest peak RSS under either compiler
(final builds; compiler plus backend):

| TU (`libs/…`) | clang-19 MB | kiln MB | RSS x | clang-19 wall s | kiln wall s ‡ | wall x |
|---|---|---|---|---|---|---|
| `beast/test/beast/websocket/read3.cpp` | 1087 | 865 | 0.80 | 40.9 | 53.8 | 1.32 |
| `geometry/test/cs_undefined/index.cpp` | 1006 | 513 | 0.51 | 33.6 | 67.2 | 2.00 |
| `beast/test/beast/websocket/write.cpp` | 999 | 798 | 0.80 | 39.5 | 54.4 | 1.38 |
| `geometry/test/algorithms/merge_elements.cpp` | 981 | 511 | 0.52 | 32.3 | 63.0 | 1.95 |
| `beast/test/beast/websocket/close.cpp` | 966 | 764 | 0.79 | 39.7 | 51.8 | 1.30 |
| `geometry/test/algorithms/disjoint/disjoint_sph.cpp` | 932 | 528 | 0.57 | 31.7 | 66.2 | 2.09 |
| `beast/test/beast/websocket/read2.cpp` | 908 | 712 | 0.78 | 36.2 | 46.4 | 1.28 |
| `geometry/test/geometries/custom_non_copiable/custom_multi_polygon.cpp` | 830 | 437 | 0.53 | 29.4 | 73.9 | 2.51 |
| `geometry/test/algorithms/envelope_expand/envelope.cpp` | 823 | 419 | 0.51 | 21.7 | 43.7 | 2.01 |
| `geometry/index/test/rtree/generated/b2d/rtree_qua_que_b2d.cpp` | 820 | 426 | 0.52 | 22.7 | 67.2 | 2.96 |

The top 10 by clang CPU time, and the 10 worst time ratios, are in the
`boost_heavy` section of docs/status/oss.md (`oss-report.py --perf --top
10`). The Math compile-fail TUs (perf cluster 3) reach 1.8 GB under kiln
but are test-phase compiles, not in this table.

## Clusters

The clusters come from the first build, counted where kiln fails and
clang passes: 30 TUs and 6 tests (all 6 "Not Run" because of the TUs).
Rows 5 and 6 appeared in an interim build: both were regressions from
row 1's fix, and row 6 stopped Graph's library from building, so `make -k`
skipped all of Geometry and most of Graph.

| # | Signature (first diagnostic) | Library | TUs / tests | Status | Repro |
|---|---|---|---|---|---|
| 1 | `no viable conversion` (P0522: `U<T>` did not deduce from Boost.Multiprecision's `number<B, ET>`, whose second argument is defaulted; Math `sinhc_pi`) | math | 1 TU, 1 test | fixed, `c52196838` | `tests/conformance/temp.arg.template/p0522-deduce-defaulted-args-run.cpp`, `p0522-partial-order-unique-ttp-clang19.cpp`, `p0522-partial-order-unique-ttp-clang22.cpp` |
| 2 | `expected expression` (`(co_yield x)`, a parenthesized yield-expression; Asio `experimental/coro`) | asio | 2 TUs, 2 tests | fixed, `9c2634218` | `tests/conformance/expr.yield/parenthesized-run.cpp` |
| 3 | `target of using declaration conflicts with declaration already in scope` (a dependent member using-declarator against a later one; Cobalt `detail/task.hpp:301`), and `no suitable member 'operator delete'` (a promise's `operator delete(void*, const std::size_t)`; Cobalt `detail/wrapper.hpp:148`) | cobalt | 26 TUs, 3 tests | fixed, `25ac60f21` | `tests/conformance/namespace.udecl/dependent-member-using-overloads-run.cpp`, `tests/conformance/dcl.fct.def.coroutine/promise-new-delete-const-size-run.cpp` |
| 4 | `expression DependentScopeDeclRefExpr is not supported yet` (an explicit instantiation definition instantiated a member class template's still-dependent partial specialization; Math `hyperexponential.hpp:285`) | math | 1 TU | fixed, `6063a6b54` | `tests/conformance/temp.explicit/member-partial-spec-dependent-run.cpp`, `tests/codegen/explicit_inst_member_partial_spec.cpp` |
| 5 | `call to 'sinhc_pi' is ambiguous` (row 1's fix also checked trailing defaults in function template partial ordering, which clang 19 and 22 do not) | math | 1 TU (interim) | fixed, `d97c0971f` | `tests/conformance/temp.arg.template/p0522-partial-order-defaulted-args-run.cpp` |
| 6 | `ambiguous partial specializations of 'lambda<protect<arg<1>>>'` (a partial specialization's defaulted non-type argument is an expression, compared with the default's value; Boost.MPL via Graph `src/graphml.cpp`) | graph (and all of geometry) | 1 TU (interim; 223 not attempted) | fixed, `78be59b8e` | `tests/conformance/temp.arg.template/p0522-partial-spec-order-defaulted-value.cpp` |

The final build has no correctness cluster left: its 4 failed TUs and 4
failed links are the control's (missing fftw3, mpreal, LEDA and the
Stanford GraphBase; Boost.Timer and Boost.Filesystem not in the row). No
xfail repro was needed.

## What remains

- The three perf clusters above, as follow-ups: header-search filesystem
  calls (kiln_pp include lookup; Graph TUs 3.5x to 5x), Geometry's heavy
  TUs (1.3x to 3x CPU), and bounding instantiation after errors (Math
  reverse-mode autodiff compile-fail TUs, 1.8 GB and the 3 GB limit).
- The full suites: Geometry has 298 more test executables and Math 917 more
  b2 tests outside the subsets.
- The 7 Graph tests that need b2's command-line arguments or input files,
  and the 4 that need Boost.Timer or Boost.Filesystem (control failures
  too); adding `timer;filesystem` to the row and teaching the shim b2's
  `<testing.arg>` would run them.
- The 7 Geometry tests that fail numeric checks on aarch64 under both
  compilers.

## Disk

At handback: `.oss/` 2.1 GB (the 1.90 checkout 882 MB, the kiln build
tree 1.2 GB; the 1.2 GB clang-control tree was deleted after its numbers
were recorded), and the worktree's cargo target dir
(`.build/target-wp-812`) 13 GB. `df -h` showed 138 GB free.
