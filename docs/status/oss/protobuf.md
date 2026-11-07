# protobuf with kiln++ (WP-716)

protobuf **v33.1** (tag `v33.1`, commit `a8d85ff`), built with kiln++ as
`CMAKE_CXX_COMPILER` through protobuf's own CMake build with its tests
enabled (`protobuf_BUILD_TESTS=ON`), and run with CTest. Abseil
20250512.1 and GoogleTest 1.15.2 are downloaded at configure time
(`protobuf_FORCE_FETCH_DEPENDENCIES=ON`) and compiled by the compiler
under test. The build includes `protoc`, which generates the test
protos, so kiln-built `protoc` output feeds the rest of the build. The
control is a clang++-19 build of the same tree. Both use the M8 harness
(`scripts/oss/`, spec/landed/harness/WP-666.md). Host: aarch64 Linux,
libstdc++ 14, C++17 (the project row), default `-fclang-compat`.
Date: 2026-11-07.

**Result: parity.** Every TU compiles, every target links and all 4
CTest tests pass: `lite-test`, `full-test` (the main gtest binary),
`lazily-build-dependencies-test` and `upb-test`. kiln needed 6 fixes in
this WP: 4 for rejected valid code, 1 for wrong code (a generic lambda
that used an uncaptured `this`) and 1 for a link failure (the GNU
`section` attribute was ignored).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh protobuf --compiler clang
scripts/oss/oss-build.sh protobuf
scripts/oss/oss-report.py protobuf
```

These run the following in `.oss/build/protobuf/<kiln|clang>/`. The
wrapper `scripts/oss/kiln-cxx` applies `ulimit -v 3000000` and
`timeout 600`.

```sh
cmake --fresh -S .oss/src/protobuf -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -Dprotobuf_BUILD_TESTS=ON -Dprotobuf_FORCE_FETCH_DEPENDENCIES=ON \
  -Dprotobuf_BUILD_CONFORMANCE=OFF
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure             # 4 tests
```

The project row in `scripts/oss/projects.toml` needed fixing. v33 has no
submodules, so `submodules = true` and `protobuf_ABSL_PROVIDER=module`
(which v33 ignores) were replaced by `submodules = false` and
`protobuf_FORCE_FETCH_DEPENDENCIES=ON`. The row is now `verified = true`.

## Results

- **TUs** are the build-phase compiles (653, CMake probes excluded).
- **Wall** is the whole `oss-build.sh` run (configure, build and test),
  taken from `report.json`.
- **Σ TU wall** sums the wrapper's per-TU compile wall times; links are
  excluded.
- **Peak TU RSS** is the wrapper's `ru_maxrss` for the compiler plus the
  backend it waited for.

Each build ran once.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (base `658857903`) | 253 / 9 (391 not attempted) | 0 / 0 | 0/4 | 51 s | — | 224 MB (`descriptor.cc`) |
| kiln, 4 fixes | 441 / 0 (212 not attempted) | 0 / 4 | 0/4 | 108 s | — | 231 MB (`descriptor.cc`) |
| kiln, 5 fixes | 652 / 1 | 9 / 0 | 3/4 (`full-test` not built) | 316 s | — | 571 MB |
| **kiln, final** (`30978fae0`) | **653 / 0** | **10 / 0** | **4/4** | 217 s † | 4394.3 s | **572 MB** (`descriptor_unittest.cc`) |
| clang++-19 (control) | 653 / 0 | 10 / 0 | 4/4 | 367 s † | 1661.1 s | 786 MB (`map_test.cc`) |

The 9 rejected TUs in the first build stopped `make -k` from building
every target that depends on `libprotobuf`. On the base before WP-686's
fixes landed, the first build stopped earlier still, at 74 TUs, on
Abseil's `charconv_bigint.cc` (WP-686 cluster 1).

† The two final builds ran at different times on a shared host, and the
control ran while other harness builds were starting after a container
restart, so wall times are not comparable. Σ TU wall is the cost
comparison: kiln takes **2.65×** clang's summed compile time. The slowest
TU takes about the same time under both compilers (`map_test.cc`: 51 s
with kiln, 46 s with clang), so the extra time comes from the many small
and mid-sized TUs (generated `.pb.cc` files and Abseil). kiln's peak RSS
is **0.73×** clang's.

## Clusters

Clusters 1–4 are from the first build. Cluster 5 appeared at the first
link, and cluster 6 once every library linked.

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `constexpr variable 'kUnionMember<int>' must be initialized by a constant expression` (`&C::m` for a member of an anonymous union, `extension_set.h`) | 4 | fixed, `8ce079f28` | `tests/conformance/class.union.anon/member-pointer-to-anonymous-member-run.cpp` |
| 2 | `call to non-static member function without an object argument` (a dependent member call in a generic lambda, `map.cc`); under `[&]`, wrong code: `this` not captured | 2 | fixed, `86f0ff973` | `tests/conformance/expr.prim.this/generic-lambda-implicit-this-member-template-run.cpp` |
| 3 | `qualified member initializers is not supported yet` (`E::E(int v) : E::Entry(v)`, `struct.pb.cc`) | 1 | fixed, `10242b86d` | `tests/conformance/class.base.init/qualified-inherited-injected-class-name-run.cpp` |
| 4 | `no member named '_impl_' in 'Empty'` (`__builtin_offsetof` into a base, generated `.pb.cc`) | 2 | fixed, `81df2d530` | `tests/conformance/ext.gnu.offsetof/base-member-run.cpp` |
| 5 | link: undefined `__start_pb_defaults` / `__stop_pb_defaults` (`__attribute__((section))` ignored, `generated_message_util.cc`) | every link | fixed, `c34ff183d` | `tests/conformance/ext.gnu.section/section-run.cpp`, `tests/codegen/section_attr.cpp` |
| 6 | `'AddAllocated' is a protected member of 'RepeatedPtrFieldBase'` (a protected member template of a private base, named from a friend of the derived class, `repeated_ptr_field.h`) | 1 | fixed, `30978fae0` | `tests/conformance/class.protected/member-template-friend-of-derived-run.cpp` |

No cluster was a bug outside kiln. Every TU that clang accepts, kiln
accepts as well.

## What remains

- Summed compile time is 2.65× clang's and needs profiling (M8 perf
  work).
- Gaps found while reducing, not on protobuf's path:
  - Conflicting `section` placements are not diagnosed (clang's
    `err_section_conflict`) (`crates/kiln_sema/src/decl.rs:9971`, `weakref_alias_attribute`).
  - A generic lambda with an empty capture calling an instance member:
    kiln's second diagnostic is `does not provide a call operator`, where
    clang says `no matching function for call to object`
    (`crates/kiln_sema/src/overload/oper.rs:593`).
- Harness: `oss-build.sh`'s tag check uses `git describe --tags
  --exact-match`. When two tags name the checkout (`v33.1` and
  `v4.33.1-objectivec`), it picks the other one and wrongly warns that
  the checkout is not at the pinned tag (`scripts/oss/oss-build.sh:67`).
- Variants not run: Debug, C++20/26, libc++, `BUILD_SHARED_LIBS=ON`,
  `protobuf_BUILD_CONFORMANCE=ON`, and an Abseil from a kiln-built
  install tree.
