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

/// A C++ unit with one entity of each category of spec/codegen.md
/// "Record kind 5": what gets a row and what does not.
const DECLS_CXX: &str = r#"
int defined_fn(int a) noexcept { return a; }
int declared_fn(long);
struct S { int m(int) const; static int sm; S(); ~S(); };
int S::sm = 1;
extern int arr[];
int arr2[4];
extern "C" int cvar;
extern "C" int cfun(const char *, ...);
static int internal_fn() { return 1; }
namespace { int anon_v; }
int renamed() asm("renamed_label");
__attribute__((weak)) int weak_def() { return 0; }
inline int inl() { return 3; }
template <class T> T tf(T t) { return t; }
struct Inc;
extern Inc inc_v;
int use() {
  extern int blk();
  S s;
  return defined_fn(1) + declared_fn(2) + s.m(1) + S::sm + arr[0] + arr2[0] + cvar + cfun("")
       + internal_fn() + anon_v + renamed() + weak_def() + inl() + tf(1) + blk()
       + (int)(long)&inc_v;
}
"#;

/// The C unit of the golden: every row is `EXTERN_C`; the unprototyped
/// declaration and the K&R definition get none.
const DECLS_C: &str = r"
enum E { EA, EB };
typedef long L;
struct node;
int c_def(int a, const char *s) { return a + s[0]; }
int c_ref(enum E, const L, struct node *, double[static 3]);
extern int c_arr[];
int c_var = 1;
_Atomic long c_at;
int noproto();
int knr(a) int a; { return a; }
void abort(void);
unsigned long strlen(const char *);
double c_d[3];
int c_use(void) {
  return c_ref(EA, 0, 0, c_d) + c_arr[0] + (int)c_at + noproto() + knr(1)
       + (int)strlen((const char *)c_arr) + (c_var ? 0 : (abort(), 0));
}
";

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
    let blocks = u32_at(b, 16);
    assert!(blocks == 1 || blocks == 2, "kind 1, and kind 5 from level default up");
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

/// The string at offset `o` of the unit's string table (MT-12).
fn string_at(b: &[u8], o: u32) -> String {
    let (st_off, st_size) = (u32_at(b, 20) as usize, u32_at(b, 24) as usize);
    let s = &b[st_off + o as usize..st_off + st_size];
    String::from_utf8(s[..s.iter().position(|&c| c == 0).unwrap()].to_vec()).unwrap()
}

// Record kind 5, the declaration rows (WP-1564; mortar `spec/odr.md`
// O-35, O-33).

const DEFINITION: u16 = 1;
const VARIABLE: u16 = 2;
const EXTERN_C: u16 = 4;
const NOEXCEPT: u16 = 8;

/// FNV-1a 64 (MT-13), written here from the spec.
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

/// One kind 5 row; `ty` is checked to be the hash of `type_name`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Decl {
    key: u64,
    ty: u64,
    size: u64,
    type_name: String,
    flags: u16,
}

/// The kind 5 rows of the unit in `b`, `None` when it has no kind 5
/// block, after checking the framing, the block (MT-11) and every row
/// (O-35: `name` 0, reserved 0, flags in bits 0-3, sorted by key, one row
/// per key).
fn decl_rows(b: &[u8]) -> Option<Vec<Decl>> {
    summary_rows(b);
    if u32_at(b, 16) == 1 {
        return None;
    }
    let st_off = u32_at(b, 20) as usize;
    let at = 32 + u32_at(b, 36) as usize;
    assert_eq!((u16_at(b, at), u16_at(b, at + 2)), (5, 1), "kind 5, version 1");
    let (block_size, row_size, rows) =
        (u32_at(b, at + 4) as usize, u32_at(b, at + 8) as usize, u32_at(b, at + 12) as usize);
    assert_eq!(row_size, 40);
    assert!(16 + rows * 40 <= block_size && at + block_size <= st_off);
    let out: Vec<Decl> = (0..rows)
        .map(|i| {
            let r = at + 16 + i * 40;
            assert_eq!(u32_at(b, r + 24), 0, "name: anchored rows only");
            assert_eq!((u16_at(b, r + 34), u32_at(b, r + 36)), (0, 0), "reserved");
            let d = Decl {
                key: u64_at(b, r),
                ty: u64_at(b, r + 8),
                size: u64_at(b, r + 16),
                type_name: string_at(b, u32_at(b, r + 28)),
                flags: u16_at(b, r + 32),
            };
            assert_eq!(d.flags & !0xf, 0, "{d:?}");
            // An empty type string: a C++-linkage function below `detail`.
            if d.type_name.is_empty() {
                assert_eq!(d.flags & (VARIABLE | EXTERN_C), 0, "{d:?}");
            } else {
                assert_eq!(d.ty, fnv(&d.type_name), "{d:?}");
            }
            d
        })
        .collect();
    for w in out.windows(2) {
        assert!(w[0].key < w[1].key, "sorted by key, one row per key: {w:?}");
    }
    Some(out)
}

/// The kind 5 rows of `src` compiled with `args` (through the IR).
fn decls_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> Option<Vec<Decl>> {
    std::fs::write(dir.join(file), src).unwrap();
    let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-"];
    a.extend(args);
    a.push(file);
    let ir = ok(dir, &a);
    decl_rows(&ir_bytes(meta_line(&ir).expect("the global")))
}

/// The rows a golden of `(symbol, flags, size, type)` stands for.
fn golden(rows: &[(&str, u16, u64, &str)]) -> Vec<Decl> {
    let mut v: Vec<Decl> = rows
        .iter()
        .map(|&(sym, flags, size, t)| Decl {
            key: fnv(sym),
            ty: fnv(t),
            size,
            type_name: t.to_owned(),
            flags,
        })
        .collect();
    v.sort_by_key(|d| d.key);
    v
}

/// `rows` as level `default` writes them: no type string for a
/// C++-linkage function, its `type` hash kept.
fn at_default(mut rows: Vec<Decl>) -> Vec<Decl> {
    for r in &mut rows {
        if r.flags & (VARIABLE | EXTERN_C) == 0 {
            r.type_name.clear();
        }
    }
    rows
}

/// The row of `symbol`, if any.
fn row<'r>(rows: &'r [Decl], symbol: &str) -> Option<&'r Decl> {
    rows.iter().find(|d| d.key == fnv(symbol))
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
    std::fs::write(d.join("big.cpp"), DECLS_CXX).unwrap();
    for (src, lang) in [("t.cpp", "c++"), ("t.c", "c"), ("big.cpp", "c++")] {
        let mut decls = Vec::new();
        for o in ["-O0", "-O2"] {
            let out = format!("{src}{o}.o");
            ok(&d, &["-fmason-metadata", o, "-c", src, "-o", &out]);
            let obj = std::fs::read(d.join(&out)).unwrap();
            let unit = container(&obj);
            let rows = summary_rows(&unit);
            assert_eq!(value(&rows, "lang"), Some(lang));
            assert_eq!(value(&rows, "source"), Some(src));
            decls.push(decl_rows(&unit).expect("kind 5 at the default level"));
        }
        // The rows are the front end's, taken before the optimizer runs.
        assert_eq!(decls[0], decls[1], "{src}");
        assert!(!decls[0].is_empty());
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
    // Every level writes the same kind 1 rows.
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
        // WP-1564: `abi.stdlib-debug` is ENFORCE on every value.
        assert!(r.contains(&("abi.stdlib-debug".into(), "off".into(), 1)));
        assert_eq!(value(&r, "abi.stdlib-hardening"), None);
        let r = rows(&["-D_GLIBCXX_USE_CXX11_ABI=0"]);
        assert_eq!(value(&r, "abi.stdlib-abi"), Some("0"));
        let r = rows(&["-D_GLIBCXX_DEBUG"]);
        assert!(r.contains(&("abi.stdlib-debug".into(), "_GLIBCXX_DEBUG".into(), 1)));
        let r = rows(&["-D_GLIBCXX_ASSERTIONS"]);
        assert!(r.contains(&("abi.stdlib-debug".into(), "off".into(), 1)));
        assert!(r.contains(&("abi.stdlib-hardening".into(), "_GLIBCXX_ASSERTIONS".into(), 0)));
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
        assert!(r.contains(&("abi.stdlib-debug".into(), "off".into(), 1)));
        assert!(r.contains(&("abi.stdlib-hardening".into(), "fast".into(), 0)));
        let r = rows(&["-D_LIBCPP_HARDENING_MODE=_LIBCPP_HARDENING_MODE_NONE"]);
        assert!(r.contains(&("abi.stdlib-hardening".into(), "none".into(), 0)));
        // The layout-changing ABI switches, in byte order of the names;
        // a switch that changes no layout is not listed.
        let r = rows(&[
            "-D_LIBCPP_ABI_NO_ITERATOR_BASES",
            "-D_LIBCPP_ABI_NO_REVERSE_ITERATOR_SECOND_MEMBER",
            "-D_LIBCPP_ABI_BOUNDED_ITERATORS",
            "-D_LIBCPP_ABI_FIX_CITYHASH_IMPLEMENTATION",
        ]);
        assert_eq!(
            value(&r, "abi.stdlib-abi"),
            Some(
                "1+_LIBCPP_ABI_BOUNDED_ITERATORS+_LIBCPP_ABI_NO_ITERATOR_BASES\
                 +_LIBCPP_ABI_NO_REVERSE_ITERATOR_SECOND_MEMBER"
            )
        );
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
        std::fs::write(d.join("t.cpp"), DECLS_CXX).unwrap();
        let abs = d.canonicalize().unwrap().display().to_string();
        let map = format!("-ffile-prefix-map={abs}=/src");
        let src = format!("{abs}/t.cpp");
        ok(&d, &["-fmason-metadata", &map, "-c", &src, "-o", "t.o"]);
        let unit = container(&std::fs::read(d.join("t.o")).unwrap());
        assert_eq!(value(&summary_rows(&unit), "source"), Some("/src/t.cpp"));
        assert!(decl_rows(&unit).is_some_and(|r| r.len() > 10));
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
    std::fs::write(
        d.join("h.h"),
        "inline int h() { return 3; }\nint from_pch(int);\nextern int pch_var;\n",
    )
    .unwrap();
    std::fs::write(d.join("u.cpp"), "int f() { return h() + from_pch(1) + pch_var; }\n").unwrap();
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
            if want {
                // Entities declared in the PCH and used by the TU get rows.
                let rows = decl_rows(&container(&obj)).unwrap();
                let want = at_default(golden(&[
                    ("_Z1fv", DEFINITION, 0, "FivE"),
                    ("_Z1hv", DEFINITION, 0, "FivE"),
                    ("_Z8from_pchi", 0, 0, "FiiE"),
                    ("pch_var", VARIABLE, 4, "i"),
                ]));
                assert_eq!(rows, want, "{a:?}");
            }
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
    // A module-attached entity's key is its mangled name, `W` in it.
    let rows = decl_rows(&container(&std::fs::read(d.join("i.o")).unwrap())).unwrap();
    let k = row(&rows, "_ZW1m1kv").expect("k's row");
    assert_eq!((k.flags, k.ty, k.type_name.as_str()), (DEFINITION, fnv("FivE"), ""));
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

/// Goldens of the kind 5 rows of a small C++ unit and a small C unit
/// (target-independent forms; the target's own are below).
#[test]
fn declaration_rows_golden() {
    let d = scratch("decls");
    let detail = ["-std=c++20", "-fmason-metadata=detail"];
    let rows = decls_of(&d, "t.cpp", DECLS_CXX, &detail).unwrap();
    let want = golden(&[
        ("_Z10defined_fni", DEFINITION | NOEXCEPT, 0, "FiiE"),
        ("_Z11declared_fnl", 0, 0, "FilE"),
        ("_ZNK1S1mEi", 0, 0, "KFiiE"),
        ("_ZN1S2smE", DEFINITION | VARIABLE, 4, "i"),
        ("arr", VARIABLE, 0, "A_i"),
        ("arr2", DEFINITION | VARIABLE, 16, "A_i"),
        ("cvar", VARIABLE | EXTERN_C, 4, "i"),
        ("cfun", EXTERN_C, 0, "FiPKczE"),
        ("_Z8weak_defv", DEFINITION, 0, "FivE"),
        ("_Z3inlv", DEFINITION, 0, "FivE"),
        ("_Z2tfIiET_S0_", DEFINITION, 0, "FiiE"),
        ("_Z3blkv", 0, 0, "FivE"),
        ("inc_v", VARIABLE, 0, "3Inc"),
        ("_Z3usev", DEFINITION, 0, "FivE"),
    ]);
    assert_eq!(rows, want, "no row: internal, anonymous namespace, structors, asm label");
    let rows = decls_of(&d, "t.cpp", DECLS_CXX, &["-std=c++20"]).unwrap();
    assert_eq!(rows, at_default(want), "default: no type string for C++ functions");
    let rows =
        decls_of(&d, "t.c", DECLS_C, &["-std=c17", "-Wno-deprecated-non-prototype"]).unwrap();
    let want = golden(&[
        ("c_def", DEFINITION | EXTERN_C, 0, "FiiPKcE"),
        ("c_ref", EXTERN_C, 0, "FiilPRPdE"),
        ("c_arr", VARIABLE | EXTERN_C, 0, "A_i"),
        ("c_var", DEFINITION | VARIABLE | EXTERN_C, 4, "i"),
        ("c_at", DEFINITION | VARIABLE | EXTERN_C, 8, "Ul"),
        ("c_d", DEFINITION | VARIABLE | EXTERN_C, 24, "A_d"),
        ("c_use", DEFINITION | EXTERN_C, 0, "FivE"),
        // Library builtins the source declares (sema declared them first).
        ("abort", EXTERN_C, 0, "FvvE"),
        ("strlen", EXTERN_C, 0, "FmPKcE"),
    ]);
    assert_eq!(rows, want, "no row: noproto (unprototyped), knr (K&R definition)");
}

/// The forms that depend on the target: `va_list`, `wchar_t`, `long
/// double`; the same in C and in C++.
#[test]
fn target_forms() {
    let d = scratch("target");
    let c = "void t_va(__builtin_va_list);\nvoid t_w(__WCHAR_TYPE__ *);\n\
             long double t_ld(long double);\n\
             void *const tab[] = { (void *)t_va, (void *)t_w, (void *)t_ld };\n";
    let cxx = format!(
        "extern \"C\" {{\nvoid t_va(__builtin_va_list);\nvoid t_w(wchar_t *);\n\
             long double t_ld(long double);\nextern void *const tab[];\n}}\n{}",
        c.lines().last().unwrap()
    );
    for (target, w, ld) in [("x86_64-linux-gnu", "i", "e"), ("aarch64-linux-gnu", "i", "g")] {
        let t = format!("--target={target}");
        let want = golden(&[
            ("t_va", EXTERN_C, 0, "FvVE"),
            ("t_w", EXTERN_C, 0, &format!("FvP{w}E")),
            ("t_ld", EXTERN_C, 0, &format!("F{ld}{ld}E")),
            ("tab", DEFINITION | VARIABLE | EXTERN_C, 24, "A_KPv"),
        ]);
        assert_eq!(decls_of(&d, "t.c", c, &[&t]).unwrap(), want, "{target} C");
        assert_eq!(decls_of(&d, "t.cpp", &cxx, &[&t]).unwrap(), want, "{target} C++");
    }
}

#[test]
fn levels_write_kind_5_from_default() {
    let d = scratch("levels");
    assert_eq!(decls_of(&d, "t.cpp", CXX, &["-fmason-metadata=min"]), None, "min: kinds 1, 2");
    let base = decls_of(&d, "t.cpp", CXX, &[]).unwrap();
    assert_eq!(base.len(), 2);
    assert_eq!(decls_of(&d, "t.cpp", CXX, &["-fmason-metadata=default"]), Some(base.clone()));
    // `detail` and `all` add the type strings of C++-linkage functions;
    // the `type` hash is the same at every level, row by row.
    let detail = decls_of(&d, "t.cpp", CXX, &["-fmason-metadata=detail"]).unwrap();
    assert!(base.iter().all(|r| r.type_name.is_empty()));
    assert_eq!(at_default(detail.clone()), base);
    assert_eq!(decls_of(&d, "t.cpp", CXX, &["-fmason-metadata=all"]), Some(detail));
    let full = decls_of(&d, "d.cpp", DECLS_CXX, &["-fmason-metadata=detail"]).unwrap();
    let dflt = decls_of(&d, "d.cpp", DECLS_CXX, &[]).unwrap();
    assert_eq!(full.len(), dflt.len());
    for (a, b) in full.iter().zip(&dflt) {
        assert_eq!((a.key, a.ty, a.size, a.flags), (b.key, b.ty, b.size, b.flags));
    }
    // Variables and C-linkage rows keep their strings at `default`.
    assert!(
        dflt.iter()
            .filter(|r| r.flags & (VARIABLE | EXTERN_C) != 0)
            .all(|r| !r.type_name.is_empty())
    );
}

/// The C side of the cross-language pairs: one declaration per rule of
/// the canonical C type, each referenced.
const XL_C: &str = r"
enum E { EA, EB };
typedef long L;
typedef __WCHAR_TYPE__ wchar_t;
typedef __UINT_LEAST16_TYPE__ char16_t;
typedef __UINT_LEAST32_TYPE__ char32_t;
struct s; struct t; struct sa; struct sb;
typedef union { struct sa *a; struct sb *b; } TU __attribute__((transparent_union));
typedef int v4 __attribute__((vector_size(16)));
int f_enum(enum E);
int f_q(const int, volatile long);
void f_td(L);
void f_w(wchar_t *);
void f_c16(char16_t, char32_t);
void f_c8(unsigned char *);
void f_bool(_Bool);
void f_rec(struct s *, struct t);
void f_va(__builtin_va_list);
void f_ne(void (*)(int));
void f_ref(int *, const int *);
void f_rs(char *restrict, char *restrict *);
void f_cv(const volatile int *);
int f_tu(TU);
int f_void(void);
int f_np();
_Atomic int a_at;
_Complex double a_cx;
_Complex float f_cx(_Complex long double);
_BitInt(37) f_bi(unsigned _BitInt(9));
v4 f_vec(v4);
_Float16 f_h(__float128, long double);
__int128 f_i128(unsigned __int128);
void f_vfp(int (*)(const char *, ...));
void f_parr(int (*)[4], int (*)[]);
void f_vla(int n, int a[n], int b[*][n]);
void f_st(int a[static 4], const int b[const 2]);
_Noreturn void f_nr(void);
[[noreturn]] void f_nr2(void);
void f_vz(int, ...);
int v_i;
extern long v_arr[];
#if __STDC_VERSION__ < 202311L
int f_kr(a) int a; { return a; }
void f_cfp(int (*)());
__attribute__((preserve_most)) void f_pm(void);
void *const no_row[] = { (void *)f_kr, (void *)f_cfp, (void *)f_pm };
#endif
void *const tab[] = { (void *)f_enum, (void *)f_q, (void *)f_td, (void *)f_w, (void *)f_c16,
  (void *)f_c8, (void *)f_bool, (void *)f_rec, (void *)f_va, (void *)f_ne, (void *)f_ref,
  (void *)f_rs, (void *)f_cv, (void *)f_tu, (void *)f_void, (void *)f_np, &a_at, &a_cx,
  (void *)f_cx, (void *)f_bi, (void *)f_vec, (void *)f_h, (void *)f_i128, (void *)f_vfp,
  (void *)f_parr, (void *)f_vla, (void *)f_st, (void *)f_nr, (void *)f_nr2, (void *)f_vz,
  &v_i, v_arr };
";

/// The C++ side: the same entities, spelled in C++.
const XL_CXX: &str = r#"
enum E { EA, EB };
namespace n { struct s; }
struct t;
struct sa;
typedef int v4 __attribute__((vector_size(16)));
extern "C" {
int f_enum(E);
int f_q(int, long);
void f_td(long);
void f_w(wchar_t *);
void f_c16(char16_t, char32_t);
void f_c8(char8_t *);
void f_bool(bool);
void f_rec(n::s *, t);
void f_va(__builtin_va_list);
void f_ne(void (*)(int) noexcept) noexcept;
void f_ref(int &, const int &&);
void f_rs(char *, char **);
void f_cv(const volatile int *);
int f_tu(sa *);
int f_void();
int f_np(void);
_Atomic int a_at;
_Complex double a_cx;
_Complex float f_cx(_Complex long double);
_BitInt(37) f_bi(unsigned _BitInt(9));
v4 f_vec(v4);
_Float16 f_h(__float128, long double);
__int128 f_i128(unsigned __int128);
void f_vfp(int (*)(const char *, ...) noexcept);
void f_parr(int (*)[4], int (*)[]);
void f_vla(int n, int *a, int (*b)[]);
void f_st(int *, const int *const);
[[noreturn]] void f_nr();
[[noreturn]] void f_nr2(void);
void f_vz(int, ...);
int v_i;
extern long v_arr[];
extern void *const tab[];
void *const tab[] = { (void *)f_enum, (void *)f_q, (void *)f_td, (void *)f_w, (void *)f_c16,
  (void *)f_c8, (void *)f_bool, (void *)f_rec, (void *)f_va, (void *)f_ne, (void *)f_ref,
  (void *)f_rs, (void *)f_cv, (void *)f_tu, (void *)f_void, (void *)f_np, &a_at, &a_cx,
  (void *)f_cx, (void *)f_bi, (void *)f_vec, (void *)f_h, (void *)f_i128, (void *)f_vfp,
  (void *)f_parr, (void *)f_vla, (void *)f_st, (void *)f_nr, (void *)f_nr2, (void *)f_vz,
  &v_i, v_arr };
}
"#;

/// O-33: a C unit and a C++ unit declaring the same C entities agree on
/// every row byte for byte (key, type, size, flags), on both targets.
#[test]
fn c_and_cxx_units_agree() {
    let d = scratch("xlang");
    let quiet = "-Wno-deprecated-non-prototype";
    for target in ["--target=x86_64-linux-gnu", "--target=aarch64-linux-gnu"] {
        let cxx = decls_of(&d, "x.cpp", XL_CXX, &["-std=c++20", target]).unwrap();
        assert_eq!(cxx.len(), 33);
        assert!(cxx.iter().all(|r| r.flags & EXTERN_C != 0));
        assert_eq!(decls_of(&d, "x.c", XL_C, &["-std=c23", target]).unwrap(), cxx, "{target}");
        // Before C23 `f_np()` is unprototyped: no row; nor for a K&R
        // definition, an unprototyped function type inside a type, or a
        // calling convention C++ does not share.
        let c17 = decls_of(&d, "x.c", XL_C, &["-std=c17", quiet, target]).unwrap();
        let mut want = cxx.clone();
        want.retain(|r| r.key != fnv("f_np"));
        let no_row_tab = c17.iter().find(|r| r.key == fnv("no_row")).unwrap().clone();
        want.push(no_row_tab);
        want.sort_by_key(|r| r.key);
        assert_eq!(c17, want, "{target}");
    }
    // The decided forms, one per rule (target-independent ones).
    let rows = decls_of(&d, "x.c", XL_C, &["-std=c23"]).unwrap();
    for (sym, t) in [
        ("f_q", "FiilE"),
        ("f_td", "FvlE"),
        ("f_c16", "FvtiE"),
        ("f_c8", "FvPhE"),
        ("f_bool", "FvbE"),
        ("f_rec", "FvPRRE"),
        ("f_va", "FvVE"),
        ("f_ne", "FvPFviEE"),
        ("f_ref", "FvPiPKiE"),
        ("f_rs", "FvPcPPcE"),
        ("f_cv", "FvPKQiE"),
        ("f_tu", "FiPRE"),
        ("f_void", "FivE"),
        ("f_np", "FivE"),
        ("a_at", "Ui"),
        ("a_cx", "Cd"),
        ("f_bi", "FDB37_DU9_E"),
        ("f_vec", "FDv4_iDv4_iE"),
        ("f_i128", "FnoE"),
        ("f_vfp", "FvPFiPKczEE"),
        ("f_parr", "FvPA4_iPA_iE"),
        ("f_vla", "FviPiPA_iE"),
        ("f_st", "FvPiPKiE"),
        ("f_nr", "FvvE"),
        ("f_vz", "FvizE"),
        ("v_i", "i"),
        ("v_arr", "A_l"),
    ] {
        assert_eq!(row(&rows, sym).map(|r| r.type_name.as_str()), Some(t), "{sym}");
    }
    assert_eq!(row(&rows, "f_enum").unwrap().ty, row(&cxx_rows(&d), "f_enum").unwrap().ty);
    // Deliberate mismatches: a parameter type, a variable's type.
    let c = decls_of(
        &d,
        "m.c",
        "int mm_f(int);\nint mm_x;\nint mm_u(void) { return mm_f(mm_x); }\n",
        &[],
    )
    .unwrap();
    let cxx = decls_of(
        &d,
        "m.cpp",
        "extern \"C\" { int mm_f(long); long mm_x; int mm_u() { return mm_f(mm_x); } }\n",
        &[],
    )
    .unwrap();
    let (cf, xf) = (row(&c, "mm_f").unwrap(), row(&cxx, "mm_f").unwrap());
    assert_ne!(cf.ty, xf.ty);
    let (cx, xx) = (row(&c, "mm_x").unwrap(), row(&cxx, "mm_x").unwrap());
    assert_ne!((cx.ty, cx.size), (xx.ty, xx.size));
    assert_eq!((cx.size, xx.size), (4, 8));
    assert_eq!(row(&c, "mm_u"), row(&cxx, "mm_u"));
}

/// Mortar's ruling 2 (WP-1565): `int` and `unsigned int` are one token
/// `i` in the canonical C type, at every level, and so is an enumeration
/// whose compatible type is either; every other integer type keeps its
/// own, and so does an enumeration of another width.
#[test]
fn int_signedness_erased() {
    let d = scratch("intsign");
    let c = "enum U { UA }; enum N { NA = -1 };\n\
             int s_f(unsigned);\n\
             unsigned s_g(unsigned *, enum U (*)(unsigned), const unsigned[]);\n\
             enum N s_e(enum U);\n\
             long k_l(unsigned long); int k_il(int); unsigned short k_s(short);\n\
             void *const tab[] = { (void *)s_f, (void *)s_g, (void *)s_e, (void *)k_l,\n\
                                   (void *)k_il, (void *)k_s };\n";
    let cxx = "extern \"C\" {\nint s_f(int);\nint s_g(int *, int (*)(int), const int *);\n\
               int s_e(int);\nlong k_l(long); int k_il(long); short k_s(unsigned short);\n\
               extern void *const tab[];\n\
               void *const tab[] = { (void *)s_f, (void *)s_g, (void *)s_e, (void *)k_l,\n\
                                     (void *)k_il, (void *)k_s };\n}\n";
    let (cr, xr) = (decls_of(&d, "s.c", c, &[]).unwrap(), decls_of(&d, "s.cpp", cxx, &[]).unwrap());
    for (sym, t) in [("s_f", "FiiE"), ("s_g", "FiPiPFiiEPKiE"), ("s_e", "FiiE")] {
        assert_eq!(row(&cr, sym).map(|r| r.type_name.as_str()), Some(t), "{sym}");
        assert_eq!(row(&cr, sym), row(&xr, sym), "{sym}: accepted");
    }
    for (sym, ct, xt) in
        [("k_l", "FlmE", "FllE"), ("k_il", "FiiE", "FilE"), ("k_s", "FtsE", "FstE")]
    {
        let (a, b) = (row(&cr, sym).unwrap(), row(&xr, sym).unwrap());
        assert_eq!((a.type_name.as_str(), b.type_name.as_str()), (ct, xt), "{sym}: kept");
        assert_ne!(a.ty, b.ty);
    }
    // Enumerations of another width keep the type kiln chose.
    let e = "enum B { BA = 1 }; enum W { WA = 0x100000000 };\n\
             enum F : unsigned char { FA }; enum G : unsigned { GA };\n\
             void e_b(enum B); void e_w(enum W); void e_f(enum F); void e_g(enum G);\n\
             void *const tab[] = { (void *)e_b, (void *)e_w, (void *)e_f, (void *)e_g };\n";
    for (args, b) in [(&["-std=c23"][..], "i"), (&["-std=c23", "-fshort-enums"][..], "h")] {
        let r = decls_of(&d, "e.c", e, args).unwrap();
        for (sym, t) in [("e_b", b), ("e_w", "m"), ("e_f", "h"), ("e_g", "i")] {
            let want = format!("Fv{t}E");
            assert_eq!(row(&r, sym).map(|r| r.type_name.as_str()), Some(&*want), "{sym} {args:?}");
        }
    }
    // C++-linkage rows keep the Itanium encoding.
    let r = decls_of(
        &d,
        "i.cpp",
        "int u_f(unsigned); int u() { return u_f(1); }\n",
        &["-fmason-metadata=detail"],
    )
    .unwrap();
    assert_eq!(row(&r, "_Z3u_fj").map(|r| r.type_name.as_str()), Some("FijE"));
}

/// Mortar's ruling 1(a) (WP-1565): no row for a C++-linkage entity named
/// by an asm label; C-linkage asm labels keep theirs, keyed by the label;
/// at most one row per key, none when a unit's declarations of one symbol
/// disagree.
#[test]
fn one_row_per_key() {
    let d = scratch("perkey");
    // glibc's const-correct `<string.h>` overloads in C++.
    let cxx = "extern \"C++\" char *strchr(char *, int) __asm__(\"strchr\");\n\
               extern \"C++\" const char *strchr(const char *, int) __asm__(\"strchr\");\n\
               extern \"C++\" int lbl_v __asm__(\"lbl_v_sym\");\n\
               extern \"C\" int scanf_r(const char *, ...) __asm__(\"__isoc23_scanf_r\");\n\
               int use(char *p) { return !!strchr(p, 1) + !!strchr((const char *)p, 1)\n\
                 + lbl_v + scanf_r(\"\"); }\n";
    let xr = decls_of(&d, "g.cpp", cxx, &["-fmason-metadata=detail"]).unwrap();
    assert!(row(&xr, "strchr").is_none() && row(&xr, "lbl_v_sym").is_none(), "{xr:?}");
    let want = golden(&[("__isoc23_scanf_r", EXTERN_C, 0, "FiPKczE")]);
    assert_eq!(row(&xr, "__isoc23_scanf_r"), want.first());
    // The C side: both keep their rows.
    let c_src = "char *strchr(const char *, int);\n\
             int scanf_r(const char *, ...) __asm__(\"__isoc23_scanf_r\");\n\
             int use(char *p) { return !!strchr(p, 1) + scanf_r(\"\"); }\n";
    let cr = decls_of(&d, "g.c", c_src, &[]).unwrap();
    assert_eq!(
        row(&cr, "strchr").map(|x| (x.flags, x.type_name.as_str())),
        Some((EXTERN_C, "FPcPKciE"))
    );
    assert_eq!(row(&cr, "__isoc23_scanf_r"), want.first());
    // An `extern "C"` declaration in another C++ unit keeps its row too.
    let other = decls_of(
        &d,
        "x.cpp",
        "extern \"C\" char *strchr(const char *, int);\nint use() { return !!strchr(\"\", 1); }\n",
        &[],
    )
    .unwrap();
    assert_eq!(row(&other, "strchr"), row(&cr, "strchr"));
    // Block-scope declarations of one symbol: disagreeing ones give no
    // row, agreeing ones one; a definition and a matching declaration one
    // `DEFINITION` row.
    let blk = "int u1(void) { extern int bx1 __asm__(\"bx\"); extern int bf1(int) __asm__(\"bf\");\n\
               return bx1 + bf1(1); }\n\
             long u2(void) { extern long bx2 __asm__(\"bx\"); extern long bf2(long) __asm__(\"bf\");\n\
               return bx2 + bf2(1); }\n\
             int u3(void) { extern int ax1 __asm__(\"ax\"); return ax1; }\n\
             int u4(void) { extern int ax2 __asm__(\"ax\"); return ax2; }\n\
             int ad __asm__(\"ad_sym\") = 1;\n\
             int u5(void) { extern int ad2 __asm__(\"ad_sym\"); return ad2; }\n\
             extern int rv; extern int rv; int rv = 2;\n\
             int rf(int); int rf(int); int rf(int a) { return a + rv; }\n";
    let br = decls_of(&d, "b.c", blk, &[]).unwrap();
    assert!(row(&br, "bx").is_none() && row(&br, "bf").is_none(), "{br:?}");
    let want = golden(&[
        ("ax", VARIABLE | EXTERN_C, 4, "i"),
        ("ad_sym", DEFINITION | VARIABLE | EXTERN_C, 4, "i"),
        ("rv", DEFINITION | VARIABLE | EXTERN_C, 4, "i"),
        ("rf", DEFINITION | EXTERN_C, 0, "FiiE"),
    ]);
    for w in &want {
        assert_eq!(br.iter().find(|x| x.key == w.key), Some(w));
    }
}

/// Mortar's ruling 4 (WP-1565): only the major bound of a variable's own
/// type is erased, in C and C++ alike; member function types carry their
/// cv- and ref-qualifiers; calls the compiler introduces get no row.
#[test]
fn ruling_4_readings() {
    let d = scratch("ruling4");
    let decl =
        "extern int a2[][3]; extern int a3[][3]; int use(void) { return a2[0][0] + a3[0][0]; }\n";
    let def = "int a2[2][3]; int a3[2][4];\n";
    for (ext, args) in [("c", &[][..]), ("cpp", &["-fmason-metadata=detail"][..])] {
        let r1 = decls_of(&d, &format!("u.{ext}"), decl, args).unwrap();
        let r2 = decls_of(&d, &format!("v.{ext}"), def, args).unwrap();
        let (x, y) = (row(&r1, "a2").unwrap(), row(&r2, "a2").unwrap());
        assert_eq!((x.type_name.as_str(), x.size, y.size), ("A_A3_i", 0, 24), "{ext}");
        assert_eq!(x.ty, y.ty, "{ext}");
        let (x, y) = (row(&r1, "a3").unwrap(), row(&r2, "a3").unwrap());
        assert_eq!((x.type_name.as_str(), y.type_name.as_str()), ("A_A3_i", "A_A4_i"), "{ext}");
    }
    let q = "struct Q { int c() const; int v() volatile; int l() &; int r() &&; int cl() const &; };\n\
             int use(Q &q) { return q.c() + q.v() + q.l() + Q().r() + q.cl(); }\n";
    let r = decls_of(&d, "q.cpp", q, &["-fmason-metadata=detail"]).unwrap();
    for (sym, t) in [
        ("_ZNK1Q1cEv", "KFivE"),
        ("_ZNV1Q1vEv", "VFivE"),
        ("_ZNR1Q1lEv", "FivRE"),
        ("_ZNO1Q1rEv", "FivOE"),
        ("_ZNKR1Q2clEv", "KFivRE"),
    ] {
        assert_eq!(row(&r, sym).map(|r| r.type_name.as_str()), Some(t), "{sym}");
    }
    // A struct copy (`memcpy`) and a zeroing loop (`memset` at -O2): no
    // row unless the source declares the function.
    let b = "struct big { char b[4096]; };\n\
             void cp(struct big *a, const struct big *b) { *a = *b; }\n\
             void z(int *p) { for (int i = 0; i < 1024; i++) p[i] = 0; }\n";
    let with = format!(
        "void *memcpy(void *, const void *, unsigned long);\n{b}\n\
                        void *(*mp)(void *, const void *, unsigned long) = memcpy;\n"
    );
    for o in ["-O0", "-O2"] {
        for (ext, src) in [("c", b), ("cpp", b)] {
            let r = decls_of(&d, &format!("m.{ext}"), src, &[o]).unwrap();
            assert!(row(&r, "memcpy").is_none() && row(&r, "memset").is_none(), "{o} {ext}: {r:?}");
        }
        let r = decls_of(&d, "w.c", &with, &[o]).unwrap();
        assert!(row(&r, "memcpy").is_some() && row(&r, "memset").is_none(), "{o}: {r:?}");
    }
}

fn cxx_rows(d: &Path) -> Vec<Decl> {
    decls_of(d, "x.cpp", XL_CXX, &["-std=c++20"]).unwrap()
}

/// Two C++ units: equal rows for matching declarations; a changed
/// parameter type changes the key, a changed return or variable type the
/// type; `noexcept` differs only in the flag.
#[test]
fn two_cxx_units() {
    let d = scratch("cxx2");
    let a = decls_of(
        &d,
        "a.cpp",
        "int g1(int); int g2(int) noexcept; extern int v1; int r1();\n\
         int use() { return g1(1) + g2(2) + v1 + r1(); }\n",
        &[],
    )
    .unwrap();
    let b = decls_of(
        &d,
        "b.cpp",
        "int g1(int); int g2(int); extern long v1; long r1(); int g1(long);\n\
         int use() { return g1(1) + g2(2) + (int)v1 + (int)r1() + g1(1L); }\n",
        &[],
    )
    .unwrap();
    assert_eq!(row(&a, "_Z2g1i"), row(&b, "_Z2g1i"));
    assert_eq!(row(&a, "_Z3usev"), row(&b, "_Z3usev"));
    let (ga, gb) = (row(&a, "_Z2g2i").unwrap(), row(&b, "_Z2g2i").unwrap());
    assert_eq!((ga.ty, ga.size, ga.flags), (gb.ty, gb.size, gb.flags | NOEXCEPT));
    let (va, vb) = (row(&a, "v1").unwrap(), row(&b, "v1").unwrap());
    assert_ne!(va.ty, vb.ty);
    assert_eq!((va.size, vb.size), (4, 8));
    assert_ne!(row(&a, "_Z2r1v").unwrap().ty, row(&b, "_Z2r1v").unwrap().ty);
    assert!(row(&a, "_Z2g1l").is_none() && row(&b, "_Z2g1l").is_some());
}
