//! WP-1923: FP pragma states in the declaration image (spec/modules.md
//! §11.11, "Forms carried by WP-1923"; [cfenv.syn], C23 7.6.2). A module
//! interface under `#pragma STDC FENV_ROUND` with inline constructors
//! (mem-initializers, a default member initializer, a block with its own
//! pragma), a class template and a function with default arguments: the
//! importer builds the module from its image (no replay), and the IR it
//! emits for them is the replay's and, mangling aside, that of the same
//! code compiled without modules. Checked by hand against clang++-22.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-bmi-fp-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs kiln++ in `dir`; asserts success; returns stderr.
fn ok(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(kilnpp())
        .args(["-std=c++20", "--target=aarch64-unknown-linux-gnu"])
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    assert_eq!(o.status.code(), Some(0), "kiln++ {args:?}:\n{stderr}");
    stderr
}

/// The interface's declarations, `export` written as `EXPORT`; in a
/// namespace, so that the module's mangling differs from the plain
/// build's only by `W1a`.
const DECLS: &str = "namespace n {\n\
                     EXPORT double gx;\n\
                     #pragma STDC FENV_ROUND FE_UPWARD\n\
                     EXPORT struct S {\n\
                     \x20 double m;\n\
                     \x20 double dm = gx * 3.0;\n\
                     \x20 inline S(double a) : m(a / 3.0) {\n\
                     #pragma STDC FENV_ROUND FE_TOWARDZERO\n\
                     \x20   m = m * a;\n\
                     \x20 }\n\
                     \x20 inline S(int a);\n\
                     };\n\
                     EXPORT template <class T> struct TS {\n\
                     \x20 T m;\n\
                     \x20 T dm = gx - 7.0;\n\
                     \x20 TS(T a) : m(a + 11.0) {}\n\
                     \x20 T get(T x = gx / 13.0) { return x * m; }\n\
                     };\n\
                     EXPORT inline double f(double x = gx * 5.0) { return x; }\n\
                     #pragma STDC FENV_ROUND FE_DOWNWARD\n\
                     inline S::S(int a) : m(a * 0.5) {}\n\
                     }\n";

/// The importer's code, under another rounding mode.
const USE: &str = "using namespace n;\n\
                   #pragma STDC FENV_ROUND FE_TONEAREST\n\
                   double use(double y) {\n\
                   #pragma STDC FENV_ROUND FE_DOWNWARD\n\
                   \x20 S s(y), s2(2);\n\
                   \x20 TS<double> t(y);\n\
                   \x20 return s.m + s2.dm + s.dm + f() + t.get() + t.dm;\n\
                   }\n";

/// The bodies of `ir`'s function definitions, sorted, with module
/// mangling (`W1a`) and attribute group numbers dropped; not the
/// importer's call of the module initializer.
fn bodies(ir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for l in ir.lines() {
        if l.starts_with("define ") && !l.contains("@_GLOBAL__sub_I_") {
            cur = Some(String::new());
        }
        if let Some(c) = cur.as_mut() {
            let l = l.replace("W1a", "");
            let l = match l.find(" #") {
                Some(i)
                    if l[i + 2..].chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '{') =>
                {
                    l[..i].to_owned()
                }
                _ => l,
            };
            c.push_str(&l);
            c.push('\n');
        }
        if l == "}" {
            out.extend(cur.take());
        }
    }
    out.sort();
    out
}

/// The rounding modes of the constrained operations in function `f` of
/// `ir`, in order.
fn rounds<'a>(ir: &'a str, f: &str) -> Vec<&'a str> {
    let head = format!("@{f}(");
    let Some(at) = ir.lines().position(|l| l.starts_with("define ") && l.contains(&head)) else {
        return Vec::new();
    };
    ir.lines()
        .skip(at)
        .take_while(|l| *l != "}")
        .filter_map(|l| l.split("metadata !\"round.").nth(1)?.split('"').next())
        .collect()
}

#[test]
fn image_carries_fp_pragma_states() {
    let d = scratch("fenv");
    let iface = format!("export module a;\n{}", DECLS.replace("EXPORT ", "export "));
    std::fs::write(d.join("a.cppm"), iface).unwrap();
    std::fs::write(d.join("u.cpp"), format!("import a;\n{USE}")).unwrap();
    std::fs::write(d.join("plain.cpp"), format!("{}{USE}", DECLS.replace("EXPORT ", ""))).unwrap();
    let v = ok(&d, &["-v", "-fkiln-bmi-image", "--precompile", "a.cppm", "-o", "a.pcm"]);
    assert!(v.contains("with a declaration image of "), "{v}");

    let ir_args = ["-fmodule-file=a=a.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"];
    let v = ok(&d, &[&["-v", "-fkiln-bmi-image"][..], &ir_args].concat());
    assert!(v.lines().any(|l| l == "note: imported 'a' from its declaration image"), "{v}");
    let image = std::fs::read_to_string(d.join("u.ll")).unwrap();
    ok(&d, &[&["-fno-kiln-bmi-image"][..], &ir_args].concat());
    let replay = std::fs::read_to_string(d.join("u.ll")).unwrap();
    assert!(image == replay, "IR differs between the image and the replay:\n{image}\n{replay}");

    // Each state where clang 22 has it: the constructors'
    // mem-initializers and default member initializers under the states
    // they were written in (upward; the out-of-line one's downward), the
    // block toward zero, the default arguments upward at the call.
    for (f, want) in [
        ("_ZN1nW1a1SC2Ed", &["upward", "upward", "towardzero"][..]),
        ("_ZN1nW1a1SC2Ei", &["downward", "downward", "upward"]),
        ("_ZN1nW1a2TSIdEC2Ed", &["upward", "upward"]),
        ("_ZN1nW1a2TSIdE3getEd", &["upward"]),
        (
            "_Z3used",
            &["downward", "downward", "upward", "downward", "upward", "downward", "downward"],
        ),
    ] {
        assert_eq!(rounds(&image, f), want, "{f}\n{image}");
    }

    ok(&d, &["-S", "-emit-llvm", "plain.cpp", "-o", "plain.ll"]);
    let plain = std::fs::read_to_string(d.join("plain.ll")).unwrap();
    assert_eq!(bodies(&image), bodies(&plain), "the module build's IR is the plain build's");
}
