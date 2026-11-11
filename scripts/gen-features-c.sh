#!/usr/bin/env bash
# Regenerate crates/kiln_pp/src/features_c_table.rs: the C-mode answers of
# the feature-test hooks (WP-1275, spec/pp.md "Feature queries in C").
#
# kiln answers a hook in C as it does in C++ (features_table.rs, from
# scripts/gen-features.sh) for the C++ standard the C standard lexes as
# (spec/driver.md "Language mode": c89/c99 -> c++98, c11/c17 -> c++11,
# c23 -> c++17; gnuNN -> gnu++NN), then applies the deltas of this table:
# every answer of `clang-19 -x c` (or, under `-fclang-compat=22`,
# `clang-22 -x c`) that differs from `clang++-19` (`clang++-22`) at that
# C++ standard. `__has_c_attribute` has no C++ counterpart, so all of its
# nonzero answers are deltas.
#
# Configurations: bit `target * 10 + std * 2 + gnu` of a mask, targets
# aarch64-unknown-linux-gnu, x86_64-unknown-linux-gnu, stds c89 c99 c11 c17
# c23, gnu 0 (cNN) or 1 (gnuNN). Under a flag kiln applies the C++ overlays,
# then these deltas, then the flag's C deltas (spec/pp.md): each flag of
# FLAGS below is probed in all 20 configurations, and every answer of
# `clang-NN -x c <flag>` that differs from that prediction (`clang++-NN
# <flag>` with the base C deltas applied) is a row of C19_FLAGS/C22_FLAGS.
# The flags are those that change a C answer: -fblocks, -fno-exceptions,
# -fmodules, -fms-extensions and -fsanitize=address|thread|memory|undefined
# (-fno-blocks is the default; it only undoes -fblocks).
#
# Candidates: every key of features_table.rs (the names either clang
# answers nonzero for in some C++ configuration), the operands of every
# `__has_*` query in the glibc and clang headers, the identifier-like
# strings of each clang's libclang-cpp (the names it answers 0 for in C++),
# the C-only keywords and the standard C attributes, and every
# `__has_cpp_attribute` key as a `__has_c_attribute` candidate. Idempotent;
# runtime about three minutes.
#
# Usage: scripts/gen-features-c.sh   (clang-19, clang++-19, clang-22,
#        clang++-22 on PATH, or CLANG19/CLANGXX19/CLANG22/CLANGXX22)
set -euo pipefail
export LC_ALL=C

WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLANG19="${CLANG19:-clang-19}"
CLANGXX19="${CLANGXX19:-clang++-19}"
CLANG22="${CLANG22:-clang-22}"
CLANGXX22="${CLANGXX22:-clang++-22}"
OUT="$WS/crates/kiln_pp/src/features_c_table.rs"
CXX_TABLE="$WS/crates/kiln_pp/src/features_table.rs"

HEADER_DIRS=(/usr/include)
for c in "$CLANG19" "$CLANG22"; do
  res="$("$c" -print-resource-dir)"
  HEADER_DIRS+=("$res/include")
  for lib in "$res"/../../libclang-cpp.so.*; do
    [ -f "$lib" ] && HEADER_DIRS+=("lib:$lib")
  done
done

python3 - "$OUT" "$CXX_TABLE" "$CLANG19" "$CLANGXX19" "$CLANG22" "$CLANGXX22" "${HEADER_DIRS[@]}" <<'PY'
import os, re, subprocess, sys, tempfile

out_path, cxx_table = sys.argv[1:3]
ORACLES = [(sys.argv[3], sys.argv[4]), (sys.argv[5], sys.argv[6])]
header_dirs = [d for d in sys.argv[7:] if not d.startswith("lib:")]
clang_libs = [d[4:] for d in sys.argv[7:] if d.startswith("lib:")]

def version(cc):
    return subprocess.run([cc, "-E", "-P", "-x", "c", "-"], input="__clang_version__\n",
                          capture_output=True, text=True).stdout.strip().strip('"').split()[0]

VERSIONS = [version(o[0]) for o in ORACLES]
TARGETS = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
CSTDS = ["c89", "c99", "c11", "c17", "c23"]
CXXSTDS = ["c++98", "c++98", "c++11", "c++11", "c++17"]
# (FeatureFlag variant, clang option), in FeatureFlag declaration order.
FLAGS = [("NoExceptions", "-fno-exceptions"), ("Modules", "-fmodules"), ("Blocks", "-fblocks"),
         ("MsExtensions", "-fms-extensions"), ("SanitizeAddress", "-fsanitize=address"),
         ("SanitizeThread", "-fsanitize=thread"), ("SanitizeMemory", "-fsanitize=memory"),
         ("SanitizeUndefined", "-fsanitize=undefined")]
# -fmodules may touch a module cache: give it a private one.
MODCACHE = tempfile.TemporaryDirectory(prefix="gen-features-c-")

# Kind order is the Rust `Kind` declaration order (the table sort order).
KINDS = [
    ("Feature", "__has_feature({})"),
    ("Extension", "__has_extension({})"),
    ("Builtin", "__has_builtin({})"),
    ("ConstexprBuiltin", "__has_constexpr_builtin({})"),
    ("Attribute", "__has_attribute({})"),
    ("Keyword", "__is_identifier({})"),
    ("CAttribute", "__has_c_attribute({})"),
]
ORDER = {k: i for i, (k, _) in enumerate(
    [("Feature", 0), ("Extension", 0), ("Builtin", 0), ("ConstexprBuiltin", 0),
     ("Attribute", 0), ("DeclspecAttribute", 0), ("CppAttribute", 0), ("Keyword", 0),
     ("CAttribute", 0)])}
TMPL = dict(KINDS)
SECTION = {"FEATURE": "Feature", "EXTENSION": "Extension", "BUILTIN": "Builtin",
           "CONSTEXPR_BUILTIN": "ConstexprBuiltin", "ATTRIBUTE": "Attribute",
           "CPP_ATTRIBUTE": "CppAttribute", "KEYWORD": "Keyword"}

# ---- candidates ------------------------------------------------------------
cands = {k: set() for k, _ in KINDS}
cur = None
for line in open(cxx_table):
    m = re.match(r"pub\(super\) static (\w+):", line)
    if m:
        cur = SECTION.get(m.group(1))
        continue
    m = re.search(r'(?:Entry\(|Kind::(\w+), )"([^"]+)"', line)
    if not m:
        continue
    kind = m.group(1) or cur
    if kind == "CppAttribute":
        cands["CAttribute"].add(m.group(2))
    elif kind in cands:
        cands[kind].add(m.group(2))
QRE = re.compile(r"__has_(builtin|feature|extension|attribute|c_attribute|constexpr_builtin)"
                 r"\s*\(\s*(?:(\w+)\s*::\s*)?(\w+)")
HK = {"builtin": "Builtin", "feature": "Feature", "extension": "Extension",
      "attribute": "Attribute", "c_attribute": "CAttribute",
      "constexpr_builtin": "ConstexprBuiltin"}
for d in header_dirs:
    for root, _, files in os.walk(d):
        for f in files:
            try:
                txt = open(os.path.join(root, f), encoding="latin-1").read()
            except OSError:
                continue
            for m in QRE.finditer(txt):
                k = HK[m.group(1)]
                if k == "CAttribute" and m.group(2):
                    cands[k].add(m.group(2) + "::" + m.group(3))
                else:
                    cands[k].add(m.group(3))
# The names clang knows but answers 0 for in C++ are in neither: take them
# from the strings of each oracle's libclang-cpp (C-only features, builtins,
# keywords and attributes).
for lib in clang_libs:
    data = open(lib, "rb").read()
    for piece in set(re.split(rb"[^\x21-\x7e]+", data)):
        if not re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]{2,60}", piece):
            continue
        w = piece.decode()
        if w.startswith("c_"):
            cands["Feature"].add(w)
            cands["Extension"].add(w)
        if re.fullmatch(r"__\w+|_[A-Y]\w*", w):
            cands["Builtin"].add(w)
            cands["Keyword"].add(w)
            if w.startswith("__builtin"):
                cands["ConstexprBuiltin"].add(w)
        if re.fullmatch(r"[a-z][a-z0-9_]{2,40}", w):
            cands["Attribute"].add(w)
cands["Keyword"] |= {"_Bool", "restrict", "_Imaginary", "_Decimal32", "_Decimal64",
                     "_Decimal128", "typeof", "typeof_unqual", "_Static_assert", "_Alignas",
                     "_Alignof", "_Generic", "_Noreturn", "_Thread_local", "_Atomic",
                     "_Complex", "__auto_type", "_BitInt", "inline", "asm"}
cands["CAttribute"] |= {"deprecated", "fallthrough", "maybe_unused", "nodiscard", "noreturn",
                        "_Noreturn", "unsequenced", "reproducible"}
# features.rs normalizes `__x__` spellings and the `__gnu__`/`_Clang` scopes.
def normal(kind, key):
    if kind in ("Feature", "Extension", "Attribute", "CAttribute"):
        s, _, n = key.rpartition("::")
        if s in ("__gnu__", "_Clang"):
            return False
        if (not s or s in ("gnu", "clang")) and len(n) >= 4 and n.startswith("__") \
                and n.endswith("__"):
            return False
    return True
qs = sorted((k, key) for k in cands for key in cands[k] if normal(k, key))

def probe(cc, lang, t, std, flag=None):
    src = "".join("L%d %s\n" % (i, TMPL[k].format(key)) for i, (k, key) in enumerate(qs))
    extra = [flag, "-fmodules-cache-path=" + MODCACHE.name] if flag == "-fmodules" else \
        ([flag] if flag else [])
    args = [cc, "--target=" + t, "-std=" + std, "-E", "-P", "-undef", "-ferror-limit=0",
            "-fno-color-diagnostics"] + extra + ["-x", lang, "-"]
    r = subprocess.run(args, input=src, capture_output=True, text=True)
    bad = set(int(m) for m in re.findall(r"^<stdin>:(\d+):", r.stderr, re.M))
    got = {}
    for line in r.stdout.split("\n"):
        p = line.split()
        if len(p) == 2 and p[0].startswith("L") and p[0][1:].isdigit():
            v = p[1].rstrip("L")
            if v.isdigit():
                got[int(p[0][1:])] = int(v)
    if not got:
        sys.exit("gen-features-c: probe failed: %s\n%s" % (" ".join(args), r.stderr[:2000]))
    res = {}
    for i, q in enumerate(qs):
        v = None if (i + 1) in bad else got.get(i)
        if v is not None and q[0] == "Keyword":
            v = 1 - v
        res[q] = v
    return res

def configs():
    for t, triple in enumerate(TARGETS):
        for s in range(len(CSTDS)):
            for gnu in (0, 1):
                cstd = ("gnu" + CSTDS[s][1:]) if gnu else CSTDS[s]
                xstd = CXXSTDS[s].replace("c++", "gnu++") if gnu else CXXSTDS[s]
                yield triple, cstd, xstd, 1 << (t * 10 + s * 2 + gnu)

def add(deltas, q, v, bit):
    deltas.setdefault(q, {}).setdefault(v, 0)
    deltas[q][v] |= bit

tables, flag_tables = [], []
for o, (cc, cxx) in enumerate(ORACLES):
    deltas = {}
    base = {}  # bit -> {query: C answer the base deltas give}
    for triple, cstd, xstd, bit in configs():
        c = probe(cc, "c", triple, cstd)
        x = probe(cxx, "c++", triple, xstd)
        base[bit] = {}
        for q in qs:
            cv = c[q]
            if cv is None:
                continue
            xv = 0 if q[0] == "CAttribute" else x[q]
            if xv is None:
                xv = 0
            if cv != xv:
                add(deltas, q, cv, bit)
                base[bit][q] = cv
    rows = []
    for (k, key), byv in deltas.items():
        for v, mask in byv.items():
            rows.append((ORDER[k], key, v, mask, k))
    rows.sort()
    tables.append(rows)
    # Under each flag: kiln predicts the C++ answer under the flag with the
    # base C deltas applied; every C answer that differs is a flag delta.
    frows = []
    for fi, (fname, opt) in enumerate(FLAGS):
        fd = {}
        for triple, cstd, xstd, bit in configs():
            c = probe(cc, "c", triple, cstd, opt)
            x = probe(cxx, "c++", triple, xstd, opt)
            for q in qs:
                cv = c[q]
                if cv is None:
                    continue
                xv = 0 if q[0] == "CAttribute" else (x[q] or 0)
                if cv != base[bit].get(q, xv):
                    add(fd, q, cv, bit)
        for (k, key), byv in fd.items():
            for v, mask in byv.items():
                frows.append((fi, ORDER[k], key, v, mask, k, fname))
    frows.sort()
    flag_tables.append(frows)

def rs(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'

def num(v):
    return "{:_}".format(v) if v >= 100_000 else str(v)

out = []
out.append("// @generated by scripts/gen-features-c.sh from clang %s and clang %s. Do not edit.\n"
           % (VERSIONS[0], VERSIONS[1]))
out.append("//\n// C-mode deltas on the C++ answers (spec/pp.md \"Feature queries in C\"):\n"
           "// configuration bit `target * 10 + std * 2 + gnu`, targets aarch64, x86_64; stds c89\n"
           "// c99 c11 c17 c23. C19/C22 are sorted by (kind, key), the\n"
           "// _FLAGS tables by (flag, kind, key); masks of one key are disjoint.\n\n")
out.append("use super::{CDelta, CFlagDelta, FeatureFlag, Kind};\n\n")
for name, o in (("C19", 0), ("C22", 1)):
    out.append("/// `clang-%s -x c` answers that differ from `clang++-%s`'s.\n"
               % (VERSIONS[o].split(".")[0], VERSIONS[o].split(".")[0]))
    out.append("pub(super) static %s: &[CDelta] = &[\n" % name)
    for _, key, v, mask, k in tables[o]:
        out.append("    CDelta(Kind::%s, %s, %s, 0x%05x),\n" % (k, rs(key), num(v), mask))
    out.append("];\n\n")
for name, o in (("C19_FLAGS", 0), ("C22_FLAGS", 1)):
    out.append("/// `clang-%s -x c <flag>` answers that differ from kiln's prediction under\n"
               "/// the flag (the C++ overlays, then `%s`).\n"
               % (VERSIONS[o].split(".")[0], name[:3]))
    out.append("pub(super) static %s: &[CFlagDelta] = &[\n" % name)
    for _, _, key, v, mask, k, fname in flag_tables[o]:
        out.append("    CFlagDelta(FeatureFlag::%s, Kind::%s, %s, %s, 0x%05x),\n"
                   % (fname, k, rs(key), num(v), mask))
    out.append("];\n\n")
text = "".join(out).rstrip("\n") + "\n"
open(out_path, "w").write(text)
print("gen-features-c: %d + %d deltas, %d + %d flag deltas, %d candidates"
      % (len(tables[0]), len(tables[1]), len(flag_tables[0]), len(flag_tables[1]), len(qs)))
PY
