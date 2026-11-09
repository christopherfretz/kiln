//! The Oven's instrumentation hooks (spec/oven.md "Trace hook").
//!
//! Every Oven event goes through the `hook_*` methods here, called from
//! a handful of entry points (listed in the spec): a top-level
//! evaluation ([`Oven::evaluate`], [`Oven::evaluate_chars`]), a call
//! entered and returned by the VM, a function compiled to VM code, and a
//! reflection meta operation. Today they feed the `-ftime-trace`
//! recorder ([`kiln_source::trace`]); `-fconstexpr-stats` /
//! `-fconstexpr-profile` (WP-1116) aggregates the same begin/end pairs
//! per call site by adding a sink here, not a second set of call sites.
//!
//! Off cost: [`Hooks::on`] and [`Hooks::calls`] are plain bools on the
//! Oven, refreshed per top-level evaluation; the VM's per-call hooks
//! test [`Hooks::calls`] (one predictable branch) and are out of line.
use std::time::Instant;

use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;
use kiln_source::trace::{self, Arg, Span};

use crate::{EvalContext, EvalKind, Oven, SemaBridge, notes};

/// The hooks' switches and the open call marks.
#[derive(Debug, Default)]
pub(crate) struct Hooks {
    /// A sink is listening: evaluations, compiles and meta operations
    /// report (`-ftime-trace`).
    pub(crate) on: bool,
    /// The per-call hooks report too (`-ftime-trace-verbose`: a clock
    /// read per call is measurable on call-heavy evaluations).
    pub(crate) calls: bool,
    /// The calls entered and not yet returned, innermost last.
    marks: Vec<CallMark>,
}

/// A call the VM entered while [`Hooks::calls`].
#[derive(Clone, Copy, Debug)]
struct CallMark {
    /// The VM frame depth with the call's frame pushed: the call has
    /// returned (or unwound) once the depth is below it.
    depth: u32,
    /// The callee (`Oven::funcs` index).
    func: u32,
    span: Span,
}

/// A top-level evaluation in progress ([`Oven::hook_eval_begin`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct EvalMark {
    span: Span,
    marks: usize,
}

impl Hooks {
    /// Reads the sinks' state (the recorder's thread-local switch).
    fn refresh(&mut self) {
        self.on = trace::enabled();
        self.calls = self.on && trace::verbose();
    }
}

/// clang's event for a top-level evaluation of `kind`: a variable's
/// initializer is `EvaluateAsInitializer`, a `static_assert` condition
/// `EvaluateAsRValue` (clang's `VerifyIntegerConstantExpression`), any
/// other `EvaluateAsConstantExpr`.
fn eval_event(kind: EvalKind, object: Option<DeclId>) -> &'static str {
    match (kind, object) {
        (EvalKind::Init, Some(_)) => "EvaluateAsInitializer",
        (EvalKind::StaticAssert, _) => "EvaluateAsRValue",
        _ => "EvaluateAsConstantExpr",
    }
}

impl Oven {
    /// Begins a top-level evaluation's event (`EvaluateAsInitializer`
    /// for a variable's initializer, else `EvaluateAsConstantExpr`).
    #[inline]
    pub(crate) fn hook_eval_begin(&mut self, ctx: &EvalContext) -> Option<EvalMark> {
        if self.ctx_depth == 0 {
            self.hooks.refresh();
        }
        if !self.hooks.on {
            return None;
        }
        let span = trace::begin_on(eval_event(ctx.kind, ctx.object))?;
        Some(EvalMark { span, marks: self.hooks.marks.len() })
    }

    /// Ends the evaluation begun by [`Oven::hook_eval_begin`]: its calls
    /// still open (a failure or an exception left them) end now. The
    /// detail is the variable's qualified name or the expression `e`'s
    /// source range (`ctx.loc` without one); `-ftime-trace-verbose` adds
    /// the kind and steps.
    #[inline(never)]
    pub(crate) fn hook_eval_end(
        &mut self,
        b: &dyn SemaBridge,
        m: EvalMark,
        e: Option<ExprId>,
        ctx: &EvalContext,
        steps: u64,
    ) {
        let now = Instant::now();
        while self.hooks.marks.len() > m.marks {
            self.end_call_mark(b, now);
        }
        trace::end_at(
            m.span,
            now,
            || match ctx.object.filter(|_| ctx.kind == EvalKind::Init) {
                Some(d) => notes::qualified_name(b.ast(), d),
                None => loc_range(b, e, ctx.loc),
            },
            || {
                if !trace::verbose() {
                    return Vec::new();
                }
                vec![
                    ("kind".into(), Arg::Str(ctx.kind.as_str().to_owned())),
                    ("steps".into(), Arg::Int(i64::try_from(steps).unwrap_or(i64::MAX))),
                ]
            },
        );
    }

    /// The VM entered `func` (its frame is pushed). Called only while
    /// [`Hooks::calls`].
    #[cold]
    #[inline(never)]
    pub(crate) fn hook_call_enter(&mut self, b: &dyn SemaBridge, func: u32) {
        let depth = self.stack.len() as u32;
        // Calls at this depth or deeper were unwound (a failed trial, an
        // exception) without a `Ret`: they end now, not at the caller's
        // return (else a loop of failed trials holds one open call each).
        if self.hooks.marks.last().is_some_and(|m| m.depth >= depth) {
            let now = Instant::now();
            while self.hooks.marks.last().is_some_and(|m| m.depth >= depth) {
                self.end_call_mark(b, now);
            }
        }
        let ast = b.ast();
        let immediate = self.funcs[func as usize]
            .decl
            .and_then(|d| ast.decls.function(ast.latest(d)))
            .is_some_and(|f| f.spec.is_immediate());
        let name = if immediate { "ConstevalCall" } else { "ConstexprCall" };
        if let Some(span) = trace::begin_on(name) {
            self.hooks.marks.push(CallMark { depth, func, span });
        }
    }

    /// The VM returned from a frame (it is popped): the calls whose
    /// frames are gone end. Called only while [`Hooks::calls`].
    #[cold]
    #[inline(never)]
    pub(crate) fn hook_call_ret(&mut self, b: &dyn SemaBridge) {
        let now = Instant::now();
        let depth = self.stack.len() as u32;
        while self.hooks.marks.last().is_some_and(|m| m.depth > depth) {
            self.end_call_mark(b, now);
        }
    }

    fn end_call_mark(&mut self, b: &dyn SemaBridge, now: Instant) {
        let Some(m) = self.hooks.marks.pop() else { return };
        let decl = self.funcs[m.func as usize].decl;
        trace::end_at(
            m.span,
            now,
            || {
                decl.map_or_else(
                    || "<expression>".to_owned(),
                    |d| notes::qualified_name(b.ast(), d),
                )
            },
            Vec::new,
        );
    }

    /// Compiles `func` to VM code inside an `OvenCompile` event (the
    /// detail: its qualified name).
    #[inline]
    pub(crate) fn hook_compile(&mut self, b: &mut dyn SemaBridge, func: u32) {
        if !self.hooks.on {
            self.compile_func(b, func);
            return;
        }
        self.traced_compile(b, func);
    }

    #[inline(never)]
    fn traced_compile(&mut self, b: &mut dyn SemaBridge, func: u32) {
        let span = trace::begin_on("OvenCompile");
        self.compile_func(b, func);
        if let Some(s) = span {
            let decl = self.funcs[func as usize].decl;
            let ast = b.ast();
            trace::end(s, || decl.map_or_else(String::new, |d| notes::qualified_name(ast, d)));
        }
    }

    /// Begins a reflection meta operation's `OvenMetaOp` event.
    #[inline]
    pub(crate) fn hook_meta_begin(&self) -> Option<Span> {
        if self.hooks.on { trace::begin_on("OvenMetaOp") } else { None }
    }

    /// Ends it: the detail is the operation's name (`members_of`).
    #[inline(never)]
    pub(crate) fn hook_meta_end(span: Span, op: crate::meta::MetaOp) {
        trace::end(span, || op.display());
    }
}

/// `e`'s source range (else `loc`) as clang writes it in a trace detail.
fn loc_range(b: &dyn SemaBridge, e: Option<ExprId>, loc: Loc) -> String {
    let Some(sm) = b.sources() else { return String::new() };
    match e {
        Some(e) => trace::range_detail(sm, b.ast().begin_loc(e), b.ast().end_loc(e)),
        None => trace::range_detail(sm, loc, loc),
    }
}
