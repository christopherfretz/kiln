#!/usr/bin/env python3
"""Generate kiln_lex's Unicode tables from the Unicode Character Database.

Outputs (both @generated, never edited by hand):

* crates/kiln_lex/src/unicode_tables.rs -- UAX #31 identifier properties
  (XID_Start, XID_Continue) and the normalization data needed to decide
  whether an identifier is in NFC ([lex.name]/1): NFC_Quick_Check No/Maybe,
  canonical combining classes, canonical decompositions and the primary
  composites (canonical pairs that are not Full_Composition_Exclusion).
* crates/kiln_lex/src/unicode_names.rs -- the character-name table for
  `\\N{name}` ([lex.universal.char]/3): every explicit character name in
  UnicodeData.txt plus every alias in NameAliases.txt, sorted by UAX #44-LM2
  loose-matching key and front-coded in blocks; plus the algorithmic NR2
  hex-suffixed name ranges (CJK UNIFIED IDEOGRAPH-XXXX and friends).
  Hangul syllable names (NR1) are computed in code, not stored.

Usage:  scripts/gen-unicode-tables.py [UCD_VERSION]   (default 15.1.0)

The files are downloaded from unicode.org into a temporary directory, or read
from $UCD_DIR if set. Their SHA-256 checksums are pinned below per version;
a mismatch (or an unpinned version) aborts the run so a table change is
always a deliberate, reviewed edit of this script. The version default
matches clang 19, kiln's conformance oracle.
"""
import hashlib
import os
import re
import sys
import tempfile
import urllib.request

VERSION = sys.argv[1] if len(sys.argv) > 1 else "15.1.0"
BASE = f"https://www.unicode.org/Public/{VERSION}/ucd/"
SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                   "crates", "kiln_lex", "src")

CHECKSUMS = {
    "15.1.0": {
        "DerivedCoreProperties.txt":
            "f55d0db69123431a7317868725b1fcbf1eab6b265d756d1bd7f0f6d9f9ee108b",
        "DerivedNormalizationProps.txt":
            "8875dccee2bc1a7c1fe568a3b502a9e78c9e0495afd96b6568b4294d0ed1f7e1",
        "NameAliases.txt":
            "fbf0e640bab36e165c4da5b6a98bdd963fcb4f923b5097f26f6f7f18b9678698",
        "UnicodeData.txt":
            "2fc713e6a31a87c4850a37fe2caffa4218180fadb5de86b43a143ddb4581fb86",
    },
}
FILES = sorted(CHECKSUMS["15.1.0"])

# NR2 (Unicode ch. 4.8): names that are a fixed prefix plus the code point in
# uppercase hex. Which `<..., First>` ranges in UnicodeData.txt carry them.
NR2_PREFIXES = [
    "CJK UNIFIED IDEOGRAPH-",
    "CJK COMPATIBILITY IDEOGRAPH-",
    "TANGUT IDEOGRAPH-",
    "KHITAN SMALL SCRIPT CHARACTER-",
    "NUSHU CHARACTER-",
]
RANGE_PREFIX = {
    "CJK Ideograph": "CJK UNIFIED IDEOGRAPH-",
    "Tangut Ideograph": "TANGUT IDEOGRAPH-",
}

# Single-byte tags for the alias types in NameAliases.txt.
ALIAS_TYPE = {"correction": "c", "control": "k", "alternate": "a",
              "figment": "f", "abbreviation": "b"}
BLOCK = 32


def fetch_all(tmp):
    paths = {}
    local = os.environ.get("UCD_DIR")
    for name in FILES:
        if local:
            path = os.path.join(local, name)
        else:
            path = os.path.join(tmp, name)
            with urllib.request.urlopen(BASE + name) as r, open(path, "wb") as f:
                f.write(r.read())
        with open(path, "rb") as f:
            digest = hashlib.sha256(f.read()).hexdigest()
        want = CHECKSUMS.get(VERSION, {}).get(name)
        if want != digest:
            sys.exit(f"{name}: sha256 {digest} does not match the pinned "
                     f"checksum {want!r} for UCD {VERSION}; review the new "
                     f"data and update CHECKSUMS")
        paths[name] = path
    return paths


def records(path):
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.split("#", 1)[0].strip()
            if line:
                yield [x.strip() for x in line.split(";")]


def parse_props(path, want):
    """Return {key: [(lo, hi)]} for lines whose property (and value) match."""
    out = {k: [] for k in want}
    for fields in records(path):
        key = tuple(fields[1:])
        if key not in out:
            continue
        cps = fields[0].split("..")
        out[key].append((int(cps[0], 16), int(cps[-1], 16)))
    return out


def merge(ranges):
    merged = []
    for lo, hi in sorted(ranges):
        if merged and lo <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], max(merged[-1][1], hi))
        else:
            merged.append((lo, hi))
    return merged


def loose_key(name):
    """UAX #44-LM2: ignore case, whitespace, underscore and medial hyphens,
    except the hyphen of U+1180 HANGUL JUNGSEONG O-E."""
    if name.upper() == "HANGUL JUNGSEONG O-E":
        return "HANGULJUNGSEONGO-E"
    out = []
    for i, c in enumerate(name):
        if c in " _":
            continue
        if (c == "-" and 0 < i < len(name) - 1 and name[i - 1].isalnum()
                and name[i + 1].isalnum()):
            continue
        out.append(c.upper())
    return "".join(out)


def parse_unicode_data(path):
    names = {}      # cp -> explicit (non-algorithmic) name
    nr2 = []        # (lo, hi, prefix)
    ccc = {}
    decomp = {}     # cp -> tuple of canonical decomposition
    first = None
    for fields in records(path):
        cp = int(fields[0], 16)
        name = fields[1]
        if int(fields[3]):
            ccc[cp] = int(fields[3])
        d = fields[5]
        if d and not d.startswith("<"):
            decomp[cp] = tuple(int(x, 16) for x in d.split())
        if name.endswith(", First>"):
            first = (cp, name[1:-len(", First>")])
            continue
        if name.endswith(", Last>"):
            lo, label = first
            for k, prefix in RANGE_PREFIX.items():
                if label == k or label.startswith(k + " "):
                    nr2.append((lo, cp, prefix))
            first = None
            continue
        if name.startswith("<"):
            continue  # <control>: only aliases name these
        prefix = next((p for p in NR2_PREFIXES if name == p + f"{cp:04X}"), None)
        if prefix:
            nr2.append((cp, cp, prefix))
        else:
            names[cp] = name
    # Merge adjacent same-prefix NR2 entries into ranges.
    merged = []
    for lo, hi, p in sorted(nr2):
        if merged and merged[-1][2] == p and lo == merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], hi, p)
        else:
            merged.append((lo, hi, p))
    return names, merged, ccc, decomp


def emit_ranges(f, name, doc, ranges):
    f.write(f"/// {doc}\n")
    f.write(f"pub(crate) static {name}: &[(u32, u32)] = &[\n")
    for lo, hi in ranges:
        f.write(f"    (0x{lo:04X}, 0x{hi:04X}),\n")
    f.write("];\n\n")


def emit_triples(f, name, ty, doc, rows, fmt):
    f.write(f"/// {doc}\n")
    f.write(f"pub(crate) static {name}: &[{ty}] = &[\n")
    for row in rows:
        f.write("    " + fmt(row) + ",\n")
    f.write("];\n\n")


def header(f):
    f.write("// @generated by scripts/gen-unicode-tables.py from the Unicode Character\n")
    f.write(f"// Database {VERSION}. Do not edit by hand; re-run the script.\n\n")


def write_tables(paths, ccc, decomp):
    core = parse_props(paths["DerivedCoreProperties.txt"],
                       [("XID_Start",), ("XID_Continue",)])
    norm = parse_props(paths["DerivedNormalizationProps.txt"],
                       [("NFC_QC", "N"), ("NFC_QC", "M"),
                        ("Full_Composition_Exclusion",)])

    def non_ascii(rs):
        return [(max(lo, 0x80), hi) for lo, hi in merge(rs) if hi >= 0x80]

    excluded = set()
    for lo, hi in norm[("Full_Composition_Exclusion",)]:
        excluded.update(range(lo, hi + 1))
    compose = sorted((d[0], d[1], cp) for cp, d in decomp.items()
                     if len(d) == 2 and cp not in excluded)
    # Combining classes as runs of equal value.
    runs = []
    for cp in sorted(ccc):
        if runs and runs[-1][1] == cp - 1 and runs[-1][2] == ccc[cp]:
            runs[-1] = (runs[-1][0], cp, ccc[cp])
        else:
            runs.append((cp, cp, ccc[cp]))
    out = os.path.join(SRC, "unicode_tables.rs")
    with open(out, "w", encoding="utf-8") as f:
        header(f)
        f.write("/// The UCD version these tables were generated from.\n")
        f.write(f"pub(crate) const UNICODE_VERSION: &str = \"{VERSION}\";\n\n")
        emit_ranges(f, "XID_START", "UAX #31 `XID_Start`, non-ASCII code points only.",
                    non_ascii(core[("XID_Start",)]))
        emit_ranges(f, "XID_CONTINUE", "UAX #31 `XID_Continue`, non-ASCII code points only.",
                    non_ascii(core[("XID_Continue",)]))
        emit_ranges(f, "NFC_QC_NO", "`NFC_Quick_Check=No`: never appears in NFC text.",
                    merge(norm[("NFC_QC", "N")]))
        emit_ranges(f, "NFC_QC_MAYBE",
                    "`NFC_Quick_Check=Maybe`: may compose with a preceding character.",
                    merge(norm[("NFC_QC", "M")]))
        emit_triples(f, "CCC", "(u32, u32, u8)",
                     "Non-zero `Canonical_Combining_Class` values as `(lo, hi, ccc)` runs.",
                     runs, lambda r: f"(0x{r[0]:04X}, 0x{r[1]:04X}, {r[2]})")
        emit_triples(f, "CANON_DECOMP", "(u32, u32, u32)",
                     "Canonical decomposition mappings `(cp, first, second)`, sorted by\n"
                     "/// `cp`; `second` is 0 for singleton decompositions. Hangul\n"
                     "/// syllables are algorithmic and not listed.",
                     sorted((cp, d[0], d[1] if len(d) == 2 else 0)
                            for cp, d in decomp.items()),
                     lambda r: f"(0x{r[0]:04X}, 0x{r[1]:04X}, 0x{r[2]:04X})")
        emit_triples(f, "COMPOSE", "(u32, u32, u32)",
                     "Primary composites `(first, second, composite)`, sorted by\n"
                     "/// `(first, second)`: canonical pair decompositions whose result is not\n"
                     "/// `Full_Composition_Exclusion`. Hangul is algorithmic.",
                     compose,
                     lambda r: f"(0x{r[0]:04X}, 0x{r[1]:04X}, 0x{r[2]:04X})")
    return os.path.getsize(out)


def write_names(paths, names, nr2):
    entries = [(n, cp, "") for cp, n in names.items()]
    for fields in records(paths["NameAliases.txt"]):
        entries.append((fields[1], int(fields[0], 16), ALIAS_TYPE[fields[2]]))
    keyed = sorted((loose_key(n), n, cp, t) for n, cp, t in entries)
    for a, b in zip(keyed, keyed[1:]):
        assert a[0] != b[0], f"loose-key collision: {a} {b}"
    for _, n, _, _ in keyed:
        assert all(c.isupper() or c.isdigit() or c in " -" for c in n), n
        assert len(n) < 94, n
    # Front-code in blocks of BLOCK entries; each block starts with a full
    # name. Line format: <chr(0x21 + shared prefix len)><suffix>;<HEX>[tag]\n
    lines, blocks, offset, prev = [], [], 0, ""
    for i, (_, n, cp, tag) in enumerate(keyed):
        if i % BLOCK == 0:
            blocks.append(offset)
            shared = 0
        else:
            shared = 0
            while shared < min(len(n), len(prev)) and n[shared] == prev[shared]:
                shared += 1
        line = f"{chr(0x21 + shared)}{n[shared:]};{cp:X}{tag}\n"
        lines.append(line)
        offset += len(line)
        prev = n
    text = "".join(lines)
    assert '"##' not in text
    max_len = max(len(n) for _, n, _, _ in keyed)
    out = os.path.join(SRC, "unicode_names.rs")
    with open(out, "w", encoding="utf-8") as f:
        header(f)
        f.write("/// Number of entries per front-coded block of [`NAMES`].\n")
        f.write(f"pub(crate) const BLOCK: usize = {BLOCK};\n\n")
        f.write("/// Longest name or alias, in bytes.\n")
        f.write(f"pub(crate) const MAX_NAME_LEN: usize = {max_len};\n\n")
        f.write("/// Byte offset in [`NAMES`] of each block's first (uncompressed) entry.\n")
        f.write("pub(crate) static NAME_BLOCKS: &[u32] = &[\n")
        for i in range(0, len(blocks), 8):
            f.write("    " + ", ".join(str(b) for b in blocks[i:i + 8]) + ",\n")
        f.write("];\n\n")
        f.write("/// `(lo, hi, prefix)`: NR2 names `prefix` + uppercase hex code point.\n")
        f.write("pub(crate) static NR2_RANGES: &[(u32, u32, &str)] = &[\n")
        for lo, hi, p in nr2:
            f.write(f"    (0x{lo:04X}, 0x{hi:04X}, \"{p}\"),\n")
        f.write("];\n\n")
        f.write(f"/// {len(keyed)} character names and aliases sorted by UAX #44-LM2 loose\n")
        f.write("/// key, one per line: a byte `0x21 + n` (the first `n` bytes are shared\n")
        f.write("/// with the previous entry's name; `n` is 0 at block starts), the rest of\n")
        f.write("/// the name, `;`, the code point in hex, and for an alias its\n")
        f.write("/// NameAliases.txt type: `c`orrection, control (`k`), `a`lternate,\n")
        f.write("/// `f`igment or a`b`breviation.\n")
        hashes = "" if '"' not in text else "#" * (1 + max(
            (len(m) for m in re.findall(r'"(#*)', text)), default=0))
        f.write(f'pub(crate) static NAMES: &str = r{hashes}"')
        f.write(text)
        f.write(f'"{hashes};\n')
    return os.path.getsize(out), len(keyed)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        paths = fetch_all(tmp)
        names, nr2, ccc, decomp = parse_unicode_data(paths["UnicodeData.txt"])
        tables = write_tables(paths, ccc, decomp)
        size, count = write_names(paths, names, nr2)
    print(f"unicode_tables.rs: {tables} bytes")
    print(f"unicode_names.rs: {size} bytes ({count} names and aliases, "
          f"{len(nr2)} NR2 ranges)")


main()
