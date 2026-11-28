//! Written array bounds and written template arguments (spec/types.md
//! §7.2 "Written bounds" and §8.4, spec/ast.md §8.3 "Arguments as
//! written"; WP-3015, WP-3016, WP-3142): a walk from a declaration (its
//! type, its initializer, the types its expressions hold, its arguments
//! as written) reaches every call written in it, also once a constant
//! bound or a non-type argument folded to a value; the canonical types
//! and the converted arguments are unchanged ([dcl.array]/1,
//! [temp.spec.partial.general], [temp.expl.spec]). A deduced variable's
//! type is no written one (spec/types.md §8.4, WP-3583), and every type
//! operand is kept as written, in plain code and in instances (spec/ast.md
//! §5.11 "Type operands as written", WP-3589).

use std::sync::Arc;

use kiln_ast::decl::{DeclKind, VarPlaceholder};
use kiln_ast::expr::{ExprKind, TypeidOperand};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::ArgView;
use kiln_ast::{Ast, DeclId, ExprId, QualType};
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
    parse_at(src, ParseOptions::default().clang_compat)
}

/// [`parse`] at clang compatibility level `compat`.
fn parse_at(src: &str, compat: u32) -> Tu {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { clang_compat: compat, ..ParseOptions::default() };
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
            let (held, args) = self.type_operands(e);
            for t in held {
                ast.types.written_exprs(t, &mut todo);
            }
            if let Some(l) = args {
                ast.types.written_arg_exprs(l, &mut todo);
            }
            todo.extend(ast.exprs.children(e, &ast.lists));
        }
        seen
    }

    /// The type operands as written of expression `e` (spec/ast.md §5.11,
    /// each through its accessor) and its template arguments as written.
    fn type_operands(&self, e: ExprId) -> (Vec<QualType>, Option<kiln_ast::ArgListId>) {
        let ast = self.ast();
        let x = ast.exprs.get(e);
        let k = x.kind();
        let held = match k {
            ExprKind::CXXNewExpr => vec![ast.exprs.new_extra(e).allocated_ty],
            ExprKind::UnaryExprOrTypeTraitExpr => ast.exprs.trait_type_arg(e).into_iter().collect(),
            // A C-style cast's type is its type as written.
            ExprKind::CStyleCastExpr => vec![x.ty()],
            ExprKind::CXXTypeidExpr => match ast.exprs.typeid_operand(e) {
                TypeidOperand::Type(t) => vec![t],
                TypeidOperand::Expr(_) => Vec::new(),
            },
            ExprKind::TypeTraitExpr => {
                ast.lists.iter::<QualType>(ast.exprs.type_trait_args(e)).collect()
            }
            ExprKind::CompoundLiteralExpr => vec![ast.exprs.compound_literal_written_type(e)],
            ExprKind::CXXUnresolvedConstructExpr => vec![ast.exprs.unresolved_construct_type(e)],
            k if k.is_cast() && k != ExprKind::ImplicitCastExpr => {
                vec![ast.exprs.cast_written_type(e).unwrap_or(x.ty())]
            }
            _ => ast.exprs.cast_written_type(e).into_iter().collect(),
        };
        let args = match k {
            ExprKind::ConceptSpecializationExpr => ast.exprs.concept_spec_args(e),
            ExprKind::DeclRefExpr | ExprKind::MemberExpr => {
                ast.exprs.decl_ref_extra(e).and_then(|x| x.template_args)
            }
            // A dependent name's, in a pattern.
            _ => ast.templates.expr_targs.get(e).copied(),
        };
        (held, args)
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
        ast.decl_written_exprs(d, &mut roots);
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

/// The client's shapes (WP-3580): a bound written as `sizeof` of a call,
/// in plain code and in a template's instances, dependent or not. Line
/// `n` of the source writes the call the walk from the declarations on
/// line `n` reaches.
const SUBST_SRC: &str = "\
long make(long);
extern long x;
namespace n { struct S { static long s(); }; }
char buf[sizeof(make(x))];
struct F { char fb[sizeof(make(x))]; };
void fl() { char lb[sizeof(make(x))]; }
void fp(char pb[sizeof(make(x))]);
template <class T> struct TB {
  char nd[sizeof(make(x))];
  T dep[sizeof(T::s())];
  char kept[sizeof(sizeof(T::s()))];
};
TB<n::S> tb;
template <class T> void tf(T, char tp[sizeof(T::s())], char tq[sizeof(make(x))]) {
  char tl[sizeof(T::s())];
  char tk[sizeof(sizeof(T::s()))];
}
void use() { tf(n::S(), nullptr, nullptr); }
";

impl Tu {
    fn decl_line(&self, d: DeclId) -> u32 {
        self.sm.line_col(self.ast().decls.get(d).loc()).map_or(0, |lc| lc.line)
    }

    /// The lines of every call reachable from `roots`.
    fn call_lines(&self, roots: Vec<ExprId>) -> Vec<u32> {
        let ast = self.ast();
        let mut lines: Vec<u32> = self
            .reach(roots)
            .into_iter()
            .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CallExpr)
            .map(|e| self.line(e))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// The pattern's and the one instance's declaration named `name` of
    /// kind `kind` (the instance's is made later).
    fn pattern_and_instance(&self, name: &str, kind: DeclKind) -> (DeclId, DeclId) {
        let all = self.named(name, kind);
        assert_eq!(all.len(), 2, "{name}: the pattern's and the instance's");
        (all[0], all[1])
    }

    /// The written bound expressions of `d`'s type (its outermost array,
    /// or the `Decayed` sugar of an adjusted parameter).
    fn bound_roots(&self, d: DeclId) -> Vec<ExprId> {
        let ast = self.ast();
        let mut roots = Vec::new();
        ast.types.written_exprs(ast.decls.get(d).ty().unwrap(), &mut roots);
        roots
    }
}

/// [dcl.array]/1, [expr.sizeof]: plain code keeps `sizeof(make(x))` on
/// a variable, a member, a local and a parameter (adjusted, as
/// `Decayed` sugar; [dcl.fct]/5).
#[test]
fn plain_sizeof_bounds_keep_their_calls() {
    let t = parse(SUBST_SRC);
    for (name, kind, line) in [
        ("buf", DeclKind::Var, 4),
        ("fb", DeclKind::Field, 5),
        ("lb", DeclKind::Var, 6),
        ("pb", DeclKind::ParmVar, 7),
    ] {
        let d = t.one(name, kind);
        assert_eq!(t.calls(t.decl_roots(d), "make"), [line], "{name}");
    }
}

/// [temp.inst]/3, [temp.res.general]: an instance's member keeps the
/// pattern's non-dependent bound (one node, shared), and the substituted
/// form of a dependent or instantiation-dependent one (a new,
/// non-dependent expression; WP-3580).
#[test]
fn an_instance_member_keeps_its_substituted_bound() {
    let t = parse(SUBST_SRC);
    let ast = t.ast();
    for (name, line) in [("nd", 9), ("dep", 10), ("kept", 11)] {
        let (pattern, inst) = t.pattern_and_instance(name, DeclKind::Field);
        assert_eq!(t.call_lines(t.decl_roots(inst)), [line], "{name}");
        let (pr, ir) = (t.bound_roots(pattern), t.bound_roots(inst));
        assert_eq!(ir.len(), 1, "{name}");
        if name == "nd" {
            assert_eq!(pr, ir, "the pattern's expression, not substituted");
        } else {
            assert_ne!(pr, ir, "{name}: the substituted bound");
            let deps = ast.exprs.get(ir[0]).deps();
            assert!(!deps.contains(kiln_ast::expr::ExprDeps::INSTANTIATION), "{name}");
        }
    }
    let dep = t.pattern_and_instance("dep", DeclKind::Field).1;
    let ty = ast.decls.get(dep).ty().unwrap();
    assert_eq!(ast.types.array_size(ty), Some(8));
    let kept = t.pattern_and_instance("kept", DeclKind::Field).1;
    let ty = ast.decls.get(kept).ty().unwrap();
    assert!(!ast.types.is_dependent(ty) && !ast.types.qflags(ty).is_instantiation_dependent());
    assert_eq!(ast.types.array_size(ty), Some(8));
}

/// [temp.inst], [dcl.fct]/5: a function template instance's parameters
/// keep their substituted bounds as `Decayed` sugar, and its locals
/// theirs (WP-3580).
#[test]
fn an_instance_parameter_and_local_keep_their_substituted_bounds() {
    let t = parse(SUBST_SRC);
    let ast = t.ast();
    for (name, kind, line) in [
        ("tp", DeclKind::ParmVar, 14),
        ("tq", DeclKind::ParmVar, 14),
        ("tl", DeclKind::Var, 15),
        ("tk", DeclKind::Var, 16),
    ] {
        let (_, inst) = t.pattern_and_instance(name, kind);
        assert_eq!(t.call_lines(t.decl_roots(inst)), [line], "{name}");
    }
    let tp = t.pattern_and_instance("tp", DeclKind::ParmVar).1;
    let ty = ast.decls.get(tp).ty().unwrap();
    assert!(
        matches!(ast.types.view(ty.ty()), kiln_ast::types::TypeView::Decayed { .. }),
        "the adjusted parameter keeps its bound"
    );
    assert!(ast.types.is_pointer(ty));
}

/// [dcl.fct]/5, [temp.inst]: an instantiated parameter keeps its type as
/// declared (`original_ty`) whenever adjusting changed it, as in plain
/// code: arrays with a substituted and a kept bound, a function type
/// (WP-3581).
#[test]
fn an_instance_parameter_keeps_its_declared_type() {
    let t = parse(
        "namespace n { struct S { static int s(); }; }\n\
         template <class T> void f(T, int p[sizeof(T::s())], decltype(T::s()) q[2], T r(int)) {}\n\
         void use() { f<n::S>(n::S(), nullptr, nullptr, nullptr); }\n\
         void g(int p[4], int q[2], n::S r(int));\n",
    );
    let ast = t.ast();
    for name in ["p", "q", "r"] {
        let all = t.named(name, DeclKind::ParmVar);
        assert_eq!(all.len(), 3, "{name}: the pattern's, f<n::S>'s and g's");
        for d in all {
            let o = ast.decls.param(d).and_then(|x| x.original_ty);
            let o = o.unwrap_or_else(|| panic!("{name} {d:?}: no original type"));
            let ty = ast.decls.get(d).ty().unwrap();
            assert!(ast.types.is_pointer(ty), "{name}: adjusted");
            if name == "r" {
                assert!(ast.types.is_function(o), "{name}: declared as a function");
            } else {
                assert!(ast.types.is_array(o), "{name}: declared as an array");
            }
        }
    }
    // The instance's declared `q` keeps both written expressions: the
    // bound `2` and the `decltype` operand `T::s()` substituted.
    let q = t.named("q", DeclKind::ParmVar);
    let inst = q[1];
    assert_eq!(t.decl_line(inst), 2, "f<n::S>'s q");
    let o = ast.decls.param(inst).and_then(|x| x.original_ty).unwrap();
    let mut roots = Vec::new();
    ast.types.written_exprs(o, &mut roots);
    assert_eq!(roots.len(), 2, "{roots:?}");
    assert_eq!(t.call_lines(roots), [2]);
    let g = t.named("p", DeclKind::ParmVar)[2];
    assert_eq!(t.decl_line(g), 4, "g's p");
    let o = ast.decls.param(g).and_then(|x| x.original_ty).unwrap();
    assert_eq!(ast.types.array_size(o), Some(4));
}

/// [dcl.type.pack.index], [expr.prim.pack.index]: after substitution a
/// pack-index type is `PackIndexing` sugar over the selected element,
/// holding the substituted index, in a variable and a parameter; a
/// pack-index expression's selected element maps to its substituted
/// index (WP-3582).
#[test]
fn pack_indexing_keeps_its_written_form() {
    use kiln_ast::types::TypeView;
    let t = parse(
        "long make(long);\n\
         template <int I, class... Ts> int pf(Ts...[I] pp, Ts... ts) {\n\
           Ts...[I] pv = pp;\n\
           auto pe = ts...[I];\n\
           return 0;\n\
         }\n\
         int use = pf<1>(2L, 'c', 3L);\n\
         template <class... Ts> struct PB { Ts...[sizeof(make(0)) - 8] pm; };\n\
         PB<short, int> pb;\n",
    );
    let ast = t.ast();
    for (name, kind) in [("pp", DeclKind::ParmVar), ("pv", DeclKind::Var), ("pm", DeclKind::Field)]
    {
        let (_, inst) = t.pattern_and_instance(name, kind);
        let ty = ast.decls.get(inst).ty().unwrap();
        let TypeView::PackIndexing { index, selected: Some(s), .. } = ast.types.view(ty.ty())
        else {
            panic!("{name}: no PackIndexing sugar");
        };
        assert_eq!(ast.types.canonical(ty), ast.types.canonical(s));
        let kiln_ast::types::ExprOperand::Expr(i) = index else { panic!("{name}: a profile") };
        let deps = ast.exprs.get(i).deps();
        assert!(!deps.contains(kiln_ast::expr::ExprDeps::INSTANTIATION), "{name}: substituted");
        let mut roots = Vec::new();
        ast.types.written_exprs(ty, &mut roots);
        assert!(roots.contains(&i), "{name}: the walk reaches the index");
    }
    // `pm` selects element 0 (`short`), through an index with a call.
    let pm = t.pattern_and_instance("pm", DeclKind::Field).1;
    let ty = ast.decls.get(pm).ty().unwrap();
    assert_eq!(t.call_lines(t.decl_roots(pm)), [8]);
    assert_eq!(ast.types.builtin(ty), Some(kiln_ast::types::BuiltinType::Short));
    // The expression: `pe`'s initializer is the selected element, which
    // maps to the substituted index.
    let pe = t.pattern_and_instance("pe", DeclKind::Var).1;
    let init = ast.decls.var(pe).and_then(|v| v.init).expect("an initializer");
    let elem = t
        .reach(vec![init])
        .into_iter()
        .find(|&e| ast.templates.pack_index_written.contains(e))
        .expect("the selected element");
    let i = *ast.templates.pack_index_written.get(elem).unwrap();
    assert!(!ast.exprs.get(i).deps().contains(kiln_ast::expr::ExprDeps::INSTANTIATION));
    assert_eq!(t.line(i), 4, "the index written in `ts...[I]`");
}

/// [dcl.array]/1 (GNU folding, clang `TryToFixInvalidVariablyModifiedType`):
/// a variable length array folded to a constant one keeps each bound as
/// written, for a static variable and an initialized automatic one with
/// two dimensions (WP-3580).
#[test]
fn a_folded_variable_length_array_keeps_its_bounds() {
    let t = parse(
        "static const double d = 3;\n\
         static char va[(int)d];\n\
         void fi() { char vi[(int)d] = \"ab\"; char vj[(int)d][(int)d] = {}; }\n",
    );
    let ast = t.ast();
    for (name, n) in [("va", 1), ("vi", 1), ("vj", 2)] {
        let v = t.one(name, DeclKind::Var);
        let ty = ast.decls.get(v).ty().unwrap();
        assert_eq!(ast.types.array_size(ty), Some(3), "{name}: folded");
        let roots = t.bound_roots(v);
        assert_eq!(roots.len(), n, "{name}: {roots:?}");
        for r in roots {
            assert_eq!(ast.exprs.get(r).kind(), ExprKind::CStyleCastExpr, "{name}");
        }
    }
}

/// A module interface parsed into an image (or, with `files`, a user
/// importing module files): the unit, its capsule and its imports'
/// replay reasons (`None`: built from the image).
fn parse_module(
    src: &str,
    record: bool,
    files: Vec<(String, std::path::PathBuf)>,
) -> (kiln_parse::ParsedTu, Option<kiln_pp::bmi::Capsule>, Vec<Option<String>>) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cppm", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts =
        ParseOptions { std: kiln_lex::Std::Cxx26, record_image: record, ..ParseOptions::default() };
    let mut ppo = kiln_pp::PpOptions::new(opts.std);
    ppo.modules = Some(kiln_pp::ModuleOptions {
        module_files: files,
        record,
        image: true,
        ..kiln_pp::ModuleOptions::default()
    });
    let mut pp = kiln_pp::Preprocessor::new(&sm, &engine, &interner, ppo);
    pp.enter_main_file(file);
    let target = Target::for_arch(Arch::X86_64);
    let (tu, mut pp) = kiln_parse::parse_preprocessed(
        pp,
        Arc::clone(&interner),
        &engine,
        target,
        &opts,
        ConstexprLimits::default(),
    );
    let diags: Vec<String> = sink.take().into_iter().map(|d| d.message).collect();
    assert!(diags.is_empty(), "{diags:?}");
    let imports = pp.image_imports().iter().map(|i| i.replayed.clone()).collect();
    (tu, pp.take_capsule(), imports)
}

/// spec/modules.md §11.11, spec/sema-templates.md §7.4: an instance the
/// interface made keeps the substituted index of its pack-index
/// expression in the importer (`pack_index_written`; WP-3582). The image
/// carries no pack-index pattern yet, so such an interface is replayed;
/// the image's cold field for the map serves the day it does.
#[test]
fn a_pack_index_survives_the_module_image() {
    let iface = "export module m;\n\
                 export template <int I, class... Ts> inline long pick(Ts... ts) {\n\
                   return ts...[I];\n\
                 }\n\
                 export inline long pick1() { return pick<1>(2L, 30L, 4L); }\n";
    let (tu, capsule, _) = parse_module(iface, true, Vec::new());
    let count = |ast: &Ast| {
        ast.templates
            .pack_index_written
            .iter()
            .filter(|&(_, &i)| {
                !ast.exprs.get(i).deps().contains(kiln_ast::expr::ExprDeps::INSTANTIATION)
            })
            .count()
    };
    assert_eq!(count(&tu.ast), 1, "pick<1>'s ts...[I]");
    let image = tu.image.expect("an image");
    assert_eq!(image.fallback, Some("a template expression (pack indexing)"));
    let capsule = capsule.expect("a module interface");
    let dir = std::env::temp_dir().join(format!("kiln-written-bounds-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pcm = dir.join("m.pcm");
    std::fs::write(&pcm, kiln_pp::bmi::write(&capsule)).unwrap();
    let user = "import m;\nlong use() { return pick1(); }\n";
    let (got, _, imports) = parse_module(user, false, vec![("m".to_owned(), pcm)]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(imports.is_empty(), "replayed, not built from an image");
    assert_eq!(count(&got.ast), 1, "the imported instance keeps its index");
}

/// The shapes of a deduced variable ([dcl.spec.auto], [dcl.type.auto.deduct],
/// [dcl.struct.bind]), one per line; `w`'s type is written, so its
/// `decltype` operand is its own, and a variable deduced from `w` writes
/// none (spec/types.md §8.4, WP-3583).
const DEDUCED: &str = "\
long make(int);
int x;
decltype(make(x)) w = 0;
auto v = w;
decltype(auto) v2 = w;
auto &r = w;
const auto *p = &w;
struct Pr { decltype(make(x)) a, b; };
Pr pr{};
auto [a, b] = pr;
decltype(make(x)) arr[2] = {};
auto [c, d] = arr;
decltype(make(x)) ctrl = w;
template <class T> long tf(T t) {
  decltype(make(t)) tw = 0;
  auto tv = tw;
  decltype(make(t)) ta[1] = {tw}; for (auto e : ta) { tv += e; }
  return tv;
}
long inst = tf(1);
auto lam = [cap = w] { return cap; };
";

/// [dcl.type.auto.deduct]/4: the deduced type is no written one, so the
/// walk from the declaration does not reach `w`'s `decltype(make(x))`
/// through it; the written control and `w` itself do.
#[test]
fn a_deduced_variable_writes_no_expression() {
    let t = parse(DEDUCED);
    let ast = t.ast();
    let roots = |d: DeclId| {
        let mut out = Vec::new();
        ast.decl_written_exprs(d, &mut out);
        out
    };
    let place = |d: DeclId| ast.decls.var(d).unwrap().spec.placeholder();
    for (name, kind) in [
        ("v", VarPlaceholder::Auto),
        ("v2", VarPlaceholder::DecltypeAuto),
        ("r", VarPlaceholder::Auto),
        ("p", VarPlaceholder::Auto),
        // [expr.prim.lambda.capture]/6: an init-capture deduces as `auto`.
        ("cap", VarPlaceholder::Auto),
    ] {
        let d = t.one(name, DeclKind::Var);
        assert_eq!(place(d), kind, "{name}");
        assert!(roots(d).is_empty(), "{name}: {:?}", roots(d));
    }
    // The type keeps `w`'s sugar (it prints as clang's does); only the
    // declaration walk stops at it.
    for name in ["v", "v2"] {
        let d = t.one(name, DeclKind::Var);
        let mut leak = Vec::new();
        ast.types.written_exprs(ast.decls.get(d).ty().unwrap(), &mut leak);
        assert_eq!(t.calls(leak, "make"), [3], "{name}");
    }
    for (name, line) in [("w", 3), ("ctrl", 13)] {
        let d = t.one(name, DeclKind::Var);
        assert_eq!(place(d), VarPlaceholder::None, "{name}");
        assert_eq!(t.calls(roots(d), "make"), [line], "{name}");
    }
}

/// [dcl.struct.bind]/1: the structured binding declaration's `auto` is a
/// placeholder (a class and an array initializer); the bindings' types
/// are their elements', written elsewhere.
#[test]
fn a_structured_binding_writes_no_expression() {
    let t = parse(DEDUCED);
    let ast = t.ast();
    for (name, line) in [("a", 10), ("c", 12)] {
        let b = t.one(name, DeclKind::Binding);
        let mut out = Vec::new();
        ast.decl_written_exprs(b, &mut out);
        assert!(out.is_empty(), "{name}: {out:?}");
        let e = ast.decls.binding_decomp(b).expect("the decomposition");
        assert_eq!(ast.decls.var(e).unwrap().spec.placeholder(), VarPlaceholder::Auto, "{name}");
        ast.decl_written_exprs(e, &mut out);
        assert!(out.is_empty(), "{name} (line {line}): {out:?}");
    }
}

/// [temp.inst], [temp.dep.expr]/3: a pattern's variable with a dependent
/// initializer has the kind with its dependent `auto` type; the
/// instance's is deduced; neither walks into its type; a range-`for`
/// variable is deduced too ([stmt.ranged]/1).
#[test]
fn an_instantiated_deduced_variable() {
    let t = parse(DEDUCED);
    let ast = t.ast();
    let tv = t.named("tv", DeclKind::Var);
    assert_eq!(tv.len(), 2, "the pattern's and tf<int>'s");
    let kinds: Vec<_> = tv.iter().map(|&d| ast.decls.var(d).unwrap().spec.placeholder()).collect();
    assert_eq!(kinds, [VarPlaceholder::Auto, VarPlaceholder::Auto]);
    for &d in &tv {
        let mut out = Vec::new();
        ast.decl_written_exprs(d, &mut out);
        assert!(out.is_empty(), "{d:?}: {out:?}");
    }
    let tw = t.named("tw", DeclKind::Var);
    assert_eq!(tw.len(), 2);
    for &d in &tw {
        assert_eq!(t.calls(t.decl_roots(d), "make"), [15], "{d:?}");
    }
    let e = t.named("e", DeclKind::Var);
    assert_eq!(e.len(), 2);
    assert_eq!(ast.decls.var(e[1]).unwrap().spec.placeholder(), VarPlaceholder::Auto);
}

/// Every type operand, one per line, in plain code and in a function
/// template's pattern and instance (spec/ast.md §5.11, WP-3589).
const OPERANDS: &str = "\
namespace std { class type_info; }
long make(int);
int x;
template <class T> concept C = true;
template <int N> int vt = N;
struct O { template <int N> static int m() { return N; } } o;
template <class T> long tf(T t) {
  long sc = static_cast<decltype(make(t))>(1);
  long cc = (decltype(make(t)))1;
  long fc = decltype(make(t))(1);
  unsigned long so = sizeof(decltype(make(t)));
  unsigned long ao = alignof(decltype(make(t)));
  bool tt = __is_same(decltype(make(t)), long);
  const std::type_info &ti = typeid(decltype(make(t)));
  long *np = new decltype(make(t))(1);
  long cl = (decltype(make(t))[]){1, 2}[0];
  int dr = vt<sizeof(make(t))>;
  int mr = o.m<sizeof(make(t))>();
  bool ci = C<decltype(make(t))>;
  return sc + cc + fc + so + ao + tt + *np + cl + dr + mr + ci + (&ti != nullptr);
}
long inst = tf(1);
void plain() {
  long sc = static_cast<decltype(make(x))>(1);
  long cc = (decltype(make(x)))1;
  long fc = decltype(make(x))(1);
  unsigned long so = sizeof(decltype(make(x)));
  unsigned long ao = alignof(decltype(make(x)));
  bool tt = __is_same(decltype(make(x)), long);
  const std::type_info &ti = typeid(decltype(make(x)));
  long *np = new decltype(make(x))(1);
  long cl = (decltype(make(x))[]){1, 2}[0];
  int dr = vt<sizeof(make(x))>;
  int mr = o.m<sizeof(make(x))>();
  bool ci = C<decltype(make(x))>;
}
";

/// [expr.static.cast], [expr.cast], [expr.type.conv], [expr.sizeof],
/// [expr.alignof], [expr.typeid], [expr.new], [temp.names], [temp.names]/8
/// (concept-id), and the GNU compound literal: the type operand as written
/// (through its accessor, spec/ast.md §5.11) reaches the `make` call in
/// its `decltype` or bound, in plain code (`plain`) and in the pattern and
/// the instance of `tf`, three times per name, at clang compatibility 22
/// and 19.
#[test]
fn every_type_operand_is_kept_as_written() {
    for compat in [22, 19] {
        every_type_operand_at(compat);
    }
}

fn every_type_operand_at(compat: u32) {
    let t = parse_at(OPERANDS, compat);
    let ast = t.ast();
    for (i, name) in
        ["sc", "cc", "fc", "so", "ao", "tt", "ti", "np", "cl", "dr", "mr", "ci"].iter().enumerate()
    {
        let vars = t.named(name, DeclKind::Var);
        assert_eq!(vars.len(), 3, "{name}: the pattern's, tf<int>'s and plain's");
        for &d in &vars {
            let line = 8 + i as u32;
            let line = if ast.decls.get(d).ctx()
                == t.named("plain", DeclKind::Function).first().copied()
            {
                line + 16
            } else {
                line
            };
            assert_eq!(t.calls(t.decl_roots(d), "make"), [line], "{name} {d:?} at {compat}");
        }
    }
}

/// GNU compound literal, [dcl.init.aggr]/4: `(T[]){1, 2}` has the
/// completed type `T[2]`; its written `T[]` is kept beside it, also in an
/// instance; a literal written with its bound keeps its node type.
#[test]
fn a_compound_literal_keeps_its_unknown_bound() {
    let t = parse(OPERANDS);
    let ast = t.ast();
    let lits: Vec<ExprId> = (1..=ast.exprs.len() as u32)
        .map(ExprId::new)
        .filter(|&e| ast.exprs.get(e).kind() == ExprKind::CompoundLiteralExpr)
        .collect();
    assert_eq!(lits.len(), 3, "the pattern's, tf<int>'s and plain's");
    // spec/pch.md: a PCH snapshot carries the written type.
    let bytes = kiln_snap::to_bytes(ast);
    let restored = kiln_snap::from_bytes::<Ast>(&bytes).expect("the snapshot decodes");
    for &e in &lits {
        assert_eq!(
            restored.exprs.compound_literal_written_type(e),
            ast.exprs.compound_literal_written_type(e)
        );
    }
    for e in lits {
        let w = ast.exprs.compound_literal_written_type(e);
        assert!(ast.types.is_array(w));
        assert_eq!(ast.types.array_size(w), None, "T[] as written: {e:?}");
        let n = ast.exprs.get(e).ty();
        if !ast.types.is_dependent(n) {
            assert_eq!(ast.types.array_size(n), Some(2), "the completed type: {e:?}");
        }
    }
    let t = parse("int f(int);\nint g() { return (int[2]){f(1), 2}[0]; }\n");
    let ast = t.ast();
    let e = (1..=ast.exprs.len() as u32)
        .map(ExprId::new)
        .find(|&e| ast.exprs.get(e).kind() == ExprKind::CompoundLiteralExpr)
        .unwrap();
    assert_eq!(ast.exprs.compound_literal_written_type(e), ast.exprs.get(e).ty());
    assert!(ast.exprs.cold.compound_literal_written.get(e).is_none());
}

/// [dcl.spec.auto]: a function whose return type was deduced from a
/// placeholder (`auto`, `decltype(auto)`, `auto -> auto`, member
/// function, redeclaration after definition) writes no return-type
/// expression, so `decl_written_exprs` does not leak expressions from
/// the deduced return type (spec/types.md §8.4, WP-3641).
#[test]
fn a_deduced_return_writes_no_expression() {
    let t = parse(
        "template<int N> struct A {};\n\
         A<1+1> g();\n\
         auto f() { return g(); }\n\
         auto f();\n\
         decltype(auto) f_decl() { return g(); }\n\
         auto f_trail() -> auto { return g(); }\n\
         struct S { auto m() { return g(); } };\n",
    );
    let ast = t.ast();
    let fs = t.named("f", DeclKind::Function);
    assert_eq!(fs.len(), 2, "definition and trailing redeclaration");
    assert!(ast.decls.function(fs[0]).unwrap().spec.return_deduced());
    let decls = [
        fs[0],
        t.one("f_decl", DeclKind::Function),
        t.one("f_trail", DeclKind::Function),
        t.one("m", DeclKind::CXXMethod),
    ];
    for d in decls {
        let mut out = Vec::new();
        ast.decl_written_exprs(d, &mut out);
        assert!(out.is_empty(), "{d:?}: deduced return leaked {out:?}");
        // The function type itself keeps `g`'s sugar; only the
        // declaration walk skips its deduced return type.
        let mut leak = Vec::new();
        ast.types.written_exprs(ast.decls.get(d).ty().unwrap(), &mut leak);
        assert_eq!(leak.len(), 1, "{d:?}: {leak:?}");
        assert_eq!(t.line(leak[0]), 2, "{d:?}: 1+1 from g");
    }
    let mut out = Vec::new();
    ast.decl_written_exprs(fs[1], &mut out);
    assert!(out.is_empty(), "trailing redecl leaked {out:?}");
}

/// [dcl.spec.auto], [except.spec]: a function whose return type was
/// deduced still appends the expressions written in its parameter types
/// (including nested function types) in order, and in its `noexcept(e)`
/// operand (spec/types.md §8.4, WP-3641).
#[test]
fn a_deduced_return_keeps_parameter_and_noexcept_exprs() {
    let t = parse(
        "template<int N> struct A {};\n\
         A<1+1> g();\n\
         auto f(int (*p)[3+1]) noexcept(2>1) { return g(); }\n\
         auto f2(A<4+1> (*cb)()) { return g(); }\n",
    );
    let ast = t.ast();
    let char_at = |e: ExprId| -> char {
        let lc = t.sm.line_col(ast.exprs.get(e).loc()).expect("source loc");
        let line = t.src.lines().nth(lc.line as usize - 1).unwrap();
        line.as_bytes()[lc.col as usize - 1] as char
    };
    let f = t.one("f", DeclKind::Function);
    assert!(ast.decls.function(f).unwrap().spec.return_deduced());
    let mut out = Vec::new();
    ast.decl_written_exprs(f, &mut out);
    assert_eq!(out.len(), 2, "3+1 then 2>1: {out:?}");
    assert_eq!(t.line(out[0]), 3);
    assert_eq!(t.line(out[1]), 3);
    assert_eq!(ast.exprs.get(out[0]).kind(), ExprKind::BinaryOperator);
    assert_eq!(char_at(out[0]), '+', "parameter bound 3+1 first");
    assert_eq!(ast.exprs.get(out[1]).kind(), ExprKind::ConstantExpr);
    let exc_inner = ast.exprs.children(out[1], &ast.lists)[0];
    assert_eq!(ast.exprs.get(exc_inner).kind(), ExprKind::BinaryOperator);
    assert_eq!(char_at(exc_inner), '>', "noexcept(2>1) second");

    // A nested function type written in a parameter keeps its own return.
    let f2 = t.one("f2", DeclKind::Function);
    assert!(ast.decls.function(f2).unwrap().spec.return_deduced());
    let mut out2 = Vec::new();
    ast.decl_written_exprs(f2, &mut out2);
    assert_eq!(out2.len(), 1, "4+1 from cb's return type: {out2:?}");
    assert_eq!(t.line(out2[0]), 4);
}

/// [dcl.fct]/2: a non-placeholder trailing return type is written, not
/// deduced, so `decl_written_exprs` walks the whole function type
/// (spec/types.md §8.4, WP-3641).
#[test]
fn a_written_trailing_return_keeps_its_expression() {
    let t = parse(
        "template<int N> struct A {};\n\
         A<1+1> g();\n\
         auto f() -> A<1+1> { return g(); }\n",
    );
    let ast = t.ast();
    let f = t.one("f", DeclKind::Function);
    assert!(!ast.decls.function(f).unwrap().spec.return_deduced());
    let mut out = Vec::new();
    ast.decl_written_exprs(f, &mut out);
    assert_eq!(out.len(), 1, "1+1 from f's trailing return: {out:?}");
    assert_eq!(t.line(out[0]), 3);
    assert_eq!(ast.exprs.get(out[0]).kind(), ExprKind::BinaryOperator);
}

/// [expr.prim.lambda.closure]/4: a lambda call operator without a
/// trailing return type has a deduced return type, so `decl_written_exprs`
/// appends only its parameter expressions (spec/types.md §8.4, WP-3641).
#[test]
fn a_lambda_deduced_return_keeps_parameter_exprs() {
    use kiln_ast::names::OverloadedOperator;
    let t = parse(
        "template<int N> struct A {};\n\
         A<1+1> g();\n\
         auto lam = [](int (*p)[3+1]) { return g(); };\n",
    );
    let ast = t.ast();
    let ops: Vec<DeclId> = (1..=ast.decls.len() as u32)
        .map(DeclId::new)
        .filter(|&d| {
            let n = ast.decls.get(d);
            n.kind() == DeclKind::CXXMethod
                && n.name().as_operator() == Some(OverloadedOperator::Call)
        })
        .collect();
    assert_eq!(ops.len(), 1, "lambda call operator: {ops:?}");
    let op = ops[0];
    assert!(ast.decls.function(op).unwrap().spec.return_deduced());
    let mut out = Vec::new();
    ast.decl_written_exprs(op, &mut out);
    assert_eq!(out.len(), 1, "3+1 only: {out:?}");
    assert_eq!(t.line(out[0]), 3);
    assert_eq!(ast.exprs.get(out[0]).kind(), ExprKind::BinaryOperator);
    // Raw `written_exprs` on the call operator's type reaches both.
    let mut all = Vec::new();
    ast.types.written_exprs(ast.decls.get(op).ty().unwrap(), &mut all);
    assert_eq!(all.len(), 2, "deduced return (line 2) + param (line 3): {all:?}");
}

/// [dcl.spec.auto.general]/12, [temp.inst]: a function template pattern
/// with a body gets `return_deduced` (its return is a dependent `auto`);
/// a body-less declaration keeps an undeduced `auto` (which writes no
/// expression either); an instantiation appends only the specialization's
/// parameter expressions, not its deduced return type (spec/types.md
/// §8.4, WP-3641).
#[test]
fn a_deduced_return_template_pattern_and_specialization() {
    use kiln_ast::expr::ExprDeps;
    use kiln_ast::types::TypeView;
    let t = parse(
        "template<int N> struct A {};\n\
         A<1+1> g();\n\
         template<class T> auto fwd(T t);\n\
         template<class T> auto id(T t) { return t; }\n\
         template<class T> auto h(T, int (*p)[sizeof(T) + 1]) { return g(); }\n\
         auto r1 = id(g());\n\
         auto r2 = h(0, nullptr);\n",
    );
    let ast = t.ast();

    // Forward-declared pattern: no body yet, return is undeduced `auto`.
    let fwd = t.one("fwd", DeclKind::Function);
    assert!(!ast.decls.function(fwd).unwrap().spec.return_deduced());
    let TypeView::Function(p) = ast.types.view(ast.decls.get(fwd).ty().unwrap().ty()) else {
        panic!("fwd is a function");
    };
    assert!(matches!(ast.types.view(p.ret().ty()), TypeView::Auto { args: None, .. }));
    let mut out = Vec::new();
    ast.decl_written_exprs(fwd, &mut out);
    assert!(out.is_empty(), "fwd: {out:?}");

    // Defined pattern `id` and its specialization `id<A<1+1>>`.
    let (id_pat, id_spec) = t.pattern_and_instance("id", DeclKind::Function);
    assert!(ast.decls.function(id_pat).unwrap().spec.return_deduced());
    assert!(ast.decls.function(id_spec).unwrap().spec.return_deduced());
    for d in [id_pat, id_spec] {
        let mut out = Vec::new();
        ast.decl_written_exprs(d, &mut out);
        assert!(out.is_empty(), "id {d:?}: {out:?}");
    }

    // Defined pattern `h` and its specialization `h<int>`.
    let (h_pat, h_spec) = t.pattern_and_instance("h", DeclKind::Function);
    assert!(ast.decls.function(h_pat).unwrap().spec.return_deduced());
    assert!(ast.decls.function(h_spec).unwrap().spec.return_deduced());
    let mut pat_out = Vec::new();
    ast.decl_written_exprs(h_pat, &mut pat_out);
    assert_eq!(pat_out.len(), 1, "h pattern: {pat_out:?}");
    assert_eq!(t.line(pat_out[0]), 5);
    assert!(ast.exprs.get(pat_out[0]).deps().contains(ExprDeps::INSTANTIATION));

    let mut spec_out = Vec::new();
    ast.decl_written_exprs(h_spec, &mut spec_out);
    assert_eq!(spec_out.len(), 1, "h<int> appends only its parameter expr: {spec_out:?}");
    assert_eq!(t.line(spec_out[0]), 5);
    assert_ne!(spec_out[0], pat_out[0], "the specialization's substituted bound");
    assert!(!ast.exprs.get(spec_out[0]).deps().contains(ExprDeps::INSTANTIATION));

    let mut h_all = Vec::new();
    ast.types.written_exprs(ast.decls.get(h_spec).ty().unwrap(), &mut h_all);
    assert_eq!(h_all.len(), 2, "deduced return (line 2) + param (line 5): {h_all:?}");
}

/// [dcl.type.auto.deduct]: expressions written in the declarator around
/// a deduced placeholder (`auto (*p)[f(1)] = &a;`) are kept on
/// `ColdDeclData::written_decl_type` and reported by
/// `decl_written_exprs`, while a plain `auto v = w;` still appends
/// nothing and keeps zero bytes in the map (spec/types.md §8.4, WP-3640).
#[test]
fn a_deduced_declarator_bound_is_kept() {
    let t = parse(
        "constexpr int f(int x) { return x; }\n\
         long make(int);\n\
         int x;\n\
         decltype(make(x)) a[1] = {};\n\
         decltype(make(x)) w = 0;\n\
         auto (*p)[f(1)] = &a;\n\
         auto v = w;\n",
    );
    let ast = t.ast();
    let p = t.one("p", DeclKind::Var);
    assert_eq!(ast.decls.var(p).unwrap().spec.placeholder(), VarPlaceholder::Auto);
    let mut out = Vec::new();
    ast.decl_written_exprs(p, &mut out);
    assert_eq!(out.len(), 1, "p's written bound f(1): {out:?}");
    assert_eq!(t.calls(out, "f"), [6]);
    // Control: `auto v = w;` still appends nothing, keeps zero bytes in
    // `written_decl_type`, and does not leak `w`'s `decltype(make(x))`.
    let v = t.one("v", DeclKind::Var);
    assert_eq!(ast.decls.written_decl_type(v), None);
    let mut v_out = Vec::new();
    ast.decl_written_exprs(v, &mut v_out);
    assert!(v_out.is_empty(), "v control leaked: {v_out:?}");
}

/// [dcl.type.auto.deduct], [temp.constr.decl]: a type-constraint's
/// template arguments on a deduced variable (`C<f(1)> auto v = w;`) are
/// kept and reported by `decl_written_exprs` in §8.4 pre-order, without
/// leaking expressions from `w`'s type (spec/types.md §8.4, WP-3640).
#[test]
fn a_constrained_placeholder_var_keeps_its_arguments() {
    let t = parse(
        "constexpr int f(int x) { return x; }\n\
         template<class, int> concept C = true;\n\
         long make(int);\n\
         int x;\n\
         decltype(make(x)) w = 0;\n\
         decltype(make(x)) a[1] = {};\n\
         C<f(1)> auto v = w;\n\
         C<f(2)> auto (*p)[f(1)] = &a;\n",
    );
    let ast = t.ast();
    let v = t.one("v", DeclKind::Var);
    let mut out = Vec::new();
    ast.decl_written_exprs(v, &mut out);
    assert_eq!(out.len(), 1, "v's constraint arg f(1): {out:?}");
    assert_eq!(t.calls(out.clone(), "f"), [7]);
    assert!(t.calls(out, "make").is_empty());

    let p = t.one("p", DeclKind::Var);
    let mut p_out = Vec::new();
    ast.decl_written_exprs(p, &mut p_out);
    assert_eq!(p_out.len(), 2, "p's bound f(1) then constraint f(2): {p_out:?}");
    assert_eq!(t.calls(vec![p_out[0]], "f"), [8]);
    assert_eq!(t.calls(vec![p_out[1]], "f"), [8]);
    assert!(t.calls(p_out.clone(), "make").is_empty());
    // Pre-order: the outer array bound `f(1)` precedes the inner element's
    // type-constraint argument `f(2)`.
    let col = |e: ExprId| t.sm.line_col(ast.exprs.get(e).loc()).unwrap().col;
    assert!(col(p_out[0]) > col(p_out[1]), "array bound before element constraint");
}

/// [dcl.spec.auto], [temp.constr.decl]: a constrained deduced return
/// (`C<f(1)> auto g() { return 0; }`) keeps its constraint argument,
/// reported before parameter and `noexcept(e)` expressions and without
/// leaking the deduced return type (spec/types.md §8.4, WP-3640).
#[test]
fn a_constrained_deduced_return_keeps_its_arguments() {
    let t = parse(
        "constexpr int f(int x) { return x; }\n\
         template<class, int> concept C = true;\n\
         template<int N> struct A {};\n\
         A<1+1> h();\n\
         C<f(1)> auto g() { return 0; }\n\
         C<f(1)> auto g2(int (*p)[f(2)]) noexcept(f(3) > 0) { return h(); }\n",
    );
    let ast = t.ast();
    let g = t.one("g", DeclKind::Function);
    assert!(ast.decls.function(g).unwrap().spec.return_deduced());
    let mut out = Vec::new();
    ast.decl_written_exprs(g, &mut out);
    assert_eq!(out.len(), 1, "g's return constraint f(1): {out:?}");
    assert_eq!(t.calls(out, "f"), [5]);

    let g2 = t.one("g2", DeclKind::Function);
    assert!(ast.decls.function(g2).unwrap().spec.return_deduced());
    let mut out2 = Vec::new();
    ast.decl_written_exprs(g2, &mut out2);
    assert_eq!(out2.len(), 3, "g2's return f(1), param f(2), noexcept f(3)>0: {out2:?}");
    let col = |e: ExprId| t.sm.line_col(ast.exprs.get(e).loc()).unwrap().col;
    assert!(col(out2[0]) < col(out2[1]) && col(out2[1]) < col(out2[2]), "pre-order: {out2:?}");
    assert_eq!(t.calls(out2, "f"), [6]);
}

/// [temp.inst], [dcl.type.auto.deduct], [dcl.spec.auto]: instantiations
/// of variable and function templates with expressions around a deduced
/// placeholder substitute those written types, so the specialization
/// reports its own non-dependent expressions (spec/types.md §8.4,
/// WP-3640).
#[test]
fn a_deduced_placeholder_in_template_keeps_substituted_exprs() {
    use kiln_ast::expr::ExprDeps;
    let t = parse(
        "constexpr int f(int x) { return x; }\n\
         template<class, int> concept C = true;\n\
         template<int N> struct A {};\n\
         A<1+1> h();\n\
         template<class T> C<f(sizeof(T))> auto tv = h();\n\
         template<class T> C<f(sizeof(T) + 2)> auto tf(int (*p)[f(sizeof(T) + 1)]) { return h(); }\n\
         template<class T> C<f(1)> auto tv2 = h();\n\
         long make(int);\n\
         int x;\n\
         decltype(make(x)) a[1] = {};\n\
         decltype(make(x)) w = 0;\n\
         template<class T> void k() { auto (*kp)[f(1)] = &a; auto kq = w; }\n\
         auto r1 = tv<int>;\n\
         auto r2 = tf<int>(nullptr);\n\
         auto r3 = tv2<int>;\n\
         void call_k() { k<int>(); }\n",
    );
    let ast = t.ast();

    let (var_pat, var_spec) = t.pattern_and_instance("tv", DeclKind::Var);
    let (mut pat_out, mut spec_out) = (Vec::new(), Vec::new());
    ast.decl_written_exprs(var_pat, &mut pat_out);
    ast.decl_written_exprs(var_spec, &mut spec_out);
    assert_eq!(pat_out.len(), 1, "tv pattern: {pat_out:?}");
    assert_eq!(spec_out.len(), 1, "tv<int>: {spec_out:?}");
    assert_eq!(t.line(pat_out[0]), 5);
    assert_eq!(t.line(spec_out[0]), 5);
    assert_ne!(spec_out[0], pat_out[0]);
    assert!(ast.exprs.get(pat_out[0]).deps().contains(ExprDeps::INSTANTIATION));
    assert!(!ast.exprs.get(spec_out[0]).deps().contains(ExprDeps::INSTANTIATION));

    let (fn_pat, fn_spec) = t.pattern_and_instance("tf", DeclKind::Function);
    let (mut fpat_out, mut fspec_out) = (Vec::new(), Vec::new());
    ast.decl_written_exprs(fn_pat, &mut fpat_out);
    ast.decl_written_exprs(fn_spec, &mut fspec_out);
    assert_eq!(fpat_out.len(), 2, "tf pattern: {fpat_out:?}");
    assert_eq!(fspec_out.len(), 2, "tf<int>: {fspec_out:?}");
    for i in 0..2 {
        assert_eq!(t.line(fpat_out[i]), 6);
        assert_eq!(t.line(fspec_out[i]), 6);
        assert_ne!(fspec_out[i], fpat_out[i]);
        assert!(ast.exprs.get(fpat_out[i]).deps().contains(ExprDeps::INSTANTIATION));
        assert!(!ast.exprs.get(fspec_out[i]).deps().contains(ExprDeps::INSTANTIATION));
    }

    // Variable template whose pattern was already deduced (non-dependent
    // declared type and non-dependent initializer): `tv2<int>` keeps its
    // placeholder kind and `written_decl_type`, appending only `f(1)` and
    // never leaking `1+1` from `h()`.
    let (_, tv2_spec) = t.pattern_and_instance("tv2", DeclKind::Var);
    assert_eq!(ast.decls.var(tv2_spec).unwrap().spec.placeholder(), VarPlaceholder::Auto);
    let mut tv2_out = Vec::new();
    ast.decl_written_exprs(tv2_spec, &mut tv2_out);
    assert_eq!(tv2_out.len(), 1, "tv2<int>: {tv2_out:?}");
    assert_eq!(t.calls(tv2_out, "f"), [7]);

    // Function-body locals whose pattern declarations were already deduced
    // (`copy_deduced_written_type`): `kp` in `k<int>` keeps only `f(1)`
    // (non-dependent, no `make(x)` leak) and `kq` appends nothing with its
    // placeholder kind preserved.
    let (_, local_ptr) = t.pattern_and_instance("kp", DeclKind::Var);
    assert_eq!(ast.decls.var(local_ptr).unwrap().spec.placeholder(), VarPlaceholder::Auto);
    let mut ptr_out = Vec::new();
    ast.decl_written_exprs(local_ptr, &mut ptr_out);
    assert_eq!(ptr_out.len(), 1, "k<int>'s kp: {ptr_out:?}");
    assert!(!ast.exprs.get(ptr_out[0]).deps().contains(ExprDeps::INSTANTIATION));
    assert_eq!(t.calls(ptr_out.clone(), "f"), [12]);
    assert!(t.calls(ptr_out, "make").is_empty());

    let (_, local_var) = t.pattern_and_instance("kq", DeclKind::Var);
    assert_eq!(ast.decls.var(local_var).unwrap().spec.placeholder(), VarPlaceholder::Auto);
    assert_eq!(ast.decls.written_decl_type(local_var), None);
    let mut var_out = Vec::new();
    ast.decl_written_exprs(local_var, &mut var_out);
    assert!(var_out.is_empty(), "k<int>'s kq leaked: {var_out:?}");
}
