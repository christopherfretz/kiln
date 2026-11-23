//! Helpers shared by the `.mason.meta` end-to-end tests (`mason_meta.rs`,
//! `mason_defs.rs`): running kiln++, the container's framing decoded from
//! mortar's `spec/meta.md` (MT-10 to MT-14, MT-26), and a small ELF64
//! reader for the section (MT-1, MT-2, MT-6).
#![allow(dead_code, reason = "each test file uses a subset")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

pub fn scratch(name: &str) -> PathBuf {
    let d =
        std::env::temp_dir().join(format!("kiln-mason-meta-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(kilnpp()).args(args).current_dir(dir).output().unwrap()
}

pub fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Runs kiln++, which must succeed silently; returns its stdout. A `.c`
/// input is compiled as C (`-x c`; kiln++ takes it as C++).
pub fn ok(dir: &Path, args: &[&str]) -> String {
    let mut args = args.to_vec();
    if let Some(i) = args.iter().position(|a| Path::new(a).extension().is_some_and(|e| e == "c")) {
        args.splice(i..i, ["-x", "c"]);
    }
    let args = &args[..];
    let r = run(dir, args);
    assert_eq!((r.status.code(), text(&r.stderr).as_str()), (Some(0), ""), "{args:?}");
    text(&r.stdout)
}

/// The `@__mason_meta` line of IR `ir`, if any.
pub fn meta_line(ir: &str) -> Option<&str> {
    ir.lines().find(|l| l.starts_with("@__mason_meta "))
}

/// The bytes of the global's `c"..."` initializer.
pub fn ir_bytes(line: &str) -> Vec<u8> {
    let start = line.find("c\"").unwrap() + 2;
    let s = &line.as_bytes()[start..];
    let mut out = Vec::new();
    let mut i = 0;
    while s[i] != b'"' {
        if s[i..].starts_with(b"\\\\") {
            // LLVM writes a backslash byte as `\\`.
            out.push(b'\\');
            i += 2;
        } else if s[i] == b'\\' {
            let hex = std::str::from_utf8(&s[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

pub fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
pub fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
pub fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// The kind 1 rows (name, value, flags) of the one unit in `b`, after
/// checking the framing (MT-10, MT-11, MT-14) and the row order (MT-26).
pub fn summary_rows(b: &[u8]) -> Vec<(String, String, u32)> {
    assert_eq!(&b[..4], b"MASN");
    assert_eq!((u16_at(b, 4), u16_at(b, 6)), (1, 0), "version 1.0");
    let unit_size = u32_at(b, 8) as usize;
    assert_eq!(unit_size, b.len(), "one unit, the whole section");
    assert_eq!(unit_size % 8, 0);
    assert_eq!(u32_at(b, 12), 32, "header_size");
    let blocks = u32_at(b, 16);
    assert!(
        (1..=5).contains(&blocks),
        "kind 1; kind 2 in C++ units; kinds 3 and 5 from level default up, 4 at detail"
    );
    let (st_off, st_size) = (u32_at(b, 20) as usize, u32_at(b, 24) as usize);
    assert!(st_off >= 32 && st_off + st_size <= unit_size);
    assert_eq!(b[st_off], 0, "the table starts with the empty string");
    assert_eq!(b[st_off + st_size - 1], 0);
    assert_eq!(u16_at(b, 28), 1, "producer kiln");
    let mut at = 32;
    let mut last = 0;
    for _ in 0..blocks {
        let (kind, size) = (u16_at(b, at), u32_at(b, at + 4) as usize);
        assert!(kind > last && (1..=5).contains(&kind), "MT-11: sorted, one per kind");
        assert!(size % 8 == 0 && at + size <= st_off);
        last = kind;
        at += size;
    }
    let (kind, version) = (u16_at(b, 32), u16_at(b, 34));
    assert_eq!((kind, version), (1, 1));
    let (block_size, row_size, rows) =
        (u32_at(b, 36) as usize, u32_at(b, 40) as usize, u32_at(b, 44) as usize);
    assert_eq!(row_size, 16);
    assert!(16 + rows * 16 <= block_size && 32 + block_size <= st_off);
    let out: Vec<_> = (0..rows)
        .map(|i| {
            let r = 48 + i * 16;
            assert_eq!(u32_at(b, r + 12), 0, "reserved");
            (string_at(b, u32_at(b, r)), string_at(b, u32_at(b, r + 4)), u32_at(b, r + 8))
        })
        .collect();
    for w in out.windows(2) {
        assert!(w[0].0.as_bytes() < w[1].0.as_bytes(), "sorted, unique: {w:?}");
    }
    out
}

/// The offset of the unit's block of `kind`, if any (MT-11).
pub fn block_at(b: &[u8], kind: u16) -> Option<usize> {
    let mut at = 32;
    for _ in 0..u32_at(b, 16) {
        if u16_at(b, at) == kind {
            return Some(at);
        }
        at += u32_at(b, at + 4) as usize;
    }
    None
}

/// The string at offset `o` of the unit's string table (MT-12).
pub fn string_at(b: &[u8], o: u32) -> String {
    let (st_off, st_size) = (u32_at(b, 20) as usize, u32_at(b, 24) as usize);
    let s = &b[st_off + o as usize..st_off + st_size];
    String::from_utf8(s[..s.iter().position(|&c| c == 0).unwrap()].to_vec()).unwrap()
}

/// FNV-1a 64 (MT-13), written here from the spec.
pub fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

pub fn value<'r>(rows: &'r [(String, String, u32)], name: &str) -> Option<&'r str> {
    rows.iter().find(|r| r.0 == name).map(|r| r.1.as_str())
}

/// The rows of `src` compiled with `args` (through the IR).
pub fn rows_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> Vec<(String, String, u32)> {
    std::fs::write(dir.join(file), src).unwrap();
    let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-"];
    a.extend(args);
    a.push(file);
    let ir = ok(dir, &a);
    summary_rows(&ir_bytes(meta_line(&ir).expect("the global")))
}

pub const SHT_PROGBITS: u32 = 1;
pub const SHT_RELA: u32 = 4;
pub const SHT_REL: u32 = 9;
pub const SHT_GROUP: u32 = 17;
pub const SHF_EXCLUDE: u64 = 0x8000_0000;

pub struct Section {
    pub name: String,
    pub ty: u32,
    pub flags: u64,
    pub data: Vec<u8>,
    pub link: u32,
    pub info: u32,
    pub align: u64,
    pub entsize: u64,
}

/// The sections of an ELF64 little-endian relocatable object.
pub fn sections(obj: &[u8]) -> Vec<Section> {
    assert_eq!(&obj[..4], b"\x7fELF");
    assert_eq!((obj[4], obj[5]), (2, 1), "ELF64, little-endian");
    let shoff = usize::try_from(u64_at(obj, 0x28)).unwrap();
    let (shentsize, shnum, shstrndx) =
        (u16_at(obj, 0x3a) as usize, u16_at(obj, 0x3c) as usize, u16_at(obj, 0x3e) as usize);
    let raw: Vec<_> = (0..shnum)
        .map(|i| {
            let h = shoff + i * shentsize;
            let (off, size) = (u64_at(obj, h + 24) as usize, u64_at(obj, h + 32) as usize);
            let ty = u32_at(obj, h + 4);
            let data = if ty == 8 { Vec::new() } else { obj[off..off + size].to_vec() };
            (u32_at(obj, h), ty, u64_at(obj, h + 8), data, h)
        })
        .collect();
    let names = raw[shstrndx].3.clone();
    raw.into_iter()
        .map(|(n, ty, flags, data, h)| {
            let s = &names[n as usize..];
            Section {
                name: text(&s[..s.iter().position(|&c| c == 0).unwrap()]),
                ty,
                flags,
                data,
                link: u32_at(obj, h + 40),
                info: u32_at(obj, h + 44),
                align: u64_at(obj, h + 48),
                entsize: u64_at(obj, h + 56),
            }
        })
        .collect()
}

/// The `.mason.meta` section's bytes, after checking every property
/// mortar requires of it (MT-1, MT-2, MT-2a, MT-6).
pub fn container(obj: &[u8]) -> Vec<u8> {
    let secs = sections(obj);
    let found: Vec<usize> = (0..secs.len()).filter(|&i| secs[i].name == ".mason.meta").collect();
    assert_eq!(found.len(), 1, "one container");
    let i = found[0];
    let s = &secs[i];
    assert_eq!(s.ty, SHT_PROGBITS, "MT-1 type");
    assert_eq!(s.flags, SHF_EXCLUDE, "MT-1/MT-6: flags exactly SHF_EXCLUDE");
    assert_eq!((s.align, s.entsize, s.link, s.info), (8, 0, 0, 0), "MT-1");
    let idx = u32::try_from(i).unwrap();
    for r in &secs {
        assert!(
            !(matches!(r.ty, SHT_REL | SHT_RELA) && r.info == idx),
            "MT-2a: {} relocates the container",
            r.name
        );
        if r.ty == SHT_GROUP {
            let members: Vec<u32> = r.data.chunks(4).skip(1).map(|c| u32_at(c, 0)).collect();
            assert!(!members.contains(&idx), "MT-2: the container is in group {}", r.name);
        }
    }
    s.data.clone()
}

/// The definition hash's scheme (kind 1 `odr.scheme`) the kind 2 tests
/// expect.
pub const DEF_SCHEME: &str = "kiln-odr-2";

/// Kind 2 row flags (O-15) and classes.
pub const VAGUE: u16 = 1;
pub const IMPLICIT_INST: u16 = 2;
pub const EXPLICIT_INST: u16 = 4;
pub const EXPLICIT_SPEC: u16 = 8;
pub const USES_INTERNAL: u16 = 16;
pub const PARTIAL: u16 = 32;
pub const SYSTEM: u16 = 256;
pub const FUNCTION: u16 = 1;
pub const VARIABLE: u16 = 2;

/// One kind 2 row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def {
    pub key: u64,
    pub hash: u128,
    pub class: u16,
    pub flags: u16,
    pub file: String,
    pub line: u32,
}

/// The `name` strings of the kind 2 rows of `b` by key (0 rows: none).
pub fn def_names(b: &[u8]) -> Vec<(u64, String)> {
    let Some(at) = block_at(b, 2) else { return Vec::new() };
    (0..u32_at(b, at + 12) as usize)
        .map(|i| at + 16 + i * 48)
        .filter(|&r| u32_at(b, r + 24) != 0)
        .map(|r| (u64_at(b, r), string_at(b, u32_at(b, r + 24))))
        .collect()
}

/// The kind 2 rows of the unit in `b`, `None` without a kind 2 block,
/// after checking the framing, the block and every row (O-15: `name` 0
/// or, at `all`, the symbol of a vague row,
/// `module` 0, reserved 0, a hash, no flag kiln does not write,
/// sorted by key, one row per key).
pub fn def_rows(b: &[u8]) -> Option<Vec<Def>> {
    let summary = summary_rows(b);
    let at = block_at(b, 2)?;
    assert_eq!((u16_at(b, at), u16_at(b, at + 2)), (2, 1), "kind 2, version 1");
    let (block_size, row_size, rows) =
        (u32_at(b, at + 4) as usize, u32_at(b, at + 8) as usize, u32_at(b, at + 12) as usize);
    assert_eq!(row_size, 48);
    assert!(rows > 0, "no block without rows");
    assert!(16 + rows * 48 <= block_size && at + block_size <= u32_at(b, 20) as usize);
    assert_eq!(value(&summary, "odr.scheme"), Some(DEF_SCHEME));
    let out: Vec<Def> = (0..rows)
        .map(|i| {
            let r = at + 16 + i * 48;
            // `name`: 0 (anchored), or at `all` a vague row's symbol (K-3f).
            let name = u32_at(b, r + 24);
            assert!(
                name == 0
                    || (u16_at(b, r + 30) & VAGUE != 0 && fnv(&string_at(b, name)) == u64_at(b, r)),
                "name {name} of row {i}"
            );
            assert_eq!(u32_at(b, r + 32), 0, "module");
            assert_eq!(u32_at(b, r + 44), 0, "reserved");
            let d = Def {
                key: u64_at(b, r),
                hash: u128::from_le_bytes(b[r + 8..r + 24].try_into().unwrap()),
                class: u16_at(b, r + 28),
                flags: u16_at(b, r + 30),
                file: string_at(b, u32_at(b, r + 36)),
                line: u32_at(b, r + 40),
            };
            assert!(d.class == FUNCTION || d.class == VARIABLE, "{d:?}");
            assert_ne!(d.hash, 0, "{d:?}");
            assert_eq!(
                d.flags
                    & !(VAGUE
                        | IMPLICIT_INST
                        | EXPLICIT_INST
                        | EXPLICIT_SPEC
                        | USES_INTERNAL
                        | PARTIAL
                        | SYSTEM),
                0,
                "{d:?}"
            );
            d
        })
        .collect();
    for w in out.windows(2) {
        assert!(w[0].key < w[1].key, "sorted by key, one row per key: {w:?}");
    }
    Some(out)
}

/// The IR of `src` compiled with `-fmason-metadata` and `args`.
pub fn ir_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> String {
    std::fs::write(dir.join(file), src).unwrap();
    let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-"];
    a.extend(args);
    a.push(file);
    ok(dir, &a)
}

/// The kind 2 rows of `src` compiled with `args` (through the IR).
pub fn defs_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> Option<Vec<Def>> {
    def_rows(&ir_bytes(meta_line(&ir_of(dir, file, src, args)).expect("the global")))
}

/// The row of `symbol`, if any.
pub fn row<'r>(rows: &'r [Def], symbol: &str) -> Option<&'r Def> {
    rows.iter().find(|d| d.key == fnv(symbol))
}

/// The symbols IR `ir` defines (functions, variables, aliases).
pub fn defined(ir: &str) -> Vec<String> {
    ir.lines()
        .filter_map(|l| {
            let rest = if l.starts_with("define ") {
                l.split_once(" @")?.1
            } else if let Some(r) = l.strip_prefix('@') {
                let (_, init) = r.split_once(" = ")?;
                if init.starts_with("external ") || init.contains("declare ") {
                    return None;
                }
                r
            } else {
                return None;
            };
            let rest = rest.trim_start_matches('"');
            Some(rest[..rest.find(['(', ' ', '"']).unwrap_or(rest.len())].to_owned())
        })
        .collect()
}

/// The objects of `units` (name, source), compiled with `args` in `d`.
pub fn objects(d: &Path, units: &[(&str, &str)], args: &[&str]) -> Vec<Vec<u8>> {
    units
        .iter()
        .map(|(name, src)| {
            std::fs::write(d.join(name), src).unwrap();
            let out = format!("{name}.o");
            let mut a = vec!["-fmason-metadata", "-c", "-o", &out];
            a.extend(args);
            a.push(name);
            ok(d, &a);
            container(&std::fs::read(d.join(&out)).unwrap())
        })
        .collect()
}
