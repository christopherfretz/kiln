#!/usr/bin/env python3
"""odr_corpus.py: build the must-equal / must-differ corpus of kiln's
`.mason.meta` kind 2 rows (mortar's O-13) and write its manifest.

Usage: python3 -I scripts/mason/odr_corpus.py --kiln PATH --out DIR
           [--corpus DIR] [--level LEVEL] [--check] [CASE...]

Each directory of the corpus (default tests/mason/corpus) is one case: its
units and headers, and `case.tsv`, one tab-separated line:

    expect  symbols  kinds  level  reason  unit...

`expect` is `equal`, `differ` or `excluded`; `symbols` the mangled names
compared (space-separated); `kinds` the row kinds that carry the expected
difference (comma-separated: 2, 3, 4, 5); `level` the lowest
`-fmason-metadata` level at which those rows are written (`min`,
`default`, `detail`, `all`); `reason` a stable id for an `excluded` case
(`-` otherwise); each `unit` is `FILE STD [FLAG...]` (space-separated).
In a flag, `{dir}` is the case's absolute source directory; the flag
`@pch:H` builds a PCH of `H` with the unit's standard and flags and adds
`-include-pch` (the PCH route). kiln++ runs in the case's directory.

For every case (or the CASE ids given) this compiles each unit with
`-fmason-metadata=LEVEL -c` (LEVEL default `default`) into DIR/CASE/ and
writes DIR/manifest.tsv, one line per case, tab-separated:

    case  expect  symbols  kinds  level  reason  (object  std  flags)...

`object` is relative to DIR; `std` is the unit's `-std` value; `flags`
the unit's other flags (space-separated, `{dir}` expanded, `-` if none).
A linker lane reads it: `equal` cases must link silently, `differ` cases
must be reported (a hash difference; a `USES_INTERNAL` row (K12) or an
`EXPLICIT_SPEC` + `IMPLICIT_INST` pair (K13) by its flags), `excluded`
cases are documented misses (spec/abi.md "Misses and exclusions") that
must not be reported.

`--check` also verifies each expectation through odr_oracle.py's decoder:
every symbol has a row in every object (no row anywhere for an excluded
case whose reason is `X15`); `equal`: one hash, no K13 pair, and flags
equal apart from `VAGUE`, `IMPLICIT_INST` and `EXPLICIT_INST` (N22, N9);
`differ`: two hashes, a K13 flag pair or `USES_INTERNAL`; `excluded`:
one hash (the miss). A symbol written `type:NAME` is the kind 3 row of
the type whose Itanium `<type>` is NAME (`type:1S`), compared by layout,
size and alignment. Exit status 1 on a failed check or a failed
compile. Python 3 standard library only. spec/harness.md "The O-13
corpus".
"""

import argparse
import os
import shutil
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odr_oracle  # noqa: E402  (beside this script)

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
VAGUE, IMPLICIT_INST, EXPLICIT_INST, EXPLICIT_SPEC, USES_INTERNAL = 1, 2, 4, 8, 16


def read_case(path):
    """(expect, symbols, kinds, level, reason, [(file, std, [flags])])."""
    line = open(os.path.join(path, "case.tsv")).read().rstrip("\n")
    f = line.split("\t")
    if len(f) < 7 or f[0] not in ("equal", "differ", "excluded"):
        raise SystemExit(f"{path}/case.tsv: malformed: {line!r}")
    units = []
    for u in f[5:]:
        w = u.split()
        units.append((w[0], w[1], w[2:]))
    return f[0], f[1].split(), f[2], f[3], f[4], units


def rows_of(obj):
    """{key: (hash, flags)} of the object's kind 2 rows and {key: (layout,
    size, align)} of its kind 3 rows."""
    data = open(obj, "rb").read()
    secs, _ = odr_oracle.elf_sections(data)
    rows, types = {}, {}
    for name, _t, sec, _l in secs:
        if name == ".mason.meta":
            for _scheme, rs in odr_oracle.units(sec, obj):
                for key, h, flags, _cls in rs:
                    rows[key] = (h.hex(), flags)
            types.update(type_rows(sec))
    return rows, types


def type_rows(sec):
    """{key: (layout, size, align)} of the kind 3 rows of a section
    (spec/codegen.md section 5.5 "Kind 3": 56-byte rows)."""
    out = {}
    at = 0
    while at + 32 <= len(sec):
        if sec[at:at + 4] != b"MASN":
            at += 8
            continue
        size, hsize, nblocks = struct.unpack_from("<III", sec, at + 8)
        b = at + hsize
        for _ in range(nblocks):
            kind, _v, bsize, rsize, rcount = struct.unpack_from("<HHIII", sec, b)
            if kind == 3:
                for i in range(rcount):
                    r = b + 16 + i * rsize
                    key, = struct.unpack_from("<Q", sec, r)
                    layout, = struct.unpack_from("<Q", sec, r + 24)
                    tsize, align = struct.unpack_from("<IH", sec, r + 36)
                    out[key] = (layout, tsize, align)
            b += bsize
        at += size
    return out


def check(expect, symbols, reason, objs):
    """The failures of one case's expectation, as text."""
    out = []
    got_all = [rows_of(o) for o in objs]
    for s in symbols:
        if s.startswith("type:"):
            # A kind 3 row, by the type's Itanium <type>.
            k = odr_oracle.fnv1a64(s[5:].encode())
            got = [t.get(k) for _, t in got_all]
        else:
            k = odr_oracle.fnv1a64(s.encode())
            got = [r.get(k) for r, _ in got_all]
        if expect == "excluded" and reason == "X15":
            if any(g is not None for g in got):
                out.append(f"{s}: a row (X15: none expected)")
            continue
        if any(g is None for g in got):
            out.append(f"{s}: no row in " + " ".join(o for o, g in zip(objs, got) if g is None))
            continue
        if s.startswith("type:"):
            differs = len(set(got)) > 1
            if differs != (expect == "differ"):
                out.append(f"{s}: kind 3 rows {'differ' if differs else 'do not differ'}: {got}")
            continue
        hashes = {g[0] for g in got}
        flags = {g[1] for g in got}
        k13 = any(f & EXPLICIT_SPEC for f in flags) and any(f & IMPLICIT_INST for f in flags)
        if expect == "equal":
            # Compared by hash; a strong and a VAGUE row (N22), an explicit
            # and an implicit instantiation (N9) are compatible (O-15).
            loose = {f & ~(VAGUE | IMPLICIT_INST | EXPLICIT_INST) for f in flags}
            if len(hashes) > 1 or len(loose) > 1 or k13:
                out.append(f"{s}: rows differ: {got}")
        elif expect == "excluded" and len(hashes) > 1:
            out.append(f"{s}: hashes differ (the miss is gone: update the case): {got}")
        elif expect == "differ":
            internal = any(f & USES_INTERNAL for f in flags)
            if len(hashes) < 2 and not k13 and not internal:
                out.append(f"{s}: rows do not differ: {got}")
    return out


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--kiln", required=True, help="the kiln++ to run")
    ap.add_argument("--out", required=True, help="output directory (objects and manifest.tsv)")
    ap.add_argument("--corpus", default=os.path.join(ROOT, "tests/mason/corpus"))
    ap.add_argument("--level", default="default", help="-fmason-metadata level (default: default)")
    ap.add_argument("--check", action="store_true", help="verify each expectation")
    ap.add_argument("cases", nargs="*")
    args = ap.parse_args(argv)
    kiln = os.path.abspath(args.kiln)
    out = os.path.abspath(args.out)
    os.makedirs(out, exist_ok=True)
    ids = args.cases or sorted(d for d in os.listdir(args.corpus)
                               if os.path.isfile(os.path.join(args.corpus, d, "case.tsv")))
    lines = []
    failed = 0
    for cid in ids:
        src = os.path.abspath(os.path.join(args.corpus, cid))
        expect, symbols, kinds, level, reason, units = read_case(src)
        dst = os.path.join(out, cid)
        shutil.rmtree(dst, ignore_errors=True)
        os.makedirs(dst)
        fields = [cid, expect, " ".join(symbols), kinds, level, reason]
        objs = []
        ok = True
        for file, std, flags in units:
            flags = [f.replace("{dir}", src) for f in flags]
            cmd = [kiln, f"-std={std}"]
            for f in flags:
                if f.startswith("@pch:"):
                    h = f[5:]
                    pch = os.path.join(dst, os.path.splitext(file)[0] + ".pch")
                    plain = [x for x in flags if not x.startswith("@pch:")]
                    r = subprocess.run([kiln, f"-std={std}", *plain, "-x", "c++-header", h, "-o", pch],
                                       cwd=src, capture_output=True, text=True)
                    if r.returncode:
                        print(f"FAIL {cid} {file}: PCH of {h}\n{r.stderr[:2000]}")
                        ok = False
                    cmd += ["-include-pch", pch]
                else:
                    cmd.append(f)
            obj = os.path.join(dst, os.path.splitext(file)[0] + ".o")
            cmd += [f"-fmason-metadata={args.level}", "-c", file, "-o", obj]
            r = subprocess.run(cmd, cwd=src, capture_output=True, text=True)
            if r.returncode:
                print(f"FAIL {cid} {file}: does not compile\n{r.stderr[:2000]}")
                ok = False
            objs.append(obj)
            fields += [os.path.relpath(obj, out), std, " ".join(flags) or "-"]
        lines.append("\t".join(fields))
        if not ok:
            failed += 1
            continue
        if args.check:
            bad = check(expect, symbols, reason, objs)
            for b in bad:
                print(f"FAIL {cid} ({expect}): {b}")
            failed += bool(bad)
            if not bad:
                print(f"ok {cid} ({expect})")
    with open(os.path.join(out, "manifest.tsv"), "w") as f:
        f.write("# case\texpect\tsymbols\tkinds\tlevel\treason\t(object\tstd\tflags)...\n")
        f.write("\n".join(lines) + "\n")
    print(f"{len(ids)} cases, {failed} failed; manifest {os.path.join(out, 'manifest.tsv')}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
