//! Nested evaluation (R-OVEN-1, spec/oven.md "Nested evaluation") and
//! content-interned results (R-OVEN-2, "Transient allocation"): a bridge
//! callback evaluates another constant expression on the same Oven while
//! an evaluation runs, and equal results freeze to one constant.

mod common;
use common::*;
use kiln_ast::consts::ConstStore;
use kiln_ast::types::RecordLayout;
use kiln_ast::{Ast, ConstValueId, DeclId, ExprId, StmtId};
use kiln_diag::Diag;
use kiln_oven::{
    Budget, EvalContext, EvalFailure, EvalKind, EvalRecord, FailKind, Oven, SemaBridge, Subobject,
    SubobjectId,
};
use kiln_source::Loc;

/// A bridge that, the first time the Oven asks for the body of `on`,
/// evaluates `expr` as a constraint on the Oven that asked (as sema does
/// when instantiating a body checks a constraint).
struct Nesting<'a> {
    inner: Bridge<'a>,
    on: DeclId,
    expr: ExprId,
    loc: Loc,
    result: Option<Result<ConstValueId, EvalFailure>>,
}

impl SemaBridge for Nesting<'_> {
    fn ast(&self) -> &Ast {
        self.inner.ast()
    }
    fn ensure_body(&mut self, f: DeclId, oven: &mut Oven) -> Option<StmtId> {
        let ast = self.inner.ast();
        if self.result.is_none() && ast.definition(f) == ast.definition(self.on) {
            let (e, ctx) = (self.expr, EvalContext::new(EvalKind::Constraint, self.loc));
            let r = oven.evaluate(self, e, ctx);
            self.result = Some(r);
        }
        self.inner.ensure_body(f, oven)
    }
    fn ensure_layout(&mut self, record: DeclId) -> &RecordLayout {
        self.inner.ensure_layout(record)
    }
    fn final_overrider(
        &mut self,
        most_derived: DeclId,
        at: SubobjectId,
        method: DeclId,
    ) -> Option<(DeclId, SubobjectId)> {
        self.inner.final_overrider(most_derived, at, method)
    }
    fn subobjects(&mut self, record: DeclId) -> &[Subobject] {
        self.inner.subobjects(record)
    }
    fn consts(&mut self) -> &mut ConstStore {
        self.inner.consts()
    }
    fn diag(&mut self, d: Diag) {
        self.inner.diag(d);
    }
}

const SRC: &str = "\
constexpr int chain(int n) { return n < 1 ? 0 : 1 + chain(n - 1); }
constexpr int arr() {
  int t[8] = {};
  for (int i = 0; i < 8; ++i) t[i] = i;
  return t[7] + chain(10);
}
static_assert(arr() == 17);
constexpr int g(int n) { return n * 2; }
static_assert(g(1) == 2);
constexpr int outer() {
  int a[4] = {1, 2, 3, 4};
  int *p = &a[1];
  int r = g(a[3]);
  return a[0] + r + *p;
}
static_assert(outer() == 11);
";

/// The `static_assert`s of `SRC`, by item index.
const ARR: usize = 0;
const G1: usize = 1;
const OUTER: usize = 2;

fn cond(tu: &Tu, i: usize) -> (ExprId, Loc) {
    match tu.items[i] {
        Item::Assert { cond, loc } => (cond, loc),
        Item::Var { .. } => panic!("item {i} is not a static_assert"),
    }
}

/// Evaluates `static_assert` `outer` with item `nested` evaluated from
/// inside it (when `g` is first compiled): the outer result, the nested
/// result and the records.
fn nest(
    budget: Budget,
    outer: usize,
    nested: usize,
) -> (Result<bool, EvalFailure>, Result<bool, EvalFailure>, Vec<EvalRecord>, Tu) {
    let mut tu = Tu::new(SRC);
    let mut oven = Oven::new(budget);
    let (e, loc) = cond(&tu, outer);
    let (ne, nloc) = cond(&tu, nested);
    let on = tu.names["g"];
    let mut bridge = Nesting { inner: tu.bridge(), on, expr: ne, loc: nloc, result: None };
    let res = oven.evaluate(&mut bridge, e, EvalContext::new(EvalKind::StaticAssert, loc));
    let nres = bridge.result.take().expect("the nested evaluation ran");
    let truth = |tu: &Tu, v: ConstValueId| match tu.ast.consts.scalar(v) {
        Some(kiln_ast::consts::Scalar::Bool { value, .. }) => value,
        s => panic!("not a bool: {s:?}"),
    };
    let res = res.map(|v| truth(&tu, v));
    let nres = nres.map(|v| truth(&tu, v));
    (res, nres, oven.stats().records().to_vec(), tu)
}

/// The record of `static_assert` item `i` evaluated alone on a fresh Oven.
fn alone(i: usize) -> EvalRecord {
    let mut tu = Tu::new(SRC);
    let mut oven = Oven::new(Budget::default());
    let (e, loc) = cond(&tu, i);
    tu.eval(&mut oven, e, loc).expect("ok");
    oven.stats().records()[0]
}

#[test]
fn a_nested_evaluation_runs_inside_a_bridge_callback() {
    let (r, n, recs, _) = nest(Budget::default(), OUTER, ARR);
    assert!(r.expect("outer"));
    assert!(n.expect("nested"));
    // The nested context finishes first; each has its own record.
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].kind, EvalKind::Constraint);
    assert_eq!(recs[0].kind.as_str(), "constraint");
    assert_eq!(recs[1].kind, EvalKind::StaticAssert);
    // Each record counts its own work only: exactly what the expression
    // costs evaluated alone.
    let (o, i) = (alone(OUTER), alone(ARR));
    for (got, want) in [(&recs[1], &o), (&recs[0], &i)] {
        assert_eq!(got.steps, want.steps);
        assert_eq!(got.calls, want.calls);
        assert_eq!(got.bytes, want.bytes);
        assert_eq!(got.peak_bytes, want.peak_bytes);
        assert_eq!(got.peak_arena, want.peak_arena);
    }
    assert!(i.peak_bytes > 0 && o.peak_bytes > 0, "both use frame memory");
    // The nested result was frozen first; the outer's equal `true` is
    // interned to the same constant and adds nothing.
    assert_eq!(recs[0].frozen_bytes, i.frozen_bytes);
    assert_eq!(recs[1].frozen_bytes, 0);
}

#[test]
fn nested_evaluations_share_the_step_budget() {
    let total = alone(OUTER).steps + alone(ARR).steps;
    let ok = Budget { steps: total, ..Budget::default() };
    let (r, n, _, _) = nest(ok, OUTER, ARR);
    assert!(r.is_ok() && n.is_ok());
    let tight = Budget { steps: total - 1, ..Budget::default() };
    let (r, n, _, _) = nest(tight, OUTER, ARR);
    let kinds = [r.err().map(|f| f.kind), n.err().map(|f| f.kind)];
    assert!(kinds.contains(&Some(FailKind::StepLimit)), "{kinds:?}");
}

#[test]
fn nested_calls_count_from_the_depth_that_started_them() {
    // `g` is compiled at depth 1 (`outer` is active); the nested context
    // counts as one more, and `arr()` -> `chain(10)` ... `chain(0)` is 12
    // calls deep on its own: 14 in all.
    let (r, n, _, _) = nest(Budget { depth: 14, ..Budget::default() }, OUTER, ARR);
    assert!(r.is_ok() && n.is_ok());
    let (r, n, _, _) = nest(Budget { depth: 13, ..Budget::default() }, OUTER, ARR);
    assert!(r.is_ok(), "the outer evaluation is not charged for the nested one");
    let f = n.expect_err("too deep");
    assert_eq!(f.kind, FailKind::DepthLimit);
    // A context that starts at the limit fails before running anything.
    let (r, n, recs, tu) = nest(Budget { depth: 2, ..Budget::default() }, OUTER, ARR);
    assert!(r.is_ok());
    let f = n.expect_err("too deep");
    assert_eq!(f.kind, FailKind::DepthLimit);
    assert_eq!(
        tu.render(&f).trim_end(),
        "7:21: constexpr evaluation exceeded maximum depth of 2 calls"
    );
    assert_eq!((recs[0].steps, recs[0].calls), (0, 0));
}

#[test]
fn a_function_whose_body_is_being_made_available_is_undefined() {
    // The nested `g(1) == 2` calls `g` while its body is requested.
    let (r, n, _, tu) = nest(Budget::default(), OUTER, G1);
    assert!(r.expect("outer"), "g is compiled once the callback returns");
    let f = n.expect_err("g is not available yet");
    assert_eq!(f.kind, FailKind::NotConstant);
    let text = tu.render(&f);
    assert!(
        text.starts_with(
            "9:15: undefined function 'g' cannot be used in a constant expression\n\
             8:15: declared here"
        ),
        "{text}"
    );
}

#[test]
fn nested_evaluations_are_not_memoized_through() {
    // `outer` runs a nested evaluation the first time only (when `g` is
    // compiled); a second evaluation finds `g` compiled. Both outer
    // records count the same.
    let mut tu = Tu::new(SRC);
    let mut oven = Oven::new(Budget::default());
    let (e, loc) = cond(&tu, OUTER);
    let (ne, nloc) = cond(&tu, ARR);
    let on = tu.names["g"];
    let mut b = Nesting { inner: tu.bridge(), on, expr: ne, loc: nloc, result: None };
    for _ in 0..2 {
        oven.evaluate(&mut b, e, EvalContext::new(EvalKind::StaticAssert, loc)).expect("ok");
    }
    let recs = oven.stats().records();
    assert_eq!(recs.len(), 3);
    assert_eq!(recs[1].steps, recs[2].steps);
    assert_eq!(recs[1].calls, recs[2].calls);
}

#[test]
fn equal_aggregates_freeze_to_one_constant() {
    let tu = ok("\
struct S { int a; int b[3]; };
constexpr S x = {1, {2, 3, 4}};
constexpr S y = {1, {2, 3, 4}};
constexpr S z = {1, {2, 3, 5}};
");
    assert_eq!(tu.value("x"), tu.value("y"));
    assert_ne!(tu.value("x"), tu.value("z"));
    let mut tu = Tu::new(
        "\
struct S { int a; int b[3]; };
constexpr S x = {1, {2, 3, 4}};
constexpr S y = {1, {2, 3, 4}};
",
    );
    let mut oven = Oven::new(Budget::default());
    tu.run(&mut oven).expect("ok");
    let recs = oven.stats().records();
    assert!(recs[0].frozen_bytes > 0);
    assert_eq!(recs[1].frozen_bytes, 0, "the second freeze interns to the first");
    assert_eq!(oven.stats().totals().frozen_bytes, recs[0].frozen_bytes);
}
