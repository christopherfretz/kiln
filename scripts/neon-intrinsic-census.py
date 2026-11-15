#!/usr/bin/env python3
"""Census of the AArch64 intrinsic headers: compile one call of every
function-like intrinsic with kiln and with clang, and tabulate the outcome
(docs/status/neon-builtins.md).

The intrinsics are the function-like names of `arm_neon.h`, `arm_fp16.h`
and `arm_acle.h`, both kiln's resource copies (clang 19's) and clang's own
(the reference clang, default `clang++-22`); a name only clang's header has
is still probed, so a name kiln's header lacks shows as an error. For each
name one wrapper TU is generated:

    #include <arm_neon.h>
    __attribute__((target("<features>"))) auto w(T0 a0, ...) {
      return NAME(a0, ..., <imm>);
    }

The parameter types come from the header: a function's prototype, or a
macro's `T __sN = __pN;` locals; an untyped macro parameter is the element
pointer of a load/store (`vld*`/`vst*`, operand 0) and otherwise an integer
immediate. The features are the function's `target("...")` attribute plus
any clang names in a "requires/needs target feature" error; the immediates
are tried as 1, then 0, then mixtures, until clang accepts the wrapper.

Each wrapper is compiled with clang (`-S -emit-llvm`) and with kiln
(`-O0 -c`), both `--target=aarch64-unknown-linux-gnu -std=c++26`, objects
in the scratch directory. The outcome is one of:

    ok            kiln compiles it
    unsupported   kiln stops at "code generation for B is not supported yet"
    error         kiln fails otherwise (first error line recorded)
    clang-rejects clang rejects every variant too (not kiln's problem)

The table (TSV: name, header, outcome, builtin-or-message) is written to
`--out`; a summary, the unsupported builtins grouped by lowering shape with
the number of intrinsics each blocks, goes to stdout. Jobs draw tokens from
the make jobserver in MAKEFLAGS (`--jobserver-auth=fifo:PATH`) when there
is one, `-j` workers at most.

Usage: scripts/neon-intrinsic-census.py --kiln <kiln++> --scratch <dir>
           --out <table.tsv> [--clang clang++-22] [-j N] [--only REGEX]
"""

import argparse
import collections
import concurrent.futures
import os
import pathlib
import re
import signal
import subprocess
import sys
import threading

ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADERS = ["arm_neon.h", "arm_fp16.h", "arm_acle.h"]
TARGET = "--target=aarch64-unknown-linux-gnu"

ELT = {
    "s8": "int8_t", "s16": "int16_t", "s32": "int32_t", "s64": "int64_t",
    "u8": "uint8_t", "u16": "uint16_t", "u32": "uint32_t", "u64": "uint64_t",
    "p8": "poly8_t", "p16": "poly16_t", "p64": "poly64_t", "p128": "poly128_t",
    "f16": "float16_t", "bf16": "bfloat16_t", "f32": "float32_t",
    "f64": "float64_t", "mf8": "mfloat8_t",
}


class Intr:
    def __init__(self, name, header, ret, params, features):
        self.name = name
        self.header = header
        self.ret = ret
        # A list of C types; None: untyped macro parameter.
        self.params = params
        self.features = features


FN_NEON = re.compile(
    r'^__ai (?:__attribute__\(\(target\("([^"]*)"\)\)\) )?(.+?)\s*\b(\w+)\((.*?)\) \{', re.M
)
FN_ACLE = re.compile(
    r"^static __inline__ ([\w ]+?)\s*__attribute__\(\((.*?)\)\)\s*(\w+)\(([^)]*)\)\s*\{",
    re.M | re.S,
)
MACRO = re.compile(r"^#define (\w+)\(([^)]*)\)((?:.*\\\n)*.*)$", re.M)


def ptypes(params):
    out = []
    for p in params.split(","):
        p = p.strip()
        if not p or p == "void":
            continue
        out.append(re.sub(r"\s*\b__\w+$", "", p))
    return out


def parse(path, header):
    text = path.read_text()
    found = {}
    if header == "arm_acle.h":
        for m in FN_ACLE.finditer(text):
            attrs, name = m.group(2), m.group(3)
            t = re.search(r'target\("([^"]*)"\)', attrs)
            found.setdefault(name, Intr(name, header, m.group(1), ptypes(m.group(4)),
                                        t.group(1) if t else ""))
    else:
        for m in FN_NEON.finditer(text):
            name = m.group(3)
            found.setdefault(name, Intr(name, header, m.group(2), ptypes(m.group(4)),
                                        m.group(1) or ""))
    for m in MACRO.finditer(text):
        name, params, body = m.group(1), m.group(2), m.group(3)
        if name in found:
            continue
        ps = [p.strip() for p in params.split(",") if p.strip()]
        if any(not re.fullmatch(r"\w+", p) for p in ps):
            continue
        types = []
        for p in ps:
            t = re.search(r"(\w+) __s\w* = " + re.escape(p) + r";", body)
            types.append(t.group(1) if t else None)
        r = re.search(r"\b(\w+) __ret\w*;", body)
        found[name] = Intr(name, header, r.group(1) if r else None, types, "")
    keep = {}
    for name, i in found.items():
        if header != "arm_acle.h":
            if name.startswith("_") or name.startswith("splat"):
                continue
        elif not name.startswith("__") or name.startswith("__ARM") or name == "__ai":
            continue
        keep[name] = i
    return keep


def elt_ptr(name, const):
    base = re.sub(r"_x[234]$", "", name)
    sfx = base.rsplit("_", 1)[-1]
    t = ELT.get(sfx, "void")
    return ("const " if const else "") + t + " *"


def wrapper(i, features, imms):
    params, args, k = [], [], 0
    for n, t in enumerate(i.params):
        if t is None:
            if n == 0 and re.match(r"v(ld|st)", i.name):
                params.append(f"{elt_ptr(i.name, i.name.startswith('vld'))}a{n}")
                args.append(f"a{n}")
            else:
                args.append(str(imms[k] if k < len(imms) else 1))
                k += 1
        else:
            params.append(f"{t} a{n}")
            args.append(f"a{n}")
    attr = f'__attribute__((target("{features}"))) ' if features else ""
    return (f"#include <{i.header}>\n{attr}auto w({', '.join(params)}) "
            f"{{ return {i.name}({', '.join(args)}); }}\n")


def n_imms(i):
    return sum(1 for n, t in enumerate(i.params)
               if t is None and not (n == 0 and re.match(r"v(ld|st)", i.name)))


def imm_variants(k):
    if k == 0:
        return [()]
    v = [(1,) * k, (0,) * k]
    if k == 2:
        v += [(0, 1), (1, 0)]
    return v


FEAT = [
    re.compile(r"requires target feature '([^']*)'"),
    re.compile(r"needs target feature (\S+)"),
]


def feature_names(err):
    out = []
    for r in FEAT:
        for f in r.findall(err):
            f = f.split("|")[0].strip("()")
            out += [x for x in f.split(",") if x]
    return out


class Jobserver:
    def __init__(self):
        m = re.search(r"--jobserver-auth=fifo:(\S+)", os.environ.get("MAKEFLAGS", ""))
        self.fd = None
        if m and os.path.exists(m.group(1)):
            self.fd = os.open(m.group(1), os.O_RDWR)
        self.lock = threading.Lock()
        self.free_implicit = True
        self.held = []
        # A killed census hands its tokens back to the shared pool.
        for sig in (signal.SIGTERM, signal.SIGINT):
            signal.signal(sig, self.bail)

    def bail(self, *_):
        with self.lock:
            for t in self.held:
                os.write(self.fd, t)
            self.held.clear()
        os._exit(1)

    def acquire(self):
        with self.lock:
            if self.free_implicit:
                self.free_implicit = False
                return None
        if self.fd is None:
            return None
        t = os.read(self.fd, 1)
        with self.lock:
            self.held.append(t)
        return t

    def release(self, tok):
        if tok is None:
            with self.lock:
                self.free_implicit = True
        elif self.fd is not None:
            with self.lock:
                self.held.remove(tok)
            os.write(self.fd, tok)


def run(cmd):
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    return p.returncode, p.stderr


def probe(i, a, js):
    tok = js.acquire()
    try:
        return probe_inner(i, a)
    finally:
        js.release(tok)


def probe_inner(i, a):
    d = pathlib.Path(a.scratch) / "w" / i.name
    d.mkdir(parents=True, exist_ok=True)
    src = d / "w.cpp"
    feats = [f for f in i.features.split(",") if f]
    ok = None
    for imms in imm_variants(n_imms(i)):
        for _ in range(4):
            src.write_text(wrapper(i, ",".join(feats), imms))
            rc, err = run([a.clang, TARGET, "-std=c++26", "-O0", "-S", "-emit-llvm",
                           str(src), "-o", str(d / "clang.ll")])
            if rc == 0:
                ok = imms
                break
            new = [f for f in feature_names(err) if f not in feats]
            if not new:
                break
            feats += new
        if ok is not None:
            break
    if ok is None:
        first = next((ln for ln in err.splitlines() if "error:" in ln), "")
        return i, "clang-rejects", first.split("error:", 1)[-1].strip()
    rc, err = run([a.kiln, TARGET, "-std=c++26", "-O0", "-c", str(src),
                   "-o", str(d / "kiln.o")])
    if rc == 0:
        return i, "ok", ""
    m = re.search(r"code generation for (\S+) is not supported yet", err)
    if m:
        return i, "unsupported", m.group(1).strip("`'")
    first = next((ln for ln in err.splitlines() if "error" in ln), err.strip()[:200])
    return i, "error", re.sub(r"^.*?error:\s*", "", first).strip()


def group(b):
    """The lowering-shape group of unsupported builtin `b`."""
    n = b.removeprefix("__builtin_neon_").removeprefix("__builtin_")
    rules = [
        (r"^v(fmlal|fmlsl)", "fp16 fmlal/fmlsl"),
        (r"^v(ceq|cge|cgt|cle|clt)z", "compare against zero"),
        (r"^vca(ge|gt|le|lt)", "absolute compare"),
        (r"^v(ceq|cge|cgt|cle|clt|tst)[sdh]?_", "compare (scalar)"),
        (r"^vcvt.*_n_", "fixed-point conversion (_n)"),
        (r"^vcvt", "conversion (rounding / scalar)"),
        (r"^vrnd", "rounding"),
        (r"^v(bcax|eor3|rax1|xar|sha512|sm3|sm4)", "SHA3 / SHA512 / SM crypto"),
        (r"^vld[1-4]q?(_x[234]|_dup|_lane)", "load (dup / lane / multi-register)"),
        (r"^vst[1-4]q?(_x[234]|_lane)", "store (lane / multi-register)"),
        (r"^v(ld|st)", "load / store (other)"),
        (r"_lane", "lane forms"),
        (r"^vq.*[bhsd]_", "saturating scalar"),
        (r"^vq", "saturating vector"),
        (r"^v\w+[bhsd]_[suf]\d+$", "scalar arithmetic"),
        (r"^arm_", "ACLE"),
    ]
    for r, g in rules:
        if re.search(r, n):
            return g
    return "vector arithmetic (other)"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--kiln", required=True)
    ap.add_argument("--clang", default="clang++-22")
    ap.add_argument("--scratch", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("-j", type=int, default=4)
    ap.add_argument("--only", default=None)
    a = ap.parse_args()
    clang_inc = pathlib.Path(subprocess.run(
        [a.clang, "-print-resource-dir"], capture_output=True, text=True, check=True
    ).stdout.strip()) / "include"
    intrs = {}
    for h in HEADERS:
        for d in (ROOT / "resource/include", clang_inc):
            for name, i in parse(d / h, h).items():
                intrs.setdefault(name, i)
    names = sorted(intrs)
    if a.only:
        names = [n for n in names if re.search(a.only, n)]
    print(f"{len(names)} intrinsics", file=sys.stderr, flush=True)
    js = Jobserver()
    rows = []
    with concurrent.futures.ThreadPoolExecutor(a.j) as ex:
        futs = [ex.submit(probe, intrs[n], a, js) for n in names]
        for k, f in enumerate(concurrent.futures.as_completed(futs)):
            rows.append(f.result())
            if k % 200 == 0:
                print(f"{k}/{len(names)}", file=sys.stderr, flush=True)
    rows.sort(key=lambda r: r[0].name)
    with open(a.out, "w") as f:
        for i, o, m in rows:
            f.write(f"{i.name}\t{i.header}\t{o}\t{m}\n")
    c = collections.Counter(o for _, o, _ in rows)
    print(f"total {len(rows)}: " + ", ".join(f"{k} {c[k]}" for k in
                                             ("ok", "unsupported", "error", "clang-rejects")))
    blocked = collections.Counter(m for _, o, m in rows if o == "unsupported")
    groups = collections.defaultdict(list)
    for b, n in blocked.items():
        groups[group(b)].append((b, n))
    print(f"\n{len(blocked)} unsupported builtins, by group (builtin: intrinsics blocked)")
    for g, bs in sorted(groups.items(), key=lambda kv: -sum(n for _, n in kv[1])):
        print(f"\n{g}: {len(bs)} builtins, {sum(n for _, n in bs)} intrinsics")
        print("  " + ", ".join(f"{b.removeprefix('__builtin_neon_')} {n}" for b, n in sorted(bs)))
    errs = collections.Counter(re.sub(r"'[^']*'|`[^`]*`", "X", m)
                               for _, o, m in rows if o == "error")
    print("\nkiln errors (other), by message:")
    for m, n in errs.most_common():
        print(f"  {n}  {m}")


if __name__ == "__main__":
    main()
