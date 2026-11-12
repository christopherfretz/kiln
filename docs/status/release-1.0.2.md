# kiln 1.0.2 release measurements

The release gate for 1.0.2, a bug-fix release
([../../CHANGELOG.md](../../CHANGELOG.md)): thirteen fixes on top of
1.0.1, cut from the `release/1.0` maintenance branch
([../VALIDATION.md](../VALIDATION.md) "Patch releases"). Every release runs the same gate
before its tag: the release archive, the full libc++ suite on aarch64
and x86_64, the full open-source corpus, and the reference workload. The
commands are those of [release-1.0.md](release-1.0.md), with `1.0.0`
replaced by `1.0.2`; the 1.0.1 results in
[release-1.0.1.md](release-1.0.1.md) are the baseline. A check passes
when it shows no regression against that baseline.

- Release candidate: `3c64e4e3d`, version `1.0.2`. The tag goes on the
  commit that adds these results, which changes only docs.
- A first candidate, `6a209a14a`, failed this gate and was not tagged:
  the open-source corpus found one translation unit that 1.0.1 compiled
  and the candidate rejected ("Open-source projects" below). The fix is
  the only difference between the two candidates, and every check on
  this page was run again on `3c64e4e3d`.
- Host: aarch64 Linux unless a section says otherwise. Each run records
  its load average, since the host is shared.

**Status: complete on `3c64e4e3d`. Every check passed.**

## RC gate: the release archive

[release-1.0.md, "RC gate"](release-1.0.md#rc-gate-the-release-archive):
`cargo xtask dist --check`, `SHA256SUMS`, `kiln++ --version` from the
unpacked archive, and the three programs. Every later section uses that
`kiln++`.

| step | result |
|---|---|
| `dist --check` | passed: `clang-scan-deps-22 -format=p1689`, `<meta>` with `-freflection`, no build-host path, `.gnu_debuglink` matches the symbols archive |
| `sha256sum -c` | both archives OK |
| `--version` | `kiln version 1.0.2`, `aarch64-unknown-linux-gnu`, LLVM backend 22 |
| hello, `<meta>`, constexpr TUs | `hello`, `import std 3`, `2 P` |

The merge gate ran on the branch by hand, both halves
([../VALIDATION.md](../VALIDATION.md) "Patch releases"): `cargo xtask ci`
with 6422 corpus tests passed and none failed, and the clang 19 exact
lane with 6297 passed and none failed.

## libc++ 22.1.8 test suite (aarch64)

[release-1.0.md (b)](release-1.0.md#b-libc-2218-test-suite-aarch64),
the whole suite (`std libcxx`) at `-j 3`.

| | total | pass | fail | xfail | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1.0.2 RC (`3c64e4e3d`) | 9247 | 8747 | 4 (+1 xpass) | 47 | 446 | 2 | 99.4% |
| 1.0.1 (`2fbf4f500`) | 9247 | 8748 | 4 (+1 xpass) | 47 | 446 | 1 | 99.4% |

Run 22:44 to 23:27 UTC on 11-11, load 9 to 11: `std/` 99.5%, `libcxx/`
97.9%. The four failures are the same four as 1.0.1, tests that fail
when run as root ([../KNOWN_ISSUES.md](../KNOWN_ISSUES.md)). The one
difference from 1.0.1 is a second timeout, `lost_wakeup`, the known load
timeout of [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md): it timed out at
1.0.0, passed at 1.0.1 and on the first candidate, and passed three of
three runs alone with this candidate at load 8 to 10. Every other test
has the outcome it had on the first candidate, whose counts were those
of 1.0.1.

## x86_64 sweep

[release-1.0.md (c)](release-1.0.md#c-x86_64-sweep): push the RC to
`sweep/rc-1.0.2`. The `oss` matrix runs the `sweep = true` rows of
`scripts/oss/projects.toml`.

| | result |
|---|---|
| libc++ (x86_64) | run 43113903235, the build and all six shards green: 8749 pass, 4 fail (the same four root-filesystem tests), 46 xfail, 1 xpass, 446 unsupported, 1 timeout (`lost_wakeup`); 99.4%. One pass more than 1.0.1: `offset_range` did not time out |
| x86_64-only failures | none: `lost_wakeup`, the known load timeout of [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md), timed out on both architectures |
| OSS projects | all ten at parity with the clang 19 control: fmt, nlohmann_json, abseil, llvm_project (1504/1504 TUs), duckdb, daw_json_link, opencv, cmake_app, bitcoin_core, grpc (5732/5732 TUs, 117/118 tests; `end2end_http2_test` reaches the 300 s limit, as it did with both compilers at 1.0.1). Of the run's 29 jobs, 28 were green when this page was written; the last, the clang control build of grpc, was still building, so grpc is compared with the 1.0.1 control |

## Open-source projects (aarch64)

[release-1.0.md (d)](release-1.0.md#d-open-source-projects-aarch64):
every row of `scripts/oss/projects.toml` with the release `kiln++`,
against the results of [release-1.0.1.md](release-1.0.1.md).

| | result |
|---|---|
| projects at parity with 1.0.1 | all 40 rows of the 1.0.1 run. Two improved, both flakes of 1.0.1 that passed this time: eigen 539/539 tests (was 538), glaze 100/102 (was 99) |
| regressions | none (the flake below is not kiln's) |
| not re-run | the ten `sweep = true` rows ran on x86_64 instead (above); `llvm_full`, `llvm_clang` and `llvm_selfhost` keep their 1.0.0 results, as at 1.0.1 |

What the first candidate failed: boost_heavy built 1369 of 1374
translation units and passed 601 of 602 tests, where 1.0.1 had 1370 and
602. Boost.Cobalt's `test/experimental/context.cpp` was rejected with
"an atomic constraint that stays value-dependent after substitution": a
requires-clause that calls a function with a
`std::source_location::current()` default argument, after the change
that checks a function's constraints from the function itself. With the
fix (tests `temp.constr.atomic/p3-srcloc-default-arg-*`) boost_heavy is
back at 1370 and 602/602, and no other row changed between the two
candidates except the flakes named here.

- rocksdb 2157/2158: one segfault, in `DBMemTableTest.IntegrityChecks`;
  on the first candidate it was a different test
  (`DBSecondaryCacheTest.NoSecondaryCacheInsertion`), and at 1.0.1 two
  others. [release-1.0.1.md](release-1.0.1.md) records the same
  behaviour with a clang 19 build of the project.
- glaze 100/102: `cbor_test` and `msgpack_test` are the known build
  failures ([oss.md](oss.md)).

## Reference workload

The reference workload's maintainers build and test the RC: its cold
build, its unit tests, and its diagnostics lane.

| | result |
|---|---|
| verdict on `3c64e4e3d` | pass: cold build 217/217 in 316 s at 3.24 GiB peak, unit tests 2104 and harness 159 all pass, diagnostics lane 945/945 |
