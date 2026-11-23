//! The resolved designator of `__builtin_offsetof` ([support.types.layout]/1;
//! spec/ast.md §5, "`OffsetOfExpr`"; WP-2360): a non-dependent
//! `OffsetOfExpr` keeps its operand type and, per component, the member,
//! the base class stepped through, or the index position, so a tooling
//! client tells apart two expressions with one value.

use std::sync::Arc;

use kiln_ast::expr::{ExprKind, OffsetofNode};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

const SRC: &str = r"
struct A { int x, y; };
struct Bq { int p, q; };
struct Base { int b0; int b1; };
struct D : Base {};
struct Arr { int a[4][3]; };
union U { int i; float f; };
struct An { int pre; struct { int in; }; };
typedef A AliasA;
unsigned long v_a = __builtin_offsetof(A, y);
unsigned long v_b = __builtin_offsetof(Bq, q);
unsigned long v_base = __builtin_offsetof(D, b1);
unsigned long v_arr = __builtin_offsetof(Arr, a[2][1]);
unsigned long v_ui = __builtin_offsetof(U, i);
unsigned long v_uf = __builtin_offsetof(U, f);
unsigned long v_anon = __builtin_offsetof(An, in);
unsigned long v_alias = __builtin_offsetof(AliasA, y);
template <class T> unsigned long tf() { return __builtin_offsetof(T, q); }
unsigned long v_inst = tf<Bq>();
int n;
unsigned long v_run = __builtin_offsetof(Arr, a[n][1]);
";

fn parse(src: &str) -> (Ast, u32) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions::default();
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    (tu.ast, engine.error_count())
}

fn ident(ast: &Ast, d: DeclId) -> &str {
    ast.decls.get(d).name().as_ident().map_or("", |s| ast.symbols().as_str(s))
}

/// The named class or union a type is (`?` for none).
fn class_name(ast: &Ast, t: QualType) -> String {
    ast.types.as_record(t).map_or_else(|| "?".to_owned(), |d| ident(ast, d).to_owned())
}

/// A designator as text: `Base(Base)`, `.name`, `[position]`.
fn shown(ast: &Ast, nodes: &[OffsetofNode]) -> String {
    nodes
        .iter()
        .map(|n| match *n {
            OffsetofNode::Base { class } => format!("Base({})", ident(ast, class)),
            OffsetofNode::Field { field, .. } => format!(".{}", ident(ast, field)),
            OffsetofNode::Index { position, .. } => format!("[{position}]"),
        })
        .collect()
}

/// Every `OffsetOfExpr`, in creation order.
fn offsetofs(ast: &Ast) -> Vec<ExprId> {
    (1..=ast.exprs.len())
        .map(|i| ExprId::new(u32::try_from(i).expect("expression index")))
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::OffsetOfExpr)
        .collect()
}

/// [support.types.layout]/1: each non-dependent `offsetof` keeps its
/// operand type as written and its designator as resolved; two with one
/// value (`A::y` and `Bq::q`, a union's two members) differ in their row.
#[test]
fn designators_are_kept_support_types_layout_1() {
    let (ast, errors) = parse(SRC);
    assert_eq!(errors, 0);
    let rows: Vec<(String, String, Option<u64>)> = offsetofs(&ast)
        .into_iter()
        .filter_map(|e| {
            let (t, nodes) = ast.exprs.offsetof_designator(e)?;
            let value = ast.exprs.offsetof_layout(e).is_none().then(|| ast.exprs.offsetof_value(e));
            Some((class_name(&ast, t), shown(&ast, nodes), value))
        })
        .collect();
    let want: Vec<(String, String, Option<u64>)> = [
        ("A", ".y", Some(4)),
        ("Bq", ".q", Some(4)),
        ("D", "Base(Base).b1", Some(4)),
        ("Arr", ".a[0][1]", Some(28)),
        ("U", ".i", Some(0)),
        ("U", ".f", Some(0)),
        ("An", "..in", Some(4)),
        ("A", ".y", Some(4)),
        // An index that is not a constant: computed when evaluated.
        ("Arr", ".a[0][1]", None),
        // The instantiation of `tf<Bq>` (at the end of the unit): the
        // resolved row.
        ("Bq", ".q", Some(4)),
    ]
    .iter()
    .map(|&(t, d, v)| (t.to_owned(), d.to_owned(), v))
    .collect();
    assert_eq!(rows, want);
}

/// The members a row names are the declarations lookup found: the
/// anonymous struct's unnamed member then `in`, a union's two members,
/// the base class's definition; an index names its child.
#[test]
fn designator_entities() {
    let (ast, errors) = parse(SRC);
    assert_eq!(errors, 0);
    let all = offsetofs(&ast);
    let rows: Vec<(ExprId, QualType, &[OffsetofNode])> = all
        .iter()
        .filter_map(|&e| ast.exprs.offsetof_designator(e).map(|(t, n)| (e, t, n)))
        .collect();
    let field = |n: &OffsetofNode| match *n {
        OffsetofNode::Field { field, .. } => Some(field),
        _ => None,
    };
    // A union's two members at one offset are two fields.
    let (ui, uf) = (field(&rows[4].2[0]).unwrap(), field(&rows[5].2[0]).unwrap());
    assert_ne!(ui, uf);
    assert_eq!(ast.decls.get(ui).ctx(), ast.decls.get(uf).ctx());
    // The base: the class `b1` is a member of.
    let OffsetofNode::Base { class } = rows[2].2[0] else { panic!("a base node") };
    let b1 = field(&rows[2].2[1]).unwrap();
    assert_eq!(ast.decls.get(b1).ctx().map(|c| ast.first(c)), Some(ast.first(class)));
    // An anonymous struct's chain: its unnamed member, then `in`.
    let (outer, inner) = (field(&rows[6].2[0]).unwrap(), field(&rows[6].2[1]).unwrap());
    assert_eq!(ident(&ast, outer), "");
    assert_eq!(ident(&ast, inner), "in");
    // The written type is kept (a typedef stays a typedef), its
    // canonical type is the class.
    assert_ne!(rows[7].1, rows[0].1);
    assert_eq!(ast.types.canonical(rows[7].1), ast.types.canonical(rows[0].1));
    // Index positions name the expression's children.
    let idx = ast.exprs.offsetof_indices(rows[3].0);
    assert_eq!(ast.lists.len(idx), 2);
    // The dependent pattern keeps its written form, no resolved row.
    let pattern: Vec<ExprId> =
        all.iter().copied().filter(|&e| ast.exprs.offsetof_pattern(e).is_some()).collect();
    assert_eq!(pattern.len(), 1);
    assert!(ast.exprs.offsetof_designator(pattern[0]).is_none());
}
