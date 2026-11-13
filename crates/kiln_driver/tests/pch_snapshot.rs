//! WP-1394: the behaviour of a PCH snapshot consumer (spec/driver.md "PCH
//! snapshot"; EXPERIMENTAL, behind `-fkiln-pch-snapshot=write|load`):
//! the diagnostics policy (clang's: the header's own diagnostics are not
//! shown again), `-verify` consumers, C headers, prefixes that end with
//! open state, the `-MD` list, and chained and module consumers. Each
//! restored compile is compared with the replay of the same PCH
//! ([cpp.include]); where clang's PCH is the reference, clang++-19's
//! output is checked when it is installed.
#![forbid(unsafe_code)]
#![allow(clippy::many_single_char_names, reason = "d(ir), r(eplay), s(napshot), o(utput)")]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kiln_bin(cxx: bool) -> PathBuf {
    let k = PathBuf::from(env!("CARGO_BIN_EXE_kiln"));
    if cxx { k.with_file_name("kiln++") } else { k }
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-pchsnap-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, files: &[(&str, &str)]) {
    for (f, t) in files {
        std::fs::write(d.join(f), t).unwrap();
    }
}

struct Out {
    code: i32,
    stderr: String,
}

fn run_with(prog: &Path, dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(prog)
        .args(args)
        .current_dir(dir)
        .env("KILN_SNAP_TIMING", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "{} {args:?} panicked:\n{stderr}", prog.display());
    Out { code: o.status.code().unwrap_or(-1), stderr }
}

fn kiln(dir: &Path, args: &[&str]) -> Out {
    run_with(&kiln_bin(true), dir, args)
}

/// The stderr without the `-v` banner, the snapshot's timing lines and
/// `-v` notes.
fn diags(o: &Out) -> String {
    let e = o.stderr.as_str();
    let e = e.split_once("End of search list.\n").map_or(e, |(_, r)| r);
    e.lines()
        .filter(|l| {
            !l.starts_with("snapshot-timing: ") && !l.starts_with("note: precompiled header")
        })
        .fold(String::new(), |mut out, l| {
            out.push_str(l);
            out.push('\n');
            out
        })
}

fn restored(o: &Out) -> bool {
    o.stderr.lines().any(|l| l.starts_with("snapshot-timing: restore "))
}

/// Builds `pch` from `header` with `flags`; asserts the snapshot was
/// written.
fn produce(d: &Path, flags: &[&str], lang: &str, header: &str, pch: &str) -> Out {
    let mut a: Vec<&str> = flags.to_vec();
    a.extend(["-fkiln-pch-snapshot=write", "-v", "-x", lang, header, "-o", pch]);
    let o = kiln(d, &a);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stderr.contains(&format!("'{pch}': snapshot written")), "{}", o.stderr);
    o
}

/// The consumer's compile of `main` with `pch`, by replay and by
/// snapshot: (replay, snapshot). `extra` comes after `-include-pch`.
fn both(d: &Path, flags: &[&str], pch: &str, extra: &[&str]) -> (Out, Out) {
    let mut a: Vec<&str> = flags.to_vec();
    a.extend(["-include-pch", pch]);
    a.extend(extra);
    let replay = kiln(d, &a);
    a.extend(["-fkiln-pch-snapshot=load", "-v"]);
    let snap = kiln(d, &a);
    (replay, snap)
}

const DIAG_PREFIX: &str = "#warning \"the prefix warns\"\n\
[[deprecated]] inline int old_api() { return 1; }\n\
inline int uses_old() { return old_api(); }\n\
template <class T> int needs_member(T t) { return t.member; }\n";
const DIAG_MAIN: &str = "int f() { return old_api(); }\n\
int g() { return needs_member(1); }\n";

/// The header's own warnings are shown when the PCH is built, not when it
/// is used, and the summary counts the consumer's only: clang's PCH.
/// clang 19:
/// `clang++-19 -std=c++26 -x c++-header p.h -o p.pch` prints the two
/// prefix warnings ("2 warnings generated."); `clang++-19 -std=c++26
/// -include-pch p.pch -fsyntax-only m.cpp` prints `m.cpp:1:18: warning:
/// 'old_api' is deprecated`, `p.h:4:52: error: member reference base type
/// 'int' is not a structure or union` and "1 warning and 1 error
/// generated.".
#[test]
fn header_diagnostics_are_not_shown_again() {
    let d = scratch("diag");
    write(&d, &[("p.h", DIAG_PREFIX), ("m.cpp", DIAG_MAIN)]);
    let p = produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
    assert!(p.stderr.contains("2 warnings generated."), "{}", p.stderr);
    let (replay, snap) = both(&d, &["-std=c++26"], "p.pch", &["-fsyntax-only", "m.cpp"]);
    assert!(restored(&snap), "{}", snap.stderr);
    assert_eq!((replay.code, snap.code), (1, 1));
    assert!(replay.stderr.contains("3 warnings and 1 error generated."), "{}", replay.stderr);
    let s = diags(&snap);
    assert!(s.ends_with("1 warning and 1 error generated.\n"), "{s}");
    assert!(!s.contains("the prefix warns") && !s.contains("p.h:3:"), "{s}");
    assert!(s.contains("m.cpp:1:18: warning: 'old_api' is deprecated"), "{s}");
    assert!(s.contains("p.h:4:52: error: member reference base type 'int'"), "{s}");
    assert!(snap.stderr.contains("the header reported 2 diagnostic(s)"), "{}", snap.stderr);
    assert!(
        snap.stderr.contains("'p.pch': snapshot restored; the 2 warning(s) and error(s)"),
        "{}",
        snap.stderr
    );
    if have("clang++-19") {
        let c = Path::new("clang++-19");
        let o = run_with(c, &d, &["-std=c++26", "-x", "c++-header", "p.h", "-o", "c.pch"]);
        assert!(o.stderr.contains("2 warnings generated."), "{}", o.stderr);
        let o = run_with(c, &d, &["-std=c++26", "-include-pch", "c.pch", "-fsyntax-only", "m.cpp"]);
        assert!(o.stderr.ends_with("1 warning and 1 error generated.\n"), "{}", o.stderr);
    }
}

/// A prefix with warnings gets a snapshot; with `-Werror` they are errors
/// and there is no PCH at all.
#[test]
fn werror_prefix_has_no_snapshot() {
    let d = scratch("werror");
    write(&d, &[("p.h", "#warning \"w\"\nint x;\n")]);
    produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
    let o = kiln(
        &d,
        &["-std=c++26", "-Werror", "-fkiln-pch-snapshot=write", "-v", "-x", "c++-header", "p.h"],
    );
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("error: \"w\" [-Werror,-W#warnings]"), "{}", o.stderr);
    assert!(!o.stderr.contains("snapshot written"), "{}", o.stderr);
    assert!(!d.join("p.h.pch").exists());
}

/// State that affects later diagnostics is carried: a `#pragma clang
/// diagnostic push` left open with its mappings, a `GCC diagnostic`
/// mapping, and the warnings kiln gives once (an undefined internal
/// function used in the prefix is reported once, at its first use).
/// Same as replay; clang 19's PCH prints the same three warnings.
#[test]
fn diagnostic_state_is_carried() {
    let d = scratch("pragmas");
    write(
        &d,
        &[
            (
                "p.h",
                "static int internal_undefined();\n\
                 inline int pfx() { return internal_undefined(); }\n\
                 #pragma GCC diagnostic warning \"-Wshadow\"\n\
                 #pragma clang diagnostic push\n\
                 #pragma clang diagnostic ignored \"-Wunused-variable\"\n",
            ),
            (
                "m.cpp",
                "int quiet() { int a = 1; return internal_undefined(); }\n\
                 #pragma clang diagnostic pop\n\
                 int loud(int x) { int b = 1; { int x = 2; (void)x; } return 0; }\n",
            ),
        ],
    );
    produce(&d, &["-std=c++26", "-Wall"], "c++-header", "p.h", "p.pch");
    let (replay, snap) = both(&d, &["-std=c++26", "-Wall"], "p.pch", &["-fsyntax-only", "m.cpp"]);
    assert!(restored(&snap), "{}", snap.stderr);
    assert_eq!(diags(&snap), diags(&replay));
    let s = diags(&snap);
    assert!(!s.contains("unused variable 'a'"), "{s}");
    assert!(s.contains("unused variable 'b'"), "{s}");
    assert!(s.contains("declaration shadows a local variable"), "{s}");
    assert_eq!(s.matches("internal linkage but is not defined").count(), 1, "{s}");
    assert!(s.contains("p.h:1:12: warning: function 'internal_undefined'"), "{s}");
}

/// `-verify` with a snapshot (spec/driver.md "PCH snapshot: behaviour"): restored
/// when the header reported nothing and holds no directive; a failed
/// expectation is reported as by replay. Otherwise a silent replay with a
/// `-v` note. clang 19 restores and does not see the header's
/// diagnostics or directives (`-Xclang -verify -include-pch p.pch` with a
/// main file expecting `expected-warning@p.h:1` fails: "expected but not
/// seen").
#[test]
fn verify_consumers() {
    let d = scratch("verify");
    write(
        &d,
        &[
            ("clean.h", "template <class T> int get(T t) { return t.member; }\nint quiet();\n"),
            ("warns.h", "#warning \"pw\"\nint quiet();\n"),
            ("dir.h", "// expected-no-diagnostics\nint quiet();\n"),
            (
                "ok.cpp",
                "int f() { return quiet() + nope; } // expected-error {{undeclared identifier}}\n\
                 int g() { return get(1); } // expected-note {{requested here}}\n\
                 // expected-error@clean.h:1 {{member reference base type}}\n",
            ),
            ("bad.cpp", "int f() { return quiet(); } // expected-error {{nothing}}\n"),
            ("mw.cpp", "int h() { return quiet(); } // expected-warning@warns.h:1 {{pw}}\n"),
            ("md.cpp", "int h() { return quiet(); }\n"),
        ],
    );
    for h in ["clean", "warns", "dir"] {
        produce(&d, &["-std=c++26"], "c++-header", &format!("{h}.h"), &format!("{h}.pch"));
    }
    let verify = |pch: &str, main: &str| {
        both(&d, &["-std=c++26"], pch, &["-Xclang", "-verify", "-fsyntax-only", main])
    };
    let (r, s) = verify("clean.pch", "ok.cpp");
    assert!(restored(&s), "{}", s.stderr);
    assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
    let (r, s) = verify("clean.pch", "bad.cpp");
    assert!(restored(&s), "{}", s.stderr);
    assert_eq!((r.code, s.code), (1, 1));
    assert_eq!(diags(&s), diags(&r));
    let (r, s) = verify("warns.pch", "mw.cpp");
    assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
    assert!(
        s.stderr.contains("snapshot not used (-verify: the header reported 1 diagnostic(s)"),
        "{}",
        s.stderr
    );
    let (r, s) = verify("dir.pch", "md.cpp");
    assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
    assert!(s.stderr.contains("'dir.h' may hold -verify directives"), "{}", s.stderr);
}

/// [cpp.include] A C header gets a snapshot (tentative definitions,
/// K&R definitions, `#pragma pack`): the restored compile's IR and
/// diagnostics are replay's.
#[test]
fn c_header() {
    let d = scratch("c");
    write(
        &d,
        &[
            (
                "p.h",
                "int counter;\nint counter;\nstatic int add(a, b) int a; int b; { return a + b; }\n\
                 _Static_assert(sizeof(int) == 4, \"int\");\n\
                 static inline int twice(int x) { return add(x, x); }\n\
                 #pragma pack(push, 1)\nstruct packed { char c; int i; };\n#pragma pack(pop)\n",
            ),
            (
                "m.c",
                "int vla(int n) { typedef int row[n]; return (int)sizeof(row); }\n\
                 int main(void) { counter++; return twice(counter) + (int)sizeof(struct packed) + vla(2); }\n",
            ),
        ],
    );
    let c = kiln_bin(false);
    let o = run_with(
        &c,
        &d,
        &["-std=c17", "-fkiln-pch-snapshot=write", "-v", "-x", "c-header", "p.h", "-o", "p.pch"],
    );
    assert!(o.stderr.contains("snapshot written"), "{}", o.stderr);
    let r = run_with(
        &c,
        &d,
        &["-std=c17", "-include-pch", "p.pch", "-S", "-emit-llvm", "m.c", "-o", "r.ll"],
    );
    let s = run_with(
        &c,
        &d,
        &[
            "-std=c17",
            "-include-pch",
            "p.pch",
            "-fkiln-pch-snapshot=load",
            "-S",
            "-emit-llvm",
            "m.c",
            "-o",
            "s.ll",
        ],
    );
    assert!(restored(&s), "{}", s.stderr);
    assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
    // The header's K&R warning is the producer's, not shown again.
    assert!(r.stderr.contains("p.h:3:12: warning: a function definition without a prototype"));
    assert_eq!(diags(&s), "");
    let ir = |f: &str| std::fs::read_to_string(d.join(f)).unwrap();
    assert_eq!(ir("s.ll"), ir("r.ll"));
    assert!(ir("s.ll").contains("@counter = dso_local global i32 0"), "{}", ir("s.ll"));
}

/// Prefixes that end with open state: carried exactly (a `push_macro`
/// whose pop is in the main file, `#pragma pack(push)`, `#pragma GCC
/// visibility push`) or refused at the producer with a `-v` note (a
/// global module fragment's `module;`); a header ending inside `#if` is
/// an error, so there is no PCH.
#[test]
fn open_state_at_the_end_of_the_prefix() {
    let d = scratch("open");
    write(
        &d,
        &[
            (
                "p.h",
                "#define LEVEL 1\n#pragma push_macro(\"LEVEL\")\n#undef LEVEL\n#define LEVEL 2\n\
                 #pragma pack(push, 1)\n#pragma GCC visibility push(hidden)\n",
            ),
            (
                "m.cpp",
                "struct S { char c; int i; };\nint hidden_fn() { return LEVEL; }\n\
                 #pragma GCC visibility pop\n#pragma pack(pop)\n#pragma pop_macro(\"LEVEL\")\n\
                 int shown() { return LEVEL * 10 + (int)sizeof(S) + hidden_fn(); }\n",
            ),
            ("gmf.h", "module;\n"),
            ("if.h", "#if 1\nint x;\n"),
        ],
    );
    produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
    let (r, s) = both(&d, &["-std=c++26"], "p.pch", &["-fsyntax-only", "m.cpp"]);
    assert!(restored(&s), "{}", s.stderr);
    assert_eq!((r.code, s.code), (0, 0), "{}", s.stderr);
    assert_eq!(diags(&s), diags(&r));
    let out = |extra: &[&str], f: &str| {
        let mut a =
            vec!["-std=c++26", "-include-pch", "p.pch", "-S", "-emit-llvm", "m.cpp", "-o", f];
        a.extend(extra);
        let o = kiln(&d, &a);
        assert_eq!(o.code, 0, "{}", o.stderr);
        assert_eq!(restored(&o), !extra.is_empty(), "{}", o.stderr);
        std::fs::read_to_string(d.join(f)).unwrap()
    };
    let (rl, sl) = (out(&[], "r.ll"), out(&["-fkiln-pch-snapshot=load"], "s.ll"));
    assert_eq!(sl, rl);
    assert!(sl.contains("define hidden noundef i32 @_Z9hidden_fnv()"), "{sl}");
    assert!(sl.contains("ret i32 2"), "{sl}");
    assert!(sl.contains("define dso_local noundef i32 @_Z5shownv()"), "{sl}");
    let o =
        kiln(&d, &["-std=c++26", "-fkiln-pch-snapshot=write", "-v", "-x", "c++-header", "gmf.h"]);
    assert!(o.stderr.contains("snapshot not written: Parser.mods: not at rest"), "{}", o.stderr);
    let o = kiln(&d, &["-std=c++26", "-fkiln-pch-snapshot=write", "-x", "c++-header", "if.h"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("unterminated conditional directive"), "{}", o.stderr);
    assert!(!d.join("if.h.pch").exists());
}

/// The `-MD` list (spec/driver.md "Dependency output"): with
/// `-include-pch`, the PCH's input files and then the main file, by
/// replay and by snapshot alike, as clang lists them; with `-include`,
/// the main file first. clang 19: `clang++-19 -include-pch p.pch -c
/// m.cpp -MD -MF m.d` writes `m.o: <abs>/p.h <abs>/inner.h m.cpp m.h`;
/// `-include p.h` writes `m.o: m.cpp p.h inner.h m.h`.
#[test]
fn depfile_order() {
    let d = scratch("md");
    write(
        &d,
        &[
            ("p.h", "#include \"inner.h\"\nint pf();\n"),
            ("inner.h", "int inner();\n"),
            ("m.h", "int mh();\n"),
            ("m.cpp", "#include \"m.h\"\nint main() { return pf(); }\n"),
        ],
    );
    produce(&d, &["-std=c++26"], "c++-header", "p.h", "p.pch");
    let dep = |extra: &[&str], f: &str| {
        let mut a = vec!["-std=c++26", "-fsyntax-only", "m.cpp", "-MD", "-MF", f, "-MT", "m.o"];
        a.extend(extra);
        let o = kiln(&d, &a);
        assert_eq!(o.code, 0, "{}", o.stderr);
        std::fs::read_to_string(d.join(f))
            .unwrap()
            .replace("\\\n", "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let replay = dep(&["-include-pch", "p.pch"], "r.d");
    let snap = dep(&["-include-pch", "p.pch", "-fkiln-pch-snapshot=load"], "s.d");
    let plain = dep(&["-include", "p.h"], "p.d");
    assert_eq!(snap, replay);
    assert_eq!(replay, "m.o: p.h inner.h m.cpp m.h");
    assert_eq!(plain, "m.o: m.cpp p.h inner.h m.h");
}

/// Chained and module consumers: a PCH built with another `-include-pch`
/// restores; so does a consumer with `-fmodule-file=` when the producer
/// had it too. A module unit consumer replays: a replay puts the header
/// before its module declaration, which is an error ([module.unit]), and
/// a restore would accept it as clang's PCH does.
#[test]
fn chained_and_module_consumers() {
    let d = scratch("chain");
    write(
        &d,
        &[
            ("a.h", "inline int a1() { return 1; }\n"),
            ("b.h", "inline int b1() { return a1() + 1; }\n"),
            ("m.cpp", "int main() { return a1() + b1() - 3; }\n"),
            ("mod.cppm", "export module mod;\nexport inline int mf() { return 2; }\n"),
            ("u.cpp", "import mod;\nint main() { return a1() + mf() - 3; }\n"),
            ("x.cppm", "export module x;\nexport inline int xf() { return a1(); }\n"),
        ],
    );
    produce(&d, &["-std=c++26"], "c++-header", "a.h", "a.pch");
    produce(&d, &["-std=c++26", "-include-pch", "a.pch"], "c++-header", "b.h", "b.pch");
    let ll = |args: &[&str], f: &str| {
        let mut a = args.to_vec();
        a.extend(["-S", "-emit-llvm", "-o", f]);
        let o = kiln(&d, &a);
        (o, std::fs::read_to_string(d.join(f)).unwrap_or_default())
    };
    let (_, r) = ll(&["-std=c++26", "-include-pch", "b.pch", "m.cpp"], "r.ll");
    let (s, sl) =
        ll(&["-std=c++26", "-include-pch", "b.pch", "-fkiln-pch-snapshot=load", "m.cpp"], "s.ll");
    assert!(restored(&s), "{}", s.stderr);
    assert_eq!(sl, r);
    let o = kiln(&d, &["-std=c++26", "--precompile", "mod.cppm", "-o", "mod.pcm"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    produce(&d, &["-std=c++26", "-fmodule-file=mod=mod.pcm"], "c++-header", "a.h", "am.pch");
    let base = ["-std=c++26", "-fmodule-file=mod=mod.pcm", "-include-pch", "am.pch"];
    let (_, r) = ll(&[&base[..], &["u.cpp"]].concat(), "ru.ll");
    let (s, sl) = ll(&[&base[..], &["-fkiln-pch-snapshot=load", "u.cpp"]].concat(), "su.ll");
    assert!(restored(&s), "{}", s.stderr);
    assert_eq!(sl, r);
    let o = kiln(
        &d,
        &[
            "-std=c++26",
            "-include-pch",
            "a.pch",
            "-fkiln-pch-snapshot=load",
            "-v",
            "-fsyntax-only",
            "x.cppm",
        ],
    );
    assert!(!restored(&o), "{}", o.stderr);
    assert!(o.stderr.contains("'x.cppm' is a module unit"), "{}", o.stderr);
    assert!(o.stderr.contains("module declaration must occur at the start"), "{}", o.stderr);
}
