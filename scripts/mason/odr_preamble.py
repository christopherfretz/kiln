#!/usr/bin/env python3
"""odr_preamble.py: context independence of kiln's `.mason.meta` kind 2 rows.

Usage: python3 -I scripts/mason/odr_preamble.py --kiln PATH [--preambles DIR]
           [--std STD]... [--stdlib LIB]... [--arg ARG]... [--all-scopes]
           [--work DIR] UNIT...

A row's hash and flags must not depend on anything else in its unit
(spec/abi.md "Definition hash": no byte of a row depends on unit-wide
state). For every UNIT, `-std` level and standard library, this compiles
the unit as it is and once behind each PREAMBLE of DIR (default:
tests/mason/context/preambles), placed before the unit's text, after it
(the trailer side), or around it, as the preamble's `// sides:` line says,
and compares the kind 2 rows of every key present in both objects: equal
hash and equal flags. A preamble whose `// expect: differ SYM...` line
names symbols must make exactly those rows differ (a must-differ case).
Only preambles with `// scope: any` apply to an arbitrary unit; those with
`// scope: defs` name the entities of tests/mason/context/defs.h and run
only with --all-scopes (the unit is then expected to include defs.h). A
unit/preamble pair that does not compile at a `-std` level (a preamble
written for a later standard) is reported as skipped, not failed, when
the plain unit does compile there.

Each mismatch prints the unit, the standard, the library, the preamble,
the side, the symbol (resolved from the object's symbol table) and the
two (hash, flags). The exit status is 1 on any mismatch, 2 when a plain
unit does not compile, else 0. Python 3 standard library only; the
decoder is odr_oracle.py's. spec/harness.md "Context independence of kind
2 rows".
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import odr_oracle  # noqa: E402  (beside this script)

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def directives(text):
    """The `// sides:`, `// scope:` and `// expect:` lines of a preamble."""
    d = {"sides": ["pre", "post"], "scope": "any", "differ": []}
    for line in text.splitlines():
        if not line.startswith("// "):
            break
        k, _, v = line[3:].partition(":")
        if k == "sides":
            d["sides"] = v.split()
        elif k == "scope":
            d["scope"] = v.strip()
        elif k == "expect" and v.split()[:1] == ["differ"]:
            d["differ"] = v.split()[1:]
    return d


def placed(side, preamble, unit):
    """The source of `unit` with `preamble` on `side`."""
    if side == "pre":
        return preamble + "\n" + unit
    if side == "post":
        return unit + "\n" + preamble + "\n"
    if side == "wrap":
        before, _, after = preamble.partition("//@ include\n")
        return before + unit + "\n" + after
    raise ValueError(side)


def rows_of(obj_path):
    """{key: (hash, flags)} of the kind 2 rows and {key: symbol}."""
    data = open(obj_path, "rb").read()
    secs, cstr = odr_oracle.elf_sections(data)
    rows = {}
    for name, _t, sec, _l in secs:
        if name == ".mason.meta":
            for _scheme, rs in odr_oracle.units(sec, obj_path):
                for key, h, flags, _cls in rs:
                    rows[key] = (h.hex(), flags)
    names = {}
    for s in odr_oracle.symbol_names(secs, cstr):
        names.setdefault(odr_oracle.fnv1a64(s), s.decode("latin1"))
    return rows, names


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--kiln", required=True, help="the kiln++ to run")
    ap.add_argument("--preambles", default=os.path.join(ROOT, "tests/mason/context/preambles"))
    ap.add_argument("--std", action="append", help="-std levels (default c++20)")
    ap.add_argument("--stdlib", action="append", help="standard libraries (default: the driver's)")
    ap.add_argument("--all-scopes", action="store_true", help="also the preambles naming defs.h")
    ap.add_argument("--work", help="keep the sources and objects here")
    ap.add_argument("--arg", action="append", default=[], help="one more kiln++ argument")
    ap.add_argument("units", nargs="+")
    args = ap.parse_args(argv)
    stds = args.std or ["c++20"]
    libs = args.stdlib or [None]
    pres = []
    for f in sorted(os.listdir(args.preambles)):
        text = open(os.path.join(args.preambles, f)).read()
        d = directives(text)
        if d["scope"] == "any" or args.all_scopes:
            pres.append((f, text, d))
    work = args.work or tempfile.mkdtemp(prefix="odr-preamble-")
    os.makedirs(work, exist_ok=True)
    mismatches = compared = skipped = 0
    status = 0
    try:
        for unit in args.units:
            src = open(unit).read()
            base = os.path.splitext(os.path.basename(unit))[0]
            inc = ["-I", os.path.dirname(os.path.abspath(unit)), "-I", os.path.join(ROOT, "tests/mason/context")]
            for std in stds:
                for lib in libs:
                    flags = [f"-std={std}"] + ([f"-stdlib={lib}"] if lib else []) + inc + args.arg

                    def compile_(text, tag):
                        p = os.path.join(work, f"{base}-{std}-{lib or 'default'}-{tag}.cpp")
                        open(p, "w").write(text)
                        o = p[:-4] + ".o"
                        r = subprocess.run([args.kiln, *flags, "-fmason-metadata", "-c", p, "-o", o],
                                           capture_output=True, text=True)
                        return (o if r.returncode == 0 else None), r.stderr

                    plain, err = compile_(src, "plain")
                    if plain is None:
                        print(f"FAIL {unit} {std} {lib}: the plain unit does not compile\n{err[:2000]}")
                        status = 2
                        continue
                    rows, names = rows_of(plain)
                    for f, text, d in pres:
                        for side in d["sides"]:
                            tag = f"{os.path.splitext(f)[0]}-{side}"
                            obj, err = compile_(placed(side, text, src), tag)
                            if obj is None:
                                skipped += 1
                                print(f"skip {unit} {std} {lib} {f} {side}: does not compile: "
                                      + (err.splitlines() or [""])[0][:200])
                                continue
                            other, onames = rows_of(obj)
                            want = {odr_oracle.fnv1a64(s.encode()) for s in d["differ"]}
                            compared += 1
                            got = set()
                            for k, v in rows.items():
                                if k in other and other[k] != v:
                                    got.add(k)
                            for k in sorted(got ^ want):
                                mismatches += 1
                                sym = names.get(k) or onames.get(k) or f"{k:#018x}"
                                what = "differs" if k in got else "does not differ (expected to)"
                                print(f"MISMATCH {unit} -std={std} {lib or ''} preamble {f} ({side}): "
                                      f"{sym} {what}: {rows.get(k)} / {other.get(k)}")
                            os.remove(obj)
                    print(f"{unit} -std={std} {lib or 'default'}: {len(rows)} rows, "
                          f"{len(pres)} preambles")
    finally:
        if not args.work:
            shutil.rmtree(work, ignore_errors=True)
    print(f"compared {compared} unit pairs, skipped {skipped}, mismatching rows {mismatches}")
    if mismatches:
        return 1
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
