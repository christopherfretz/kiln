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

mod mason_common;

use std::path::Path;

use mason_common::*;

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

// Record kind 5, the declaration rows (WP-1564; mortar `spec/odr.md`
// O-35, O-33).

const DEFINITION: u16 = 1;
const VARIABLE: u16 = 2;
const EXTERN_C: u16 = 4;
const NOEXCEPT: u16 = 8;

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
    let st_off = u32_at(b, 20) as usize;
    let at = block_at(b, 5)?;
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
            "odr.scheme",
            "producer",
            "source",
            "std",
        ],
        "no header: no abi.stdlib-abi or abi.stdlib-debug; odr.scheme for the kind 2 row of `h` \
         (WP-1581), no odr.layout-scheme (no kind 3 row: no class); never reloc-model"
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
/// A C unit's `abi.exceptions` follows `-fexceptions` (mortar's read-back
/// of 11-17); `abi.rtti` is off in C either way.
#[test]
fn c_exceptions_row() {
    let d = scratch("cexc");
    let rows = |extra: &[&str]| rows_of(&d, "t.c", C, extra);
    for (args, want) in [(&[][..], "off"), (&["-fexceptions"][..], "on")] {
        let r = rows(args);
        assert_eq!(value(&r, "abi.exceptions"), Some(want), "{args:?}");
        assert_eq!(value(&r, "abi.rtti"), Some("off"), "{args:?}");
    }
}

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
        // WP-1581: the C `f_vla` has a variable bound (`b[*][n]`), so it
        // gets no row; the C++ declaration has no variable bound.
        let mut cxx_c = cxx.clone();
        cxx_c.retain(|r| r.key != fnv("f_vla"));
        assert_eq!(decls_of(&d, "x.c", XL_C, &["-std=c23", target]).unwrap(), cxx_c, "{target}");
        // Before C23 `f_np()` is unprototyped: no row; nor for a K&R
        // definition, an unprototyped function type inside a type, or a
        // calling convention C++ does not share.
        let c17 = decls_of(&d, "x.c", XL_C, &["-std=c17", quiet, target]).unwrap();
        let mut want = cxx_c.clone();
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
        ("f_parr", "FvPA_iPA_iE"),
        ("f_st", "FvPiPKiE"),
        ("f_nr", "FvvE"),
        ("f_vz", "FvizE"),
        ("v_i", "i"),
        ("v_arr", "A_l"),
    ] {
        assert_eq!(row(&rows, sym).map(|r| r.type_name.as_str()), Some(t), "{sym}");
    }
    assert_eq!(row(&rows, "f_vla"), None, "a variable bound: no row (WP-1581)");
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

/// Mortar's answer A(i) to WP-1566 (C23 6.2.7: array types are
/// compatible when their elements are and present constant bounds are
/// equal): in the canonical C type a bound is erased on every array type
/// that is not the element type of another array, so `int (*)[]` and
/// `int (*)[4]` agree (and `[3]` against `[4]` behind a pointer is a
/// documented miss), while an inner dimension still differs. C++-linkage
/// rows keep the Itanium form (only a variable's own major bound erased).
#[test]
fn c_array_bounds_erased() {
    let d = scratch("cbounds");
    let unit = |ptr: &str, fun: &str, mat: &str| {
        format!(
            "extern int (*p){ptr};\nvoid f(int (*){fun});\nextern int m[2]{mat};\n\
             typedef int row_t{ptr};\nextern row_t *t;\n\
             int use(void) {{ f(0); return (*p)[0] + m[0][0] + (*t)[0]; }}\n"
        )
    };
    let c_rows = |name: &str, src: &str| decls_of(&d, name, src, &[]).unwrap();
    let ra = c_rows("a.c", &unit("[]", "[4]", "[3]"));
    let rb = c_rows("b.c", &unit("[4]", "[]", "[3]"));
    let re = c_rows("e.c", &unit("[3]", "[3]", "[4]"));
    for sym in ["p", "f", "t", "m", "use"] {
        assert_eq!(row(&ra, sym), row(&rb, sym), "{sym}");
    }
    assert_eq!(row(&ra, "p").unwrap().type_name, "PA_i");
    assert_eq!(row(&ra, "t").unwrap().type_name, "PA_i");
    assert_eq!(row(&ra, "f").unwrap().type_name, "FvPA_iE");
    assert_eq!(row(&ra, "m").unwrap().type_name, "A_A3_i");
    // `[3]` against `[4]` behind a pointer: a miss (mortar's X19).
    assert_eq!(row(&ra, "p"), row(&re, "p"));
    assert_eq!(row(&ra, "f"), row(&re, "f"));
    // An inner dimension still differs.
    assert_ne!(row(&ra, "m").unwrap().ty, row(&re, "m").unwrap().ty);
    assert_eq!(row(&re, "m").unwrap().type_name, "A_A4_i");
    // A C++ unit's `extern "C"` declarations agree with the C units.
    let rx =
        decls_of(&d, "x.cpp", &format!("extern \"C\" {{\n{}}}\n", unit("[4]", "[4]", "[3]")), &[])
            .unwrap();
    for sym in ["p", "f", "t", "m", "use"] {
        assert_eq!(row(&ra, sym), row(&rx, sym), "{sym}");
    }
    // C++ linkage: the Itanium form, bounds behind a pointer kept.
    let ry = decls_of(&d, "y.cpp", "int (*cp)[4];\nint (*cq)[];\n", &["-fmason-metadata=detail"])
        .unwrap();
    assert_eq!(row(&ry, "cp").unwrap().type_name, "PA4_i");
    assert_eq!(row(&ry, "cq").unwrap().type_name, "PA_i");
}

/// WP-1581: a C-linkage function whose type has a variable array bound
/// (a VLA parameter type, `[*]`, a variably modified type inside a
/// function pointer parameter) gets no kind 5 row; a top-level `int
/// a[n]` parameter is adjusted to `int *`, so its bound is not in the
/// type. A C++ unit's `extern "C"` declaration with constant bounds
/// still agrees with C's.
#[test]
fn variable_bounds_have_no_row() {
    let d = scratch("vla");
    let c = "void f1(int n, int b[][n][n]) {}\n\
             void f2(int n, int b[][4][4]) {}\n\
             void f3(int n, void (*cb)(int m, int (*a)[m])) {}\n\
             void f4(int n, int b[][*]);\n\
             void f5(int n, int a[n]) {}\n\
             void use(void) { f4(1, 0); }\n";
    for std in ["-std=c17", "-std=c23"] {
        let rows = decls_of(&d, "v.c", c, &[std]).unwrap();
        for none in ["f1", "f3", "f4"] {
            assert_eq!(row(&rows, none), None, "{none} {std}");
        }
        assert_eq!(row(&rows, "f2").unwrap().type_name, "FviPA_A4_iE");
        assert_eq!(row(&rows, "f5").unwrap().type_name, "FviPiE");
    }
    let x = "extern \"C\" void f2(int n, int b[][4][4]) {}\n";
    let rx = decls_of(&d, "v.cpp", x, &[]).unwrap();
    let rc = decls_of(&d, "v.c", c, &[]).unwrap();
    assert_eq!(row(&rx, "f2"), row(&rc, "f2"));
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

// Record kind 3, the type rows (WP-1566; mortar `spec/odr.md` O-17, O-18).

const T_ENUM: u16 = 1;
const T_UNION: u16 = 2;
const T_POLYMORPHIC: u16 = 4;
const T_HAS_VBASES: u16 = 8;
const T_SYSTEM: u16 = 32;

/// One kind 3 row; `key` is checked to be the hash of `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeRow {
    name: String,
    layout: u64,
    size: u32,
    align: u16,
    flags: u16,
    file: String,
    line: u32,
    /// The kind 4 entry (`None`: `details` is `0xffffffff`).
    details: Option<Vec<Item>>,
}

/// One kind 4 item (O-19 as fixed for WP-1576): `{u32 name; u32 type;
/// u64 value; u32 width; u32 flags}`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    name: String,
    ty: String,
    value: u64,
    width: u32,
    flags: u32,
}

const I_MEMBER: u32 = 0;
const I_BASE: u32 = 1;
const I_VPTR: u32 = 2;
const I_SLOT: u32 = 3;
const I_ENUMERATOR: u32 = 4;
const I_UNDERLYING: u32 = 5;
const I_BITFIELD: u32 = 16;
const I_VIRTUAL: u32 = 32;

/// The kind 4 entry at byte `at` of the kind 4 block's body: `{u32 count;
/// u32 reserved}` then `count` 24-byte items, 8-aligned, inside the block.
fn detail_entry(b: &[u8], at: u32) -> Vec<Item> {
    let blk = block_at(b, 4).expect("a kind 4 block for a row with details");
    assert_eq!(
        (u16_at(b, blk + 2), u32_at(b, blk + 8), u32_at(b, blk + 12)),
        (1, 0, 0),
        "version 1, no rows"
    );
    let body = blk + 16;
    let end = blk + u32_at(b, blk + 4) as usize;
    assert_eq!(at % 8, 0, "entries are 8-aligned from the body start");
    let e = body + at as usize;
    let count = u32_at(b, e) as usize;
    assert_eq!(u32_at(b, e + 4), 0, "reserved");
    assert!(e + 8 + count * 24 <= end, "the entry is inside the block");
    (0..count)
        .map(|i| {
            let o = e + 8 + i * 24;
            let it = Item {
                name: string_at(b, u32_at(b, o)),
                ty: string_at(b, u32_at(b, o + 4)),
                value: u64_at(b, o + 8),
                width: u32_at(b, o + 16),
                flags: u32_at(b, o + 20),
            };
            assert!(
                it.flags & 15 <= I_UNDERLYING && it.flags & !(15 | I_BITFIELD | I_VIRTUAL) == 0
            );
            it
        })
        .collect()
}

/// The kind 3 rows of the unit in `b`, `None` without a kind 3 block,
/// after checking the block and every row (O-17: 56 bytes, `hash` zero,
/// flags in bits 0, 1, 2, 3, 5; sorted by key, then name, one row per
/// name), with each row's kind 4 entry decoded.
fn type_rows(b: &[u8]) -> Option<Vec<TypeRow>> {
    summary_rows(b);
    let st_off = u32_at(b, 20) as usize;
    let at = block_at(b, 3)?;
    assert_eq!(u16_at(b, at + 2), 1, "version 1");
    let (block_size, row_size, rows) =
        (u32_at(b, at + 4) as usize, u32_at(b, at + 8) as usize, u32_at(b, at + 12) as usize);
    assert_eq!(row_size, 56);
    assert!(16 + rows * 56 <= block_size && at + block_size <= st_off);
    let mut keys = Vec::new();
    let out: Vec<TypeRow> = (0..rows)
        .map(|i| {
            let r = at + 16 + i * 56;
            assert!(b[r + 8..r + 24].iter().all(|&x| x == 0), "hash: not hashed");
            let details = u32_at(b, r + 52);
            let t = TypeRow {
                name: string_at(b, u32_at(b, r + 32)),
                layout: u64_at(b, r + 24),
                size: u32_at(b, r + 36),
                align: u16_at(b, r + 40),
                flags: u16_at(b, r + 42),
                file: string_at(b, u32_at(b, r + 44)),
                line: u32_at(b, r + 48),
                details: (details != 0xffff_ffff).then(|| detail_entry(b, details)),
            };
            assert_eq!(u64_at(b, r), fnv(&t.name), "{t:?}");
            let all = T_ENUM | T_UNION | T_POLYMORPHIC | T_HAS_VBASES | T_SYSTEM;
            assert_eq!(t.flags & !all, 0, "{t:?}");
            keys.push((u64_at(b, r), t.name.clone()));
            t
        })
        .collect();
    for w in keys.windows(2) {
        assert!((w[0].0, w[0].1.as_bytes()) < (w[1].0, w[1].1.as_bytes()), "sorted: {w:?}");
    }
    Some(out)
}

/// The kind 3 rows of `src` compiled with `args` (through the IR).
fn types_of(dir: &Path, file: &str, src: &str, args: &[&str]) -> Option<Vec<TypeRow>> {
    std::fs::write(dir.join(file), src).unwrap();
    let mut a = vec!["-fmason-metadata", "-S", "-emit-llvm", "-o", "-"];
    a.extend(args);
    a.push(file);
    let ir = ok(dir, &a);
    type_rows(&ir_bytes(meta_line(&ir).expect("the global")))
}

/// The row of type `name` (its `<type>` encoding), if any.
fn trow<'r>(rows: &'r [TypeRow], name: &str) -> Option<&'r TypeRow> {
    rows.iter().find(|t| t.name == name)
}

/// The per-TU component of a `<type>` encoding, if any (`_GLOBAL__N`, a
/// local name `Z`, `Ut`/`Ul`, clang's `$_N`), as in
/// `crates/kiln_codegen/tests/mason_tu_local.rs`.
fn per_tu_component(enc: &str) -> Option<&'static str> {
    let b = enc.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n: usize = enc[start..i].parse().unwrap();
            let name = &enc[i..(i + n).min(enc.len())];
            if name.starts_with("_GLOBAL__N") {
                return Some("_GLOBAL__N");
            }
            if name.starts_with('$') {
                return Some("$_N");
            }
            i += n;
            continue;
        }
        match b[i] {
            b'Z' => return Some("Z...E"),
            b'U' if matches!(b.get(i + 1), Some(b't' | b'l')) => return Some("Ut/Ul"),
            b'L' if b.get(i + 1) == Some(&b'_') && b.get(i + 2) == Some(&b'Z') => i += 2,
            b'L' if b.get(i + 1).is_some_and(u8::is_ascii_lowercase) => {
                while i < b.len() && b[i] != b'E' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Classes written in the test with fixed-width members, so the
/// target-neutral rows are the same on every target (pointers and the
/// vptr are pinned to x86-64 where they matter).
const TYPES_CXX: &str = r"
namespace ns {
struct Plain { int a; short b; char c; };
union U { int i; float f; };
struct Bits { unsigned a : 3; unsigned b : 5; int : 0; unsigned c : 7; };
struct __attribute__((packed)) Packed { char c; int i; };
struct Anon { union { int i; float f; }; struct { short s; } named; int tail; };
template <class T> struct Box { T t; };
struct Base { virtual ~Base(); virtual int f() = 0; int b; };
struct Derived : Base { int f() override; long long d; };
struct VB : virtual Base { int g; };
struct Holder { Box<int> bi; Plain p; };
}
namespace { struct Impl { int i; }; }
static int local() { struct Local { int l; } l{}; return l.l; }
static auto lam = [] { return 1; };
typedef struct { int t; } TypedefNamed;
struct HasTU { Impl impl; };
int use(ns::Derived &d, ns::VB &v) {
  ns::Plain p{}; ns::U u{}; ns::Bits bt{}; ns::Packed pk{}; ns::Anon an{}; ns::Holder h{};
  ns::Box<Impl> bx{}; ns::Box<decltype(lam)> bl{}; TypedefNamed td{}; HasTU tu{};
  (void)bl;
  return d.f() + v.g + p.a + u.i + (int)bt.a + pk.i + an.i + h.bi.t + bx.t.i + local() + lam()
       + td.t + tu.impl.i;
}
";

#[test]
fn type_rows_golden() {
    let d = scratch("types");
    let rows = types_of(&d, "t.cpp", TYPES_CXX, &["-std=c++20", "--target=x86_64-linux-gnu"])
        .expect("kind 3 at the default level");
    let got: Vec<(&str, u32, u16, u16)> =
        rows.iter().map(|t| (t.name.as_str(), t.size, t.align, t.flags)).collect();
    let mut want = vec![
        ("N2ns5PlainE", 8, 4, 0),
        ("N2ns1UE", 4, 4, T_UNION),
        ("N2ns4BitsE", 8, 4, 0),
        ("N2ns6PackedE", 5, 1, 0),
        ("N2ns4AnonE", 12, 4, 0),
        ("N2ns3BoxIiEE", 4, 4, 0),
        ("N2ns4BaseE", 16, 8, T_POLYMORPHIC),
        ("N2ns7DerivedE", 24, 8, T_POLYMORPHIC),
        ("N2ns2VBE", 32, 8, T_POLYMORPHIC | T_HAS_VBASES),
        ("N2ns6HolderE", 12, 4, 0),
    ];
    want.sort_by_key(|w| (fnv(w.0), w.0));
    assert_eq!(
        got, want,
        "no row: Impl, Box<Impl>, Local, the closure, Box<closure>, the typedef-named class, HasTU (a member of TU-local type)"
    );
    for t in &rows {
        assert_eq!(t.file, "t.cpp", "{t:?}");
        assert_eq!(per_tu_component(&t.name), None);
    }
    assert_eq!(trow(&rows, "N2ns5PlainE").unwrap().line, 3);
    let summary = rows_of(&d, "t.cpp", TYPES_CXX, &["--target=x86_64-linux-gnu"]);
    // WP-1581: the layout serialization is `odr.layout-scheme` (mortar's
    // ruling 3 of 11-17); `odr.scheme` names the kind 2 hash.
    assert!(summary.contains(&("odr.layout-scheme".into(), "kiln-odr-1".into(), 0)));
}

/// O-11's bump rule: the layout hashes of a fixed corpus are checked in,
/// so that a change to the serialization (spec/codegen.md "The layout
/// hash") fails here and comes with a new `odr.layout-scheme` name.
#[test]
fn layout_hash_golden() {
    let d = scratch("golden");
    let rows =
        types_of(&d, "t.cpp", TYPES_CXX, &["-std=c++20", "--target=x86_64-linux-gnu"]).unwrap();
    let got: Vec<(&str, u64)> = rows.iter().map(|t| (t.name.as_str(), t.layout)).collect();
    let want: Vec<(&str, u64)> = LAYOUT_GOLDEN.to_vec();
    assert_eq!(got, want);
}

const LAYOUT_GOLDEN: &[(&str, u64)] = &[
    ("N2ns4BaseE", 0x430e_a296_2b19_2380),
    ("N2ns3BoxIiEE", 0xe5a3_9015_eaa6_57a5),
    ("N2ns4BitsE", 0x2596_b995_af2d_1553),
    ("N2ns5PlainE", 0x7025_3c0d_624a_0453),
    ("N2ns6PackedE", 0x00a9_6b70_c2d2_6707),
    ("N2ns4AnonE", 0x736d_d532_6f5f_1e91),
    ("N2ns1UE", 0x446c_77b8_71da_0761),
    ("N2ns7DerivedE", 0xae1b_3f9e_9d66_24f0),
    ("N2ns2VBE", 0xfa60_0edf_16e3_42fb),
    ("N2ns6HolderE", 0xff8c_da38_b490_dc1f),
];

/// O-18: every layout change the hash must see changes it; the options
/// that change no layout (RTTI, optimization, debug info) do not.
#[test]
fn layout_hash_sees_layout_changes() {
    let d = scratch("changes");
    let base = "struct B1 { int b; }; struct B2 { int c; };\n\
                struct S : B1 { virtual void f(); virtual void g(); int a; char c; unsigned w : 3; };\n";
    let variants = [
        ("member type", base.replace("int a;", "unsigned a;")),
        ("member order", base.replace("int a; char c;", "char c; int a;")),
        ("added base", base.replace("S : B1", "S : B1, B2")),
        ("virtual base", base.replace("S : B1", "S : virtual B1")),
        ("added virtual", base.replace("virtual void g();", "virtual void g(); virtual void h();")),
        (
            "virtual order",
            base.replace(
                "virtual void f(); virtual void g();",
                "virtual void g(); virtual void f();",
            ),
        ),
        ("bit-field width", base.replace("w : 3", "w : 4")),
        ("packed", base.replace("struct S", "struct __attribute__((packed)) S")),
        ("base layout", base.replace("struct B1 { int b; }", "struct B1 { long b; }")),
    ];
    let hash = |src: &str, args: &[&str]| {
        let src = format!("{src}void S::f() {{}}\n");
        let mut a = vec!["--target=x86_64-linux-gnu"];
        a.extend(args);
        let rows = types_of(&d, "t.cpp", &src, &a).unwrap();
        trow(&rows, "1S").map(|t| t.layout).expect("S's row")
    };
    let h = hash(base, &[]);
    for (what, v) in &variants {
        assert_ne!(hash(v, &[]), h, "{what}");
    }
    for args in [&["-fno-rtti"][..], &["-O2"], &["-g"], &["-std=c++17"]] {
        assert_eq!(hash(base, args), h, "{args:?}");
    }
    // An overrider in a derived class changes the derived class's hash only.
    let derived = format!("{base}struct D : S {{ int d; }};\nD dd;\n");
    let over = derived.replace("int d; }", "int d; void g() override; }");
    let rows = |src: &str| {
        types_of(&d, "t.cpp", &format!("{src}void S::f() {{}}\n"), &["--target=x86_64-linux-gnu"])
            .unwrap()
    };
    let (r0, r1) = (rows(&derived), rows(&over));
    assert_eq!(trow(&r0, "1S"), trow(&r1, "1S"));
    assert_ne!(trow(&r0, "1D").unwrap().layout, trow(&r1, "1D").unwrap().layout);
}

/// The N31 shape (two units, each with its own unnamed-namespace `Impl`,
/// a template over it, a local class of a `static` function) writes no
/// row for any of them; the classes the two units share get equal rows.
#[test]
fn two_units_and_tu_local_types() {
    let d = scratch("n31");
    let unit = |n: i32| {
        format!(
            "template <class T> struct Opt {{ T t; bool engaged; }};\n\
             struct Shared {{ int a; Opt<int> o; }};\n\
             namespace {{ struct Impl {{ int v{n}; }}; }}\n\
             static int local() {{ struct L {{ int l = {n}; }} l; return l.l; }}\n\
             int u{n}(Shared &s) {{ Opt<Impl> o{{}}; return s.a + o.t.v{n} + local(); }}\n"
        )
    };
    let a = types_of(&d, "a.cpp", &unit(1), &[]).unwrap();
    let b = types_of(&d, "b.cpp", &unit(2), &[]).unwrap();
    let names = |r: &[TypeRow]| r.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
    let mut want = vec!["6Shared".to_string(), "3OptIiE".to_string()];
    want.sort_by_key(|n| (fnv(n), n.clone()));
    assert_eq!(names(&a), want, "no row: Impl, Opt<Impl>, L");
    assert_eq!(names(&a), names(&b));
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(
            (x.layout, x.size, x.align, x.flags, x.line),
            (y.layout, y.size, y.align, y.flags, y.line)
        );
    }
}

/// Levels and languages: `min` and C write no kind 3; a PCH's classes
/// get the same rows as the same header included.
#[test]
fn type_rows_levels_c_and_pch() {
    let d = scratch("tlevels");
    let src = "struct P { int x; };\nint f(P p) { return p.x; }\n";
    assert_eq!(types_of(&d, "t.cpp", src, &["-fmason-metadata=min"]), None);
    for l in ["detail", "all"] {
        let r = types_of(&d, "t.cpp", src, &[&format!("-fmason-metadata={l}")]).unwrap();
        assert_eq!(r.len(), 1, "{l}");
    }
    assert_eq!(
        types_of(&d, "t.c", "struct P { int x; };\nint f(struct P p) { return p.x; }\n", &[]),
        None
    );
    std::fs::write(d.join("h.h"), "struct InPch { int a; double b; };\n").unwrap();
    ok(&d, &["-x", "c++-header", "h.h", "-o", "h.pch"]);
    let u = "int f(InPch &p) { return p.a; }\nInPch g;\n";
    let with_pch = types_of(&d, "u.cpp", u, &["-include-pch", "h.pch"]).unwrap();
    let with_h = types_of(&d, "u.cpp", u, &["-include", "h.h"]).unwrap();
    assert_eq!(with_pch, with_h);
    assert_eq!(
        with_h.iter().map(|t| (t.name.as_str(), t.file.as_str(), t.line)).collect::<Vec<_>>(),
        [("5InPch", "./h.h", 1)]
    );
}

/// Real library code: two units sharing standard headers write rows with
/// no per-TU component, flagged SYSTEM, and equal rows for every class
/// they share. Skipped for a library whose headers are not installed.
#[test]
fn library_rows_agree_across_units() {
    let d = scratch("library");
    let hdr = "#include <vector>\n#include <string>\n#include <map>\n#include <optional>\n#include <variant>\n";
    let a = format!(
        "{hdr}int a(std::map<std::string, std::vector<int>> &m) {{ std::optional<std::string> o; std::variant<int, std::string> v; return (int)m.size() + (int)o.has_value() + (int)v.index(); }}\n"
    );
    let b = format!(
        "{hdr}std::string b(std::vector<std::string> &v) {{ std::map<std::string, std::vector<int>> m; return v.empty() ? std::string() : v[0] + std::to_string(m.size()); }}\n"
    );
    for lib in ["-stdlib=libstdc++", "-stdlib=libc++"] {
        std::fs::write(d.join("a.cpp"), &a).unwrap();
        if !run(&d, &["-std=c++20", lib, "-fsyntax-only", "a.cpp"]).status.success() {
            eprintln!("skipping {lib}");
            continue;
        }
        let ra = types_of(&d, "a.cpp", &a, &["-std=c++20", lib, "-fmason-metadata=all"]).unwrap();
        let rb = types_of(&d, "b.cpp", &b, &["-std=c++20", lib, "-fmason-metadata=all"]).unwrap();
        assert!(ra.len() > 20 && rb.len() > 20, "{lib}: {} {}", ra.len(), rb.len());
        let mut shared = 0;
        for t in ra.iter().chain(&rb) {
            assert_eq!(per_tu_component(&t.name), None, "{lib}: {t:?}");
            assert!(t.flags & T_SYSTEM != 0, "{lib}: {t:?}");
        }
        for t in &ra {
            if let Some(u) = trow(&rb, &t.name) {
                assert_eq!(t, u, "{lib}");
                shared += 1;
            }
        }
        assert!(shared > 10, "{lib}: {shared}");
    }
}

// Enumeration rows (WP-1576; mortar's answer B2 to WP-1566).

/// Enumerations the test defines: every kind that gets a row, and every
/// kind that does not.
const ENUMS_CXX: &str = r"
namespace ns {
enum Plain { P0, P1, P2 = 7 };
enum class Scoped : unsigned char { a, b = 200 };
enum Fixed : long long { Neg = -2, Big = 0x7fffffffffff };
enum Unused { U0 };
enum Opaque : int;
enum K7 { R, G, B };
inline int isB(int e) { return e == B; }
template <class T> struct Tpl { enum Inner : short { i0, i1 }; Inner in; };
}
namespace { enum Hidden { H }; }
enum { Unnamed = 3 };
int use(ns::Tpl<int> &t, ns::Scoped s) {
  enum Local { L } l = L;
  return ns::P2 + (int)s + (int)ns::Big + t.in + Unnamed + H + l;
}
";

#[test]
fn enumeration_rows() {
    let d = scratch("enums");
    let rows = types_of(&d, "e.cpp", ENUMS_CXX, &["-std=c++20"]).unwrap();
    let mut got: Vec<(&str, u32, u16, u16, u32)> = rows
        .iter()
        .filter(|t| t.flags & T_ENUM != 0)
        .map(|t| (t.name.as_str(), t.size, t.align, t.flags, t.line))
        .collect();
    got.sort_unstable();
    assert_eq!(
        got,
        [
            ("N2ns2K7E", 4, 4, T_ENUM, 8),
            ("N2ns3TplIiE5InnerE", 2, 2, T_ENUM, 10),
            ("N2ns5FixedE", 8, 8, T_ENUM, 5),
            ("N2ns5PlainE", 4, 4, T_ENUM, 3),
            ("N2ns6ScopedE", 1, 1, T_ENUM, 4),
        ],
        "no row: Unused (never referenced), Opaque (no definition), Hidden (unnamed \
         namespace), the unnamed enumeration, Local (a function's)"
    );
    // The class template specialization holding an enumeration gets its
    // own row; its layout hash covers the member's type by name.
    assert!(trow(&rows, "N2ns3TplIiEE").is_some());
    // The layout hashes are pinned (spec/codegen.md "The layout hash",
    // the enumeration form).
    let hashes: Vec<(&str, u64)> = rows
        .iter()
        .filter(|t| t.flags & T_ENUM != 0)
        .map(|t| (t.name.as_str(), t.layout))
        .collect();
    assert_eq!(hashes, ENUM_GOLDEN);
    // C units write no kind 3, enumerations included.
    assert_eq!(types_of(&d, "e.c", "enum E { A };\nint f(void) { return A; }\n", &[]), None);
}

const ENUM_GOLDEN: &[(&str, u64)] = &[
    ("N2ns6ScopedE", 0xa3c0_930a_26ea_bd07),
    ("N2ns5PlainE", 0xb82c_ad72_ebf7_2c97),
    ("N2ns2K7E", 0xa1a7_dff2_4d14_cb65),
    ("N2ns3TplIiE5InnerE", 0x549c_fc8a_602e_f638),
    ("N2ns5FixedE", 0xe8f1_29f4_51ad_0c20),
];

/// Two units disagreeing on an enumeration (an enumerator added under
/// `#ifdef`) or on its underlying type write different layout hashes; the
/// K7 shape (an enumerator used only as a constant in an inline function
/// no unit emits) still writes the row.
#[test]
fn enumeration_rows_two_units() {
    let d = scratch("enums2");
    let src = "enum Color { Red, Green,\n#ifdef EXTRA\n Extra,\n#endif\n Blue };\n\
               inline int isBlue(int c) { return c == Blue; }\n";
    let ua = types_of(&d, "a.cpp", src, &[]).unwrap();
    let ub = types_of(&d, "b.cpp", src, &[]).unwrap();
    let ux = types_of(&d, "x.cpp", src, &["-DEXTRA"]).unwrap();
    let (ra, rb, rx) =
        (trow(&ua, "5Color").unwrap(), trow(&ub, "5Color").unwrap(), trow(&ux, "5Color").unwrap());
    assert_eq!((&ra.name, ra.layout, ra.size, ra.flags), (&rb.name, rb.layout, rb.size, rb.flags));
    assert_ne!(ra.layout, rx.layout);
    assert_eq!((ra.size, ra.flags), (rx.size, rx.flags));
    let fixed = types_of(&d, "f.cpp", &src.replace("enum Color", "enum Color : int"), &[]).unwrap();
    assert_ne!(trow(&fixed, "5Color").unwrap().layout, ra.layout, "fixed underlying type");
    let scoped = "enum class Color { Red, Green, Blue };\nint f() { return (int)Color::Blue; }\n";
    let us = types_of(&d, "s.cpp", scoped, &[]).unwrap();
    assert_ne!(trow(&us, "5Color").unwrap().layout, ra.layout, "scoped");
    // At `detail` the items name the difference: Blue's value.
    let da = types_of(&d, "a.cpp", src, &["-fmason-metadata=detail"]).unwrap();
    let dx = types_of(&d, "x.cpp", src, &["-fmason-metadata=detail", "-DEXTRA"]).unwrap();
    let ia = trow(&da, "5Color").unwrap().details.clone().unwrap();
    let ix = trow(&dx, "5Color").unwrap().details.clone().unwrap();
    let item = |name: &str, ty: &str, value, flags| Item {
        name: name.into(),
        ty: ty.into(),
        value,
        width: 0,
        flags,
    };
    assert_eq!(
        ia,
        [
            item("", "j", 0, I_UNDERLYING),
            item("Red", "", 0, I_ENUMERATOR),
            item("Green", "", 1, I_ENUMERATOR),
            item("Blue", "", 2, I_ENUMERATOR),
        ]
    );
    assert_eq!(ix[3], item("Extra", "", 2, I_ENUMERATOR));
    assert_eq!(rehash_enum(&ia, 0), trow(&da, "5Color").unwrap().layout);
}

// Record kind 4, the layout details (WP-1576; mortar's answer B3).

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&u32::try_from(s.len()).unwrap().to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// The layout hash of an enumeration re-computed from its items and the
/// one fact without an item (`bits`: scoped, fixed).
fn rehash_enum(items: &[Item], bits: u8) -> u64 {
    let mut out = vec![b'E', bits];
    put_str(&mut out, &items[0].ty);
    out.extend_from_slice(&u32::try_from(items.len() - 1).unwrap().to_le_bytes());
    for it in &items[1..] {
        put_str(&mut out, &it.name);
        out.extend_from_slice(&it.value.to_le_bytes());
    }
    fnv_bytes(&out)
}

/// The layout hash of a class without bases, vptr or unnamed member
/// types, re-computed from its row and items and the facts without an
/// item (`attrs`).
fn rehash_plain_class(t: &TypeRow, attrs: u8) -> u64 {
    let items = t.details.as_ref().unwrap();
    let mut out = vec![if t.flags & T_UNION != 0 { b'U' } else { b'C' }];
    out.extend_from_slice(&u64::from(t.size).to_le_bytes());
    out.extend_from_slice(&u32::from(t.align).to_le_bytes());
    out.push(attrs);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(items.len()).unwrap().to_le_bytes());
    for it in items {
        assert_eq!(it.flags & 15, I_MEMBER);
        out.extend_from_slice(&it.value.to_le_bytes());
        let w = if it.flags & I_BITFIELD != 0 { it.width } else { u32::MAX };
        out.extend_from_slice(&w.to_le_bytes());
        out.push(b'T');
        put_str(&mut out, &it.ty);
    }
    out.push(0);
    out.extend_from_slice(&0u32.to_le_bytes());
    fnv_bytes(&out)
}

fn fnv_bytes(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Kind 4 at `detail` and `all` only, built from the walk that feeds the
/// layout hash: the items re-hash to the row's layout hash where every
/// hashed fact has an item, and the class rows (and their hashes) are
/// the same as at `default`.
#[test]
fn layout_details() {
    let d = scratch("details");
    let args = ["-std=c++20", "--target=x86_64-linux-gnu"];
    let dflt = types_of(&d, "t.cpp", TYPES_CXX, &args).unwrap();
    assert!(dflt.iter().all(|t| t.details.is_none()), "no kind 4 at default");
    for level in ["-fmason-metadata=detail", "-fmason-metadata=all"] {
        let mut a = args.to_vec();
        a.push(level);
        let rows = types_of(&d, "t.cpp", TYPES_CXX, &a).unwrap();
        assert!(rows.iter().all(|t| t.details.is_some()), "{level}: an entry per row");
        let strip: Vec<TypeRow> =
            rows.iter().cloned().map(|t| TypeRow { details: None, ..t }).collect();
        assert_eq!(strip, dflt, "{level}: the kind 3 rows are the level's same");
        for (name, attrs) in [
            ("N2ns5PlainE", 0),
            ("N2ns1UE", 0),
            ("N2ns4BitsE", 0),
            ("N2ns6PackedE", 1),
            ("N2ns3BoxIiEE", 0),
            ("N2ns6HolderE", 0),
        ] {
            let t = trow(&rows, name).unwrap();
            assert_eq!(rehash_plain_class(t, attrs), t.layout, "{name}");
        }
    }
    let rows =
        types_of(&d, "t.cpp", TYPES_CXX, &[&args[..], &["-fmason-metadata=detail"]].concat())
            .unwrap();
    let items = |n: &str| trow(&rows, n).unwrap().details.clone().unwrap();
    let it = |name: &str, ty: &str, value, width, flags| Item {
        name: name.into(),
        ty: ty.into(),
        value,
        width,
        flags,
    };
    assert_eq!(
        items("N2ns4BitsE"),
        [
            it("a", "j", 0, 3, I_BITFIELD),
            it("b", "j", 3, 5, I_BITFIELD),
            it("", "i", 32, 0, I_BITFIELD),
            it("c", "j", 32, 7, I_BITFIELD),
        ]
    );
    assert_eq!(
        items("N2ns4BaseE"),
        [
            it("b", "i", 64, 0, I_MEMBER),
            it("", "", 0, 0, I_VPTR),
            it("_ZN2ns4BaseD1Ev", "", 1, 0, I_SLOT),
            it("_ZN2ns4BaseD0Ev", "", 2, 0, I_SLOT),
            it("__cxa_pure_virtual", "", 3, 0, I_SLOT),
        ],
        "slot 0 is offset-to-top (no item kind); RTTI is not hashed"
    );
    assert_eq!(items("N2ns7DerivedE")[0], it("", "N2ns4BaseE", 0, 0, I_BASE));
    let vb = items("N2ns2VBE");
    assert_eq!(vb[0], it("", "N2ns4BaseE", 128, 0, I_BASE | I_VIRTUAL), "the direct base");
    assert_eq!(vb[1], it("", "N2ns4BaseE", 128, 0, I_BASE | I_VIRTUAL), "the virtual bases");
    // An anonymous union and a member of unnamed class type: the
    // member's item (empty type, `width` the number of items that follow
    // and describe it: mortar's O-19 ruling of 11-17), then the nested
    // class's members.
    assert_eq!(
        items("N2ns4AnonE"),
        [
            it("", "", 0, 2, I_MEMBER),
            it("i", "i", 0, 0, I_MEMBER),
            it("f", "f", 0, 0, I_MEMBER),
            it("named", "", 32, 1, I_MEMBER),
            it("s", "s", 0, 0, I_MEMBER),
            it("tail", "i", 64, 0, I_MEMBER),
        ]
    );
    // Two deep (the outer count includes the inner member's item and its
    // items; a class with such a member has a row since WP-1581), and an
    // empty unnamed class (0). Not hashed: the counts are only in kind 4.
    let nest = "struct N { struct { int a; struct { int b; int c; } in; } out; struct {} e; int t; };\n\
                int f() { N n{}; return n.t; }\n";
    let r =
        types_of(&d, "n.cpp", nest, &[&args[..], &["-fmason-metadata=detail"]].concat()).unwrap();
    let n = trow(&r, "1N").unwrap();
    assert_eq!(
        n.details.clone().unwrap(),
        [
            it("out", "", 0, 4, I_MEMBER),
            it("a", "i", 0, 0, I_MEMBER),
            it("in", "", 32, 2, I_MEMBER),
            it("b", "i", 0, 0, I_MEMBER),
            it("c", "i", 32, 0, I_MEMBER),
            it("e", "", 96, 0, I_MEMBER),
            it("t", "i", 128, 0, I_MEMBER),
        ]
    );
    let plain = types_of(&d, "n.cpp", nest, &args).unwrap();
    assert_eq!(trow(&plain, "1N").unwrap().layout, n.layout);
    // Two classes whose hashes differ have a differing item.
    let one = |src: &str| {
        let r = types_of(&d, "c.cpp", src, &["-fmason-metadata=detail"]).unwrap();
        trow(&r, "1S").unwrap().clone()
    };
    let (s1, s2) = (
        one("struct S { int a; char c; }; int f(S &s) { return s.a; }\n"),
        one("struct S { char c; int a; }; int f(S &s) { return s.a; }\n"),
    );
    assert_ne!(s1.layout, s2.layout);
    assert_ne!(s1.details.as_ref().unwrap()[0], s2.details.as_ref().unwrap()[0]);
}
