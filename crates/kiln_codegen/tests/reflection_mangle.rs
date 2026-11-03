//! The placeholder mangling of reflection template arguments and the
//! emission of `std::meta::info`-templated entities (spec/reflection.md
//! §8.3, WP-485). kiln only: clang 19/22 lack `^^`, so the IR is checked
//! by shape rather than against an oracle.
//!
//! The grammar: `<expr-primary> ::= L <info-type> <number> E`, with
//! `<info-type> ::= u6__info` a substitution candidate and `<number>` the
//! `ReflectionId` index (TU-local, so every such symbol is internal).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use kiln_abi::mangle::{AnonIds, mangle_decl};
use kiln_abi::{Arch, Target};
use kiln_ast::decl::DeclKind;
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Parses `src` with `-freflection` and lowers it; returns the IR text
/// and the symbols of the function template specializations named `name`
/// mangled twice over one fresh [`AnonIds`], with its literal count.
fn lower(src: &str, name: &str) -> (String, Vec<String>, Vec<String>, u64) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions { reflection: true, ..ParseOptions::default() },
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let opts = CodegenOptions { source_filename: "t.cpp".to_owned(), ..CodegenOptions::default() };
    let m = emit(&mut tu.ast, &mut tu.oven, &target, &opts).expect("codegen");
    let ir = m.to_string();
    let ast = &tu.ast;
    let specs: Vec<_> = ast
        .decls
        .ids()
        .filter(|&d| {
            ast.decls.get(d).kind() == DeclKind::Function
                && ast.spec_info(d).is_some()
                && kiln_abi::mangle::decl_name(ast, d) == name
        })
        .collect();
    let mut anon = AnonIds::new();
    let first: Vec<String> = specs.iter().map(|&d| mangle_decl(ast, &mut anon, d)).collect();
    let second: Vec<String> = specs.iter().map(|&d| mangle_decl(ast, &mut anon, d)).collect();
    (ir, first, second, anon.reflection_literals())
}

/// The `define`d or global symbols of `ir` starting with `prefix`, with
/// their linkage word.
fn symbols(ir: &str, prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in ir.lines() {
        let (link, rest) = if let Some(r) = line.strip_prefix("define ") {
            let w = r.split_whitespace().next().unwrap_or("");
            (w.to_owned(), r)
        } else if let Some(r) = line.strip_prefix('@') {
            let w = r.split(" = ").nth(1).and_then(|s| s.split_whitespace().next()).unwrap_or("");
            (w.to_owned(), line)
        } else {
            continue;
        };
        if let Some(at) = rest.find(&format!("@{prefix}")) {
            let s: String = rest[at + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
                .collect();
            out.push((s, link));
        }
    }
    out
}

const SHAPES: &str = r"
using info = decltype(^^int);
using I = int;
using J = int;
struct P { int x; double y; };
template <info R> int f() { static int n = 0; return ++n; }
template <info R> int v = 1;
template <info R> struct S {
  static int count;
  virtual int get() const { return 1; }
};
template <info R> int S<R>::count = 0;
template <info A, info B> int two() { return 2; }
template <class T> struct Box {};
template <info R, class T> int h(Box<T>, T) { return 3; }
int use() {
  S<^^int> s;
  S<^^I>::count++;
  S<^^int>::count++;
  return f<^^int>() + f<^^I>() + f<^^J>() + f<^^int>() + v<^^P::x> + v<^^P::y>
       + s.get() + two<^^I, ^^J>() + two<^^int, ^^int>() + h<^^P>(Box<P>{}, P{});
}
";

/// [temp.type]/2: `^^int`, `^^I` and `^^J` are three specializations;
/// each specialization is defined once, with internal linkage.
#[test]
fn distinct_reflections_distinct_internal_symbols() {
    let (ir, first, _, _) = lower(SHAPES, "f");
    let f = symbols(&ir, "_Z1fILu6__info");
    let defs: BTreeSet<_> =
        f.iter().filter(|(s, _)| s.ends_with("Eiv")).map(|(s, _)| s.clone()).collect();
    assert_eq!(defs.len(), 3, "{f:?}");
    assert_eq!(first.len(), 3);
    assert!(f.iter().all(|(_, l)| l == "internal"), "{f:?}");
    // The static of each specialization is its own internal object.
    let statics = symbols(&ir, "_ZZ1fILu6__info");
    assert_eq!(statics.len(), 3, "{statics:?}");
    assert!(statics.iter().all(|(_, l)| l == "internal"), "{statics:?}");
    // The variable template, the class template's static member, vtable
    // and member function: one internal symbol per reflection.
    for (prefix, n) in [("_Z1vILu6__info", 2), ("_ZN1SILu6__info", 2), ("_ZTV1SILu6__info", 1)] {
        let s = symbols(&ir, prefix);
        let names: BTreeSet<_> = s.iter().map(|(n, _)| n.clone()).collect();
        assert!(names.len() >= n, "{prefix}: {s:?}");
        assert!(s.iter().all(|(_, l)| l == "internal"), "{prefix}: {s:?}");
    }
    let counts: BTreeSet<_> = symbols(&ir, "_ZN1SILu6__info")
        .into_iter()
        .filter(|(s, _)| s.ends_with("5countE"))
        .map(|(s, _)| s)
        .collect();
    assert_eq!(counts.len(), 2, "S<^^int>::count and S<^^I>::count: {counts:?}");
}

/// A second reflection literal in one symbol reuses the info type
/// through the substitution table (`S_` is the template name `two`, `S0_`
/// the info type).
#[test]
fn second_literal_uses_substitution() {
    let (ir, ..) = lower(SHAPES, "two");
    let two: BTreeSet<_> = symbols(&ir, "_Z3twoILu6__info").into_iter().map(|(s, _)| s).collect();
    assert_eq!(two.len(), 2, "{two:?}");
    for s in &two {
        let rest = s.strip_prefix("_Z3twoILu6__info").unwrap();
        let (id, tail) = rest.split_once('E').unwrap();
        assert!(id.bytes().all(|b| b.is_ascii_digit()), "{s}");
        let id2 = tail.strip_prefix("LS0_").and_then(|t| t.strip_suffix("EEiv")).unwrap_or("");
        assert!(!id2.is_empty() && id2.bytes().all(|b| b.is_ascii_digit()), "{s}");
    }
    // two<^^int, ^^int> repeats one id; two<^^I, ^^J> has two.
    let same = two.iter().filter(|s| {
        let r = s.strip_prefix("_Z3twoILu6__info").unwrap();
        let (a, t) = r.split_once("ELS0_").unwrap();
        t.strip_suffix("EEiv") == Some(a)
    });
    assert_eq!(same.count(), 1, "{two:?}");
    // The info type is itself a candidate, so the later references count
    // past it: in `h<^^P>(Box<P>, P)` S_ = h, S0_ = u6__info, S1_ = P,
    // S2_ = Box, S3_ = T0_ (= P), S4_ = Box<T0_>; the second parameter is
    // S3_ (llvm-cxxfilt: `int h<(__info)N, P>(Box<P>, P)`).
    let h: Vec<_> = symbols(&ir, "_Z1hILu6__info").into_iter().map(|(s, _)| s).collect();
    assert!(!h.is_empty() && h.iter().all(|s| s.ends_with("E1PEi3BoxIT0_ES3_")), "{h:?}");
}

/// Mangling is a function of the TU: emitting it twice gives the same IR.
#[test]
fn deterministic() {
    let (a, ..) = lower(SHAPES, "f");
    let (b, ..) = lower(SHAPES, "f");
    assert_eq!(a, b);
}

/// 2,000 specializations differing only in the reflection: 2,000 distinct
/// symbols, equal on a second mangling, and the mangler's literal counter
/// (`AnonIds::reflection_literals`) counts one literal per symbol.
#[test]
fn two_thousand_specializations() {
    const N: usize = 2000;
    let mut src =
        String::from("using info = decltype(^^int);\ntemplate <info R> int spec() { return 1; }\n");
    for i in 0..N {
        let _ = writeln!(src, "struct C{i} {{}};");
    }
    src.push_str("int (*table[])() = {\n");
    for i in 0..N {
        let _ = writeln!(src, "  &spec<^^C{i}>,");
    }
    src.push_str("};\n");
    let (ir, first, second, literals) = lower(&src, "spec");
    assert_eq!(first.len(), N);
    assert_eq!(first, second, "a second mangling is identical");
    let distinct: BTreeSet<_> = first.iter().collect();
    assert_eq!(distinct.len(), N);
    assert_eq!(literals, 2 * N as u64);
    let defs = symbols(&ir, "_Z4specILu6__info");
    assert_eq!(defs.iter().filter(|(_, l)| l == "internal").count(), N);
}
