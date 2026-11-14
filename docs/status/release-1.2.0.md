# kiln 1.2.0 release measurements

The release gate for 1.2.0, the release that makes a precompiled header
a snapshot the consumer restores ([../../CHANGELOG.md](../../CHANGELOG.md)),
cut from `main` onto the `release/1.2` branch. Every release runs the
same gate before its tag: the release archive, the full libc++ suite on
aarch64 and x86_64, the full open-source corpus, gcc.c-torture, and a
downstream reflection-heavy build. The commands are those of
[release-1.0.md](release-1.0.md), with `1.0.0` replaced by `1.2.0`; the
1.1.0 results in [release-1.1.0.md](release-1.1.0.md) are the baseline.
A check passes when it shows no regression against that baseline. For
precompiled headers the gate adds `scripts/pch-release-gate.sh`: the
whole test corpus and the libc++ suite, each compiled through a
snapshot and through the replay fallback, with the two compared
([pch-snapshot-gates.md](pch-snapshot-gates.md)).

- Release candidate: `2c2e2d1bb`, version `1.2.0`. The tag goes on the
  commit that adds these results, which changes only docs.
- Host: aarch64 Linux unless a section says otherwise. The host is
  shared and was busier than at 1.1.0: this gate ran at load 25 to 44,
  beside other builds.
- One defect found during the gate is not fixed in this release and is
  listed in [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md): on x86-64 with AVX
  enabled, 256-bit vectors passed or returned by value use a different
  calling convention from clang and GCC. Every earlier release has it;
  no check on this page regressed because of it. The fix is on `main`.

**Status: complete on `2c2e2d1bb`. Every check passed.**

## RC gate: the release archive

[release-1.0.md, "RC gate"](release-1.0.md#rc-gate-the-release-archive):
`cargo xtask dist --check`, `SHA256SUMS`, `kiln++ --version` from the
unpacked archive, and the three programs. Every later section uses that
`kiln++`.

| step | result |
|---|---|
| `dist --check` | passed, including `.gnu_debuglink` matching the symbols archive |
| `sha256sum -c` | both archives OK |
| `--version` | `kiln version 1.2.0`, `aarch64-unknown-linux-gnu`, LLVM backend 22 |
| hello, `<meta>`, constexpr TUs | `hello`, `import std 3`, `2 P` |

The merge gate ran on the branch by hand, both halves
([../VALIDATION.md](../VALIDATION.md) "Patch releases"): `cargo xtask ci`
with 6938 corpus tests passed and none failed, and the clang 19 exact lane with 6809 corpus tests passed and
none failed (147 of 151 passed and none failed in each of the two
module-interface lanes).

## Precompiled headers: the release sweep

`scripts/pch-release-gate.sh <kiln++> <dir> all` with the release
`kiln++` ([pch-snapshot-gates.md](pch-snapshot-gates.md)). Producer and
consumer use no snapshot option: the default build and the default
`-include-pch`. The script's verdict: pass.

Whole corpus (7087 tests, 2920 s):

| pass | fail | skip | restored | replayed | failed before the consumer |
|---|---|---|---|---|---|
| 5720 | 0 | 1367 | 5719 | 1 (allowed: the module unit) | 0 |

| producers | single pass | two passes |
|---|---|---|
| 1422 | 1412 | 10 |

libc++ pair (4299 tests, 3894 of them compiled with a PCH):

| run | pass | fail | xfail | unsupported | wall |
|---|---|---|---|---|---|
| no PCH | 4261 | 1 | 1 | 36 | 1165 s |
| snapshot, default path | 4261 | 1 | 1 | 36 | 1456 s |

Differences between the two runs: none, test by test. The one failure
is `unique.ptr.special/io.verify.cpp` in both, as in
[pch-snapshot-gates.md](pch-snapshot-gates.md).

## C mode: gcc.c-torture

`cargo xtask gcc-torture` ([../VALIDATION.md](../VALIDATION.md)) at
`-std=gnu23` and at `-std=gnu17`, each at `-O0` and `-O2`, with GCC 16.1
and clang 22 as the two oracles.

| | kiln | gcc 16.1 | clang 22 | of |
|---|---:|---:|---:|---:|
| gnu23, `2c2e2d1bb` | 6912 | 7572 | 6887 | 7572 |
| gnu23, 1.1.0 | 6912 | 7572 | 6887 | 7572 |
| gnu17, `2c2e2d1bb` | 7072 | 7570 | 7057 | 7572 |
| gnu17, 1.1.0 | 7072 | 7570 | 7057 | 7572 |

The counts of 1.1.0 at both standards.

## libc++ 22.1.8 test suite (aarch64)

[release-1.0.md (b)](release-1.0.md#b-libc-2218-test-suite-aarch64),
the whole suite (`std libcxx`) at `-j 3`.

| | total | pass | fail | xfail | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1.2.0 RC (`2c2e2d1bb`) | 9247 | 8746 | 4 (+2 xpass) | 46 | 446 | 3 | 99.4% |
| 1.1.0 (`1d3f41331`) | 9247 | 8747 | 4 (+1 xpass) | 47 | 446 | 2 | 99.4% |

Run 05:39 to 08:22 UTC on 11-14, load 25 to 44: `std/` 99.5%, `libcxx/`
97.9%. Two tests have a different outcome from 1.1.0:

- `libcxx/depr/depr.rel_ops/rel_ops.depr_in_cxx20.verify.cpp` passes
  where it was expected to fail (the deprecation warning it checks for
  is now given).
- `std/.../ifstream.members/buffered_reads.pass.cpp` timed out (24.6 s
  against the 20 s limit). It reads large files and sits beside
  `offset_range`, one of the two known load timeouts
  ([../KNOWN_ISSUES.md](../KNOWN_ISSUES.md)). Checked with controls
  after the run, on the same loaded host, alone, with a 60 s limit,
  three times each: it passes with the RC in 50 to 52 s and with 1.1.0
  in 47 to 59 s. It passed in the x86_64 sweep below. It is a load
  timeout, not a change in the compiler.

The four failures are the tests that fail when run as root; the other
two timeouts are `lost_wakeup` and `offset_range`.

## x86_64 sweep

[release-1.0.md (c)](release-1.0.md#c-x86_64-sweep): push the RC to
`sweep/rc-1.2.0`. The `oss` matrix runs the `sweep = true` rows of
`scripts/oss/projects.toml`.

| | result |
|---|---|
| libc++ (x86_64) | run 43551187823 on `2c2e2d1bb`, all 30 jobs green: 8748 pass, 4 fail (the same four root-filesystem tests), 45 xfail, 2 xpass, 446 unsupported, 2 timeouts (`lost_wakeup` and `offset_range`); 99.4%. The pass count of 1.1.0; the one changed outcome is the `rel_ops` test above |
| x86_64-only failures | none |
| OSS projects | all ten at parity with the clang 19 control, with no failure cluster and the translation-unit and test counts of 1.1.0: fmt, nlohmann_json, abseil, llvm_project (1504/1504 TUs), duckdb (4618/4628 tests under both), daw_json_link, opencv, cmake_app (721/723 under both), bitcoin_core, grpc (5732/5732 TUs, 117/118 tests under both) |

## Open-source projects (aarch64)

[release-1.0.md (d)](release-1.0.md#d-open-source-projects-aarch64):
every row of `scripts/oss/projects.toml` with the release `kiln++`,
against the results of [release-1.1.0.md](release-1.1.0.md).

| | result |
|---|---|
| projects at parity with 1.1.0 | all 46 rows of the 1.1.0 run, the six C rows included: the same number of tests passed and of translation units compiled, except the one difference below |
| regressions | none |
| not re-run | the ten `sweep = true` rows ran on x86_64 instead (above); `llvm_full`, `llvm_clang` and `llvm_selfhost` keep their 1.0.0 results, as at 1.1.0 |

The one difference from the 1.1.0 run: rocksdb 2158/2158 (1.1.0:
2157/2158). The one segfault that moved between tests from release to
release, and that a clang 19 build of the project shows too
([release-1.0.1.md](release-1.0.1.md)), did not occur in this run.

Every row ran once; no clone or download on the shared filesystem
needed a second attempt this time.

The known failures that did not change: glog 22/24 (`logging` and
`symbolize`, the project's own test race on this host,
[release-1.1.0.md](release-1.1.0.md)), glaze 100/102 (`cbor_test` and
`msgpack_test`, the known build failures, [oss.md](oss.md)), range_v3
254/255 and 256/257 translation units, libunifex 91/94 (the three
`io_uring` tests), folly 725/726 translation units, folly_coro 616/618,
boost_heavy 602/602 tests and 1370/1374 translation units, boost_meta
2082/2087 and 2235/2253, boost_compiled 1326/1330 and 1904/1923.

## Downstream build

A large reflection-heavy downstream codebase was built and tested with
the RC, with its precompiled prefix header built and used the default
way.

| | result |
|---|---|
| verdict on `2c2e2d1bb` | pass: diagnostics lane 945/945 twice; 944 of 944 consumers restored the snapshot, none replayed, no `-Wpch-snapshot-unused`; the prefix's PCH built in one pass in 4.4 s (98.7 MB, 284 MiB peak) |
| time | the lane took 702 s and 683 s; with 1.1.0, which had no usable precompiled header for this prefix, 1344 s on the same host |
| replay fallback | `-fno-kiln-pch-snapshot` on every consumer: 945/945, 1503 s |
| differing options | five consumers given an extra `-D`: one `-Wpch-snapshot-unused` warning naming the macro, exit 0 also under `-Werror`, replay, and an object byte-identical to the one built with no PCH; `-Werror=pch-snapshot-unused` makes it an error |
