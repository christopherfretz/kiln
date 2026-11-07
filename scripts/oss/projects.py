#!/usr/bin/env python3
"""Helpers for the OSS build harness (spec/landed/harness/WP-666.md).

  projects.py list [--wave N]            project names, one per line
  projects.py shell <project>            the project's table as bash assignments
  projects.py report-json <args...>      write report.json for one build dir
"""

import argparse
import json
import os
import re
import shlex
import sys
import time
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
TOML = os.path.join(HERE, "projects.toml")

REQUIRED = ("url", "tag", "build", "cxx_std", "test", "wave")
BUILDS = ("cmake", "meson", "make")
KINDS = ("ok", "reject", "panic", "backend", "link", "timeout", "oom", "other")
CTEST_RE = re.compile(r"(\d+)% tests passed, (\d+) tests? failed out of (\d+)")
# lit's summary (LLVM's check-* targets): a total line, then one
# `  <Result> : N (p%)` line per non-empty result category.
LIT_TOTAL_RE = re.compile(r"^Total Discovered Tests: (\d+)")
LIT_LINE_RE = re.compile(r"^  ([A-Za-z ]+?)\s*:\s*(\d+)")
LIT_FAILED = ("Failed", "Unexpectedly Passed", "Timed Out")
# The per-project address-space cap may be raised to at most 6 GB virt
# (WP-782: a few tablegen'd LLVM TUs).
VMEM_MAX_KB = 6000000


def load(path=TOML):
    with open(path, "rb") as f:
        data = tomllib.load(f)
    for name, p in data.items():
        missing = [k for k in REQUIRED if k not in p]
        if missing:
            raise SystemExit(f"{path}: [{name}] lacks {', '.join(missing)}")
        if p["build"] not in BUILDS:
            raise SystemExit(f"{path}: [{name}] build must be one of {BUILDS}")
        p.setdefault("configure", [])
        p.setdefault("build_targets", [])
        p.setdefault("c_compiler", "clang-19")
        p.setdefault("verified", False)
        p.setdefault("submodules", False)
        p.setdefault("source_subdir", "")
        p.setdefault("notes", "")
        p.setdefault("deps", [])
        p.setdefault("install_configure", [])
        p.setdefault("test_timeout", 1800)
        p.setdefault("vmem_kb", 3000000)
        if not (isinstance(p["vmem_kb"], int) and 0 < p["vmem_kb"] <= VMEM_MAX_KB):
            raise SystemExit(f"{path}: [{name}] vmem_kb must be an integer in 1..{VMEM_MAX_KB}")
        if not (isinstance(p["test_timeout"], int) and p["test_timeout"] > 0):
            raise SystemExit(f"{path}: [{name}] test_timeout must be a positive integer")
    for name, p in data.items():
        bad = [d for d in p["deps"] if d not in data or data[d]["build"] != "cmake"]
        if bad:
            raise SystemExit(f"{path}: [{name}] deps must name cmake projects: {', '.join(bad)}")
    return data


def read_jsonl(path):
    """The wrapper's log: one JSON object per line; a torn line is skipped."""
    out = []
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    out.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
    except FileNotFoundError:
        pass
    return out


def is_tu(rec):
    return rec.get("source") not in (None, "link", "none")


def summarize(records):
    """TU and link counts of the build phase (spec: report.json `tus`, `links`)."""
    build = [r for r in records if r.get("phase") == "build"]
    tus = [r for r in build if is_tu(r)]
    links = [r for r in build if r.get("source") == "link"]
    by_kind = {}
    for r in tus:
        by_kind[r.get("kind", "other")] = by_kind.get(r.get("kind", "other"), 0) + 1
    peak = max(tus, key=lambda r: r.get("max_rss_kb", 0), default=None)
    return {
        "tus": {
            "total": len(tus),
            "ok": by_kind.get("ok", 0),
            "failed": len(tus) - by_kind.get("ok", 0),
            "by_kind": dict(sorted(by_kind.items())),
        },
        "links": {
            "total": len(links),
            "ok": sum(1 for r in links if r.get("kind") == "ok"),
            "failed": sum(1 for r in links if r.get("kind") != "ok"),
        },
        "peak_tu_rss_kb": peak.get("max_rss_kb", 0) if peak else 0,
        "peak_tu": peak.get("source") if peak else None,
        "max_tu_wall_s": max((r.get("wall_s", 0) for r in tus), default=0),
    }


def test_summary(log_path):
    """(summary line, {passed, failed, total} or None) of a test log."""
    try:
        with open(log_path, encoding="utf-8", errors="replace") as f:
            lines = [l.rstrip("\n") for l in f]
    except FileNotFoundError:
        return None, None
    for line in reversed(lines):
        m = CTEST_RE.search(line)
        if m:
            total, failed = int(m.group(3)), int(m.group(2))
            return line.strip(), {"passed": total - failed, "failed": failed, "total": total}
    lit = lit_summary(lines)
    if lit:
        return lit
    last = next((l.strip() for l in reversed(lines) if l.strip()), "")
    return last, None


def lit_summary(lines):
    """The last lit summary block of a log: (`Total Discovered Tests: T,
    passed P, failed F`, counts), or None. `passed` is every test that did
    not fail (Passed, Unsupported, Expectedly Failed, Skipped, ...);
    `failed` counts Failed, Unexpectedly Passed and Timed Out."""
    start = None
    for i in range(len(lines) - 1, -1, -1):
        if LIT_TOTAL_RE.match(lines[i]):
            start = i
            break
    if start is None:
        return None
    total = int(LIT_TOTAL_RE.match(lines[start]).group(1))
    failed = 0
    for line in lines[start + 1 :]:
        m = LIT_LINE_RE.match(line)
        if not m:
            break
        if m.group(1).strip() in LIT_FAILED:
            failed += int(m.group(2))
    passed = total - failed
    return (
        f"Total Discovered Tests: {total}, passed {passed}, failed {failed}",
        {"passed": passed, "failed": failed, "total": total},
    )


def cmd_report_json(a):
    recs = read_jsonl(os.path.join(a.build_dir, "compile.jsonl"))
    s = summarize(recs)
    summary, counts = (None, None)
    if a.test_rc is not None:
        summary, counts = test_summary(os.path.join(a.build_dir, "test.log"))
    p = load()[a.project]
    rep = {
        "schema": 1,
        "project": a.project,
        "compiler": a.compiler,
        "compiler_path": a.compiler_path,
        "url": p["url"],
        "tag": p["tag"],
        "cxx_std": p["cxx_std"],
        "date": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "configure_rc": a.configure_rc,
        "build_rc": a.build_rc,
        "test_rc": a.test_rc,
        "test_summary": summary,
        "tests": counts,
        "wall_s": round(a.wall, 1),
        **s,
    }
    out = os.path.join(a.build_dir, "report.json")
    with open(out, "w", encoding="utf-8") as f:
        json.dump(rep, f, indent=2)
        f.write("\n")
    print(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    l = sub.add_parser("list")
    l.add_argument("--wave", type=int)
    s = sub.add_parser("shell")
    s.add_argument("project")
    r = sub.add_parser("report-json")
    r.add_argument("--project", required=True)
    r.add_argument("--compiler", required=True)
    r.add_argument("--compiler-path", required=True)
    r.add_argument("--build-dir", required=True)
    r.add_argument("--configure-rc", type=int, required=True)
    r.add_argument("--build-rc", type=int)
    r.add_argument("--test-rc", type=int)
    r.add_argument("--wall", type=float, required=True)
    a = ap.parse_args()

    if a.cmd == "list":
        for name, p in load().items():
            if a.wave is None or p["wave"] == a.wave:
                print(name)
    elif a.cmd == "shell":
        data = load()
        if a.project not in data:
            sys.exit(f"projects.py: unknown project '{a.project}' (see {TOML})")
        p = data[a.project]
        q = shlex.quote
        for k in ("url", "tag", "build", "cxx_std", "test", "c_compiler", "source_subdir", "test_timeout", "vmem_kb"):
            print(f"P_{k.upper()}={q(str(p[k]))}")
        print(f"P_SUBMODULES={'1' if p['submodules'] else ''}")
        print("P_CONFIGURE=(" + " ".join(q(x) for x in p["configure"]) + ")")
        print("P_BUILD_TARGETS=(" + " ".join(q(x) for x in p["build_targets"]) + ")")
        print("P_DEPS=(" + " ".join(q(x) for x in p["deps"]) + ")")
        print("P_INSTALL_CONFIGURE=(" + " ".join(q(x) for x in p["install_configure"]) + ")")
    elif a.cmd == "report-json":
        cmd_report_json(a)


if __name__ == "__main__":
    main()
