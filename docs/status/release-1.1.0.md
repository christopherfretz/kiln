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

**Status: gate running. No results yet.**
