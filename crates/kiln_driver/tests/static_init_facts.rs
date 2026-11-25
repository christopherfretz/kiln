//! The static-initialization facts (spec/ast.md §25, WP-3650): with
//! `-fkiln-static-init-facts` (`FrontEndOptions::static_init_facts`) sema
//! records, at the end of the translation unit, how each variable of
//! static or thread storage duration is initialized and destroyed, and
//! code generation reads the record. Each test checks the recorded facts
//! through the front-end entry, and that the IR is byte for byte the same
//! with the option on and off (the facts are code generation's own
//! decisions).
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use kiln_ast::Ast;
use kiln_ast::ids::DeclId;
use kiln_ast::static_init::{NotEvaluatedWhy, StaticDestruction, StaticInit, StaticInitFact};
use kiln_driver::front_end::{FrontEndOptions, run_front_end};

/// A fresh temporary directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("kiln-static-init-{name}-{}-{n}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, bytes: &str) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p.to_str().unwrap().to_owned()
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn kiln() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_kiln")).with_file_name("kiln++")
}

fn ident(ast: &Ast, d: DeclId) -> &str {
    ast.decls.get(d).name().as_ident().map_or("", |s| ast.symbols().as_str(s))
}

/// The recorded fact of every named variable that has one, by name (a
/// name with several facts keeps the first; the tests use distinct
/// names).
fn facts(src: &str, extra: &[&str]) -> BTreeMap<String, StaticInitFact> {
    let s = Scratch::new("facts");
    let main = s.write("main.cpp", src);
    let mut cl: Vec<String> = vec!["-fsyntax-only".into(), "-std=c++26".into(), "-w".into()];
    cl.extend(extra.iter().map(|&a| a.to_owned()));
    cl.push(main);
    let mut o = FrontEndOptions::default();
    o.static_init_facts = true;
    let r = run_front_end("kiln++", &cl, o, |fe| {
        assert!(fe.ast.static_init_facts);
        let mut m = BTreeMap::new();
        for d in fe.ast.decls.ids() {
            if let Some(f) = fe.ast.static_init_fact(d) {
                m.entry(ident(fe.ast, d).to_owned()).or_insert(f);
            }
        }
        m
    })
    .unwrap();
    r.value.unwrap()
}

/// Compiles `src` to IR with the option off and on, and asserts the two
/// are the same; returns the IR.
fn same_ir(src: &str, extra: &[&str]) -> String {
    let s = Scratch::new("ir");
    let main = s.write("main.cpp", src);
    let ir = |on: bool, out: &str| {
        let mut c = Command::new(kiln());
        c.args(["-std=c++26", "-w", "-S", "-emit-llvm"]).args(extra);
        if on {
            c.arg("-fkiln-static-init-facts");
        }
        let o = c.arg(&main).arg("-o").arg(out).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        std::fs::read_to_string(out).unwrap()
    };
    let off = ir(false, &s.path("off.ll"));
    let on = ir(true, &s.path("on.ll"));
    assert_eq!(off, on, "the IR differs with -fkiln-static-init-facts");
    off
}

fn fact(init: StaticInit, destruction: StaticDestruction) -> StaticInitFact {
    StaticInitFact { init, destruction: Some(destruction) }
}

use StaticDestruction::{Constant as DConst, Registered, Trivial};
use StaticInit::{Constant, Dynamic, EarlyStatic, NotEvaluated};

fn check(src: &str, extra: &[&str], want: &[(&str, StaticInitFact)]) -> String {
    let got = facts(src, extra);
    for (name, f) in want {
        assert_eq!(got.get(*name), Some(f), "{name}: {got:?}");
    }
    same_ir(src, extra)
}

/// The case reduced from real code: a static data member of a class
/// template, of the template's own type, list-initialized out of line.
#[test]
fn class_template_member_of_own_type() {
    let src = "template <class T> struct Tr { int v; static Tr r; };\n\
               template <class T> Tr<T> Tr<T>::r{1};\n\
               int use() { return Tr<int>::r.v; }\n";
    check(src, &[], &[("r", fact(Constant, Trivial))]);
}

/// libstdc++'s `std::atomic<int>`, `std::once_flag` and `std::mutex` as
/// static data members of a class template: all constant-initialized
/// (their constructors are `constexpr`), none destroyed.
#[test]
fn library_members_of_a_class_template() {
    let src = "#include <atomic>\n#include <mutex>\n\
               template <class T> struct H {\n\
                 static std::atomic<int> a; static std::once_flag o; static std::mutex m; };\n\
               template <class T> std::atomic<int> H<T>::a{0};\n\
               template <class T> std::once_flag H<T>::o;\n\
               template <class T> std::mutex H<T>::m;\n\
               int use() { H<int>::a++; std::call_once(H<int>::o, [] {});\n\
                 H<int>::m.lock(); return 0; }\n";
    check(
        src,
        &[],
        &[
            ("a", fact(Constant, Trivial)),
            ("o", fact(Constant, Trivial)),
            ("m", fact(Constant, Trivial)),
        ],
    );
}

/// A call to a non-`constexpr` function initializes dynamically; a
/// scalar initializer that folds outside a constant context is still
/// initialized statically (early static).
#[test]
fn dynamic_and_early_static() {
    let src = "int f();\nint g;\n\
               int dyn = f();\n\
               long early = (long)&g;\n\
               int ice = __builtin_is_constant_evaluated() ? f() : 2;\n\
               int zero;\n\
               struct S { static const int k = 5; static int n; };\n\
               int S::n = S::k + 1;\n";
    let ir = check(
        src,
        &[],
        &[
            ("dyn", fact(Dynamic, Trivial)),
            ("early", fact(EarlyStatic, Trivial)),
            ("ice", fact(EarlyStatic, Trivial)),
            ("zero", fact(Constant, Trivial)),
            ("n", fact(Constant, Trivial)),
        ],
    );
    assert!(
        ir.contains("@ice = global i32 2") || ir.contains("@ice = dso_local global i32 2"),
        "{ir}"
    );
}

/// A `constexpr` destructor of a constant-initialized variable is a
/// constant destruction; a non-trivial non-`constexpr` one is registered.
#[test]
fn constant_and_registered_destruction() {
    let src = "struct D { ~D(); };\n\
               struct CD { constexpr CD() {} constexpr ~CD() {} int x = 1; };\n\
               D d;\n\
               CD cd;\n\
               constinit CD ci;\n";
    check(
        src,
        &[],
        &[
            ("d", fact(Constant, Registered)),
            ("cd", fact(Constant, DConst)),
            ("ci", fact(Constant, DConst)),
        ],
    );
}

/// A reference bound to a temporary: a constant one is constant and
/// destroys nothing; a dynamic one registers the temporary's destructor
/// when it has one.
#[test]
fn references_bound_to_temporaries() {
    let src = "int f();\nstruct D { ~D(); };\n\
               const int &ri = 3;\n\
               const int &rf = f();\n\
               const D &rd = D();\n\
               int g;\nint &rg = g;\n";
    check(
        src,
        &[],
        &[
            ("ri", fact(Constant, Trivial)),
            ("rf", fact(Dynamic, Trivial)),
            ("rd", fact(Dynamic, Registered)),
            ("rg", fact(Constant, Trivial)),
        ],
    );
}

/// `thread_local` variables and static locals, including a
/// `thread_local` static local.
#[test]
fn thread_local_and_static_locals() {
    let src = "int f();\nstruct D { ~D(); };\n\
               thread_local int tl = f();\n\
               thread_local int tc = 2;\n\
               thread_local D td;\n\
               int h() { static int s = f(); static int c = 3; static D sd;\n\
                 static thread_local int stl = f(); return s + c + stl; }\n";
    check(
        src,
        &[],
        &[
            ("tl", fact(Dynamic, Trivial)),
            ("tc", fact(Constant, Trivial)),
            ("td", fact(Constant, Registered)),
            ("s", fact(Dynamic, Trivial)),
            ("c", fact(Constant, Trivial)),
            ("sd", fact(Constant, Registered)),
            ("stl", fact(Dynamic, Trivial)),
        ],
    );
}

/// Inline variables and variable template specializations.
#[test]
fn inline_and_variable_templates() {
    let src = "int f();\n\
               inline int iv = 7;\n\
               inline int ivd = f();\n\
               template <class T> T vt = T(f());\n\
               template <class T> constexpr T vc = T(4);\n\
               int use() { return iv + ivd + vt<int> + vc<int>; }\n";
    check(
        src,
        &[],
        &[
            ("iv", fact(Constant, Trivial)),
            ("ivd", fact(Dynamic, Trivial)),
            ("vt", fact(Dynamic, Trivial)),
            ("vc", fact(Constant, Trivial)),
        ],
    );
}

/// An evaluation that hits a budget gives no verdict: not evaluated, and
/// why; code generation then decides on its own.
#[test]
fn failed_evaluation_is_not_evaluated() {
    let not_evaluated = |why| StaticInitFact { init: NotEvaluated(why), destruction: None };
    let deep = "constexpr int g(int n) { return n ? g(n - 1) : 0; }\n\
                int deep = g(100000);\n";
    check(deep, &[], &[("deep", not_evaluated(NotEvaluatedWhy::DepthLimit))]);
    let steps = "constexpr int loop(int n) { int s = 0; for (int i = 0; i < n; ++i) s += i; return s; }\n\
                 int steps = loop(1000000);\n";
    check(
        steps,
        &["-fconstexpr-steps=1000"],
        &[("steps", not_evaluated(NotEvaluatedWhy::StepLimit))],
    );
}

/// An initializer with errors is not evaluated.
#[test]
fn initializer_with_errors() {
    let got = facts("int bad = undeclared_name;\nint ok = 1;\n", &[]);
    assert!(matches!(got.get("bad").map(|f| f.init), None | Some(NotEvaluated(_))), "{got:?}");
    assert_eq!(got.get("ok"), Some(&fact(Constant, Trivial)));
}

/// Off (the default), nothing is recorded and the dump has no fact; on,
/// the dump shows it.
#[test]
fn off_records_nothing_and_dump_shows_on() {
    let src = "int f();\nint dyn = f();\n";
    let s = Scratch::new("off");
    let main = s.write("main.cpp", src);
    let r = run_front_end(
        "kiln++",
        &["-fsyntax-only".to_owned(), main.clone()],
        FrontEndOptions::default(),
        |fe| {
            assert!(!fe.ast.static_init_facts);
            fe.ast.decls.ids().all(|d| fe.ast.static_init_fact(d).is_none())
        },
    )
    .unwrap();
    assert_eq!(r.value, Some(true));
    let dump = |on: bool| {
        let mut c = Command::new(kiln());
        c.args(["-std=c++26", "-fsyntax-only", "-fdump-ast"]);
        if on {
            c.arg("-fkiln-static-init-facts");
        }
        let o = c.arg(&main).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap()
    };
    assert!(!dump(false).contains("static-init="));
    let on = dump(true);
    assert!(
        on.lines().any(|l| l.contains("VarDecl dyn 'int'")
            && l.ends_with("static-init=dynamic static-destroy=trivial")),
        "{on}"
    );
}
