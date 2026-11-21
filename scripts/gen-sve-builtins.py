#!/usr/bin/env python3
"""Generate crates/kiln_ast/src/sve_builtins.rs, the lowering of every SVE
and SME ACLE builtin (`__builtin_sve_*`, `__builtin_sme_*`) that clang's
`arm_sve.h` and `arm_sme.h` alias (WP-1739; spec/landed/codegen/WP-810.md
"Builtin aliases").

clang lowers these builtins in `EmitAArch64SVEBuiltinExpr` /
`EmitAArch64SMEBuiltinExpr`, driven by the TableGen `arm_sve_builtin_cg`
table (an intrinsic and `SVETypeFlags` per builtin) and a few dozen special
cases. Rather than port that code, the table is read off clang's own output:
for each builtin, a translation unit calls it once with each operand the
result of an opaque call (`g0()`, `g1()`, ...) and each immediate a literal,
and the `-O0` IR of the caller is the builtin's lowering. Each builtin's row
is that IR as a short *recipe*: the operand evaluations in clang's order,
then the instructions, operands renamed to earlier steps; an immediate is
found by compiling with two of its valid values and diffing. The intrinsics
the lowering declares are listed in the order clang declares them (a
lowering may declare one it does not call), with their attributes.

The recipes are grouped into *families*: a recipe with its intrinsic names
and types replaced by placeholders. A row names its family and the
intrinsics and types that fill it, so the table holds each shape once.

Both clang 22 (the lowering at `-fclang-compat=22`) and clang 19 (the one at
19, for the builtins clang 19 has) are run; a builtin whose clang 19 recipe
differs keeps both. Builtins with a type kiln does not have (`svmfloat8_t`,
`mfloat8_t`, `fpm_t`: clang 21's FP8) are listed, not lowered.

The first run compiles every translation unit with both clangs and writes
the recipes as JSON (`--raw`); `--from-raw` writes the table from it. With
`--census KILN` the same translation units are compiled by kiln (at the
default `-fclang-compat`, then at 19 for the clang 19 recipes) and each
caller's IR and the intrinsic declarations are compared with clang's; a
TSV row per builtin and version goes to `--out`.

The semantic checks clang's Sema applies to a call of these builtins
(WP-1792, WP-1793: the required target features, the streaming mode, the
ZA and ZT0 state and the immediate operands' checks) are not visible in
the IR; `--checks` writes them to crates/kiln_ast/src/sve_checks.rs from
the tables clang itself is built with (and BuiltinsAArch64NeonSVEBridge.def
for the `arm_neon_sve_bridge.h` builtins): clang-tblgen's
`--gen-arm-{sve,sme}-builtins`, `--gen-arm-{sve,sme}-sema-rangechecks`,
`--gen-arm-{sve,sme}-streaming-attrs` and `--gen-arm-sme-builtin-za-state`
run over `arm_sve.td` and `arm_sme.td` (clang/include/clang/Basic of the
llvmorg-22.1.8 and llvmorg-19.1.7 tags; clang-tblgen-22 and
clang-tblgen-19 from Debian's clang-tools-22 and clang-tools-19).

Usage: scripts/gen-sve-builtins.py --work DIR [--jobs N] [--only REGEX] --raw JSON
       scripts/gen-sve-builtins.py --work DIR --from-raw JSON
       scripts/gen-sve-builtins.py --work DIR --from-raw JSON --census KILN --out TSV
       scripts/gen-sve-builtins.py --work DIR --checks --td22 DIR --tblgen22 BIN
                                   --td19 DIR --tblgen19 BIN
"""

import argparse
import concurrent.futures
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/kiln_ast/src/sve_builtins.rs"
CHECKS_OUT = ROOT / "crates/kiln_ast/src/sve_checks.rs"
TARGET = "--target=aarch64-unknown-linux-gnu"
FEATURES = (
    "sve2p2 sme2p2 sve-aes sve-sha3 sve-sm4 sve-bitperm f32mm f64mm i8mm bf16 "
    "sve-b16b16 sme-f64f64 sme-i16i64 sme-f16f16 sme-b16b16 sme-lutv2 lut "
    "faminmax sve-bfscale sme-mop4 sme-tmop sve-aes2 ssve-aes sve-f16f32mm "
    "ssve-bitperm sme2p1 sve2p1 sme-fa64 fp8"
).split()
FEATURES19 = (
    "sve2p1 sme2p1 sve2-aes sve2-sha3 sve2-sm4 sve2-bitperm f32mm f64mm i8mm bf16 b16b16 "
    "sme-f64f64 sme-i16i64 sme-f16f16 sme-b16b16 lut faminmax sme-fa64 sme-lutv2"
).split()


def march(cc):
    """The `-march` enabling every feature an SVE or SME builtin needs."""
    if cc.endswith("19"):
        return "-march=armv9.5-a+" + "+".join(FEATURES19)
    return "-march=armv9.6-a+" + "+".join(FEATURES)
# The FP8 types kiln does not have.
UNSUPPORTED_TYPES = re.compile(r"mfloat8|fpm_t")

ALIAS = re.compile(
    r"(__aio?) __attribute__\(\(__clang_arm_builtin_alias\((\w+)\)\)\)\s*\n([^;{]*?);"
)
DECL = re.compile(r"^(.*?)\b(\w+)\((.*)\)$", re.S)


def clang_include(clang):
    out = subprocess.run([clang, "-print-resource-dir"], capture_output=True, text=True,
                         check=True).stdout.strip()
    return pathlib.Path(out) / "include"


def parse_headers(inc):
    """builtin -> (ret, [params]) from arm_sve.h and arm_sme.h, preferring
    the non-overloaded (`__ai`) declaration; and the typedef preamble."""
    decls, pre = {}, []
    for h in ("arm_sve.h", "arm_sme.h", "arm_neon_sve_bridge.h"):
        text = (inc / h).read_text()
        for kind, b, d in ALIAS.findall(text):
            m = DECL.match(" ".join(d.split()))
            ret, params = m.group(1).strip(), m.group(3).strip()
            params = [] if params in ("", "void") else [p.strip() for p in params.split(",")]
            # `svreinterpret`'s parameter is named `op`.
            params = [re.sub(r"(\w)\s+op$", r"\1", p) for p in params]
            if b not in decls or (kind == "__ai" and decls[b][2] != "__ai"):
                decls[b] = (ret, params, kind)
        if h == "arm_sve.h":
            head = text.split("/* Function attributes */")[0]
            for line in head.splitlines():
                if line.startswith("typedef") and not UNSUPPORTED_TYPES.search(line):
                    pre.append(line)
            for e in re.findall(r"enum sv\w+\s*\{[^}]*\};", head):
                pre.append(e)
    pre.insert(0, "typedef __bf16 bfloat16_t;")
    # The NEON vector types (`svaddqv`, `svget_neonq`) without arm_neon.h.
    for t in ("int8x16", "int16x8", "int32x4", "int64x2", "uint8x16", "uint16x8", "uint32x4",
              "uint64x2", "float16x8", "float32x4", "float64x2", "bfloat16x8"):
        e, n = t.split("x")
        pre.append(f"typedef __attribute__((neon_vector_type({n}))) {e}_t {t}_t;")
    return {b: (r, p) for b, (r, p, _) in decls.items()}, pre


def tu(pre, b, ret, params, imms, attrs):
    """The translation unit calling builtin `b`: operand k is `gk()`, or
    the literal `imms[k]` (an immediate)."""
    lines = ["#include <stdint.h>", *pre, 'extern "C" {',
             f"static __inline__ __attribute__((__always_inline__, __nodebug__, "
             f"__clang_arm_builtin_alias({b}))) {ret} k_({', '.join(params)});"]
    for k, p in enumerate(params):
        if k not in imms:
            lines.append(f"{p} g{k}(void);")
    lines.append(f"{ret} w(void) {attrs} {{")
    lines.append("  return k_(")
    first = "\n".join(lines).count("\n") + 2
    for k, p in enumerate(params):
        arg = f"({p}){imms[k]}" if k in imms else f"g{k}()"
        lines.append(f"    {arg}{',' if k + 1 < len(params) else ''}")
    lines += ["  );", "}", "}"]
    return "\n".join(lines) + "\n", first


def compile_tu(cc, src, path, extra):
    path.write_text(src)
    ll = path.with_suffix(".ll")
    r = subprocess.run([cc, TARGET, march(cc), "-S", "-emit-llvm", "-O0", *extra, "-o", str(ll),
                        str(path)], capture_output=True, text=True)
    if r.returncode != 0:
        return None, r.stderr
    text = ll.read_text()
    ll.unlink()
    return text, r.stderr


INTLIKE = re.compile(r"^(u?int(8|16|32|64)_t|enum sv\w+|bool)$")
SENTINEL = 100


def fix_errors(err, imms, allowed, attrs):
    """Adjusts the immediates and the caller's attributes after clang's
    errors `err`; returns the new attributes, or None if nothing changed.
    An out-of-range immediate is found by its value (each starts at its
    own sentinel, `SENTINEL + k`)."""
    changed = False
    by_value = {v: k for k, v in imms.items()}
    for msg in re.findall(r"error: (.*)$", err, re.M):
        if m := re.match(r"argument value (-?\d+) is outside the valid range \[(-?\d+), (-?\d+)\]",
                         msg):
            k = by_value.get(int(m.group(1)))
            if k is not None:
                lo, hi = int(m.group(2)), int(m.group(3))
                allowed[k] = list(range(lo, hi + 1))
                imms[k] = lo
                changed = True
        elif m := re.match(r"argument should be the value (.*)$", msg):
            vals = [int(x) for x in re.findall(r"\d+", m.group(1))]
            for k, v in imms.items():
                if v not in vals:
                    allowed[k] = vals
                    imms[k] = vals[0]
                    changed = True
        elif m := re.match(r"argument should be a multiple of (\d+)", msg):
            step = int(m.group(1))
            for k, v in imms.items():
                if v % step:
                    imms[k] = v - v % step
                    allowed.setdefault(k, [imms[k], imms[k] + step])
                    changed = True
        elif "streaming" in msg and "__arm_streaming" not in attrs and "non-streaming" not in msg:
            attrs += " __arm_streaming"
            changed = True
        elif "ZA state" in msg and '"za"' not in attrs:
            attrs += ' __arm_inout("za")'
            changed = True
        elif "ZT0 state" in msg and "zt0" not in attrs:
            attrs += ' __arm_inout("zt0")'
            changed = True
    return attrs if changed else None


def first_error(err):
    lines = [x for x in err.splitlines() if "error:" in x]
    return re.sub(r"^.*?error: ", "", lines[0]) if lines else (err.strip() or "?")


def probe(cc, pre, b, ret, params, work, extra=()):
    """Compiles builtin `b` until clang accepts it: immediates found from
    clang's errors (clang reports a non-constant immediate at the call, so
    every integer operand starts as an immediate and each is then tried as
    a run-time operand), streaming / ZA state added. Returns (ir, imms,
    attrs, allowed) or (None, error)."""
    imms, allowed, attrs = {}, {}, ""
    path = work / (b + ".cpp")

    def settle():
        nonlocal attrs
        for _ in range(12):
            src, _ = tu(pre, b, ret, params, imms, attrs)
            ir, err = compile_tu(cc, src, path, extra)
            if ir is not None:
                return ir, None
            a = fix_errors(err, imms, allowed, attrs)
            if a is None:
                return None, first_error(err)
            attrs = a
        return None, "no convergence"

    ir, err = settle()
    if ir is None and "must be a constant integer" in err:
        for k, p in enumerate(params):
            if INTLIKE.match(p):
                # An enum's value must be in its range to be constant.
                imms[k] = 1 if p.startswith("enum") else SENTINEL + k
        ir, err = settle()
        for k in sorted(imms):
            v = imms.pop(k)
            src, _ = tu(pre, b, ret, params, imms, attrs)
            ir2, err2 = compile_tu(cc, src, path, extra)
            if ir2 is None:
                imms[k] = v
            else:
                allowed.pop(k, None)
                ir = ir2
        if imms:
            src, _ = tu(pre, b, ret, params, imms, attrs)
            ir, err = compile_tu(cc, src, path, extra)
            err = first_error(err) if ir is None else None
    path.unlink(missing_ok=True)
    if ir is None:
        return None, err
    return ir, imms, attrs, allowed


# ---- IR -> recipe ---------------------------------------------------------

def function_body(ir, name):
    m = re.search(r"^define [^\n]*@" + name + r"\(\)[^\n]*\{\n(.*?)^\}", ir, re.M | re.S)
    return [ln.strip() for ln in m.group(1).splitlines() if ln.strip()]


def declarations(ir):
    """The intrinsic declarations in module order: (name, full declare
    line, attribute group text)."""
    groups = dict(re.findall(r"^attributes (#\d+) = \{ (.*) \}$", ir, re.M))
    out = []
    for line in re.findall(r"^declare .*$", ir, re.M):
        m = re.search(r"@([\w.]+)\(", line)
        if not m.group(1).startswith("llvm."):
            continue
        g = re.search(r"\) (#\d+)$", line)
        out.append((m.group(1), re.sub(r" #\d+$", "", line), groups.get(g.group(1), "") if g else ""))
    return out


VAL = re.compile(r"%(\d+)")


def recipe(ir):
    """The caller `w`'s body as recipe steps: `A k` (operand k), the
    instructions with `%N` renamed to the step index, and `R x` (the
    result)."""
    steps, names = [], {}
    for ln in function_body(ir, "w"):
        m = re.match(r"(?:%(\d+) = )?(.*)$", ln)
        lhs, rhs = m.group(1), m.group(2)
        g = re.match(r"call (.*) @g(\d+)\(\)$", rhs)
        if g:
            names[lhs] = len(steps)
            steps.append(f"A {g.group(2)} {g.group(1)}")
            continue
        rhs = VAL.sub(lambda v: f"%{names[v.group(1)]}", rhs)
        if rhs.startswith("ret "):
            steps.append("R " + rhs[4:])
            continue
        if lhs is not None:
            names[lhs] = len(steps)
        steps.append(rhs)
    return steps


def tokens(steps):
    return re.findall(r"-?\d+|[^\d]+", "\n".join(steps))


def lower(cc, pre, b, ret, params, work, extra=()):
    """The recipe of builtin `b` under compiler `cc`: its steps, with each
    immediate operand k shown as `#k` (or `#k*s+o`), and its declarations.
    """
    r = probe(cc, pre, b, ret, params, work, extra)
    if r[0] is None:
        return {"error": r[1]}
    ir, imms, attrs, allowed = r
    steps = recipe(ir)
    base = tokens(steps)
    marks = {}
    for k in sorted(imms):
        v1 = imms[k]
        others = [v for v in allowed.get(k, [v1 + 1]) if v != v1]
        if not others:
            continue
        v2 = others[-1]
        alt = dict(imms)
        alt[k] = v2
        src, _ = tu(pre, b, ret, params, alt, attrs)
        ir2, err = compile_tu(cc, src, work / (b + ".2.cpp"), extra)
        (work / (b + ".2.cpp")).unlink(missing_ok=True)
        if ir2 is None:
            return {"error": f"imm {k}={v2}: " + first_error(err)}
        t2 = tokens(recipe(ir2))
        if len(t2) != len(base):
            return {"error": f"imm {k} changes the shape"}
        for i, (x, y) in enumerate(zip(base, t2)):
            if x != y:
                c1, c2 = int(x), int(y)
                s = (c2 - c1) // (v2 - v1)
                o = c1 - s * v1
                if s * (v2 - v1) != c2 - c1:
                    return {"error": f"imm {k}: {c1},{c2} not linear"}
                marks[i] = f"#{k}" + (f"*{s}" if s != 1 else "") + (f"+{o}" if o else "")
    toks = [marks.get(i, t) for i, t in enumerate(base)]
    steps = "".join(toks).split("\n")
    return {"steps": steps, "decls": declarations(ir), "imms": imms, "attrs": attrs.strip()}


def run_all(cc, pre, decls, work, jobs, extra=()):
    work.mkdir(parents=True, exist_ok=True)
    out = {}
    with concurrent.futures.ThreadPoolExecutor(jobs) as ex:
        futs = {ex.submit(lower, cc, pre, b, r, p, work, extra): b for b, (r, p) in decls.items()}
        for i, f in enumerate(concurrent.futures.as_completed(futs)):
            out[futs[f]] = f.result()
            if i % 500 == 0:
                print(f"  {cc}: {i}/{len(futs)}", file=sys.stderr)
    return out


# ---- recipe -> table --------------------------------------------------------

ELT = {"half": "h", "bfloat": "bf", "float": "f", "double": "d", "ptr": "p"}
MANGLE = {"h": "f16", "bf": "bf16", "f": "f32", "d": "f64", "p": "p0", "c": "taarch64.svcountt"}
# Types written as such in a family; every other one is a slot of the row.
LITERAL = {"i32", "i64", "p", "void"}


class Text:
    """A cursor over one line of LLVM IR text."""

    WORD = re.compile(r"[^\s,()\[\]{}<>]+")

    def __init__(self, s):
        self.s, self.i = s, 0

    def ws(self):
        while self.i < len(self.s) and self.s[self.i] == " ":
            self.i += 1

    def peek(self, t):
        self.ws()
        return self.s.startswith(t, self.i)

    def eat(self, t):
        if not self.peek(t):
            raise ValueError(f"expected {t!r} at {self.i} in {self.s!r}")
        self.i += len(t)

    def word(self):
        self.ws()
        m = self.WORD.match(self.s, self.i)
        if not m:
            raise ValueError(f"expected a word at {self.i} in {self.s!r}")
        self.i = m.end()
        return m.group(0)

    def ty(self):
        """A type, abbreviated: `s4i32` (scalable), `v4i32`, `{a,b}`, `c`
        (`target("aarch64.svcount")`), `h` `bf` `f` `d` `p`, `iN`, `void`."""
        if self.peek("<"):
            self.eat("<")
            k = "v"
            if self.peek("vscale x"):
                self.eat("vscale x")
                k = "s"
            n = self.word()
            self.eat("x")
            e = self.ty()
            self.eat(">")
            return f"{k}{n}{e}"
        if self.peek("{"):
            self.eat("{")
            f = [self.ty()]
            while self.peek(","):
                self.eat(",")
                f.append(self.ty())
            self.eat("}")
            return "{" + ",".join(f) + "}"
        if self.peek('target("aarch64.svcount")'):
            self.eat('target("aarch64.svcount")')
            return "c"
        w = self.word()
        return ELT.get(w, w)


def mangle(t):
    """LLVM's intrinsic-name mangling of the abbreviated type `t`."""
    if t in MANGLE:
        return MANGLE[t]
    if m := re.fullmatch(r"([sv])(\d+)(.+)", t):
        return ("nx" if m.group(1) == "s" else "") + "v" + m.group(2) + mangle(m.group(3))
    return t


def struct_fields(t):
    return t[1:-1].split(",") if t.startswith("{") else []


class Family:
    """One recipe written as a family: its types become row slots."""

    def __init__(self):
        self.types = []

    def tok(self, t):
        if t in LITERAL:
            return t
        if t not in self.types:
            self.types.append(t)
        return f"${self.types.index(t)}"

    def value(self, x, t):
        """The operand `x` of type `t`."""
        w = x.word()
        if w.startswith("%"):
            return w
        if w.startswith("#"):
            return self.tok(t) + w
        k = {"poison": "P", "undef": "U", "zeroinitializer": "Z", "false": "0", "true": "1"}.get(w, w)
        if not re.fullmatch(r"[PUZ]|-?\d+", k):
            raise ValueError(f"constant {w!r}")
        return f"{self.tok(t)}:{k}"

    def typed(self, x):
        t = x.ty()
        return self.value(x, t)


def name_template(fam, name, types, bases):
    """The intrinsic `name` as its base name (added to `bases`) and the
    slots of the overload types it is mangled with."""
    cands = {}
    for t in types:
        for u in [t, *struct_fields(t)]:
            cands.setdefault(mangle(u), u)
    parts = name.split(".")
    suffix = []
    while len(parts) > 2:
        if parts[-2:] == ["taarch64", "svcountt"] and "taarch64.svcountt" in cands:
            suffix.insert(0, "c")
            parts = parts[:-2]
        elif parts[-1] in cands:
            suffix.insert(0, cands[parts[-1]])
            parts = parts[:-1]
        else:
            break
    base = ".".join(parts)
    if base not in bases:
        bases[base] = len(bases)
    return base, [fam.tok(u) for u in suffix]


def family_of(b, rec, proto, bases):
    """(family text, base names, slot types) of builtin `b`'s recipe."""
    fam = Family()
    ret, params = proto
    args = {}
    for st in rec["steps"]:
        if st.startswith("A "):
            _, k, t = st.split(" ", 2)
            args[int(k)] = Text(t).ty()
    for k in rec["imms"]:
        args[int(k)] = "#"
    rstep = rec["steps"][-1]
    rty = "void" if rstep == "R void" else Text(rstep[2:]).ty()
    sig = ["S", fam.tok(rty)] + [args[k] if args[k] == "#" else fam.tok(args[k])
                                 for k in range(len(params))]
    ops = [" ".join(sig)]
    names, used = [], []
    for name, line, attrs in rec["decls"]:
        x = Text(line)
        x.eat("declare")
        r = x.ty()
        x.eat("@" + name + "(")
        ps = []
        while not x.peek(")"):
            ps.append(x.ty())
            while not (x.peek(",") or x.peek(")")):
                if x.peek("captures(none)"):
                    x.eat("captures(none)")
                else:
                    x.word()
            if x.peek(","):
                x.eat(",")
        types = [r, *ps]
        base, suf = name_template(fam, name, types, bases)
        names.append(name)
        used.append(base)
        ops.append(" ".join(["D", f"@{len(used) - 1}" + "".join("." + s for s in suf),
                             fam.tok(r), *[fam.tok(p) for p in ps]]))
    for st in rec["steps"]:
        if st.startswith("A "):
            ops.append("a " + st.split()[1])
            continue
        if st == "R void":
            ops.append("r")
            continue
        if st.startswith("R "):
            ops.append("r " + fam.typed(Text(st[2:])))
            continue
        x = Text(st)
        op = x.word()
        if op == "call":
            x.ty()
            x.ws()
            m = re.compile(r"@([\w.]+)\(").match(x.s, x.i)
            x.i = m.end()
            vals = []
            while not x.peek(")"):
                vals.append(fam.typed(x))
                if x.peek(","):
                    x.eat(",")
            x.eat(")")
            ops.append(" ".join(["c", str(names.index(m.group(1))), *vals]))
        elif op in ("select", "insertelement", "shufflevector"):
            a = fam.typed(x)
            x.eat(",")
            b = fam.typed(x)
            x.eat(",")
            c = fam.typed(x)
            ops.append(" ".join([{"select": "sel", "insertelement": "ie",
                                  "shufflevector": "sv"}[op], a, b, c]))
        elif op == "extractvalue":
            a = fam.typed(x)
            x.eat(",")
            ops.append(f"ev {a} {x.word()}")
        elif op == "insertvalue":
            a = fam.typed(x)
            x.eat(",")
            v = fam.typed(x)
            x.eat(",")
            ops.append(f"iv {a} {v} {x.word()}")
        elif op == "getelementptr":
            t = x.ty()
            x.eat(",")
            base = fam.typed(x)
            x.eat(",")
            idx = fam.typed(x)
            ops.append(f"gep {fam.tok(t)} {base} {idx}")
        elif op in ("zext", "sext", "trunc", "bitcast"):
            v = fam.typed(x)
            x.eat("to")
            ops.append(f"{op} {v} {fam.tok(x.ty())}")
        elif op in ("shl", "mul", "add", "sub", "and", "or", "lshr"):
            flags = []
            while x.peek("nuw") or x.peek("nsw"):
                flags.append(x.word())
            t = x.ty()
            a = fam.value(x, t)
            x.eat(",")
            ops.append(f"{'.'.join([op, *flags])} {a} {fam.value(x, t)}")
        elif op == "icmp":
            pred = x.word()
            t = x.ty()
            a = fam.value(x, t)
            x.eat(",")
            ops.append(f"icmp.{pred} {a} {fam.value(x, t)}")
        else:
            raise ValueError(f"{b}: instruction {st!r}")
        x.ws()
        if x.i != len(x.s):
            raise ValueError(f"{b}: trailing text in {st!r}")
    return ";".join(ops), used, fam.types


def param_attrs(line):
    """`.` / `i` (immarg) / `c` (nocapture) per parameter of a declaration."""
    inner = line[line.index("(") + 1:line.rindex(")")]
    out = ""
    depth, cur = 0, ""
    for ch in inner + ",":
        if ch in "<{(":
            depth += 1
        elif ch in ">})":
            depth -= 1
        if ch == "," and depth == 0:
            if cur.strip():
                out += "i" if "immarg" in cur else "c" if "capture" in cur else "."
            cur = ""
        else:
            cur += ch
    return out


def emit(raw):
    """The text of crates/kiln_ast/src/sve_builtins.rs from the raw recipes."""
    d22, r22, r19 = raw["d22"], raw["r22"], raw["r19"]
    in19 = set(raw["d19"])
    bases, fams, types, attrs = {}, {}, {}, {}
    battrs = {}
    rows, failed = [], []

    def slot(lst, table):
        return ",".join(str(table.setdefault(t, len(table))) for t in lst)

    def spec(b, rec, ver):
        text, used, tys = family_of(b, rec, d22[b], bases)
        for (name, line, a), base in zip(rec["decls"], used):
            key = (base, ver)
            val = (attrs.setdefault(a, len(attrs)), param_attrs(line))
            if battrs.setdefault(key, val) != val:
                raise ValueError(f"{b}: {base} has two attribute sets for LLVM {ver}")
        f = fams.setdefault(text, len(fams))
        return f"{f}:{','.join(str(bases[u]) for u in used)}:{slot(tys, types)}"

    for b in sorted(r22):
        if "error" in r22[b]:
            failed.append((b, r22[b]["error"]))
            continue
        s22 = spec(b, r22[b], 22)
        if b not in in19:
            s19 = "-"
        elif "error" in r19.get(b, {"error": "missing"}):
            failed.append((b + " (clang 19)", r19.get(b, {}).get("error", "missing")))
            s19 = "-"
        else:
            s19 = spec(b, r19[b], 19)
            if s19 == s22:
                s19 = "="
        rows.append(f"{b[len('__builtin_'):]} {s22} {s19}")

    def lines(seq):
        return "\n".join(seq)

    blines = []
    for base, i in sorted(bases.items(), key=lambda x: x[1]):
        cols = [base]
        for ver in (22, 19):
            a = battrs.get((base, ver))
            cols += [str(a[0]), a[1] or "-"] if a else ["-", "-"]
        blines.append(" ".join(cols))
    out = [
        "// @generated by scripts/gen-sve-builtins.py from clang 22's and clang 19's",
        "// -O0 lowering of the SVE and SME ACLE builtins (arm_sve.h, arm_sme.h,",
        "// arm_neon_sve_bridge.h). Do not edit. Format: crate::sve.",
        "",
        "/// The function attribute sets of the intrinsics, one per line.",
        f'pub(crate) const ATTRS: &str = r"{lines(attrs)}";',
        "",
        "/// The intrinsics: `name attrs22 params22 attrs19 params19`.",
        f'pub(crate) const BASES: &str = r"{lines(blines)}";',
        "",
        "/// The types filling the families' slots.",
        f'pub(crate) const TYPES: &str = r"{lines(types)}";',
        "",
        "/// The families: a lowering with its types and intrinsics as slots.",
        f'pub(crate) const FAMILIES: &str = r"{lines(fams)}";',
        "",
        "/// The builtins, sorted: `name spec22 spec19` (`=`: the same, `-`: none).",
        f'pub(crate) const ROWS: &str = r"{lines(rows)}";',
        "",
    ]
    return "\n".join(out), failed


# ---- semantic checks: clang-tblgen's tables (WP-1792, WP-1793) ------------

# The tables each ACLE's `.td` file generates, by clang-tblgen backend.
TABLES = {"sve": ("builtins", "sema-rangechecks", "streaming-attrs"),
          "sme": ("builtins", "sema-rangechecks", "streaming-attrs", "builtin-za-state")}
# clang `SemaARM::ArmStreamingType` as one letter.
MODES = {"ArmStreaming": "s", "ArmStreamingCompatible": "c", "VerifyRuntimeMode": "v",
         "ArmNonStreaming": "n"}
# clang `ArmSMEState` (the ZA state in bits 0-1, the ZT0 state in 2-3).
STATES = {"ArmInZA": 1, "ArmOutZA": 2, "ArmInOutZA": 3,
          "ArmInZT0": 4, "ArmOutZT0": 8, "ArmInOutZT0": 12}
# One type of a builtin prototype string as SveEmitter writes it
# (`SVEType::builtin_str`): a vector or immediate prefix, a sign, the base
# type, `C` (const) and `*` (pointer).
PROTO_TYPE = re.compile(r"(q\d+|V\d+|I)?([SU])?(UWi|LLLi|Wi|Qa|[vbcsihyfdm])(C)?(\*)?")


def ice_args(proto):
    """The arguments of builtin prototype `proto` (return type first) that
    are integer constant expressions (`I`)."""
    out, i, k = [], 0, -1
    while i < len(proto):
        m = PROTO_TYPE.match(proto, i)
        if not m or m.end() == i:
            raise ValueError(f"prototype {proto!r} at {i}")
        if m.group(1) == "I":
            out.append(k)
        i, k = m.end(), k + 1
    return out


def tblgen(bin_, td, work, ver):
    """{(acle, table): text} of clang-tblgen `bin_` over the `.td` files
    in directory `td`."""
    out = {}
    for acle, tables in TABLES.items():
        for t in tables:
            dst = work / f"{acle}-{t}-{ver}.inc"
            subprocess.run([bin_, f"--gen-arm-{acle}-{t}", "-I", str(td),
                            str(pathlib.Path(td) / f"arm_{acle}.td"), "-o", str(dst)], check=True)
            out[(acle, t)] = dst.read_text()
    return out


def imm_kinds(td):
    """ImmCheckType number -> name (without `ImmCheck`) from the `.td`
    files in `td` (clang 22: arm_immcheck_incl.td; 19: arm_sve_sme_incl.td)."""
    kinds = {}
    for f in ("arm_immcheck_incl.td", "arm_sve_sme_incl.td"):
        path = pathlib.Path(td) / f
        if path.exists():
            for name, n in re.findall(r"^def ImmCheck(\w+)\s*:\s*ImmCheckType<(\d+)>", path.read_text(),
                                      re.M):
                kinds[int(n)] = name
    return kinds


def case_groups(text):
    """(case names, body lines) for each group of `case X::BI__builtin_*:`
    labels sharing a body."""
    groups, names, body = [], [], []
    for line in text.splitlines():
        line = line.strip()
        if m := re.match(r"case \w+::BI(__builtin_\w+):$", line):
            if body:
                groups.append((names, body))
                names, body = [], []
            names.append(m.group(1))
        elif line and not line.startswith("#") and names:
            body.append(line)
    if names:
        groups.append((names, body))
    return groups


def parse_checks(tabs, kinds, td):
    """builtin -> (features, mode, state, ice args, imm checks) of one clang.
    The NEON-SVE bridge builtins (`svget_neonq`, ...) are not in the `.td`
    files but in BuiltinsAArch64NeonSVEBridge.def of `td`."""
    out = {}
    bridge = (pathlib.Path(td) / "BuiltinsAArch64NeonSVEBridge.def").read_text()
    for acle in TABLES:
        text = tabs[(acle, "builtins")] + (bridge if acle == "sve" else "")
        # clang 22: `Builtin::Info{...StrOffsets{n /* name */, n /* proto */,
        # n /* attrs */, n /* features */}...`; 19: `TARGET_BUILTIN(...)`.
        rows = re.findall(r"StrOffsets\{\d+ /\* (\w+) \*/, \d+ /\* (\S*) \*/, \d+ /\* \S* \*/, "
                          r"\d+ /\* (\S*) \*/\}", text)
        rows = [(f"__builtin_{acle}_{n}", p, f) for n, p, f in rows]
        rows += re.findall(r'TARGET_BUILTIN\((__builtin_\w+), "([^"]*)", "[^"]*", "([^"]*)"\)', text)
        for b, proto, feats in rows:
            out[b] = {"features": feats, "mode": "-", "state": 0, "ice": ice_args(proto),
                      "imms": []}
        for names, body in case_groups(tabs[(acle, "sema-rangechecks")]):
            imms = []
            for line in body:
                if m := re.search(r"(?:emplace_back|make_tuple)\((\d+), (\d+), (\d+)\)", line):
                    imms.append((int(m.group(1)), kinds[int(m.group(2))], int(m.group(3))))
            for b in names:
                out[b]["imms"] = imms
        for names, body in case_groups(tabs[(acle, "streaming-attrs")]):
            mode = MODES[re.search(r"BuiltinType = (\w+);", " ".join(body)).group(1)]
            for b in names:
                out[b]["mode"] = mode
        if (acle, "builtin-za-state") in tabs:
            for names, body in case_groups(tabs[(acle, "builtin-za-state")]):
                state = sum(STATES[x] for x in re.findall(r"Arm\w+", " ".join(body)))
                for b in names:
                    out[b]["state"] = state
    return out


def emit_checks(c22, c19):
    """The text of crates/kiln_ast/src/sve_checks.rs."""
    feats = {}

    def spec(r):
        f = feats.setdefault(r["features"], len(feats))
        imms = ",".join(f"{a}.{k}.{w}" for a, k, w in r["imms"])
        ice = ".".join(str(k) for k in r["ice"])
        return f"{f}:{r['mode']}:{r['state']}:{ice}:{imms}"

    rows = []
    for b in sorted(c22.keys() | c19.keys()):
        s22 = spec(c22[b]) if b in c22 else "-"
        s19 = spec(c19[b]) if b in c19 else "-"
        rows.append(f"{b[len('__builtin_'):]} {s22} {'=' if s19 == s22 else s19}")
    out = [
        "// @generated by scripts/gen-sve-builtins.py --checks from clang-tblgen's",
        "// SVE and SME tables of clang 22 and clang 19 (arm_sve.td, arm_sme.td).",
        "// Do not edit. Format: crate::sve::checks.",
        "",
        "/// The required-feature expressions, one per line.",
        f'pub(crate) const FEATURES: &str = r"{chr(10).join(feats)}";',
        "",
        "/// The builtins, sorted: `name spec22 spec19` (`=`: the same, `-`: none).",
        f'pub(crate) const CHECKS: &str = r"{chr(10).join(rows)}";',
        "",
    ]
    return "\n".join(out)


# ---- census: kiln against clang ----------------------------------------------

def expected(rec):
    """clang's steps of `rec` with its immediates' values put back."""
    def put(m):
        k, sc, o = int(m.group(1)), int(m.group(2) or 1), int(m.group(3) or 0)
        return str(rec["imms"][str(k)] * sc + o)
    return [re.sub(r"#(\d+)(?:\*(-?\d+))?(?:\+(-?\d+))?", put, st) for st in rec["steps"]]


def census_one(kiln, pre, b, proto, rec, work, ver):
    """`(status, detail)` of kiln's lowering of `b` against clang `ver`'s."""
    ret, params = proto
    imms = {int(k): v for k, v in rec["imms"].items()}
    src, _ = tu(pre, b, ret, params, imms, rec["attrs"])
    path = work / f"{b}.{ver}.cpp"
    path.write_text(src)
    ll = path.with_suffix(".ll")
    cc = "clang++-22" if ver == 22 else "clang++-19"
    extra = [] if ver == 22 else ["-fclang-compat=19"]
    r = subprocess.run([kiln, TARGET, march(cc), "-S", "-emit-llvm", "-O0", "-fno-crash-diagnostics",
                        *extra, "-o", str(ll), str(path)], capture_output=True, text=True)
    path.unlink()
    if r.returncode != 0:
        ll.unlink(missing_ok=True)
        return "error", " ".join(r.stderr.split())[:300]
    ir = ll.read_text()
    ll.unlink()
    got = recipe(ir)
    want = expected(rec)
    if got != want:
        for g, w in zip(got + ["<end>"] * len(want), want + ["<end>"] * len(got)):
            if g != w:
                return "ir", f"kiln: {g} | clang: {w}"
    gd, wd = declarations(ir), [tuple(x) for x in rec["decls"]]
    if gd != wd:
        for g, w in zip(gd + [("<end>",)] * len(wd), wd + [("<end>",)] * len(gd)):
            if g != w:
                return "decl", f"kiln: {' '.join(g)} | clang: {' '.join(w)}"
    return "ok", ""


def census(kiln, raw, work, jobs, only, out):
    """Compiles every census translation unit with kiln and writes the
    comparison with clang 22 (and 19) to `out` as TSV."""
    work.mkdir(parents=True, exist_ok=True)
    _, pre = parse_headers(clang_include("clang++-22"))
    d22 = raw["d22"]
    tasks = []
    for ver in (22, 19):
        recs = raw["r22" if ver == 22 else "r19"]
        for b in sorted(recs):
            if only and not re.search(only, b):
                continue
            if "error" not in recs[b]:
                tasks.append((b, ver, recs[b]))
    rows = []
    with concurrent.futures.ThreadPoolExecutor(jobs) as ex:
        futs = {ex.submit(census_one, kiln, pre, b, d22[b], rec, work, ver): (b, ver)
                for b, ver, rec in tasks}
        for i, f in enumerate(concurrent.futures.as_completed(futs)):
            b, ver = futs[f]
            st, det = f.result()
            rows.append((b, ver, st, det))
            if i % 1000 == 0:
                print(f"  census: {i}/{len(futs)}", file=sys.stderr)
    rows.sort()
    pathlib.Path(out).write_text("".join(f"{b}\t{v}\t{s}\t{d}\n" for b, v, s, d in rows))
    for ver in (22, 19):
        c = {}
        for _, v, st, _ in rows:
            if v == ver:
                c[st] = c.get(st, 0) + 1
        print(f"clang {ver}: " + " ".join(f"{k}={n}" for k, n in sorted(c.items())))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--work", required=True)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--only")
    ap.add_argument("--raw", help="write the raw recipes as JSON here")
    ap.add_argument("--from-raw", help="emit the table from this JSON, compiling nothing")
    ap.add_argument("--census", metavar="KILN", help="compare KILN's lowering with --from-raw's")
    ap.add_argument("--out", help="the census TSV")
    ap.add_argument("--checks", action="store_true", help="write the semantic checks' table")
    for v in ("22", "19"):
        ap.add_argument(f"--td{v}", help=f"clang {v}'s clang/include/clang/Basic")
        ap.add_argument(f"--tblgen{v}", default=f"clang-tblgen-{v}", help=f"clang {v}'s clang-tblgen")
    a = ap.parse_args()
    work = pathlib.Path(a.work)
    if a.checks:
        work.mkdir(parents=True, exist_ok=True)
        c22 = parse_checks(tblgen(a.tblgen22, a.td22, work, 22), imm_kinds(a.td22), a.td22)
        c19 = parse_checks(tblgen(a.tblgen19, a.td19, work, 19), imm_kinds(a.td19), a.td19)
        CHECKS_OUT.write_text(emit_checks(c22, c19))
        return
    if a.census:
        census(a.census, json.loads(pathlib.Path(a.from_raw).read_text()), work, a.jobs, a.only,
               a.out)
        return
    if a.from_raw:
        text, failed = emit(json.loads(pathlib.Path(a.from_raw).read_text()))
        OUT.write_text(text)
        for b, e in failed:
            print(f"not lowered: {b}: {e}", file=sys.stderr)
        return
    d22, pre22 = parse_headers(clang_include("clang++-22"))
    d19, _ = parse_headers(clang_include("clang++-19"))
    if a.only:
        d22 = {b: v for b, v in d22.items() if re.search(a.only, b)}
    skip = {b for b, (r, p) in d22.items() if UNSUPPORTED_TYPES.search(" ".join([r, *p]))}
    todo = {b: v for b, v in d22.items() if b not in skip}
    print(f"{len(d22)} builtins, {len(skip)} with FP8 types", file=sys.stderr)
    r22 = run_all("clang++-22", pre22, todo, work / "c22", a.jobs)
    r19 = run_all("clang++-19", pre22, {b: v for b, v in todo.items() if b in d19},
                  work / "c19", a.jobs)
    raw = {"d22": d22, "d19": sorted(d19), "skip": sorted(skip), "r22": r22, "r19": r19}
    if a.raw:
        pathlib.Path(a.raw).write_text(json.dumps(raw, indent=1))


if __name__ == "__main__":
    main()
