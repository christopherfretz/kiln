# kiln 1.0.1 release measurements

The release gate for 1.0.1, a bug-fix release
([../../CHANGELOG.md](../../CHANGELOG.md)). Every release runs the same
gate before its tag: the release archive, the full libc++ suite on
aarch64 and x86_64, the full open-source corpus, and a downstream
reflection-heavy build. The commands are those of [release-1.0.md](release-1.0.md),
with `1.0.0` replaced by `1.0.1`; the 1.0.0 results there are the
baseline. A check passes when it shows no regression against that
baseline.

- Release candidate: `2fbf4f500`, version `1.0.1`. The tag goes on the
  commit that merges this page, which changes only docs.
- Host: aarch64 Linux unless a section says otherwise. Each run records
  its load average, since the host is shared.

**Status: complete on `2fbf4f500`. Every check passed.**

## RC gate: the release archive

[release-1.0.md, "RC gate"](release-1.0.md#rc-gate-the-release-archive):
`cargo xtask dist --check`, `SHA256SUMS`, `kiln++ --version` from the
unpacked archive, and the three programs. Every later section uses that
`kiln++`.

| step | result |
|---|---|
| `dist --check` | passed: `clang-scan-deps-22 -format=p1689`, `<meta>` with `-freflection`, no build-host path, `.gnu_debuglink` matches the symbols archive |
| `sha256sum -c` | both archives OK |
| `--version` | `kiln version 1.0.1`, `aarch64-unknown-linux-gnu`, LLVM backend 22 |
| hello, `<meta>`, constexpr TUs | `hello`, `import std 3`, `2 P` |

## libc++ 22.1.8 test suite (aarch64)

[release-1.0.md (b)](release-1.0.md#b-libc-2218-test-suite-aarch64),
the whole suite (`std libcxx`) at `-j 3`.

| | total | pass | fail | xfail | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1.0.1 RC (`2fbf4f500`) | 9247 | 8748 | 4 (+1 xpass) | 47 | 446 | 1 | 99.4% |
| 1.0.0 (`ae54fd2fa`) | 9247 | 8747 | 4 (+1 xpass) | 47 | 446 | 2 | 99.4% |

Run 01:07 to 01:41 UTC on 11-11, load 7 to 9: `std/` 99.5%, `libcxx/`
97.9%. The four failures are the same four as 1.0.0, tests that fail
when run as root ([../KNOWN_ISSUES.md](../KNOWN_ISSUES.md)): the `directory_iterator` and `recursive_directory_iterator`
constructors, `remove` and `remove_all`. The xpass is the same,
`variant.hash/hash.depr.verify.cpp`. One timeout,
`ifstream.members/offset_range.pass.cpp`, is one of 1.0.0's two;
`lost_wakeup` passed this time.

## x86_64 sweep

[release-1.0.md (c)](release-1.0.md#c-x86_64-sweep): push the RC to
`sweep/rc-1.0.1`. The `oss` matrix runs the `sweep = true` rows of
`scripts/oss/projects.toml` (WP-1265).

| | result |
|---|---|
| libc++ (x86_64) | run 42940030601, 30/30 jobs green: 8748 pass, 4 fail (the same four root-filesystem tests), 46 xfail, 1 xpass, 446 unsupported, 2 timeouts (`lost_wakeup`, `offset_range`); 99.4% |
| x86_64-only failures | `lost_wakeup` only, the known load timeout of [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md) |
| OSS projects | all ten at parity with the clang 19 control: fmt, nlohmann_json, abseil, llvm_project (1504/1504 TUs), duckdb, daw_json_link, opencv, cmake_app, bitcoin_core, grpc |

## Open-source projects (aarch64)

[release-1.0.md (d)](release-1.0.md#d-open-source-projects-aarch64):
every row of `scripts/oss/projects.toml` with the release `kiln++`,
against the baselines of [oss.md](oss.md). Rows not re-run keep their
results and are listed here.

| | result |
|---|---|
| projects at parity with 1.0.0 | all 40 rows of the 1.0.0 final RC's run. Three improved: folly 3773/3773 tests (was 3772), boost_compiled 1326/1330 (was 1325), glog 23/24 (was 22/24) |
| regressions | none (the flakes below are not kiln's) |
| not re-run | the ten `sweep = true` rows ran on x86_64 instead (above); `llvm_full`, `llvm_clang` and `llvm_selfhost` keep their 1.0.0 results, as at 1.0.0 |

Flakes, each checked against a clang 19 build of the same project:

- eigen 538/539: `matrix_function_1` fails on a random seed, as at
  1.0.0; the clang build fails with the same seeds.
- glaze 99/102: `cbor_test` and `msgpack_test` are the known build
  failures ([oss.md](oss.md)). `threading_test` reached ctest's 300 s
  timeout twice at load 12 to 17. Run alone at the same load, the kiln
  binary passes in 50 s (246 s of CPU) and the clang binary in 173 s
  (857 s of CPU).
- rocksdb: one segfault in the first run
  (`IndexBlockKVChecksumTest`, `BinarySearchProtBytes1`) and two in the
  re-run (`DBMergeOperandTest.GetMergeOperandsShortCircuitBaseValue`,
  `TimestampCompatibleCompactionTest.CompactFilesRangeCheckL1`). Three
  more runs of the whole selection, at load 12 to 14, passed 2158/2158
  with both compilers. Each of the three tests also passed 200 of 200
  runs (eight copies at a time) with both compilers.


## Downstream build

The maintainers of a large reflection-heavy codebase build and test the RC: its cold
build, its unit tests, and its diagnostics lane.

| | result |
|---|---|
| verdict on `2fbf4f500` | pass: cold build 217/217 in 338 s at 3.244 GiB peak, unit tests 2104 and harness 142 all pass, diagnostics lane 945/945 |
