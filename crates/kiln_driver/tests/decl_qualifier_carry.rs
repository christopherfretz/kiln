//! Declaration written nested-name-specifiers (`Ast::decl_qualifier`,
//! spec/ast.md §4.2, WP-3690) carried across PCH snapshots and module
//! declaration images.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_ast::Ast;
use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::dump::AstNames;
use kiln_ast::ids::DeclId;
use kiln_ast::types::write_nns;
use kiln_driver::front_end::{FrontEndOptions, run_front_end};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-decl-qualifier-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

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

fn qual_str(ast: &Ast, d: DeclId) -> Option<String> {
    let nns = ast.decl_qualifier(d)?;
    let names = AstNames::new(ast);
    let mut out = String::new();
    write_nns(&ast.types, &names, nns, &mut out);
    Some(out)
}

fn all_qualifiers(ast: &Ast) -> Vec<(DeclId, DeclKind, String)> {
    ast.decls.ids().filter_map(|d| Some((d, ast.decls.get(d).kind(), qual_str(ast, d)?))).collect()
}

const HEADER: &str = "#pragma once\n\
template <class T, int> struct P { void f(); };\n\
namespace n { constexpr int k = 1, j = 1; void g(); void h(); }\n\
namespace na = n;\n\
template <class T> struct P<T, n::j> { void f(); };\n\
template <class T> inline void P<T, n::k>::f() {}\n\
inline void n::g() {}\n\
inline void na::h() {}\n\
template <int> struct Q { static int s; enum E { e }; void m(); };\n\
template <int N> int Q<N>::s = 0;\n\
template <int N> inline void Q<N>::m() {}\n\
constexpr int make(int) { return 1; }\n\
template <> inline int Q<sizeof(make(0))>::s = 5;\n\
struct Pl { static int s; static int t; };\n\
template <int> using AL = Pl;\n\
inline int AL<sizeof(make(0))>::s = 2;\n\
typedef Pl TD;\n\
inline int TD::t = 3;\n\
using enum Q<sizeof(make(0))>::E;\n\
using n::g;\n\
extern template int Q<2>::s;\n\
namespace m {\n\
  template <class T> struct X {};\n\
  struct F { void h(); };\n\
}\n\
extern template struct m::X<int>;\n\
struct Host {\n\
  friend void m::F::h();\n\
  friend struct m::X<int>;\n\
};\n";

/// `Ast::decl_qualifier` survives snapshot serialization (`kiln_snap`)
/// identically, and a real `.pch` snapshot of every qualified shape restores
/// and passes `-fkiln-pch-snapshot=check` against direct replay.
#[test]
fn decl_qualifiers_survive_a_snapshot() {
    let dir = scratch("pch");
    std::fs::write(dir.join("p.h"), HEADER).unwrap();
    std::fs::write(dir.join("m.cpp"), "int use() { return Q<2>::s + AL<4>::s + TD::t + e; }\n")
        .unwrap();

    // Direct front-end parse + `kiln_snap::Snap` round-trip of `Ast`.
    let header = dir.join("p.h");
    let main = dir.join("m.cpp");
    let cl: Vec<String> = [
        "-fsyntax-only",
        "-std=c++26",
        "-include",
        header.to_str().unwrap(),
        main.to_str().unwrap(),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let (direct, restored) = run_front_end("kiln++", &cl, FrontEndOptions::default(), |fe| {
        let before = all_qualifiers(fe.ast);
        let bytes = kiln_snap::to_bytes(fe.ast);
        let after: Ast = kiln_snap::from_bytes(&bytes).expect("Ast snapshot decode");
        (before, all_qualifiers(&after))
    })
    .unwrap()
    .value
    .unwrap();
    assert!(!direct.is_empty());
    assert_eq!(restored, direct);
    let spellings: Vec<&str> = restored.iter().map(|(_, _, s)| s.as_str()).collect();
    for want in [
        "P<T, n::k>::",
        "n::",
        "na::",
        "Q<N>::",
        "Q<sizeof (make(0))>::",
        "AL<sizeof (make(0))>::",
        "TD::",
        "Q<2>::",
        "m::",
        "m::F::",
    ] {
        assert!(spellings.contains(&want), "missing {want:?} in {spellings:?}");
    }

    // End-to-end PCH producer + consumer restore-vs-replay check.
    let (code, err) = kiln(&dir, &["-std=c++26", "-x", "c++-header", "p.h", "-o", "p.pch"]);
    assert_eq!(code, 0, "{err}");
    let (code, err) = kiln(
        &dir,
        &[
            "-std=c++26",
            "-fsyntax-only",
            "-fkiln-pch-snapshot=check",
            "-include-pch",
            "p.pch",
            "m.cpp",
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("snapshot-timing: restore"), "the snapshot was not restored:\n{err}");
}

/// `Ast::decl_qualifier` survives a module declaration image
/// (`-fkiln-bmi-image`): carried qualified definitions, alias-template and
/// typedef qualifiers, `using enum`, `using`, qualified friends, and
/// qualified explicit class instantiations keep their written qualifiers
/// in the importer.
#[test]
#[allow(clippy::too_many_lines)]
fn decl_qualifiers_survive_a_declaration_image() {
    let dir = scratch("image");
    let unit = "\
export module dq;
export namespace n {
  constexpr int k = 1, j = 1;
  void g();
  void h();
}
export namespace na = n;
export void n::g() {}
export void na::h() {}
export template <class T, int N> struct P { void f(); };
template <class T, int N> void P<T, N>::f() {}
export constexpr int make(int) { return 1; }
export struct Pl { static int s; static int t; };
export template <int> using AL = Pl;
int AL<sizeof(make(0))>::s = 2;
export typedef Pl TD;
int TD::t = 3;
export using n::g;
export namespace m {
  enum Color { red };
  template <class T> struct X {};
  struct F { void h(); };
}
export using enum m::Color;
template struct m::X<int>;
export struct Host {
  friend void n::g();
  friend struct m::F;
};
";
    std::fs::write(dir.join("dq.cppm"), unit).unwrap();
    std::fs::write(
        dir.join("use.cpp"),
        "import dq;\nint u() { n::g(); m::X<int> x; (void)x; return Pl::s + Pl::t + red; }\n",
    )
    .unwrap();
    let pcm = dir.join("dq.pcm");
    let pcm_str = pcm.to_str().unwrap();
    let (code, err) = kiln(
        &dir,
        &[
            "-v",
            "-std=c++26",
            "-fkiln-bmi-image",
            "--precompile",
            "-x",
            "c++-module",
            "dq.cppm",
            "-o",
            pcm_str,
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("with a declaration image of "), "no declaration image:\n{err}");

    let mod_file = format!("-fmodule-file=dq={pcm_str}");
    let (code, err) = kiln(
        &dir,
        &["-v", "-std=c++26", "-fsyntax-only", "-fkiln-bmi-image", &mod_file, "use.cpp"],
    );
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("imported 'dq' from its declaration image"),
        "did not import from declaration image:\n{err}"
    );

    let use_cpp = dir.join("use.cpp");
    let cl: Vec<String> =
        ["-fsyntax-only", "-std=c++26", "-fkiln-bmi-image", &mod_file, use_cpp.to_str().unwrap()]
            .into_iter()
            .map(str::to_owned)
            .collect();
    let r = run_front_end("kiln++", &cl, FrontEndOptions::default(), |fe| {
        let ast = fe.ast;
        let qs = all_qualifiers(ast);
        let has = |kind: DeclKind, want: &str| qs.iter().any(|(_, k, s)| *k == kind && s == want);
        assert!(has(DeclKind::Function, "n::"), "missing n:: function in {qs:?}");
        assert!(has(DeclKind::Function, "na::"), "missing na:: function in {qs:?}");
        assert!(has(DeclKind::CXXMethod, "P<T, N>::"), "missing P<T, N>:: in {qs:?}");
        assert!(
            has(DeclKind::Var, "AL<sizeof (make(0))>::"),
            "missing AL<sizeof (make(0))>:: in {qs:?}"
        );
        assert!(has(DeclKind::Var, "TD::"), "missing TD:: in {qs:?}");
        assert!(has(DeclKind::UsingEnum, "m::"), "missing using enum m:: in {qs:?}");
        assert!(has(DeclKind::Using, "n::"), "missing using n:: in {qs:?}");
        assert!(has(DeclKind::CXXRecord, "m::"), "missing m::X<int> explicit inst in {qs:?}");
        // Qualified friends survive on the Friend decl (and befriended method).
        let host = ast
            .decls
            .ids()
            .find(|&d| {
                ast.decls.get(d).kind() == DeclKind::CXXRecord
                    && ast
                        .decls
                        .get(d)
                        .name()
                        .as_ident()
                        .is_some_and(|s| ast.symbols().as_str(s) == "Host")
            })
            .expect("Host");
        let friends: Vec<DeclId> = ast
            .decls
            .members(host)
            .filter(|&d| ast.decls.get(d).kind() == DeclKind::Friend)
            .collect();
        assert_eq!(friends.len(), 2);
        assert_eq!(qual_str(ast, friends[0]).as_deref(), Some("n::"));
        if let FriendTarget::Decl(befriended) = ast.decls.friend_target(friends[0]) {
            assert_eq!(qual_str(ast, befriended).as_deref(), Some("n::"));
        } else {
            panic!("expected friend decl target");
        }
        assert_eq!(qual_str(ast, friends[1]).as_deref(), Some("m::"));
    })
    .unwrap();
    assert_eq!(r.errors, 0, "{:?}", r.first_error);
}
