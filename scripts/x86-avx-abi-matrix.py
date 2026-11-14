#!/usr/bin/env python3
"""x86-64 SysV call-signature matrix for vectors and their wrappers
(WP-1418, spec/landed/abi/WP-1418.md).

Generates one C++ and one C translation unit holding every shape the AVX
ABI level changes (bare 128/256/512-bit vectors of each element kind,
structs, unions, arrays and nested wrappers of one vector, mixed
aggregates that stay in memory, `_Complex`, `__int128`, non-trivial
classes, member functions, lambdas, function pointers, variadic calls),
compiles each with kiln and with one or more clangs under every flag set
of the matrix (`-mavx`, `-mavx2`, `-mavx512f`, `-march=`, `-mno-avx`
after `-march=`, ...), and compares the `define`, `declare` and `call`
lines (attribute-group numbers dropped). Prints one line per (flags,
compiler, language) with the mismatches, and exits 1 if there is any.

    scripts/x86-avx-abi-matrix.py --kiln <kiln++> [--clang clang++-19]...
"""

import argparse
import os
import re
import subprocess
import sys
import tempfile

FLAGS = [
    [],
    ["-mavx"],
    ["-mavx2"],
    ["-mavx512f"],
    ["-march=haswell"],
    ["-march=x86-64-v3"],
    ["-march=x86-64-v4"],
    ["-march=skylake-avx512"],
    ["-march=haswell", "-mno-avx"],
    ["-march=x86-64-v4", "-mno-avx512f"],
    ["-mavx512f", "-mno-avx"],
    ["-mno-avx", "-mavx2"],
]

# (name, element type, bytes) of the vectors.
VECS = [
    ("f4", "float", 16), ("f8", "float", 32), ("f16", "float", 64),
    ("d2", "double", 16), ("d4", "double", 32), ("d8", "double", 64),
    ("c32", "char", 32), ("s16", "short", 32), ("i8", "int", 32),
    ("l4", "long long", 32), ("l8", "long long", 64), ("c64", "char", 64),
    ("q2", "__int128", 32), ("q4", "__int128", 64), ("q1", "__int128", 16),
    ("f32", "float", 128),
]


def common():
    """Declarations shared by C and C++: (prelude, [(type name, is_class)])."""
    out = []
    types = []
    for n, e, b in VECS:
        out.append(f"typedef {e} {n} __attribute__((vector_size({b})));")
        types.append(n)
    out.append("typedef float e8 __attribute__((ext_vector_type(8)));")
    out.append("typedef double e3 __attribute__((ext_vector_type(3)));")
    types += ["e8", "e3"]
    aggs = {
        # One vector, in each wrapper.
        "SW8": "struct SW8 { f8 v; };",
        "SD4": "struct SD4 { d4 v; };",
        "SI8": "struct SI8 { i8 v; };",
        "SW16": "struct SW16 { f16 v; };",
        "SL8": "struct SL8 { l8 v; };",
        "SQ2": "struct SQ2 { q2 v; };",
        "UW8": "union UW8 { f8 v; };",
        "UW16": "union UW16 { f16 v; };",
        "UMix": "union UMix { f8 v; d4 w; i8 x; };",
        "UVA": "union UVA { f8 v; float a[8]; };",
        "UV4": "union UV4 { f8 v; f4 h; };",
        "UV16": "union UV16 { f16 v; f8 h; };",
        "SA1": "struct SA1 { f8 v[1]; };",
        "SA1W": "struct SA1W { d8 v[1]; };",
        "SNest": "struct SNest { struct SW8 in; };",
        "SNest2": "struct SNest2 { struct SNest in; };",
        "SNestU": "struct SNestU { union UW8 u; };",
        # Mixed aggregates that stay in memory.
        "SMix": "struct SMix { f8 v; int i; };",
        "SPair": "struct SPair { f4 a, b; };",
        "SPair8": "struct SPair8 { f8 a, b; };",
        "SArr": "struct SArr { float a[8]; };",
        "SArrD": "struct SArrD { double a[4]; };",
        "SA2": "struct SA2 { f4 v[2]; };",
        "SPad": "struct SPad { f4 v; char c; };",
        "SBig": "struct SBig { f16 v; f16 w; };",
        # Unaffected by AVX.
        "S4": "struct S4 { f4 v; };",
        "SCD": "struct SCD { double _Complex c; };",
    }
    for k, v in aggs.items():
        out.append(v)
        types.append(f"struct {k}" if v.startswith("struct") else f"union {k}")
    types += ["double _Complex", "__int128", "unsigned __int128", "long double"]
    return out, types


def ident(t):
    return re.sub(r"[^A-Za-z0-9]", "_", t)


def body(types, cxx):
    out = []
    for i, t in enumerate(types):
        k = ident(t)
        out.append(f"{t} ext_{k}({t} a, int n, {t} b);")
        out.append(f"{t} id_{k}({t} a) {{ return a; }}")
        out.append(f"{t} use_{k}({t} a) {{ return ext_{k}(a, 1, a); }}")
        out.append(f"{t} ptr_{k}({t} (*p)({t}), {t} a) {{ return p(a); }}")
        out.append(f"void va_{k}(int n, ...);")
        out.append(f"void pass_va_{k}({t} a) {{ va_{k}(1, a, a); }}")
        out.append(f"void many_{k}(double, double, double, double, double, double, double, {t}, {t});")
        out.append(f"void call_many_{k}({t} a) {{ many_{k}(0, 0, 0, 0, 0, 0, 0, a, a); }}")
    if cxx:
        out.append("struct NT { f8 v; NT(const NT &); ~NT(); };")
        out.append("NT nt_ret(NT a) { return a; }")
        out.append("struct Mem { d4 m(d4 a, struct SD4 b) { return a + b.v; }"
                   " static struct SW8 s(struct SW8 a) { return a; }"
                   " virtual f8 v(f8 a) { return a; } };")
        out.append("d4 call_mem(Mem &m, d4 a, struct SD4 b) { return m.m(a, b) + m.v((f8)a) [0]; }")
        out.append("struct SW8 call_static(struct SW8 a) { return Mem::s(a); }")
        out.append("f8 lam(f8 a) { auto l = [](f8 x, union UW8 u) { return x + u.v; };"
                   " return l(a, (union UW8){a}); }")
        out.append("struct B1 { virtual ~B1(); virtual f8 g(f8); };")
        out.append("struct B2 { virtual f8 h(f8); };")
        out.append("struct D : B1, B2 { f8 h(f8 a) override { return a; } };")
        out.append("D *mk() { return new D; }")
        # A base beside or around the vector (clang 21+: MEMORY unless the
        # base is as wide as the class).
        out.append("struct EmptyB {}; struct DerV : EmptyB { f8 v; }; struct FromW : SW8 {};")
        out.append("struct DerV der_v(struct DerV a) { return a; }")
        out.append("struct FromW from_w(struct FromW a) { return a; }")
        out.append("f8 via_mp(Mem &m, f8 (Mem::*p)(f8), f8 a) { return (m.*p)(a); }")
    return out


def compile_ir(cc, src, flags, lang, extra):
    with tempfile.TemporaryDirectory(prefix="avx-abi-wp1418-") as d:
        out = os.path.join(d, "o.ll")
        cmd = [cc, "--target=x86_64-unknown-linux-gnu", *extra, *flags,
               "-x", lang, "-S", "-emit-llvm", "-O0", "-w", "-o", out, src]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
        if r.returncode != 0:
            return None, r.stderr[-2000:]
        with open(out) as f:
            return f.read(), ""


def sig_lines(ir):
    keep = []
    for line in ir.splitlines():
        s = line.strip()
        if s.startswith(("define ", "declare ")) or " call " in s or s.startswith("call "):
            # Signatures only: no intrinsics (the copies around a call
            # are not part of the convention), value numbers dropped.
            if "@llvm." in s:
                continue
            s = re.sub(r" #\d+", "", s)
            s = re.sub(r", !dbg !\d+", "", s)
            s = re.sub(r"%\d+", "%_", s)
            keep.append(s)
    return keep


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--kiln", required=True)
    ap.add_argument("--clang", action="append", default=[])
    ap.add_argument("--keep", help="write the generated sources here")
    a = ap.parse_args()
    clangs = a.clang or ["clang++-19"]
    prelude, types = common()
    srcs = {
        "c++": "\n".join(prelude + body(types, True)) + "\n",
        "c": "\n".join(prelude + body(types, False)) + "\n",
    }
    bad = 0
    total = 0
    with tempfile.TemporaryDirectory(prefix="avx-abi-src-wp1418-") as d:
        paths = {}
        for lang, text in srcs.items():
            p = os.path.join(a.keep or d, "matrix." + ("cpp" if lang == "c++" else "c"))
            with open(p, "w") as f:
                f.write(text)
            paths[lang] = p
        for clang in clangs:
            ver = subprocess.run([clang, "--version"], capture_output=True, text=True).stdout
            major = re.search(r"clang version (\d+)", ver).group(1)
            kextra = [f"-fclang-compat={major}"]
            for flags in FLAGS:
                for lang, p in paths.items():
                    std = ["-std=c++20"] if lang == "c++" else ["-std=c17"]
                    ki, kerr = compile_ir(a.kiln, p, flags, lang, std + kextra)
                    ci, cerr = compile_ir(clang, p, flags, lang, std)
                    tag = f"clang {major} {lang} {' '.join(flags) or '(none)'}"
                    if ki is None or ci is None:
                        print(f"ERROR {tag}: kiln: {kerr.strip()[:300]} clang: {cerr.strip()[:300]}")
                        bad += 1
                        continue
                    k, c = sig_lines(ki), sig_lines(ci)
                    total += len(c)
                    diff = [(x, y) for x, y in zip(c, k) if x != y]
                    if len(k) != len(c):
                        diff.append((f"{len(c)} lines", f"{len(k)} lines"))
                    if diff:
                        bad += 1
                        print(f"MISMATCH {tag}: {len(diff)} lines")
                        for x, y in diff[:6]:
                            print(f"  - {x}\n  + {y}")
                    else:
                        print(f"ok {tag}: {len(c)} lines")
    print(f"{total} signature lines compared, {bad} mismatching configurations")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
