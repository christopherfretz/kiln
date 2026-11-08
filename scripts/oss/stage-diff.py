#!/usr/bin/env python3
"""stage-diff.py: the bootstrap IR comparison of WP-815.

  stage-diff.py make --log compile.jsonl --sample N --out DIR NAME=CXX...
  stage-diff.py compare --out DIR NAME...

`make` takes every `kind == "ok"` build-phase TU of a harness log (the
wrapper's compile.jsonl, spec/landed/harness/WP-666.md), keeps a fixed sample
of N of them (sorted by source path, every len/N-th, so the sample is the
same for the same tree), and writes DIR/Makefile: for each compiler NAME=CXX
and each sampled TU, the TU's own command with CXX as the compiler, its
`-c`, `-o` and dependency-file flags dropped and `-S -emit-llvm -O2
-fdiscard-value-names -o DIR/NAME/<i>.ll` appended. Run it with `make -k -f
DIR/Makefile` so it draws from the shared jobserver. `-fdiscard-value-names`
is explicit because clang keeps value names by default only in a build with
assertions, and the compilers compared differ in that.

`compare` normalizes each DIR/<NAME>/<i>.ll and compares it across the
NAMEs. Normalized: the `!llvm.ident` string (a vendor build names itself
there). Everything else must match byte for byte. It prints one line per
differing TU and a summary, and writes the first differing hunk of each to
DIR/diffs/<i>.diff.
"""

import argparse
import difflib
import json
import os
import re
import shlex
import sys

DROP_WITH_VALUE = ("-o", "-MF", "-MT", "-MQ")
DROP = ("-c", "-MD", "-MMD")
IDENT_RE = re.compile(r'^(!\d+ = !\{!")[^"]*clang version [^"]*("\})$', re.M)


def sample(log, n):
    seen = {}
    with open(log, encoding="utf-8") as f:
        for line in f:
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            if r.get("phase") == "build" and r.get("kind") == "ok" and r.get("source") not in ("link", "none", None):
                seen[os.path.normpath(os.path.join(r["cwd"], r["source"]))] = r
    tus = [seen[k] for k in sorted(seen)]
    if n >= len(tus):
        return tus
    step = len(tus) / n
    return [tus[int(i * step)] for i in range(n)]


def emit_args(argv):
    out, skip = [], False
    for a in argv[1:]:
        if skip:
            skip = False
            continue
        if a in DROP_WITH_VALUE:
            skip = True
            continue
        if a in DROP or (a.startswith("-o") and len(a) > 2):
            continue
        out.append(a)
    return out + ["-S", "-emit-llvm", "-O2", "-fdiscard-value-names"]


def cmd_make(a):
    tus = sample(a.log, a.sample)
    comps = [c.split("=", 1) for c in a.compilers]
    out = os.path.abspath(a.out)
    os.makedirs(out, exist_ok=True)
    rules, targets = [], []
    index = []
    for i, r in enumerate(tus):
        index.append({"i": i, "source": r["source"], "cwd": r["cwd"]})
        args = emit_args(r["argv"])
        for name, cxx in comps:
            t = f"{out}/{name}/{i}.ll"
            targets.append(t)
            cmd = " ".join(shlex.quote(x) for x in [cxx] + args + ["-o", t])
            rules.append(f"{t}:\n\t@mkdir -p {out}/{name} && cd {shlex.quote(r['cwd'])} && {cmd}\n")
    with open(f"{out}/Makefile", "w") as f:
        f.write("all: " + " ".join(targets) + "\n\n" + "\n".join(rules))
    with open(f"{out}/index.json", "w") as f:
        json.dump(index, f, indent=1)
    print(f"stage-diff: {len(tus)} TUs x {len(comps)} compilers -> {out}/Makefile")


def normalized(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        return IDENT_RE.sub(r"\1clang version\2", f.read())


def cmd_compare(a):
    out = os.path.abspath(a.out)
    with open(f"{out}/index.json") as f:
        index = json.load(f)
    os.makedirs(f"{out}/diffs", exist_ok=True)
    same = differ = missing = 0
    for e in index:
        i = e["i"]
        paths = [f"{out}/{n}/{i}.ll" for n in a.names]
        if not all(os.path.exists(p) for p in paths):
            missing += 1
            print(f"MISSING {i} {e['source']}: " + " ".join(n for n, p in zip(a.names, paths) if not os.path.exists(p)))
            continue
        texts = [normalized(p) for p in paths]
        bad = [n for n, t in zip(a.names[1:], texts[1:]) if t != texts[0]]
        if not bad:
            same += 1
            continue
        differ += 1
        print(f"DIFF {i} {e['source']}: {a.names[0]} vs {' '.join(bad)}")
        k = a.names.index(bad[0])
        d = difflib.unified_diff(texts[0].splitlines(), texts[k].splitlines(), a.names[0], bad[0], n=3, lineterm="")
        with open(f"{out}/diffs/{i}.diff", "w") as f:
            f.write("\n".join(list(d)[:400]) + "\n")
    print(f"stage-diff: {same} identical, {differ} differ, {missing} missing, of {len(index)} ({' / '.join(a.names)})")
    return 1 if differ or missing else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("make")
    m.add_argument("--log", required=True)
    m.add_argument("--sample", type=int, default=200)
    m.add_argument("--out", required=True)
    m.add_argument("compilers", nargs="+", metavar="NAME=CXX")
    c = sub.add_parser("compare")
    c.add_argument("--out", required=True)
    c.add_argument("names", nargs="+", metavar="NAME")
    a = ap.parse_args()
    if a.cmd == "make":
        cmd_make(a)
        return 0
    return cmd_compare(a)


if __name__ == "__main__":
    sys.exit(main())
