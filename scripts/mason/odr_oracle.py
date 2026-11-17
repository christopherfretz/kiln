#!/usr/bin/env python3
"""odr_oracle.py: the must-equal oracle of kiln's `.mason.meta` kind 2 rows.

Usage: python3 -I scripts/mason/odr_oracle.py [--names] [--quiet] PATH...

Each PATH is an ELF64 little-endian relocatable object, an `ar` archive of
them, or a directory searched recursively for `*.o` and `*.a` files. The
script reads every `.mason.meta` section (mortar's spec/meta.md: units
MT-10, blocks MT-11, the string table MT-12), decodes kind 1 (the unit
summary: `odr.scheme`) and kind 2 (one 48-byte definition row per symbol,
odr.md O-15), groups the kind 2 rows of all units by `key` and reports:

  (a) keys whose rows disagree in `hash` among rows of one `odr.scheme`
      (rows of different schemes are never compared);
  (b) K13 pairs: a key with an `EXPLICIT_SPEC` row and an `IMPLICIT_INST`
      row (an explicit specialization declared after an implicit
      instantiation in another unit);
  (c) per-flag row counts and the share of `PARTIAL` rows.

The exit status is 1 when (a) or (b) is not empty, 2 on a malformed input,
else 0. `--names` resolves each reported key back to a symbol name through
the symbol tables of the objects that carry the rows (key = FNV-1a 64 of
the name, MT-13). Python 3 standard library only; spec/harness.md
"The ODR oracle".
"""

import argparse
import os
import struct
import sys
from collections import defaultdict

FLAGS = [
    (1, "VAGUE"),
    (2, "IMPLICIT_INST"),
    (4, "EXPLICIT_INST"),
    (8, "EXPLICIT_SPEC"),
    (16, "USES_INTERNAL"),
    (32, "PARTIAL"),
    (64, "INLINED_ONLY"),
    (128, "GMF"),
    (256, "SYSTEM"),
]
EXPLICIT_SPEC = 8
IMPLICIT_INST = 2
PARTIAL = 32


class Malformed(Exception):
    pass


def fnv1a64(b):
    h = 0xCBF29CE484222325
    for c in b:
        h = ((h ^ c) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def elf_sections(obj):
    """(name, type, data) of each section of an ELF64 LE object."""
    if obj[:4] != b"\x7fELF" or obj[4] != 2 or obj[5] != 1:
        return None
    shoff, = struct.unpack_from("<Q", obj, 0x28)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", obj, 0x3A)
    heads = []
    for i in range(shnum):
        o = shoff + i * shentsize
        name, typ, _flags, _addr, off, size, link = struct.unpack_from("<IIQQQQI", obj, o)
        heads.append((name, typ, off, size, link))
    _, _, stroff, strsize, _ = heads[shstrndx]
    strtab = obj[stroff:stroff + strsize]

    def cstr(tab, o):
        return tab[o:tab.index(b"\0", o)]

    out = []
    for name, typ, off, size, link in heads:
        data = obj[off:off + size] if typ != 8 else b""  # SHT_NOBITS
        out.append((cstr(strtab, name).decode("latin1"), typ, data, link))
    return out, cstr


def symbol_names(secs, cstr):
    """The names of the defined symbols of an object."""
    names = set()
    for _name, typ, data, link in secs:
        if typ != 2:  # SHT_SYMTAB
            continue
        strtab = secs[link][2]
        for o in range(24, len(data), 24):  # entry 0 is the null symbol
            st_name, _info, _other, shndx = struct.unpack_from("<IBBH", data, o)
            if st_name and shndx != 0:
                names.add(cstr(strtab, st_name))
    return names


def units(sec, where):
    """Yield (scheme, [(key, hash, flags, class)]) per unit of a section."""
    at = 0
    while at < len(sec):
        if sec[at:at + 8] == b"\0" * 8:
            at += 8
            continue
        if sec[at:at + 4] != b"MASN":
            raise Malformed(f"{where}: no unit at offset {at} (MT-10)")
        major, _minor, size, hsize, nblocks, so, ss = struct.unpack_from("<HHIIIII", sec, at + 4)
        if major != 1 or size < hsize or size % 8 or at + size > len(sec):
            raise Malformed(f"{where}: bad unit header at offset {at} (MT-10)")
        unit = sec[at:at + size]

        def string(o):
            if so == 0 or o >= ss:
                raise Malformed(f"{where}: string offset {o} outside the table (MT-12)")
            e = unit.index(b"\0", so + o)
            return unit[so + o:e].decode("utf-8", "replace")

        scheme = None
        rows = []
        b = hsize
        for _ in range(nblocks):
            kind, version, bsize, rsize, rcount = struct.unpack_from("<HHIII", unit, b)
            if bsize < 16 or b + bsize > size or 16 + rsize * rcount > bsize:
                raise Malformed(f"{where}: bad block at unit offset {b} (MT-11)")
            if kind == 1:
                for i in range(rcount):
                    n, v, _f = struct.unpack_from("<III", unit, b + 16 + i * rsize)
                    if string(n) == "odr.scheme":
                        scheme = string(v)
            elif kind == 2 and version == 1:
                if rsize != 48:
                    raise Malformed(f"{where}: kind 2 row size {rsize}, not 48 (O-15)")
                for i in range(rcount):
                    r = b + 16 + i * 48
                    key, = struct.unpack_from("<Q", unit, r)
                    h = unit[r + 8:r + 24]
                    cls, flags = struct.unpack_from("<HH", unit, r + 28)
                    rows.append((key, h, flags, cls))
            b += bsize
        yield scheme, rows
        at += size


def members(path):
    """(label, bytes) of each ELF object in `path` (an object or archive)."""
    data = open(path, "rb").read()
    if not data.startswith(b"!<arch>\n"):
        yield path, data
        return
    at = 8
    longnames = b""
    while at + 60 <= len(data):
        head = data[at:at + 60]
        name = head[:16].decode("latin1").rstrip()
        size = int(head[48:58].decode("latin1").strip() or 0)
        body = data[at + 60:at + 60 + size]
        if name == "//":
            longnames = body
        elif name not in ("/", "/SYM64/"):
            if name.startswith("/") and name[1:].isdigit():
                o = int(name[1:])
                name = longnames[o:longnames.index(b"/\n", o)].decode("latin1")
            yield f"{path}({name.rstrip('/')})", body
        at += 60 + size + (size & 1)


def inputs(paths):
    for p in paths:
        if os.path.isdir(p):
            for root, dirs, files in os.walk(p):
                dirs.sort()
                for f in sorted(files):
                    if f.endswith((".o", ".a")):
                        yield os.path.join(root, f)
        else:
            yield p


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("paths", nargs="+", help="objects, archives or build directories")
    ap.add_argument("--names", action="store_true", help="report symbol names, not keys")
    ap.add_argument("--quiet", action="store_true", help="print only the summary")
    args = ap.parse_args(argv)

    # key -> [(scheme, hash, flags, where)]
    by_key = defaultdict(list)
    names = {}
    nobjects = nunits = 0
    try:
        for path in inputs(args.paths):
            for label, obj in members(path):
                parsed = elf_sections(obj)
                if parsed is None:
                    continue
                secs, cstr = parsed
                metas = [s for s in secs if s[0] == ".mason.meta"]
                if not metas:
                    continue
                nobjects += 1
                keys = set()
                for _n, _t, data, _l in metas:
                    for scheme, rows in units(data, label):
                        nunits += 1
                        for key, h, flags, _cls in rows:
                            by_key[key].append((scheme, h, flags, label))
                            keys.add(key)
                if args.names and keys:
                    for s in symbol_names(secs, cstr):
                        k = fnv1a64(s)
                        if k in keys:
                            names.setdefault(k, s.decode("latin1"))
    except Malformed as e:
        print(f"odr_oracle: malformed: {e}", file=sys.stderr)
        return 2

    def show(k):
        return names.get(k, f"{k:#018x}") if args.names else f"{k:#018x}"

    disagree, k13 = [], []
    counts = defaultdict(int)
    total = 0
    for key in sorted(by_key):
        rows = by_key[key]
        total += len(rows)
        for _s, _h, flags, _w in rows:
            for bit, _name in FLAGS:
                if flags & bit:
                    counts[bit] += 1
        per_scheme = defaultdict(dict)
        for scheme, h, _f, where in rows:
            per_scheme[scheme].setdefault(h, where)
        for scheme, hashes in sorted(per_scheme.items(), key=lambda x: str(x[0])):
            if len(hashes) > 1:
                disagree.append((key, scheme, sorted(hashes.values())))
        spec = [w for _s, _h, f, w in rows if f & EXPLICIT_SPEC]
        inst = [w for _s, _h, f, w in rows if f & IMPLICIT_INST]
        if spec and inst:
            k13.append((key, spec[0], inst[0]))

    if not args.quiet:
        for key, scheme, wheres in disagree:
            print(f"disagree: {show(key)} (odr.scheme {scheme}): {len(wheres)} hashes, e.g. "
                  + ", ".join(wheres[:2]))
        for key, s, i in k13:
            print(f"K13: {show(key)}: EXPLICIT_SPEC in {s}, IMPLICIT_INST in {i}")
    print(f"objects {nobjects}, units {nunits}, kind 2 rows {total}, keys {len(by_key)}")
    print("flags: " + ", ".join(f"{n} {counts[b]}" for b, n in FLAGS))
    share = 100.0 * counts[PARTIAL] / total if total else 0.0
    print(f"PARTIAL share {share:.1f}%")
    print(f"disagreeing keys {len(disagree)}, K13 pairs {len(k13)}")
    return 1 if disagree or k13 else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
