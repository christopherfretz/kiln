# eigen with kiln++ (WP-709)

This page covers eigen **3.4.1** (tag `3.4.1`, from GitLab). It was
built with kiln++ as `CMAKE_CXX_COMPILER`, through its own CMake build
and a subset of its ctest suite. The control is a clang++-19 build of
the same tree, and the harness is M8's (`scripts/oss/`).

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity on the subset, once WP-686's two fixes are in.**
- All 539 TUs compile under kiln. The first build had 45 failures.
- On this branch, 535 of 539 tests link and pass. The other four are
  the `operator new`/`delete` link cluster (cluster 4 below), which
  WP-686 fixed on main.
- With those fixes applied by hand, the four targets link and pass too.
  That makes 539 of 539, the same as clang.
- Eigen needed three fixes in kiln. None was a miscompile. Every test
  that built passed in both kiln builds.

## The subset

The whole suite (`buildtests`) is about 1000 TUs. Several of them take
multiple GiB under clang, and the host is shared. The project row
therefore names 132 test targets in `build_targets`, and its `test`
command filters ctest with `-R '^(<names>)(_[0-9]+)?$'`. With
`EIGEN_SPLIT_LARGE_TESTS`, a target such as `geo_quaternion` builds one
executable per part (`geo_quaternion_1` …), so the subset is 539 TUs
and 539 ctest tests.

The subset covers:
- the Core basics (`basicstuff`, `packetmath`, `vectorization_logic`,
  `mixingtypes`, `redux`, `block`, `indexed_view`, `reshape`, …);
- products, the triangular and self-adjoint views, and the
  decompositions (`cholesky`, `lu`, `qr*`, `jacobisvd`,
  `eigensolver_generic`, `schur_real`, …);
- geometry, the STL containers, and all of SparseCore (`sparse_*`,
  `simplicial_cholesky`, `sparselu`, `conjugate_gradient`);
- `half_float`, `bfloat16_float`, `stl_iterators`, and
  `initializer_list_construction`;
- from `unsupported/` and Tensor: `autodiff*`, `matrix_function`,
  `splines`, `kronecker_product`, `special_functions`, the
  thread-pool tests, and 25 `cxx11_tensor_*` tests.

Some heavy tests are left out to bound the build: `adjoint`,
`geo_alignedbox`, `polynomialsolver`, `eigensolver_selfadjoint`,
`matrix_exponential`, `linearstructure`, `array_reverse`, `umeyama`,
`dontalign`, `inplace_decomposition`, `blasutil`, `mapstaticmethods`,
`bdcsvd`, `product_extra`, and most of `product_*` and `cxx11_tensor_*`.

The 3.4.1 tag has no `constexpr` test, and its bfloat16 test is named
`bfloat16_float`. `cxx11_tensor_symmetry` is not a target at this tag:
the clang control build asked for it and failed with `make` rc 2 on
"No rule to make target". It has since been dropped from the list.

Eigen sets `CMAKE_CXX_STANDARD 11` and adds `-std=c++11` itself
(`EIGEN_TEST_CXX11`). Its tests therefore compile as C++11, whatever
`cxx_std` the harness passes.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh eigen --compiler clang
scripts/oss/oss-build.sh eigen
scripts/oss/oss-report.py eigen
```

These run the following in `.oss/build/eigen/<kiln|clang>/`:

```sh
cmake --fresh -S .oss/src/eigen -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTING=ON -DEIGEN_BUILD_DOC=OFF -DEIGEN_BUILD_BLAS=OFF -DEIGEN_BUILD_LAPACK=OFF
nice -n 10 make -k <the 132 targets of scripts/oss/projects.toml>
ctest --output-on-failure -R '^(<the 132 names>)(_[0-9]+)?$'
```

The project row is now `verified = true`.

## Results

The columns are measured as in magic-enum.md. "Wall" is the whole
`make -k` plus ctest. "Σ TU wall" sums the wall time of every compile.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (base `acd894637`) | 494 / 45 | 490 / 4 | 490/539 (49 not built) | 1028 s | — | 777 MB (`mixingtypes.cpp`) |
| clang++-19 (control) | 539 / 0 | 539 / 539 | 539/539 | 1073 s | 3306.5 s | 1408 MB (`mixingtypes.cpp`) |
| **kiln, final (this branch)** | **539 / 0** | **535 / 4** | **535/539** (4 not built) | 1226 s | 3508.5 s | **797 MB (`mixingtypes.cpp`)** |
| kiln, final + WP-686 allocation fixes (4 targets rebuilt) | 539 / 0 | 539 / 539 | 539/539 | — | — | — |

Compared with clang, kiln takes 1.06× the summed TU time and 0.57× the
peak RSS. Its slowest TU takes 41.3 s, against clang's 43.1 s. All
three builds ran while four other agents shared the host. The first
kiln build also overlapped the clang build. So the wall times are
upper bounds and are not comparable with each other.

## Clusters

The TU counts are from the first kiln build.

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `out-of-line definition of 'InnerIterator' does not match any declaration in '<error type>'` (`SparseCwiseUnaryOp.h:49`: `unary_evaluator<CwiseUnaryOp<UnaryOp,ArgType>, IteratorBased>::InnerIterator`, where the primary's third parameter defaults to `typename T::Scalar`) | 33 (every sparse test, `num_dimensions`, `kronecker_product`) | fixed in `40e9720f6` | `tests/conformance/temp.spec.partial.member/out-of-line-member-defaulted-args.cpp` |
| 2 | `RTTI for type '_ZTI13__Float32x4_t' is not supported yet` (and the other NEON vector types) | 10 (`packetmath_*`) | fixed in `21ae16216` | `tests/conformance/expr.typeid/vector-complex-type-info-run.cpp` |
| 3 | `ISO C++17 does not allow dynamic exception specifications` under `-std=c++11` (`CommaInitializer.h:107`, `EIGEN_EXCEPTION_SPEC`) | 2 (`symbolic_index_2`, `indexed_view_3`) | fixed in `a12d0d641` | `tests/conformance/except.spec/dynamic-exception-spec-cxx11.cpp` |
| 4 | link: undefined `Quaternion<float,0>::operator new(unsigned long)` / `PlainObjectBase<...>::operator new[]` / `operator delete[]` | 4 links (`dynalloc`, `geo_quaternion_{1,2,3}`) | WP-686 fixed it on main (`91621d627`, `6fb6c7666`); not re-fixed here | the WP-686 tests `tests/conformance/expr.new/class-operator-new-template-run.cpp` and `class-template-allocation-functions-instantiated-run.cpp` |

Cluster 4: a class template's member `operator new`, `new[]`, `delete`
and `delete[]` were marked used without a location, so they were never
instantiated. WP-686 found this in abseil. Its fix is not in this
branch's base, so the final build on this branch still has the four
link failures. Those four targets were rebuilt with a kiln++ that had
WP-686's three `mark_used_at` lines applied by hand (not committed),
and all four linked and passed.

## What remains

- Nothing in the subset. With main's WP-686 fixes, all 539 TUs and
  539 tests are at parity.
- Not run: the rest of the suite (about 460 more TUs, including the
  multi-GiB `product_*`, `bdcsvd` and `cxx11_tensor_*` tests).
- Not implemented: before C++17, kiln does not give the
  `-Wdeprecated-dynamic-exception-spec` warning.
- Not implemented: `throw(Ts...)` with a pack expansion is still a
  parser gap (spec/parse.md).
