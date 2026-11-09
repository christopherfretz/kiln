# x86_64 sweep (WP-961)

The development container is aarch64. `.github/workflows/sweep.yml` runs
the libc++ 22 suite and ten OSS projects on GitHub's `ubuntu-24.04`
x86_64 runners, in the CI reference container `gcc:16.1.0-trixie`
(Debian 13, clang 19 and 22, GCC 14's libstdc++), with kiln++ from
`cargo xtask dist`. How to run it: docs/VALIDATION.md "x86_64 sweep".
This page records the first sweep (2026-11-08/09) and its triage.

## Runs

Run URLs are `https://github.com/christopherfretz/kiln/actions/runs/<id>`.

- x86-1, `42542596193` (`7dd11f237`): cancelled. A packaging bug in the
  `build` job (the dist archive's tar invocation), fixed in `faeab29f4`.
- x86-2, `42543199768` (`faeab29f4`): **the full sweep**. libc++ 6
  shards, all ten projects with kiln and with the clang 19 control. The
  `libcxx summary` job failed on an artifact path (the upload kept the
  runner's `__w/_temp` prefix); fixed in `b3281188c`, the counts below are
  merged locally from its six results files.
- x86-3, `42544313562` (`212ace07b`): one-off, kiln only, abseil,
  bitcoin_core and daw_json_link. abseil and bitcoin_core with
  `-Wno-invalid-constexpr` added to their flags; daw_json_link with the
  `-mbranches-within-32B-boundaries` fix.
- x86-4, `42545156708` (`9afe2a822`): one-off, kiln only, libc++ again
  (the summary job verified) and all ten projects with
  `-Wno-invalid-constexpr` added to every project's flags.

The one-off flag masks main's WP-873 never-constant regression (see
"Triage"), which otherwise hides most of abseil, grpc, bitcoin_core and
daw_json_link. It was only on the `sweep/x86-3` and `sweep/x86-4`
branches; `sweep.yml` on main has no such flag. All four `sweep/x86-*`
branches are deleted (a run continues when its branch is deleted).

## libc++ 22.1.8 (x86-2; x86-4 is identical)

| scope | total | pass | fail | xfail | xpass | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| all, x86_64 | 9247 | 8714 | 38 | 46 | 1 | 446 | 2 | 99.0% |
| `std/`, x86_64 | 8424 | 8200 | 33 | 35 | 1 | 153 | 2 | 99.1% |
| `libcxx/`, x86_64 | 823 | 514 | 5 | 11 | 0 | 293 | 0 | 97.0% |
| all, aarch64 M5 record (WP-461, older main) | 9247 | 8716 | 9 | 64 | 0 | 457 | 1 | 99.2% |

The M5 record predates main's current head, so the x86_64-only test is
direct: every test that failed on x86_64, plus the rest of its directory
(308 tests), re-run on aarch64 with kiln++ from the same commit, and
compared with `scripts/libcxx-results.py --baseline`. Two are x86_64-only
(below); the other 39 fail identically on aarch64. One test failed only
on aarch64: `std/experimental/simd/simd.class/simd_copy.pass.cpp`, a
timeout of the local debug kiln++.

## OSS projects

kiln against the clang 19 control on an identical runner (x86-2), libstdc++ 14.
Tests are "passed / total".

- **fmt** `12.1.0`: 21/21, 50/50 TUs; clang 21/21.
- **nlohmann_json** `v3.12.0`: 97/97, 93/93 TUs; clang 97/97. (The
  aarch64 run counts 101 tests: a different ctest selection, same on both
  compilers.)
- **abseil** `20260107.1`: x86-2 37/232, 89/102 TUs (WP-873 regression,
  recorded as-is). With the one-off flag (x86-3, x86-4): 232/232, 413/413
  TUs; clang 232/232.
- **llvm_project** `llvmorg-19.1.7`: `SupportTests` 1337 passed and
  `ADTTests` 1576 passed, as clang; `IRTests` did not build: one TU,
  `lib/Analysis/ConstantFolding.cpp`, rejected (`no matching function for
  call to 'ConstantFoldFP128'`, x86_64-only, `_Float128`), 922/923 TUs
  before the build stopped. clang: 1337 / 1576 / 732, 1504/1504 TUs.
- **duckdb** `v1.5.6`: 4618/4628, 421/421 TUs; clang 4618/4628 (the same
  10 fail under both).
- **grpc** `v1.85.1`: x86-2 0/118, 65/918 TUs: 822 TUs hit the WP-873
  regression through abseil's headers and 31 the missing `__rdtsc`
  (fixed). The full builds, kiln with the one-off flag (x86-4) and the
  clang control (x86-2), were still running when this page was written
  (5732 TUs on a 4-vCPU runner; the aarch64 builds took 4–5 h); their
  `oss-grpc-*` artifacts hold the result.
- **bitcoin_core** `v32.1`: x86-2 220/357 (WP-873). With the one-off flag
  (x86-3, x86-4): 357/357, 475/475 TUs; clang 357/357, 487/487 TUs. The
  12 TUs are the SSE4.1/AVX2/SHA-NI/SSE4.2/CLMUL code that CMake's
  `HAVE_*` probes enable only when the `-m<feature>` flags are accepted
  (x86_64-only).
- **cmake_app** `v4.4.5`: 721/723, 587/587 TUs; clang 721/723 (the same
  `RunCMake.CPack_DEB.DEBUGINFO` and `RunCMake.PrecompileHeaders` fail
  under both).
- **opencv** `4.12.0`: 7/7, 473/473 TUs; clang 7/7, 537/537 TUs. The 64
  TUs are the SSE3…AVX2 dispatch sources, which OpenCV's CPU-feature
  probes drop as "not supported" when `-m<feature>` is rejected
  (x86_64-only).
- **daw_json_link** `v3.38.1`: x86-2 0/147, 0/159 TUs: every TU rejected
  `-mbranches-within-32B-boundaries` (the project's x86 flag; fixed). x86-3
  7/147 (WP-873). x86-4: 147/147, 159/159 TUs; clang 147/147.

## Triage

### x86_64-only

- `-mbranches-within-32B-boundaries` was an unknown argument: every
  daw_json_link TU. **Fixed**: the driver accepts it on x86 (clang's
  meaning, an assembler padding option with no effect on IR) and rejects
  it on other targets (spec/driver.md; `crates/kiln_driver/tests/hardening.rs`).
- `__rdtsc` was undeclared (ia32intrin.h's `_rdtsc()` expands to it): 31
  grpc TUs. **Fixed**: an alias of `__builtin_ia32_rdtsc`, target-gated
  (spec/sema.md; `tests/conformance/target.x86/rdtsc-ir.cpp`,
  `rdtsc-not-aarch64.cpp`).
- x86 `-m<feature>` / `-mno-<feature>` (`-msse4.2`, `-mavx2`, `-mpclmul`,
  `-msha`, …) are unknown arguments; kiln takes only `-march=<cpu>`. No
  test fails, but CMake probes turn off SIMD code: bitcoin_core 12 TUs,
  opencv 64 TUs. **Filed** (follow-up), xfail
  `tests/conformance/target.x86/m-feature-flags.cpp`.
- glibc's x86-64 `bits/floatn.h` says `typedef __float128 _Float128;`.
  kiln makes `_Float128` its own P1467 type and drops that typedef, so a
  `_Float128` function (`logf128`) is not a `__float128` one, as it is
  under clang. llvm_project's `ConstantFolding.cpp`, and with it
  `IRTests`. **Filed** (follow-up), xfail
  `tests/conformance/basic.extended.fp/float128-glibc-typedef-x86.cpp`.
- `std/algorithms/alg.sorting/alg.sort/stable.sort/stable_sort.pass.cpp`:
  `static_assert(test<char>())` fails. The cause is not x86_64: constexpr
  `++`/`--` on a signed type narrower than `int` overflows in the narrow
  type in the Oven (`kiln_oven` `compile.rs` `inc_dec`) instead of
  promoting and converting back, which wraps. x86_64 exposes it because
  plain `char` is signed there. **Filed** as WP-981, xfail
  `tests/conformance/expr.pre.incr/narrow-signed-wrap-constexpr.cpp`;
  **fixed** in WP-980 (the test now passes).
- `std/atomics/atomics.types.operations/atomics.types.operations.wait/lost_wakeup.pass.cpp`:
  TIMEOUT (run) in x86-2 and x86-4, passes on aarch64. A run-time stress
  test on a 4-vCPU runner under `-j 4`. **Flaky / load**.

### Both arches: main regressions

- WP-873's never-constant check fires on valid code (abseil, grpc via
  abseil, bitcoin_core, daw_json_link `iso8601_utils.h`). Same on both
  arches; main regression (WP-873 never-constant), fixed by WP-962.
- WP-980, libc++ 22 failures that pass in the WP-461 M5 record (same on
  aarch64 at this commit). The fix WP starts from these paths:
  - `cannot yet mangle expression type MaterializeTemporaryExpr` (27):
    - `libcxx/time/time.clock/time.clock.utc/time.clock.utc.members/from_sys.pass.cpp`
    - `libcxx/time/time.clock/time.clock.utc/time.clock.utc.members/to_sys.pass.cpp`
    - `libcxx/time/time.zone/time.zone.info/time.zone.info.local/ostream.pass.cpp`
    - `libcxx/time/time.zone/time.zone.info/time.zone.info.sys/ostream.pass.cpp`
    - `libcxx/time/time.zone/time.zone.timezone/time.zone.members/get_info.sys_time.rule_selection.pass.cpp`
    - `std/time/time.clock/time.clock.local/ostream.pass.cpp`
    - `std/time/time.clock/time.clock.system/sys_time.ostream.pass.cpp`
    - `std/time/time.clock/time.clock.utc/time.clock.utc.members/from_sys.pass.cpp`
    - `std/time/time.clock/time.clock.utc/time.clock.utc.members/to_sys.pass.cpp`
    - `std/time/time.syn/formatter.file_time.pass.cpp`
    - `std/time/time.syn/formatter.local_info.pass.cpp`
    - `std/time/time.syn/formatter.local_time.pass.cpp`
    - `std/time/time.syn/formatter.sys_info.pass.cpp`
    - `std/time/time.syn/formatter.sys_time.pass.cpp`
    - `std/time/time.syn/formatter.zoned_time.pass.cpp`
    - `std/time/time.zone/time.zone.exception/time.zone.exception.ambig/ctor.pass.cpp`
    - `std/time/time.zone/time.zone.exception/time.zone.exception.ambig/types.pass.cpp`
    - `std/time/time.zone/time.zone.exception/time.zone.exception.nonexist/ctor.pass.cpp`
    - `std/time/time.zone/time.zone.exception/time.zone.exception.nonexist/types.pass.cpp`
    - `std/time/time.zone/time.zone.info/time.zone.info.local/ostream.pass.cpp`
    - `std/time/time.zone/time.zone.info/time.zone.info.sys/ostream.pass.cpp`
    - `std/time/time.zone/time.zone.timezone/time.zone.members/sys_info.zdump.pass.cpp`
    - `std/time/time.zone/time.zone.timezone/time.zone.members/to_sys.pass.cpp`
    - `std/time/time.zone/time.zone.zonedtime/deduction.pass.cpp`
    - `std/time/time.zone/time.zone.zonedtime/time.zone.zonedtime.ctor/string_view_local_time.pass.cpp`
    - `std/time/time.zone/time.zone.zonedtime/time.zone.zonedtime.ctor/time_zone_pointer_local_time.pass.cpp`
    - `std/time/time.zone/time.zone.zonedtime/time.zone.zonedtime.members/assign.local_time.pass.cpp`
  - mdspan (5):
    - `std/containers/views/mdspan/layout_right/index_operator.pass.cpp` (static assertion not an integral constant expression)
    - `std/containers/views/mdspan/layout_stride/comparison.pass.cpp` (cannot initialize return object)
    - `std/containers/views/mdspan/layout_stride/ctor.strided_mapping.pass.cpp` (cannot initialize return object)
    - `std/containers/views/mdspan/layout_stride/index_operator.pass.cpp` (static_cast not allowed)
    - `std/containers/views/mdspan/mdspan/index_operator.pass.cpp` (static assertion not an integral constant expression)
  - simd (1): `std/experimental/simd/simd.class/simd_unary.pass.cpp`
    (cannot initialize a vector member subobject with a vector rvalue).

### Both arches: environment and known

- Environment (the container runs as root, so permission-denied paths are
  reachable), `run: assertion failed`:
  `std/input.output/filesystems/class.directory_iterator/directory_iterator.members/ctor.pass.cpp`,
  `std/input.output/filesystems/class.rec.dir.itr/rec.dir.itr.members/ctor.pass.cpp`,
  `std/input.output/filesystems/fs.op.funcs/fs.op.remove/remove.pass.cpp`,
  `std/input.output/filesystems/fs.op.funcs/fs.op.remove_all/remove_all.pass.cpp`.
- Long test, TIMEOUT on both:
  `std/input.output/file.streams/fstreams/ifstream.members/offset_range.pass.cpp`.
- XPASS on both: `std/utilities/variant/variant.hash/hash.depr.verify.cpp`.
- bitcoin_core's configure: `-ftrapv` is unsupported on both arches
  (deliberate, spec/driver.md), so its probe turns it off;
  `-Wthread-safety-pointer` is accepted by kiln and not by clang 19 (a
  clang 22 warning).
