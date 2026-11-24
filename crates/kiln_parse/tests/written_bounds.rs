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
