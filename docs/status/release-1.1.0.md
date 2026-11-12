# kiln 1.1.0 release measurements

The release gate for 1.1.0, the release that adds C mode
([../../CHANGELOG.md](../../CHANGELOG.md)), cut from `main` onto the
`release/1.1` branch. Every release runs the same gate before its tag:
the release archive, the full libc++ suite on aarch64 and x86_64, the
full open-source corpus, and the reference workload. The commands are
those of [release-1.0.md](release-1.0.md), with `1.0.0` replaced by
`1.1.0`; the 1.0.2 results in [release-1.0.2.md](release-1.0.2.md) are
the baseline. A check passes when it shows no regression against that
baseline. For C mode the gate adds the six C rows of
`scripts/oss/projects.toml` and gcc.c-torture at gnu23 and gnu17
([c-mode.md](c-mode.md)).

- Release candidate: `1d3f41331`, version `1.1.0`. The tag goes on the
  commit that adds these results, which changes only docs.
- A first candidate, `ec6413a0a`, failed this gate and was not tagged:
  gcc.c-torture at gnu23 passed 6910 tests where `main` had passed 6912
  before the cut ("C mode" below). The candidate passed every other
  check that had finished when it was withdrawn. The final candidate is
  the first plus two commits, the fix and a docs-only change, and every
  check on this page was run again on `1d3f41331`.
- Host: aarch64 Linux unless a section says otherwise. Each run records
  its load average, since the host is shared; this gate ran at load 16
  to 26.

**Status: complete on `1d3f41331`. Every check passed.**

## RC gate: the release archive

[release-1.0.md, "RC gate"](release-1.0.md#rc-gate-the-release-archive):
`cargo xtask dist --check`, `SHA256SUMS`, `kiln++ --version` from the
unpacked archive, and the three programs. Every later section uses that
`kiln++`.

| step | result |
|---|---|
| `dist --check` | passed: `clang-scan-deps-22 -format=p1689`, `<meta>` with `-freflection`, no build-host path, `.gnu_debuglink` matches the symbols archive |
| `sha256sum -c` | both archives OK |
| `--version` | `kiln version 1.1.0`, `aarch64-unknown-linux-gnu`, LLVM backend 22 |
| hello, `<meta>`, constexpr TUs | `hello`, `import std 3`, `2 P` |

The merge gate ran on the branch by hand, both halves
([../VALIDATION.md](../VALIDATION.md) "Patch releases"): `cargo xtask ci`
with 6759 corpus tests passed and none failed, and the clang 19 exact
lane with 6631 passed and none failed.

## C mode: gcc.c-torture

`cargo xtask gcc-torture` ([../VALIDATION.md](../VALIDATION.md)) at
`-std=gnu23` and at `-std=gnu17`, each at `-O0` and `-O2`, with GCC 16.1
and clang 22 as the two oracles. C mode is new in this release, so the
baseline is what `main` recorded in [c-mode.md](c-mode.md) before the
cut. [c-mode.md](c-mode.md) is regenerated from the gnu23 run on
`1d3f41331`.

| | kiln | gcc 16.1 | clang 22 | of |
|---|---:|---:|---:|---:|
| gnu23, `1d3f41331` | 6912 | 7572 | 6887 | 7572 |
| gnu23, first candidate `ec6413a0a` | 6910 | 7572 | 6887 | 7572 |
| gnu17, `1d3f41331` | 7072 | 7570 | 7057 | 7572 |
| gnu17, first candidate `ec6413a0a` | 7070 | 7570 | 7057 | 7572 |

On `1d3f41331`, at both standards, no test fails with kiln that both
oracles pass, and the gnu23 count is the 6912 that `main` had before the
cut.

What the first candidate failed: `compile/20000701-1.c`, at `-O0` and at
`-O2`, at both standards. It has a conditional expression whose arms are
`void`-typed lvalues (`c ? *p : *q` with `void *` operands, a GNU C
form). A change made for 1.1.0 loaded each arm of a C conditional as a
value, and a `void` lvalue has none to load. With the fix (tests
`tests/c/expr/void-deref-conditional-ir.c` and
`tests/c/run/void-deref-conditional-arms.c`) such an arm is evaluated
and never loaded, and the rows pass again. The fix landed on `main`
first and was cherry-picked.

## libc++ 22.1.8 test suite (aarch64)

[release-1.0.md (b)](release-1.0.md#b-libc-2218-test-suite-aarch64),
the whole suite (`std libcxx`) at `-j 3`.

| | total | pass | fail | xfail | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1.1.0 RC (`1d3f41331`) | 9247 | 8747 | 4 (+1 xpass) | 47 | 446 | 2 | 99.4% |
| 1.0.2 (`3c64e4e3d`) | 9247 | 8747 | 4 (+1 xpass) | 47 | 446 | 2 | 99.4% |

Run 08:04 to 09:42 UTC on 11-12, load 16 to 24: `std/` 99.5%, `libcxx/`
97.9%. Every test has the outcome it had at 1.0.2 and on the first
candidate (the per-test results of the two candidates are identical).
The four failures are the tests that fail when run as root, and the two
timeouts are `lost_wakeup` and `offset_range`, the known load timeouts
([../KNOWN_ISSUES.md](../KNOWN_ISSUES.md)).

## x86_64 sweep

[release-1.0.md (c)](release-1.0.md#c-x86_64-sweep): push the RC to
`sweep/rc-1.1.0`. The `oss` matrix runs the `sweep = true` rows of
`scripts/oss/projects.toml`.

| | result |
|---|---|
| libc++ (x86_64) | run 43174666660 on `1d3f41331`, all 30 jobs green: 8748 pass, 4 fail (the same four root-filesystem tests), 46 xfail, 1 xpass, 446 unsupported, 2 timeouts (`lost_wakeup` and `offset_range`); 99.4%. One pass fewer than 1.0.2, where `offset_range` did not time out |
| x86_64-only failures | none: both timeouts are the known load timeouts of [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md), and both timed out on aarch64 too |
| OSS projects | all ten at parity with the clang 19 control, with no failure cluster: fmt, nlohmann_json, abseil, llvm_project (1504/1504 TUs), duckdb (4618/4628 tests under both), daw_json_link, opencv, cmake_app (721/723 under both), bitcoin_core, grpc (5732/5732 TUs, 117/118 tests under both) |
| first candidate | run 43157977757 on `ec6413a0a`, all 30 jobs green, with the same libc++ counts and the same translation-unit and test counts for every project |

## Open-source projects (aarch64)

[release-1.0.md (d)](release-1.0.md#d-open-source-projects-aarch64):
every row of `scripts/oss/projects.toml` with the release `kiln++`,
against the results of [release-1.0.2.md](release-1.0.2.md), plus the
six C rows, which are new in this release.

| | result |
|---|---|
| projects at parity with 1.0.2 | all 40 rows of the 1.0.2 run: the same number of tests passed and of translation units compiled, except the three differences below, none of them a regression |
| C rows (new) | all six build and pass: c_demo 3/3 TUs, zlib 6/6 tests and 37/37 TUs, sqlite 2/2 TUs and its workload, lua 34/34 TUs and its test run, c_ares 2/2 tests and 126/126 TUs, zstd 3/3 tests and 118/118 TUs |
| regressions | none |
| not re-run | the ten `sweep = true` rows ran on x86_64 instead (above); `llvm_full`, `llvm_clang` and `llvm_selfhost` keep their 1.0.0 results, as at 1.0.2 |

The three differences from the 1.0.2 run:

- glog 22/24 (1.0.2: 23/24): `logging` fails as well as `symbolize`.
  It is the project's own test race on this host, as at 1.0.0
  ([release-1.0.md](release-1.0.md), 22/24 under kiln and under clang).
  Checked with controls at the time of this run: glog built with kiln
  1.0.2 and with clang 19 also scored 22/24, and `logging` run alone
  failed 18 of 150 times in the clang 19 build and 8 of 100 times in
  the RC build, in the same two ways (ctest's 30 s limit, and an abort
  with `std::system_error`).
- reflect_cpp 484/484 tests, 327/327 translation units (1.0.2:
  326/326): one more translation unit was compiled and counted.
- rocksdb 2157/2158, as at 1.0.2: one segfault, this time in
  `TimestampCompatibleCompactionTest.CompactFilesRangeCheckL1`; at 1.0.2
  it was `DBMemTableTest.IntegrityChecks`.
  [release-1.0.1.md](release-1.0.1.md) records the same behaviour with a
  clang 19 build of the project.

Two rows needed a second run for reasons outside the compiler, and both
runs are recorded here:

- spdlog 1/1 on the second run; the first failed one assertion of 578,
  `count_files("test_logs")` throwing "Failed open folder" (an `opendir`
  failure on the shared filesystem). The same failure appears with every
  compiler on this host: in 30 runs of the test binary each, the kiln
  1.0.2 build failed 8 times, the clang 19 build 4 times and the RC
  build 3 times.
- tl_expected 1/1 tests and 13/13 translation units on the second run;
  on the first its configure step stalled in a download of a test
  dependency and was stopped.
- boost_heavy 602/602 tests and 1370/1374 translation units, boost_meta
  2082/2087 and 2235/2253, boost_compiled 1326/1330 and 1904/1923: the
  counts of 1.0.2, with the same tests failing. They ran on a third
  attempt: twice the clone of boost stopped inside `git` (a submodule's
  objects could not be unpacked on the shared filesystem) before any
  compiler ran.

The known failures that did not change: glaze 100/102 (`cbor_test` and
`msgpack_test`, the known build failures, [oss.md](oss.md)), range_v3
254/255 and 256/257 translation units, libunifex 91/94 (the three
`io_uring` tests), folly 725/726 translation units, folly_coro 616/618
(the same two tests as at 1.0.2).

## Reference workload

The reference workload's maintainers build and test the RC: its cold
build, its unit tests, and its diagnostics lane.

| | result |
|---|---|
| verdict on `1d3f41331` | pass: cold build 217/217 in 333 s at 3.26 GiB peak (1.0.2: 316 s, 3.24 GiB; on this shared host the first candidate's build took 357 s beside another build), unit tests 2104 and harness 159 all pass, diagnostics lane 945/945, the same three build warnings as 1.0.2 |
| first candidate `ec6413a0a` | pass on the same lane, and on the workload's second, larger lane (cold build, 1843 test runs with no new failure, diagnostics 208/208). The second lane was not repeated on `1d3f41331`: the two candidates differ in one C-only code path and a docs change |
