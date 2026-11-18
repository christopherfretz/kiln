#!/usr/bin/env python3
"""Compare, summarize and merge the per-case TSVs of `cargo xtask gxx-dg`.

The runner (spec/harness.md "The g++.dg runner") writes one row per case
(a test at one standard or optimization level): test, opt, then each
compiler's class and detail (kiln, gcc, clang), negative, xfail, dropped.
A skip row has kiln class `skip`; every other row is an applicable case.
Standard library only; run it as `python3 -I scripts/gxx-dg-compare.py`.

  summary A.tsv [--label L]
      the headline ("N of M decided cases pass (P%), K expect-unknown"), the class counts
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
  baseline --out B.txt --commit C HOST=RUN.tsv ...
      the reduced baseline the release gate compares against
      (docs/status/gxx-dg-baseline.txt): per case the test id, the option
      set and kiln's class on each host, nothing else (no diagnostic text,
      no line of any GCC file)

Wherever a TSV is read, a baseline file is accepted too, as FILE@HOST
(e.g. docs/status/gxx-dg-baseline.txt@aarch64): its rows carry kiln's
class only. `compare --gate` is the release gate's pass rule
(spec/harness.md "Release gate"): exit status 1 when a case went from
pass to a failing class and is not in the `--explained` list (lines
`test [opt]` followed by the reason; `#` starts a comment).
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
    "crash", "timeout", "not-run", "expect-unknown",
]
# Neither a pass nor a failure: the test file does not decide the case
# (WP-1554; spec/harness.md "The g++.dg runner").
UNDECIDED = ("expect-unknown",)
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


BASELINE_MAGIC = "# kiln gxx-dg baseline v1"
# One letter per class in a baseline; `.` is a case the host does not have.
CODES = {
    "pass": "p", "compile-reject": "r", "accept-invalid": "i", "wrong-line": "l",
    "wrong": "w", "crash": "c", "timeout": "t", "not-run": "n",
    "expect-unknown": "u", "skip": "s",
}
CLASS_OF = {v: k for k, v in CODES.items()}


def write_baseline(out, commit, runs):
    """runs: [(host, rows)]. One line per test (under an `@dir/` line),
    one token per option set: its code letter, then a class letter per host."""
    hosts = [h for h, _ in runs]
    by = [{(r["test"], r["opt"]): r["kiln"] for r in rows} for _, rows in runs]
    keys = sorted(set().union(*by))
    opts = sorted({o for _, o in keys})
    if len(opts) > 52:
        sys.exit("baseline: more than 52 option sets")
    letters = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"
    code = dict(zip(opts, letters))
    lines = [BASELINE_MAGIC,
             "# The kiln class of every case of a full `cargo xtask gxx-dg` run per host,",
             "# reduced for the release gate (scripts/gxx-dg-compare.py baseline; read it",
             "# with FILE@HOST). Test ids and result classes only.",
             f"#commit {commit}",
             "#hosts " + " ".join(hosts),
             "#opts " + " ".join(f"{code[o]}={o}" for o in opts),
             "#classes " + " ".join(f"{v}={k}" for k, v in CODES.items()) + " .=absent"]
    cur_dir, cur_test, toks = None, None, []

    def flush():
        if cur_test is not None:
            lines.append(cur_test.rsplit("/", 1)[-1] + "\t" + " ".join(toks))
    for t, o in keys:
        if t != cur_test:
            flush()
            d = t.rsplit("/", 1)[0] + "/" if "/" in t else ""
            if d != cur_dir:
                lines.append("@" + d)
                cur_dir = d
            cur_test, toks = t, []
        cls = "".join(CODES.get(m.get((t, o), ""), ".") if (t, o) in m else "." for m in by)
        toks.append(code[o] + cls)
    flush()
    with open(out, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    return len(keys)


def read_baseline(path, host):
    """The rows of one host of a baseline file, as read_tsv's dicts."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    if not lines or lines[0] != BASELINE_MAGIC:
        sys.exit(f"{path}: not a gxx-dg baseline")
    meta = {}
    for ln in lines:
        if ln.startswith("#") and " " in ln and not ln.startswith("# "):
            k, v = ln[1:].split(" ", 1)
            meta[k] = v
    hosts = meta.get("hosts", "").split()
    if host not in hosts:
        sys.exit(f"{path}: no host {host!r} (hosts: {' '.join(hosts)})")
    hi = hosts.index(host)
    opt = dict(kv.split("=", 1) for kv in meta.get("opts", "").split())
    rows, d = [], ""
    for ln in lines[1:]:
        if not ln or ln.startswith("#"):
            continue
        if ln.startswith("@"):
            d = ln[1:]
            continue
        name, toks = ln.split("\t", 1)
        for tok in toks.split(" "):
            c = tok[1 + hi]
            if c == ".":
                continue
            row = dict.fromkeys(HEADER, "")
            row.update(test=d + name, opt=opt[tok[0]], kiln=CLASS_OF[c])
            rows.append(row)
    return "\t".join(HEADER), rows


def read_tsv(path):
    """The header and the rows (dicts) of one TSV, or of one host of a
    baseline file (FILE@HOST)."""
    if "@" in path:
        file, host = path.rsplit("@", 1)
        return read_baseline(file, host)
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
    u = sum(1 for r in app if r["kiln"] in UNDECIDED)
    base = len(app) - u
    pct = 100.0 * p / base if base else 0.0
    return (f"{tests} tests, {len(rows)} rows, {len(app)} applicable cases; "
            f"{p} of {base} decided cases pass ({pct:.1f}%), {u} expect-unknown")


def bucket(r):
    """The runner's triage bucket of a case kiln fails, else None."""
    k, clang = r["kiln"], r["clang"]
    has_clang = clang not in ("", "-")
    if k in ("pass", "not-run", "skip") or k in UNDECIDED:
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
    ok = ("pass", "skip") + UNDECIDED
    regress = [k for k in both if b[k]["kiln"] == "pass" and f[k]["kiln"] not in ok]
    fixed = [k for k in both if b[k]["kiln"] not in ok and f[k]["kiln"] == "pass"]
    between = sum(1 for k in both if b[k]["kiln"] != f[k]["kiln"]
                  and b[k]["kiln"] not in ok and f[k]["kiln"] not in ok)
    undecided = sum(1 for k in both if (b[k]["kiln"] in UNDECIDED) != (f[k]["kiln"] in UNDECIDED))
    changed = sum(1 for k in both if b[k]["kiln"] != f[k]["kiln"])
    print(f"\npass -> failing: {len(regress)}; failing -> pass: {len(fixed)}; "
          f"between two failing classes: {between}; into or out of expect-unknown: {undecided}; "
          f"class changes: {changed}\n")
    explained = read_explained(a.explained) if a.explained else {}
    print(f"### pass in {labels[0]}, failing in {labels[1]} ({len(regress)})\n")
    for k in regress:
        r = f[k]
        why = explained.get(k)
        tag = f" EXPLAINED: {why}" if why is not None else ""
        print(f"- {k[0]} [{k[1]}] {r['kiln']}: {r['kiln_detail']} "
              f"(gcc {r['gcc'] or '-'}, clang {r['clang'] or '-'}){tag}")
    if a.gate:
        bad = [k for k in regress if k not in explained]
        print(f"\ngate: {len(regress) - len(bad)} explained, {len(bad)} unexplained "
              f"pass -> failing; {len(f)} cases run: {'pass' if not bad else 'FAIL'}")
        if bad:
            sys.exit(1)
    if a.list:
        print(f"\n### failing in {labels[0]}, pass in {labels[1]} ({len(fixed)})\n")
        for k in fixed:
            print(f"- {k[0]} [{k[1]}] was {b[k]['kiln']}: {b[k]['kiln_detail']}")


def read_explained(path):
    """{(test, opt): reason} from lines `test [opt] reason`."""
    out = {}
    with open(path, encoding="utf-8") as f:
        for n, ln in enumerate(f, 1):
            ln = ln.split("#", 1)[0].strip()
            if not ln:
                continue
            test, _, rest = ln.partition(" [")
            opt, sep, why = rest.partition("]")
            if not sep:
                sys.exit(f"{path}:{n}: expected `test [opt] reason`")
            out[(test, opt)] = why.strip()
    return out


def cmd_baseline(a):
    runs = []
    for spec in a.runs:
        host, sep, path = spec.partition("=")
        if not sep:
            sys.exit(f"baseline: {spec!r} is not HOST=RUN.tsv")
        runs.append((host, read_tsv(path)[1]))
    n = write_baseline(a.out, a.commit, runs)
    print(f"baseline: {n} cases, hosts {' '.join(h for h, _ in runs)} -> {a.out}")


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
    c.add_argument("--explained", help="the explained pass-to-failing cases (`test [opt] reason`)")
    c.add_argument("--gate", action="store_true",
                   help="exit 1 on a pass-to-failing case not in --explained")
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
    b = sub.add_parser("baseline", help="the reduced baseline of one run per host")
    b.add_argument("--out", required=True)
    b.add_argument("--commit", required=True, help="the kiln commit the runs were made at")
    b.add_argument("runs", nargs="+", metavar="HOST=RUN.tsv")
    b.set_defaults(fn=cmd_baseline)
    a = ap.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
