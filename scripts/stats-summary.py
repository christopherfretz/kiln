#!/usr/bin/env python3
"""stats-summary.py <log.jsonl>... [--label NAME]

Summarizes the compile statistics log the driver appends to under
KILN_STATS_LOG=<path> (spec/driver.md "Compile statistics log", WP-1370):
per `opt` trigger set, the jobs, their `opt`, `llc` and total time, the
share of compile time spent in `opt`, and for the jobs whose only trigger
is `alwaysinline` the `available_externally` count and the distribution of
`alwaysinline_defs` and of the `opt` time; then the same for the `-g` jobs.
Plain text (aligned columns), no dependencies. Link lines (`input` null)
are counted apart; `opt/total` is the share of the jobs' own time (the
link excluded).
"""

import json
import sys


def pct(xs, p):
    if not xs:
        return 0
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(p * (len(xs) - 1) + 0.5))]


def fmt_ms(ms):
    return f"{ms / 1000:.1f} s" if ms >= 1000 else f"{ms:.0f} ms"


def table(rows, header):
    w = [max(len(str(r[i])) for r in [header] + rows) for i in range(len(header))]
    out = []
    for r in [header] + rows:
        cells = [str(c).ljust(w[i]) if i == 0 else str(c).rjust(w[i]) for i, c in enumerate(r)]
        out.append("  ".join(cells).rstrip())
    return "\n".join(out)


def summarize(jobs, title):
    out = [title]
    if not jobs:
        out.append("  (no jobs)")
        return "\n".join(out)
    total = sum(j["total_ms"] for j in jobs)
    opt = sum(j["children_ms"].get("opt", 0) for j in jobs)
    levels = {}
    for j in jobs:
        levels[j["opt_level"]] = levels.get(j["opt_level"], 0) + 1
    out.append(f"  jobs: {len(jobs)}   levels: " + ", ".join(f"-O{k}: {v}" for k, v in sorted(levels.items())))
    out.append(f"  failed jobs (exit 1): {sum(1 for j in jobs if j['exit'])}")
    out.append(f"  total compile time: {fmt_ms(total)}   in opt: {fmt_ms(opt)} ({100 * opt / total if total else 0:.1f}%)")
    sets = {}
    for j in jobs:
        sets.setdefault("{" + ",".join(j["triggers"]) + "}", []).append(j)
    rows = []
    for k, js in sorted(sets.items(), key=lambda kv: -len(kv[1])):
        t = sum(j["total_ms"] for j in js)
        o = sum(j["children_ms"].get("opt", 0) for j in js)
        l = sum(j["children_ms"].get("llc", 0) for j in js)
        rows.append([k, len(js), f"{100 * len(js) / len(jobs):.1f}%", fmt_ms(o), fmt_ms(l), fmt_ms(t),
                     f"{100 * o / t if t else 0:.1f}%"])
    out.append("")
    out.append(table(rows, ["trigger set", "jobs", "of jobs", "opt", "llc", "total", "opt/total"]))
    ai = [j for j in jobs if j["triggers"] == ["alwaysinline"]]
    out.append("")
    out.append(f"  {{alwaysinline}}-only jobs: {len(ai)}")
    if ai:
        ae = sum(1 for j in ai if (j["available_externally_defs"] or 0) > 0)
        out.append(f"    with available_externally_defs > 0: {ae}")
        defs = [j["alwaysinline_defs"] or 0 for j in ai]
        calls = [j["alwaysinline_callsites"] or 0 for j in ai]
        opts = [j["children_ms"].get("opt", 0) for j in ai]
        share = [100 * j["children_ms"].get("opt", 0) / j["total_ms"] for j in ai if j["total_ms"]]
        rows = [
            ["alwaysinline_defs", pct(defs, 0.5), pct(defs, 0.9), max(defs)],
            ["alwaysinline_callsites", pct(calls, 0.5), pct(calls, 0.9), max(calls)],
            ["opt ms per job", f"{pct(opts, 0.5):.1f}", f"{pct(opts, 0.9):.1f}", f"{max(opts):.1f}"],
            ["opt % of job total", f"{pct(share, 0.5):.1f}", f"{pct(share, 0.9):.1f}", f"{max(share):.1f}"],
        ]
        out.append(table([["    " + r[0]] + r[1:] for r in rows], ["    distribution", "median", "p90", "max"]))
    return "\n".join(out)


def main(argv):
    label = None
    paths = []
    it = iter(argv)
    for a in it:
        if a == "--label":
            label = next(it, None)
        else:
            paths.append(a)
    if not paths:
        print(__doc__.strip().splitlines()[0], file=sys.stderr)
        return 2
    jobs, links, bad = [], [], 0
    for p in paths:
        with open(p, encoding="utf-8") as f:
            for line in f:
                try:
                    j = json.loads(line)
                except ValueError:
                    bad += 1
                    continue
                (links if j.get("input") is None else jobs).append(j)
    # A job with no module (-fsyntax-only, an error before IR) has no opt.
    with_ir = [j for j in jobs if j.get("ir_bytes") is not None]
    name = label or ", ".join(paths)
    print(summarize(with_ir, f"== {name}: jobs with a module"))
    print(f"  lines without a module: {len(jobs) - len(with_ir)}"
          f" (of them failed: {sum(1 for j in jobs if j.get('ir_bytes') is None and j['exit'])})"
          f"   link lines: {len(links)}"
          f"   link time: {fmt_ms(sum(l['children_ms'].get('clang++', 0) for l in links))}"
          f"   unparsable lines: {bad}")
    g = [j for j in with_ir if j["debug"]]
    if g:
        print()
        print(summarize(g, f"== {name}: -g jobs"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
