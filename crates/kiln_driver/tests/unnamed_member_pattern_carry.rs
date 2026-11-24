//! An instantiated unnamed member class links to its pattern
//! (spec/ast.md §8.2, WP-3273), and what is built on the link carries:
//! a precompiled header's snapshot restores the same unit
//! ([cpp.include]: a precompiled prefix must not change it), with
//! specializations instantiated after the restore from the snapshot's
//! patterns; a module's declaration image carries the unnamed class as
//! a member of its specialization ([module.import]).
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_ast::Ast;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::DeclId;
use kiln_driver::front_end::{FrontEndOptions, run_front_end};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-unnamed-member-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs `kiln++ args` in `dir`: (exit code, stderr).
fn kiln(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(kilnpp())
        .args(args)
        .current_dir(dir)
        .env("KILN_SNAP_TIMING", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    (o.status.code().unwrap_or(-1), stderr)
}

const HEADER: &str = "#pragma once\n\
template <class T> struct S {\n\
  struct { T x; } m;\n\
  union { T a; int b; };\n\
  struct { struct { T y; } in; } n;\n\
};\n\
template <class T> struct P;\n\
template <class T> struct P<T *> { struct { T z; } m; };\n\
inline S<int> s1;\n\
inline P<int *> p1;\n";

const MAIN: &str = "S<long> s2;\n\
P<char *> p2;\n\
long f() { return s1.m.x + s1.a + s1.n.in.y + s2.m.x + s2.b + p1.m.z + p2.m.z; }\n";

fn unnamed_class(ast: &Ast, d: DeclId) -> bool {
    let n = ast.decls.get(d);
    n.kind() == DeclKind::CXXRecord && n.name().is_empty() && ast.definition(d) == Some(d)
}

/// The instantiated unnamed classes, each with whether its pattern is an
/// unnamed class of the template's own definition.
fn links(ast: &Ast) -> Vec<bool> {
    let in_instantiation = |d: DeclId| {
        let mut c = Some(d);
        while let Some(x) = c.filter(|&x| ast.decls.get(x).kind() == DeclKind::CXXRecord) {
            if ast.spec_info(x).is_some() || ast.member_spec_info(x).is_some() {
                return true;
            }
            c = ast.decls.get(x).ctx();
        }
        false
    };
    ast.decls
        .ids()
        .filter(|&d| unnamed_class(ast, d) && in_instantiation(d))
        .map(|x| {
            ast.spec_info(x).is_none()
                && ast
                    .instantiated_from(x)
                    .is_some_and(|p| unnamed_class(ast, p) && !in_instantiation(p))
        })
        .collect()
}

fn front_end(dir: &Path, extra: &[&str]) -> Vec<bool> {
    let main = dir.join("m.cpp");
    let mut cl: Vec<String> =
        ["-fsyntax-only", "-std=c++26"].iter().map(|&s| s.to_owned()).collect();
    cl.extend(extra.iter().map(|&s| s.to_owned()));
    cl.push(main.to_str().unwrap().to_owned());
    let r = run_front_end("kiln++", &cl, FrontEndOptions::default(), |fe| links(fe.ast)).unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
    r.value.unwrap()
}

/// The links as the unit builds them (`-include`, through the library
/// entry), then the same unit with the header restored from its
/// snapshot (`-include-pch`, which the library entry refuses): the
/// snapshot carries the link tables whole, and the restored compile
/// (S<long> and P<char *> instantiated after the restore, from the
/// snapshot's patterns) gives the module the direct compile gives.
#[test]
fn links_survive_a_snapshot() {
    let dir = scratch("pch");
    std::fs::write(dir.join("p.h"), HEADER).unwrap();
    std::fs::write(dir.join("m.cpp"), MAIN).unwrap();
    let header = dir.join("p.h");
    let direct = front_end(&dir, &["-include", header.to_str().unwrap()]);
    // S<int>, S<long>: 4 each; P<int *>, P<char *>: 1 each.
    assert_eq!(direct, vec![true; 10]);

    let ir = ["-std=c++26", "-S", "-emit-llvm", "-O0"];
    let (code, err) =
        kiln(&dir, &[&ir[..], &["-include", "p.h", "m.cpp", "-o", "direct.ll"]].concat());
    assert_eq!(code, 0, "{err}");
    let (code, err) = kiln(&dir, &["-std=c++26", "-x", "c++-header", "p.h", "-o", "p.pch"]);
    assert_eq!(code, 0, "{err}");
    let (code, err) =
        kiln(&dir, &[&ir[..], &["-include-pch", "p.pch", "m.cpp", "-o", "restored.ll"]].concat());
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("snapshot-timing: restore"), "the snapshot was not restored:\n{err}");
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    assert_eq!(read("restored.ll"), read("direct.ll"));
}

/// A module's declaration image ([module.import]) carries an
/// instantiation's unnamed member class as a member reference (its
/// pattern and enclosing specialization): the importer's class is the
/// one it instantiates again, linked to its pattern. Before the link the
/// image refused a unit whose initializer names the class (`s1{{1}}`: "a
/// class named before its record"). A carried body naming a member of
/// the unnamed class (`u1{{2}}` for the anonymous union's `a`) still
/// keeps a unit from having an image: the image carries members of
/// specializations, not of their nested classes.
#[test]
fn links_survive_a_declaration_image() {
    let dir = scratch("image");
    let unit = "export module um;\n\
                export template <class T> struct S { struct { T x; } m; };\n\
                export template <class T> struct U { union { T a; int b; }; };\n\
                export S<int> s1{{1}};\n\
                export U<int> u1;\n";
    std::fs::write(dir.join("um.cppm"), unit).unwrap();
    std::fs::write(
        dir.join("m.cpp"),
        "import um;\nS<long> s2;\nU<long> u2{{4}};\ndecltype(s1.m) c{3};\nlong f() { return s1.m.x + u1.a + c.x + u2.a; }\n",
    )
    .unwrap();
    let pcm = dir.join("um.pcm");
    let pcm = pcm.to_str().unwrap();
    let (code, err) = kiln(
        &dir,
        &[
            "-v",
            "-std=c++26",
            "-fkiln-bmi-image",
            "--precompile",
            "-x",
            "c++-module",
            "um.cppm",
            "-o",
            pcm,
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("with a declaration image of "), "{err}");
    let file = format!("-fmodule-file=um={pcm}");
    let (code, err) =
        kiln(&dir, &["-v", "-std=c++26", "-fsyntax-only", "-fkiln-bmi-image", &file, "m.cpp"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("imported 'um' from its declaration image"), "{err}");
    // S<int>, S<long>: the member's class; U<int>, U<long>: the union.
    assert_eq!(front_end(&dir, &["-fkiln-bmi-image", &file]), vec![true; 4]);
}
