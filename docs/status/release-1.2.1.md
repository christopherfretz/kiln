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

- Release candidate: the commit that sets version `1.2.1`. The tag goes
  on the commit that adds these results, which changes only docs.
- Host: aarch64 Linux unless a section says otherwise.

**Status: gate in progress. Not tagged.**
