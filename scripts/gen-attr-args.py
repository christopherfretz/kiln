#!/usr/bin/env python3
"""Regenerate crates/kiln_sema/src/attr_args_table.rs and
docs/status/attr-args.tsv from the clang 19 and clang 22 oracles.

The table records, for every `__attribute__((x))`, `[[gnu::x]]` and
`[[clang::x]]` attribute clang knows on aarch64-unknown-linux-gnu and
x86_64-unknown-linux-gnu, how clang parses its arguments and how many it
takes (spec/parse.md §6.2 "Argument table"; WP-1879):

- whether the arguments are parsed at all (clang `ParseAttributeArgsCommon`)
  or by a parser of the attribute's own (left as balanced tokens by kiln);
- whether a leading identifier is an identifier argument (clang
  `attributeHasIdentifierArg`) rather than an expression;
- the argument count clang's `handleCommonAttributeFeatures` checks:
  exactly N, at least N, at most M, or unchecked.

Method: every candidate name (the attribute keys of
crates/kiln_pp/src/features_table.rs, which gen-features.sh measured) is
written with each probe argument list below in each probe position (a
function, a variable, a class, a parameter, a statement, a type, ...), one
per line, so that clang's diagnostics map back to (name, position, form).
clang checks the subject first and the count second, so the count comes
from a position the attribute appertains to. Parse probes use undeclared
identifiers: "use of undeclared identifier" at one means that argument was
parsed as an expression. The derived model is checked against every probe
(a count diagnostic the model does not predict, or one it predicts at a
position where clang gave none, fails the run).

Idempotent (byte-identical output for the same clangs). Runtime: about a
minute (64 clang runs of a few thousand lines each).

Usage: scripts/gen-attr-args.py            (clang++-19 and clang++-22 on PATH)
       CLANGXX=... CLANGXX22=... JOBS=4 scripts/gen-attr-args.py
"""
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

WS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FEAT = os.path.join(WS, "crates/kiln_pp/src/features_table.rs")
OUT_RS = os.path.join(WS, "crates/kiln_sema/src/attr_args_table.rs")
OUT_TSV = os.path.join(WS, "docs/status/attr-args.tsv")
CLANGS = [(19, os.environ.get("CLANGXX", "clang++-19")), (22, os.environ.get("CLANGXX22", "clang++-22"))]
TARGETS = [("aarch64-unknown-linux-gnu", False), ("x86_64-unknown-linux-gnu", True)]
JOBS = int(os.environ.get("JOBS", "4"))

# Spelling bits, as kiln_sema::attr_triage.
G, GS, CL = 1, 2, 4
SPELLINGS = [(G, None), (GS, "gnu"), (CL, "clang")]

# Positions probed for a string literal; the last one stands for every
# later position.
NSTR = 5
S8 = ", ".join(['"s"'] * 8)
I8 = ", ".join(["1"] * 8)
# Count probes (every position) and parse probes (a function declaration).
COUNT_FORMS = [("p0", ""), ("i1", "(1)"), ("i8", f"({I8})"), ("s1", '("s")'), ("s8", f"({S8})")]
PARSE_FORMS = [
    ("u1", "(kiln_u1)"),
    ("uu", "(kiln_u1, kiln_u2)"),
    ("iu", "(1, kiln_u2)"),
    ("su", '("s", kiln_u2)'),
] + [
    # String-literal positions (clang `attributeStringLiteralListArg`):
    # `1` at position k of five string literals.
    (f"k{k}", "(" + ", ".join("1" if i == k else '"s"' for i in range(NSTR)) + ")")
    for k in range(NSTR)
]

# Probe positions: (id, GNU form, C++11 form). `A` is the attribute, `@`
# the line's unique suffix.
POSITIONS = [
    ("fn", "A void f@();", "[[A]] void f@();"),
    ("fndef", "A void fd@() {}", "[[A]] void fd@() {}"),
    ("inline", "A inline void fi@() {}", "[[A]] inline void fi@() {}"),
    ("fnptr", "A void *fp@(void *, int, int);", "[[A]] void *fp@(void *, int, int);"),
    ("fnva", "A void fv@(const char *, ...);", "[[A]] void fv@(const char *, ...);"),
    ("fntmpl", "template <class T> A void ftm@(T);", "template <class T> [[A]] void ftm@(T);"),
    ("var", "A int v@;", "[[A]] int v@;"),
    ("pvar", "A int *pv@;", "[[A]] int *pv@;"),
    ("tls", "A thread_local int tl@;", "[[A]] thread_local int tl@;"),
    ("vartmpl", "template <class T> A T vt@;", "template <class T> [[A]] T vt@;"),
    ("class", "struct A s@ {};", "struct [[A]] s@ {};"),
    ("classtmpl", "template <class T> struct A cs@ {};", "template <class T> struct [[A]] cs@ {};"),
    ("union", "union A u@ { int a; unsigned b; };", "union [[A]] u@ { int a; unsigned b; };"),
    ("enum", "enum A e@ { e@a };", "enum [[A]] e@ { e@a };"),
    ("enumerator", "enum ee@ { ee@a A };", "enum ee@ { ee@a [[A]] };"),
    ("param", "void p@(int a A);", "void p@([[A]] int a);"),
    ("pparam", "void pp@(int *a A);", "void pp@([[A]] int *a);"),
    ("field", "struct fl@ { int m A; };", "struct fl@ { [[A]] int m; };"),
    ("bitfield", "struct bf@ { int m : 3 A; };", "struct bf@ { [[A]] int m : 3; };"),
    ("method", "struct m@ { A void f(); };", "struct m@ { [[A]] void f(); };"),
    ("vmethod", "struct vm@ { A virtual void f(); };", "struct vm@ { [[A]] virtual void f(); };"),
    ("ctor", "struct c@ { A c@(); };", "struct c@ { [[A]] c@(); };"),
    ("typedef", "typedef int t@ A;", "using t@ [[A]] = int;"),
    ("type", "int A *ty@;", "int [[A]] *ty@;"),
    ("ptype", "int * A pt@;", "int * [[A]] pt@;"),
    ("fntype", "void ft@() A;", "void ft@() [[A]];"),
    ("ns", "namespace n@ A {}", "namespace [[A]] n@ {}"),
    ("using", "namespace un@ { int x@; } using un@::x@ A;", "namespace un@ { int x@; } using un@::x@ [[A]];"),
    ("stmt", "void st@() { A return st@(); }", "void st@() { [[A]] return st@(); }"),
    ("compound", "void cp@() { A { } }", "void cp@() { [[A]] { } }"),
    ("loop", "void lp@() { A for (;;) {} }", "void lp@() { [[A]] for (;;) {} }"),
    ("null", "void nl@(int x) { switch (x) { case 0: x = 1; A; case 1: break; } }",
     "void nl@(int x) { switch (x) { case 0: x = 1; [[A]]; case 1: break; } }"),
    ("local", "void lv@() { A int x; (void)x; }", "void lv@() { [[A]] int x; (void)x; }"),
    ("label", "void lb@() { l: A; goto l; }", "void lb@() { [[A]] l:; goto l; }"),
]

# Late-parsing and pack probes (clang `LateParsed`: the arguments of a
# member's or a function's GNU attribute are parsed after the class or
# with the parameters in scope; clang `attributeAcceptsExprPack`).
EXTRA = [
    ("lmember", "(kiln_late)", "struct lm@ { int x A; void f() A; int kiln_late; };",
     "struct lm@ { [[A]] int x; [[A]] void f(); int kiln_late; };"),
    ("lparam", "(kiln_p)", "void lp@(int kiln_p) A;", "void lp@(int kiln_p) [[A]];"),
    ("pack", "(I...)", "template <int... I> A void pk@();", "template <int... I> [[A]] void pk@();"),
]

DIAG = re.compile(r"^[^:]*:(\d+):(\d+): (error|warning): (.*)$")
COUNT = re.compile(
    r"^'([^']+)' attribute (takes no arguments|takes one argument|requires exactly (\d+) arguments"
    r"|takes at least (\d+) arguments?|takes no more than (\d+) arguments?)$")
# A parse error: the attribute is dropped before its count is checked.
PARSE_ERROR = re.compile(r"^expected |expects |requires parentheses")
NOT_APPLY = re.compile(r"only applies to|cannot be applied to|attribute ignored|is ignored|"
                       r"not supported|unknown attribute|cannot appear here|"
                       r"declaration cannot be|appertain")


def candidates():
    """The attribute names gen-features.sh measured: (GNU names, {scope: names})."""
    text = open(FEAT).read()
    gnu, scoped = set(), {"gnu": set(), "clang": set()}

    def block(name):
        m = re.search(r"static " + name + r": &\[Entry\] = &\[(.*?)\n\];", text, re.S)
        return re.findall(r'Entry\("([^"]+)"', m.group(1))

    for k in block("ATTRIBUTE"):
        gnu.add(k)
    for k in block("CPP_ATTRIBUTE"):
        if "::" in k:
            s, n = k.split("::", 1)
            if s in scoped:
                scoped[s].add(n)
    for kind, k in re.findall(r'Delta\(Kind::(Attribute|CppAttribute), "([^"]+)"', text):
        if kind == "Attribute":
            gnu.add(k)
        elif "::" in k:
            s, n = k.split("::", 1)
            if s in scoped:
                scoped[s].add(n)
    return {G: sorted(gnu), GS: sorted(scoped["gnu"]), CL: sorted(scoped["clang"])}


def spell(bit, name, args):
    if bit == G:
        return f"__attribute__(({name}{args}))"
    scope = "gnu" if bit == GS else "clang"
    return f"{scope}::{name}{args}"


def probe_lines(bit, names, forms, positions):
    """The probe lines: [((name, position, form), source line)]."""
    out = []
    for ni, name in enumerate(names):
        for fid, args in forms:
            for pi, (pid, gform, cform) in enumerate(positions):
                attr = spell(bit, name, args)
                tmpl = gform if bit == G else cform
                # `A` is the only stand-alone `A` token of each template.
                src = re.sub(r"(?<![A-Za-z0-9_])A(?![A-Za-z0-9_])", lambda _: attr, tmpl)
                out.append(((name, pid, fid), src.replace("@", f"_{ni}_{fid}_{pi}")))
    return out


def run_clang(cxx, target, src):
    """clang's diagnostics for `src` by line, or None when clang crashed."""
    with tempfile.NamedTemporaryFile("w", suffix=".cpp", delete=False, dir=os.environ.get("TMPDIR")) as f:
        f.write(src)
        path = f.name
    try:
        p = subprocess.run(
            [cxx, "-fsyntax-only", "-std=c++26", f"--target={target}", "-ferror-limit=0",
             "-fno-caret-diagnostics", "-fno-diagnostics-show-option", "-fno-crash-diagnostics",
             path],
            capture_output=True, text=True)
    finally:
        os.unlink(path)
    if p.returncode not in (0, 1) or "PLEASE submit a bug report" in p.stderr:
        return None
    diags = {}
    for line in p.stderr.splitlines():
        m = DIAG.match(line)
        if m:
            diags.setdefault(int(m.group(1)), []).append((m.group(3), m.group(4)))
    return diags


def run_lines(cxx, target, lines):
    """{key: diagnostics} for probe lines; a line that crashes clang (found
    by bisection) gets [("crash", "")]."""
    src = "int kiln_dummy;\n" + "\n".join(l for _, l in lines) + "\n"
    d = run_clang(cxx, target, src)
    if d is None:
        if len(lines) == 1:
            return {lines[0][0]: [("crash", "")]}
        mid = len(lines) // 2
        r = run_lines(cxx, target, lines[:mid])
        r.update(run_lines(cxx, target, lines[mid:]))
        return r
    return {key: d.get(i + 2, []) for i, (key, _) in enumerate(lines)}


def probe(cfg):
    """Everything one (clang, target) says: {(bit, name): {(position, form): diags}}."""
    (ver, cxx), (target, x86) = cfg
    jobs = []
    for bit, names in candidates().items():
        jobs.append((bit, probe_lines(bit, names, COUNT_FORMS, POSITIONS)))
        jobs.append((bit, probe_lines(bit, names, PARSE_FORMS, POSITIONS[:1])))
        for pid, args, gform, cform in EXTRA:
            jobs.append((bit, probe_lines(bit, names, [(pid, args)], [(pid, gform, cform)])))
    with ThreadPoolExecutor(JOBS) as ex:
        outs = list(ex.map(lambda j: (j[0], run_lines(cxx, target, j[1])), jobs))
    res = {}
    for bit, d in outs:
        for (name, pid, fid), diags in d.items():
            res.setdefault((bit, name), {})[(pid, fid)] = diags
    return res


ARGC = {"p0": 0, "i1": 1, "s1": 1, "i8": 8, "s8": 8}
UNBOUNDED = 255


def count_msg(ds):
    """The argument-count diagnostic among `ds`, as (kind, n)."""
    for _, m in ds:
        c = COUNT.match(m)
        if not c:
            continue
        t = c.group(2)
        if t == "takes no arguments":
            return ("exactly", 0)
        if t == "takes one argument":
            return ("exactly", 1)
        n = int(re.search(r"\d+", t).group())
        if t.startswith("requires exactly"):
            return ("exactly", n)
        return ("at least", n) if t.startswith("takes at least") else ("at most", n)
    return None


def predicted(lo, hi, argc):
    """clang `handleCommonAttributeFeatures` for `argc` arguments."""
    if lo == hi:
        return None if argc == lo else ("exactly", lo)
    if lo and argc < lo:
        return ("at least", lo)
    if hi != UNBOUNDED and argc > hi:
        return ("at most", hi)
    return None


def has(ds, text):
    return any(text in m for _, m in ds)


def derive(rec):
    """The model of one attribute under one (clang, target, spelling), or
    None when clang does not know it there. Returns (record, problems)."""
    if has(rec.get(("fn", "p0"), []), "unknown attribute"):
        return None, []
    # clang checks the subject before the count, so the count is read at
    # the positions where some probe gets a count diagnostic. Elsewhere
    # clang never counts (another subject; a type attribute's position).
    positions = []
    for pid, _, _ in POSITIONS:
        lines = [rec.get((pid, f), []) for f in ARGC]
        if not any(l and l[0][0] == "crash" for l in lines) and any(count_msg(l) for l in lines):
            positions.append(pid)
    seen = {count_msg(rec.get((pid, f), [])) for pid in positions for f in ARGC} - {None}
    exact = sorted({n for k, n in seen if k == "exactly"})
    lo = max([n for k, n in seen if k == "at least"], default=0)
    hi = min([n for k, n in seen if k == "at most"], default=UNBOUNDED)
    if exact:
        lo = hi = exact[0]
    fn = lambda f: rec.get(("fn", f), [])
    und = lambda f, u: has(fn(f), f"undeclared identifier '{u}'")
    strl = lambda f: has(fn(f), "expected string literal as argument of")
    strings = sum(1 << k for k in range(NSTR) if strl(f"k{k}"))
    parsed = any(und(f, u) for f in ("u1", "uu", "iu", "su") for u in ("kiln_u1", "kiln_u2")) \
        or strings != 0
    late = und("u1", "kiln_u1") and not any(
        has(rec.get((p, p), []), f"undeclared identifier '{u}'")
        for p, u in (("lmember", "kiln_late"), ("lparam", "kiln_p")))
    pk = rec.get(("pack", "pack"), [])
    pack = not has(pk, "does not support argument pack expansion") and not any(
        k == "error" and PARSE_ERROR.search(m) for k, m in pk)
    if not parsed:
        parse = "custom"
    elif und("u1", "kiln_u1") or strl("u1"):
        parse = "expr"
    else:
        parse = "ident"
    if parse == "custom":
        # A parser of its own (`availability`, a type argument): kiln
        # leaves the arguments to the attribute's handler.
        lo, hi, positions = 0, UNBOUNDED, []
    problems = []
    if len(exact) > 1:
        problems.append(f"conflicting counts {sorted(seen)}")
    # The model must reproduce every count probe at those positions (a
    # probe whose arguments clang rejected while parsing them says
    # nothing).
    for pid in positions:
        for f, argc in ARGC.items():
            ds = rec.get((pid, f), [])
            if any(k == "error" and PARSE_ERROR.search(m) for k, m in ds):
                continue
            want, got = predicted(lo, hi, argc), count_msg(ds)
            if want != got:
                problems.append(f"{pid}/{f}: model {want}, clang {got}")
    r = {"parse": parse, "late": late, "pack": pack, "strings": strings, "min": lo, "max": hi,
         "positions": positions}
    return r, problems


# Row flags (crates/kiln_sema/src/attr_args.rs).
F_IDENT, F_CUSTOM, F_LATE, F_PACK = 1, 2, 4, 8
F_STRINGS_SHIFT = 4
SPELLING_INDEX = {G: 0, GS: 1, CL: 2}
SPELLING_NAME = {G: "__attribute__", GS: "gnu::", CL: "clang::"}


def flags_of(r):
    f = r["strings"] << F_STRINGS_SHIFT
    if r["parse"] == "ident":
        f |= F_IDENT
    if r["parse"] == "custom":
        f |= F_CUSTOM
    if r["late"]:
        f |= F_LATE
    if r["pack"]:
        f |= F_PACK
    return f


def clang_version(cxx):
    out = subprocess.run([cxx, "-E", "-P", "-x", "c++", "-"], input="__clang_version__\n",
                         capture_output=True, text=True).stdout
    return out.strip().strip('"').split()[0]


def main():
    configs = [(c, t) for t in TARGETS for c in CLANGS]
    # Config index as kiln_sema::attr_triage::Row: aarch64 at 19, at 22,
    # x86-64 at 19, at 22.
    results, problems = {}, []
    for ci, cfg in enumerate(configs):
        for (bit, name), rec in probe(cfg).items():
            r, pr = derive(rec)
            for p in pr:
                problems.append(f"clang {cfg[0][0]} {cfg[1][0]} {SPELLING_NAME[bit]}{name}: {p}")
            if r is not None:
                results[(name, bit, ci)] = r
    if problems:
        sys.stderr.write("gen-attr-args: the model does not reproduce clang:\n  "
                         + "\n  ".join(problems) + "\n")
        sys.exit(1)
    versions = " and ".join(f"clang {clang_version(cxx)}" for _, cxx in CLANGS)
    # TSV: one line per attribute, spelling and distinct model.
    groups = {}
    for (name, bit, ci), r in results.items():
        key = (name, r["min"], r["max"], flags_of(r))
        groups.setdefault(key, [0, {}])
        groups[key][0] |= 1 << (SPELLING_INDEX[bit] * 4 + ci)
        if r["positions"]:
            groups[key][1].setdefault(r["positions"][0], 0)
    cfg_names = ["a64-19", "a64-22", "x86-19", "x86-22"]
    tsv = [f"# @generated by scripts/gen-attr-args.py from {versions}. Do not edit.",
           "# name\tspellings (* = aarch64 and x86-64 at clang 19 and 22)\tmin\tmax\tparse"
           "\tstring-literal positions (4+: 4 and later)\tcount probed at",
           ]
    for (name, lo, hi, f), (mask, pos) in sorted(groups.items()):
        sp = []
        for b in (G, GS, CL):
            cs = [cfg_names[ci] for ci in range(4) if mask >> (SPELLING_INDEX[b] * 4 + ci) & 1]
            if cs:
                sp.append(SPELLING_NAME[b] + ("*" if len(cs) == 4 else "(" + " ".join(cs) + ")"))
        sp = " ".join(sp)
        parse = "custom" if f & F_CUSTOM else ("ident" if f & F_IDENT else "expr")
        if f & F_LATE:
            parse += ",late"
        if f & F_PACK:
            parse += ",pack"
        strs = ",".join(str(k) + ("+" if k == NSTR - 1 else "") for k in range(NSTR)
                        if f >> (F_STRINGS_SHIFT + k) & 1) or "-"
        tsv.append(f"{name}\t{sp}\t{lo}\t{'-' if hi == UNBOUNDED else hi}\t{parse}\t{strs}\t"
                   f"{','.join(sorted(pos)) or '-'}")
    with open(OUT_TSV, "w") as f:
        f.write("\n".join(tsv) + "\n")
    rs = [f"// @generated by scripts/gen-attr-args.py from {versions}. Do not edit.",
          "//",
          "// The argument rules of every attribute clang knows (spec/parse.md §6.2",
          "// \"Argument table\"): `r(name, mask, min, max, flags)`, `mask` bit",
          "// `spelling * 4 + config` (spellings `__attribute__`, `gnu::`, `clang::`;",
          "// configs aarch64 at clang 19, at 22, x86-64 at 19, at 22), `max`",
          "// `UNBOUNDED` for no upper bound. Sorted by name, then mask.",
          "",
          "use super::{Row, UNBOUNDED, r};",
          "",
          "/// The rows.",
          "pub(super) static ROWS: &[Row] = &["]
    for (name, lo, hi, f), (mask, _) in sorted(groups.items(), key=lambda kv: (kv[0][0], kv[1][0])):
        hs = "UNBOUNDED" if hi == UNBOUNDED else str(hi)
        rs.append(f'    r("{name}", 0x{mask:03x}, {lo}, {hs}, 0x{f:03x}),')
    rs.append("];")
    with open(OUT_RS, "w") as f:
        f.write("\n".join(rs) + "\n")
    print(f"gen-attr-args: {len(groups)} rows ({len(results)} attribute spellings) -> "
          f"{os.path.relpath(OUT_RS, WS)}, {os.path.relpath(OUT_TSV, WS)}")


if __name__ == "__main__":
    main()
