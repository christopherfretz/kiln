//! Written nested-name-specifiers on declarations (`Ast::decl_qualifier`,
//! spec/ast.md §4.2, WP-3690): declarator-ids (out-of-line members, static
//! data members, nested classes, namespace-qualified definitions, explicit
//! specializations, member templates, qualified friends), `using enum`,
//! `using` (consistent with `Ast::using_qualifier`), and explicit
//! instantiations of members, classes, and templates.
#![forbid(unsafe_code)]

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::consts::Scalar;
use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::dump::AstNames;
use kiln_ast::ids::DeclId;
use kiln_ast::types::{ArgView, Nns, TypeView, write_nns};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

fn parse(src: &str) -> (Ast, SourceManager) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    assert_eq!(engine.error_count(), 0, "unexpected parse/sema errors");
    drop(engine);
    (tu.ast, sm)
}

fn line_of(ast: &Ast, sm: &SourceManager, d: DeclId) -> u32 {
    let loc = ast.decls.get(d).loc();
    sm.line_col(loc).map_or(0, |lc| lc.line)
}

fn qual_str(ast: &Ast, d: DeclId) -> Option<String> {
    let nns = ast.decl_qualifier(d)?;
    let names = AstNames::new(ast);
    let mut out = String::new();
    write_nns(&ast.types, &names, nns, &mut out);
    Some(out)
}

fn decl_at(ast: &Ast, sm: &SourceManager, line: u32, kind: DeclKind) -> DeclId {
    ast.decls
        .ids()
        .find(|&d| ast.decls.get(d).kind() == kind && line_of(ast, sm, d) == line)
        .unwrap_or_else(|| panic!("no {kind:?} at line {line}"))
}

const ALL_SHAPES: &str = "\
template <class T, int> struct P { void f(); static int s; };
namespace n {
  constexpr int k = 1, j = 1;
  void g();
  void g2();
  void h();
  template <class T> void ft() {}
  template <class T> int vt = 0;
}
namespace na = n;
template <class T> struct P<T, n::j> { void f(); static int s; };
template <class T> void P<T, n::k>::f() {}
void n::g() {}
void ::n::g2() {}
void na::h() {}
template <int> struct Q {
  static int s;
  enum E { e };
  void m();
  struct Inner;
  template <class U> void tm(U);
};
template <int N> int Q<N>::s = 0;
template <int N> void Q<N>::m() {}
template <int N> struct Q<N>::Inner { int x; };
template <int N> template <class U> void Q<N>::tm(U) {}
constexpr int make(int) { return 1; }
template <> int Q<sizeof(make(0))>::s = 5;
template <> void Q<sizeof(make(0))>::m() {}
struct Pl { static int s; static int t; };
template <int> using AL = Pl;
int AL<sizeof(make(0))>::s = 2;
typedef Pl TD;
int TD::t = 3;
using enum Q<sizeof(make(0))>::E;
enum LocalE { le };
using enum LocalE;
using n::g;
template int Q<2>::s;
template void Q<2>::m();
template struct Q<2>::Inner;
template struct Q<3>;
template void n::ft<int>();
template int n::vt<int>;
namespace m {
  template <class T> struct X {};
  struct F { void h(); };
}
template struct m::X<int>;
struct Host {
  friend void m::F::h();
  friend struct m::X<int>;
  friend struct m::F;
  friend void uf();
};
";

#[test]
#[allow(clippy::too_many_lines)]
fn written_qualifiers_for_declarators_using_enum_friends_and_insts() {
    let (ast, sm) = parse(ALL_SHAPES);

    // Out-of-line member of a partial specialization: keeps `P<T, n::k>::`,
    // not the canonical/partial's `P<T, n::j>::` or `P<T, 1>::`.
    let pf = decl_at(&ast, &sm, 12, DeclKind::CXXMethod);
    assert_eq!(qual_str(&ast, pf).as_deref(), Some("P<T, n::k>::"));
    assert_eq!(qual_str(&ast, ast.first(pf)), None);

    // Namespace-qualified, global-qualified, and namespace-alias-qualified defs.
    assert_eq!(qual_str(&ast, decl_at(&ast, &sm, 13, DeclKind::Function)).as_deref(), Some("n::"));
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 14, DeclKind::Function)).as_deref(),
        Some("::n::")
    );
    assert_eq!(qual_str(&ast, decl_at(&ast, &sm, 15, DeclKind::Function)).as_deref(), Some("na::"));
    // Unqualified in-namespace declarations stay `None`.
    assert_eq!(qual_str(&ast, decl_at(&ast, &sm, 4, DeclKind::Function)), None);

    // Out-of-line static data member, member function, nested class, and member template.
    assert_eq!(qual_str(&ast, decl_at(&ast, &sm, 23, DeclKind::Var)).as_deref(), Some("Q<N>::"));
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 24, DeclKind::CXXMethod)).as_deref(),
        Some("Q<N>::")
    );
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 25, DeclKind::CXXRecord)).as_deref(),
        Some("Q<N>::")
    );
    let tm_pat = decl_at(&ast, &sm, 26, DeclKind::CXXMethod);
    let tm_tmpl = ast.described_template(tm_pat).expect("member function template");
    assert_eq!(qual_str(&ast, tm_pat).as_deref(), Some("Q<N>::"));
    assert_eq!(qual_str(&ast, tm_tmpl).as_deref(), Some("Q<N>::"));
    assert_eq!(qual_str(&ast, ast.first(tm_tmpl)), None);

    // Explicit specializations of members with an unevaluated expression arg.
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 28, DeclKind::Var)).as_deref(),
        Some("Q<sizeof (make(0))>::")
    );
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 29, DeclKind::CXXMethod)).as_deref(),
        Some("Q<sizeof (make(0))>::")
    );

    // Alias template specialization and typedef kept as written.
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 32, DeclKind::Var)).as_deref(),
        Some("AL<sizeof (make(0))>::")
    );
    assert_eq!(qual_str(&ast, decl_at(&ast, &sm, 34, DeclKind::Var)).as_deref(), Some("TD::"));

    // `using enum` with and without a qualifier, plus `using` consistency.
    assert_eq!(
        qual_str(&ast, decl_at(&ast, &sm, 35, DeclKind::UsingEnum)).as_deref(),
        Some("Q<sizeof (make(0))>::")
    );
    assert_eq!(qual_str(&ast, decl_at(&ast, &sm, 37, DeclKind::UsingEnum)), None);
    let using_g = decl_at(&ast, &sm, 38, DeclKind::Using);
    assert_eq!(ast.decl_qualifier(using_g), ast.using_qualifier(using_g));
    assert_eq!(qual_str(&ast, using_g).as_deref(), Some("n::"));

    // Explicit instantiations of members of `Q<2>`, unqualified `Q<3>`,
    // qualified `m::X<int>`, and qualified `n::ft<int>` / `n::vt<int>`.
    let q_tmpl = decl_at(&ast, &sm, 16, DeclKind::ClassTemplate);
    let q_specs = ast.templates.specs_of(q_tmpl);
    let find_q = |want_val: i128| -> DeclId {
        q_specs
            .iter()
            .copied()
            .find(|&s| {
                let Some((_, si)) = ast.spec_info(s) else { return false };
                let args = ast.types.args(si.args);
                args.first().is_some_and(|a| match a.view() {
                    ArgView::Value { value, .. } => matches!(
                        ast.consts.scalar(value),
                        Some(Scalar::Int { bits, .. }) if bits.to_i128() == Some(want_val)
                    ),
                    _ => false,
                })
            })
            .unwrap_or_else(|| panic!("no Q<{want_val}> specialization"))
    };
    let q2 = find_q(2);
    let member_of = |rec: DeclId, name: &str| -> DeclId {
        ast.decls
            .members(rec)
            .find(|&d| {
                ast.decls.get(d).name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
            })
            .unwrap_or_else(|| panic!("no member {name}"))
    };
    assert_eq!(qual_str(&ast, member_of(q2, "s")).as_deref(), Some("Q<2>::"));
    assert_eq!(qual_str(&ast, member_of(q2, "m")).as_deref(), Some("Q<2>::"));
    assert_eq!(qual_str(&ast, member_of(q2, "Inner")).as_deref(), Some("Q<2>::"));

    let q3 = find_q(3);
    assert_eq!(qual_str(&ast, q3), None);
    assert_eq!(qual_str(&ast, member_of(q3, "s")), None);

    let ft_tmpl = decl_at(&ast, &sm, 7, DeclKind::FunctionTemplate);
    let ft_int = ast.templates.specs_of(ft_tmpl)[0];
    assert_eq!(qual_str(&ast, ft_int).as_deref(), Some("n::"));

    let vt_tmpl = decl_at(&ast, &sm, 8, DeclKind::VarTemplate);
    let vt_int = ast.templates.specs_of(vt_tmpl)[0];
    assert_eq!(qual_str(&ast, vt_int).as_deref(), Some("n::"));

    let x_tmpl = decl_at(&ast, &sm, 46, DeclKind::ClassTemplate);
    let x_int = ast.templates.specs_of(x_tmpl)[0];
    assert_eq!(qual_str(&ast, x_int).as_deref(), Some("m::"));

    // Qualified and unqualified friends.
    let host = decl_at(&ast, &sm, 50, DeclKind::CXXRecord);
    let friends: Vec<DeclId> =
        ast.decls.members(host).filter(|&d| ast.decls.get(d).kind() == DeclKind::Friend).collect();
    assert_eq!(friends.len(), 4);
    assert_eq!(qual_str(&ast, friends[0]).as_deref(), Some("m::F::"));
    if let FriendTarget::Decl(befriended) = ast.decls.friend_target(friends[0]) {
        assert_eq!(qual_str(&ast, befriended).as_deref(), Some("m::F::"));
        assert_eq!(qual_str(&ast, ast.first(befriended)), None);
    } else {
        panic!("expected friend decl target");
    }
    assert_eq!(qual_str(&ast, friends[1]).as_deref(), Some("m::"));
    assert_eq!(qual_str(&ast, friends[2]).as_deref(), Some("m::"));
    assert_eq!(qual_str(&ast, friends[3]), None);
}

/// Two declarations of the same explicit specialization (`Q<1>::s` and
/// `Q<sizeof(one(0))>::s`, both `Q<1>` after conversion) name one entity
/// yet keep their two distinct written qualifiers.
#[test]
fn two_specializations_same_entity_keep_distinct_written_qualifiers() {
    let src = "\
template <int> struct Q { static int s; };
constexpr char one(int) { return 1; }
template <> int Q<1>::s;
template <> int Q<sizeof(one(0))>::s = 5;
";
    let (ast, sm) = parse(src);
    let d1 = decl_at(&ast, &sm, 3, DeclKind::Var);
    let d2 = decl_at(&ast, &sm, 4, DeclKind::Var);
    assert_ne!(d1, d2);
    assert_eq!(ast.first(d1), ast.first(d2), "both declarations name Q<1>::s");
    let q1 = ast.decl_qualifier(d1).expect("Q<1>::");
    let q2 = ast.decl_qualifier(d2).expect("Q<sizeof (one(0))>::");
    assert_ne!(q1, q2, "distinct written qualifiers");
    assert!(ast.types.same_nns(q1, q2), "same canonical qualifier");
    assert_eq!(qual_str(&ast, d1).as_deref(), Some("Q<1>::"));
    assert_eq!(qual_str(&ast, d2).as_deref(), Some("Q<sizeof (one(0))>::"));
}

/// The returned `Nns` keeps full sugar: alias template specialization (`AL`),
/// typedef (`TD`), namespace alias (`na`), and written expression template
/// arguments (`ArgView::Expr`), never only canonical/converted forms.
#[test]
fn alias_template_typedef_and_written_expr_args_keep_sugar() {
    let (ast, sm) = parse(ALL_SHAPES);

    let al_var = decl_at(&ast, &sm, 32, DeclKind::Var);
    let al_nns = ast.decl_qualifier(al_var).expect("AL<...>::");
    let Nns::Type { ty: al_ty, .. } = ast.types.nns_view(al_nns) else {
        panic!("expected Nns::Type for AL<...>::");
    };
    let TypeView::TemplateSpecialization { alias, args, .. } = ast.types.view(al_ty.ty()) else {
        panic!("expected TemplateSpecialization for AL<...>::");
    };
    assert!(alias, "AL<...>:: must keep alias template specialization sugar");
    assert!(
        matches!(ast.types.args(args)[0].view(), ArgView::Expr { .. }),
        "AL<sizeof(make(0))>:: must keep expression argument as written"
    );

    let td_var = decl_at(&ast, &sm, 34, DeclKind::Var);
    let td_nns = ast.decl_qualifier(td_var).expect("TD::");
    let Nns::Type { ty: td_ty, .. } = ast.types.nns_view(td_nns) else {
        panic!("expected Nns::Type for TD::");
    };
    assert!(
        matches!(ast.types.view(td_ty.ty()), TypeView::Typedef { .. }),
        "TD:: must keep typedef sugar"
    );

    let na_fn = decl_at(&ast, &sm, 15, DeclKind::Function);
    let na_nns = ast.decl_qualifier(na_fn).expect("na::");
    assert!(
        matches!(ast.types.nns_view(na_nns), Nns::NamespaceAlias { .. }),
        "na:: must keep namespace alias sugar"
    );

    let q_spec_var = decl_at(&ast, &sm, 28, DeclKind::Var);
    let q_nns = ast.decl_qualifier(q_spec_var).expect("Q<sizeof (make(0))>::");
    let Nns::Type { ty: q_ty, .. } = ast.types.nns_view(q_nns) else {
        panic!("expected Nns::Type for Q<sizeof (make(0))>::");
    };
    let TypeView::TemplateSpecialization { alias: false, args: q_args, .. } =
        ast.types.view(q_ty.ty())
    else {
        panic!("expected class TemplateSpecialization for Q<sizeof (make(0))>::");
    };
    assert!(
        matches!(ast.types.args(q_args)[0].view(), ArgView::Expr { .. }),
        "Q<sizeof(make(0))>:: must keep expression argument as written"
    );
}
