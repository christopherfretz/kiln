#!/usr/bin/env python3
"""Generate crates/kiln_driver/src/x86_targets.rs, the x86-64 `-march=` CPU and
target-feature tables (spec/landed/driver/WP-630.md):

  CPUS     every `-march=` CPU kiln accepts that the clang binary knows: the
           `"target-features"` clang gives a function under `-march=<cpu>`,
           and how its predefined macros differ from the default (`x86-64`)
           ones (`clang -march=<cpu> -dM -E` against `clang -dM -E`)
  IMPLIES  each target feature and the features enabling it enables
           (LLVM's `ImpliedFeatures*`, transitively, itself included), without
           clang's `initFeatureMap` additions (`sse` adds `mmx`, `sse4.2` adds
           `popcnt` and `crc32`, `avx` adds `xsave`, an `avx512*` feature adds
           `evex512`), which kiln applies itself

Both are plain data (spec/landed/driver/WP-660.md): every string is one
entry of `TEXT` (an `(offset, length)` pair into the single `STRS` blob, each
distinct string once), every list one slice of the `u16` `TEXT` indices in
`LISTS` (each distinct list once), and a row holds `(start, length)` pairs
into `LISTS`, so the loader relocates none of it (a `&str` per string would
be a dynamic relocation in the PIE, dirtying its page at every start). The
decoder is emitted with the tables (`DECODER` below).

IMPLIES is read from clang's `"target-features"` for
`__attribute__((target("no-popcnt,no-crc32,no-mmx,no-xsave,<f>")))` on
`i386` (whose base set is only `x87`): the leading `no-` items suppress
clang's additions. A name clang does not take there (`cmov`, `cx8`, a lone
`evex512`) implies only itself. The feature names are those the CPUs, the builtin table
(crates/kiln_ast/src/x86_builtins.rs) and the resource headers' `target`
attributes name, closed under the implications.

Usage: scripts/gen-x86-targets.py [path/to/clang++]  (default: clang 21 at
/usr/lib/llvm-21/bin/clang++)
       scripts/gen-x86-targets.py --reemit
         rewrite crates/kiln_driver/src/x86_targets.rs from its own rows (this
         layout or the pre-WP-660 `Cpu { name: "...", ... }` one) without
         probing clang: for a change of layout, not of content
"""

import concurrent.futures
import importlib.util
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/kiln_driver/src/x86_targets.rs"
FEATURES_RS = ROOT / "crates/kiln_driver/src/features.rs"
BUILTINS_RS = ROOT / "crates/kiln_ast/src/x86_builtins.rs"
SUPPRESS = ["popcnt", "crc32", "mmx", "xsave"]


def clang(cxx, args, src):
    return subprocess.run([cxx, *args, "-x", "c++", "-"], input=src, capture_output=True,
                          text=True)


def target_features(cxx, args, src="void f(){}\n"):
    r = clang(cxx, ["-S", "-emit-llvm", "-o", "-", *args], src)
    if r.returncode:
        return None
    m = re.search(r'"target-features"="([^"]*)"', r.stdout)
    return m.group(1).split(",") if m else []


def macros(cxx, args):
    r = clang(cxx, ["--target=x86_64-unknown-linux-gnu", "-E", "-dM", *args], "")
    out = {}
    for line in r.stdout.splitlines():
        name, _, value = line.removeprefix("#define ").partition(" ")
        out[name] = value
    return out


def closure(cxx, f):
    attr = ",".join([f"no-{s}" for s in SUPPRESS if s != f] + [f])
    src = f'__attribute__((target("{attr}"))) void f(){{}}\n'
    got = target_features(cxx, ["--target=i386-unknown-linux-gnu", "-march=i386"], src)
    if got is None:
        return None
    on = {g[1:] for g in got if g.startswith("+")} - {"x87"}
    if f.startswith("avx512"):
        on.discard("evex512")
    if f not in on and f != "x87":
        return None
    return sorted(on | {f})


# The decoder, emitted ahead of the tables (rustfmt-clean as it stands).
DECODER = r"""//! The x86-64 `-march=` CPU and target-feature tables
//! (spec/landed/driver/WP-630.md "Tables"), as plain data
//! (spec/landed/driver/WP-660.md): every string is a slice of one blob and
//! every list a slice of one index array, so the loader relocates none of it.
//! The readers decode a row by value.

/// A list of strings of the tables (a slice of `LISTS`).
#[derive(Clone, Copy, Debug)]
pub struct List(&'static [u16]);

impl List {
    /// The strings, in order.
    pub fn iter(self) -> impl ExactSizeIterator<Item = &'static str> + Clone {
        self.0.iter().map(|&i| text(i))
    }

    /// The number of strings.
    pub fn len(self) -> usize {
        self.0.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }

    /// Whether `s` is one of the strings.
    pub fn contains(self, s: &str) -> bool {
        self.iter().any(|t| t == s)
    }
}

/// A list of (name, value) pairs of the tables.
#[derive(Clone, Copy, Debug)]
pub struct Pairs(&'static [u16]);

impl Pairs {
    /// The pairs, in order.
    pub fn iter(self) -> impl ExactSizeIterator<Item = (&'static str, &'static str)> + Clone {
        self.0.chunks_exact(2).map(|p| (text(p[0]), text(p[1])))
    }

    /// The number of pairs.
    pub fn len(self) -> usize {
        self.0.len() / 2
    }

    /// Whether there is no pair.
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
}

/// An x86-64 `-march=` CPU.
#[derive(Clone, Copy, Debug)]
pub struct Cpu {
    /// The `-march=` name.
    pub name: &'static str,
    /// The enabled target features, sorted.
    pub features: List,
    /// The predefined macros it defines or redefines (name, value), against
    /// the default `x86-64`, sorted.
    pub define: Pairs,
    /// The default predefined macros it does not define, sorted.
    pub undef: List,
}

/// A row of [`CPUS`]: the name's index in [`TEXT`] and the `(start,
/// length)` of each list in [`LISTS`] (`define` holds a name and a value per
/// macro).
#[derive(Clone, Copy, Debug)]
struct Row {
    name: u16,
    features: (u16, u16),
    define: (u16, u16),
    undef: (u16, u16),
}

const fn c(name: u16, features: (u16, u16), define: (u16, u16), undef: (u16, u16)) -> Row {
    Row { name, features, define, undef }
}

/// A row of [`IMPLIES`]: the feature's index in [`TEXT`] and the `(start,
/// length)` of the features it enables in [`LISTS`].
#[derive(Clone, Copy, Debug)]
struct Implies {
    name: u16,
    list: (u16, u16),
}

const fn i(name: u16, list: (u16, u16)) -> Implies {
    Implies { name, list }
}

/// String `i` of [`TEXT`] (ASCII, so any range is on character boundaries).
fn text(i: u16) -> &'static str {
    let (at, len) = TEXT[usize::from(i)];
    let at = usize::from(at);
    &STRS[at..at + usize::from(len)]
}

fn slice((start, len): (u16, u16)) -> &'static [u16] {
    let start = usize::from(start);
    &LISTS[start..start + usize::from(len)]
}

fn cpu_of(r: &Row) -> Cpu {
    Cpu {
        name: text(r.name),
        features: List(slice(r.features)),
        define: Pairs(slice(r.define)),
        undef: List(slice(r.undef)),
    }
}

/// Every `-march=` CPU kiln accepts that clang 21 knows, in table order.
pub fn cpus() -> impl ExactSizeIterator<Item = Cpu> + Clone {
    CPUS.iter().map(cpu_of)
}

/// The CPU `-march=<name>` names, if clang 21 knows it.
pub fn cpu(name: &str) -> Option<Cpu> {
    CPUS.iter().find(|r| text(r.name) == name).map(cpu_of)
}

/// Each target feature and the features enabling it enables (itself
/// included), sorted by name.
pub fn implications() -> impl ExactSizeIterator<Item = (&'static str, List)> + Clone {
    IMPLIES.iter().map(|r| (text(r.name), List(slice(r.list))))
}

/// The features enabling target feature `f` enables (itself included), if
/// the table has `f` (a binary search over the sorted names).
pub fn implies(f: &str) -> Option<List> {
    let k = IMPLIES.binary_search_by(|r| text(r.name).cmp(f)).ok()?;
    Some(List(slice(IMPLIES[k].list)))
}
"""


def emit(rows, implies):
    """The text of crates/kiln_driver/src/x86_targets.rs: `rows` are the CPUs
    in table order, (name, sorted features, sorted (macro, value) pairs,
    sorted undefined macros); `implies` maps each feature to the sorted
    features it enables."""
    blob, text, at = [], [], {}
    size = 0

    def idx(t):
        nonlocal size
        if not t.isascii() or '"' in t or "\\" in t or len(t) > 255:
            sys.exit(f"cannot place {t!r} in STRS")
        if t not in at:
            at[t] = len(text)
            text.append((size, len(t)))
            if t:
                blob.append(t)
            size += len(t)
        return at[t]

    flat, spans = [], {}

    def span(items):
        key = tuple(idx(t) for t in items)
        if key not in spans:
            spans[key] = (len(flat), len(key))
            flat.extend(key)
        return spans[key]

    cpu_rows = []
    for cpu, on, define, undef in rows:
        cpu_rows.append((idx(cpu), span(on), span([x for kv in define for x in kv]), span(undef), cpu))
    imp_rows = [(idx(f), span(implies[f]), f) for f in sorted(implies)]
    if size > 0xFFFF or len(text) > 0xFFFF or len(flat) > 0xFFFF:
        sys.exit("STRS, TEXT or LISTS outgrew the u16 row encoding")
    lines = [
        "// @generated by scripts/gen-x86-targets.py from clang's target features and",
        "// predefined macros (spec/landed/driver/WP-630.md). Do not edit.",
        "",
    ]
    lines += DECODER.rstrip("\n").split("\n")
    lines += [
        "",
        "/// Every string of the tables, concatenated, each once.",
        "#[rustfmt::skip]",
        "static STRS: &str = concat!(",
    ]
    lines += [f'    "{t}",' for t in blob]
    lines += [
        ");",
        "",
        "/// Each distinct string's `(offset, length)` in [`STRS`].",
        "#[rustfmt::skip]",
        f"static TEXT: [(u16, u8); {len(text)}] = [",
    ]
    lines += [f"    ({o}, {n}), // {t}" for t, (o, n) in zip(at, text)]
    lines += [
        "];",
        "",
        "/// The lists of the rows, as indices into [`TEXT`], each distinct list once.",
        "#[rustfmt::skip]",
        f"static LISTS: [u16; {len(flat)}] = [",
    ]
    lines += ["    " + ", ".join(str(x) for x in flat[k:k + 20]) + "," for k in range(0, len(flat), 20)]
    lines += [
        "];",
        "",
        "/// Every `-march=` CPU kiln accepts that clang 21 knows: (name) in [`TEXT`],",
        "/// (features, macros and their values, undefined macros) in [`LISTS`].",
        "#[rustfmt::skip]",
        f"static CPUS: [Row; {len(cpu_rows)}] = [",
    ]
    for n, (fs, fl), (ds, dl), (us, ul), cpu in cpu_rows:
        lines.append(f"    c({n}, ({fs}, {fl}), ({ds}, {dl}), ({us}, {ul})), // {cpu}")
    lines += [
        "];",
        "",
        "/// Each target feature and the features enabling it enables (itself",
        "/// included), sorted by name: (feature) in [`TEXT`], (features) in",
        "/// [`LISTS`].",
        "#[rustfmt::skip]",
        f"static IMPLIES: [Implies; {len(imp_rows)}] = [",
    ]
    for n, (ls, ll), f in imp_rows:
        lines.append(f"    i({n}, ({ls}, {ll})), // {f}")
    lines.append("];")
    return "\n".join(lines) + "\n"


def parse(text):
    """The CPU rows and implications of a generated x86_targets.rs, in either
    layout, as `emit` takes them."""
    q = lambda t: re.findall(r'"([^"]*)"', t)
    pairs = lambda t: re.findall(r'\("([^"]*)", "([^"]*)"\)', t)
    rows, implies = [], {}
    old = re.findall(r'^    Cpu \{ name: "([^"]*)", features: &\[(.*?)\], define: &\[(.*?)\], '
                     r'undef: &\[(.*?)\] \},$', text, re.M)
    if old:
        for cpu, f, d, u in old:
            if 2 * len(pairs(d)) != len(q(d)):
                sys.exit(f"{cpu}: cannot read its macros")
            rows.append((cpu, q(f), pairs(d), q(u)))
        for f, imp in re.findall(r'^    \("([^"]*)", &\[(.*?)\]\),$', text, re.M):
            implies[f] = q(imp)
    else:
        def block(start):
            b = text[text.index(start):]
            return b[:b.index("\n];") if "[" in start else b.index("\n);")]
        strs = "".join(re.findall(r'^    "([^"]*)",$', block("static STRS: &str = concat!("), re.M))
        tx = [strs[int(o):int(o) + int(n)] for o, n, _ in
              re.findall(r"^    \((\d+), (\d+)\), // (.*)$", block("static TEXT: ["), re.M)]
        flat = [int(x) for x in re.findall(r"\d+", block("static LISTS: [").split("\n", 1)[1])]
        lst = lambda s, n: [tx[k] for k in flat[int(s):int(s) + int(n)]]
        for m in re.finditer(r"^    c\((\d+), \((\d+), (\d+)\), \((\d+), (\d+)\), \((\d+), (\d+)\)\), "
                             r"// (\S+)$", text, re.M):
            n, fs, fl, ds, dl, us, ul, comment = m.groups()
            if tx[int(n)] != comment:
                sys.exit(f"row comment {comment} names {tx[int(n)]}")
            d = lst(ds, dl)
            rows.append((tx[int(n)], lst(fs, fl), list(zip(d[::2], d[1::2])), lst(us, ul)))
        for m in re.finditer(r"^    i\((\d+), \((\d+), (\d+)\)\), // (\S+)$", text, re.M):
            n, ls, ll, comment = m.groups()
            if tx[int(n)] != comment:
                sys.exit(f"row comment {comment} names {tx[int(n)]}")
            implies[comment] = lst(ls, ll)
    if not rows or not implies:
        sys.exit("no rows found")
    return rows, implies


def main():
    if sys.argv[1:] == ["--reemit"]:
        rows, implies = parse(OUT.read_text())
        OUT.write_text(emit(rows, implies))
        print(f"{OUT.relative_to(ROOT)}: {len(rows)} CPUs, {len(implies)} features re-emitted")
        return
    cxx = sys.argv[1] if len(sys.argv) > 1 else "/usr/lib/llvm-21/bin/clang++"
    text = FEATURES_RS.read_text()
    cpus = []
    for table in ("X86_CPUS", "X86_CPUS_22"):
        m = re.search(rf"const {table}: &\[&str\] = &\[(.*?)\];", text, re.S)
        cpus += re.findall(r'"([^"]+)"', m.group(1))
    base = macros(cxx, [])
    rows, names = [], set()
    for cpu in cpus:
        tf = target_features(cxx, ["--target=x86_64-unknown-linux-gnu", f"-march={cpu}"])
        if tf is None:
            print(f"clang does not know -march={cpu}; left out")
            continue
        on = sorted(f[1:] for f in tf if f.startswith("+"))
        names |= set(on)
        mm = macros(cxx, [f"-march={cpu}"])
        define = sorted((k, v) for k, v in mm.items() if base.get(k) != v)
        undef = sorted(k for k in base if k not in mm)
        rows.append((cpu, on, define, undef))
    spec = importlib.util.spec_from_file_location("gen_x86_builtins",
                                                  ROOT / "scripts/gen-x86-builtins.py")
    builtins = importlib.util.module_from_spec(spec)
    sys.dont_write_bytecode = True
    spec.loader.exec_module(builtins)
    names |= set(re.findall(r"[\w.-]+", " ".join(
        r[3] for r in builtins.parse_ast(BUILTINS_RS.read_text()))))
    for h in (ROOT / "resource/include").glob("*.h"):
        for t in re.findall(r'__target__\("([^"]*)"\)', h.read_text(errors="replace")):
            names |= {x.removeprefix("no-") for x in t.split(",")}
    implies, todo = {}, set(names)
    with concurrent.futures.ThreadPoolExecutor(os.cpu_count()) as ex:
        while todo:
            batch = sorted(todo)
            todo = set()
            for f, c in zip(batch, ex.map(lambda f: closure(cxx, f), batch)):
                if c is None:
                    # `cmov` and `cx8` are not `target` attribute names, and
                    # clang drops a lone `evex512`: nothing else to enable.
                    print(f"clang takes no target(\"{f}\"); implies only itself")
                    c = [f]
                implies[f] = c
                todo |= set(c) - set(implies) - set(batch)
    OUT.write_text(emit(rows, implies))
    print(f"{OUT.relative_to(ROOT)}: {len(rows)} CPUs, {len(implies)} features")


if __name__ == "__main__":
    main()
