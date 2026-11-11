//! WP-1278: attributes in the declaration image (spec/modules.md §11.11
//! "Attributes"). An importer that builds a module from its image sees
//! the attributes the replayed source has: codegen visibility, `abi_tag`
//! mangling, `exclude_from_explicit_instantiation`, and the nodiscard and
//! deprecated warnings at the use site with their notes in the header,
//! and the nonnull indices the sanitizer checks.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-bmi-attrs-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs kiln++ in `dir`: exit code and stderr.
fn kiln(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(kilnpp())
        .arg("-std=c++20")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    (o.status.code().unwrap_or(-1), stderr)
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let (code, stderr) = kiln(dir, args);
    assert_eq!(code, 0, "kiln++ {args:?}:\n{stderr}");
    stderr
}

/// The header #included in the purview of `a` (AM).
const AH: &str = "#define HIDE __attribute__((visibility(\"hidden\")))\n\
                  namespace aa {\n\
                  [[nodiscard]] inline int must(int x) { return x + 1; }\n\
                  [[deprecated(\"use must\")]] inline int old(int x) { return x; }\n\
                  HIDE int hidden_fn(int x);\n\
                  __attribute__((visibility(\"default\"))) int shown_fn(int x);\n\
                  [[gnu::abi_tag(\"v1\")]] int tagged(int x);\n\
                  struct [[gnu::abi_tag(\"t2\")]] Tagged { int v; };\n\
                  Tagged make_tagged(int x);\n\
                  struct [[nodiscard]] Res { int v; };\n\
                  Res make_res();\n\
                  __attribute__((nonnull(2))) int second(int *p, int *q);\n\
                  __attribute__((nonnull)) int every(int *p, int *q);\n\
                  template <class T> struct B {\n\
                    __attribute__((exclude_from_explicit_instantiation)) int ex() { return 1; }\n\
                    int in() { return 2; }\n\
                  };\n\
                  }\n";
const AM: &str = "export module a;\nexport {\n#include \"a.h\"\n}\n";

/// Uses of each attributed declaration: two nodiscard warnings, one
/// deprecated warning, and calls whose IR shows visibility, mangling and
/// explicit-instantiation linkage.
const AUSE: &str = "import a;\n\
                    extern template struct aa::B<int>;\n\
                    int use() {\n\
                    \x20 aa::must(1);\n\
                    \x20 aa::make_res();\n\
                    \x20 int r = aa::old(2);\n\
                    \x20 aa::B<int> b;\n\
                    \x20 return r + aa::hidden_fn(1) + aa::shown_fn(2) + aa::tagged(3)\n\
                    \x20      + aa::make_tagged(4).v + b.ex() + b.in()\n\
                    \x20      + aa::second(&r, &r) + aa::every(&r, &r);\n\
                    }\n";

/// Whether a `-v` compile of `args` built `a` from its image.
fn from_image(d: &Path, args: &[&str]) -> bool {
    let (_, stderr) = kiln(d, &[&["-v", "-fkiln-bmi-image"], args].concat());
    stderr.lines().any(|l| l == "note: imported 'a' from its declaration image")
}

#[test]
fn image_attributes_behave_as_replayed() {
    let d = scratch("attrs");
    std::fs::write(d.join("a.h"), AH).unwrap();
    std::fs::write(d.join("a.cppm"), AM).unwrap();
    std::fs::write(d.join("u.cpp"), AUSE).unwrap();
    let v = ok(&d, &["-v", "-fkiln-bmi-image", "--precompile", "a.cppm", "-o", "a.pcm"]);
    assert!(v.contains("with a declaration image of "), "{v}");
    let ir_args = ["-fmodule-file=a=a.pcm", "-S", "-emit-llvm", "u.cpp", "-o", "u.ll"];
    assert!(from_image(&d, &ir_args), "the image is used");

    // The same warnings (with notes in the header) and IR either way, and
    // both are clang's (checked by hand against clang++-22).
    let mut warns = Vec::new();
    let mut irs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        warns.push(ok(&d, &[&[flag][..], &ir_args].concat()));
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    let w = &warns[0];
    for want in [
        "u.cpp:4:3: warning: ignoring return value of function declared with 'nodiscard' attribute",
        "u.cpp:5:3: warning: ignoring return value of type 'Res' declared with 'nodiscard' attribute",
        "u.cpp:6:15: warning: 'old' is deprecated: use must",
        "./a.h:4:3: note: 'old' has been explicitly marked deprecated here",
    ] {
        assert!(w.contains(want), "{want}\n{w}");
    }
    assert_eq!(warns[0], warns[1], "image and replay warn alike");
    let ir = &irs[0];
    for want in [
        "declare hidden noundef i32 @_ZN2aaW1a9hidden_fnEi(",
        "declare noundef i32 @_ZN2aaW1a8shown_fnEi(",
        "declare noundef i32 @_ZN2aaW1a6taggedB2v1Ei(",
        "declare i32 @_ZN2aaW1a11make_taggedB2t2Ei(",
        "define linkonce_odr dso_local noundef i32 @_ZN2aaW1a1BIiE2exEv(",
        "declare noundef i32 @_ZN2aaW1a1BIiE2inEv(",
    ] {
        assert!(ir.contains(want), "{want}\n{ir}");
    }
    assert!(irs[0] == irs[1], "IR differs between the image and the replay");

    // The nonnull indices: -fsanitize=nonnull-attribute checks the second
    // argument of `second` and both of `every`, either way.
    let san = [&ir_args[..], &["-fsanitize=nonnull-attribute"]].concat();
    assert!(from_image(&d, &san), "the image is used");
    let mut irs = Vec::new();
    for flag in ["-fkiln-bmi-image", "-fno-kiln-bmi-image"] {
        ok(&d, &[&[flag][..], &san].concat());
        irs.push(std::fs::read_to_string(d.join("u.ll")).unwrap());
    }
    let checks = irs[0].matches("call void @__ubsan_handle_nonnull_arg").count();
    assert_eq!(checks, 3, "{}", irs[0]);
    assert!(irs[0] == irs[1], "sanitized IR differs between the image and the replay");
}
