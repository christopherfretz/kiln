# EnTT with kiln++ (WP-690)

This page covers EnTT **v3.15.0** (tag `v3.15.0`). It was built with
kiln++ as `CMAKE_CXX_COMPILER`, through its own CMake build and its
googletest suite. The control is a clang++-19 build of the same tree,
and the harness is M8's (`scripts/oss/`).

EnTT fetches googletest with FetchContent at `GIT_TAG main`, which is
not pinned. Both builds here got commit `6450691`. The same compiler
builds it.

Host: aarch64 Linux, libstdc++ 14, default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.**
- All 73 TUs compile and all 68 test executables link.
- All 68 ctest tests pass under both compilers.
- EnTT needed ten fixes in kiln, in the eight clusters below. None was
  a miscompile.

## Commands

```sh
source scripts/env.sh
cargo build --profile harness -p kiln_driver
scripts/oss/oss-build.sh entt --compiler clang
scripts/oss/oss-build.sh entt
scripts/oss/oss-report.py entt
```

These run the following in `.oss/build/entt/<kiln|clang>/`:

```sh
cmake --fresh -S .oss/src/entt -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=20 -DCMAKE_BUILD_TYPE=Release \
  -DENTT_BUILD_TESTING=ON -DENTT_BUILD_BENCHMARK=OFF -DENTT_BUILD_EXAMPLE=OFF
nice -n 10 make -k
ctest --output-on-failure          # 68 tests, one executable each
```

The row in `scripts/oss/projects.toml` worked unchanged. It is now
`verified = true`, with a note that googletest is unpinned.

## Results

The columns are measured the same way as in magic-enum.md.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (base `2fac46d71`) | 4 / 1 | 0 / 0 | 0/68 (not built) | 14 s | 10.9 s (5 TUs) | 146 MB (`gtest-all.cc`) |
| clang++-19 (control) | 73 / 0 | 68 / 68 | 68/68 | 31 s | 371.1 s | 468 MB (`registry.cpp`) |
| **kiln, final** | **73 / 0** | **68 / 68** | **68/68** | 58 s | 453.1 s | **328 MB (`storage.cpp`)** |

In the first build, `odr.cpp` failed (cluster 1). It belongs to a
library that the test executables need, so `make -k` built nothing past
googletest.

Compared with clang, kiln takes 1.22× the summed TU time and 0.70× the
peak RSS. Its slowest TU is `storage.cpp`: 19.6 s, against clang's
17.8 s. The final kiln build ran while other agents shared the host;
an earlier kiln build with 71 TUs took 35 s of wall time. So the
58 s wall time is an upper bound.

## Clusters

The TU counts are from the build in which each cluster was the first
diagnostic of those TUs.

| # | Signature (first diagnostic) | TUs | Fix | Repro |
|---|---|---|---|---|
| 1 | `no matching constructor for initialization` (`locator.hpp:134`, `reset(const node_type &other = {})`) | 1 (`odr.cpp`), which blocked the rest | `342451413` | `tests/conformance/dcl.fct.default/nested-class-of-template.cpp` |
| 2 | static data member `storage_policy{traits::in_place_delete}` rejected (copy-list init of an enum from `bool`) | 14 | `3be0211b9` | `tests/conformance/class.static.data/template-direct-list-init.cpp` |
| 3 | `not yet implemented: non-type template argument of this type` (`&array::global`), and `&C::f<T>` with an `auto` return type | 2 | `09835917c` | `tests/conformance/temp.arg.nontype/pointer-to-array-argument.cpp`, `tests/conformance/dcl.spec.auto/member-template-address-deduced.cpp` |
| 4 | `list-initialization of a reference type is not supported yet` (`nth_argument_t<...>{}`) | 1 | `6bab087ac` | `tests/conformance/expr.type.conv/reference-list-cast.cpp` |
| 5 | `missing default argument on parameter ...` (`sort(Compare = Compare{}, Args &&...)`) | 3 | `782ff2864` | `tests/conformance/dcl.fct.default/pack-after-default.cpp` |
| 6 | link: undefined `group_handler<...>::remove_if` / `push_on_destroy` from `delegate::wrap` | 4 (group, helper, organizer, registry) | `be32e403d`, `2cf97700b` | `tests/conformance/temp.arg.nontype/kept-argument-odr-use.cpp`, `tests/conformance/expr.prim.lambda/member-pointer-no-this-capture.cpp` |
| 7 | `no matching member function for call to 'on_destroy'` (`&remove<decltype(pool)>` beside `<cstdio>`'s `remove`) | 1 (`reactive_mixin.cpp`) | `875a9363c` | `tests/conformance/over.over/template-id-with-non-template.cpp` |
| 8 | `invalid operands to binary expression ('const auto' and ...)` (`this->push_instance(...)`, a `static auto` member) | 1 (`storage_no_instance.cpp`) | `b46fa7c7e` | `tests/conformance/dcl.spec.auto/static-member-through-dependent-base.cpp` |

No miscompiles were found. Every test that linked passed.

spec/landed/sema/WP-690.md describes each fix and names its spec
sentence.

## What remains

- **googletest is unpinned.** EnTT fetches `GIT_TAG main`, so a later
  build can get a different googletest. This page used `6450691`.
- **Variants not run:** Debug, C++17 (`CMAKE_CXX_STANDARD`), and the
  benchmark and example targets (`ENTT_BUILD_BENCHMARK`,
  `ENTT_BUILD_EXAMPLE`).
- **Diagnostic differences seen while writing the error tests, not
  fixed.** None of these blocks EnTT.
  - kiln warns `-Wbraced-scalar-init` on a braced initializer of a
    dependent `T` that becomes a scalar; clang does not
    (`crates/kiln_sema/src/init.rs`, `init_list_entity`).
  - kiln has no "passing argument to parameter here" note after a
    failed default-argument conversion.
  - `decltype(&g<int>)`, where `g` is an `auto` function template with
    no definition, gives a different error than clang.
  - A `DependentScopeDeclRefExpr` error is at a different column than
    clang's: 4:30 where clang gives 4:27.
