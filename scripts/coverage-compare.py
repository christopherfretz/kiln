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

Totals (functions, counters, counted executions; gcov executable and
executed lines) are printed per program and mode. Exit 0 when every
comparison is equal, 1 otherwise.

  scripts/coverage-compare.py [--kiln PATH] [--clang clang++-19]
                              [--mode instr|gcov|both] [files...]

The profile runtime is the oracle LLVM's compiler-rt
(`<llvm>/lib/clang/<N>/lib/linux/libclang_rt.profile-<arch>.a`), which
the link (`clang++`) adds for both compilers. `-fcoverage-mapping` (and so
`llvm-cov export -summary-only` region totals) is not compared: kiln does
not emit the mapping yet (WP-941 follow-up).
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

# (program, mode) -> why kiln differs today; reported, not failed.
# Empty: every program compares equal (an entry that becomes equal fails).
KNOWN: dict = {}


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


def build_and_run(cc, extra, src, work, flags, env):
    exe = work / "a.out"
    obj = work / f"{src.stem}.o"
    run([cc, "-std=c++26", "-O0", *extra, *flags, "-c", str(src), "-o", str(obj)], work)
    run([cc, *flags, str(obj), "-o", str(exe)], work)
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


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--kiln", default=default_kiln())
    ap.add_argument("--clang", default="clang++-19")
    ap.add_argument("--llvm", default="19", help="LLVM major of llvm-profdata / llvm-cov")
    ap.add_argument("--mode", choices=["instr", "gcov", "both"], default="both")
    ap.add_argument("--keep", action="store_true", help="keep the build directories")
    ap.add_argument("files", nargs="*")
    args = ap.parse_args()
    files = [Path(f).resolve() for f in args.files]
    files = files or sorted((REPO / "scripts/coverage").glob("*.cpp"))
    prof = tool("llvm-profdata", args.llvm)
    cov = tool("llvm-cov", args.llvm)
    compat = re.search(r"(\d+)", Path(args.clang).name)
    kextra = ["-ftrivial-auto-var-init=uninitialized"]
    if compat:
        kextra.append(f"-fclang-compat={compat.group(1)}")
    compilers = [("kiln", args.kiln, kextra), ("clang", args.clang, [])]
    scratch = tempfile.mkdtemp(prefix="coverage-compare-")
    args.scratch = scratch
    ok = True
    try:
        for src in files:
            print(src.name)
            for mode, fn, t in (("instr", compare_instr, prof), ("gcov", compare_gcov, cov)):
                if args.mode not in (mode, "both"):
                    continue
                equal = fn(src.name, src, compilers, t, args)
                why = KNOWN.get((src.name, mode))
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
