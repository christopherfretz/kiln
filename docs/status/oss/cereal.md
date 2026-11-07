# cereal with kiln++ (WP-704)

cereal **v1.3.2** (tag `v1.3.2`), built with kiln++ as
`CMAKE_CXX_COMPILER` through cereal's own CMake build and its doctest
unit tests (bundled doctest and rapidjson), against a clang++-19 control
build of the same tree, with the M8 harness (`scripts/oss/`,
spec/landed/harness/WP-666.md). Host: aarch64 Linux, libstdc++ 14,
default `-fclang-compat`, C++17. Date: 2026-11-07.

**Result: parity.** Every TU compiles, every test executable links, and
all 40 ctest tests pass under both compilers. kiln needed three fixes
(one parser/sema, one AST dependence, one deduction); the third changes
generated code (a reference now binds directly to a conversion
template's lvalue result instead of a temporary copy).

## Commands

```sh
source scripts/env.sh                          # shared jobserver for make
cargo build --profile harness -p kiln_driver   # $CARGO_TARGET_DIR/harness/kiln++
scripts/oss/oss-build.sh cereal --compiler clang
scripts/oss/oss-build.sh cereal
scripts/oss/oss-report.py cereal
```

which runs, in `.oss/build/cereal/<kiln|clang>/` (wrapper
`scripts/oss/kiln-cxx`: `ulimit -v 3000000`, `timeout 600`):

```sh
cmake --fresh -S .oss/src/cereal -B . -G "Unix Makefiles" \
  -DCMAKE_CXX_COMPILER=scripts/oss/kiln-cxx -DCMAKE_C_COMPILER=clang-19 \
  -DCMAKE_CXX_STANDARD=17 -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_TESTS=ON -DSKIP_PERFORMANCE_COMPARISON=ON \
  -DSKIP_PORTABILITY_TEST=ON -DBUILD_SANDBOX=OFF -DBUILD_DOC=OFF \
  -DWITH_WERROR=OFF -DCEREAL_INSTALL=OFF \
  "-DCMAKE_CXX_FLAGS=-Wno-error=missing-template-arg-list-after-template-kw -Wno-error=c++11-narrowing-const-reference"
nice -n 10 make -k                    # draws from the shared jobserver
ctest --output-on-failure
```

The project row in `scripts/oss/projects.toml` needed two changes, both
for the clang++-19 control build (it is now `verified = true`):

- `-DSKIP_PORTABILITY_TEST=ON`: cereal makes that option only on Apple;
  elsewhere it adds a `-m32` copy of every unit test and a 32-bit
  portability test, which has no multilib on aarch64.
- The two `-Wno-error=` flags: clang 19 makes
  `-Wmissing-template-arg-list-after-template-kw` an error by default
  (cereal `details/tuple.hpp` `template get<…>` spellings), and
  `-Wc++11-narrowing-const-reference` fires on aarch64, where `char` is
  unsigned (`std::map<char, …>{{-1, …}}` style initializers in
  `unittests/multimap.hpp`, `unordered_map.hpp` and siblings).

Boost is absent, so the Boost-dependent tests are not configured; the
C++17 tests (`unittests/cpp17`: `optional`, `variant`) are.

## Results

TUs are the harness's build-phase compiles (CMake probes excluded); each
test is its own executable, so links = TUs. Wall is the whole
`oss-build.sh` run (configure, build, test) from `report.json`; Σ TU
wall sums the wrapper's per-TU wall times; peak TU RSS is the wrapper's
`ru_maxrss` (the compiler and the backend it waited for). One run each.

| Build | TUs ok / failed | Links | Tests (ctest) | Wall | Σ TU wall | Peak TU RSS |
|---|---|---|---|---|---|---|
| kiln, first (main `cd5fd187e`) | 0 / 39 | 0 / 0 | 1/40 | 28 s | — | 234 MB (`load_construct.cpp`) |
| kiln, after fix 1 | 34 / 5 | 34 / 0 | 35/40 | 26 s | — | 246 MB |
| clang++-19 (control) | 39 / 0 | 39 / 0 | 40/40 | 28 s | 377.5 s | 530 MB (`polymorphic.cpp`) |
| **kiln, final** (WP-704 fixes) | **39 / 0** | **39 / 0** | **40/40** | 31 s | 473.7 s | **257 MB** (`polymorphic.cpp`) |

The one test passing in the first kiln build is
`test_cmake_config_module` (a CMake script, no compile). kiln takes
1.25× clang's summed TU time and 0.48× its peak RSS (LLVM codegen runs
out of process in llc-19). The largest TU, `polymorphic.cpp`, takes
22.5 s under kiln and 20.7 s under clang.

Warnings: clang prints 113 (78 `-Wmissing-template-arg-list-after-template-kw`,
30 `-Wc++11-narrowing-const-reference`, 5 `-Wdeprecated-copy`); kiln
prints none. kiln does not implement those three warnings (see What
remains).

## Clusters

| # | Signature (first diagnostic) | TUs | Status | Repro |
|---|---|---|---|---|
| 1 | `use of class template 'GenericValue' requires template arguments…` at `cereal/archives/json.hpp:426` `typedef …::Document::GenericValue GenericValue;` (the injected-class-name of `GenericDocument`'s base `GenericValue<…>`) | 39 (every TU includes the JSON archive) | fixed, commit `7a5acad18` (sema, parse: a base specialization's injected-class-name not followed by `<` is a type) | `tests/conformance/temp.local/base-specialization-injected-name.cpp` |
| 2 | `no viable conversion from 'PolyDerived *' to 'std::unique_ptr<PolyBase>'` at `unittests/polymorphic.hpp:275` (`std::unique_ptr<PolyBase> o_unique( new PolyDerived( rngI(), … ) );` in a function template, `rngI` a generic lambda) | 2 (`polymorphic.cpp`, `user_data_adapters.cpp`) | fixed, commit `9a38643d1` (ast: a type-dependent new-initializer makes a new-expression value-dependent, not type-dependent) | `tests/conformance/temp.dep.expr/new-expression-dependent-initializer.cpp` |
| 3 | `static assertion failed … cereal detected different or invalid types in corresponding member load_minimal and save_minimal functions` at `details/traits.hpp:772` (and the non-member and versioned variants at :778, :871, :875) | 3 (`structs_minimal.cpp`, `structs_specialized.cpp`, `bitset.cpp`) | fixed, commit `777919151` (sema: a conversion template returning a reference keeps the reference target's cv in deduction) | `tests/conformance/temp.deduct.conv/reference-target-keeps-cv.cpp` |

Cluster 3's root cause: cereal's `NoConvertConstRef<U>` has a deleted
`template <class D, …> operator D()` and `template <class D, …> operator
D const &()`. Binding `U const &` to it, kiln deduced the second template
with A = `U` (cv dropped) against P = `D const`, failed, and fell back to
a temporary initialized by either conversion: ambiguous, so the
`load_minimal` signature check failed. kiln now keeps the referee's cv
when the result is a reference (clang's handling of [temp.deduct.conv]/4).

No miscompiles: every unit test (archives, polymorphism, versioning,
minimal load/save, the C++17 `optional`/`variant` tests) passes under
the kiln build.

## What remains

- Warnings kiln does not emit, all on cereal's path:
  `-Wmissing-template-arg-list-after-template-kw` (clang 19:
  default-error), `-Wc++11-narrowing-const-reference` (clang 19:
  default-error) and `-Wdeprecated-copy` (under `-Wextra`). A kiln build
  without the `-Wno-error=` flags therefore accepts code clang rejects.
- Variants not run: Debug, C++11/14 (cereal's default standard), C++26,
  `THREAD_SAFE=ON`, the Boost comparison and sandbox targets, the 32-bit
  portability test (no `-m32` on this host).
