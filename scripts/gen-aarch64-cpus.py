#!/usr/bin/env python3
"""Generate crates/kiln_driver/src/aarch64_cpus.rs, the AArch64 `-mcpu=` CPU
table of clang 19 and clang 22 (spec/driver.md "Target features", WP-1151).

For every CPU either clang lists (`--print-supported-cpus`), and for each of
the two clangs that accepts it, one line of the `CPUS_19` / `CPUS_22` blob:

  name canonical arch driver closure keep

  canonical  the `-target-cpu` clang's driver passes (an alias's CPU)
  arch       the architecture feature clang's driver lists first (`v9a`)
  driver     the rest of the driver's `-target-feature` list for
             `-mcpu=<name>` (the CPU's extensions, LLVM's
             `ExtensionSet::addCPUDefaults` touched set), `+` dropped, minus
             the `+outline-atomics` / `-fmv` kiln adds itself
  closure    the features the function's `"target-features"` adds to them
             (`AArch64TargetInfo::setFeatureEnabled` of the architecture
             feature: the earlier levels and their defaults), minus the base
             `fp-armv8,neon,v8a,outline-atomics`
  keep       the features the driver lists for `-march=armv8-a -mcpu=<name>`
             beyond `-march=armv8-a`'s (clang 19's `+zcm,+zcz` of an Apple
             CPU, which `-march=` does not override)

Each list is `,`-separated, `-` when empty. Every feature is `+` (asserted).
The table is plain data (spec/landed/driver/WP-660.md): two `&str` blobs,
scanned by `aarch64_cpus::lookup`, so the loader relocates nothing per CPU.

It also generates crates/kiln_driver/src/aarch64_extensions.rs, the
`-march=` / `-mcpu=` `+ext` modifier table of each clang (WP-1167): one
line of the `EXTENSIONS_19` / `EXTENSIONS_22` blob per LLVM feature of
LLVM's `ExtensionSet` graph,

  feature names deps

  names      the `+name` / `+noname` modifiers naming it
             (`--print-supported-extensions`, plus the alias `rdma`; every
             LLVM feature name `llc -mattr=help` lists is tried as a
             modifier and must be one of these or rejected), `-` for a
             feature no modifier names (`chk`, `rcpc-immo`, `fptoint`)
  deps       the features it directly depends on (LLVM
             `ExtensionDependencies`): enabling it enables them, disabling
             one of them disables it

measured as: `-march=armv8-a+nofp+<name>` lists the modifier's feature
and its dependency closure (nothing else is on), whose transitive
reduction is `deps`; a feature no modifier names gets the dependency
whose `+no` modifier turns it off (`-march=<top>+<every name>+no<name>`
for the closures' ones, `-mcpu=<a CPU with it>+no<name>` for the CPUs').
The model (these edges plus LLVM's hard-coded `-crypto` and clang 22
`-sve2-<x>` disables, spec/driver.md "Target features") is then checked
against every measured `+nofp+<name>` closure, every `+no<name>` from
the all-on set and every CPU feature's `+no<name>` killers; a mismatch
fails the script.

Usage: scripts/gen-aarch64-cpus.py [clang++-19 [clang++-22]]
"""

import concurrent.futures
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/kiln_driver/src/aarch64_cpus.rs"
OUT_EXT = ROOT / "crates/kiln_driver/src/aarch64_extensions.rs"
TARGET = "--target=aarch64-linux-gnu"
DROP = {"+outline-atomics", "-fmv"}
BASE = {"fp-armv8", "neon", "v8a", "outline-atomics"}


def run(cxx, args, src=""):
    return subprocess.run([cxx, TARGET, *args, "-x", "c++", "-"], input=src,
                          capture_output=True, text=True)


def cpus(cxx):
    r = subprocess.run([cxx, TARGET, "--print-supported-cpus"], capture_output=True, text=True)
    out = r.stdout + r.stderr
    body = out.split("Available CPUs for this target:", 1)[1].split("Use -mcpu", 1)[0]
    return [l.strip() for l in body.splitlines() if l.strip()]


def driver(cxx, args):
    """(target-cpu, [features]) of `-###`, or None when the driver errors."""
    r = run(cxx, ["-###", "-c", *args])
    if "error:" in r.stderr:
        return None
    cc1 = [l for l in r.stderr.splitlines() if '"-cc1"' in l][0]
    toks = re.findall(r'"((?:[^"\\]|\\.)*)"', cc1)
    cpu = toks[toks.index("-target-cpu") + 1]
    feats = [toks[i + 1] for i, t in enumerate(toks) if t == "-target-feature"]
    return cpu, [f for f in feats if f not in DROP]


def ir_features(cxx, args):
    r = run(cxx, ["-S", "-emit-llvm", "-o", "-", *args], "void f(){}\n")
    m = re.search(r'"target-features"="([^"]*)"', r.stdout)
    return m.group(1).split(",") if m else []


def plain(feats):
    assert all(f.startswith("+") for f in feats), feats
    return [f[1:] for f in feats]


def row(cxx, name, march_base):
    d = driver(cxx, [f"-mcpu={name}"])
    if d is None:
        return None
    canonical, feats = d
    feats = plain(feats)
    archs = [f for f in feats if re.fullmatch(r"v[89](\.[0-9])?[ar]", f)]
    assert len(archs) == 1, (name, feats)
    arch, rest = archs[0], [f for f in feats if f != archs[0]]
    ir = plain([f for f in ir_features(cxx, [f"-mcpu={name}"]) if f != "-fmv"])
    closure = sorted(set(ir) - set(feats) - BASE)
    keep = driver(cxx, ["-march=armv8-a", f"-mcpu={name}"])[1]
    keep = plain([f for f in keep if f not in march_base])
    lists = [",".join(x) or "-" for x in (rest, closure, keep)]
    return " ".join([name, canonical, arch, *lists])


def table(cxx, names):
    march_base = driver(cxx, ["-march=armv8-a"])[1]
    with concurrent.futures.ThreadPoolExecutor(8) as ex:
        rows = ex.map(lambda n: row(cxx, n, march_base), names)
    return [r for r in rows if r is not None]


def version(cxx):
    r = subprocess.run([cxx, "--version"], capture_output=True, text=True)
    return r.stdout.splitlines()[0]


def blob(name, cxx, rows):
    lines = "\n".join(f"{r}" for r in rows)
    return (f"/// {version(cxx)}: `name canonical arch driver closure keep`, one CPU a line.\n"
            f"pub const {name}: &str = \"\\\n{lines}\n\";\n")


def ext_names(cxx):
    r = subprocess.run([cxx, TARGET, "--print-supported-extensions"], capture_output=True,
                       text=True)
    body = (r.stdout + r.stderr).split("Description", 1)[1]
    return [l.split()[0] for l in body.splitlines() if l.startswith("    ") and l.strip()]


def llc_features(cxx):
    llc = re.sub(r"clang\+\+", "llc", cxx)
    r = subprocess.run([llc, "-mtriple=aarch64", "-mattr=help"], capture_output=True, text=True)
    body = (r.stdout + r.stderr).split("Available features for this target:", 1)[1]
    body = body.split("Use +feature", 1)[0]
    return [l.split()[0] for l in body.splitlines() if l.strip()]


def is_arch(f):
    return re.fullmatch(r"v[89](\.[0-9])?[ar]", f) is not None


def on_off(feats):
    """The `+` and `-` features of a driver list, minus the architecture."""
    return ({f[1:] for f in feats if f[0] == "+" and not is_arch(f[1:])},
            {f[1:] for f in feats if f[0] == "-"})


class Model:
    """LLVM's `ExtensionSet` enable/disable over the derived graph."""

    def __init__(self, deps, v22):
        self.deps, self.v22 = deps, v22

    def enable(self, s, f):
        if f in s:
            return
        s.add(f)
        for d in sorted(self.deps.get(f, ())):
            self.enable(s, d)

    def disable(self, s, f):
        also = {"crypto": ["aes", "sha2", "sha3", "sm4"]}
        if self.v22:
            also.update({"sve2-aes": ["sve-aes"], "sve2-sha3": ["sve-sha3"],
                         "sve2-sm4": ["sve-sm4"], "sve2-bitperm": ["sve-bitperm", "sve2"]})
        for a in also.get(f, ()):
            self.disable(s, a)
        if f not in s:
            return
        s.remove(f)
        for g in sorted(self.deps):
            if f in self.deps[g]:
                self.disable(s, g)


def ext_table(cxx, v22, cpu_rows):
    names = ext_names(cxx) + ["rdma"]
    top = "armv9.7-a" if v22 else "armv9.5-a"
    cmd = "+".join(names)
    # Every LLVM feature name is a modifier only if clang lists it.
    for f in llc_features(cxx):
        if f not in names:
            assert driver(cxx, [f"-march=armv8-a+{f}"]) is None, (cxx, f)
    with concurrent.futures.ThreadPoolExecutor(8) as ex:
        on = dict(zip(names, ex.map(
            lambda n: on_off(driver(cxx, [f"-march=armv8-a+nofp+{n}"])[1])[0], names)))
        full = on_off(driver(cxx, [f"-march={top}+{cmd}"])[1])[0]
        off = dict(zip(names, ex.map(
            lambda n: on_off(driver(cxx, [f"-march={top}+{cmd}+no{n}"])[1])[1], names)))
    # The modifier's own feature: its name, or the one its closure adds to
    # every smaller closure's.
    prim = {}
    for n in names:
        if n in on[n]:
            prim[n] = n
            continue
        below = set().union(*[on[m] for m in names if on[m] < on[n]])
        (prim[n],) = on[n] - below
    named = set(prim.values())
    closure = {prim[n]: on[n] for n in names}
    # The features no modifier names, with the modifiers turning them off.
    hidden = sorted(set().union(*on.values()) - named)
    killers = {h: {prim[n] for n in names if h in off[n]} for h in hidden}
    cpu_hidden = {}
    for line in cpu_rows:
        w = line.split(" ")
        for f in w[3].split(","):
            if f != "-" and f not in named and not is_arch(f) and f not in cpu_hidden:
                cpu_hidden[f] = w[0]
    with concurrent.futures.ThreadPoolExecutor(8) as ex:
        jobs = [(h, c, n) for h, c in sorted(cpu_hidden.items()) for n in names]
        kills = ex.map(lambda j: f"-{j[0]}" in driver(cxx, [f"-mcpu={j[1]}+no{j[2]}"])[1], jobs)
        for (h, c, n), k in zip(jobs, kills):
            if k:
                killers.setdefault(h, set()).add(prim[n])
    # A hidden feature depends directly on the killer with the largest
    # closure among those whose closure (all of which kill it) it explains.
    deps = {}
    for h in sorted(killers):
        fits = [k for k in sorted(killers[h]) if closure[k] <= killers[h]]
        assert fits or not killers[h], (cxx, h, killers[h])
        best = max(fits, key=lambda k: len(closure[k]), default=None)
        closure[h] = {h} | (closure[best] if best else set())
    # Transitive reduction of every closure.
    for f, cl in sorted(closure.items()):
        rest = cl - {f}
        deps[f] = {d for d in rest if not any(d in closure.get(e, {e}) - {e} for e in rest - {d})}
    deps = {f: d for f, d in deps.items() if d}
    # Check the model against every measurement.
    m = Model(deps, v22)
    for n in names:
        s = set()
        m.enable(s, prim[n])
        assert s == on[n], (cxx, n, s ^ on[n])
        s = set(full)
        m.disable(s, prim[n])
        assert full - s == off[n], (cxx, n, (full - s) ^ off[n])
    for h in cpu_hidden:
        got = set()
        for p in named:
            s = set()
            m.enable(s, h)
            m.disable(s, p)
            if h not in s:
                got.add(p)
        assert got == killers.get(h, set()), (cxx, h, got, killers.get(h))
    nodes = sorted(named | set(deps) | set().union(*deps.values()))
    by = {}
    for n in sorted(names):
        by.setdefault(prim[n], []).append(n)
    rows = [f"{f} {','.join(by.get(f, [])) or '-'} {','.join(sorted(deps.get(f, ()))) or '-'}"
            for f in nodes]
    return rows, len(names)


def ext_blob(name, cxx, rows, count):
    lines = "\n".join(rows)
    return (f"/// {version(cxx)} ({count} modifiers): `feature names deps`, one feature a line.\n"
            f"pub const {name}: &str = \"\\\n{lines}\n\";\n")


def main():
    c19 = sys.argv[1] if len(sys.argv) > 1 else "clang++-19"
    c22 = sys.argv[2] if len(sys.argv) > 2 else "clang++-22"
    names = sorted(set(cpus(c19)) | set(cpus(c22)))
    t19, t22 = table(c19, names), table(c22, names)
    OUT.write_text(
        "// @generated by scripts/gen-aarch64-cpus.py; do not edit.\n"
        "//! The `AArch64` `-mcpu=` CPUs of clang 19 and clang 22 and their\n"
        "//! features (scripts/gen-aarch64-cpus.py documents the columns;\n"
        "//! spec/driver.md \"Target features\").\n\n"
        + blob("CPUS_19", c19, t19) + "\n" + blob("CPUS_22", c22, t22))
    print(f"{OUT}: {len(t19)} clang 19 CPUs, {len(t22)} clang 22 CPUs")
    (e19, n19), (e22, n22) = ext_table(c19, False, t19), ext_table(c22, True, t22)
    OUT_EXT.write_text(
        "// @generated by scripts/gen-aarch64-cpus.py; do not edit.\n"
        "//! The `AArch64` `-march=` / `-mcpu=` `+ext` modifiers of clang 19 and\n"
        "//! clang 22 and LLVM's feature dependencies (scripts/gen-aarch64-cpus.py\n"
        "//! documents the columns; spec/driver.md \"Target features\").\n\n"
        + ext_blob("EXTENSIONS_19", c19, e19, n19) + "\n"
        + ext_blob("EXTENSIONS_22", c22, e22, n22))
    print(f"{OUT_EXT}: {n19} clang 19 modifiers, {n22} clang 22 modifiers")


if __name__ == "__main__":
    main()
