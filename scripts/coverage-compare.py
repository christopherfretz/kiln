#!/usr/bin/env python3
"""Coverage comparison of kiln against clang (WP-941, spec/codegen.md
"Instrumentation").

Builds every program under scripts/coverage/ (or the files given) with
kiln++ and with the clang oracle, runs both binaries and compares what
the coverage tools report:

  instr  `-fprofile-instr-generate`: `llvm-profdata merge` of the run, then
         `llvm-profdata show --all-functions --counts`; every function's
         hash, counter count and counter values must be equal.
  gcov   `--coverage`: `llvm-cov gcov` of the run; every source line's
         execution count column must be equal (the header is not
         compared).
  map    `-fprofile-instr-generate -fcoverage-mapping` (WP-1003):
         `llvm-cov export -summary-only` of the run, then `llvm-cov show`;
         the function, line, region and branch totals (count and covered)
         of every non-system file and every line's execution count must be
         equal.

Totals (functions, counters, counted executions; gcov executable and
executed lines) are printed per program and mode. A (program, mode) pair
in `KNOWN` is an accepted, explained difference: it is reported, and it
fails once it becomes equal (drop the entry then). Exit 0 when every
other comparison is equal, 1 otherwise.

  scripts/coverage-compare.py [--kiln PATH] [--clang clang++-19]
                              [--mode instr|gcov|map|both|all] [files...]

The profile runtime is the oracle LLVM's compiler-rt
(`<llvm>/lib/clang/<N>/lib/linux/libclang_rt.profile-<arch>.a`), which
the link (`clang++`) adds for both compilers. Without it (a tiny link is
probed first) the script prints SKIP and exits 0. `--llvm 22` with
`--clang clang++-22` compares under `-fclang-compat=22` with the LLVM 22
tools.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# (program, mode) or (program, mode, LLVM major) -> why kiln differs today;
# reported, not failed. An entry that becomes equal fails.
KNOWN: dict = {
    ("signature_exprs.cpp", "instr"): "clang's MapRegionCounters also walks the "
    "written signature (return-type template arguments, decltype and noexcept "
    "operands), so those feed its hash and a noexcept `&&` takes a counter; kiln "
    "walks the body only (WP-941 follow-up)",
}


def norm_name(fn):
    """The unnamed-type discriminator `$_N` of a local lambda: kiln numbers
    them in a different order than clang (a mangling follow-up of WP-941),
    which renames but does not change a function's counters."""
    return re.sub(r"\$_\d+", "$_N", fn)


def tool(name, version):
    for cand in (f"{name}-{version}", f"/usr/lib/llvm-{version}/bin/{name}", name):
        if shutil.which(cand):
            return cand
    sys.exit(f"coverage-compare: {name} (LLVM {version}) not found")


def default_kiln():
    target = os.environ.get("CARGO_TARGET_DIR") or str(REPO / "target")
    return str(Path(target) / "debug" / "kiln++")


def run(cmd, cwd, env=None, check=True):
    e = dict(os.environ)
    if env:
        e.update(env)
    p = subprocess.run(cmd, cwd=cwd, env=e, capture_output=True, text=True)
    if check and p.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} exited {p.returncode}:\n{p.stderr[-2000:]}")
    return p


def profdata_functions(text):
    """{function: (hash, [counts])} from `llvm-profdata show --counts`."""
    funcs = {}
    name = None
    for line in text.splitlines():
        m = re.match(r"^  (\S.*):$", line)
        if m:
            name = m.group(1)
            funcs[name] = [None, []]
            continue
        if name is None:
            continue
        m = re.match(r"^    Hash: (0x[0-9a-f]+)", line)
        if m:
            funcs[name][0] = m.group(1)
        m = re.match(r"^    Block counts: \[(.*)\]", line)
        if m and m.group(1).strip():
            funcs[name][1] = [int(x) for x in m.group(1).split(",")]
        m = re.match(r"^    Function count: (\d+)", line)
        if m:
            funcs[name][1].insert(0, int(m.group(1)))
    out = {}
    for k, (h, c) in funcs.items():
        out.setdefault(norm_name(k), []).append((h, tuple(c)))
    return {k: sorted(v) for k, v in out.items()}


def gcov_lines(path):
    """[(line, count)] of a .gcov file, header (line 0) dropped."""
    out = []
    for line in path.read_text(errors="replace").splitlines():
        parts = line.split(":", 2)
        if len(parts) < 3:
            continue
        count, num = parts[0].strip(), parts[1].strip()
        if num == "0" or count == "-":
            continue
        out.append((int(num), count.rstrip("*")))
    return out


LINK = None


def build_and_run(cc, extra, src, work, flags, env):
    exe = work / "a.out"
    obj = work / f"{src.stem}.o"
    run([cc, "-std=c++26", "-O0", *extra, *flags, "-c", str(src), "-o", str(obj)], work)
    # kiln links with the oracle LLVM's clang (or --link).
    link = [LINK] if LINK else [cc, *(x for x in extra if x.startswith("--llvm-dir"))]
    run([*link, *flags, str(obj), "-o", str(exe)], work)
    run([str(exe)], work, env=env, check=False)
    return exe


def compare_instr(name, src, compilers, prof, args):
    res = {}
    for label, cc, extra in compilers:
        work = Path(tempfile.mkdtemp(prefix=f"cov-{label}-", dir=args.scratch))
        build_and_run(cc, extra, src, work, ["-fprofile-instr-generate"],
                      {"LLVM_PROFILE_FILE": str(work / "run.profraw")})
        run([prof, "merge", "-o", "run.profdata", "run.profraw"], work)
        out = run([prof, "show", "--all-functions", "--counts", "run.profdata"], work).stdout
        res[label] = profdata_functions(out)
        if not args.keep:
            shutil.rmtree(work)
    k, c = res["kiln"], res["clang"]
    ok = k == c
    for label, f in (("kiln", k), ("clang", c)):
        print(f"  instr {label:5}: functions {sum(len(v) for v in f.values())}, counters "
              f"{sum(len(c) for v in f.values() for _, c in v)}, executions "
              f"{sum(sum(c) for v in f.values() for _, c in v)}")
    if not ok:
        for fn in sorted(set(k) | set(c)):
            if k.get(fn) != c.get(fn):
                print(f"    {fn}: kiln {k.get(fn)} clang {c.get(fn)}")
    return ok


def compare_gcov(name, src, compilers, cov, args):
    res = {}
    for label, cc, extra in compilers:
        work = Path(tempfile.mkdtemp(prefix=f"gcov-{label}-", dir=args.scratch))
        build_and_run(cc, extra, src, work, ["--coverage"], {})
        run([cov, "gcov", "-o", str(work / f"{src.stem}.o"), str(src)], work)
        res[label] = gcov_lines(work / f"{src.name}.gcov")
        if not args.keep:
            shutil.rmtree(work)
    k, c = res["kiln"], res["clang"]
    for label, f in (("kiln", k), ("clang", c)):
        executed = sum(1 for _, n in f if n not in ("#####", "=====", "0"))
        print(f"  gcov  {label:5}: lines {len(f)}, executed {executed}")
    if k != c:
        ck = dict(c)
        for line, n in k:
            if ck.get(line) != n:
                print(f"    line {line}: kiln {n} clang {ck.get(line)}")
        for line in sorted(set(dict(c)) - set(dict(k))):
            print(f"    line {line}: kiln - clang {ck[line]}")
    return k == c


def cov_summary(text, root):
    """{file name: (functions, lines, regions, branches) as (count,
    covered)} of the files under `root` in `llvm-cov export
    -summary-only` JSON."""
    import json
    out = {}
    for f in json.loads(text)["data"][0]["files"]:
        if not f["filename"].startswith(str(root)):
            continue
        s = f["summary"]
        out[Path(f["filename"]).name] = tuple(
            (s[k]["count"], s[k]["covered"]) for k in ("functions", "lines", "regions", "branches"))
    return out


def show_lines(text):
    """[(file, line, count)] of `llvm-cov show` text: the lines with a
    count."""
    out, cur = [], None
    for line in text.splitlines():
        if line.startswith("/") and line.endswith(":"):
            cur = Path(line[:-1]).name
            continue
        m = re.match(r"^\s*(\d+)\|\s*([0-9.]+[kMGEP]?)\|", line)
        if m and cur:
            out.append((cur, int(m.group(1)), m.group(2)))
    return out


def compare_map(name, src, compilers, tools, args):
    prof, cov = tools
    res = {}
    for label, cc, extra in compilers:
        work = Path(tempfile.mkdtemp(prefix=f"map-{label}-", dir=args.scratch))
        exe = build_and_run(cc, extra, src, work,
                            ["-fprofile-instr-generate", "-fcoverage-mapping"],
                            {"LLVM_PROFILE_FILE": str(work / "run.profraw")})
        run([prof, "merge", "-o", "run.profdata", "run.profraw"], work)
        pd = ["-instr-profile=run.profdata", str(exe)]
        summ = run([cov, "export", "-summary-only", *pd], work).stdout
        show = run([cov, "show", "-show-instantiations=false", "-use-color=false", *pd], work)
        res[label] = (cov_summary(summ, src.parent), show_lines(show.stdout))
        if not args.keep:
            shutil.rmtree(work)
    (ks, kl), (cs, cl) = res["kiln"], res["clang"]
    for label, s in (("kiln", ks), ("clang", cs)):
        t = [tuple(sum(v[i][j] for v in s.values()) for j in (0, 1)) for i in range(4)]
        print(f"  map   {label:5}: functions {t[0][1]}/{t[0][0]}, lines {t[1][1]}/{t[1][0]}, "
              f"regions {t[2][1]}/{t[2][0]}, branches {t[3][1]}/{t[3][0]}")
    if ks != cs:
        for f in sorted(set(ks) | set(cs)):
            if ks.get(f) != cs.get(f):
                print(f"    {f}: kiln {ks.get(f)} clang {cs.get(f)}")
    if kl != cl:
        cd = {(f, n): c for f, n, c in cl}
        kd = {(f, n): c for f, n, c in kl}
        for key in sorted(set(cd) | set(kd)):
            if cd.get(key) != kd.get(key):
                print(f"    {key[0]}:{key[1]}: kiln {kd.get(key)} clang {cd.get(key)}")
    return ks == cs and kl == cl


def missing_profile_runtime(compilers, scratch):
    """The first link driver that cannot link `-fprofile-instr-generate`
    (no `libclang_rt.profile`; `--coverage` links the same runtime), with
    its error, or None. A tiny program is linked with each one."""
    work = Path(tempfile.mkdtemp(prefix="cov-probe-", dir=scratch))
    (work / "probe.cpp").write_text("int main() { return 0; }\n")
    for label, cc, extra in compilers:
        # The link command of build_and_run.
        link = [LINK] if LINK else [cc, *(x for x in extra if x.startswith("--llvm-dir"))]
        p = run([*link, "-fprofile-instr-generate", "probe.cpp", "-o", f"probe-{label}"],
                work, check=False)
        if p.returncode != 0:
            lines = p.stderr.splitlines()
            err = next((l for l in lines if "cannot find" in l),
                       next((l for l in lines if "error" in l), "link failed"))
            return f"{link[0]}: {err}"
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kiln", default=default_kiln())
    ap.add_argument("--clang", default="clang++-19")
    ap.add_argument("--llvm", default="19", help="LLVM major of llvm-profdata / llvm-cov")
    ap.add_argument("--mode", choices=["instr", "gcov", "map", "both", "all"], default="all",
                    help="`both` is instr and gcov; `all` adds map")
    ap.add_argument("--keep", action="store_true", help="keep the build directories")
    ap.add_argument("--link", help="the driver that links both (default: each compiler); "
                    "e.g. clang++-19 when LLVM 22's profile runtime is not installed")
    ap.add_argument("files", nargs="*")
    args = ap.parse_args()
    global LINK
    LINK = args.link
    files = [Path(f).resolve() for f in args.files]
    files = files or sorted((REPO / "scripts/coverage").glob("*.cpp"))
    prof = tool("llvm-profdata", args.llvm)
    cov = tool("llvm-cov", args.llvm)
    compat = re.search(r"(\d+)", Path(args.clang).name)
    kextra = ["-ftrivial-auto-var-init=uninitialized"]
    if compat:
        kextra.append(f"-fclang-compat={compat.group(1)}")
    if Path(f"/usr/lib/llvm-{args.llvm}").is_dir():
        kextra.append(f"--llvm-dir=/usr/lib/llvm-{args.llvm}")
    compilers = [("kiln", args.kiln, kextra), ("clang", args.clang, [])]
    scratch = tempfile.mkdtemp(prefix="coverage-compare-")
    args.scratch = scratch
    ok = True
    try:
        missing = missing_profile_runtime(compilers, scratch)
        if missing:
            print(f"SKIP: no profile runtime (libclang_rt.profile) to link with: {missing}")
            return 0
        for src in files:
            print(src.name)
            modes = (("instr", compare_instr, prof), ("gcov", compare_gcov, cov),
                     ("map", compare_map, (prof, cov)))
            for mode, fn, t in modes:
                if args.mode not in (mode, "all") and not (args.mode == "both" and mode != "map"):
                    continue
                equal = fn(src.name, src, compilers, t, args)
                why = KNOWN.get((src.name, mode, args.llvm)) or KNOWN.get((src.name, mode))
                if not equal and why:
                    print(f"  KNOWN {mode}: {why}")
                elif equal and why:
                    print(f"  {mode} now equal: drop its KNOWN entry")
                    ok = False
                else:
                    ok &= equal
    finally:
        if not args.keep:
            shutil.rmtree(scratch, ignore_errors=True)
    print("EQUAL" if ok else "DIFFERENT")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
