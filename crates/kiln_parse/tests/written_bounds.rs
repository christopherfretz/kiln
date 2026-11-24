//! Written array bounds and written template arguments (spec/types.md
//! §7.2 "Written bounds" and §8.4, spec/ast.md §8.3 "Arguments as
//! written"; WP-3015, WP-3016, WP-3142): a walk from a declaration (its
//! type, its initializer, the types its expressions hold, its arguments
//! as written) reaches every call written in it, also once a constant
//! bound or a non-type argument folded to a value; the canonical types
//! and the converted arguments are unchanged ([dcl.array]/1,
//! [temp.spec.partial.general], [temp.expl.spec]).

use std::sync::Arc;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::templates::SpecKind;
use kiln_ast::types::ArgView;
use kiln_ast::{Ast, DeclId, ExprId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// One shape per line: the walk from the declaration on line `n` finds
/// the call written on line `n`.
const SRC: &str = "\
constexpr int f(int x) { return x; }
int arr[f(2)];
int two[2];
unsigned long sz = sizeof(int[f(13)]);
void par(int (&xr)[f(19)]);
auto *nw = new int[f(7)][f(3)];
struct M { int m[f(4)]; };
template <class T> struct TA { T a[f(5)]; };
TA<int> ta;
template <class T, int N> struct P {};
template <class T> struct P<T, f(1)> {};
template <> struct P<int, f(2)> {};
template <class T, int N> int v = 0;
template <class T> int v<T, f(3)> = 1;
template <> int v<int, f(4)> = 2;
int g(...);
template <class T> struct S {
  template <int N> void h();
  template <> void h<sizeof(sizeof(g(T())))>() {}
};
S<int> si;
template <int N> void fh() {}
template <> void fh<f(6)>() {}
auto cast = (int (*)[f(8)])nullptr;
";

struct Tu {
    src: String,
    sm: SourceManager,
    parsed: kiln_parse::ParsedTu,
}

fn parse(src: &str) -> Tu {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions::default();
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors = engine.error_count();
    drop(engine);
    let msgs: Vec<String> = sink.take().into_iter().map(|d| d.message).collect();
    assert_eq!(errors, 0, "the source has errors: {msgs:?}");
    Tu { src: src.to_owned(), sm, parsed: tu }
}

impl Tu {
    fn ast(&self) -> &Ast {
        &self.parsed.ast
    }

    /// The declarations named `name` of kind `kind`, in order.
    fn named(&self, name: &str, kind: DeclKind) -> Vec<DeclId> {
        let ast = self.ast();
        (1..=ast.decls.len() as u32)
            .map(DeclId::new)
            .filter(|&d| {
                let n = ast.decls.get(d);
                n.kind() == kind
                    && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
            })
            .collect()
    }

    fn one(&self, name: &str, kind: DeclKind) -> DeclId {
        let all = self.named(name, kind);
        assert_eq!(all.len(), 1, "{kind:?} '{name}': {all:?}");
        all[0]
    }

    /// The identifier written at expression `e`'s location.
    fn ident_at(&self, e: ExprId) -> &str {
        let Some(lc) = self.sm.line_col(self.ast().exprs.get(e).loc()) else { return "" };
        let line = self.src.lines().nth(lc.line as usize - 1).unwrap_or("");
        let rest = line.get(lc.col as usize - 1..).unwrap_or("");
        let n = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(rest.len());
        &rest[..n]
    }

    fn line(&self, e: ExprId) -> u32 {
        self.sm.line_col(self.ast().exprs.get(e).loc()).map_or(0, |lc| lc.line)
    }

    /// Every expression reachable from `roots`: their subexpressions and
    /// the expressions written in the types they hold.
    fn reach(&self, roots: Vec<ExprId>) -> Vec<ExprId> {
        let ast = self.ast();
        let mut seen = Vec::new();
        let mut todo = roots;
        while let Some(e) = todo.pop() {
            if seen.contains(&e) {
                continue;
            }
            seen.push(e);
            let x = ast.exprs.get(e);
            let held = match x.kind() {
                ExprKind::CXXNewExpr => Some(ast.exprs.new_extra(e).allocated_ty),
                ExprKind::UnaryExprOrTypeTraitExpr => ast.exprs.trait_type_arg(e),
                // A C-style cast's type is its type as written.
                ExprKind::CStyleCastExpr => Some(x.ty()),
                _ => ast.exprs.cast_written_type(e),
            };
            if let Some(t) = held {
                ast.types.written_exprs(t, &mut todo);
            }
            todo.extend(ast.exprs.children(e, &ast.lists));
        }
        seen
    }

    /// The lines of the calls of a function named `callee` among the
    /// expressions reachable from `roots`.
    fn calls(&self, roots: Vec<ExprId>, callee: &str) -> Vec<u32> {
        let ast = self.ast();
        let mut lines: Vec<u32> = self
            .reach(roots)
            .into_iter()
            .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CallExpr)
            .filter(|&e| {
                // The callee as written (a `DeclRefExpr`, or an
                // `UnresolvedLookupExpr` in a template) names `callee`.
                self.reach(vec![ast.exprs.callee(e)]).into_iter().any(|r| {
                    matches!(
                        ast.exprs.get(r).kind(),
                        ExprKind::DeclRefExpr | ExprKind::UnresolvedLookupExpr
                    ) && self.ident_at(r) == callee
                })
            })
            .map(|e| self.line(e))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// The walk from declaration `d`: its type and its initializer.
    fn decl_roots(&self, d: DeclId) -> Vec<ExprId> {
        let ast = self.ast();
        let mut roots = Vec::new();
        if let Some(t) = ast.decls.get(d).ty() {
            ast.types.written_exprs(t, &mut roots);
        }
        roots.extend(ast.decls.var(d).and_then(|v| v.init));
        roots
    }

    /// The expressions of `d`'s template arguments as written.
    fn written_arg_roots(&self, d: DeclId) -> Vec<ExprId> {
        let ast = self.ast();
        let w = ast.templates.args_as_written(d).expect("arguments as written");
        let mut roots = Vec::new();
        ast.types.written_arg_exprs(w, &mut roots);
        roots
    }
}

/// [dcl.array]/1: a constant bound keeps its written expression on the
/// declaration's type, and the canonical type is the value's.
#[test]
fn a_variable_bound_keeps_its_call() {
    let t = parse(SRC);
    let ast = t.ast();
    let arr = t.one("arr", DeclKind::Var);
    assert_eq!(t.calls(t.decl_roots(arr), "f"), [2]);
    let two = t.one("two", DeclKind::Var);
    let (ta, tb) = (ast.decls.get(arr).ty().unwrap(), ast.decls.get(two).ty().unwrap());
    assert_ne!(ta, tb, "the written bound is sugar");
    assert_eq!(ast.types.canonical(ta), ast.types.canonical(tb), "one type, int[2]");
    assert_eq!(ast.types.array_size(ta), Some(2));
    assert!(!ast.types.is_dependent(ta));
    // The plain bound `2` is kept as written too (an integer literal).
    let mut roots = Vec::new();
    ast.types.written_exprs(tb, &mut roots);
    assert_eq!(roots.len(), 1);
    assert_eq!(ast.exprs.get(roots[0]).kind(), ExprKind::IntegerLiteral);
}

/// [expr.sizeof], [expr.new]/6, [expr.cast]: the type operand of
/// `sizeof`, a cast and a `new` hold the written bound; the first bound
/// of `new` is the expression's operand.
#[test]
fn type_operands_keep_their_calls() {
    let t = parse(SRC);
    let sz = t.one("sz", DeclKind::Var);
    assert_eq!(t.calls(t.decl_roots(sz), "f"), [4]);
    let nw = t.one("nw", DeclKind::Var);
    assert_eq!(t.calls(t.decl_roots(nw), "f"), [6]);
    // Both bounds of `new int[f(7)][f(3)]`.
    let ast = t.ast();
    let n = t
        .reach(t.decl_roots(nw))
        .into_iter()
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CallExpr)
        .count();
    assert_eq!(n, 2, "f(7) and f(3)");
    let cast = t.one("cast", DeclKind::Var);
    assert_eq!(t.calls(t.decl_roots(cast), "f"), [24]);
}

/// [dcl.fct]/5 (a reference to an array is not adjusted), [class.mem]:
/// a parameter's and a member's written bound.
#[test]
fn parameter_and_member_bounds_keep_their_calls() {
    let t = parse(SRC);
    let x = t.one("xr", DeclKind::ParmVar);
    assert_eq!(t.calls(t.decl_roots(x), "f"), [5]);
    let m = t.one("m", DeclKind::Field);
    assert_eq!(t.calls(t.decl_roots(m), "f"), [7]);
}

/// [temp.inst]: the pattern's bound and the instantiation's, which keeps
/// the pattern's expression (it does not depend on `T`).
#[test]
fn a_template_member_bound_in_pattern_and_instantiation() {
    let t = parse(SRC);
    let ast = t.ast();
    let fields = t.named("a", DeclKind::Field);
    assert_eq!(fields.len(), 2, "the pattern's and TA<int>'s");
    for &a in &fields {
        assert_eq!(t.calls(t.decl_roots(a), "f"), [8], "{a:?}");
    }
    let inst = ast.decls.get(fields[1]).ty().unwrap();
    assert!(!ast.types.is_dependent(inst));
    assert_eq!(ast.types.array_size(inst), Some(5));
}

/// [temp.spec.partial.general]/8, [temp.expl.spec]: the arguments as
/// written keep the non-type argument's expression; the converted ones,
/// which matching uses, its value.
#[test]
fn specialization_arguments_keep_their_calls() {
    let t = parse(SRC);
    let ast = t.ast();
    let records = t.named("P", DeclKind::CXXRecord);
    let partial = *records
        .iter()
        .find(|&&d| ast.partial_spec_info(d).is_some())
        .expect("the partial specialization");
    assert_eq!(t.calls(t.written_arg_roots(partial), "f"), [11]);
    let (_, info) = ast.partial_spec_info(partial).unwrap();
    assert!(
        matches!(ast.types.args(info.args)[1].view(), ArgView::Value { .. }),
        "the converted argument is the value"
    );
    let explicit = *records
        .iter()
        .find(|&&d| {
            ast.spec_info(d).is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
        })
        .expect("the explicit specialization");
    assert_eq!(t.calls(t.written_arg_roots(explicit), "f"), [12]);
    let vars = t.named("v", DeclKind::Var);
    let vpartial = *vars.iter().find(|&&d| ast.partial_spec_info(d).is_some()).unwrap();
    assert_eq!(t.calls(t.written_arg_roots(vpartial), "f"), [14]);
    let vexplicit = *vars
        .iter()
        .find(|&&d| {
            ast.spec_info(d).is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
        })
        .unwrap();
    assert_eq!(t.calls(t.written_arg_roots(vexplicit), "f"), [15]);
    let fh = t.named("fh", DeclKind::Function);
    let fexplicit = *fh
        .iter()
        .find(|&&d| ast.templates.args_as_written(d).is_some())
        .expect("the explicit specialization's declaration");
    assert_eq!(t.calls(t.written_arg_roots(fexplicit), "f"), [23]);
}

/// [temp.expl.spec]/2, [temp.dep.constexpr]: a class-scope explicit
/// specialization in a class template keeps its instantiation-dependent
/// argument as an expression; its `SpecInfo` keeps the canonical form.
#[test]
fn a_class_scope_explicit_specialization_keeps_its_arguments() {
    let t = parse(SRC);
    let ast = t.ast();
    let hs = t.named("h", DeclKind::CXXMethod);
    let pattern_spec = *hs
        .iter()
        .find(|&&d| ast.templates.args_as_written(d).is_some())
        .expect("the explicit specialization in S");
    assert_eq!(t.calls(t.written_arg_roots(pattern_spec), "g"), [19]);
    let w = ast.templates.args_as_written(pattern_spec).unwrap();
    assert!(matches!(ast.types.args(w)[0].view(), ArgView::Expr { .. }), "not the value 8");
    // The converted argument is the canonical (instantiation-dependent)
    // expression's profile, which matching uses.
    let (_, info) = ast.spec_info(pattern_spec).expect("a specialization");
    assert!(matches!(ast.types.args(info.args)[0].view(), ArgView::Profile(_)));
    // `S<int>` redeclares it as a specialization of its member template
    // (CWG 727), the argument substituted: the value 8.
    let inst = hs
        .iter()
        .filter(|&&d| ast.decls.get(d).ctx() != ast.decls.get(pattern_spec).ctx())
        .filter_map(|&d| ast.spec_info(d))
        .find(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
        .expect("the specialization in S<int>");
    assert!(matches!(ast.types.args(inst.1.args)[0].view(), ArgView::Value { .. }));
}

/// Two writings of one value are one specialization: matching stays on
/// the converted arguments, the written ones differ.
#[test]
fn written_arguments_do_not_change_matching() {
    let t = parse(
        "constexpr int f(int x) { return x; }\n\
         template <class T, int N> struct P { static constexpr int k = 0; };\n\
         template <class T> struct P<T, f(1)> { static constexpr int k = 1; };\n\
         static_assert(P<int, 1>::k == 1);\n\
         template <> struct P<char, f(2)> { static constexpr int k = 2; };\n\
         static_assert(P<char, 2>::k == 2);\n\
         int a[f(3)];\n\
         extern int a[3];\n",
    );
    let ast = t.ast();
    let a = t.named("a", DeclKind::Var);
    assert_eq!(a.len(), 2);
    let (t0, t1) = (ast.decls.get(a[0]).ty().unwrap(), ast.decls.get(a[1]).ty().unwrap());
    assert_eq!(ast.types.canonical(t0), ast.types.canonical(t1));
}
