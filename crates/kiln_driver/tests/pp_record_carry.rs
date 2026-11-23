//! The preprocessing record across a PCH snapshot and module imports
//! (spec/pp.md "Preprocessing record", stage 4; WP-2113), end to end
//! through `kiln++`: a PCH built recording gives the consumer the record
//! of the same compile without the PCH, a PCH built without it leaves the
//! record `restored` with a note, a header unit's exports keep the unit's
//! keys, and a named module imported from its declaration image is named
//! by the record ([cpp.include], [cpp.import], [module.import]).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// A fresh scratch directory with `files`.
fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-pp-record-carry-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    for (f, t) in files {
        std::fs::write(d.join(f), t).unwrap();
    }
    d
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(kilnpp())
        .args(args)
        .current_dir(dir)
        .env("XDG_CACHE_HOME", dir.join("cache"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    }
}

fn ok(dir: &Path, args: &[&str]) -> Out {
    let o = run(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    o
}

/// A header with every table: definitions, a function-like macro, a
/// condition reading macros, a query, a paste, push/pop and an include
/// guard.
const H: &str = "\
#ifndef H_H
#define H_H
#define A 1
#define F(x) (x + A)
#if defined(A) && A > 0
int h1 = F(2);
#endif
#if __has_include(<stddef.h>)
#define HAS 1
#endif
#pragma push_macro(\"A\")
#undef A
#define A 2
#pragma pop_macro(\"A\")
#define CAT(a, b) a##b
int CAT(x, y) = A;
#endif
";

/// A main file going on from the header: a guarded `#include` skipped,
/// expansions and reads of the header's macros, an `#undef` of one.
const MAIN: &str = "\
#include \"h.h\"
#if A == 1 && HAS
int t1 = F(3) + CAT(x, y);
#endif
#undef F
#define F 7
int t2 = F;
";

/// `-fsyntax-only -fkiln-dump-pp-record t.cpp` with the PCH (its snapshot
/// loaded and checked to be restored), and the same compile with the
/// header as `-include`.
fn dumps(d: &Path) -> (Out, Out) {
    let snap = ok(
        d,
        &[
            "-include-pch",
            "h.pch",
            "-fkiln-pch-snapshot=load",
            "-v",
            "-fkiln-dump-pp-record",
            "-fsyntax-only",
            "t.cpp",
        ],
    );
    assert!(snap.stderr.contains("snapshot matches; restoring it"), "{}", snap.stderr);
    let plain = ok(d, &["-include", "h.h", "-fkiln-dump-pp-record", "-fsyntax-only", "t.cpp"]);
    (snap, plain)
}

/// A PCH built recording carries the header's record: the consumer's
/// record is the record of the same compile with `-include h.h`, entry
/// for entry, and complete. Built with `-fkiln-dump-pp-record` it is the
/// same.
#[test]
fn pch_snapshot_carries_the_record() {
    let d = tree("on", &[("h.h", H), ("t.cpp", MAIN)]);
    for flag in ["-fkiln-pp-record", "-fkiln-dump-pp-record"] {
        let p = ok(
            &d,
            &[flag, "-fkiln-pch-snapshot=write", "-v", "-x", "c++-header", "h.h", "-o", "h.pch"],
        );
        assert!(p.stderr.contains("snapshot written"), "{}", p.stderr);
        let (snap, plain) = dumps(&d);
        assert!(!snap.stderr.contains("preprocessing record"), "{}", snap.stderr);
        assert_eq!(snap.stdout, plain.stdout, "{flag}");
        assert!(snap.stdout.ends_with("\nrecord complete\n"), "{}", snap.stdout);
        // The header's rows are there: its conditionals, its push/pop,
        // the main file's expansion of its macro and the skipped include.
        for line in [
            "pop h.h:",
            "query h.h:",
            "made h.h:",
            "inc t.cpp:0 include \"h.h\" name t.cpp:9..t.cpp:14 skipped-guard file h.h",
        ] {
            assert!(snap.stdout.contains(line), "{line}:\n{}", snap.stdout);
        }
        assert!(
            snap.stdout.lines().any(|l| l.starts_with("exp t.cpp:") && l.contains(" F def h.h:"))
        );
    }
}

/// A PCH built without the record restores all the same: the record of a
/// recording consumer is `restored`, with a `-v` note saying why; and a
/// consumer that does not record is unchanged by a PCH that has one.
#[test]
fn pch_snapshot_without_the_record() {
    let d = tree("off", &[("h.h", H), ("t.cpp", MAIN)]);
    ok(&d, &["-fkiln-pch-snapshot=write", "-x", "c++-header", "h.h", "-o", "h.pch"]);
    let (snap, plain) = dumps(&d);
    assert!(
        snap.stderr.contains(
            "note: precompiled header 'h.pch': snapshot restored without its preprocessing record"
        ),
        "{}",
        snap.stderr
    );
    assert!(snap.stdout.ends_with("\nrecord incomplete: restored\n"), "{}", snap.stdout);
    assert!(plain.stdout.ends_with("\nrecord complete\n"), "{}", plain.stdout);
    ok(
        &d,
        &[
            "-fkiln-pp-record",
            "-fkiln-pch-snapshot=write",
            "-x",
            "c++-header",
            "h.h",
            "-o",
            "h.pch",
        ],
    );
    let args =
        ["-include-pch", "h.pch", "-fkiln-pch-snapshot=load", "-v", "-fsyntax-only", "t.cpp"];
    let o = ok(&d, &args);
    assert!(o.stderr.contains("snapshot matches; restoring it"), "{}", o.stderr);
    assert_eq!(o.stdout, "");
}

const HU: &str = "#ifndef HU_H\n#define HU_H\n#define HV 7\n#define G(x) x+1\n#endif\n";
const HU_USE: &str = "import \"hu.h\";\nint v = G(HV);\n#if HV == 7\nint w;\n#endif\n";

/// A header unit's exported macros keep the unit's keys: the importer's
/// expansions and reads name the definitions made in the unit, whether
/// the unit is built from source or read from its module file, and the
/// record is complete.
#[test]
fn header_unit_exports_keep_their_keys() {
    let d = tree("hu", &[("hu.h", HU), ("u.cpp", HU_USE)]);
    ok(&d, &["-std=c++20", "-fmodule-header", "hu.h"]);
    let dump = |extra: &[&str]| {
        let mut a = vec!["-std=c++20", "-fsyntax-only", "-fkiln-dump-pp-record"];
        a.extend_from_slice(extra);
        a.push("u.cpp");
        ok(&d, &a).stdout
    };
    for out in [dump(&["-fmodule-file=hu.pcm"]), dump(&[])] {
        assert!(out.ends_with("\nrecord complete\n"), "{out}");
        let exps: Vec<_> = out.lines().filter(|l| l.starts_with("exp u.cpp:")).collect();
        assert_eq!(exps.len(), 3, "{out}");
        assert!(exps[0].starts_with("exp u.cpp:23 G def hu.h:47 "), "{out}");
        assert!(exps[1].starts_with("exp u.cpp:25 HV def hu.h:34 "), "{out}");
        assert!(out.contains(" expanded HV hu.h:34 at u.cpp:34 "), "{out}");
    }
}

const N: &str = "export module N;\n#define NV 4\nexport int n = NV;\nexport struct S { int a; };\n";
const N_USE: &str = "import N;\n#define Z 1\nint v = n + Z;\nS s;\n";

/// A named module imported from its declaration image is not
/// preprocessed here: the record names the image and is `imported`.
/// Replayed from its capsule, the module's rows are recorded as source.
#[test]
fn named_module_image_is_named() {
    let d = tree("image", &[("n.cppm", N), ("u.cpp", N_USE)]);
    ok(&d, &["-std=c++20", "-fkiln-bmi-image", "--precompile", "n.cppm", "-o", "N.pcm"]);
    let args = |image: bool| {
        let mut a = vec!["-std=c++20", "-fsyntax-only", "-v", "-fkiln-dump-pp-record"];
        if image {
            a.push("-fkiln-bmi-image");
        }
        a.extend(["-fmodule-file=N=N.pcm", "u.cpp"]);
        a
    };
    let o = ok(&d, &args(true));
    assert!(o.stderr.contains("imported 'N' from its declaration image"), "{}", o.stderr);
    assert!(o.stdout.contains("\nimage u.cpp:7 N files n.cppm\n"), "{}", o.stdout);
    assert!(o.stdout.ends_with("\nrecord incomplete: imported\n"), "{}", o.stdout);
    let o = ok(&d, &args(false));
    assert!(!o.stdout.contains("\nimage "), "{}", o.stdout);
    assert!(o.stdout.contains("def n.cppm:25 NV file"), "{}", o.stdout);
    assert!(o.stdout.ends_with("\nrecord complete\n"), "{}", o.stdout);
}
