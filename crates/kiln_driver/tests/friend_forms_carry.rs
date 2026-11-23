//! A friend's written form (spec/ast.md §23, WP-3164) through
//! `kiln++`: `-fkiln-dump-friend-forms` prints the rows; a PCH snapshot
//! carries them in declaration order ([cpp.include]: a precompiled
//! prefix must not change them), and a friend imported from a module's
//! declaration image ([module.import]) answers `NotCarried`.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kiln_ast::decl::DeclKind;
use kiln_ast::friend_form::FriendFormAnswer;
use kiln_driver::front_end::{FrontEndOptions, run_front_end};

fn kilnpp() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("kiln-friend-forms-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs `kiln++ args` in `dir`: (exit code, stdout, stderr).
fn kiln(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(kilnpp())
        .args(args)
        .current_dir(dir)
        .env("KILN_SNAP_TIMING", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(!stderr.contains("panicked"), "kiln++ {args:?} panicked:\n{stderr}");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), stderr)
}

/// Friends of every kind with a row, in a header.
const HEADER: &str = "#pragma once\n\
template <int N> struct Bq { void g(); struct Y {}; template <int M> void t(); };\n\
int f(...);\n\
template <int U> void h();\n\
template <class T> struct Z { Z(); };\n\
namespace N { void k(); }\n\
template <class T> struct S {\n\
  friend void Bq<sizeof(sizeof(f(T())))>::g();\n\
  friend void h<sizeof(sizeof(f(T())))>();\n\
  friend void Bq<8>::t<sizeof(sizeof(f(T())))>();\n\
  friend void Bq<sizeof(T)>::g();\n\
  friend Z<T>::Z();\n\
};\n\
struct Q { friend void ::N::k(); };\n";

const MAIN: &str = "struct R { friend void N::k(); };\nS<int> s;\n";

/// The dump flag's golden, and the same rows (positions in the header)
/// whether the header is included or restored from its snapshot.
#[test]
fn rows_survive_a_snapshot_in_order() {
    let dir = scratch("pch");
    std::fs::write(dir.join("p.h"), HEADER).unwrap();
    std::fs::write(dir.join("m.cpp"), MAIN).unwrap();
    let dump = ["-std=c++26", "-fsyntax-only", "-fkiln-dump-friend-forms"];
    let (code, direct, err) = kiln(&dir, &[&dump[..], &["-include", "p.h", "m.cpp"]].concat());
    assert_eq!(code, 0, "{err}");
    let want = "p.h:8:41 FriendForm function qualifier 'Bq<sizeof (sizeof (f(T())))>::' name 'g'@p.h:8:41\n\
                p.h:9:13 FriendForm specialization name 'h'@p.h:9:13 args <sizeof (sizeof (f(T())))> entity h@p.h:4:23 found [h@p.h:4:23]\n\
                p.h:10:20 FriendForm specialization qualifier 'Bq<8>::' name 't'@p.h:10:20 args <sizeof (sizeof (f(T())))> entity Bq<8>::t@p.h:2:75 found [Bq<8>::t@p.h:2:75]\n\
                p.h:11:28 FriendForm function qualifier 'Bq<sizeof(T)>::' name 'g'@p.h:11:28 entity Bq::g@p.h:2:35 dependent\n\
                p.h:12:14 FriendForm constructor qualifier 'Z<T>::' name 'Z<T>'@p.h:12:14 entity Z::Z<T>@p.h:5:31 dependent\n\
                p.h:14:29 FriendForm function qualifier '::N::' name 'k'@p.h:14:29 global\n\
                1:27 FriendForm function qualifier 'N::' name 'k'@1:27\n";
    assert_eq!(direct, want);

    let (code, _, err) = kiln(&dir, &["-std=c++26", "-x", "c++-header", "p.h", "-o", "p.pch"]);
    assert_eq!(code, 0, "{err}");
    let (code, restored, err) =
        kiln(&dir, &[&dump[..], &["-include-pch", "p.pch", "m.cpp"]].concat());
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("snapshot-timing: restore"), "the snapshot was not restored:\n{err}");
    assert_eq!(restored, direct);
}

/// A unit's friends imported from its declaration image have no row the
/// image carries: `NotCarried`, not `None` (§23.4, §23.5).
#[test]
fn image_friends_are_not_carried() {
    let dir = scratch("image");
    let unit = "export module fm;\n\
                export struct A { friend void f(A); friend class B; int x; };\n\
                export void f(A);\n";
    std::fs::write(dir.join("fm.cppm"), unit).unwrap();
    std::fs::write(dir.join("use.cpp"), "import fm;\nA a;\n").unwrap();
    let pcm = dir.join("fm.pcm");
    let pcm = pcm.to_str().unwrap();
    let (code, _, err) = kiln(
        &dir,
        &[
            "-v",
            "-std=c++26",
            "-fkiln-bmi-image",
            "--precompile",
            "-x",
            "c++-module",
            "fm.cppm",
            "-o",
            pcm,
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("with a declaration image of "), "{err}");
    let file = format!("-fmodule-file=fm={pcm}");
    let (code, _, err) =
        kiln(&dir, &["-v", "-std=c++26", "-fsyntax-only", "-fkiln-bmi-image", &file, "use.cpp"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("imported 'fm' from its declaration image"), "{err}");
    let user = dir.join("use.cpp");
    let cl: Vec<String> = [
        "-fsyntax-only",
        "-std=c++26",
        "-fkiln-bmi-image",
        &format!("-fmodule-file=fm={pcm}"),
        user.to_str().unwrap(),
    ]
    .iter()
    .map(|&s| s.to_owned())
    .collect();
    let r = run_front_end("kiln++", &cl, FrontEndOptions::default(), |fe| {
        let ast = fe.ast;
        let friends: Vec<_> =
            ast.decls.ids().filter(|&d| ast.decls.get(d).kind() == DeclKind::Friend).collect();
        assert_eq!(friends.len(), 2);
        friends.iter().all(|&d| matches!(ast.friend_form(d), FriendFormAnswer::NotCarried))
    })
    .unwrap();
    assert_eq!((r.value, r.errors), (Some(true), 0), "{:?}", r.first_error);
}
