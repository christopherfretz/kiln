# kiln 1.0.1 release measurements

The release gate for 1.0.1, a bug-fix release
([../../CHANGELOG.md](../../CHANGELOG.md)). Every release runs the same
gate before its tag: the release archive, the full libc++ suite on
aarch64 and x86_64, the full open-source corpus, and the reference
workload. The commands are those of [release-1.0.md](release-1.0.md),
with `1.0.0` replaced by `1.0.1`; the 1.0.0 results there are the
baseline. A check passes when it shows no regression against that
baseline.

- Release candidate: `<rc-sha>`, version `1.0.1`, the commit that
  merges this page.
- Host: aarch64 Linux unless a section says otherwise. Each run records
  its load average, since the host is shared.

**Status: not started.**

## RC gate: the release archive

[release-1.0.md, "RC gate"](release-1.0.md#rc-gate-the-release-archive):
`cargo xtask dist --check`, `SHA256SUMS`, `kiln++ --version` from the
unpacked archive, and the three programs. Every later section uses that
`kiln++`.

| step | result |
|---|---|
| `dist --check` | |
| `sha256sum -c` | |
| `--version` | |
| hello, `<meta>`, constexpr TUs | |

## libc++ 22.1.8 test suite (aarch64)

[release-1.0.md (b)](release-1.0.md#b-libc-2218-test-suite-aarch64),
the whole suite (`std libcxx`) at `-j 3`.

| | total | pass | fail | xfail | unsupported | timeout | pass rate |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1.0.1 RC | | | | | | | |
| 1.0.0 (`ae54fd2fa`) | 9247 | 8747 | 4 (+1 xpass) | 47 | 446 | 2 | 99.4% |

## x86_64 sweep

[release-1.0.md (c)](release-1.0.md#c-x86_64-sweep): push the RC to
`sweep/rc-1.0.1`. The `oss` matrix runs the `sweep = true` rows of
`scripts/oss/projects.toml` (WP-1265).

| | result |
|---|---|
| libc++ (x86_64) | |
| x86_64-only failures | |
| OSS projects | |

## Open-source projects (aarch64)

[release-1.0.md (d)](release-1.0.md#d-open-source-projects-aarch64):
every row of `scripts/oss/projects.toml` with the release `kiln++`,
against the baselines of [oss.md](oss.md). Rows not re-run keep their
results and are listed here.

| | result |
|---|---|
| projects at parity with 1.0.0 | |
| regressions | |
| not re-run | |

## Reference workload

The reference workload's maintainers build and test the RC: its cold
build, its unit tests, and its diagnostics lane.

| | result |
|---|---|
| verdict on `<rc-sha>` | |
