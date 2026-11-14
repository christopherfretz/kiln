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

**Status: gate running. No results yet.**
