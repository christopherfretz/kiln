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
