//! End-to-end tests of `-verify` (WP-143, spec/driver.md "-verify",
//! spec/diag.md §11): the report text and exit status against
//! clang 19's VerifyDiagnosticConsumer, pinned here and, when
//! `clang++-19` is installed, compared live.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

const CLANG: &str = "clang++-19";

const H_H: &str = r#"int hh = q1;
#define BAD undeclared_m
int hh2 = q2; // expected-error {{undeclared identifier 'q2'}}
"#;

const F1: &str = r#"#include "h.h" // expected-error@h.h:1 {{undeclared identifier 'q1'}}
int a = BAD; // expected-error {{undeclared_m}}
int b; // #bmark
int b; // expected-error {{redefinition}}
// expected-note@#bmark {{previous definition}}
// expected-error@#nomark {{x}}
template<class T> void t(T x) { x.foo(); } // expected-error-re {{member reference base type '{{.*}}' is not a {{struct|class}}ure or union}}
void u() { t(1); } // expected-note {{requested here}}
/* expected-warning@+1 2 {{unused}} */
void w() { int x1; }
#line 100
int c = zz; // expected-error {{zz}}
// expected-error@-1 {{bogus}}
"#;

const F2: &str = r#"void g(int);
void g(double);
void h() { g("x"); } // foo-error {{no matching function}}
// bar-note@-3 {{candidate}} bar-note@-2 {{candidate function not viable}}
#warning hello // foo-warning {{hello}}
"#;

const F3: &str = r#"int x = y; // expected-error {{undeclared}} expected-note {{nothing}}
struct S { int m; };
int z = S().n; // expected-error-re {{no member named '{{[a-z]}}' in '{{(struct )?}}S'}}
// expected-warning@*:* {{anywhere}}
// expected-error@* {{any line}}
"#;

const MATCHED: &str = r#"int x = y; // expected-error {{use of undeclared identifier 'y'}}
#warning w
// expected-warning@-1 {{w}}
static_assert(false); // expected-error {{static assertion failed}}
"#;

/// A fatal error stops the parse; the directives after it are still read.
const FATAL: &str = r#"#include "nope.h" // expected-error {{not found}}
int a = q; // expected-error {{undeclared}}
"#;

/// The error limit stops the diagnostics, not the comment scan.
const LIMIT: &str = r#"int v0 = q0; // expected-error {{undeclared}}
int v1 = q1; // expected-error {{undeclared}}
int v2 = q2; // expected-error {{undeclared}}
int v3 = q3; // expected-error {{undeclared}}
"#;

fn setup(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kiln-verify-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in [
        ("h.h", H_H),
        ("f1.cpp", F1),
        ("f2.cpp", F2),
        ("f3.cpp", F3),
        ("m.cpp", MATCHED),
        ("fat.cpp", FATAL),
        ("lim.cpp", LIMIT),
    ] {
        std::fs::write(dir.join(name), text).unwrap();
    }
    dir
}

fn run(exe: &str, dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(exe).current_dir(dir).args(args).output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn kiln(dir: &Path, args: &[&str]) -> (i32, String) {
    let mut all = vec!["-std=c++26", "-fsyntax-only"];
    all.extend_from_slice(args);
    run(env!("CARGO_BIN_EXE_kiln"), dir, &all)
}

/// spec/diag.md §11.5-11.6: `@file:line` through an `#include`, markers
/// (and an undefined one), `#line`, `-re`, counts; the report groups by
/// level and kind, prints `(directive at ...)` for remote directives and
/// ends with the problem count; exit status 1.
#[test]
fn report_matches_clang_19() {
    let dir = setup("report");
    let (code, err) = kiln(&dir, &["-verify", "f1.cpp"]);
    let want = "\
error: 'expected-error' diagnostics expected but not seen: \n  File f1.cpp Line 100 (directive at f1.cpp:101): bogus
error: 'expected-error' diagnostics seen but not expected: \n  File f1.cpp Line 6: use of undefined marker '#nomark'
error: 'expected-warning' diagnostics expected but not seen: \n  File f1.cpp Line 10 (directive at f1.cpp:9): unused
  File f1.cpp Line 10 (directive at f1.cpp:9): unused
4 errors generated.
";
    assert_eq!((code, err.as_str()), (1, want));
    std::fs::remove_dir_all(&dir).ok();
}

/// spec/diag.md §11.5: every diagnostic matched, errors included, is
/// silent success (exit 0).
#[test]
fn all_matched_is_success() {
    let dir = setup("matched");
    assert_eq!(kiln(&dir, &["-verify", "m.cpp"]), (0, String::new()));
    assert_eq!(kiln(&dir, &["-Xclang", "-verify", "m.cpp"]), (0, String::new()));
    let (code, err) = kiln(&dir, &["m.cpp"]);
    assert_eq!(code, 1, "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

/// spec/driver.md "-verify": after a fatal error the preprocessor still
/// lexes to the end of the file, so a later directive is "expected but
/// not seen" (its diagnostic was suppressed), as in clang.
#[test]
fn directives_after_fatal_error_are_read() {
    let dir = setup("fatal");
    let want = "\
error: 'expected-error' diagnostics expected but not seen: \n  File fat.cpp Line 2: undeclared
1 error generated.
";
    assert_eq!(kiln(&dir, &["-verify", "fat.cpp"]), (1, want.to_string()));
    let (code, err) = kiln(&dir, &["-verify", "-ferror-limit=2", "lim.cpp"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("File lim.cpp Line 4: undeclared"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

/// spec/driver.md "-verify": verify needs `-fsyntax-only` for now.
#[test]
fn verify_requires_syntax_only() {
    let dir = setup("action");
    let (code, err) = run(env!("CARGO_BIN_EXE_kiln"), &dir, &["-verify", "m.cpp"]);
    assert_eq!(code, 1);
    assert!(err.contains("-verify without -fsyntax-only"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

/// spec/diag.md §11: the same stderr and exit status as
/// `clang++-19 -fsyntax-only -Xclang ...` on every case (skipped when
/// clang is absent).
#[test]
fn same_as_clang_19() {
    if Command::new(CLANG).arg("--version").output().is_err() {
        eprintln!("skipping: {CLANG} not found");
        return;
    }
    let dir = setup("clang");
    let cases: &[&[&str]] = &[
        &["-verify", "f1.cpp"],
        &["-verify=foo,bar", "f2.cpp"],
        &["-verify=foo", "-verify=bar", "-verify-ignore-unexpected=note", "f2.cpp"],
        &["-verify", "f3.cpp"],
        &["-verify", "-verify-ignore-unexpected", "f3.cpp"],
        &["-verify", "m.cpp"],
        &["-verify", "fat.cpp"],
        &["-verify", "-ferror-limit=2", "lim.cpp"],
    ];
    for case in cases {
        let mut c = vec!["-std=c++2c", "-fsyntax-only"];
        for a in *case {
            if a.starts_with("-verify") {
                c.push("-Xclang");
            }
            c.push(a);
        }
        assert_eq!(kiln(&dir, case), run(CLANG, &dir, &c), "{case:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
