//! WP-1557: `-fmason-metadata[=level]` end to end (spec/codegen.md
//! "Link-time metadata (`.mason.meta`)"; the bytes are mortar's
//! `spec/meta.md` §4 and its rule ids are cited as MT-n).
//!
//! - the IR global's shape, for C++ and C, and none without the flag;
//! - the object's section at `-O0` and `-O2`: `SHT_PROGBITS`, flags
//!   exactly `SHF_EXCLUDE`, alignment 8, in no group, no relocation
//!   section (MT-1, MT-2, MT-2a, MT-6), read with a small ELF64 reader
//!   here rather than an external tool, so the check runs on every host;
//! - the TU summary rows (MT-26), decoded by a reader written from the
//!   spec in this file;
//! - determinism (MT-38): one TU compiled from two directories with a
//!   prefix map gives identical sections;
//! - a PCH or BMI built with or without the flag serves both, and the
//!   flag changes nothing for `-E` and `-fsyntax-only`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d =
        std::env::temp_dir().join(format!("kiln-mason-meta-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(kilnpp()).args(args).current_dir(dir).output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Runs kiln++, which must succeed silently; returns its stdout. A `.c`
/// input is compiled as C (`-x c`; kiln++ takes it as C++).
fn ok(dir: &Path, args: &[&str]) -> String {
    let mut args = args.to_vec();
    if let Some(i) = args.iter().position(|a| Path::new(a).extension().is_some_and(|e| e == "c")) {
        args.splice(i..i, ["-x", "c"]);
    }
    let args = &args[..];
    let r = run(dir, args);
    assert_eq!((r.status.code(), text(&r.stderr).as_str()), (Some(0), ""), "{args:?}");
    text(&r.stdout)
}

const CXX: &str = "inline int h() { return 2; }\nint f() { return h(); }\n";
const C: &str = "int g(void) { return 1; }\n";

/// The `@__mason_meta` line of IR `ir`, if any.
fn meta_line(ir: &str) -> Option<&str> {
    ir.lines().find(|l| l.starts_with("@__mason_meta "))
}

/// The bytes of the global's `c"..."` initializer.
fn ir_bytes(line: &str) -> Vec<u8> {
    let start = line.find("c\"").unwrap() + 2;
    let s = &line.as_bytes()[start..];
    let mut out = Vec::new();
    let mut i = 0;
    while s[i] != b'"' {
        if s[i] == b'\\' {
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

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// The kind 1 rows (name, value, flags) of the one unit in `b`, after
/// checking the framing (MT-10, MT-11, MT-14) and the row order (MT-26).
fn summary_rows(b: &[u8]) -> Vec<(String, String, u32)> {
    assert_eq!(&b[..4], b"MASN");
    assert_eq!((u16_at(b, 4), u16_at(b, 6)), (1, 0), "version 1.0");
    let unit_size = u32_at(b, 8) as usize;
    assert_eq!(unit_size, b.len(), "one unit, the whole section");
    assert_eq!(unit_size % 8, 0);
    assert_eq!(u32_at(b, 12), 32, "header_size");
    assert_eq!(u32_at(b, 16), 1, "one block: kind 1 alone");
    let (st_off, st_size) = (u32_at(b, 20) as usize, u32_at(b, 24) as usize);
    assert!(st_off >= 32 && st_off + st_size <= unit_size);
    assert_eq!(b[st_off], 0, "the table starts with the empty string");
    assert_eq!(b[st_off + st_size - 1], 0);
    assert_eq!(u16_at(b, 28), 1, "producer kiln");
    let (kind, version) = (u16_at(b, 32), u16_at(b, 34));
    assert_eq!((kind, version), (1, 1));
    let (block_size, row_size, rows) =
        (u32_at(b, 36) as usize, u32_at(b, 40) as usize, u32_at(b, 44) as usize);
    assert_eq!(row_size, 16);
    assert!(16 + rows * 16 <= block_size && 32 + block_size <= st_off);
    let string = |o: u32| {
        let s = &b[st_off + o as usize..st_off + st_size];
        String::from_utf8(s[..s.iter().position(|&c| c == 0).unwrap()].to_vec()).unwrap()
    };
    let out: Vec<_> = (0..rows)
        .map(|i| {
            let r = 48 + i * 16;
            assert_eq!(u32_at(b, r + 12), 0, "reserved");
            (string(u32_at(b, r)), string(u32_at(b, r + 4)), u32_at(b, r + 8))
        })
        .collect();
    for w in out.windows(2) {
        assert!(w[0].0.as_bytes() < w[1].0.as_bytes(), "sorted, unique: {w:?}");
    }
    out
}

fn value<'r>(rows: &'r [(String, String, u32)], name: &str) -> Option<&'r str> {
    rows.iter().find(|r| r.0 == name).map(|r| r.1.as_str())
}

/// The rows of `src` compiled with `args` (through the IR).
fn rows_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> Vec<(String, String, u32)> {
    std::fs::write(dir.join(file), src).unwrap();
    let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-"];
    a.extend(args);
    a.push(file);
    let ir = ok(dir, &a);
    summary_rows(&ir_bytes(meta_line(&ir).expect("the global")))
}

const SHT_PROGBITS: u32 = 1;
const SHT_RELA: u32 = 4;
const SHT_REL: u32 = 9;
const SHT_GROUP: u32 = 17;
const SHF_EXCLUDE: u64 = 0x8000_0000;

struct Section {
    name: String,
    ty: u32,
    flags: u64,
    data: Vec<u8>,
    link: u32,
    info: u32,
    align: u64,
    entsize: u64,
}

/// The sections of an ELF64 little-endian relocatable object.
fn sections(obj: &[u8]) -> Vec<Section> {
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
fn container(obj: &[u8]) -> Vec<u8> {
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

#[test]
fn ir_global_shape_cxx_and_c() {
    let d = scratch("ir");
    std::fs::write(d.join("t.cpp"), CXX).unwrap();
    std::fs::write(d.join("t.c"), C).unwrap();
    for (file, extra) in [("t.cpp", &[][..]), ("t.c", &[][..]), ("t.cpp", &["-flto"][..])] {
        let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-", file];
        a.extend(extra);
        let ir = ok(&d, &a);
        let line = meta_line(&ir).expect("the global");
        let n = ir_bytes(line).len();
        let head = format!("@__mason_meta = private constant [{n} x i8] c\"MASN\\01\\00\\00\\00");
        assert!(line.starts_with(&head), "{line}");
        // The initializer, then exactly these attributes.
        let marker = "\", section \".mason.meta\", align 8, !exclude !";
        let (init, md) = line.rsplit_once(marker).unwrap_or_else(|| panic!("{line}"));
        assert!(!init[line.find("c\"").unwrap() + 2..].contains('"'), "{line}");
        let md = format!("!{md}");
        assert!(ir.lines().any(|l| l == format!("{md} = !{{}}")), "{md} is the empty node");
        assert!(
            ir.lines().any(|l| l.starts_with("@llvm.compiler.used = appending global")
                && l.contains("ptr @__mason_meta")),
            "listed in @llvm.compiler.used"
        );
        assert!(!ir.contains("@llvm.used"), "not in @llvm.used (SHF_GNU_RETAIN)");
        assert!(!line.contains("comdat") && !line.contains("thread_local"));
    }
    for args in [&["t.cpp"][..], &["-fmason-metadata", "-fno-mason-metadata", "t.cpp"][..]] {
        let mut a = vec!["-S", "-emit-llvm", "-o", "-"];
        a.extend(args);
        assert!(!ok(&d, &a).contains("mason"), "{args:?}");
    }
}

#[test]
fn object_section_at_o0_and_o2() {
    let d = scratch("obj");
    std::fs::write(d.join("t.cpp"), CXX).unwrap();
    std::fs::write(d.join("t.c"), C).unwrap();
    for (src, lang) in [("t.cpp", "c++"), ("t.c", "c")] {
        for o in ["-O0", "-O2"] {
            let out = format!("{src}{o}.o");
            ok(&d, &["-fmason-metadata", o, "-c", src, "-o", &out]);
            let obj = std::fs::read(d.join(&out)).unwrap();
            let rows = summary_rows(&container(&obj));
            assert_eq!(value(&rows, "lang"), Some(lang));
            assert_eq!(value(&rows, "source"), Some(src));
        }
    }
    ok(&d, &["-c", "t.cpp", "-o", "off.o"]);
    let obj = std::fs::read(d.join("off.o")).unwrap();
    assert!(sections(&obj).iter().all(|s| s.name != ".mason.meta"));
}

#[test]
fn tu_summary_rows() {
    let d = scratch("rows");
    let rows = rows_of(&d, "t.cpp", CXX, &["-std=c++20", "-fno-rtti"]);
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "abi.char",
            "abi.exceptions",
            "abi.long-double",
            "abi.rtti",
            "abi.short-enums",
            "abi.stdlib",
            "abi.wchar",
            "isa",
            "lang",
            "producer",
            "source",
            "std",
        ],
        "no header: no abi.stdlib-abi or abi.stdlib-debug; never odr.scheme or reloc-model"
    );
    let enforced: Vec<&str> = rows.iter().filter(|r| r.2 == 1).map(|r| r.0.as_str()).collect();
    assert_eq!(
        enforced,
        ["abi.char", "abi.long-double", "abi.short-enums", "abi.stdlib", "abi.wchar"]
    );
    assert!(rows.iter().all(|r| r.2 <= 1));
    assert_eq!(value(&rows, "std"), Some("c++20"));
    assert_eq!(value(&rows, "abi.rtti"), Some("off"));
    assert_eq!(value(&rows, "abi.exceptions"), Some("on"));
    assert_eq!(value(&rows, "abi.short-enums"), Some("off"));
    assert_eq!(value(&rows, "abi.long-double"), Some("128"));
    assert_eq!(value(&rows, "abi.wchar"), Some("4"));
    assert!(value(&rows, "producer").unwrap().starts_with("kiln "));
    assert!(!value(&rows, "isa").unwrap().is_empty());
    // The driver's decisions, not the defaults.
    let rows = rows_of(&d, "t.cpp", CXX, &["-fshort-enums", "-funsigned-char", "-fno-exceptions"]);
    assert_eq!(value(&rows, "abi.short-enums"), Some("on"));
    assert_eq!(value(&rows, "abi.char"), Some("unsigned"));
    assert_eq!(value(&rows, "abi.exceptions"), Some("off"));
    let rows = rows_of(&d, "t.cpp", CXX, &["-fsigned-char", "-std=gnu++17"]);
    assert_eq!(value(&rows, "abi.char"), Some("signed"));
    assert_eq!(value(&rows, "std"), Some("c++17"));
    // C: no C++ library rows.
    let rows = rows_of(&d, "t.c", C, &["-std=c11"]);
    assert_eq!(value(&rows, "lang"), Some("c"));
    assert_eq!(value(&rows, "std"), Some("c11"));
    assert!(rows.iter().all(|r| !r.0.starts_with("abi.stdlib")));
    // Every level writes the same unit today.
    let base = rows_of(&d, "t.cpp", CXX, &[]);
    for l in ["min", "detail", "all"] {
        assert_eq!(rows_of(&d, "t.cpp", CXX, &[&format!("-fmason-metadata={l}")]), base, "{l}");
    }
}

/// The standard library rows come from the macro table at the end of the
/// TU. Skipped where the library's headers are not installed.
#[test]
fn stdlib_rows_from_the_macros() {
    let d = scratch("stdlib");
    let src = "#include <cstddef>\nstd::size_t n;\n";
    let have = |args: &[&str]| {
        std::fs::write(d.join("h.cpp"), src).unwrap();
        let mut a = vec!["-fsyntax-only", "h.cpp"];
        a.extend(args);
        run(&d, &a).status.success()
    };
    if have(&["-stdlib=libstdc++"]) {
        let rows = |extra: &[&str]| {
            let mut a = vec!["-stdlib=libstdc++"];
            a.extend(extra);
            rows_of(&d, "h.cpp", src, &a)
        };
        let r = rows(&[]);
        assert_eq!(value(&r, "abi.stdlib"), Some("libstdc++"));
        assert_eq!(value(&r, "abi.stdlib-abi"), Some("1"));
        assert!(r.contains(&("abi.stdlib-debug".into(), "off".into(), 0)));
        let r = rows(&["-D_GLIBCXX_USE_CXX11_ABI=0"]);
        assert_eq!(value(&r, "abi.stdlib-abi"), Some("0"));
        let r = rows(&["-D_GLIBCXX_DEBUG"]);
        assert!(r.contains(&("abi.stdlib-debug".into(), "_GLIBCXX_DEBUG".into(), 1)));
        let r = rows(&["-D_GLIBCXX_ASSERTIONS"]);
        assert!(r.contains(&("abi.stdlib-debug".into(), "_GLIBCXX_ASSERTIONS".into(), 0)));
    } else {
        eprintln!("skipping libstdc++: <cstddef> not found");
    }
    if have(&["-stdlib=libc++"]) {
        let rows = |extra: &[&str]| {
            let mut a = vec!["-stdlib=libc++"];
            a.extend(extra);
            rows_of(&d, "h.cpp", src, &a)
        };
        let r = rows(&["-D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_FAST"]);
        assert_eq!(value(&r, "abi.stdlib"), Some("libc++"));
        assert_eq!(value(&r, "abi.stdlib-abi"), Some("1"));
        assert!(r.contains(&("abi.stdlib-debug".into(), "_LIBCPP_HARDENING_MODE_FAST".into(), 0)));
        let r = rows(&["-D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_NONE"]);
        assert!(r.contains(&("abi.stdlib-debug".into(), "off".into(), 0)));
    } else {
        eprintln!("skipping libc++: <cstddef> not found");
    }
}

/// MT-38: the same TU and options give the same section from two
/// directories, the path rewritten by `-ffile-prefix-map`.
#[test]
fn same_bytes_from_two_directories() {
    let root = scratch("determinism");
    let mut units = Vec::new();
    for dir in ["one", "two"] {
        let d = root.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("t.cpp"), CXX).unwrap();
        let abs = d.canonicalize().unwrap().display().to_string();
        let map = format!("-ffile-prefix-map={abs}=/src");
        let src = format!("{abs}/t.cpp");
        ok(&d, &["-fmason-metadata", &map, "-c", &src, "-o", "t.o"]);
        let unit = container(&std::fs::read(d.join("t.o")).unwrap());
        assert_eq!(value(&summary_rows(&unit), "source"), Some("/src/t.cpp"));
        units.push(unit);
    }
    assert_eq!(units[0], units[1]);
}

/// A PCH or module file built with the flag serves a compile without it
/// and the reverse, silently; only the compile's own flag decides the
/// section. `-E` and `-fsyntax-only` are unaffected.
#[test]
fn pch_bmi_and_front_end_unaffected() {
    let d = scratch("pch");
    std::fs::write(d.join("h.h"), "inline int h() { return 3; }\n").unwrap();
    std::fs::write(d.join("u.cpp"), "int f() { return h(); }\n").unwrap();
    ok(&d, &["-fmason-metadata", "-x", "c++-header", "h.h", "-o", "on.pch"]);
    ok(&d, &["-x", "c++-header", "h.h", "-o", "off.pch"]);
    assert_eq!(std::fs::read(d.join("on.pch")).unwrap(), std::fs::read(d.join("off.pch")).unwrap());
    for pch in ["on.pch", "off.pch"] {
        for (flag, want) in [(None, false), (Some("-fmason-metadata"), true)] {
            let mut a = vec!["-include-pch", pch, "-c", "u.cpp", "-o", "u.o"];
            a.extend(flag);
            ok(&d, &a);
            let obj = std::fs::read(d.join("u.o")).unwrap();
            assert_eq!(sections(&obj).iter().any(|s| s.name == ".mason.meta"), want, "{a:?}");
        }
    }
    std::fs::write(d.join("m.cppm"), "export module m;\nexport inline int k() { return 1; }\n")
        .unwrap();
    std::fs::write(d.join("i.cpp"), "import m;\nint z() { return k(); }\n").unwrap();
    ok(&d, &["-std=c++20", "-fmason-metadata", "--precompile", "m.cppm", "-o", "on.pcm"]);
    ok(&d, &["-std=c++20", "--precompile", "m.cppm", "-o", "off.pcm"]);
    assert_eq!(std::fs::read(d.join("on.pcm")).unwrap(), std::fs::read(d.join("off.pcm")).unwrap());
    ok(&d, &["-std=c++20", "-fmodule-file=m=on.pcm", "-c", "i.cpp", "-o", "i.o"]);
    ok(&d, &["-std=c++20", "-fmason-metadata", "-fmodule-file=m=off.pcm", "-c", "i.cpp"]);
    let pp = |flag: &[&str]| {
        let mut a = vec!["-E", "u.cpp"];
        a.extend(flag);
        ok(&d, &a)
    };
    assert_eq!(pp(&[]), pp(&["-fmason-metadata"]));
    assert_eq!(ok(&d, &["-fsyntax-only", "-fmason-metadata", "-include", "h.h", "u.cpp"]), "");
}

#[test]
fn invalid_level_is_rejected() {
    let d = scratch("invalid");
    std::fs::write(d.join("t.cpp"), CXX).unwrap();
    let r = run(&d, &["-fmason-metadata=max", "-c", "t.cpp"]);
    assert_eq!(r.status.code(), Some(1));
    assert!(
        text(&r.stderr).contains("invalid value 'max' in '-fmason-metadata=max'"),
        "{}",
        text(&r.stderr)
    );
}
