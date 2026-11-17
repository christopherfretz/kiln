//! WP-1619: `-fmason-metadata-system-unit` (spec/codegen.md §5.5
//! "`SYSTEM`"; spec/driver.md "Codegen flags"), end to end through
//! kiln++: every kind 2 and kind 3 row of the unit is `SYSTEM`, wherever
//! its definition lies, and nothing else in the section changes (the
//! hash, the key, the other flags, the places, the row order, every other
//! block). Diagnostics are not affected. The same on every route that
//! writes the section: IR and object, `-O0` and `-O2`, a PCH built with
//! or without the flag, a module interface unit and its importer.

mod mason_common;

use std::path::Path;

use mason_common::*;

/// Kind 2's `SYSTEM` (bit 8, flags at @30 of a 48-byte row).
const DEF_SYSTEM: u16 = 256;
/// Kind 3's `SYSTEM` (bit 5, flags at @42 of a 56-byte row).
const TYPE_SYSTEM: u16 = 32;

/// One kind 2 or kind 3 row, every field decoded (the string-table
/// offsets as strings).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    kind: u16,
    key: u64,
    hash: u128,
    /// Kind 2: `class`; kind 3: `layout`.
    word: u64,
    /// Kind 2: 0; kind 3: `name`.
    name: String,
    /// Kind 3: `size`, `align`, `details`; kind 2: `module`, 0, `reserved`.
    extra: (u32, u16, u32),
    flags: u16,
    file: String,
    line: u32,
}

impl Row {
    fn system(&self) -> bool {
        self.flags & if self.kind == 2 { DEF_SYSTEM } else { TYPE_SYSTEM } != 0
    }
}

/// The row offsets and flag offset of the unit's kind 2 and kind 3 rows.
fn row_sites(b: &[u8]) -> Vec<(u16, usize)> {
    let mut out = Vec::new();
    for (kind, size) in [(2u16, 48usize), (3, 56)] {
        let Some(at) = block_at(b, kind) else { continue };
        assert_eq!(u32_at(b, at + 8) as usize, size, "kind {kind} row size");
        for i in 0..u32_at(b, at + 12) as usize {
            out.push((kind, at + 16 + i * size));
        }
    }
    out
}

/// Every kind 2 and kind 3 row of unit `b`, in the unit's order.
fn rows(b: &[u8]) -> Vec<Row> {
    summary_rows(b);
    row_sites(b)
        .into_iter()
        .map(|(kind, r)| {
            let hash = u128::from_le_bytes(b[r + 8..r + 24].try_into().unwrap());
            if kind == 2 {
                Row {
                    kind,
                    key: u64_at(b, r),
                    hash,
                    word: u64::from(u16_at(b, r + 28)),
                    name: string_at(b, u32_at(b, r + 24)),
                    extra: (u32_at(b, r + 32), 0, u32_at(b, r + 44)),
                    flags: u16_at(b, r + 30),
                    file: string_at(b, u32_at(b, r + 36)),
                    line: u32_at(b, r + 40),
                }
            } else {
                Row {
                    kind,
                    key: u64_at(b, r),
                    hash,
                    word: u64_at(b, r + 24),
                    name: string_at(b, u32_at(b, r + 32)),
                    extra: (u32_at(b, r + 36), u16_at(b, r + 40), u32_at(b, r + 52)),
                    flags: u16_at(b, r + 42),
                    file: string_at(b, u32_at(b, r + 44)),
                    line: u32_at(b, r + 48),
                }
            }
        })
        .collect()
}

/// Unit `b` with `SYSTEM` set on every kind 2 and kind 3 row: what the
/// flag must produce from the unit compiled without it, byte for byte.
fn all_system(b: &[u8]) -> Vec<u8> {
    let mut out = b.to_vec();
    for (kind, r) in row_sites(b) {
        let (at, bit) = if kind == 2 { (r + 30, DEF_SYSTEM) } else { (r + 42, TYPE_SYSTEM) };
        let f = u16_at(&out, at) | bit;
        out[at..at + 2].copy_from_slice(&f.to_le_bytes());
    }
    out
}

/// The unit kiln++ writes for `args` in `d`: through the IR
/// (`-emit-llvm`), or from the object (`-c`, the `llc` route).
/// `on` is `off` with every `SYSTEM` bit set and one more kind 1 row,
/// `unit.system` = `yes`, flags 0 (WP-1639): every other summary row and
/// every kind 2 and 3 row field by field.
fn system_of(off: &[u8], on: &[u8], what: &str) {
    let mut k1 = summary_rows(off);
    assert!(k1.iter().all(|r| r.0 != "unit.system"), "{what}");
    k1.push(("unit.system".to_owned(), "yes".to_owned(), 0));
    k1.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    assert_eq!(summary_rows(on), k1, "{what}");
    assert_eq!(rows(on), rows(&all_system(off)), "{what}");
}

fn unit(d: &Path, obj: bool, args: &[&str]) -> Vec<u8> {
    let mut a = vec!["-fmason-metadata=detail"];
    a.extend(args);
    if obj {
        a.extend(["-c", "-o", "out.o"]);
        ok(d, &a);
        container(&std::fs::read(d.join("out.o")).unwrap())
    } else {
        a.extend(["-S", "-emit-llvm", "-o", "-"]);
        ir_bytes(meta_line(&ok(d, &a)).expect("the global"))
    }
}

/// A user header found through `-I`: an inline function, a class, an
/// enumeration, a function template (instantiated by the unit).
const USER_H: &str = "#pragma once\n\
    enum class Mode { a, b };\n\
    struct Box { int v; Mode m; virtual int get() const { return v; } };\n\
    inline int twice(int x) { return 2 * x; }\n\
    template <class T> T pick(T a, T b) { return a < b ? b : a; }\n";

const USER_CPP: &str = "#include \"user.h\"\n\
    int use(Mode m) { Box b{}; b.v = twice(3); return b.get() + pick(1, 2) + (m == Mode::a); }\n";

/// The user header's rows: none `SYSTEM` without the flag, all with it;
/// and the section with the flag is the section without it with the
/// `SYSTEM` bits set and the `unit.system` row added ([`system_of`]).
#[test]
fn user_header_rows_and_byte_identity() {
    let d = scratch("sysunit-user");
    std::fs::create_dir_all(d.join("inc")).unwrap();
    std::fs::write(d.join("inc/user.h"), USER_H).unwrap();
    std::fs::write(d.join("u.cpp"), USER_CPP).unwrap();
    for obj in [false, true] {
        let base = ["-std=c++20", "-Iinc", "u.cpp"];
        let off = unit(&d, obj, &base);
        let on = unit(&d, obj, &[&base[..], &["-fmason-metadata-system-unit"]].concat());
        let (r_off, r_on) = (rows(&off), rows(&on));
        // A definition row per entity kind, and type rows for the class
        // and the enumeration.
        let names: Vec<&str> = r_off.iter().filter(|r| r.kind == 3).map(|r| &*r.name).collect();
        assert!(names.contains(&"3Box") && names.contains(&"4Mode"), "{r_off:#?}");
        for sym in ["_Z5twicei", "_Z4pickIiET_S0_S0_", "_ZNK3Box3getEv"] {
            assert!(r_off.iter().any(|r| r.kind == 2 && r.key == fnv(sym)), "{sym}: {r_off:#?}");
        }
        assert!(r_off.iter().all(|r| !r.system()), "{r_off:#?}");
        assert!(r_on.iter().all(Row::system), "{r_on:#?}");
        // Field by field, `SYSTEM` aside.
        assert_eq!(r_on.len(), r_off.len());
        for (a, b) in r_off.iter().zip(&r_on) {
            let bit = if a.kind == 2 { DEF_SYSTEM } else { TYPE_SYSTEM };
            assert_eq!(Row { flags: b.flags & !bit, ..b.clone() }, *a);
        }
        // The whole unit.
        system_of(&off, &on, &format!("obj {obj}"));
        // The negation, last wins; without `-fmason-metadata` nothing.
        let neg = [&base[..], &["-fmason-metadata-system-unit", "-fno-mason-metadata-system-unit"]]
            .concat();
        assert_eq!(unit(&d, obj, &neg), off);
    }
    let ir = ok(
        &d,
        &[
            "-std=c++20",
            "-Iinc",
            "-fmason-metadata-system-unit",
            "-S",
            "-emit-llvm",
            "-o",
            "-",
            "u.cpp",
        ],
    );
    assert_eq!(ir, ok(&d, &["-std=c++20", "-Iinc", "-S", "-emit-llvm", "-o", "-", "u.cpp"]));
}

/// A real system header's rows are `SYSTEM` with and without the flag.
#[test]
fn system_header_rows_in_both_cases() {
    let d = scratch("sysunit-system");
    std::fs::create_dir_all(d.join("inc")).unwrap();
    std::fs::write(d.join("inc/user.h"), USER_H).unwrap();
    let src = "#include <utility>\n#include \"user.h\"\n\
               int sys() { std::pair<int, long> p(1, 2); int a = 1, b = 2; std::swap(a, b); return p.first + a + twice(b); }\n";
    std::fs::write(d.join("s.cpp"), src).unwrap();
    let base = ["-std=c++20", "-Iinc", "s.cpp"];
    let off = rows(&unit(&d, false, &base));
    let on = rows(&unit(&d, false, &[&base[..], &["-fmason-metadata-system-unit"]].concat()));
    let lib = |r: &&Row| !r.file.ends_with("user.h") && !r.file.ends_with("s.cpp");
    let lib_off: Vec<_> = off.iter().filter(lib).collect();
    assert!(lib_off.iter().any(|r| r.kind == 2) && lib_off.iter().any(|r| r.kind == 3), "{off:#?}");
    assert!(lib_off.iter().all(|r| r.system()), "{lib_off:#?}");
    assert!(off.iter().filter(|r| !lib(r)).all(|r| !r.system()), "{off:#?}");
    assert!(on.iter().all(Row::system), "{on:#?}");
}

/// The flag makes no header a system header for diagnostics: a warning
/// in the user header is still given, with the same text.
#[test]
fn diagnostics_unchanged() {
    let d = scratch("sysunit-diag");
    std::fs::create_dir_all(d.join("inc")).unwrap();
    std::fs::write(d.join("inc/w.h"), "inline int w() { int unused = 1; return 0; }\n").unwrap();
    std::fs::write(d.join("w.cpp"), "#include \"w.h\"\nint f() { return w(); }\n").unwrap();
    let err = |flag: &[&str]| {
        let mut a =
            vec!["-fmason-metadata", "-Wunused-variable", "-Iinc", "-c", "w.cpp", "-o", "w.o"];
        a.extend(flag);
        let r = run(&d, &a);
        assert_eq!(r.status.code(), Some(0));
        text(&r.stderr)
    };
    let off = err(&[]);
    assert!(off.contains("w.h:1:22: warning: unused variable 'unused'"), "{off}");
    assert_eq!(err(&["-fmason-metadata-system-unit"]), off);
}

/// The routes: `-O0`/`-O2`, IR and object, a PCH built without the flag
/// used with it and the reverse (the unit's flag decides; the PCH is
/// accepted), a module interface unit and an importer (the module file is
/// the same with and without the flag).
#[test]
fn routes() {
    let d = scratch("sysunit-routes");
    std::fs::create_dir_all(d.join("inc")).unwrap();
    std::fs::write(d.join("inc/user.h"), USER_H).unwrap();
    std::fs::write(d.join("u.cpp"), USER_CPP).unwrap();
    let flag = "-fmason-metadata-system-unit";
    let check = |off: &[u8], on: &[u8], what: &str| {
        assert!(!rows(off).is_empty() && rows(off).iter().all(|r| !r.system()), "{what}");
        system_of(off, on, what);
    };
    for o in ["-O0", "-O2"] {
        for obj in [false, true] {
            let base = ["-std=c++20", o, "-Iinc", "u.cpp"];
            let off = unit(&d, obj, &base);
            check(&off, &unit(&d, obj, &[&base[..], &[flag]].concat()), &format!("{o} {obj}"));
        }
    }
    // PCH: the flag is not part of the PCH's identity.
    let pch = |name: &str, extra: &[&str]| {
        let mut a =
            vec!["-std=c++20", "-fmason-metadata", "-x", "c++-header", "inc/user.h", "-o", name];
        a.extend(extra);
        ok(&d, &a);
    };
    pch("off.pch", &[]);
    pch("on.pch", &[flag]);
    assert_eq!(std::fs::read(d.join("on.pch")).unwrap(), std::fs::read(d.join("off.pch")).unwrap());
    std::fs::write(d.join("p.cpp"), USER_CPP.replace("#include \"user.h\"\n", "")).unwrap();
    let textual = unit(&d, true, &["-std=c++20", "-include", "inc/user.h", "p.cpp"]);
    for p in ["off.pch", "on.pch"] {
        let base = ["-std=c++20", "-include-pch", p, "p.cpp"];
        let off = unit(&d, true, &base);
        assert_eq!(rows(&off), rows(&textual), "{p}");
        check(&off, &unit(&d, true, &[&base[..], &[flag]].concat()), p);
    }
    // A module interface unit and its importer.
    let m = "module;\n#include \"user.h\"\nexport module m;\n\
             export inline int k(int x) { return twice(x) + pick(x, 1); }\n\
             export struct MBox { Box b; int get() const { return b.get(); } };\n";
    std::fs::write(d.join("m.cppm"), m).unwrap();
    std::fs::write(d.join("i.cpp"), "import m;\nint z() { MBox b{}; return k(1) + b.get(); }\n")
        .unwrap();
    for (out, extra) in [("off.pcm", &[][..]), ("on.pcm", &[flag][..])] {
        let mut a =
            vec!["-std=c++20", "-fmason-metadata", "-Iinc", "--precompile", "m.cppm", "-o", out];
        a.extend(extra);
        ok(&d, &a);
    }
    assert_eq!(std::fs::read(d.join("on.pcm")).unwrap(), std::fs::read(d.join("off.pcm")).unwrap());
    let base = ["-std=c++20", "-Iinc", "m.cppm"];
    check(&unit(&d, true, &base), &unit(&d, true, &[&base[..], &[flag]].concat()), "interface");
    for p in ["off.pcm", "on.pcm"] {
        let mf = format!("-fmodule-file=m={p}");
        let base = ["-std=c++20", &mf, "i.cpp"];
        let off = unit(&d, true, &base);
        check(&off, &unit(&d, true, &[&base[..], &[flag]].concat()), &format!("importer {p}"));
    }
}
