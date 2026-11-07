# range-v3 built with kiln

WP-669 (robustness wave 1). range-v3 built through its own CMake build and
test suite with kiln, and with clang++-19 as the control.

- **Project**: range-v3 tag `0.12.0`, commit `a81477931a8aa2ad025c6bda0609f38e09e4d7ec`.
- **kiln**: `kiln version 0.0.1 (C++26 frontend, LLVM IR backend)`, harness
  profile build (`cargo build --profile harness -p kiln_driver`), branch
  `wp/669` at the commits below.
- **Control**: `Debian clang version 19.1.7 (3+b1)`.
- **Tools**: cmake 3.31.6, GNU Make 4.4.1, aarch64 Linux, libstdc++ headers.

## Result

kiln reaches parity with clang++-19. Both builds compile 239 of 240 TUs.
The one TU that fails, `test/view/tokenize.cpp`, fails under both compilers:
a `static_assert` in the test expects the libstdc++ `std::regex_token_iterator`
to model a forward range, and it does not. That is a libstdc++ and test
issue, not a kiln bug. Both builds pass 238 of 239 ctest tests, and the only
test that does not run is `tokenize`, which did not build. No test fails at run time, so
there are no miscompiles.

## Commands

The compiler is called through a logging wrapper, `kxx`. It runs `$KXX` under
`ulimit -v 3000000`, `timeout 600` and `/usr/bin/time -f '%e %M'`, and logs one
line per TU: source, exit code, wall, max RSS and first error. The same
wrapper is used for both builds, with `KXX` set to the harness `kiln++` or to
`clang++-19`, and with `KILN_RESOURCE_DIR=<kiln checkout>/resource`.

```sh
cmake -G "Unix Makefiles" -DCMAKE_CXX_COMPILER=<dir>/kxx -DCMAKE_C_COMPILER=clang-19 \
  -DRANGES_CXX_STD=20 -DRANGE_V3_TESTS=ON -DRANGE_V3_EXAMPLES=OFF -DRANGE_V3_PERF=OFF \
  -DRANGE_V3_DOCS=OFF -DCMAKE_BUILD_TYPE=Release \
  -DRANGES_ENABLE_WERROR=OFF "-DCMAKE_CXX_FLAGS=-Wno-deprecated-ofast" <src>
nice -n 10 make -k
nice -n 10 ctest --output-on-failure --timeout 120
```

Two settings were added for both builds:

- `-DRANGES_ENABLE_WERROR=OFF`, because range-v3 builds its tests with
  `-Weverything -Werror` under clang.
- `-Wno-deprecated-ofast`, because clang 19 deprecates the `-Ofast` that
  range-v3 passes in Release.

`make` takes its parallelism from the shared jobserver in `MAKEFLAGS`, which
`scripts/env.sh` sets, rather than from a `-j` of its own.

## Numbers

| | clang++-19 | kiln, first build | kiln, final build |
|---|---|---|---|
| TUs ok / failed (of 240) | 239 / 1 | 202 / 38 | 239 / 1 |
| ctest passed (of 239) | 238 (tokenize not run) | 201 (38 not run) | 238 (tokenize not run) |
| ctest wall | 1.92 s | — | 1.48 s |
| `make` wall | 273 s | 95 s | 45 s |
| sum of per-TU wall | 1713.6 s | 366.6 s | 848.0 s |
| peak TU RSS | 391536 KB (`test/view/zip.cpp`) | 175852 KB | 174172 KB (`test/algorithm/transform.cpp`) |

How to read the wall numbers: the host was shared, its load varied
between runs (load average roughly 2 to 16 on 24 cores), and `make` drew
tokens from a shared jobserver. The wall numbers are therefore noisy, and the
final kiln build ran under the heaviest load. For the same TU, clang's wall
divided by kiln's has a median of 1.72 in the first kiln build and 0.79 in
the final build. Also note that the clang control runs LLVM's `-Ofast`
pipeline inside the TU, while kiln stops at LLVM IR. Peak RSS does not depend
on load: kiln's per-TU RSS has a median of 0.40 times clang's, and its
largest TU uses 45% of clang's largest.

## Clusters (first kiln build)

Each repro file is the conformance test that was reduced from the cluster.

| # | Signature (first error) | TUs | Status | Repro |
|---|---|---|---|---|
| A | `test_utils.hpp:144` non-const lvalue reference to type `subrange<…>` cannot bind (`as_lvalue` returning a by-value parameter) | 18 | fixed in `f7a58ea73` | `tests/conformance/class.copy.elision/p3-cxx20-two-stage.cpp` |
| B | `common_tuple.hpp:82` declaration type contains unexpanded parameter pack | 6 | fixed in `2495fa7cb` | `tests/conformance/temp.variadic/p5-partial-expr-expansion-member-template.cpp` |
| C | `invoke.hpp:133` reference to non-static member function must be called; `type_traits:1499` incomplete type `<bound member function type>` | 7 | fixed in `21b8bc6e9` | `tests/conformance/expr.mptr.oper/p6-bound-member-unevaluated.cpp`, `p6-bound-member-unevaluated-sfinae.cpp` |
| D | `set_union` / `set_symmetric_difference`: no matching function for call to object | 2 | fixed in `732cff17d` | `tests/conformance/expr.cond/p4-base-not-converted-to-derived.cpp` |
| E | `utility/common_type.cpp` static_assert failed (conditional operator over base/derived) | 1 | fixed in `732cff17d` | `tests/conformance/expr.cond/p4-base-not-converted-to-derived.cpp` |
| G | `iterator_range.hpp:228` invalid use of `this` (`&C::m` dependent, `this` in a friend declarator) | 1 | fixed in `a29935d08` | `tests/conformance/expr.unary.op/p3-qualified-member-pointer-dependent-type.cpp`, `tests/conformance/expr.prim.this/p2-not-in-friend-declarator.cpp` |
| H | `meta.hpp:1852` constraints not satisfied for alias template `_t` (deduced template template argument not checked) | 1 | fixed in `160661979` | `tests/conformance/temp.arg.template/p3-deduced-argument-checked.cpp` |
| F | `concat.hpp:310` no matching member function for call to `visit_i` | 1 | fixed in `56db3a105` and `f239e83a9` | `tests/conformance/temp.arg.template/p3-trailing-pack-may-be-empty.cpp`, `tests/conformance/temp.func.order/p3-constant-argument-more-specialized.cpp` |
| — | `test/view/tokenize.cpp` static_assert: regex token iterator range is not a forward range | 1 | not a kiln bug (clang fails it the same way) | — |

## Other differences (follow-ups, no TU affected)

- **Configure probes**: kiln rejects `-fdiagnostics-show-template-tree`,
  `-fstrict-vtable-pointers` and `-fvectorize`, which clang accepts, so
  range-v3's `check_cxx_compiler_flag` probes report Failed under kiln. These
  flags only affect diagnostics and optimization. Either accept them or ignore
  them with a warning (`crates/kiln_driver/src/args.rs`).
- **`__no_sanitize__` spelling**: `__attribute__((__no_sanitize__("…")))`
  (from `RANGES_INTENDED_MODULAR_ARITHMETIC`, `range/v3/utility/random.hpp`)
  gives 36 `-Wunknown-attributes` warnings. kiln does not strip the
  `__x__` underscores from the GNU attribute name `no_sanitize` before
  looking it up.
- **`-Wmissing-noreturn` false positive**: kiln warns on a lambda whose body
  only throws but whose return type is explicit
  (`[](int, int) -> bool { throw 0; }`, `test/view/chunk_by.cpp:156`).
  There are 2 such warnings, and clang gives none
  (`crates/kiln_sema/src/missing_noreturn.rs`).
- **Warnings kiln does not have**: under `-Weverything`, clang emits 52150
  warnings and kiln 2417. The gap is almost entirely `-Wunsafe-buffer-usage`
  (49158), plus `-Wdeprecated-declarations` (612) and `-Wshadow` (1). These are
  in the diagnostics long tail.

## What remains

Nothing blocks the build. The follow-ups above are all diagnostics or driver
polish. `tokenize` fails under clang too and needs a libstdc++ or range-v3
change.
