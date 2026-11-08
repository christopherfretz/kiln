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

## libc++ (WP-694)

Built and tested with `-stdlib=libc++` (the system libc++ 19) by both kiln++ and the clang++-19 control: `scripts/oss/oss-build.sh --stdlib libc++ <project>`, report via `scripts/oss/oss-report.py --stdlib libc++ --perf <project>`. The clang-control build tree was deleted after these numbers were recorded. Wall-time differences are dominated by file-system syscalls on the bind mount (see the perf finding in [../oss.md](../oss.md#libc-results-wp-694)).

The harness entry now passes `-DRANGES_ENABLE_WERROR=OFF -DCMAKE_CXX_FLAGS=-Wno-deprecated-ofast` (as WP-669 did by hand): without them the clang-19 control fails every TU on range-v3's `-Werror` plus clang 19's `-Ofast` deprecation. Both compilers fail the same TU and link: `test/view/tokenize.cpp` (libc++ 19's `regex_token_iterator` does not model a forward iterator, so the view does not satisfy the test's concept checks) and the `calendar` example link (the system Boost is built against libstdc++). Neither is a kiln issue.

<!-- oss:range_v3:libcxx -->
### range_v3 (0.12.0), libc++

| | kiln | clang-19 |
|---|---|---|
| configure | ok | ok |
| build | rc 2 | rc 2 |
| TUs ok / failed | 256 / 1 | 256 / 1 |
| links ok / failed | 254 / 1 | 254 / 1 |
| tests passed / total | 254 / 255 | 254 / 255 |
| wall | 216 s | 451 s |
| peak TU RSS | 243 MB | 512 MB |

Failure clusters (kiln): 2.

1. **1x** `link: (.text+NxN): undefined reference to Q`; 1 also fails with clang
   - e.g. `link`: `(.text+0x108): undefined reference to `std::_Rb_tree_decrement(std::_Rb_tree_node_base*)'`
   - repro: `cd $KILN/.oss/build/range_v3/kiln-libcxx/example && kiln++ -stdlib=libc++ -Wno-deprecated-ofast -O3 -DNDEBUG -Xlinker --dependency-file=CMakeFiles/range.v3.calendar.dir/link.d CMakeFiles/range.v3.calendar.dir/calendar.cpp.o -o range.v3.calendar /usr/lib/aarch64-linux-gnu/libboost_date_time.a /usr/lib/aarch64-linux-gnu/libboost_program_options.a`
2. **1x** `reject: error: static assertion failed due to requirement Q: Concept assertion failed : forward_range<decltype(rng)>`; 1 also fails with clang
   - e.g. `$KILN/.oss/src/range_v3/test/view/tokenize.cpp`: `$KILN/.oss/src/range_v3/test/view/tokenize.cpp:25:5: error: static assertion failed due to requirement 'static_cast<bool>(forward_range<ranges::tokenize_view<ranges::ref_view<std::string>, std::regex, int>>)': Concept assertion failed : forward_range<decltype(rng)>`
   - repro: `cd $KILN/.oss/build/range_v3/kiln-libcxx/test/view && kiln++ -stdlib=libc++ -I$KILN/.oss/src/range_v3/include -Wno-deprecated-ofast -O3 -DNDEBUG -std=gnu++20 -std=c++2a -Wall -Wextra -D__extern_always_inline=inline -ftemplate-backtrace-limit=0 -fmacro-backtrace-limit=1 -fomit-frame-pointer -Ofast -fstrict-aliasing -ffast-math -fsized-deallocation -march=native -mtune=native -Weverything -pedantic -pedantic-errors -Wno-c++98-compat -Wno-c++98-compat-pedantic -Wno-weak-vtables -Wno-padded -Wno-missing-variable-declarations -Wno-documentation -Wno-documentation-unknown-command -Wno-old-style-cast -Wno-missing-prototypes -MD -MT test/view/CMakeFiles/range.v3.view.tokenize.dir/tokenize.cpp.o -MF CMakeFiles/range.v3.view.tokenize.dir/tokenize.cpp.o.d -o CMakeFiles/range.v3.view.tokenize.dir/tokenize.cpp.o -c $KILN/.oss/src/range_v3/test/view/tokenize.cpp`

kiln test run: `99% tests passed, 1 tests failed out of 255`

Performance over the 256 TUs that compiled with both (CPU time, peak RSS of the compiler and its backend):
geomean kiln/clang-19 time **1.06x**, RSS **0.37x**; total 556 s vs 566 s; peak TU RSS 243 MB vs 512 MB.

| heaviest TUs (by clang-19 time) | clang-19 s | kiln s | time x | clang-19 MB | kiln MB | RSS x |
|---|---|---|---|---|---|---|
| `$KILN/.oss/src/range_v3/test/algorithm/search.cpp` | 12.4 | 5.5 | 0.44 | 280 | 120 | 0.43 |
| `$KILN/.oss/src/range_v3/example/calendar.cpp` | 9.2 | 8.7 | 0.95 | 512 | 243 | 0.47 |
| `$KILN/.oss/src/range_v3/test/algorithm/transform.cpp` | 9.2 | 7.8 | 0.85 | 344 | 168 | 0.49 |
| `$KILN/.oss/src/range_v3/test/algorithm/set_symmetric_difference4.cpp` | 8.3 | 6.6 | 0.80 | 400 | 162 | 0.41 |
| `$KILN/.oss/src/range_v3/test/algorithm/set_symmetric_difference2.cpp` | 8.2 | 6.4 | 0.78 | 403 | 156 | 0.39 |

TUs over 1.2x on time or RSS: 46 of the 253 that take clang-19 at least 0.5 s.
- `$KILN/.oss/src/range_v3/test/algorithm/merge.cpp`: time 1.4 / 1.0 s = 1.44x, RSS 67 / 192 MB = 0.35x
- `$KILN/.oss/src/range_v3/test/algorithm/max.cpp`: time 2.0 / 1.4 s = 1.40x, RSS 75 / 204 MB = 0.37x
- `$KILN/.oss/src/range_v3/test/algorithm/lower_bound.cpp`: time 1.7 / 1.2 s = 1.37x, RSS 72 / 202 MB = 0.36x
- `$KILN/.oss/src/range_v3/test/algorithm/make_heap.cpp`: time 1.9 / 1.4 s = 1.36x, RSS 73 / 207 MB = 0.35x
- `$KILN/.oss/src/range_v3/test/algorithm/any_of.cpp`: time 1.4 / 1.0 s = 1.35x, RSS 67 / 192 MB = 0.35x
<!-- /oss:range_v3:libcxx -->
