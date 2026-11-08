#!/usr/bin/env python3
"""oss-report.py [--root DIR] [--update FILE] [--perf [--top N]] [--stdlib libc++]
                     [project ...]

Reads .oss/build/<project>/{kiln,clang}/report.json and compile.jsonl (written
by oss-build.sh and kiln-cxx) and prints, per project, a Markdown table (kiln
vs clang: TUs, links, tests, wall, peak TU RSS) and kiln's failure clusters.
With --update FILE (docs/status/oss.md) it rewrites each project's section
between `<!-- oss:<project> -->` and `<!-- /oss:<project> -->` and the
projects table between `<!-- oss:projects -->` and `<!-- /oss:projects -->`.
With --perf each section also gets the per-TU kiln/clang time and peak-RSS
comparison (spec/landed/harness/WP-792.md); --top N lists the N heaviest
and the N slowest TUs (default 5, WP-812). With --stdlib libc++ it reads
the libc++ builds, .oss/build/<project>/{kiln,clang}-libcxx/ (WP-694): the
sections are `<!-- oss:<project>:libcxx -->` blocks and the statuses go to
the projects table's `libc++` column. The projects table is regenerated only
in a file that already has its markers. The cluster signature rule is the
contract in spec/landed/harness/WP-666.md.
"""

import argparse
import json
import math
import os
import re
import shlex
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.dont_write_bytecode = True
sys.path.insert(0, HERE)
import projects  # noqa: E402

COMPILERS = ("kiln", "clang")

# --- the cluster signature (spec/landed/harness/WP-666.md "Clusters") ---
LEAD_LOC_RE = re.compile(r"^\s*[^\s:]+:(?:\d+(?::\d+)?|\([^)]*\)):\s*")
QUOTE_RE = re.compile(r"'[^']*'|\"[^\"]*\"|‘[^’]*’|`[^'`]*'")
OBJ_RE = re.compile(r"[^\s:]+\.(?:o|obj|a)\b")
PATH_RE = re.compile(r"(?:[\w.+~-]*/)+([\w.+~-]+)")
NUM_RE = re.compile(r"\d+")
WS_RE = re.compile(r"\s+")


def normalize(diag):
    s = LEAD_LOC_RE.sub("", diag or "", count=1)
    s = QUOTE_RE.sub("Q", s)
    s = OBJ_RE.sub("O", s)
    s = PATH_RE.sub(r"\1", s)
    s = NUM_RE.sub("N", s)
    return WS_RE.sub(" ", s).strip()


def signature(rec):
    return f"{rec.get('kind', 'other')}: {normalize(rec.get('diag', ''))}".rstrip(": ").rstrip()


# --- loading ---
STDLIBS = {"libstdc++": "", "libc++": "-libcxx"}


def load_project(root, name, sfx=""):
    out = {}
    for c in COMPILERS:
        d = os.path.join(root, "build", name, c + sfx)
        rep = None
        try:
            with open(os.path.join(d, "report.json"), encoding="utf-8") as f:
                rep = json.load(f)
        except FileNotFoundError:
            pass
        out[c] = (rep, projects.read_jsonl(os.path.join(d, "compile.jsonl")))
    return out


def failures(recs):
    return [r for r in recs if r.get("phase") == "build" and r.get("source") != "none" and r.get("kind") != "ok"]


def clusters(kiln_recs, clang_recs):
    # The two builds share the source tree but not the build dir: match TUs
    # by source path (links by output name).
    def key(r):
        return r.get("output") if r.get("source") == "link" else r.get("source")

    clang_failed = {key(r) for r in failures(clang_recs)}
    groups = {}
    for r in failures(kiln_recs):
        groups.setdefault(signature(r), []).append(r)
    out = []
    for sig, rs in groups.items():
        also = sum(1 for r in rs if key(r) in clang_failed)
        out.append((sig, rs, also))
    out.sort(key=lambda t: (-len(t[1]), t[0]))
    return out


# --- rendering ---
def rel(text, base):
    """Spell the kiln checkout (the parent of the .oss root) as $KILN."""
    return text.replace(base + "/", "$KILN/") if base and text else text


def repro(rec, base):
    argv = list(rec.get("argv") or [])
    if argv:
        argv[0] = os.path.basename(argv[0])
    return rel(f"cd {shlex.quote(rec.get('cwd', '.'))} && {shlex.join(argv)}", base)


def cell_rc(rep, key):
    if rep is None:
        return "-"
    v = rep.get(key)
    return "not run" if v is None else ("ok" if v == 0 else f"rc {v}")


def cell_tus(rep, key):
    if rep is None:
        return "-"
    t = rep.get(key) or {}
    return f"{t.get('ok', 0)} / {t.get('failed', 0)}"


def cell_tests(rep):
    if rep is None or rep.get("test_rc") is None:
        return "-"
    t = rep.get("tests")
    if t:
        return f"{t['passed']} / {t['total']}"
    return "ok" if rep["test_rc"] == 0 else f"rc {rep['test_rc']}"


def cell_wall(rep):
    return "-" if rep is None else f"{rep.get('wall_s', 0):.0f} s"


def cell_rss(rep):
    if rep is None or not rep.get("peak_tu_rss_kb"):
        return "-"
    return f"{rep['peak_tu_rss_kb'] / 1024:.0f} MB"


def control_name(rep):
    """The control column's name: `clang-NN` or `gcc-NN` from the control
    report's compiler path (a toml `control_cxx`), else clang-19."""
    path = (rep or {}).get("compiler_path", "")
    m = re.search(r"(clang|g)\+\+-(\d+)$", path)
    if not m:
        return "clang-19"
    return f"{'clang' if m.group(1) == 'clang' else 'gcc'}-{m.group(2)}"


def render(name, data, base, stdlib="libstdc++"):
    kr, kl = data["kiln"]
    cr, cl = data["clang"]
    tag = (kr or cr or {}).get("tag", "?")
    lines = [f"### {name} ({tag}){', libc++' if stdlib == 'libc++' else ''}", ""]
    lines.append(f"| | kiln | {control_name(cr)} |")
    lines.append("|---|---|---|")
    lines.append(f"| configure | {cell_rc(kr, 'configure_rc')} | {cell_rc(cr, 'configure_rc')} |")
    lines.append(f"| build | {cell_rc(kr, 'build_rc')} | {cell_rc(cr, 'build_rc')} |")
    lines.append(f"| TUs ok / failed | {cell_tus(kr, 'tus')} | {cell_tus(cr, 'tus')} |")
    lines.append(f"| links ok / failed | {cell_tus(kr, 'links')} | {cell_tus(cr, 'links')} |")
    lines.append(f"| tests passed / total | {cell_tests(kr)} | {cell_tests(cr)} |")
    lines.append(f"| wall | {cell_wall(kr)} | {cell_wall(cr)} |")
    lines.append(f"| peak TU RSS | {cell_rss(kr)} | {cell_rss(cr)} |")
    lines.append("")
    cs = clusters(kl, cl)
    if not cs:
        lines.append("Failure clusters (kiln): none.")
    else:
        lines.append(f"Failure clusters (kiln): {len(cs)}.")
        lines.append("")
        for i, (sig, rs, also) in enumerate(cs, 1):
            r = rs[0]
            note = f"; {also} also fail{'s' if also == 1 else ''} with clang" if also else ""
            lines.append(f"{i}. **{len(rs)}x** `{sig}`{note}")
            lines.append(f"   - e.g. `{rel(r.get('source') or '', base)}`: `{rel(r.get('diag', ''), base)}`")
            lines.append(f"   - repro: `{repro(r, base)}`")
    if kr and kr.get("test_rc") not in (None, 0) and kr.get("test_summary"):
        lines.append("")
        lines.append(f"kiln test run: `{kr['test_summary']}`")
    return "\n".join(lines) + "\n", cs


def status_of(data, cs):
    kr = data["kiln"][0]
    cr = data["clang"][0]
    if kr is None:
        return None
    if kr.get("configure_rc") != 0:
        return "configuring"
    # A cluster whose every TU also fails with the control does not block.
    cs = [c for c in cs if c[2] < len(c[1])]
    if cs:
        return f"blocked ({len(cs)} cluster{'s' if len(cs) != 1 else ''})"
    control_failed = cr is not None and cr.get("build_rc") not in (0, None)
    if (kr.get("build_rc") not in (0, None) and not control_failed) or kr.get("test_rc") is None:
        return "building"
    kt, ct = kr.get("tests"), (cr or {}).get("tests")
    if kt and ct:
        if kt["passed"] >= ct["passed"] and kt["total"] == ct["total"]:
            return "tests at parity"
        return f"blocked (tests {kt['passed']}/{kt['total']} vs clang {ct['passed']}/{ct['total']})"
    if kr.get("test_rc") == 0 and (cr is None or cr.get("test_rc") == 0):
        return "tests at parity"
    return "blocked (tests)"


# --- --perf (spec/landed/harness/WP-792.md) ---
PERF_SLOW = 1.2
PERF_FLOOR_S = 0.5  # below this clang time, process start-up dominates the ratio


def ok_tus(recs):
    """source -> the last ok build-phase TU record of that source."""
    out = {}
    for r in recs:
        if r.get("phase") == "build" and projects.is_tu(r) and r.get("kind") == "ok":
            out[r["source"]] = r
    return out


def geomean(xs):
    xs = [x for x in xs if x > 0]
    return math.exp(sum(math.log(x) for x in xs) / len(xs)) if xs else 0.0


def perf_rows(kiln_recs, clang_recs):
    """(metric, rows): one row per TU that compiled with both compilers,
    (source, clang s, kiln s, clang KB, kiln KB). Time is CPU seconds when
    both logs carry `cpu_s`, else wall seconds."""
    k, c = ok_tus(kiln_recs), ok_tus(clang_recs)
    common = sorted(set(k) & set(c))
    cpu = bool(common) and all("cpu_s" in k[s] and "cpu_s" in c[s] for s in common)
    t = "cpu_s" if cpu else "wall_s"
    rows = [
        (s, max(c[s].get(t, 0), 0.001), max(k[s].get(t, 0), 0.001), c[s].get("max_rss_kb", 0) or 1, k[s].get("max_rss_kb", 0) or 1)
        for s in common
    ]
    return ("CPU" if cpu else "wall"), rows


def render_perf(kiln_recs, clang_recs, base, control="clang-19", top=5):
    metric, rows = perf_rows(kiln_recs, clang_recs)
    if not rows:
        return "Performance: no TU compiled with both compilers.\n"
    short = lambda s: rel(s, base)  # noqa: E731
    gt = geomean([kt / ct for _, ct, kt, _, _ in rows])
    gr = geomean([kr / cr for _, _, _, cr, kr in rows])
    tk, tc = sum(r[2] for r in rows), sum(r[1] for r in rows)
    pk, pc = max(r[4] for r in rows), max(r[3] for r in rows)
    lines = [
        f"Performance over the {len(rows)} TUs that compiled with both ({metric} time, peak RSS of the compiler and its backend):",
        f"geomean kiln/{control} time **{gt:.2f}x**, RSS **{gr:.2f}x**; total {tk:.0f} s vs {tc:.0f} s; peak TU RSS {pk / 1024:.0f} MB vs {pc / 1024:.0f} MB.",
        "",
        f"| heaviest TUs (by {control} time) | {control} s | kiln s | time x | {control} MB | kiln MB | RSS x |",
        "|---|---|---|---|---|---|---|",
    ]
    for s, ct, kt, cr, kr in sorted(rows, key=lambda r: -r[1])[:top]:
        lines.append(f"| `{short(s)}` | {ct:.1f} | {kt:.1f} | {kt / ct:.2f} | {cr / 1024:.0f} | {kr / 1024:.0f} | {kr / cr:.2f} |")
    big = [r for r in rows if r[1] >= PERF_FLOOR_S]
    slow = [r for r in big if r[2] / r[1] > PERF_SLOW or r[4] / r[3] > PERF_SLOW]
    lines.append("")
    lines.append(f"TUs over {PERF_SLOW}x on time or RSS: {len(slow)} of the {len(big)} that take {control} at least {PERF_FLOOR_S} s.")
    for s, ct, kt, cr, kr in sorted(slow, key=lambda r: -max(r[2] / r[1], r[4] / r[3]))[:top]:
        lines.append(f"- `{short(s)}`: time {kt:.1f} / {ct:.1f} s = {kt / ct:.2f}x, RSS {kr / 1024:.0f} / {cr / 1024:.0f} MB = {kr / cr:.2f}x")
    return "\n".join(lines) + "\n"


# --- --update ---
def replace_block(text, key, body):
    begin, end = f"<!-- oss:{key} -->", f"<!-- /oss:{key} -->"
    block = f"{begin}\n{body.rstrip()}\n{end}"
    pat = re.compile(re.escape(begin) + r".*?" + re.escape(end), re.S)
    if pat.search(text):
        return pat.sub(lambda _m: block, text, count=1)
    return text.rstrip("\n") + "\n\n" + block + "\n"


def old_statuses(text):
    """project -> (status, libc++ status or None) from the projects table."""
    m = re.search(r"<!-- oss:projects -->(.*?)<!-- /oss:projects -->", text, re.S)
    out = {}
    for line in (m.group(1) if m else "").splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) >= 4 and cells[0] not in ("project", "") and not cells[0].startswith("---"):
            out[cells[0].strip("`")] = (cells[3], cells[4] if len(cells) >= 5 else None)
    return out


def projects_table(toml, statuses):
    """statuses: project -> (status, libc++ status or None)."""
    lines = ["| project | wave | tag | status | libc++ |", "|---|---|---|---|---|"]
    for name, p in sorted(toml.items(), key=lambda kv: (kv[1]["wave"], kv[0])):
        st, lst = statuses.get(name, (None, None))
        lines.append(f"| `{name}` | {p['wave']} | `{p['tag']}` | {st or 'not started'} | {lst or 'not run'} |")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("projects", nargs="*")
    ap.add_argument("--root", default=os.environ.get("KILN_OSS_ROOT") or os.path.join(HERE, "..", "..", ".oss"))
    ap.add_argument("--toml", default=projects.TOML)
    ap.add_argument("--update", metavar="FILE")
    ap.add_argument("--perf", action="store_true", help="add the per-TU kiln/clang time and RSS comparison")
    ap.add_argument("--top", type=int, default=5, metavar="N", help="with --perf, list the N heaviest and N slowest TUs (default 5)")
    ap.add_argument("--stdlib", choices=tuple(STDLIBS), default="libstdc++", help="which builds to read (WP-694)")
    a = ap.parse_args()
    sfx = STDLIBS[a.stdlib]
    root = os.path.abspath(a.root)
    base = os.path.dirname(root)
    names = a.projects
    if not names:
        bdir = os.path.join(root, "build")
        names = sorted(
            d for d in (os.listdir(bdir) if os.path.isdir(bdir) else [])
            if any(os.path.exists(os.path.join(bdir, d, c + sfx, "report.json")) for c in COMPILERS)
        )
    if not names:
        sys.exit(f"oss-report: no reports under {root}/build")
    sections, statuses = {}, {}
    for n in names:
        data = load_project(root, n, sfx)
        if data["kiln"][0] is None and data["clang"][0] is None:
            print(f"oss-report: no {a.stdlib} report for {n} under {root}/build/{n}", file=sys.stderr)
            continue
        md, cs = render(n, data, base, a.stdlib)
        if a.perf:
            md += "\n" + render_perf(data["kiln"][1], data["clang"][1], base, control_name(data["clang"][0]), a.top)
        sections[n] = md
        st = status_of(data, cs)
        if st:
            statuses[n] = st
    print("\n".join(sections.values()), end="")
    if a.update:
        with open(a.update, encoding="utf-8") as f:
            text = f.read()
        if "<!-- oss:projects -->" in text:
            merged = old_statuses(text)
            col = 1 if sfx else 0
            for n, st in statuses.items():
                row = list(merged.get(n, (None, None)))
                row[col] = st
                merged[n] = tuple(row)
            text = replace_block(text, "projects", projects_table(projects.load(a.toml), merged))
        for n, md in sections.items():
            text = replace_block(text, n + (":libcxx" if sfx else ""), md)
        with open(a.update, "w", encoding="utf-8") as f:
            f.write(text)
        print(f"oss-report: updated {a.update}", file=sys.stderr)


if __name__ == "__main__":
    main()
