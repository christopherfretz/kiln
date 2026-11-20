# kiln 1.3.0 release measurements

Written by `cargo xtask release-gate record` (docs/RELEASING.md). The tables between the `release-gate` markers are generated from the gate's state records; text outside them is written by hand and kept.

The release gate for 1.3.0, a feature release
([../../CHANGELOG.md](../../CHANGELOG.md)), run with
`cargo xtask release-gate` for the first time: the release archive, the
merge gate and the clang 19 oracle lane, the precompiled-header release
sweep, gcc.c-torture in two modes, libc++ 22.1.8 and GCC's C++ testsuite
on aarch64 and x86_64, the x86_64 sweep, the full open-source corpus and a
downstream reflection-heavy build. The baseline is the 1.2.1 gate
([release-1.2.1.md](release-1.2.1.md)) and the counts stored with it; a
step passes when it is not below that baseline.

- Release candidate: `ade1f5ae6`, version `1.3.0`. The tag goes on the
  commit that adds these results, which changes only docs.
- Host: aarch64 Linux unless a row says x86_64 (hosted). The host is
  shared with other builds; the load at each step is in the summary.
- This is the fourth candidate. The third, `ec6b56961`, regressed one
  project: boost_heavy compiled 1362 of its 1374 translation units
  (1.2.1: 1370). A qualified name used as the operand of `&` inside a
  template could leak that state into a class template instantiated while
  analysing it, so a member named in the instantiated class was access
  checked as if it formed a pointer to member ([class.protected]) and
  rejected. The fix (WP-1727, with the conformance test
  `tests/conformance/class.access/address-operand-nested-instantiation.cpp`)
  is the only change from the third candidate, and boost_heavy is back at
  1370/1374 below. Every step was run again on this candidate.

## Summary

<!-- release-gate:begin summary commit=ade1f5ae6 version=1.3.0 mode=release fnv64=a86b8c9d04c6d6c0 -->
- Release candidate: `ade1f5ae6`, version `1.3.0`, mode `release`.
- 71 steps, run 2026-11-19 20:16 UTC to 2026-11-20 04:39 UTC; load (1-minute average at each step's start and end): 7.54-7.76, 7.76-12.35, 12.35-12.75, 11.30-8.87, 12.75-8.57, 7.76-11.43, 11.43-13.14, 13.14-11.02, 11.02-12.45, 12.45-12.25, 12.25-11.75, 11.75-11.75, 11.75-10.72, 10.72-11.86, 11.86-10.64, 10.64-9.07, 9.07-8.88, 8.88-11.06, 11.06-10.85, 10.85-11.32, 11.32-12.69, 12.69-12.30, 12.30-11.50, 11.50-8.30, 8.30-9.30, 9.30-9.63, 9.63-8.84, 8.84-9.06, 9.06-8.41, 8.41-6.52, 6.52-8.24, 8.24-7.52, 7.52-9.67, 9.67-10.73, 10.73-8.63, 8.80-8.21, 11.52-10.06, 10.06-8.02, 8.02-8.32, 8.32-7.83, 7.83-9.93, 9.93-6.58, 6.58-10.27, 10.27-10.78, 10.78-9.29, 9.29-10.54, 10.54-10.97, 10.97-10.98, 10.98-11.31, 11.31-10.50, 10.50-9.89, 9.89-11.83, 11.83-7.55, 7.55-7.40, 7.40-7.93, 7.93-18.36, 18.36-10.24, 10.24-9.21, 9.21-8.15, 8.15-7.51, 7.51-7.66, 7.66-7.97, 7.97-7.74, 7.74-8.49, 8.49-8.49, 8.49-7.45, 7.45-8.39, 8.57-8.57.

**Status: every step passed.**
<!-- release-gate:end summary -->

## RC gate: the release archive

<!-- release-gate:begin archive commit=ade1f5ae6 version=1.3.0 mode=release fnv64=e6483305c2a1a3a3 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `archive` | aarch64 | `ade1f5ae6` | 2 | archives_ok=2 | pass | kiln version 1.3.0 (C++26 frontend, LLVM IR backend); kiln++ 33c028a2295f26e2 |
<!-- release-gate:end archive -->

## The merge gate, both oracle lanes

<!-- release-gate:begin merge-gate commit=ade1f5ae6 version=1.3.0 mode=release fnv64=f49eb79fc25932b6 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `merge-gate` | aarch64 | `ade1f5ae6` | 8122 | passed=8122 failed=0 lines=6 | pass | 6 corpus lines, 0 failed |
| `oracle19` | aarch64 | `ade1f5ae6` | 7992 | passed=7992 failed=0 lines=3 | pass | 3 corpus lines, 0 failed |
<!-- release-gate:end merge-gate -->

## Precompiled headers: the release sweep

<!-- release-gate:begin pch commit=ade1f5ae6 version=1.3.0 mode=release fnv64=7ca78075b8cd212a -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `pch-sweep` | aarch64 | `ade1f5ae6` | 4299 | libcxx_pair_tests=4299 | pass | pch-release-gate: pass |
<!-- release-gate:end pch -->

## C mode: gcc.c-torture

<!-- release-gate:begin torture commit=ade1f5ae6 version=1.3.0 mode=release fnv64=cc65bc6a4c1c1581 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `torture-gnu23` | aarch64 | `ade1f5ae6` | 7572 | pass=6938 run=7572 expect_unknown=0 | pass | not below the stored baseline (1 counts) |
| `torture-gnu17` | aarch64 | `ade1f5ae6` | 7572 | pass=7098 run=7572 expect_unknown=0 | pass | not below the stored baseline (1 counts) |
<!-- release-gate:end torture -->

## libc++ 22.1.8 test suite

<!-- release-gate:begin libcxx commit=ade1f5ae6 version=1.3.0 mode=release fnv64=7c4ec1a7de7ae178 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `libcxx-aarch64` | aarch64 | `ade1f5ae6` | 9247 | pass=8748 fail=6 xfail=46 xpass=0 unsupported=446 killed=0 timeout=1 error=0 total=9247 | pass | not below the stored baseline (1 counts) |
| `libcxx-x86_64` | x86_64 (hosted) | `ade1f5ae6` | 9247 | pass=8749 fail=6 xfail=45 xpass=0 unsupported=446 killed=0 timeout=1 error=0 total=9247 | pass | not below the stored baseline (1 counts) |
<!-- release-gate:end libcxx -->

## GCC C++ testsuite (g++.dg)

<!-- release-gate:begin gxx-dg commit=ade1f5ae6 version=1.3.0 mode=release fnv64=9a12731b47eedf53 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `gxx-dg-aarch64` | aarch64 | `ade1f5ae6` | 56766 | pass=50268 decided=56568 run=56766 | pass | gate: 1 explained, 0 unexplained pass -> failing; 57552 cases run: pass |
| `gxx-dg-x86_64` | x86_64 (hosted) | `ade1f5ae6` | 57070 | pass=50486 decided=56870 run=57070 | pass | gate: 1 explained, 0 unexplained pass -> failing; 57749 cases run: pass |

Explained pass-to-failing cases (docs/status/gxx-dg-explained.txt): 1
- g++.dg/cpp0x/explicit-inst1.C [-std=c++98] WP-1693: kiln follows C++03 14.7.2/5 and DR 275 (an unqualified explicit instantiation outside the template's namespace is ill-formed), GCC and clang accept in C++98; docs/status/deviations.md
<!-- release-gate:end gxx-dg -->

## Open-source projects

<!-- release-gate:begin oss commit=ade1f5ae6 version=1.3.0 mode=release fnv64=d52b8e0e2459bfa1 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `oss-x86_64` | x86_64 (hosted) | `ade1f5ae6` | 14 | projects=14 reported=14 below_baseline=0 | pass | all 14 sweep rows at their baseline |
| `oss-fmt` | aarch64 | `ade1f5ae6` | 50 | tus_ok=50 tus_total=50 tests_passed=21 tests_total=21 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-fmt_module` | aarch64 | `ade1f5ae6` | 2 | tus_ok=2 tus_total=2 tests_passed=1 tests_total=1 | pass | at or above the oss.md baseline |
| `oss-cmake_modules` | aarch64 | `ade1f5ae6` | 4 | tus_ok=4 tus_total=4 tests_passed=0 tests_total=0 | pass | at or above the oss.md baseline |
| `oss-c_demo` | aarch64 | `ade1f5ae6` | 3 | tus_ok=3 tus_total=3 tests_passed=0 tests_total=0 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-nlohmann_json` | aarch64 | `ade1f5ae6` | 93 | tus_ok=93 tus_total=93 tests_passed=97 tests_total=97 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-spdlog` | aarch64 | `ade1f5ae6` | 140 | tus_ok=140 tus_total=140 tests_passed=1 tests_total=1 | pass | at or above the oss.md baseline |
| `oss-catch2` | aarch64 | `ade1f5ae6` | 167 | tus_ok=167 tus_total=167 tests_passed=81 tests_total=81 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-catch2_cxx23` | aarch64 | `ade1f5ae6` | 167 | tus_ok=167 tus_total=167 tests_passed=81 tests_total=81 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-catch2_werror` | aarch64 | `ade1f5ae6` | 167 | tus_ok=167 tus_total=167 tests_passed=81 tests_total=81 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-googletest` | aarch64 | `ade1f5ae6` | 85 | tus_ok=85 tus_total=85 tests_passed=63 tests_total=63 | pass | at or above the oss.md baseline |
| `oss-doctest` | aarch64 | `ade1f5ae6` | 121 | tus_ok=121 tus_total=121 tests_passed=193 tests_total=193 | pass | at or above the oss.md baseline |
| `oss-magic_enum` | aarch64 | `ade1f5ae6` | 22 | tus_ok=22 tus_total=22 tests_passed=15 tests_total=15 | pass | at or above the oss.md baseline |
| `oss-cli11` | aarch64 | `ade1f5ae6` | 65 | tus_ok=65 tus_total=65 tests_passed=86 tests_total=86 | pass | at or above the oss.md baseline |
| `oss-tl_expected` | aarch64 | `ade1f5ae6` | 13 | tus_ok=13 tus_total=13 tests_passed=1 tests_total=1 | pass | at or above the oss.md baseline |
| `oss-range_v3` | aarch64 | `ade1f5ae6` | 257 | tus_ok=256 tus_total=257 tests_passed=254 tests_total=255 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-abseil` | aarch64 | `ade1f5ae6` | 413 | tus_ok=413 tus_total=413 tests_passed=232 tests_total=232 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-re2` | aarch64 | `ade1f5ae6` | 51 | tus_ok=51 tus_total=51 tests_passed=20 tests_total=20 | pass | at or above the oss.md baseline |
| `oss-benchmark` | aarch64 | `ade1f5ae6` | 73 | tus_ok=73 tus_total=73 tests_passed=84 tests_total=84 | pass | at or above the oss.md baseline |
| `oss-glog` | aarch64 | `ade1f5ae6` | 23 | tus_ok=23 tus_total=23 tests_passed=23 tests_total=24 | pass | at or above the oss.md baseline |
| `oss-yaml_cpp` | aarch64 | `ade1f5ae6` | 51 | tus_ok=51 tus_total=51 tests_passed=1 tests_total=1 | pass | at or above the oss.md baseline |
| `oss-simdjson` | aarch64 | `ade1f5ae6` | 135 | tus_ok=135 tus_total=135 tests_passed=117 tests_total=117 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-eigen` | aarch64 | `ade1f5ae6` | 539 | tus_ok=539 tus_total=539 tests_passed=538 tests_total=539 | pass | at or above the oss.md baseline |
| `oss-entt` | aarch64 | `ade1f5ae6` | 73 | tus_ok=73 tus_total=73 tests_passed=68 tests_total=68 | pass | at or above the oss.md baseline |
| `oss-taskflow` | aarch64 | `ade1f5ae6` | 27 | tus_ok=27 tus_total=27 tests_passed=2462 tests_total=2462 | pass | at or above the oss.md baseline |
| `oss-cereal` | aarch64 | `ade1f5ae6` | 39 | tus_ok=39 tus_total=39 tests_passed=40 tests_total=40 | pass | at or above the oss.md baseline |
| `oss-protobuf` | aarch64 | `ade1f5ae6` | 653 | tus_ok=653 tus_total=653 tests_passed=4 tests_total=4 | pass | at or above the oss.md baseline |
| `oss-leveldb` | aarch64 | `ade1f5ae6` | 121 | tus_ok=121 tus_total=121 tests_passed=30 tests_total=30 | pass | at or above the oss.md baseline |
| `oss-rocksdb` | aarch64 | `ade1f5ae6` | 573 | tus_ok=573 tus_total=573 tests_passed=2158 tests_total=2158 | pass | at or above the oss.md baseline |
| `oss-llvm_project` | aarch64 | `ade1f5ae6` | 1504 | tus_ok=1504 tus_total=1504 tests_passed=0 tests_total=0 | pass | at or above the oss.md baseline |
| `oss-llvm_full` | aarch64 | `ade1f5ae6` | 2780 | tus_ok=2780 tus_total=2780 tests_passed=60697 tests_total=60705 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-llvm_clang` | aarch64 | `ade1f5ae6` | 3573 | tus_ok=3573 tus_total=3573 tests_passed=2960 tests_total=2961 | pass | at or above the oss.md baseline |
| `oss-llvm_selfhost` | aarch64 | `ade1f5ae6` | 3573 | tus_ok=3573 tus_total=3573 tests_passed=2960 tests_total=2961 | pass | at or above the oss.md baseline |
| `oss-folly` | aarch64 | `ade1f5ae6` | 726 | tus_ok=724 tus_total=726 tests_passed=3704 tests_total=3773 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-duckdb` | aarch64 | `ade1f5ae6` | 421 | tus_ok=421 tus_total=421 tests_passed=4615 tests_total=4628 | pass | at or above the oss.md baseline |
| `oss-z3` | aarch64 | `ade1f5ae6` | 972 | tus_ok=972 tus_total=972 tests_passed=0 tests_total=0 | pass | at or above the oss.md baseline |
| `oss-stdexec` | aarch64 | `ade1f5ae6` | 231 | tus_ok=231 tus_total=231 tests_passed=907 tests_total=907 | pass | at or above the oss.md baseline |
| `oss-libunifex` | aarch64 | `ade1f5ae6` | 114 | tus_ok=114 tus_total=114 tests_passed=91 tests_total=94 | pass | at or above the oss.md baseline |
| `oss-folly_coro` | aarch64 | `ade1f5ae6` | 429 | tus_ok=429 tus_total=429 tests_passed=603 tests_total=618 | pass | at or above the oss.md baseline |
| `oss-ctre` | aarch64 | `ade1f5ae6` | 25 | tus_ok=25 tus_total=25 tests_passed=0 tests_total=0 | pass | at or above the oss.md baseline |
| `oss-glaze` | aarch64 | `ade1f5ae6` | 89 | tus_ok=87 tus_total=89 tests_passed=100 tests_total=102 | pass | at or above the oss.md baseline |
| `oss-mp_units` | aarch64 | `ade1f5ae6` | 64 | tus_ok=64 tus_total=64 tests_passed=25 tests_total=25 | pass | at or above the oss.md baseline |
| `oss-scnlib` | aarch64 | `ade1f5ae6` | 47 | tus_ok=47 tus_total=47 tests_passed=25 tests_total=25 | pass | at or above the oss.md baseline |
| `oss-daw_json_link` | aarch64 | `ade1f5ae6` | 159 | tus_ok=159 tus_total=159 tests_passed=147 tests_total=147 | pass | at or above the oss.md baseline |
| `oss-reflect_cpp` | aarch64 | `ade1f5ae6` | 327 | tus_ok=327 tus_total=327 tests_passed=484 tests_total=484 | pass | at or above the oss.md baseline |
| `oss-boost_heavy` | aarch64 | `ade1f5ae6` | 1374 | tus_ok=1370 tus_total=1374 tests_passed=602 tests_total=602 | pass | at or above the oss.md baseline |
| `oss-boost_meta` | aarch64 | `ade1f5ae6` | 2253 | tus_ok=2235 tus_total=2253 tests_passed=2082 tests_total=2087 | pass | at or above the oss.md baseline |
| `oss-boost_compiled` | aarch64 | `ade1f5ae6` | 1923 | tus_ok=1904 tus_total=1923 tests_passed=1326 tests_total=1330 | pass | at or above the oss.md baseline |
| `oss-onetbb` | aarch64 | `ade1f5ae6` | 184 | tus_ok=184 tus_total=184 tests_passed=139 tests_total=139 | pass | at or above the oss.md baseline |
| `oss-opencv` | aarch64 | `ade1f5ae6` | 635 | tus_ok=635 tus_total=635 tests_passed=7 tests_total=7 | pass | at or above the oss.md baseline |
| `oss-cmake_app` | aarch64 | `ade1f5ae6` | 587 | tus_ok=587 tus_total=587 tests_passed=691 tests_total=714 | pass | at or above the oss.md baseline |
| `oss-bitcoin_core` | aarch64 | `ade1f5ae6` | 477 | tus_ok=477 tus_total=477 tests_passed=357 tests_total=357 | pass | at or above the oss.md baseline |
| `oss-grpc` | aarch64 | `ade1f5ae6` | 5732 | tus_ok=5732 tus_total=5732 tests_passed=117 tests_total=118 | pass | at or above the oss.md baseline |
| `oss-jsoncpp_meson` | aarch64 | `ade1f5ae6` | 7 | tus_ok=7 tus_total=7 tests_passed=3 tests_total=3 | pass | at or above the oss.md baseline |
| `oss-hunspell_autotools` | aarch64 | `ade1f5ae6` | 36 | tus_ok=36 tus_total=36 tests_passed=130 tests_total=130 | pass | at or above the oss.md baseline |
| `oss-zlib` | aarch64 | `ade1f5ae6` | 37 | tus_ok=37 tus_total=37 tests_passed=6 tests_total=6 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-sqlite` | aarch64 | `ade1f5ae6` | 2 | tus_ok=2 tus_total=2 tests_passed=0 tests_total=0 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-lua` | aarch64 | `ade1f5ae6` | 34 | tus_ok=34 tus_total=34 tests_passed=0 tests_total=0 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-c_ares` | aarch64 | `ade1f5ae6` | 126 | tus_ok=126 tus_total=126 tests_passed=2 tests_total=2 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
| `oss-zstd` | aarch64 | `ade1f5ae6` | 118 | tus_ok=118 tus_total=118 tests_passed=3 tests_total=3 | pass | no docs/status/oss.md section: counts recorded, nothing to compare |
<!-- release-gate:end oss -->

rocksdb ran four times before it reached its baseline of 2158 tests. All
four builds compiled all 573 translation units; the test counts were
2153, 2156, 2154 and 2158. The second run shared the host with the
precompiled-header sweep. A control
(`release-gate control oss-rocksdb rocksdb --previous <1.2.1 kiln++> --times 3`)
built and tested rocksdb three times with each compiler:

| run | the release candidate | 1.2.1 |
|---|---|---|
| 1 | 2154/2158 | 2157/2158 |
| 2 | 2156/2158 | 2153/2158 |
| 3 | 2157/2158 | 2154/2158 |

Both compilers miss the same way. In the last run of each, the tests that
failed were `DBSecondaryCacheTest.TestSecondaryCacheOptionTwoDB/"lru"`
with the candidate, and that test,
`DBSecondaryCacheTest.SecondaryCacheIntensiveTesting`,
`DBMergeOperandTest.GetMergeOperandsShortCircuitInSst` and
`MemoryTest.MemTableAndTableReadersTotal` with 1.2.1. These are timing-
and memory-accounting tests that race on this shared host; the earlier
candidates measured 2152 and 2155. The count above is the run that
reached the baseline.

The other rows are compared by the gate with the counts in
[oss.md](oss.md) and its per-project pages. glog passed 23 of its 24
tests, one more than at 1.2.1 (22/24); its remaining failure is the
project's own test race on this host ([release-1.1.0.md](release-1.1.0.md)).

## Downstream build

<!-- release-gate:begin external commit=ade1f5ae6 version=1.3.0 mode=release fnv64=ed52c1d8482b8196 -->
| step | host | commit | units | counts | verdict | detail |
|---|---|---|---:|---|---|---|
| `external-downstream` | aarch64 | `ade1f5ae6` | 945 | units=945 | pass | downstream: PASS at ade1f5ae6 (source build in the peer image): diagnostics lane 945/945 on both passes, 944/944 consumers restored, 0 replayed, 0 -Wpch-snapshot-unused, PCH 100168963 bytes, container peak 1.08 GiB, diag log identical to RC 3 units=945 |
<!-- release-gate:end external -->
