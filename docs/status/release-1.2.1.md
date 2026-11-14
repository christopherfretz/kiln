# kiln 1.2.1 release measurements

The release gate for 1.2.1, a bug-fix release
([../../CHANGELOG.md](../../CHANGELOG.md)): one fix on top of 1.2.0, the
x86-64 calling convention for 256-bit and 512-bit vectors passed or
returned by value when AVX is enabled, cut from the `release/1.2`
maintenance branch ([../VALIDATION.md](../VALIDATION.md) "Patch
releases"). Every release runs the same gate before its tag: the release
archive, the full libc++ suite on aarch64 and x86_64, the full
open-source corpus, gcc.c-torture, the precompiled-header release sweep,
and a downstream reflection-heavy build. The commands are those of
[release-1.0.md](release-1.0.md), with `1.0.0` replaced by `1.2.1`; the
1.2.0 results in [release-1.2.0.md](release-1.2.0.md) are the baseline.
A check passes when it shows no regression against that baseline.

- Release candidate: `a589008e8`, version `1.2.1`. The tag goes on the
  commit that adds these results, which changes only docs.
- Host: aarch64 Linux unless a section says otherwise. The host is
  shared: this gate ran at load 18 to 42, beside other builds.
- The fix changes code generation for x86-64 only. On aarch64 the gate
  shows that nothing else moved. The changed code runs in the x86_64
  sweep and in the hosted merge-gate lanes below, where the fix's
  link-and-run test (kiln objects against clang 19 objects, in both
  pairings) passed on an x86-64 machine.

**Status: complete on `a589008e8`. Every check passed.**

## RC gate: the release archive

[release-1.0.md, "RC gate"](release-1.0.md#rc-gate-the-release-archive):
`cargo xtask dist --check`, `SHA256SUMS`, `kiln++ --version` from the
unpacked archive, and the three programs. Every later section uses that
`kiln++`.

| step | result |
|---|---|
| `dist --check` | passed, including `.gnu_debuglink` matching the symbols archive |
| `sha256sum -c` | both archives OK |
| `--version` | `kiln version 1.2.1`, `aarch64-unknown-linux-gnu`, LLVM backend 22 |
| hello, `<meta>`, constexpr TUs | `hello`, `import std 3`, `2 P` |

The merge gate ran on the branch by hand, both halves
([../VALIDATION.md](../VALIDATION.md) "Patch releases"): `cargo xtask ci`
with 6947 corpus tests passed and none failed, and the clang 19 exact
lane with 6818 corpus tests passed and none failed (147 of 151 passed
and none failed in each of the two module-interface lanes).

A release branch gets no hosted run of its own, so the merge gate's
hosted lanes were also run on the release candidate's tree, on x86_64
and on aarch64 (run 43609184919, through a pull request that was never
merged): all 38 checks passed. The x86_64 unit lane ran
`x86_avx_abi_interop`, the fix's link-and-run test.

## Precompiled headers: the release sweep

`scripts/pch-release-gate.sh <kiln++> <dir> all` with the release
`kiln++` ([pch-snapshot-gates.md](pch-snapshot-gates.md)). The script's
verdict: pass.

Whole corpus (7096 tests, 3592 s):

| pass | fail | skip | restored | replayed | failed before the consumer |
|---|---|---|---|---|---|
| 5729 | 0 | 1367 | 5728 | 1 (allowed: the module unit) | 0 |

| producers | single pass | two passes |
|---|---|---|
| 1431 | 1421 | 10 |

libc++ pair (4299 tests, 3894 of them compiled with a PCH):

| run | pass | fail | xfail | unsupported | wall |
|---|---|---|---|---|---|
| no PCH | 4261 | 1 | 1 | 36 | 2295 s |
| snapshot, default path | 4261 | 1 | 1 | 36 | 3096 s |

Differences between the two runs: none, test by test. The one failure
is `unique.ptr.special/io.verify.cpp` in both, as at 1.2.0. The counts
are those of 1.2.0 plus the nine corpus tests the fix added; the wall
times are longer because the host was busier.

## C mode: gcc.c-torture

`cargo xtask gcc-torture` ([../VALIDATION.md](../VALIDATION.md)) at
`-std=gnu23` and at `-std=gnu17`, each at `-O0` and `-O2`, with GCC 16.1
and clang 22 as the two oracles.

| | kiln | gcc 16.1 | clang 22 | of |
|---|---:|---:|---:|---:|
| gnu23, `a589008e8` | 6912 | 7572 | 6887 | 7572 |
| gnu23, 1.2.0 | 6912 | 7572 | 6887 | 7572 |
| gnu17, `a589008e8` | 7072 | 7570 | 7057 | 7572 |
| gnu17, 1.2.0 | 7072 | 7570 | 7057 | 7572 |

The outcome of every test is that of 1.2.0, at both standards.

## libc++ 22.1.8 test suite (aarch64)

[release-1.0.md (b)](release-1.0.md#b-libc-2218-test-suite-aarch64),
the whole suite (`std libcxx`) at `-j 3`.

| | total | pass | fail | xfail | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1.2.1 RC (`a589008e8`) | 9247 | 8746 | 4 (+2 xpass) | 46 | 446 | 3 | 99.4% |
| 1.2.0 (`2c2e2d1bb`) | 9247 | 8746 | 4 (+2 xpass) | 46 | 446 | 3 | 99.4% |

Run 12:23 to 14:47 UTC on 11-14, load 18 to 33. No test has a different
outcome from the 1.2.0 run. The four failures are the tests that fail
when run as root; the three timeouts are `lost_wakeup`, `offset_range`
and `buffered_reads`, the load timeouts of
[release-1.2.0.md](release-1.2.0.md).

## x86_64 sweep

[release-1.0.md (c)](release-1.0.md#c-x86_64-sweep): push the RC to
`sweep/rc-1.2.1`. The `oss` matrix runs the `sweep = true` rows of
`scripts/oss/projects.toml`.

| | result |
|---|---|
| libc++ (x86_64) | run 43595523994 on `a589008e8`, all 30 jobs green: 8749 pass, 4 fail (the same four root-filesystem tests), 45 xfail, 2 xpass, 446 unsupported, 1 timeout (`offset_range`); 99.4%. One test has a different outcome from the 1.2.0 sweep: `lost_wakeup`, a known load timeout, passed |
| x86_64-only failures | none |
| OSS projects | all ten at parity with the clang 19 control, with no failure cluster and the translation-unit and test counts of 1.2.0: fmt, nlohmann_json, abseil, llvm_project (1504/1504 TUs), duckdb (4618/4628 tests under both), daw_json_link, opencv, cmake_app (721/723 under both), bitcoin_core, grpc (5732/5732 TUs, 117/118 tests under both) |

## Open-source projects (aarch64)

[release-1.0.md (d)](release-1.0.md#d-open-source-projects-aarch64):
every row of `scripts/oss/projects.toml` with the release `kiln++`,
against the results of [release-1.2.0.md](release-1.2.0.md).

| | result |
|---|---|
| projects at parity with 1.2.0 | 45 of the 46 rows of the 1.2.0 run, the six C rows included: the same number of tests passed and of translation units compiled |
| one difference | glaze 99/102 tests (1.2.0: 100/102). Not a compiler difference; see below |
| regressions | none |
| not re-run | the ten `sweep = true` rows ran on x86_64 instead (above); `llvm_full`, `llvm_clang` and `llvm_selfhost` keep their 1.0.0 results, as at 1.2.0 |

The one difference: in glaze one more test failed than at 1.2.0,
`json_test`, on two of its 62411 assertions ("read_directory reads each
file unpadded": one of 40 rounds of remove, create, write and read back
a directory). Two controls followed. Built and run again under ctest,
the same test failed the same way; run directly, its binary passed ten
times of ten. In a third ctest run `json_test` passed and its sibling
`json_test_non_null`, built from the same source, failed the same
assertion instead. The source of the two tests was then compiled with
the 1.2.0 release and with the release candidate, with the build's exact
flags: the object files have the same size and differ in one byte, the
digit of the version string in `.comment`, and are identical with that
section removed. The compiler writes the same code; the failure is the
test's directory round trip racing on this host's shared filesystem under
load. The count is reported as measured.

Two rows ran more than once: glaze, for the controls above, and spdlog,
whose one test failed in the first run opening its log directory (a
filesystem error on the shared volume), and passed in a second build
and in ten direct runs of ten.

The known failures that did not change: glog 22/24 (`logging` and
`symbolize`, the project's own test race on this host,
[release-1.1.0.md](release-1.1.0.md)), glaze's `cbor_test` and
`msgpack_test` (the known build failures, [oss.md](oss.md)), range_v3
254/255 and 256/257 translation units, libunifex 91/94 (the three
`io_uring` tests), folly 725/726 translation units, folly_coro 616/618
(the same two tests), boost_heavy 602/602 tests and 1370/1374
translation units, boost_meta 2082/2087 and 2235/2253, boost_compiled
1326/1330 and 1904/1923. rocksdb passed 2158/2158, as at 1.2.0.

## Downstream build

A large reflection-heavy downstream codebase was built and tested with
the RC, with its precompiled prefix header built and used the default
way. It is an aarch64 build, so it does not run the changed code; it
shows that nothing else moved.

| | result |
|---|---|
| verdict on `a589008e8` | pass: diagnostics lane 945/945 twice; 944 of 944 consumers restored the snapshot, none replayed, no `-Wpch-snapshot-unused`; the prefix's PCH built in one pass in 4.7 s (98.7 MB, the byte count of 1.2.0; 284 MiB peak) |
| time | the lane took 696 s and 680 s (1.2.0: 702 s and 683 s) |
| not re-run | the replay-fallback pass and the differing-options probe of [release-1.2.0.md](release-1.2.0.md): the precompiled-header code is that of 1.2.0 |
