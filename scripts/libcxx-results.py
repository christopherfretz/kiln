#!/usr/bin/env python3
"""libcxx-results.py [--baseline FILE ...] [--label NAME] RESULTS.tsv ...

Merges the `--results` files of `cargo xtask libcxx` runs (one per shard of
`--shard K/N`; format: spec/libcxx.md "Results file") and prints a Markdown
summary: the counts per outcome for the whole selection and for `std/` and
`libcxx/`, the pass rate (pass / (total - unsupported), as `--report`
computes it), and every test that failed the run (FAIL, XPASS, KILLED,
TIMEOUT, ERROR) with its detail and first cluster key.

With --baseline (results files of another run, e.g. the other architecture)
each failing test is marked `new` when the baseline has it as PASS, XFAIL or
UNSUPPORTED, `same` when it failed there too, `absent` when the baseline does
not have it; and the tests that fail only in the baseline are listed.

Exit status 2 when a test appears in two files (overlapping shards) or a
line is malformed, else 0. Used by .github/workflows/sweep.yml (WP-961).
"""

import argparse
import sys


def die(msg):
    print(f"libcxx-results: {msg}", file=sys.stderr)
    sys.exit(2)


FAILING = ("FAIL", "XPASS", "KILLED", "TIMEOUT", "ERROR")
ORDER = ("PASS", "FAIL", "XFAIL", "XPASS", "UNSUPPORTED", "KILLED", "TIMEOUT", "ERROR")


def load(paths):
    """{test: (outcome, detail, key)}; exits 2 on a duplicate or a bad line."""
    out = {}
    for path in paths:
        with open(path, encoding="utf-8") as f:
            for n, line in enumerate(f, 1):
                line = line.rstrip("\n")
                if not line or line.startswith("#"):
                    continue
                cols = line.split("\t")
                if len(cols) != 5 or cols[0] not in ORDER:
                    die(f"{path}:{n}: malformed line")
                if cols[1] in out:
                    die(f"{cols[1]} appears twice (overlapping shards?)")
                out[cols[1]] = (cols[0], cols[2], cols[3])
    return out


def counts(results, prefix=""):
    c = dict.fromkeys(ORDER, 0)
    for test, (outcome, _, _) in results.items():
        if test.startswith(prefix):
            c[outcome] += 1
    return c


def rate(c):
    denom = sum(c.values()) - c["UNSUPPORTED"]
    return f"{100.0 * c['PASS'] / denom:.1f}%" if denom else "-"


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--baseline", action="append", default=[])
    ap.add_argument("--label", default="libc++ suite")
    ap.add_argument("files", nargs="+")
    a = ap.parse_args()
    res = load(a.files)
    base = load(a.baseline) if a.baseline else None
    out = [f"## {a.label}", "", f"{len(a.files)} results file(s), {len(res)} tests.", ""]
    out.append("| scope | total | " + " | ".join(o.lower() for o in ORDER) + " | pass rate |")
    out.append("|---|---:|" + "---:|" * len(ORDER) + "---:|")
    for label, prefix in (("all", ""), ("`std/`", "std/"), ("`libcxx/`", "libcxx/")):
        c = counts(res, prefix)
        total = sum(c.values())
        if total == 0 and prefix:
            continue
        cells = " | ".join(str(c[o]) for o in ORDER)
        out.append(f"| {label} | {total} | {cells} | {rate(c)} |")
    failing = sorted(t for t, (o, _, _) in res.items() if o in FAILING)
    out += ["", f"### Failing tests ({len(failing)})", ""]
    for t in failing:
        o, detail, key = res[t]
        mark = ""
        if base is not None:
            b = base.get(t)
            mark = " **new**" if b and b[0] not in FAILING else " same" if b else " absent"
        key = "" if key == "-" else f" — `{key}`"
        out.append(f"- {o} ({detail}){mark}: `{t}`{key}")
    if base is not None:
        gone = sorted(t for t, (o, _, _) in base.items() if o in FAILING and t in res and res[t][0] not in FAILING)
        out += ["", f"### Failing only in the baseline ({len(gone)})", ""]
        out += [f"- {base[t][0]} -> {res[t][0]}: `{t}`" for t in gone]
    print("\n".join(out))


if __name__ == "__main__":
    main()
