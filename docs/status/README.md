# Status pages (index, 2026-11-09)

Measured status of kiln, one page per measurement. Each page says how it
was produced and how to re-run it. The milestones and their exit criteria
are in [../ROADMAP.md](../ROADMAP.md). The summary for users is
[../COMPATIBILITY.md](../COMPATIBILITY.md), and the gaps are in
[../KNOWN_ISSUES.md](../KNOWN_ISSUES.md). Language features of every
standard, C++98 to C++26, are in
[../STANDARD_SUPPORT.md](../STANDARD_SUPPORT.md), checked by
`cargo xtask features`.

## Headline numbers

Each number is from the page named, at the run that page records.

- libc++ 22.1.8 suite at `-std=c++26`: 99.4% of 9247 tests
  ([libcxx-1.0.md](libcxx-1.0.md)). libc++ 19.1.7: 98.3% of `std/`
  ([libcxx.md](libcxx.md)).
- Compile time and memory against clang 19 on the `bench/` corpus:
  geometric mean 0.21x time, 0.17x peak RSS ([perf.md](perf.md)).
- Open-source projects built with their own build systems and test
  suites: 47, of which 46 at parity with clang 19 or better
  ([../COMPATIBILITY.md](../COMPATIBILITY.md#libraries-and-applications);
  the runs are in [oss.md](oss.md) and [oss/](oss/)).
- clang 22's negative tests: kiln reports no error in 4.6% of files where
  clang expects one, with 0 crashes in 2850 cases
  ([accepts-invalid.md](accepts-invalid.md)).

## Pages

Releases:

- `release-1.0.md`: the 1.0 release candidate's measurements, one
  section per measurement with the command that fills each row, and the
  release-archive gate (WP-1202).
- `release-1.0.1.md`: the 1.0.1 release gate, run with the commands of
  `release-1.0.md` against its 1.0.0 results.
- `release-1.0.2.md`: the 1.0.2 release gate, run with the same commands
  against the 1.0.1 results.
- `release-1.1.0.md`: the 1.1.0 release gate (C mode): the same commands
  against the 1.0.2 results, plus the C projects and gcc.c-torture.
- `release-1.2.0.md`: the 1.2.0 release gate (precompiled header
  snapshots): the same commands against the 1.1.0 results, plus the
  snapshot-against-replay sweeps.
- `release-1.2.1.md`: the 1.2.1 release gate (the x86-64 AVX calling
  convention fix): the same commands against the 1.2.0 results.

Standard libraries:

- `libcxx.md`: libc++ 19 end to end (ROADMAP M3b), sample and full-suite
  pass rates, failure clusters, and the run rows of every libc++ run.
- `libcxx-1.0.md`: libc++ 22 full suite on the 1.0 release candidate.
- `libcxx-m5.md`: libc++ 22 full suite (ROADMAP M5), failure clusters by
  class (correctness, diagnostic wording, harness, environment), fixes.
- `libcxx-m3.md`: the frozen M3 gate subset. `libcxx-longtail.md`: the
  WP-276 long tail of the libc++ 19 sample. `libcxx-syntax.md`: first
  blockers per libc++ header under `-fsyntax-only` (historical, M3).
- `libstdcxx.md`: every public GCC 14 libstdc++ header as a one-line TU
  (`cargo xtask libstdcxx`).

Open-source projects and platforms:

- `oss.md` and `oss/`: ROADMAP M8 builds, one page per project.
- `x86_64-sweep.md`: the WP-961 x86_64 sweep on GitHub's runners
  (`.github/workflows/sweep.yml`): libc++ 22 and ten projects against the
  clang 19 control, with x86_64-only triage.

C mode:

- `c-mode.md`: `kiln` on C code (WP-1282): the C OSS rows against clang 19,
  gcc.c-torture at gnu23 and gnu17 (`cargo xtask gcc-torture --report`),
  and the failure clusters ranked for wave 2.

Performance:

- `perf.md`: `cargo xtask bench` against clang 19 (generated; do not edit
  by hand), and the 1.0 release measurements.
- `perf-history.md`: the per-work-package measurements made by hand
  before 1.0 (WP-313 to WP-1171), moved verbatim from `perf.md`.
- `perf-reflection.md`: a large reflection-heavy codebase under
  kiln, and the perf work packages that followed (WP-551).
- `header-search-syscalls.md`: file-system system calls per TU against
  clang 19 (WP-820).
- `bench-regressions-WP-798.md`: triage of one bench regression report.
- `upstream/`: drafts of LLVM issue reports found while comparing with
  clang.

Correctness and diagnostics:

- `pch.md`: the PCH's current state (kiln 1.2.0): the snapshot consumer
  against replay, no PCH and clang 19's PCH, and where it stands
  (WP-1404, WP-1397; the retired declaration image's measurements are in
  its history).
- `pch-image-census.md`: how much of every libc++ header the module
  declaration image carries, by first fallback reason
  (`-fkiln-bmi-image-census`, WP-1070).
- `pch-snapshot-gates.md`: the snapshot PCH's gates: restore/replay
  accounting over the corpus (with the synthetic `<bits/stdc++.h>`
  prefix) and libc++, the refusal reasons, the fixes they found, the CI
  lane (WP-1395).
- `pp-parity.md`: `kiln -E` against `clang -E` on the preprocessor corpus
  (ROADMAP M1).
- `accepts-invalid.md`: clang 22's `clang/test/{CXX,SemaCXX}` negative
  tests through kiln (`cargo xtask clang-neg`, WP-912), with triage
  clusters and repros.
- `floating-point.md`: the FP-environment pragmas kiln rejects, and the
  analysis and size estimate of `#pragma STDC FENV_ACCESS ON` (WP-1399).
- `silent-drops.md`: the WP-805 audit of constructs kiln ignored instead
  of implementing or rejecting.
- `deviations.md`: deliberate deviations from the oracle where kiln
  follows the standard over a clang defect, each with its spec paragraph
  and pinning test.
- `debuginfo.md`: `-g` against clang 19 (WP-814).

Design records:

- `reflection-spike.md`: the WP-267 P2996 design spike.
- `wp-184-oracle-switch.md`: the clang/LLVM 19 to 22 oracle switch.
- `cxx-features.md`: moved to `../STANDARD_SUPPORT.md`.
- `wp-NNN-progress.md`: progress logs of early work packages (M2 and
  M3), kept as history. They do not describe the current state.
