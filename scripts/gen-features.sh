#!/usr/bin/env bash
# Regenerate crates/kiln_pp/src/features_table.rs from the clang 19 and
# clang 22 oracles.
#
# The table answers kiln_pp's FeatureQueries hooks (`__has_feature`,
# `__has_extension`, `__has_builtin`, `__has_constexpr_builtin`,
# `__has_attribute`, `__has_cpp_attribute`, `__has_declspec_attribute`,
# `__is_identifier`) exactly as `clang++-19 -E` answers them, and, under
# `-fclang-compat=22` (spec/pp.md "clang compatibility level"), exactly as
# `clang++-22 -E` does, for
#   targets  aarch64-unknown-linux-gnu, x86_64-unknown-linux-gnu
#            (default CPU: -march/-mcpu/-m<feature> are not modeled),
#   stds     c++98 c++11 c++14 c++17 c++20 c++23 c++26,
#   flags    the driver options in FLAGS below, each recorded as an overlay
#            on the base (no-flag) answers.
#
# Method: (1) candidate names are every identifier-shaped string in either
# libclang-cpp (which holds every builtin, attribute, feature and keyword
# spelling clang knows) plus every operand of a `__has_*` query in the libc++,
# glibc and clang builtin headers; (2) discovery: each candidate is queried
# with every hook by both clangs, under every target x std with no flags and
# every target with each flag set at c++98 and c++26, to find the names that
# are ever non-zero (ever a keyword for `__is_identifier`); (3) those names
# are queried under every clang x target x std x flag set, and the clang 19
# base table plus one overlay per flag set is emitted, followed by the clang
# 22 deltas: the base answers that differ from clang 19's and, per flag, the
# answers the clang 22 base plus the clang 19 overlay get wrong; (4) the
# emitted model is checked to reproduce every probe of both clangs. `-undef`
# keeps predefined macros from expanding the operands of the hooks that
# expand them (`__has_*attribute`), so names are recorded as written. `__x__`
# spellings of features/attributes and `__gnu__`/`_Clang` scopes are not
# stored: features.rs normalizes them as clang does (checked here).
#
# Idempotent (byte-identical output for the same clangs). Runtime: about
# fifteen minutes and 300 MB peak RSS at JOBS=2 (parallel clang processes).
#
# Usage: scripts/gen-features.sh            (clang++-19 and clang++-22 on PATH)
#        CLANGXX=/path/to/clang++-19 CLANGXX22=/path/to/clang++-22 JOBS=4 scripts/gen-features.sh
set -euo pipefail
export LC_ALL=C

WS="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLANGXX="${CLANGXX:-clang++-19}"
CLANGXX22="${CLANGXX22:-clang++-22}"
JOBS="${JOBS:-2}"
OUT="$WS/crates/kiln_pp/src/features_table.rs"

ORACLES=()
HEADER_DIRS=(/usr/include)
for spec in "19 $CLANGXX" "22 $CLANGXX22"; do
  want="${spec%% *}" cxx="${spec#* }"
  major="$(echo __clang_major__ | "$cxx" -E -P -x c++ - | tr -d '[:space:]')"
  if [[ "$major" != "$want" ]]; then
    echo "gen-features: expected clang $want, $cxx reports __clang_major__=$major" >&2
    exit 1
  fi
  version="$(echo '__clang_version__' | "$cxx" -E -P -x c++ - | tr -d '"' | awk '{print $1}')"
  root="$(cd "$(dirname "$(readlink -f "$(command -v "$cxx")")")/.." && pwd)"
  lib="$(ls "$root"/lib/libclang-cpp.so."$want"* 2>/dev/null | head -n1 || true)"
  [[ -n "$lib" && -f "$lib" ]] || { echo "gen-features: no libclang-cpp.so.$want* in $root/lib" >&2; exit 1; }
  ORACLES+=("$cxx" "$version" "$lib")
  [[ -d "$root/include/c++/v1" ]] && HEADER_DIRS+=("$root/include/c++/v1")
  HEADER_DIRS+=("$("$cxx" -print-resource-dir)/include")
done

python3 - "$OUT" "$JOBS" "${ORACLES[@]}" "${HEADER_DIRS[@]}" <<'PY'
import os, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

out_path, jobs = sys.argv[1:3]
# (clang++, version, libclang-cpp) for clang 19 (index 0) and clang 22 (1).
ORACLES = [tuple(sys.argv[3:6]), tuple(sys.argv[6:9])]
header_dirs = sys.argv[9:]
jobs = int(jobs)
version = ORACLES[0][1]

TARGETS = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
STDS = ["c++98", "c++11", "c++14", "c++17", "c++20", "c++23", "c++26"]
NCFG = len(TARGETS) * len(STDS)
ALL = (1 << NCFG) - 1

def cfg_bit(t, s):
    return 1 << (t * len(STDS) + s)

# (Rust variant, driver options, doc). "-std=gnu++" is special-cased.
FLAGS = [
    ("NoExceptions", ["-fno-exceptions"], "`-fno-exceptions` (also `-fno-cxx-exceptions`)"),
    ("NoRtti", ["-fno-rtti"], "`-fno-rtti`"),
    ("NoRttiData", ["-fno-rtti-data"], "`-fno-rtti-data`"),
    ("GnuMode", ["-std=gnu++"], "`-std=gnu++NN` instead of `-std=c++NN`"),
    ("PedanticErrors", ["-pedantic-errors"], "`-pedantic-errors` (extensions are errors)"),
    ("Modules", ["-fmodules"], "`-fmodules`"),
    ("Blocks", ["-fblocks"], "`-fblocks`"),
    ("MsExtensions", ["-fms-extensions"], "`-fms-extensions`"),
    ("MsCompatibility", ["-fms-compatibility"], "`-fms-compatibility`"),
    ("Declspec", ["-fdeclspec"], "`-fdeclspec`"),
    ("NoGnuKeywords", ["-fno-gnu-keywords"], "`-fno-gnu-keywords`"),
    ("NoOperatorNames", ["-fno-operator-names"], "`-fno-operator-names`"),
    ("NoBuiltin", ["-fno-builtin"], "`-fno-builtin`"),
    ("Char8", ["-fchar8_t"], "`-fchar8_t`"),
    ("NoChar8", ["-fno-char8_t"], "`-fno-char8_t`"),
    ("Coroutines", ["-fcoroutines"], "`-fcoroutines`"),
    ("NoCoroutines", ["-fno-coroutines"], "`-fno-coroutines`"),
    ("SizedDeallocation", ["-fsized-deallocation"], "`-fsized-deallocation`"),
    ("NoSizedDeallocation", ["-fno-sized-deallocation"], "`-fno-sized-deallocation`"),
    ("AlignedAllocation", ["-faligned-allocation"], "`-faligned-allocation`"),
    ("NoAlignedAllocation", ["-fno-aligned-allocation"], "`-fno-aligned-allocation`"),
    ("FixedPoint", ["-ffixed-point"], "`-ffixed-point`"),
    ("OpenMp", ["-fopenmp"], "`-fopenmp`"),
    ("DefineTargetOsMacros", ["-fdefine-target-os-macros"], "`-fdefine-target-os-macros`"),
    ("ExperimentalNewConstantInterpreter", ["-fexperimental-new-constant-interpreter"],
     "`-fexperimental-new-constant-interpreter`"),
    ("ExperimentalLibrary", ["-fexperimental-library"], "`-fexperimental-library`"),
    ("PtrauthIntrinsics", ["-fptrauth-intrinsics"], "`-fptrauth-intrinsics` (aarch64)"),
    ("PtrauthCalls", ["-fptrauth-calls"], "`-fptrauth-calls` (aarch64)"),
    ("PtrauthReturns", ["-fptrauth-returns"], "`-fptrauth-returns` (aarch64)"),
    ("PtrauthAuthTraps", ["-fptrauth-auth-traps"], "`-fptrauth-auth-traps` (aarch64)"),
    ("PtrauthVtablePointerAddressDiscrimination",
     ["-fptrauth-vtable-pointer-address-discrimination"],
     "`-fptrauth-vtable-pointer-address-discrimination` (aarch64)"),
    ("PtrauthVtablePointerTypeDiscrimination", ["-fptrauth-vtable-pointer-type-discrimination"],
     "`-fptrauth-vtable-pointer-type-discrimination` (aarch64)"),
    ("PtrauthInitFini", ["-fptrauth-init-fini"], "`-fptrauth-init-fini` (aarch64)"),
    ("SanitizeAddress", ["-fsanitize=address"], "`-fsanitize=address`"),
    ("SanitizeHwaddress", ["-fsanitize=hwaddress"], "`-fsanitize=hwaddress`"),
    ("SanitizeThread", ["-fsanitize=thread"], "`-fsanitize=thread`"),
    ("SanitizeMemory", ["-fsanitize=memory"], "`-fsanitize=memory`"),
    ("SanitizeUndefined", ["-fsanitize=undefined"], "`-fsanitize=undefined`"),
    ("SanitizeDataflow", ["-fsanitize=dataflow"], "`-fsanitize=dataflow`"),
    ("SanitizeSafeStack", ["-fsanitize=safe-stack"], "`-fsanitize=safe-stack`"),
    ("SanitizeLeak", ["-fsanitize=leak"], "`-fsanitize=leak`"),
    ("SanitizeScudo", ["-fsanitize=scudo"], "`-fsanitize=scudo`"),
    ("SanitizeKcfi", ["-fsanitize=kcfi"], "`-fsanitize=kcfi`"),
    ("SanitizeCfi", ["-fsanitize=cfi", "-flto", "-fvisibility=hidden", "-fno-sanitize-ignorelist"],
     "`-fsanitize=cfi` (with the `-flto -fvisibility=hidden` it requires)"),
    ("SanitizeFunction", ["-fsanitize=function"], "`-fsanitize=function`"),
    ("SanitizeNumerical", ["-fsanitize=numerical"], "`-fsanitize=numerical` (x86-64)"),
    ("SanitizeCoverage", ["-fsanitize-coverage=trace-pc-guard"], "`-fsanitize-coverage=...`"),
]

# kind -> query template; "{}" is the name.
KINDS = [
    ("Feature", "__has_feature({})"),
    ("Extension", "__has_extension({})"),
    ("Builtin", "__has_builtin({})"),
    ("ConstexprBuiltin", "__has_constexpr_builtin({})"),
    ("Attribute", "__has_attribute({})"),
    ("DeclspecAttribute", "__has_declspec_attribute({})"),
    ("CppAttribute", "__has_cpp_attribute({})"),
    ("Keyword", "__is_identifier({})"),
]
SCOPES = ["clang", "gnu", "msvc", "gsl", "omp", "riscv"]

def args_for(o, t, s, flag):
    std = STDS[s]
    extra = []
    if flag is not None:
        fa = FLAGS[flag][1]
        if fa == ["-std=gnu++"]:
            std = std.replace("c++", "gnu++")
        else:
            extra = fa
    return [ORACLES[o][0], "--target=" + TARGETS[t], "-std=" + std, "-E", "-P", "-undef",
            "-ferror-limit=0", "-fno-color-diagnostics", *extra, "-x", "c++", "-"]

def supported(o, t, flag):
    r = subprocess.run(args_for(o, t, len(STDS) - 1, flag), input="x\n", capture_output=True, text=True)
    return r.returncode == 0

# ---- candidates ------------------------------------------------------------
cands = set()
for _, _, libclang in ORACLES:
    data = open(libclang, "rb").read()
    cands |= set(m.decode() for m in re.findall(
        rb"(?<![A-Za-z0-9_$])[A-Za-z_][A-Za-z0-9_]{1,79}(?![A-Za-z0-9_$])", data))
    del data
QRE = re.compile(r"__has_(?:builtin|feature|extension|attribute|cpp_attribute|declspec_attribute|"
                 r"constexpr_builtin)\s*\(\s*(?:(\w+)\s*::\s*)?(\w+)")
for d in header_dirs:
    for root, _, files in os.walk(d):
        for f in files:
            try:
                txt = open(os.path.join(root, f), encoding="latin-1").read()
            except OSError:
                continue
            for m in QRE.finditer(txt):
                cands.add(m.group(2))
cands = sorted(cands)

TMPL = dict(KINDS)
# The hooks whose operand clang (and kiln) macro-expands.
EXPANDING = {"Attribute", "DeclspecAttribute", "CppAttribute"}

CHUNK = 100_000  # queries per clang run (bounds clang's memory use)

def probe(o, t, s, flag, qs, nonzero_only=False):
    """{(kind, key): value} for the [(kind, key)] queries (zero answers
    omitted if `nonzero_only`); keys whose line drew a diagnostic map to
    None."""
    res = {}
    for base_i in range(0, len(qs), CHUNK):
        part = qs[base_i:base_i + CHUNK]
        src = "".join("L%d %s\n" % (i, TMPL[k].format(key)) for i, (k, key) in enumerate(part))
        r = subprocess.run(args_for(o, t, s, flag), input=src, capture_output=True, text=True)
        bad = set(int(m) for m in re.findall(r"^<stdin>:(\d+):", r.stderr, re.M))
        got = {}
        for line in r.stdout.split("\n"):
            p = line.split()
            if len(p) == 2 and p[0].startswith("L") and p[0][1:].isdigit():
                i = int(p[0][1:])
                v = p[1].rstrip("L")
                if v.isdigit() and i < len(part):
                    got[i] = int(v)
        for i, (k, key) in enumerate(part):
            if (i + 1) in bad or i not in got:
                res[(k, key)] = None
            elif not nonzero_only or norm(k, got[i]) != 0:
                res[(k, key)] = got[i]
    return res

def norm(kind, v):
    # Keyword: store 1 for "is a keyword" (__is_identifier == 0).
    if v is None:
        return None
    return (1 - v) if kind == "Keyword" else v

# ---- discovery ---------------------------------------------------------------
NORACLE = len(ORACLES)
sup = {(o, t, f): supported(o, t, f)
       for o in range(NORACLE) for t in range(len(TARGETS)) for f in range(len(FLAGS))}
disc_cfgs = [(o, t, s, None) for o in range(NORACLE) for t in range(len(TARGETS))
             for s in range(len(STDS))]
disc_cfgs += [(o, t, s, f) for o in range(NORACLE) for t in range(len(TARGETS))
              for f in range(len(FLAGS)) if sup[(o, t, f)] for s in (0, len(STDS) - 1)]
# Builtin macros (`__has_include`, `__building_module`, `__LINE__`, ...) and
# special forms are expanded/handled by kiln_pp itself before any hook sees
# them, and a function-like one without its `(` would swallow the rest of a
# probe: they are queried only through the non-expanding hooks (clang's
# `__has_builtin(__is_target_arch)` is 1, for example).
def macros(c):
    o, t, s, f = c
    src = "".join("#ifdef %s\nM%d\n#endif\n" % (n, i) for i, n in enumerate(cands))
    r = subprocess.run(args_for(o, t, s, f), input=src, capture_output=True, text=True)
    return {cands[int(m)] for m in re.findall(r"^M(\d+)$", r.stdout, re.M)}
special = {"_Pragma", "__pragma", "__VA_OPT__", "__VA_ARGS__", "defined"}
with ThreadPoolExecutor(jobs) as ex:
    for ms in ex.map(macros, disc_cfgs):
        special |= ms
plain = [n for n in cands if n not in special]
special = sorted(special)

found = {k: set() for k, _ in KINDS}
broken = set()
groups = []
for kind, _ in KINDS:
    names = plain if kind in EXPANDING else plain + special
    groups.append([(kind, n) for n in names])
    if kind == "CppAttribute":
        for sc in SCOPES:
            groups.append([(kind, sc + "::" + n) for n in names])
with ThreadPoolExecutor(jobs) as ex:
    for qs in groups:
        for res in ex.map(lambda c, qs=qs: probe(*c, qs, True), disc_cfgs):
            for (k, key), v in res.items():
                v = norm(k, v)
                if v is None:
                    broken.add(key.split("::")[-1])
                elif v:
                    found[k].add(key)
del groups

def dunder(n):
    return len(n) >= 4 and n.startswith("__") and n.endswith("__")

def normalizes(kind, key):
    """True if clang strips `__x__` from this key's name (Preprocessor
    HasFeature; AttributeCommonInfo normalizeAttrName)."""
    sc, _, n = key.rpartition("::")
    if not dunder(n):
        return False
    if kind in ("Feature", "Extension", "Attribute"):
        return True
    return kind == "CppAttribute" and sc in ("", "gnu", "clang")

def strip_dunder(key):
    sc, _, n = key.rpartition("::")
    n = n[2:-2]
    return (sc + "::" + n) if sc else n

if len(broken) > 100:
    sys.exit("gen-features: %d names drew diagnostics (a probe derailed?): %s"
             % (len(broken), " ".join(sorted(broken)[:50])))
# Names diagnosed in any configuration (`__VA_OPT__`, `_Pragma`, ...) are
# special forms kiln handles itself.
for k in found:
    found[k] = {key for key in found[k] if key.split("::")[-1] not in broken}
# Normalized `__x__` spellings are not stored (features.rs strips them as
# clang does); the full probe checks they answer like `x`.
dunder_check = {}
for k in found:
    for key in list(found[k]):
        if normalizes(k, key):
            dunder_check.setdefault(k, set()).add(key)
            found[k].discard(key)
            found[k].add(strip_dunder(key))

kind_filter = {k: sorted(v) for k, v in found.items()}
for k, v in dunder_check.items():
    kind_filter[k] = sorted(set(kind_filter[k]) | v)

# ---- full probe --------------------------------------------------------------
fq = [(k, key) for k, _ in KINDS for key in kind_filter[k]]
cfgs = [(o, t, s, f) for o in range(NORACLE) for t in range(len(TARGETS)) for s in range(len(STDS))
        for f in [None] + list(range(len(FLAGS))) if f is None or sup[(o, t, f)]]
with ThreadPoolExecutor(jobs) as ex:
    results = dict(zip(cfgs, ex.map(lambda c: probe(*c, fq), cfgs)))

for c, res in results.items():
    for (k, key), v in res.items():
        if v is None:
            sys.exit("gen-features: %s %s drew a diagnostic in %s" % (k, key, c))
        # Normalization check: `__x__` answers like `x`.
        if k in dunder_check and key in dunder_check[k]:
            if res[(k, strip_dunder(key))] != v:
                sys.exit("gen-features: %s(%s) != %s(%s) in %s" % (k, key, k, strip_dunder(key), c))

def answers(o, f):
    """{(kind, key): {cfg_bit: value}} of oracle o under flag f (None: base)."""
    a = {}
    for (oo, t, s, ff), res in results.items():
        if oo != o or ff != f:
            continue
        for (k, key), v in res.items():
            if k in dunder_check and key in dunder_check[k]:
                continue
            a.setdefault((k, key), {})[cfg_bit(t, s)] = norm(k, v)
    return a

def group(per, keep):
    """[(value, mask)] of the {cfg_bit: value} map's bits that `keep`."""
    byv = {}
    for b, v in per.items():
        if keep(b, v):
            byv[v] = byv.get(v, 0) | b
    return sorted(byv.items())

base = answers(0, None)
entries = []  # (kind, key, value, mask)
for (k, key), per in sorted(base.items()):
    for v, m in group(per, lambda b, v: v):
        entries.append((k, key, v, m))

overlays = []  # (flag, kind, key, value, mask)
flag_masks = {}
for f in range(len(FLAGS)):
    fm = 0
    for o in range(NORACLE):
        for t in range(len(TARGETS)):
            if sup[(o, t, f)]:
                for s in range(len(STDS)):
                    fm |= cfg_bit(t, s)
    flag_masks[f] = fm
    fa = answers(0, f)
    for (k, key), per in sorted(fa.items()):
        for v, m in group(per, lambda b, v: base[(k, key)][b] != v):
            overlays.append((f, k, key, v, m))

# clang 22: the base answers that differ from clang 19's, then per flag the
# answers that the clang 22 base with the clang 19 overlay applied gets wrong.
base22 = answers(1, None)
compat22 = []  # (kind, key, value, mask)
for (k, key), per in sorted(base22.items()):
    for v, m in group(per, lambda b, v: base[(k, key)][b] != v):
        compat22.append((k, key, v, m))
overlays19_by = {}
for ov in overlays:
    overlays19_by.setdefault((ov[0], ov[1], ov[2]), []).append(ov)
def before22(f, k, key, b):
    v = base22[(k, key)][b]
    for (_, _, _, ov, om) in overlays19_by.get((f, k, key), ()):
        if om & b:
            v = ov
    return v
compat22_overlays = []  # (flag, kind, key, value, mask)
for f in range(len(FLAGS)):
    fa = answers(1, f)
    for (k, key), per in sorted(fa.items()):
        for v, m in group(per, lambda b, v: before22(f, k, key, b) != v):
            compat22_overlays.append((f, k, key, v, m))

# ---- check the model reproduces every probe ---------------------------------
def by_key(rows, ki):
    d = {}
    for r in rows:
        d.setdefault((r[ki], r[ki + 1]), []).append(r)
    return d
entries_by, overlays_by = by_key(entries, 0), by_key(overlays, 1)
compat22_by, compat22_overlays_by = by_key(compat22, 0), by_key(compat22_overlays, 1)
def model(o, k, key, b, f):
    v = 0
    for (_, _, ev, em) in entries_by.get((k, key), ()):
        if em & b:
            v = ev
    if o == 1:
        for (_, _, ev, em) in compat22_by.get((k, key), ()):
            if em & b:
                v = ev
    if f is not None:
        for (of, _, _, ov, om) in overlays_by.get((k, key), ()):
            if of == f and om & b:
                v = ov
        if o == 1:
            for (of, _, _, ov, om) in compat22_overlays_by.get((k, key), ()):
                if of == f and om & b:
                    v = ov
    return v
for (o, t, s, f), res in results.items():
    for (k, key), v in res.items():
        if k in dunder_check and key in dunder_check[k]:
            continue
        if model(o, k, key, cfg_bit(t, s), f) != norm(k, v):
            sys.exit("gen-features: model mismatch %s %s %s" % (k, key, (o, t, s, f)))

used_flags = sorted({ov[0] for ov in overlays} | {ov[0] for ov in compat22_overlays})

# ---- __FLT_EVAL_METHOD__ -----------------------------------------------------
flt = []
for t in range(len(TARGETS)):
    vals = set()
    for o in range(NORACLE):
        r = subprocess.run(args_for(o, t, len(STDS) - 1, None), input="__FLT_EVAL_METHOD__\n",
                           capture_output=True, text=True, check=True)
        vals.add(int(r.stdout.split()[0]))
    if len(vals) != 1:
        sys.exit("gen-features: __FLT_EVAL_METHOD__ differs between the clangs on %s" % TARGETS[t])
    flt.append(vals.pop())

# ---- emit --------------------------------------------------------------------
def rs(s):
    return '"' + s + '"'

def num(v):
    s = str(v)
    if len(s) <= 4:
        return s
    parts = []
    while s:
        parts.insert(0, s[-3:])
        s = s[:-3]
    return "_".join(parts)

o = []
w = o.append
w("// @generated by scripts/gen-features.sh from clang %s and clang %s. Do not edit."
  % (version, ORACLES[1][1]))
w("//")
w("// Configurations: bit `target * %d + std` of a mask, targets %s," % (len(STDS), ", ".join(TARGETS)))
w("// stds %s. Overlays apply on top of the base entries when their" % " ".join(STDS))
w("// flag is set. Entries are sorted by key (overlays by flag, then key). Under")
w("// `-fclang-compat=22` the COMPAT22 deltas apply after the base entries and the")
w("// COMPAT22_OVERLAYS after each flag's overlays.")
w("")
w("use super::{Delta, Entry, Kind, Overlay};")
w("")
w("/// Driver options that change the answer of at least one feature query")
w("/// (clang %s or %s; flags that change none are not listed)." % (version, ORACLES[1][1]))
w("#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]")
w("#[non_exhaustive]")
w("pub enum FeatureFlag {")
for f in used_flags:
    w("    /// %s." % FLAGS[f][2])
    w("    %s," % FLAGS[f][0])
w("}")
w("")
w("impl FeatureFlag {")
w("    /// Every flag, in overlay order.")
w("    pub const ALL: &'static [FeatureFlag] = &[")
for f in used_flags:
    w("        FeatureFlag::%s," % FLAGS[f][0])
w("    ];")
w("")
w("    /// The driver options the clangs were probed with for this flag.")
w("    pub fn options(self) -> &'static [&'static str] {")
w("        match self {")
for f in used_flags:
    w("            FeatureFlag::%s => &[%s]," % (FLAGS[f][0], ", ".join(rs(a) for a in FLAGS[f][1])))
w("        }")
w("    }")
w("")
w("    /// Configuration mask of the targets clang accepts this flag for.")
w("    #[allow(clippy::match_same_arms)] // one arm per flag, as generated")
w("    pub(super) fn supported(self) -> u16 {")
w("        match self {")
for f in used_flags:
    w("            FeatureFlag::%s => 0x%04x," % (FLAGS[f][0], flag_masks[f]))
w("        }")
w("    }")
w("}")
w("")
w("/// `__FLT_EVAL_METHOD__` per target.")
w("pub(super) const FLT_EVAL_METHOD: [i32; %d] = [%s];" % (len(TARGETS), ", ".join(map(str, flt))))
w("")
kinds = [k for k, _ in KINDS]
for k in kinds:
    es = sorted((e for e in entries if e[0] == k), key=lambda e: (e[1], e[2]))
    name = re.sub(r"(?<!^)([A-Z])", r"_\1", k).upper()
    w("/// `%s` entries: (key, value, configuration mask)." % dict(KINDS)[k].format("x"))
    w("pub(super) static %s: &[Entry] = &[" % name)
    for _, key, v, m in es:
        w("    Entry(%s, %s, 0x%04x)," % (rs(key), num(v), m))
    w("];")
    w("")
w("/// Flag overlays: (flag, kind, key, value, configuration mask).")
w("pub(super) static OVERLAYS: &[Overlay] = &[")
for f, k, key, v, m in sorted(overlays, key=lambda x: (used_flags.index(x[0]), kinds.index(x[1]), x[2], x[3])):
    w("    Overlay(FeatureFlag::%s, Kind::%s, %s, %s, 0x%04x)," % (FLAGS[f][0], k, rs(key), num(v), m))
w("];")
w("")
w("/// clang %s base answers that differ from clang %s's: (kind, key, value," % (ORACLES[1][1], version))
w("/// configuration mask), sorted by kind, then key.")
w("pub(super) static COMPAT22: &[Delta] = &[")
for k, key, v, m in sorted(compat22, key=lambda x: (kinds.index(x[0]), x[1], x[2])):
    w("    Delta(Kind::%s, %s, %s, 0x%04x)," % (k, rs(key), num(v), m))
w("];")
w("")
w("/// clang %s flag overlays, applied after the clang %s ones: (flag, kind, key," % (ORACLES[1][1], version))
w("/// value, configuration mask).")
w("pub(super) static COMPAT22_OVERLAYS: &[Overlay] = &[")
for f, k, key, v, m in sorted(compat22_overlays, key=lambda x: (used_flags.index(x[0]), kinds.index(x[1]), x[2], x[3])):
    w("    Overlay(FeatureFlag::%s, Kind::%s, %s, %s, 0x%04x)," % (FLAGS[f][0], k, rs(key), num(v), m))
w("];")
open(out_path, "w").write("\n".join(o) + "\n")

print("gen-features: %d candidates, %d configs probed; %s; %d overlays over %d flags; "
      "%d clang 22 deltas, %d clang 22 overlays; FLT_EVAL_METHOD %s"
      % (len(cands), len(results),
         ", ".join("%s %d" % (k, sum(1 for e in entries if e[0] == k)) for k in kinds),
         len(overlays), len(used_flags), len(compat22), len(compat22_overlays), flt),
      file=sys.stderr)
unused = [FLAGS[f][0] for f in range(len(FLAGS)) if f not in used_flags]
print("gen-features: flags with no effect: %s" % " ".join(unused), file=sys.stderr)
PY

# The checked-in table is rustfmt-formatted (cargo fmt --check runs in CI).
rustfmt --edition 2024 --config-path "$WS/rustfmt.toml" "$OUT"
