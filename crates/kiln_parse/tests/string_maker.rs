//! The maker of a string kiln makes itself (spec/ast.md §5.8.1, WP-3000):
//! `ExprArena::string_maker` and `Ast::string_maker_of` lead every pointer
//! to such a string, held in a constant, to one maker: a source-location
//! use of one role evaluated in one declaration ([support.srcloc.cons]/2:
//! a default argument evaluated at a call is a use in the caller), a
//! reflection ([meta.reflection.names]) or a contract site
//! ([basic.contract.eval]). Source-location strings are one object per
//! (content, role, declaration), never shared with reflection's or a
//! contract site's ([lex.string]/9 and [intro.object]/9 tie no identity to
//! the contents); a literal the source spells has no maker.

use std::sync::Arc;

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{SrcLocRole, StringMaker};
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const SRC: &str = "\
namespace std::meta {
using info = decltype(^^int);
consteval const char *name_of(info r);
}
void take(const char *);
void f(int) { constexpr const char *p = __builtin_FUNCTION(); take(p); }
void f(long) { constexpr const char *p = __builtin_FUNCTION(); take(p); }
constexpr const char *g(const char *s = __builtin_FUNCTION()) { return s; }
void h() { constexpr const char *q = g(); take(q); }
namespace x { void h() { constexpr const char *q = g(); take(q); } }
void file_user() { constexpr const char *fl = __builtin_FILE(); take(fl); }
constexpr const char *file_ns = __builtin_FILE();
void fn();
constexpr const char *refl = std::meta::name_of(^^fn);
void fn() { constexpr const char *own = __builtin_FUNCTION(); take(own); }
constexpr const char *lit = \"f\";
int get();
void c() { contract_assert(get()); }
";

fn parse(f: impl FnOnce(&Ast)) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", SRC.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { reflection: true, contracts: Some(true), ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let diags = sink.take();
    assert!(diags.is_empty(), "{diags:?}");
    f(&tu.ast);
}

/// Every variable named `name`, in declaration order.
fn vars(ast: &Ast, name: &str) -> Vec<DeclId> {
    (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .filter(|&d| {
            let n = ast.decls.get(d);
            n.kind() == DeclKind::Var
                && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .collect()
}

/// The only variable named `name`.
fn var(ast: &Ast, name: &str) -> DeclId {
    let v = vars(ast, name);
    assert_eq!(v.len(), 1, "{name}");
    v[0]
}

/// The function named `name` declared in `ctx` (its first declaration).
fn function(ast: &Ast, ctx: DeclId, name: &str, nth: usize) -> DeclId {
    let fs: Vec<DeclId> = (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .filter(|&d| {
            let n = ast.decls.get(d);
            n.kind() == DeclKind::Function
                && n.ctx() == Some(ctx)
                && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .collect();
    fs[nth]
}

/// The constant value of variable `d`.
fn value(ast: &Ast, d: DeclId) -> ConstValueId {
    ast.object_value(d).expect("a constant")
}

/// The maker and origin of the string `d`'s value points into.
fn maker(ast: &Ast, d: DeclId) -> (StringMaker, ExprId) {
    ast.string_maker_of(value(ast, d)).expect("a kiln-made string")
}

/// The contents of the string `d`'s value points into.
fn contents(ast: &Ast, d: DeclId) -> String {
    match ast.consts.scalar(value(ast, d)) {
        Some(Scalar::Pointer { base: PtrBase::StringLit { lit, .. }, .. }) => {
            String::from_utf8_lossy(ast.strings.bytes(lit)).into_owned()
        }
        other => panic!("not a string pointer: {other:?}"),
    }
}

/// [support.srcloc.cons], [lex.string]/9: the plain name of `f(int)` and
/// of `f(long)` are two objects, each made by a `Function` use in its own
/// function.
#[test]
fn overloads_get_one_object_each_support_srcloc_cons() {
    parse(|ast| {
        let tu = ast.tu;
        let ps = vars(ast, "p");
        assert_eq!(ps.len(), 2);
        let (fi, fl) = (function(ast, tu, "f", 0), function(ast, tu, "f", 1));
        let (m0, o0) = maker(ast, ps[0]);
        let (m1, o1) = maker(ast, ps[1]);
        assert_eq!(contents(ast, ps[0]), "f");
        assert_eq!(contents(ast, ps[1]), "f");
        assert_eq!(m0, StringMaker::SourceLoc { role: SrcLocRole::Function, decl: fi });
        assert_eq!(m1, StringMaker::SourceLoc { role: SrcLocRole::Function, decl: fl });
        assert_ne!(o0, o1, "one object per declaration");
        assert_eq!(ast.exprs.string_maker(o0), Some(m0));
    });
}

/// [support.srcloc.cons]/2, [dcl.fct.default]/9: a default argument
/// evaluated at a call is a use in the caller, so `h()` and `x::h()` each
/// make their own `"h"`.
#[test]
fn default_argument_is_the_callers_use_support_srcloc_cons_2() {
    parse(|ast| {
        let qs = vars(ast, "q");
        assert_eq!(qs.len(), 2);
        let h = function(ast, ast.tu, "h", 0);
        let (m0, o0) = maker(ast, qs[0]);
        let (m1, o1) = maker(ast, qs[1]);
        assert_eq!(contents(ast, qs[0]), "h");
        assert_eq!(m0, StringMaker::SourceLoc { role: SrcLocRole::Function, decl: h });
        let StringMaker::SourceLoc { role: SrcLocRole::Function, decl: xh } = m1 else {
            panic!("{m1:?}");
        };
        assert_ne!(xh, h);
        assert_ne!(o0, o1);
    });
}

/// The role is part of the object: a file name made in a function, and
/// one made at namespace scope (in the translation unit), are two
/// objects, `File` each.
#[test]
fn file_role_and_context() {
    parse(|ast| {
        let (fl, ns) = (var(ast, "fl"), var(ast, "file_ns"));
        let user = function(ast, ast.tu, "file_user", 0);
        assert_eq!(contents(ast, fl), "t.cpp");
        let (m0, o0) = maker(ast, fl);
        let (m1, o1) = maker(ast, ns);
        assert_eq!(m0, StringMaker::SourceLoc { role: SrcLocRole::File, decl: user });
        assert_eq!(m1, StringMaker::SourceLoc { role: SrcLocRole::File, decl: ast.tu });
        assert_ne!(o0, o1);
    });
}

/// [meta.reflection.names]: reflection's name of `fn` is an object of its
/// own, not the object of `fn`'s `__builtin_FUNCTION()`.
#[test]
fn reflection_strings_are_their_own_meta_reflection_names() {
    parse(|ast| {
        let (refl, own) = (var(ast, "refl"), var(ast, "own"));
        assert_eq!(contents(ast, refl), "fn");
        assert_eq!(contents(ast, own), "fn");
        let (m0, o0) = maker(ast, refl);
        let (m1, o1) = maker(ast, own);
        assert_eq!(m0, StringMaker::Reflection);
        let fnd = ast.decls.get(own).ctx().expect("fn");
        assert_eq!(m1, StringMaker::SourceLoc { role: SrcLocRole::Function, decl: fnd });
        assert_ne!(o0, o1);
    });
}

/// [basic.contract.eval]: a contract site's strings are made by the site
/// (spec/contracts.md §6.3), never shared with a source-location string;
/// the predicate `get()` is no function's plain name.
#[test]
fn contract_site_strings_basic_contract_eval() {
    parse(|ast| {
        let sites: Vec<_> = ast.contract_sites.iter().map(|(_, s)| *s).collect();
        assert_eq!(sites.len(), 1);
        for v in [sites[0].file, sites[0].function, sites[0].comment] {
            let (m, _) = ast.string_maker_of(v).expect("a kiln-made string");
            assert_eq!(m, StringMaker::ContractSite);
        }
    });
}

/// A literal the source spells has no maker.
#[test]
fn written_literal_has_no_maker_lex_string() {
    parse(|ast| {
        let lit = var(ast, "lit");
        assert_eq!(contents(ast, lit), "f");
        assert_eq!(ast.string_maker_of(value(ast, lit)), None);
    });
}
