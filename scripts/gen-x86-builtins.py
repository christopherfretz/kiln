#!/usr/bin/env python3
"""Generate the x86 target builtin tables (spec/landed/sema/WP-630.md):

  crates/kiln_ast/src/x86_builtins.rs       every `__builtin_ia32_*` name kiln's
                                            resource headers reference, plus
                                            `__builtin_ia32_pause` and clang's
                                            x86 header builtins (`_mm_pause`, ...):
                                            prototype, attributes, required
                                            target features, immediate checks
  crates/kiln_codegen/src/x86_intrinsics.rs the builtins clang lowers 1:1 to an
                                            `llvm.x86.*` intrinsic

There is no `BuiltinsX86.def` to read, so everything is recovered from a clang
(>= 21) binary by probing (spec/landed/sema/WP-630.md "Provenance"):

  1. Declarations: a TU naming every builtin, `-ast-dump=json`; the implicit
     `FunctionDecl`s carry the type and the `NoThrow` / `Const` attributes.
  2. Immediates: which integer parameters clang requires to be integer
     constant expressions, a valid combination of values, and per parameter
     the range / rounding / scale check clang applies (from its diagnostics).
  3. Features: one TU per builtin, codegen from a function without the
     feature: clang's "needs target feature X" names the required-features
     string (default flags, then -mno-sse2, -mno-sse, ... for the baseline).
  4. Lowering: each builtin is called from a wrapper with its features and
     two different immediates; after `opt -passes=mem2reg`, a body that is
     loads, bitcasts, one `llvm.x86.*` call taking the arguments in order
     (immediates as constants) and a store is a 1:1 lowering.

clang 20 dropped the MMX builtins (its `mmintrin.h` uses SSE2) and the
`vpopcnt*` ones; clang 19's prototypes for those are in CLANG19_ONLY below.

Usage: scripts/gen-x86-builtins.py [path/to/clang++]  (default: clang 21 at
/usr/lib/llvm-21/bin/clang++; `opt` is taken from the same directory)
"""

import collections
import concurrent.futures
import itertools
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
INC = ROOT / "resource/include"
OUT_AST = ROOT / "crates/kiln_ast/src/x86_builtins.rs"
OUT_CG = ROOT / "crates/kiln_codegen/src/x86_intrinsics.rs"
TRIPLE = "--target=x86_64-unknown-linux-gnu"

# clang's x86 header builtins (`TARGET_HEADER_BUILTIN`, `ALL_LANGUAGES`):
# declared by `xmmintrin.h` / `emmintrin.h` as ordinary functions, builtins
# only once so declared. (`_mm_prefetch` is a macro in clang 19's header.)
HEADER = {
    "_mm_clflush": "void _mm_clflush(void const *)",
    "_mm_getcsr": "unsigned int _mm_getcsr(void)",
    "_mm_lfence": "void _mm_lfence(void)",
    "_mm_mfence": "void _mm_mfence(void)",
    "_mm_pause": "void _mm_pause(void)",
    "_mm_setcsr": "void _mm_setcsr(unsigned int)",
    "_mm_sfence": "void _mm_sfence(void)",
}
# Not referenced by the headers: `pause` by libstdc++'s <bits/atomic_wait.h>,
# the rest answered by clang 19's `__has_builtin` under the default CPU
# (crates/kiln_pp/src/features_table.rs).
EXTRA = [
    "__builtin_ia32_pause", "__builtin_ia32_rdtsc", "__builtin_ia32_lfence",
    "__builtin_ia32_mfence", "__builtin_ia32_sfence", "__builtin_ia32_clflush",
    "__builtin_ia32_ldmxcsr", "__builtin_ia32_stmxcsr",
]

# clang 19 builtins clang 21 no longer has: (prototype, attributes,
# features, immediate checks), from clang 19's BuiltinsX86.def.
_MMX = [
    ("vec_init_v2si", "V2iii"), ("vec_init_v4hi", "V4sssss"), ("vec_init_v8qi", "V8ccccccccc"),
    ("paddb", "V8cV8cV8c"), ("paddw", "V4sV4sV4s"), ("paddd", "V2iV2iV2i"),
    ("paddsb", "V8cV8cV8c"), ("paddsw", "V4sV4sV4s"), ("paddusb", "V8cV8cV8c"),
    ("paddusw", "V4sV4sV4s"), ("psubb", "V8cV8cV8c"), ("psubw", "V4sV4sV4s"),
    ("psubd", "V2iV2iV2i"), ("psubsb", "V8cV8cV8c"), ("psubsw", "V4sV4sV4s"),
    ("psubusb", "V8cV8cV8c"), ("psubusw", "V4sV4sV4s"), ("pmulhw", "V4sV4sV4s"),
    ("pmullw", "V4sV4sV4s"), ("pmaddwd", "V2iV4sV4s"), ("pand", "V1LLiV1LLiV1LLi"),
    ("pandn", "V1LLiV1LLiV1LLi"), ("por", "V1LLiV1LLiV1LLi"), ("pxor", "V1LLiV1LLiV1LLi"),
    ("psllw", "V4sV4sV1LLi"), ("pslld", "V2iV2iV1LLi"), ("psllq", "V1LLiV1LLiV1LLi"),
    ("psrlw", "V4sV4sV1LLi"), ("psrld", "V2iV2iV1LLi"), ("psrlq", "V1LLiV1LLiV1LLi"),
    ("psraw", "V4sV4sV1LLi"), ("psrad", "V2iV2iV1LLi"), ("psllwi", "V4sV4si"),
    ("pslldi", "V2iV2ii"), ("psllqi", "V1LLiV1LLii"), ("psrlwi", "V4sV4si"),
    ("psrldi", "V2iV2ii"), ("psrlqi", "V1LLiV1LLii"), ("psrawi", "V4sV4si"),
    ("psradi", "V2iV2ii"), ("packsswb", "V8cV4sV4s"), ("packssdw", "V4sV2iV2i"),
    ("packuswb", "V8cV4sV4s"), ("punpckhbw", "V8cV8cV8c"), ("punpckhwd", "V4sV4sV4s"),
    ("punpckhdq", "V2iV2iV2i"), ("punpcklbw", "V8cV8cV8c"), ("punpcklwd", "V4sV4sV4s"),
    ("punpckldq", "V2iV2iV2i"), ("pcmpeqb", "V8cV8cV8c"), ("pcmpeqw", "V4sV4sV4s"),
    ("pcmpeqd", "V2iV2iV2i"), ("pcmpgtb", "V8cV8cV8c"), ("pcmpgtw", "V4sV4sV4s"),
    ("pcmpgtd", "V2iV2iV2i"),
]
_MMX_SSE = [
    ("pavgb", "V8cV8cV8c"), ("pavgw", "V4sV4sV4s"), ("pmaxsw", "V4sV4sV4s"),
    ("pmaxub", "V8cV8cV8c"), ("pminsw", "V4sV4sV4s"), ("pminub", "V8cV8cV8c"),
    ("pmovmskb", "iV8c"), ("pmulhuw", "V4sV4sV4s"), ("psadbw", "V4sV8cV8c"),
    ("pshufw", "V4sV4sIc"), ("cvtpi2ps", "V4fV4fV2i"), ("cvtps2pi", "V2iV4f"),
    ("cvttps2pi", "V2iV4f"),
]
_MMX_SSE2 = [
    ("cvtpd2pi", "V2iV2d"), ("cvtpi2pd", "V2dV2i"), ("cvttpd2pi", "V2iV2d"),
    ("paddq", "V1LLiV1LLiV1LLi"), ("pmuludq", "V1LLiV2iV2i"), ("psubq", "V1LLiV1LLiV1LLi"),
]
_MMX_SSSE3 = [
    ("pabsb", "V8cV8c"), ("pabsd", "V2iV2i"), ("pabsw", "V4sV4s"),
    ("palignr", "V8cV8cV8cIc"), ("phaddd", "V2iV2iV2i"), ("phaddsw", "V4sV4sV4s"),
    ("phaddw", "V4sV4sV4s"), ("phsubd", "V2iV2iV2i"), ("phsubsw", "V4sV4sV4s"),
    ("phsubw", "V4sV4sV4s"), ("pmaddubsw", "V8cV8cV8c"), ("pmulhrsw", "V4sV4sV4s"),
    ("pshufb", "V8cV8cV8c"), ("psignb", "V8cV8cV8c"), ("psignw", "V4sV4sV4s"),
    ("psignd", "V2iV2iV2i"),
]
_POPCNT = [
    ("vpopcntd_128", "V4iV4i", "avx512vpopcntdq,avx512vl"),
    ("vpopcntq_128", "V2LLiV2LLi", "avx512vpopcntdq,avx512vl"),
    ("vpopcntd_256", "V8iV8i", "avx512vpopcntdq,avx512vl"),
    ("vpopcntq_256", "V4LLiV4LLi", "avx512vpopcntdq,avx512vl"),
    ("vpopcntb_128", "V16cV16c", "avx512vl,avx512bitalg"),
    ("vpopcntw_128", "V8sV8s", "avx512vl,avx512bitalg"),
    ("vpopcntb_256", "V32cV32c", "avx512vl,avx512bitalg"),
    ("vpopcntw_256", "V16sV16s", "avx512vl,avx512bitalg"),
    ("vpopcntb_512", "V64cV64c", "avx512bitalg,evex512"),
    ("vpopcntw_512", "V32sV32s", "avx512bitalg,evex512"),
    ("vpopcntd_512", "V16iV16i", "avx512vpopcntdq,evex512"),
    ("vpopcntq_512", "V8LLiV8LLi", "avx512vpopcntdq,evex512"),
]
CLANG19_ONLY = {}
for feats, rows in (("mmx", _MMX), ("mmx,sse", _MMX_SSE), ("mmx,sse2", _MMX_SSE2),
                    ("mmx,ssse3", _MMX_SSSE3)):
    for n, sig in rows:
        CLANG19_ONLY["__builtin_ia32_" + n] = (sig, "nc", feats, [])
# clang 19 `SemaX86`: `vec_ext_v2si`'s index is in [0, 1].
CLANG19_ONLY["__builtin_ia32_vec_ext_v2si"] = ("iV2iIi", "nc", "mmx", [("range", 1, 0, 1)])
CLANG19_ONLY["__builtin_ia32_maskmovq"] = ("vV8cV8cc*", "n", "mmx,sse", [])
CLANG19_ONLY["__builtin_ia32_movntq"] = ("vV1LLi*V1LLi", "n", "mmx,sse", [])
for n, sig, feats in _POPCNT:
    CLANG19_ONLY["__builtin_ia32_" + n] = (sig, "nc", feats, [])

INTS = {"char", "signed char", "unsigned char", "short", "unsigned short", "int",
        "unsigned int", "long", "unsigned long", "long long", "unsigned long long"}
SCALARS = {
    "void": "v", "bool": "b", "_Bool": "b", "char": "c", "signed char": "Sc",
    "unsigned char": "Uc", "short": "s", "unsigned short": "Us", "int": "i",
    "unsigned int": "Ui", "long": "Li", "unsigned long": "ULi", "long long": "LLi",
    "unsigned long long": "ULLi", "float": "f", "double": "d", "_Float16": "x", "__bf16": "y",
}
VEC = re.compile(r"__attribute__\(\(__vector_size__\((\d+) \* sizeof\(([\w ]+)\)\)\)\) ([\w ]+)$")


def encode(t):
    """A clang type spelling in `Builtins.td` notation (kiln's decoder)."""
    t = t.strip()
    if t.endswith("*"):
        inner = t[:-1].strip()
        const = False
        if inner.startswith("const "):
            inner, const = inner[6:], True
        if inner.endswith(" const"):
            inner, const = inner[:-6], True
        return encode(inner) + ("C" if const else "") + "*"
    m = VEC.match(t)
    if m:
        assert m.group(2) == m.group(3), t
        return f"V{m.group(1)}{SCALARS[m.group(3)]}"
    return SCALARS[t]


class Clang:
    def __init__(self, cxx, work):
        self.cxx = cxx
        self.opt = str(pathlib.Path(cxx).parent / "opt")
        self.work = work

    def run(self, src, *flags, name="probe.cpp"):
        p = self.work / name
        p.write_text(src)
        return subprocess.run([self.cxx, TRIPLE, "-w", *flags, str(p)],
                              capture_output=True, text=True)


def errors_by_line(stderr, fname, keys):
    res = collections.defaultdict(list)
    for m in re.finditer(rf"^.*{re.escape(fname)}:(\d+):\d+: error: (.*)$", stderr, re.M):
        res[keys[int(m.group(1)) - 1]].append(m.group(2))
    return res


def call(name, ps, vals):
    """A call of `name` with params `ps`: `vals[k]` for parameter k, else a
    value-initialized local of the parameter's type."""
    pre, args = [], []
    for k, p in enumerate(ps):
        if k in vals:
            args.append(str(vals[k]))
        else:
            pre.append(f"typedef {p} t{k}; t{k} v{k}{{}};")
            args.append(f"v{k}")
    return " ".join(pre) + f" (void){name}({', '.join(args)});"


def header_decls():
    return "".join(f'extern "C" {d};\n' for d in HEADER.values())


def probe_decls(cl, names):
    src = header_decls() + "".join(f"void f{i}() {{ (void){n}; }}\n" for i, n in enumerate(names))
    r = cl.run(src, "-fsyntax-only", "-ferror-limit=0", "-fno-spell-checking",
               "-Xclang", "-ast-dump=json")
    decls = {}

    def walk(n):
        name = n.get("name", "")
        if n.get("kind") == "FunctionDecl" and name in names and "type" in n:
            params = [p["type"]["qualType"] for p in n.get("inner", [])
                      if p.get("kind") == "ParmVarDecl"]
            attrs = {p["kind"] for p in n.get("inner", []) if p.get("kind", "").endswith("Attr")}
            if name in HEADER or n.get("isImplicit"):
                decls[name] = (n["type"]["qualType"], params, attrs)
        for c in n.get("inner", []):
            walk(c)

    walk(json.loads(r.stdout))
    return decls


def ret_type(t, ps):
    sfx = "(" + ", ".join(ps) + ")" if ps else "(void)"
    t = t.removesuffix(" noexcept")
    if not t.endswith(sfx):
        sfx = "(" + ", ".join(ps) + ")"
    assert t.endswith(sfx), t
    return t[: -len(sfx)].strip()


def probe_imms(cl, decls):
    """{name: {param: kind}}, {name: {param: valid value}}, tile builtins."""
    lines, keys = [], []
    for n, (t, ps, a) in sorted(decls.items()):
        ints = [i for i, p in enumerate(ps) if p in INTS]
        for i in ints:
            vals = {k: 0 for k in ints if k != i}
            vals[i] = "nc"
            lines.append(f"void g{len(lines)}(int nc) {{ {call(n, ps, vals)} }}")
            keys.append((n, i))
    res = errors_by_line(cl.run(header_decls() + "\n".join(lines) + "\n", "-fsyntax-only",
                                "-ferror-limit=0", name="ice.cpp").stderr, "ice.cpp",
                         [None] * HEADER_LINES + keys)
    ice = collections.defaultdict(list)
    for (n, i), es in res.items():
        if any("must be a constant integer" in e for e in es):
            ice[n].append(i)
    for v in ice.values():
        v.sort()
    lines, keys = [], []
    for n, ii in sorted(ice.items()):
        for combo in itertools.product([0, 4, 1, 2, 8, 3], repeat=len(ii)):
            lines.append(f"void g{len(lines)}() {{ {call(n, decls[n][1], dict(zip(ii, combo)))} }}")
            keys.append((n, combo))
    res = errors_by_line(cl.run(header_decls() + "\n".join(lines) + "\n", "-fsyntax-only",
                                "-ferror-limit=0", name="ice.cpp").stderr, "ice.cpp",
                         [None] * HEADER_LINES + keys)
    good = {}
    for k in keys:
        if k[0] not in good and not res.get(k):
            good[k[0]] = dict(zip(ice[k[0]], k[1]))
    assert set(good) == set(ice), set(ice) - set(good)
    lines, keys = [], []
    probes = (1000000, 9)
    for n, ii in sorted(ice.items()):
        for i in ii:
            for val in probes:
                vals = dict(good[n])
                vals[i] = val
                lines.append(f"void g{len(lines)}() {{ {call(n, decls[n][1], vals)} }}")
                keys.append((n, i, val))
        # Clang's duplicate-tile check: every immediate the same.
        if len(ii) >= 2:
            lines.append(f"void g{len(lines)}() {{ {call(n, decls[n][1], dict.fromkeys(ii, 0))} }}")
            keys.append((n, "dup", 0))
    res = errors_by_line(cl.run(header_decls() + "\n".join(lines) + "\n", "-fsyntax-only",
                                "-ferror-limit=0", name="ice.cpp").stderr, "ice.cpp",
                         [None] * HEADER_LINES + keys)
    kinds, tiles = {}, set()
    for n, ii in sorted(ice.items()):
        ks = {}
        for i in ii:
            kind = ("ice",)
            for e in res.get((n, i, 1000000), []):
                m = re.search(r"valid range \[(-?\d+), (-?\d+)\]( \[-W)?", e)
                if m:
                    kind = ("range" if m.group(3) else "range!", int(m.group(1)), int(m.group(2)))
                elif "rounding" in e:
                    kind = ("round" if not res.get((n, i, 9)) else "sae",)
                elif "scale" in e:
                    kind = ("scale",)
                else:
                    sys.exit(f"unexpected diagnostic for {n} argument {i}: {e}")
            ks[i] = kind
        kinds[n] = ks
        if any("different tiles" in e for e in res.get((n, "dup", 0), [])):
            tiles.add(n)
    return kinds, good, tiles


HEADER_LINES = len(HEADER)


def wrapper(fn, n, decl, kinds, vals, attrs=""):
    """`extern "C" void fn(R *r, P0 *a0, ...) { *r = n(*a0, ..., IMM, ...); }`"""
    t, ps, _ = decl
    rt = ret_type(t, ps)
    sig, args = [], []
    if rt != "void":
        sig.append(f"__typeof__({rt}) *r")
    for k, p in enumerate(ps):
        if k in kinds:
            args.append(str(vals[k]))
        else:
            sig.append(f"__typeof__({p}) *a{k}")
            args.append(f"*a{k}")
    body = f"{n}({', '.join(args)})"
    body = f"*r = {body};" if rt != "void" else f"{body};"
    return f'extern "C" {attrs} void {fn}({", ".join(sig)}) {{ {body} }}'


def probe_features(cl, decls, kinds, good):
    def one(job):
        n, flags = job
        src = header_decls() + wrapper("w", n, decls[n], kinds.get(n, {}), good.get(n, {})) + "\n"
        r = subprocess.run([cl.cxx, TRIPLE, "-x", "c++", "-", "-S", "-emit-llvm", "-o",
                            os.devnull, "-w", *flags], input=src, capture_output=True, text=True)
        m = re.search(r"error: '(\w+)' needs target feature (.*)$", r.stderr, re.M)
        if m:
            return n, m.group(2)
        if "this builtin is only available on 32-bit targets" in r.stderr:
            return n, "only32"
        if "error" in r.stderr:
            sys.exit(f"feature probe of {n} failed:\n{r.stderr}")
        return n, None

    feats, only32, todo = {}, set(), sorted(decls)
    with concurrent.futures.ThreadPoolExecutor(os.cpu_count()) as ex:
        for flags in ([], ["-mno-sse2"], ["-mno-sse"], ["-mno-mmx"], ["-mno-fxsr"]):
            for n, f in ex.map(one, [(n, flags) for n in todo]):
                if f == "only32":
                    only32.add(n)
                elif f is not None:
                    feats[n] = f
            todo = [n for n in todo if n not in feats and n not in only32]
    return feats, only32


def target_attr(f):
    if not f:
        return ""
    return '__attribute__((target("' + ",".join(a.split("|")[0] for a in f.split(",")) + '")))'


def variant_vals(n, kinds, good, variant):
    v = dict(good.get(n, {}))
    if variant == 1:
        for i, k in kinds.get(n, {}).items():
            if k[0] in ("range", "range!"):
                hi = 7 if k[2] == 31 else k[2]  # cmpps > 7 needs AVX
                v[i] = hi if hi != v[i] else k[1]
            elif k[0] in ("round", "sae", "scale"):
                v[i] = 8
            else:
                v[i] = 1 if v[i] != 1 else 2
        if len(kinds.get(n, {})) == 3 and all(k[0] == "range!" for k in kinds[n].values()):
            v = {i: 5 + j for j, i in enumerate(sorted(kinds[n]))}
    return v


def probe_lowering(cl, decls, kinds, good, feats, names):
    """{name: (intrinsic, ret, params, fn attributes)} for the 1:1 builtins."""
    def ir(variant):
        src = header_decls() + "\n".join(
            wrapper(f"w{i}", n, decls[n], kinds.get(n, {}),
                    variant_vals(n, kinds, good, variant), target_attr(feats.get(n, "")))
            for i, n in enumerate(names)) + "\n"
        r = cl.run(src, "-S", "-emit-llvm", "-O0", "-Xclang", "-disable-O0-optnone",
                   "-fno-discard-value-names", "-o", "-", name=f"lower{variant}.cpp")
        if r.returncode:
            sys.exit(r.stderr)
        r2 = subprocess.run([cl.opt, "-S", "-passes=mem2reg"], input=r.stdout,
                            capture_output=True, text=True)
        if r2.returncode:
            sys.exit(r2.stderr)
        return r2.stdout

    def classify(n, body, variant):
        vals = variant_vals(n, kinds, good, variant)
        ks = kinds.get(n, {})
        defs, the_call = {}, None
        for line in body.splitlines():
            line = line.strip()
            if not line or line.endswith(":"):
                continue
            m = re.match(r"%([\w.]+) = load (.*?), ptr %a(\d+)", line)
            if m:
                defs[m.group(1)] = ("arg", int(m.group(3)))
                continue
            m = re.match(r"%([\w.]+) = bitcast (.*?) %([\w.]+) to (.*)$", line)
            if m and m.group(3) in defs:
                defs[m.group(1)] = defs[m.group(3)]
                continue
            m = re.match(r"(?:%([\w.]+) = )?call (.*?) @(llvm\.x86\.[\w.]+)\((.*)\)$", line)
            if m and the_call is None:
                the_call = (m.group(3), m.group(4))
                if m.group(1):
                    defs[m.group(1)] = ("ret",)
                continue
            m = re.match(r"store (.*?) %([\w.]+), ptr %r", line)
            if m and defs.get(m.group(2)) == ("ret",):
                continue
            if line == "ret void":
                continue
            return None
        if the_call is None:
            return None
        ops = [o.strip() for o in re.split(r",(?![^<(]*[>)])", the_call[1]) if o.strip()]
        if len(ops) != len(decls[n][1]):
            return None
        for k, o in enumerate(ops):
            m = re.match(r"(.*?) (%[\w.]+|-?\d+)$", o)
            if not m:
                return None
            v = m.group(2)
            if k in ks:
                if v.startswith("%") or int(v) not in (vals[k], vals[k] - 256):
                    return None
            elif not v.startswith("%") or defs.get(v[1:]) != ("arg", k):
                return None
        return the_call[0]

    res = collections.defaultdict(list)
    decl_lines, attrs = {}, {}
    for variant in (0, 1):
        text = ir(variant)
        for m in re.finditer(r"^define [^@]*@w(\d+)\([^)]*\)[^{]*\{\n(.*?)^\}", text, re.M | re.S):
            n = names[int(m.group(1))]
            res[n].append(classify(n, m.group(2), variant))
        for m in re.finditer(r"^declare (.*?) @(llvm\.x86\.[\w.]+)\((.*?)\)(?: #(\d+))?$",
                             text, re.M):
            decl_lines[m.group(2)] = (m.group(1), m.group(3), m.group(4))
        attrs.update(re.findall(r"^attributes #(\d+) = \{ (.*) \}$", text, re.M))
    one = {}
    for n, (a, b) in res.items():
        if a and a == b:
            ret, params, grp = decl_lines[a]
            if "x86_amx" in ret + params or "{" in ret:
                continue
            one[n] = (a, ret, params, attrs.get(grp, ""))
    return one


MEM = {
    "nocallback nofree nosync nounwind willreturn memory(none)": "None",
    "nocallback nofree nosync nounwind willreturn memory(read)": "Read",
    "nocallback nofree nosync nounwind willreturn memory(argmem: read)": "ArgRead",
    "nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)": "ArgReadWrite",
    "nounwind": "Unknown",
    "nounwind memory(argmem: readwrite)": "UnknownArgReadWrite",
    "nounwind memory(inaccessiblemem: readwrite)": "UnknownInaccessible",
}


def imm_rust(kind, i):
    k = kind[0]
    if k == "range":
        return f"Imm::Range({i}, {kind[1]}, {kind[2]})"
    if k == "range!":
        return f"Imm::RangeError({i}, {kind[1]}, {kind[2]})"
    if k == "round":
        return f"Imm::Rounding({i})"
    if k == "sae":
        return f"Imm::Sae({i})"
    if k == "scale":
        return f"Imm::Scale({i})"
    return None


def main():
    cxx = sys.argv[1] if len(sys.argv) > 1 else "/usr/lib/llvm-21/bin/clang++"
    with tempfile.TemporaryDirectory() as tmp:
        cl = Clang(cxx, pathlib.Path(tmp))
        used = set(EXTRA) | set(HEADER)
        for h in INC.glob("*.h"):
            used |= set(re.findall(r"__builtin_ia32_\w+", h.read_text(errors="replace")))
        names = sorted(used)
        decls = probe_decls(cl, set(names))
        missing = sorted(set(names) - set(decls) - set(CLANG19_ONLY))
        if missing:
            sys.exit(f"no prototype for: {missing}")
        stale = sorted(set(CLANG19_ONLY) & set(decls))
        if stale:
            sys.exit(f"CLANG19_ONLY names clang declares: {stale}")
        kinds, good, tiles = probe_imms(cl, decls)
        feats, only32 = probe_features(cl, decls, kinds, good)
        lowerable = sorted(n for n in decls if n not in only32 and n != "__builtin_ia32_prefetchi")
        one = probe_lowering(cl, decls, kinds, good, feats, lowerable)

    rows = []
    for n in names:
        if n in CLANG19_ONLY:
            sig, attrs, f, imms = CLANG19_ONLY[n]
            checks = [f"Imm::Range({i}, {lo}, {hi})" for _, i, lo, hi in imms]
            rows.append((n, sig, attrs, f, checks))
            continue
        t, ps, a = decls[n]
        ks = kinds.get(n, {})
        sig = encode(ret_type(t, ps)) + "".join(
            ("I" if i in ks else "") + encode(p) for i, p in enumerate(ps))
        attrs = ("n" if "NoThrowAttr" in a else "") + ("c" if "ConstAttr" in a else "")
        checks = [c for i, k in sorted(ks.items()) if (c := imm_rust(k, i))]
        if n in tiles:
            checks.append("Imm::DistinctTiles")
        if n in only32:
            checks.append("Imm::Only32Bit")
        rows.append((n, sig, attrs, feats.get(n, ""), checks))
    rows.sort(key=lambda r: r[0])
    attr_const = {"nc": "NC", "n": "N", "": "NONE", "c": "C"}
    used = sorted({attr_const[r[2]] for r in rows} | {"Imm", "Row", "r"}
                  | ({"h"} if HEADER else set()), key=lambda s: (s.lower() != s, s))
    lines = [
        "// @generated by scripts/gen-x86-builtins.py from clang's declarations (clang 19's",
        "// x86 builtin names: those resource/include references; spec/landed/sema/WP-630.md).",
        "// Do not edit.",
        "",
        f"use crate::decl::builtin::x86::{{{', '.join(sorted(used))}}};",
        "",
        "/// Every x86 target builtin, sorted by name: (index, name, prototype,",
        "/// attributes, required target features, immediate checks); `h` rows are",
        "/// header builtins.",
        "#[rustfmt::skip]",
        "pub(crate) const X86: &[Row] = &[",
    ]
    for i, (n, sig, attrs, f, checks) in enumerate(rows):
        ctor = "h" if n in HEADER else "r"
        lines.append(f'    {ctor}({i}, "{n}", "{sig}", {attr_const[attrs]}, "{f}", '
                     f'&[{", ".join(checks)}]),')
    lines.append("];")
    OUT_AST.write_text("\n".join(lines) + "\n")

    cg = [
        "// @generated by scripts/gen-x86-builtins.py from clang's IR for each x86 builtin",
        "// (spec/landed/codegen/WP-630.md). Do not edit.",
        "",
        "use crate::x86::{Map, Mem, m};",
        "",
        "/// The x86 builtins clang lowers to one `llvm.x86.*` call taking the",
        "/// arguments in order, sorted by name: (builtin, intrinsic, return type,",
        "/// parameter types, intrinsic attributes).",
        "#[rustfmt::skip]",
        "pub(crate) const MAP: &[Map] = &[",
    ]
    for n in sorted(one):
        intr, ret, params, attrs = one[n]
        if attrs not in MEM:
            sys.exit(f"unexpected intrinsic attributes {attrs!r} for {intr}")
        cg.append(f'    m("{n}", "{intr}", "{ret}", "{params}", Mem::{MEM[attrs]}),')
    cg.append("];")
    OUT_CG.write_text("\n".join(cg) + "\n")
    print(f"{OUT_AST.relative_to(ROOT)}: {len(rows)} builtins "
          f"({len(CLANG19_ONLY)} clang 19 only, {len(HEADER)} header builtins)")
    print(f"{OUT_CG.relative_to(ROOT)}: {len(one)} 1:1 lowerings")


if __name__ == "__main__":
    main()
