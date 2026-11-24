//! An instantiation's lifetime-extended temporaries are its own
//! ([class.temporary]/6, P2718R0 for range-based `for`; spec/ast.md
//! "Lifetime-extended temporaries of instantiations", WP-3271): every
//! `MaterializeTemporaryExpr` reached from an instantiated variable's
//! initializer that is lifetime-extended names a `LifetimeExtendedTemporary`
//! whose `extending` declaration is that instantiated variable, and no such
//! temporary (nor its record) is reached from the pattern's variable. The
//! substituter shares non-dependent subexpressions with the pattern by id
//! (spec/sema-templates.md §5.4), so a shared initializer that held an
//! extended temporary would otherwise point back into the pattern.

use std::collections::HashMap;
use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Every extending context the substituter copies, in a function
/// template instantiated twice. Compiles cleanly with clang 19 and 22
/// (`-std=c++23`).
const SRC: &str = "\
struct S { int a[2] = {1, 2}; const int *begin() const { return a; } const int *end() const { return a + 2; } ~S(); };
S make();
const S &id(const S &s);
const S &dflt(const S &s = S{});
struct P { int x, y; };
P makep();
struct A { const S &s; const int &i; };
struct W { S s; };
template <class T> int f() {
  int n = 0;
  for (int x : make()) n += x;
  for (int x : id(make())) n += x;
  for (int x : dflt()) n += x;
  for (int x : W{make()}.s) n += x;
  const S &r = make();
  const int &i = make().a[0];
  auto &&[u, v] = makep();
  const auto &[p, q] = P{1, 2};
  A agg{make(), 3};
  A agg2 = {S{}, i};
  static const S &st = S{};
  const int &lit = 1;
  return n + r.a[0] + i + u + v + p + q + agg.s.a[0] + agg.i + agg2.s.a[1] + st.a[0] + lit;
}
template int f<int>();
template int f<long>();
struct Wrap { static const S &wrap(const S &s); };
struct Wrap2 : Wrap {};
template <class T> int g() {
  int n = 0;
  for (int x : T::wrap(id(make()))) n += x;
  for (int x : T::wrap(dflt())) n += x;
  return n;
}
template int g<Wrap>();
template int g<Wrap2>();
template <class T> struct X { static inline const A m{W{make()}.s, 8}; };
template <class T> const S &vt = W{make()}.s;
int use() { return X<int>::m.i + X<long>::m.i + vt<int>.a[0] + vt<long>.a[0]; }
";

struct Tu {
    ast: Ast,
    diags: Vec<String>,
}

fn parse(src: &str, std: kiln_lex::Std) -> Tu {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let dopts = DiagOptions::default();
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let opts = ParseOptions { std, ..ParseOptions::default() };
    let ast = parse_file(&sm, &interner, file, &engine, Target::for_arch(Arch::X86_64), &opts);
    let diags = sink.take().iter().map(|d| d.message.clone()).collect();
    Tu { ast, diags }
}

fn ident(ast: &Ast, d: DeclId) -> String {
    ast.decls
        .get(d)
        .name()
        .as_ident()
        .map_or_else(String::new, |s| ast.symbols().as_str(s).to_owned())
}

/// The function `d` is declared in (through block scopes).
fn enclosing_fn(ast: &Ast, d: DeclId) -> Option<DeclId> {
    let mut c = ast.decls.get(d).ctx();
    while let Some(x) = c {
        if ast.decls.get(x).kind().is_function() {
            return Some(x);
        }
        c = ast.decls.get(x).ctx();
    }
    None
}

/// The lifetime-extended `MaterializeTemporaryExpr`s reached from `init`
/// (everything below it, lambda bodies aside).
fn extended_in(ast: &Ast, init: ExprId) -> Vec<ExprId> {
    let mut out = Vec::new();
    let mut stack = vec![init];
    while let Some(e) = stack.pop() {
        let ex = &ast.exprs;
        if ex.get(e).kind() == ExprKind::MaterializeTemporaryExpr
            && ex.lifetime_extended_temporary(e).is_some()
        {
            out.push(e);
        }
        for k in [ExprKind::CXXDefaultArgExpr, ExprKind::CXXDefaultInitExpr] {
            if ex.get(e).kind() == k
                && let Some(r) = ex.default_arg_rewritten(e)
            {
                stack.push(r);
            }
        }
        ex.children_into(e, &ast.lists, &mut stack);
    }
    out
}

/// Every variable with an initializer: its name (`__range1` numbered in
/// declaration order per function, a decomposition as `[binding]`), the
/// function it is local to, and its extended temporaries.
fn variables(ast: &Ast) -> Vec<(String, Option<DeclId>, DeclId, Vec<ExprId>)> {
    let mut ranges: HashMap<Option<DeclId>, u32> = HashMap::new();
    let mut out = Vec::new();
    for d in ast.decls.ids() {
        if !matches!(ast.decls.get(d).kind(), DeclKind::Var | DeclKind::VarTemplateSpecialization) {
            continue;
        }
        let Some(init) = ast.decls.var(d).and_then(|v| v.init) else { continue };
        let f = enclosing_fn(ast, d);
        let mut name = ident(ast, d);
        if name.starts_with("__range") {
            let n = ranges.entry(f).or_default();
            *n += 1;
            name = format!("{name}#{n}");
        }
        let name = if name.is_empty() { "[binding]".to_owned() } else { name };
        out.push((name, f, d, extended_in(ast, init)));
    }
    out
}

fn functions_named(ast: &Ast, name: &str) -> Vec<DeclId> {
    ast.decls
        .ids()
        .filter(|&d| ast.decls.get(d).kind() == DeclKind::Function && ident(ast, d) == name)
        .filter(|&d| ast.decls.function(d).is_some_and(|f| f.body.is_some()))
        .collect()
}

/// Every extended temporary is reached from one variable's initializer
/// alone and names that variable; returns the problems.
fn ownership_problems(
    ast: &Ast,
    vars: &[(String, Option<DeclId>, DeclId, Vec<ExprId>)],
) -> Vec<String> {
    let mut owner: HashMap<ExprId, DeclId> = HashMap::new();
    let mut bad = Vec::new();
    for (name, f, v, temps) in vars {
        for &t in temps {
            if let Some(o) = owner.insert(t, *v) {
                bad.push(format!("{name}: temporary {t:?} shared with {}", ident(ast, o)));
            }
            let let_ = ast.exprs.lifetime_extended_temporary(t).expect("extended");
            let data = ast.decls.lifetime_extended(let_).expect("LetData");
            if data.extending != *v {
                bad.push(format!("{name}: extended by another {}", ident(ast, data.extending)));
            }
            if f.is_some() && enclosing_fn(ast, let_) != *f {
                bad.push(format!("{name}: its record is outside its function"));
            }
        }
    }
    bad
}

/// The extended temporaries of each variable of function `f`, by name.
fn counts(
    vars: &[(String, Option<DeclId>, DeclId, Vec<ExprId>)],
    f: DeclId,
) -> Vec<(String, usize)> {
    vars.iter().filter(|x| x.1 == Some(f)).map(|x| (x.0.clone(), x.3.len())).collect()
}

/// The pattern of the function template named `name` and its
/// instantiations.
fn pattern_and_instantiations(ast: &Ast, name: &str) -> (DeclId, Vec<DeclId>) {
    let fs = functions_named(ast, name);
    let pattern =
        fs.iter().copied().find(|&f| ast.instantiated_from(f).is_none()).expect("pattern");
    let insts = fs.iter().copied().filter(|&f| ast.instantiated_from(f) == Some(pattern)).collect();
    (pattern, insts)
}

/// Each instantiation of `f` (a reference bound to a temporary and to a
/// member of one, the reference members of aggregates, structured
/// bindings, a static local, four range-based `for`s with their P2718R0
/// temporaries and a default argument's) extends as many temporaries as
/// the pattern, all its own; so do the instantiations of `g`, whose
/// pattern's ranges are type-dependent and extend nothing, and the
/// specializations of an inline static data member and a variable
/// template.
#[test]
fn instantiated_variables_extend_their_own_temporaries() {
    let tu = parse(SRC, kiln_lex::Std::Cxx23);
    assert!(tu.diags.is_empty(), "{:?}", tu.diags);
    let ast = &tu.ast;
    let vars = variables(ast);
    let bad = ownership_problems(ast, &vars);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let (pf, fs) = pattern_and_instantiations(ast, "f");
    assert_eq!(fs.len(), 2);
    let expect = counts(&vars, pf);
    assert!(expect.iter().filter(|x| x.1 > 0).count() >= 10, "{expect:?}");
    for f in fs {
        assert_eq!(counts(&vars, f), expect);
    }
    let (pg, gs) = pattern_and_instantiations(ast, "g");
    assert_eq!(gs.len(), 2);
    assert!(counts(&vars, pg).iter().all(|x| x.1 == 0), "a dependent range extends nothing");
    for g in gs {
        let c: Vec<_> =
            counts(&vars, g).into_iter().filter(|x| x.0.starts_with("__range")).collect();
        assert_eq!(c, [("__range1#1".to_owned(), 1), ("__range1#2".to_owned(), 1)]);
    }
    for name in ["m", "vt"] {
        let specs: Vec<_> =
            vars.iter().filter(|x| x.0 == name && ast.instantiated_from(x.2).is_some()).collect();
        assert_eq!(specs.len(), 2, "{name}");
        for s in specs {
            assert_eq!(s.3.len(), if name == "m" { 2 } else { 1 }, "{name}");
        }
    }
}
