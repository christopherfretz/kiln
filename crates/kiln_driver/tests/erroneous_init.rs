//! C++26 erroneous values at run time (P2795R5, [basic.indet]; spec/codegen.md
//! §7r): which automatic variables kiln's codegen fills with zero bytes,
//! under which standard and `-ftrivial-auto-var-init` flags. The cases
//! here are the kiln-only ones: `tests/codegen/erroneous_init.cpp`
//! compares the fill itself with clang's `-ftrivial-auto-var-init=zero`,
//! which (unlike kiln) also fills `[[indeterminate]]` variables and classes
//! with a non-trivial default constructor.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

/// Compiles `src` with `-S -emit-llvm -o -` plus `args`; (exit code, IR, stderr).
fn emit(name: &str, src: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let d = std::env::temp_dir().join(format!("kiln-erroneous-init-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("t.cpp"), src).unwrap();
    let o: Output = Command::new(kilnpp())
        .args(["-S", "-emit-llvm", "-O0", "-o", "-"])
        .args(args)
        .arg("t.cpp")
        .current_dir(&d)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&d);
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// The body of `@name`'s definition (up to its closing brace).
fn body<'a>(ir: &'a str, name: &str) -> &'a str {
    let start = ir.find(&format!("@{name}(")).unwrap_or_else(|| panic!("no @{name} in\n{ir}"));
    let rest = &ir[start..];
    &rest[..rest.find("\n}\n").unwrap_or(rest.len())]
}

fn fills(f: &str) -> usize {
    f.matches("!annotation").count()
}

const SRC: &str = "void use(void *);\n\
    struct P { char c; int i; };\n\
    struct C { int v; C() {} };\n\
    extern \"C\" void plain() { int x; int a[16]; P p; use(&x); use(a); use(&p); }\n\
    extern \"C\" void indet() { int x [[indeterminate]]; P p [[indeterminate]]; use(&x); use(&p); }\n\
    extern \"C\" void ctor() { C c; use(&c); }\n\
    extern \"C\" void big() { char b[4097]; char e[4096]; use(b); use(e); }\n";

#[test]
fn cxx26_fills_by_default() {
    let (code, ir, err) = emit("cxx26", SRC, &["-std=c++26"]);
    assert_eq!((code, err.as_str()), (Some(0), ""), "{ir}");
    let p = body(&ir, "plain");
    assert_eq!(fills(p), 3, "{p}");
    assert!(p.contains("store i32 0, ptr %1, align 4, !annotation"), "{p}");
    assert!(p.contains("call void @llvm.memset.p0.i64(ptr align 4 %2, i8 0, i64 64, i1 false), !annotation"), "{p}");
    assert!(ir.contains("!{!\"auto-init\"}"), "{ir}");
    // [dcl.attr.indet]: no erroneous value; a class whose constructor runs
    // is initialized by it, not filled first.
    assert_eq!(fills(body(&ir, "indet")), 0);
    assert_eq!(fills(body(&ir, "ctor")), 0);
    // The 4096-byte limit: `e` (4096) is filled, `b` (4097) is not.
    let b = body(&ir, "big");
    assert_eq!(fills(b), 1, "{b}");
    assert!(b.contains("i8 0, i64 4096, i1 false), !annotation"), "{b}");
}

#[test]
fn cxx23_and_uninitialized_do_not_fill() {
    for (name, args) in [
        ("cxx23", &["-std=c++23"][..]),
        ("cxx17", &["-std=c++17"][..]),
        ("uninit", &["-std=c++26", "-ftrivial-auto-var-init=uninitialized"][..]),
    ] {
        let (code, ir, err) = emit(name, SRC, args);
        assert_eq!(code, Some(0), "{name}: {err}");
        assert!(!ir.contains("auto-init") && !ir.contains("llvm.memset"), "{name}:\n{ir}");
    }
    // The flag also turns the fill on before C++26 (clang's meaning).
    let (_, ir, _) = emit("cxx23zero", SRC, &["-std=c++23", "-ftrivial-auto-var-init=zero"]);
    assert_eq!(fills(body(&ir, "plain")), 3, "{ir}");
}

#[test]
fn max_size_and_remark() {
    let (code, ir, err) = emit("remark", SRC, &["-std=c++26", "-Rauto-var-init"]);
    assert_eq!(code, Some(0));
    assert!(
        err.contains(
            "remark: erroneous value of variable 'b' not written: its size (4097 bytes) exceeds \
             -ftrivial-auto-var-init-max-size=4096 [-Rauto-var-init]"
        ),
        "{err}"
    );
    assert_eq!(err.matches("remark:").count(), 1, "{err}");
    // 0 is no limit, as for clang; a smaller limit leaves `a` (64) alone.
    let (_, ir0, _) = emit("max0", SRC, &["-std=c++26", "-ftrivial-auto-var-init-max-size=0"]);
    assert_eq!(fills(body(&ir0, "big")), 2);
    let (_, ir8, _) = emit("max8", SRC, &["-std=c++26", "-ftrivial-auto-var-init-max-size=8"]);
    assert_eq!(fills(body(&ir8, "plain")), 2, "{ir8}");
    assert_eq!(fills(body(&ir, "plain")), 3);
}

#[test]
fn flag_values() {
    let (code, _, err) = emit("pattern", SRC, &["-ftrivial-auto-var-init=pattern"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("unsupported argument 'pattern' to option '-ftrivial-auto-var-init='"), "{err}");
    let (code, _, err) = emit("badmax", SRC, &["-ftrivial-auto-var-init-max-size=x"]);
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("invalid integral value 'x'"), "{err}");
}
