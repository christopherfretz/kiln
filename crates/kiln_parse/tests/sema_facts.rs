//! Facts overload resolution and instantiation read that the `Ast` keeps
//! after the translation unit, for tooling clients (spec/ast.md §5.10 and
//! §8.5; WP-2020, WP-2021): the written name of a member access
//! ([expr.ref], [class.access.base]/5) and the definition-context lookup
//! set of a dependent operator expression ([temp.dep.candidate]/1,
//! [over.match.oper]/3.2). Each test parses a TU, drops sema, and reads
//! the `Ast` alone.

use std::sync::Arc;

use kiln_ast::expr::{ExprKind, MemberAccessName};
use kiln_ast::types::Nns;
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The `Ast` of `src` (which must compile cleanly) and its interner.
fn parse(src: &str) -> (Ast, Arc<Interner>) {
    let interner = Arc::new(Interner::new());
    (parse_in(src, &interner), interner)
}

fn parse_in(src: &str, interner: &Arc<Interner>) -> Ast {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let ast = parse_file(&sm, interner, file, &engine, target, &opts);
    let diags: Vec<String> = sink.take().iter().map(|d| d.message.clone()).collect();
    assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    ast
}

/// The simple name of `d`.
fn name(ast: &Ast, i: &Interner, d: DeclId) -> String {
    ast.names.simple_spelling(ast.decls.get(d).name(), i).unwrap_or_default()
}

/// Every expression of `ast`, in creation order.
fn exprs(ast: &Ast) -> impl Iterator<Item = ExprId> + '_ {
    (1..=ast.exprs.len() as u32).map(ExprId::new)
}

/// The overloaded member accesses (`MemberExpr`s of overloaded type)
/// whose first candidate is named `member`, with their names.
fn overloaded(ast: &Ast, i: &Interner, member: &str) -> Vec<MemberAccessName> {
    exprs(ast)
        .filter(|&e| {
            let n = ast.exprs.get(e);
            n.kind() == ExprKind::MemberExpr
                && n.ty() == QualType::OVERLOAD
                && name(ast, i, ast.exprs.decl(e)) == member
        })
        .map(|e| ast.member_access_name(e).expect("a member access"))
        .collect()
}

/// The class a type nested-name-specifier names.
fn qualifier_class(ast: &Ast, i: &Interner, m: &MemberAccessName) -> String {
    match ast.types.nns_view(m.qualifier.expect("a qualifier")) {
        Nns::Type { ty, .. } => name(ast, i, ast.types.as_record(ty).expect("a class qualifier")),
        n => panic!("expected a type qualifier, found {n:?}"),
    }
}

/// [expr.ref]/7, [class.access.base]/5: an overloaded member access of a
/// non-dependent object in a template, whose call stays dependent, keeps
/// its naming class (the object's class) without a qualifier; one the
/// call resolves keeps it too.
#[test]
fn non_dependent_object() {
    let (ast, i) = parse(
        "struct obj { int f(int); int f(long); };\n\
         template <class T> int g(T t) { obj o; return o.f(t); }\n\
         int h() { obj o; return o.f(1); }\n",
    );
    let got = overloaded(&ast, &i, "f");
    assert_eq!(got.len(), 2, "{got:?}");
    for m in got {
        assert_eq!(m.qualifier, None);
        assert_eq!(m.template_args, None);
        assert_eq!(name(&ast, &i, m.naming_class.expect("a naming class")), "obj");
    }
}

/// An implicit `this->f` in a member template: the naming class is the
/// class of `*this`.
#[test]
fn implicit_this() {
    let (ast, i) = parse(
        "struct C {\n\
           int f(int); int f(long);\n\
           template <class T> int g(T t) { return f(t) + this->f(t); }\n\
         };\n",
    );
    let got = overloaded(&ast, &i, "f");
    assert_eq!(got.len(), 2, "{got:?}");
    for m in got {
        assert_eq!(m.qualifier, None);
        assert_eq!(name(&ast, &i, m.naming_class.expect("a naming class")), "C");
    }
}

/// [basic.lookup.qual]/1, [temp.arg.explicit]/1: `p->Base::f<int>(x)` on
/// a non-dependent `Derived *` keeps its qualifier, its explicit template
/// arguments and the qualifier's class as naming class; with dependent
/// arguments (`p->Base::f<T>(x)`) the access is a
/// `CXXDependentScopeMemberExpr`, and the query gives its qualifier and
/// arguments without a naming class.
#[test]
fn qualified_template_id() {
    let (ast, i) = parse(
        "struct Base { template <class U> int f(U); int f(int, int); };\n\
         struct Derived : Base { int f(long); };\n\
         template <class T> int g(Derived *p, T x) {\n\
           return p->Base::f<int>(x) + p->Base::f<T>(x);\n\
         }\n",
    );
    // The second access was overloaded until its dependent arguments made
    // it dependent; that first form is not in the tree any more.
    let got = overloaded(&ast, &i, "f");
    assert_eq!(got.len(), 2, "{got:?}");
    let m = *got.iter().find(|m| m.template_args.is_some()).expect("`p->Base::f<int>`");
    assert_eq!(qualifier_class(&ast, &i, &m), "Base");
    let targs = m.template_args.expect("explicit template arguments");
    assert_eq!(ast.types.args(targs).len(), 1);
    assert_eq!(name(&ast, &i, m.naming_class.expect("a naming class")), "Base");
    let dependent: Vec<MemberAccessName> = exprs(&ast)
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CXXDependentScopeMemberExpr)
        .map(|e| ast.member_access_name(e).expect("a member access"))
        .collect();
    assert_eq!(dependent.len(), 1, "{dependent:?}");
    assert_eq!(qualifier_class(&ast, &i, &dependent[0]), "Base");
    assert!(dependent[0].template_args.is_some());
    assert_eq!(dependent[0].naming_class, None);
}

/// [temp.dep.candidate]/1: a dependent `a + b` keeps the operator
/// functions the unqualified lookup found at the definition (one in a
/// namespace that is not associated with the operands, through a
/// using-declaration too), not one declared after it; a lookup that
/// found nothing is an empty set; equal sets are stored once.
#[test]
fn dependent_operator_definition_set() {
    let (ast, i) = parse(
        "namespace N { struct S {}; }\n\
         namespace U { int operator-(N::S, int); }\n\
         using U::operator-;\n\
         int operator+(N::S, N::S);\n\
         template <class X> int f(X a, X b) { return a + b; }\n\
         template <class X> int g(X a, X b) { return (a + b) + (a - 1); }\n\
         template <class X> int h(X a) { return a * 1; }\n\
         int operator+(N::S, int);\n\
         int operator*(N::S, int);\n",
    );
    let op = |k: ExprKind, spelled: &str| -> Vec<Vec<(String, bool)>> {
        exprs(&ast)
            .filter(|&e| ast.exprs.get(e).kind() == k)
            .filter_map(|e| ast.dependent_oper_candidates(e))
            .map(|cs| {
                cs.iter().map(|c| (name(&ast, &i, c.decl), c.found != c.decl)).collect::<Vec<_>>()
            })
            .filter(|cs| cs.is_empty() || cs.iter().all(|(n, _)| n == spelled))
            .collect()
    };
    let plus = op(ExprKind::BinaryOperator, "operator+");
    // `a + b` in `f`, twice in `g`... the sets of `+` hold the one
    // `operator+` declared before the templates.
    let plus: Vec<_> = plus.into_iter().filter(|cs| !cs.is_empty()).collect();
    assert!(plus.len() >= 2, "{plus:?}");
    for cs in &plus {
        assert_eq!(cs, &[("operator+".to_owned(), false)]);
    }
    let minus = op(ExprKind::BinaryOperator, "operator-");
    let minus: Vec<_> = minus.into_iter().filter(|cs| !cs.is_empty()).collect();
    assert_eq!(minus, [[("operator-".to_owned(), true)]], "found through the using-declaration");
    // `a * 1`: nothing at the definition.
    let times: Vec<ExprId> = exprs(&ast)
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::BinaryOperator)
        .filter(|&e| ast.dependent_oper_candidates(e).is_some_and(<[_]>::is_empty))
        .collect();
    assert_eq!(times.len(), 1);
    let ol = &ast.templates.oper_lookups;
    assert!(ol.set_count() < ol.len(), "equal sets are interned once");
}

/// [temp.variadic]/11, [temp.dep.candidate]/1: a fold-expression keeps
/// the set its operator's unqualified lookup found at the definition, as
/// a dependent binary operator does, for tooling clients (spec/ast.md
/// §8.5; WP-2625); its rewritten forms' names too (`!=` from `==`).
#[test]
fn fold_operator_definition_set() {
    let (ast, i) = parse(
        "namespace N { struct S {}; }\n\
         N::S operator+(N::S, N::S);\n\
         bool operator==(N::S, N::S);\n\
         template <class... X> auto f(X... a) { return (a + ...); }\n\
         template <class... X> bool g(X... a) { return (... != a); }\n\
         template <class... X> auto h(X... a) { return (1 * ... * a); }\n\
         N::S operator*(int, N::S);\n",
    );
    let sets: Vec<Vec<String>> = exprs(&ast)
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CXXFoldExpr)
        .map(|e| {
            let cs = ast.dependent_oper_candidates(e).expect("a fold records its set");
            cs.iter().map(|c| name(&ast, &i, c.decl)).collect()
        })
        .collect();
    assert_eq!(
        sets,
        [vec!["operator+".to_owned()], vec!["operator==".to_owned()], vec![]],
        "`+`; `!=` finds its rewritten form's `==`; `*` declared after the definition"
    );
}

/// [basic.scope.scope]/5 (P2169): a non-static data member `_` is
/// name-independent unless it is a member of an anonymous union (or
/// anonymous struct), also in an instantiation; a member of a named
/// union, a later `_` of the class and an automatic variable are
/// (WP-2477).
#[test]
fn anonymous_union_member_is_not_name_independent() {
    let (ast, i) = parse(
        "struct S { union { int _; }; int _; union U { int _; } u; };\n\
         template <class T> struct Q { union { T _; }; T _; };\n\
         Q<int> q;\n\
         void f() { union { int _; }; int _ = 1; }\n",
    );
    let mut got: Vec<(bool, bool)> = Vec::new();
    for d in ast.decls.ids() {
        let n = ast.decls.get(d);
        if !matches!(n.kind(), kiln_ast::decl::DeclKind::Field | kiln_ast::decl::DeclKind::Var)
            || name(&ast, &i, d) != "_"
        {
            continue;
        }
        let in_anonymous = n
            .ctx()
            .and_then(|c| ast.decls.record(c))
            .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::ANONYMOUS));
        got.push((in_anonymous, n.misc().name_independent()));
    }
    assert!(got.len() >= 8, "{got:?}");
    for (in_anonymous, independent) in got {
        assert_eq!(independent, !in_anonymous);
    }
}
