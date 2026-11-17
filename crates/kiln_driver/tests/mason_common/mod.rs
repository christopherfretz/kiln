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
