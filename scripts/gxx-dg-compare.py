#!/usr/bin/env python3
"""Compare, summarize and merge the per-case TSVs of `cargo xtask gxx-dg`.

The runner (spec/harness.md "The g++.dg runner") writes one row per case
(a test at one standard or optimization level): test, opt, then each
compiler's class and detail (kiln, gcc, clang), negative, xfail, dropped.
A skip row has kiln class `skip`; every other row is an applicable case.
Standard library only; run it as `python3 -I scripts/gxx-dg-compare.py`.

  summary A.tsv [--label L]
      the headline ("N of M applicable cases pass (P%)"), the class counts
      per compiler and kiln's failures by triage bucket
  compare BEFORE.tsv AFTER.tsv [--labels B,A] [--list]
      both headlines, the class matrix (kiln's class before against after,
      over the cases both runs have), and every case that went from pass
      to a failing class (with --list: also the fail-to-pass cases)
  clusters A.tsv [--bucket B] [--top N] [--examples K]
      the clusters of kiln's failures (the runner's cluster key: class and
      first error with quoted names and numbers folded), per triage bucket
  merge --out M.tsv [--expect N] S1.tsv S2.tsv ...
      concatenate the shard TSVs of one run (`--shard K/N`) into one;
      fails when the headers differ, a case appears twice or, with
      --expect, the number of shards is not N
"""

import argparse
import collections
import sys

HEADER = [
    "test", "opt", "kiln", "kiln_detail", "gcc", "gcc_detail",
    "clang", "clang_detail", "negative", "xfail", "dropped",
]
# The runner's class names (xtask/src/gcc_tests/outcome.rs), report order.
CLASSES = [
    "pass", "compile-reject", "accept-invalid", "wrong-line", "wrong",
    "crash", "timeout", "not-run",
]
# The runner's triage buckets (gxx.rs `bucket`), report order.
BUCKETS = [
    "kiln crash / hang",
    "wrong-code",
    "rejects-valid, clang accepts",
    "rejects-valid, clang rejects too",
    "rejects-valid, no clang result",
    "accepts-invalid, clang rejects",
    "accepts-invalid, clang accepts too",
    "accepts-invalid, no clang result",
    "wrong-line, clang matches",
    "wrong-line, clang differs too",
    "wrong-line, no clang result",
]


def read_tsv(path):
    """The header and the rows (dicts) of one TSV."""
    with open(path, encoding="utf-8", errors="replace") as f:
        lines = f.read().split("\n")
    head = lines[0].split("\t")
    if head[:len(HEADER)] != HEADER:
        sys.exit(f"{path}: not a gxx-dg TSV (header {head!r})")
    rows = []
    for ln in lines[1:]:
        if not ln:
            continue
        cells = ln.split("\t")
        cells += [""] * (len(head) - len(cells))
        rows.append(dict(zip(head, cells)))
    return lines[0], rows


def applicable(rows):
    return [r for r in rows if r["kiln"] != "skip"]


def headline(rows):
    app = applicable(rows)
    tests = len({r["test"] for r in rows})
    p = sum(1 for r in app if r["kiln"] == "pass")
    pct = 100.0 * p / len(app) if app else 0.0
    return (f"{tests} tests, {len(rows)} rows, {len(app)} applicable cases; "
            f"{p} of {len(app)} applicable cases pass ({pct:.1f}%)")


def bucket(r):
    """The runner's triage bucket of a case kiln fails, else None."""
    k, clang = r["kiln"], r["clang"]
    has_clang = clang not in ("", "-")
    if k in ("pass", "not-run", "skip"):
        return None
    if k in ("crash", "timeout"):
        return "kiln crash / hang"
    if k == "wrong":
        return "wrong-code"
    if k == "compile-reject":
        if not has_clang:
            return "rejects-valid, no clang result"
        return "rejects-valid, clang accepts" if clang == "pass" else "rejects-valid, clang rejects too"
    if k == "accept-invalid":
        if not has_clang:
            return "accepts-invalid, no clang result"
        return ("accepts-invalid, clang accepts too" if clang == "accept-invalid"
                else "accepts-invalid, clang rejects")
    if k == "wrong-line":
        if not has_clang:
            return "wrong-line, no clang result"
        return "wrong-line, clang matches" if clang == "pass" else "wrong-line, clang differs too"
    return None


def cluster_key(cls, detail):
    """outcome.rs `cluster_key`: quoted names and digit runs folded."""
    out = []
    quote = None
    last_digit = False
    for ch in detail:
        if quote is not None:
            if ch == quote or (quote == "‘" and ch == "’"):
                quote = None
                out.append(ch)
            continue
        if ch in ("'", "‘", "`"):
            quote = ch
            out.append(ch)
            out.append("…")
            last_digit = False
            continue
        if ch.isdigit() and ch.isascii():
            if not last_digit:
                out.append("N")
            last_digit = True
            continue
        last_digit = False
        out.append(ch)
    return f"{cls}: {''.join(out)}"


def table(head, rows):
    out = ["| " + " | ".join(head) + " |", "|---|" + "---:|" * (len(head) - 1)]
    out += ["| " + " | ".join(str(c) for c in row) + " |" for row in rows]
    return "\n".join(out)


def cmd_summary(a):
    _, rows = read_tsv(a.tsv)
    app = applicable(rows)
    print(f"## {a.label or a.tsv}\n")
    print(headline(rows) + "\n")
    roles = ["kiln", "gcc", "clang"]
    body = []
    for c in CLASSES:
        counts = [sum(1 for r in app if r[x] == c) for x in roles]
        if any(counts):
            body.append([c] + counts)
    body.append(["total"] + [sum(1 for r in app if r[x] not in ("", "-")) for x in roles])
    print(table(["class"] + roles, body) + "\n")
    bc = collections.Counter(bucket(r) for r in app)
    print(table(["bucket", "cases"], [[b, bc[b]] for b in BUCKETS if bc[b]]))


def cmd_compare(a):
    labels = (a.labels or "before,after").split(",")
    if len(labels) != 2:
        sys.exit("--labels takes two comma-separated names")
    _, before = read_tsv(a.before)
    _, after = read_tsv(a.after)
    for lab, rows in zip(labels, (before, after)):
        print(f"- {lab}: {headline(rows)}")
    b = {(r["test"], r["opt"]): r for r in before}
    f = {(r["test"], r["opt"]): r for r in after}
    both = sorted(set(b) & set(f))
    print(f"- cases in both: {len(both)}; only {labels[0]}: {len(set(b) - set(f))}; "
          f"only {labels[1]}: {len(set(f) - set(b))}\n")
    cols = CLASSES + ["skip"]
    m = collections.Counter((b[k]["kiln"], f[k]["kiln"]) for k in both)
    used = [c for c in cols if any(m[(c, x)] or m[(x, c)] for x in cols)]
    print(f"Class matrix (rows: {labels[0]}, columns: {labels[1]}; kiln, cases)\n")
    print(table([f"{labels[0]} \\ {labels[1]}"] + used,
                [[c] + [m[(c, x)] for x in used] for c in used]))
    regress = [k for k in both if b[k]["kiln"] == "pass" and f[k]["kiln"] not in ("pass", "skip")]
    fixed = [k for k in both if b[k]["kiln"] not in ("pass", "skip") and f[k]["kiln"] == "pass"]
    between = sum(1 for k in both if b[k]["kiln"] != f[k]["kiln"]
                  and b[k]["kiln"] not in ("pass", "skip") and f[k]["kiln"] not in ("pass", "skip"))
    print(f"\npass -> failing: {len(regress)}; failing -> pass: {len(fixed)}; "
          f"between two failing classes: {between}\n")
    print(f"### pass in {labels[0]}, failing in {labels[1]} ({len(regress)})\n")
    for k in regress:
        r = f[k]
        print(f"- {k[0]} [{k[1]}] {r['kiln']}: {r['kiln_detail']} "
              f"(gcc {r['gcc'] or '-'}, clang {r['clang'] or '-'})")
    if a.list:
        print(f"\n### failing in {labels[0]}, pass in {labels[1]} ({len(fixed)})\n")
        for k in fixed:
            print(f"- {k[0]} [{k[1]}] was {b[k]['kiln']}: {b[k]['kiln_detail']}")


def cmd_clusters(a):
    _, rows = read_tsv(a.tsv)
    by = collections.defaultdict(lambda: collections.defaultdict(list))
    for r in applicable(rows):
        bk = bucket(r)
        if bk:
            by[bk][cluster_key(r["kiln"], r["kiln_detail"])].append(f"{r['test']} {r['opt']}")
    for bk in BUCKETS:
        if a.bucket and a.bucket != bk:
            continue
        cl = sorted(by[bk].items(), key=lambda kv: (-len(kv[1]), kv[0]))
        if not cl:
            continue
        total = sum(len(v) for _, v in cl)
        print(f"### {bk} ({total} cases, {len(cl)} clusters)\n")
        for key, ex in cl[:a.top]:
            tests = len({e.split(" ")[0] for e in ex})
            print(f"{len(ex):6} ({tests} tests)  {key}")
            for e in ex[:a.examples]:
                print(f"         {e}")
        print()


def cmd_merge(a):
    if a.expect is not None and len(a.shards) != a.expect:
        sys.exit(f"merge: {len(a.shards)} shard TSVs, expected {a.expect}")
    head = None
    seen = set()
    out = []
    for p in a.shards:
        h, rows = read_tsv(p)
        if head is None:
            head = h
        elif h != head:
            sys.exit(f"merge: {p}'s header differs from the first shard's")
        for r in rows:
            k = (r["test"], r["opt"])
            if k in seen:
                sys.exit(f"merge: case {k[0]} [{k[1]}] appears twice ({p})")
            seen.add(k)
            out.append(r)
    out.sort(key=lambda r: (r["test"], r["opt"]))
    cols = head.split("\t")
    with open(a.out, "w", encoding="utf-8") as f:
        f.write(head + "\n")
        for r in out:
            f.write("\t".join(r.get(c, "") for c in cols) + "\n")
    print(f"merge: {len(a.shards)} shards, {headline(out)} -> {a.out}")


def main():
    ap = argparse.ArgumentParser(
        prog="gxx-dg-compare.py", description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("summary", help="headline, classes and buckets of one TSV")
    s.add_argument("tsv")
    s.add_argument("--label")
    s.set_defaults(fn=cmd_summary)
    c = sub.add_parser("compare", help="two runs by case")
    c.add_argument("before")
    c.add_argument("after")
    c.add_argument("--labels", help="two comma-separated run names")
    c.add_argument("--list", action="store_true", help="also list the fail-to-pass cases")
    c.set_defaults(fn=cmd_compare)
    k = sub.add_parser("clusters", help="kiln's failure clusters per bucket")
    k.add_argument("tsv")
    k.add_argument("--bucket", help="one bucket name (default: every bucket)")
    k.add_argument("--top", type=int, default=30)
    k.add_argument("--examples", type=int, default=3)
    k.set_defaults(fn=cmd_clusters)
    m = sub.add_parser("merge", help="concatenate the shard TSVs of one run")
    m.add_argument("--out", required=True)
    m.add_argument("--expect", type=int, help="the number of shards")
    m.add_argument("shards", nargs="+")
    m.set_defaults(fn=cmd_merge)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
