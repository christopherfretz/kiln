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

- Host: aarch64 Linux unless a section says otherwise. Each run records
  its load average, since the host is shared.

**Status: in progress.**
