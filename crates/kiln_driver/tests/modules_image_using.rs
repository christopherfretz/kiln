//! WP-2025: the declaration image carries the link from a
//! using-declaration to its shadows (spec/modules.md §11.11 kind 21;
//! spec/ast.md §4.2; [namespace.udecl], [enum.udecl]). A module interface
//! with namespace-scope, `using enum` and class-scope using-declarations:
//! the importer that builds the module from its image has the same
//! `using_shadows` and `shadow_using` entries as the one that replays it.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kiln-bmi-using-{}-{name}", std::process::id()));
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

/// The `-fkiln-memory-report` lines of the two maps.
fn shadow_maps(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter(|l| {
            l.starts_with("memory: decls.cold.using_shadows ")
                || l.starts_with("memory: decls.cold.shadow_using ")
        })
        .collect()
}

#[test]
fn image_carries_using_shadows() {
    let d = scratch("udecl");
    std::fs::write(
        d.join("a.cppm"),
        "export module a;\n\
         namespace n {\n\
         export void f();\n\
         export void f(int);\n\
         export enum class E { x, y };\n\
         export struct B { void m(); void m(int); };\n\
         }\n\
         export namespace m {\n\
         using n::f;\n\
         using enum n::E;\n\
         struct D : n::B { using n::B::m; };\n\
         }\n",
    )
    .unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "import a;\nvoid g() { m::f(); m::f(1); m::D d; d.m(); d.m(1); (void)m::x; }\n",
    )
    .unwrap();
    let v = ok(&d, &["-v", "-fkiln-bmi-image", "--precompile", "a.cppm", "-o", "a.pcm"]);
    assert!(v.contains("with a declaration image of "), "{v}");

    let use_args = ["-fkiln-memory-report", "-fmodule-file=a=a.pcm", "-fsyntax-only", "u.cpp"];
    let image = ok(&d, &[&["-v", "-fkiln-bmi-image"][..], &use_args].concat());
    assert!(image.lines().any(|l| l == "note: imported 'a' from its declaration image"), "{image}");
    let replay = ok(&d, &[&["-fno-kiln-bmi-image"][..], &use_args].concat());
    // Three using-declarations (`f`, `using enum`, `m`), two shadows each.
    let want = |m: &str, n: usize| format!("memory: decls.cold.{m} count={n} ");
    let maps = shadow_maps(&replay);
    assert_eq!(maps.len(), 2, "{replay}");
    assert!(maps[0].starts_with(&want("using_shadows", 3)), "{replay}");
    assert!(maps[1].starts_with(&want("shadow_using", 6)), "{replay}");
    assert_eq!(shadow_maps(&image), maps, "the image's maps are the replay's");
}

/// WP-3145, WP-3146: a class-scope shadow a member hides ([namespace.udecl]/14,
/// in either declaration order) and one an implicitly-declared assignment
/// operator hides ([namespace.udecl]/4, declared at the class's `}`) are
/// marked hidden in the importer that builds the module from its image as
/// in the one that replays it, so the calls below are not ambiguous.
#[test]
fn image_keeps_hidden_shadows() {
    let d = scratch("hidden");
    std::fs::write(
        d.join("h.cppm"),
        "export module h;\n\
         export struct B { int f() { return 1; } int f(int) { return 2; } };\n\
         export struct D : B { using B::f; int f() { return 3; } };\n\
         export struct E : B { int f() { return 4; } using B::f; };\n\
         export struct A;\n\
         export struct P {\n\
         P &operator=(const P &) { return *this; }\n\
         P &operator=(P &&) { return *this; }\n\
         P &operator=(const A &) { return *this; }\n\
         P &operator=(int) { return *this; }\n\
         };\n\
         export struct A : P { using P::operator=; };\n",
    )
    .unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "import h;\n\
         int g(D &d, E &e, A &a, A &b) { a = b; a = 1; return d.f() + d.f(0) + e.f() + e.f(0); }\n",
    )
    .unwrap();
    let v = ok(&d, &["-v", "-fkiln-bmi-image", "--precompile", "h.cppm", "-o", "h.pcm"]);
    assert!(v.contains("with a declaration image of "), "{v}");

    let use_args = ["-fkiln-memory-report", "-fmodule-file=h=h.pcm", "-fsyntax-only", "u.cpp"];
    let image = ok(&d, &[&["-v", "-fkiln-bmi-image"][..], &use_args].concat());
    assert!(image.lines().any(|l| l == "note: imported 'h' from its declaration image"), "{image}");
    let replay = ok(&d, &[&["-fno-kiln-bmi-image"][..], &use_args].concat());
    let hidden = |r: &str| -> Option<String> {
        r.lines()
            .find(|l| l.starts_with("memory: decls.cold.hidden_shadow "))
            .map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
    };
    // `B::f()` in `D` and in `E`, `P::operator=(const A&)` in `A`.
    let want = "memory: decls.cold.hidden_shadow count=3";
    assert_eq!(hidden(&replay).as_deref(), Some(want), "{replay}");
    assert_eq!(hidden(&image), hidden(&replay), "the image's marks are the replay's");
}

/// WP-3403: the shadows two using-declarators of one class make for one
/// entity through two bases (each marked hidden by the member declared
/// after them, [namespace.udecl]/14) import from the image as the replay
/// has them. A class template's using-declarations are not in the image
/// (it falls back to replay), so a specialization's marks are pinned by
/// the snapshot test in `kiln_parse/tests/using_hidden_shadow.rs`.
#[test]
fn image_keeps_repeated_shadow_marks() {
    let d = scratch("repeated-hidden");
    std::fs::write(
        d.join("h.cppm"),
        "export module h;\n\
         export struct A { int f() { return 1; } int f(int) { return 2; } };\n\
         export struct B1 : virtual A {};\n\
         export struct B2 : virtual A {};\n\
         export struct G : B1, B2 { using B1::f; using B2::f; int f() { return 5; } };\n",
    )
    .unwrap();
    std::fs::write(
        d.join("u.cpp"),
        "import h;\n\
         int g(G &g) { return g.f() + g.f(0); }\n",
    )
    .unwrap();
    let v = ok(&d, &["-v", "-fkiln-bmi-image", "--precompile", "h.cppm", "-o", "h.pcm"]);
    assert!(v.contains("with a declaration image of "), "{v}");

    let use_args = ["-fkiln-memory-report", "-fmodule-file=h=h.pcm", "-fsyntax-only", "u.cpp"];
    let image = ok(&d, &[&["-v", "-fkiln-bmi-image"][..], &use_args].concat());
    assert!(image.lines().any(|l| l == "note: imported 'h' from its declaration image"), "{image}");
    let replay = ok(&d, &[&["-fno-kiln-bmi-image"][..], &use_args].concat());
    let hidden = |r: &str| -> Option<String> {
        r.lines()
            .find(|l| l.starts_with("memory: decls.cold.hidden_shadow "))
            .map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
    };
    // `A::f()` through each of `G`'s two using-declarators.
    let want = "memory: decls.cold.hidden_shadow count=2";
    assert_eq!(hidden(&replay).as_deref(), Some(want), "{replay}");
    assert_eq!(hidden(&image), hidden(&replay), "the image's marks are the replay's");
}
