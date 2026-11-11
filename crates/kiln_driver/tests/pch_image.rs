//! WP-1280: the PCH's preprocessor-state record end to end through
//! `kiln++` (spec/driver.md "Precompiled headers", `-fkiln-pch-image`;
//! the record is spec/modules.md §11.13). `-E` and `-dM -E` with the
//! record installed match replaying the header ([cpp.include]/2): its
//! macros and `#undef`s ([cpp.replace]), include guards and
//! `#pragma once`, `push_macro`/`pop_macro`, poisoned identifiers, the
//! `pack` and `clang diagnostic` stacks ([cpp.pragma]) and `__COUNTER__`.
//! The self-check mode compares the record with replay; the fingerprint
//! still rejects a header edited after the PCH was built.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pch-image-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    let o =
        Command::new(kilnpp()).args(args).current_dir(dir).stdin(Stdio::null()).output().unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr,
    }
}

fn ok(dir: &Path, args: &[&str]) -> Out {
    let o = kiln(dir, args);
    assert_eq!(o.code, 0, "kiln++ {args:?}:\n{}", o.stderr);
    o
}

/// Everything after the line `begin_tu` (the consumer's own text).
fn tu_part(s: &str) -> &str {
    s.split_once("begin_tu\n").map_or("", |(_, t)| t)
}

const GUARDED: &str = "#ifndef G_H\n#define G_H\n#define G 7\nint g;\n#endif\n";
const ONCE: &str = "#pragma once\n#define O 8\nint o;\n";
const HEADER: &str = "\
#include \"g.h\"
#include \"o.h\"
#define OBJ 1 + 2
#define FN(a, ...) a(__VA_ARGS__) #a
#undef __GNUC_MINOR__
#define M 1
#pragma push_macro(\"M\")
#undef M
#define M 2
#pragma push_macro(\"U\")
#define U 3
#pragma GCC poison bad
#pragma pack(push, 2)
#pragma clang diagnostic push
#pragma clang diagnostic ignored \"-W#warnings\"
#if __COUNTER__ + __COUNTER__
#endif
";
const TU: &str = "\
begin_tu
#include \"g.h\"
#include \"o.h\"
OBJ FN(f, 1, 2) M U G O __COUNTER__ __GNUC_MINOR__
#pragma pop_macro(\"M\")
#pragma pop_macro(\"U\")
M U
#warning silenced
#pragma clang diagnostic pop
#warning loud
struct S { char c; int i; };
static_assert(sizeof(S) == 2 + sizeof(int));
#pragma pack(pop)
";

/// The same state used by a compile: `#pragma pack(pop)` pops the
/// header's push.
const COMPILE: &str = "\
#include \"g.h\"
#include \"o.h\"
int a = OBJ + M + U + G + O + __COUNTER__;
#pragma pop_macro(\"M\")
int b = M;
#pragma clang diagnostic pop
struct S { char c; int i; };
static_assert(sizeof(S) == 2 + sizeof(int));
#pragma pack(pop)
";

fn setup(name: &str) -> PathBuf {
    let d = scratch(name);
    std::fs::write(d.join("g.h"), GUARDED).unwrap();
    std::fs::write(d.join("o.h"), ONCE).unwrap();
    std::fs::write(d.join("h.h"), HEADER).unwrap();
    std::fs::write(d.join("t.cpp"), TU).unwrap();
    std::fs::write(d.join("c.cpp"), COMPILE).unwrap();
    ok(&d, &["-fkiln-pch-image", "-x", "c++-header", "h.h", "-o", "h.pch"]);
    d
}

/// `-E` and `-dM -E` with the record installed (`load`) match replaying
/// the header, as does every diagnostic of the rest of the TU.
#[test]
fn load_matches_replay() {
    let d = setup("load");
    let replay = ok(&d, &["-include-pch", "h.pch", "-E", "-P", "t.cpp"]);
    let load = ok(&d, &["-fkiln-pch-image=load", "-include-pch", "h.pch", "-E", "-P", "t.cpp"]);
    assert_eq!(tu_part(&replay.stdout), tu_part(&load.stdout));
    assert_eq!(replay.stderr, load.stderr);
    let text = tu_part(&load.stdout);
    // Guarded and once headers are not entered again; `__COUNTER__`
    // continues; the macro stacks pop to the header's pushes.
    assert!(text.starts_with("1 + 2 f(1, 2) \"f\" 2 3 7 8 2 __GNUC_MINOR__\n"), "{text}");
    assert!(text.contains("\n1 U\n"), "{text}");
    assert!(!load.stderr.contains("silenced"), "{}", load.stderr);
    assert!(load.stderr.contains("loud"), "{}", load.stderr);
    let replay = ok(&d, &["-include-pch", "h.pch", "-dM", "-E", "t.cpp"]);
    let load = ok(&d, &["-fkiln-pch-image=load", "-include-pch", "h.pch", "-dM", "-E", "t.cpp"]);
    assert_eq!(replay.stdout, load.stdout);
    assert!(load.stdout.contains("#define OBJ 1 + 2\n"), "{}", load.stdout);
}

/// A poisoned identifier is diagnosed after the record as after the
/// header ([cpp.pragma], GCC `#pragma GCC poison`).
#[test]
fn poison_crosses_the_boundary() {
    let d = setup("poison");
    std::fs::write(d.join("p.cpp"), "begin_tu\nbad\n").unwrap();
    let replay = kiln(&d, &["-include-pch", "h.pch", "-E", "-P", "p.cpp"]);
    let load = kiln(&d, &["-fkiln-pch-image=load", "-include-pch", "h.pch", "-E", "-P", "p.cpp"]);
    assert_eq!(replay.code, 1, "{}", replay.stderr);
    assert_eq!((replay.code, &replay.stderr), (load.code, &load.stderr));
}

/// The self-check (`check`) replays the header and finds the state it
/// reached equal to the record's, in `-E` and in a compile (whose
/// `#pragma pack` comes from replay).
#[test]
fn check_agrees_with_replay() {
    let d = setup("check");
    for (mode, tu) in [("-E", "t.cpp"), ("-fsyntax-only", "c.cpp")] {
        let o = ok(&d, &["-fkiln-pch-image=check", "-include-pch", "h.pch", mode, tu]);
        assert!(!o.stderr.contains("record"), "{mode}: {}", o.stderr);
    }
}

/// The producer's `-include`s are part of the record: a compile replays
/// them before the header, and `load` leaves them out of `-E`.
#[test]
fn producer_includes() {
    let d = setup("includes");
    std::fs::write(d.join("pre.h"), "#define PRE 5\n").unwrap();
    ok(&d, &["-fkiln-pch-image", "-include", "pre.h", "-x", "c++-header", "h.h", "-o", "hp.pch"]);
    std::fs::write(d.join("i.cpp"), "begin_tu\nPRE OBJ\nint x = PRE;\n").unwrap();
    std::fs::write(d.join("c2.cpp"), "int x = PRE + OBJ;\n").unwrap();
    let args = ["-fkiln-pch-image=check", "-include", "pre.h", "-include-pch", "hp.pch"];
    let o = ok(&d, &[&args[..], &["-fsyntax-only", "c2.cpp"]].concat());
    assert_eq!(o.stderr, "");
    let load = ["-fkiln-pch-image=load", "-include", "pre.h", "-include-pch", "hp.pch"];
    let o = ok(&d, &[&load[..], &["-E", "-P", "i.cpp"]].concat());
    assert_eq!(tu_part(&o.stdout), "5 1 + 2\nint x = 5;\n");
}

/// A record that disagrees with replay is reported: here the header saw
/// `-DX` when it was built and the consumer, with the configuration
/// check off, does not define it.
#[test]
fn check_reports_a_mismatch() {
    let d = scratch("mismatch");
    std::fs::write(d.join("x.h"), "#ifdef X\n#define Z 1\n#endif\n").unwrap();
    std::fs::write(d.join("t.cpp"), "int z;\n").unwrap();
    ok(&d, &["-fkiln-pch-image", "-DX", "-x", "c++-header", "x.h", "-o", "x.pch"]);
    let args = ["-fkiln-pch-image=check", "-Xclang", "-fno-validate-pch", "-include-pch", "x.pch"];
    let o = kiln(&d, &[&args[..], &["-fsyntax-only", "t.cpp"]].concat());
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(
        o.stderr.contains(
            "error: the preprocessor-state record of precompiled header 'x.pch' differs from \
             replaying the header: only the record has 'define Z "
        ),
        "{}",
        o.stderr
    );
}

/// A header edited after the PCH was built: `load` still runs the
/// fingerprint check, which rejects it; without the check the record
/// itself notices (spec/modules.md §11.13, the file table's lengths and
/// hashes), here when it reads the changed file's spellings.
#[test]
fn stale_header_rejected() {
    let d = setup("stale");
    std::fs::write(d.join("g.h"), GUARDED.replace("G 7", "G 9")).unwrap();
    let load = ["-fkiln-pch-image=load", "-include-pch", "h.pch", "-E", "t.cpp"];
    let o = kiln(&d, &load);
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(
        o.stderr.contains("g.h' has been modified since the precompiled header"),
        "{}",
        o.stderr
    );
    assert_eq!(o.stdout, "");
    std::fs::write(d.join("s.cpp"), "G\n").unwrap();
    let load = ["-fkiln-pch-image=load", "-include-pch", "h.pch", "-E", "s.cpp"];
    let o = kiln(&d, &[&["-Xclang", "-fno-validate-pch"][..], &load].concat());
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("cannot load the preprocessor-state record"), "{}", o.stderr);
}

/// A PCH built without the record is replayed, with a warning.
#[test]
fn no_record_is_replayed() {
    let d = setup("none");
    ok(&d, &["-x", "c++-header", "h.h", "-o", "plain.pch"]);
    let o = ok(&d, &["-fkiln-pch-image=load", "-include-pch", "plain.pch", "-E", "-P", "t.cpp"]);
    assert!(
        o.stderr.contains(
            "warning: the preprocessor-state record of precompiled header 'plain.pch' is not used \
             (it was built without -fkiln-pch-image); the header is replayed"
        ),
        "{}",
        o.stderr
    );
    assert!(tu_part(&o.stdout).starts_with("1 + 2"), "{}", o.stdout);
    let o = kiln(&d, &["-fkiln-pch-image=bogus", "-E", "t.cpp"]);
    assert_eq!(o.code, 1, "{}", o.stderr);
}
