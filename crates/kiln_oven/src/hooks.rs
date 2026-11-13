//! The Oven's instrumentation hooks (spec/oven.md "Trace hook").
//!
//! Every Oven event goes through the `hook_*` methods here, called from
//! a handful of entry points (listed in the spec): a top-level
//! evaluation ([`Oven::evaluate`], [`Oven::evaluate_chars`]), a call
//! entered and returned by the VM, a function compiled to VM code, and a
//! reflection meta operation. Today they feed the `-ftime-trace`
//! recorder ([`kiln_source::trace`]) and the `-fconstexpr-stats` /
//! `-fconstexpr-profile` sink ([`crate::profile`], WP-1116), which
//! aggregates the same begin/end pairs per function and per evaluation
//! site.
//!
//! Off cost: [`Hooks::on`] and [`Hooks::calls`] are plain bools on the
//! Oven, refreshed per top-level evaluation; the VM's per-call hooks
//! test [`Hooks::calls`] (one predictable branch) and are out of line.
use std::time::Instant;

use kiln_ast::{DeclId, ExprId};
use kiln_source::Loc;
use kiln_source::trace::{self, Arg, Span};

use crate::profile::{Level, Profile, Spent};
use crate::{Cost, EvalContext, EvalKind, Oven, SemaBridge, notes};

/// The hooks' switches and the open call marks.
#[derive(Debug, Default)]
pub(crate) struct Hooks {
    /// A sink is listening: evaluations, compiles and meta operations
    /// report (`-ftime-trace`, `-fconstexpr-stats`).
    pub(crate) on: bool,
    /// The per-call hooks report too (`-ftime-trace-verbose`,
    /// `-fconstexpr-profile`: a clock read per call is measurable on
    /// call-heavy evaluations).
    pub(crate) calls: bool,
    /// The trace records call events (`-ftime-trace-verbose`).
    trace_calls: bool,
    /// The calls entered and not yet returned, innermost last; under
    /// [`Level::Calls`], the top-level evaluations in progress too
    /// ([`EVAL`]).
    marks: Vec<CallMark>,
    /// The profile sink (`-fconstexpr-stats`, `-fconstexpr-profile`).
    pub(crate) prof: Option<Box<Profile>>,
}

impl Hooks {
    /// No call or evaluation mark is open (`Oven::snap_ready`).
    pub(crate) fn at_rest(&self) -> bool {
        self.marks.is_empty()
    }
}

/// [`CallMark::func`] of an evaluation's mark: its depth is 0, so no
/// call's end closes it.
const EVAL: u32 = u32::MAX;

/// A call the VM entered while [`Hooks::calls`].
#[derive(Clone, Copy, Debug)]
struct CallMark {
    /// The VM frame depth with the call's frame pushed: the call has
    /// returned (or unwound) once the depth is below it.
    depth: u32,
    /// The callee (`Oven::funcs` index), or [`EVAL`].
    func: u32,
    /// The trace event (`-ftime-trace-verbose`).
    span: Option<Span>,
    /// The profile's start (`-fconstexpr-profile`).
    prof: Option<ProfMark>,
}

/// Where a profiled call or evaluation started.
#[derive(Clone, Copy, Debug)]
struct ProfMark {
    start: Instant,
    /// The step counter at entry.
    steps0: u64,
    /// Live bytes at entry: the call's peak is the most above them.
    live0: u64,
    /// `Memory::peak` at entry, restored (as a maximum) at the end.
    peak_saved: u64,
    /// What the calls and nested evaluations it made spent.
    child_steps: u64,
    child_ns: u64,
    /// `RunState::nesting` at entry: a variable's initialization evaluated
    /// inside restores the step counter, so its steps are not the
    /// enclosing call's.
    nesting: u32,
}

/// A top-level evaluation in progress ([`Oven::hook_eval_begin`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct EvalMark {
    span: Option<Span>,
    marks: usize,
    /// A profile mark was pushed on [`Hooks::marks`].
    prof: bool,
}

impl Hooks {
    /// Reads the sinks' state (the recorder's thread-local switch and
    /// the profile's level).
    pub(crate) fn refresh(&mut self) {
        let trace = trace::enabled();
        let level = self.prof.as_ref().map_or(Level::Off, |p| p.level);
        self.trace_calls = trace && trace::verbose();
        self.on = trace || level != Level::Off;
        self.calls = self.trace_calls || level == Level::Calls;
    }

    fn profiling_calls(&self) -> bool {
        self.prof.as_ref().is_some_and(|p| p.level == Level::Calls)
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
        self.eval_mark(ctx)
    }

    #[inline(never)]
    fn eval_mark(&mut self, ctx: &EvalContext) -> Option<EvalMark> {
        let span = trace::begin_on(eval_event(ctx.kind, ctx.object));
        let prof = self.hooks.profiling_calls();
        if span.is_none() && !prof {
            return None;
        }
        let marks = self.hooks.marks.len();
        if prof {
            // The outermost context restarts the step counter.
            let steps0 = if self.ctx_depth == 0 { 0 } else { self.run.counters.steps };
            let pm = ProfMark {
                start: Instant::now(),
                steps0,
                live0: 0,
                peak_saved: 0,
                child_steps: 0,
                child_ns: 0,
                nesting: self.run.nesting,
            };
            self.hooks.marks.push(CallMark { depth: 0, func: EVAL, span: None, prof: Some(pm) });
        }
        Some(EvalMark { span, marks, prof })
    }

    /// Ends the evaluation begun by [`Oven::hook_eval_begin`]: its calls
    /// still open (a failure or an exception left them) end now. The
    /// detail is the variable's qualified name or the expression `e`'s
    /// source range (`ctx.loc` without one); `-ftime-trace-verbose` adds
    /// the kind and steps. The profile charges the evaluation's site.
    #[inline(never)]
    pub(crate) fn hook_eval_end(
        &mut self,
        b: &dyn SemaBridge,
        m: EvalMark,
        e: Option<ExprId>,
        ctx: &EvalContext,
        c: &Cost,
        ok: bool,
    ) {
        let now = Instant::now();
        let base = m.marks + usize::from(m.prof);
        while self.hooks.marks.len() > base {
            self.end_call_mark(b, now);
        }
        if m.prof
            && let Some(CallMark { prof: Some(pm), .. }) = self.hooks.marks.pop()
        {
            let s = self.spent(pm, now, c.peak);
            if let Some(p) = self.hooks.prof.as_mut() {
                p.site_end(ctx.loc, ctx.kind, ctx.object, ok, s);
            }
        }
        let Some(span) = m.span else { return };
        let steps = c.steps;
        trace::end_at(
            span,
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
        let span = if self.hooks.trace_calls { trace::begin_on(name) } else { None };
        let prof = self.hooks.prof.as_mut().filter(|p| p.level == Level::Calls).map(|p| {
            p.func_enter(func);
            let live0 = self.mem.live_bytes();
            let pm = ProfMark {
                start: Instant::now(),
                steps0: self.run.counters.steps,
                live0,
                peak_saved: self.mem.peak,
                child_steps: 0,
                child_ns: 0,
                nesting: self.run.nesting,
            };
            // The call's own peak: `Memory::peak` restarts at what is
            // live now and is raised back at its end.
            self.mem.peak = live0;
            pm
        });
        if span.is_some() || prof.is_some() {
            self.hooks.marks.push(CallMark { depth, func, span, prof });
        }
    }

    /// The VM answered a call of `func` from the memo table, charging
    /// `steps` and a peak of `peak` bytes. Called only while
    /// [`Hooks::calls`].
    #[cold]
    #[inline(never)]
    pub(crate) fn hook_memo_hit(&mut self, func: u32, steps: u64, peak: u64) {
        let Some(p) = self.hooks.prof.as_mut().filter(|p| p.level == Level::Calls) else {
            return;
        };
        p.func_enter(func).memo_hits += 1;
        p.func_end(func, Spent { steps, peak, ..Spent::default() });
        let nesting = self.run.nesting;
        if let Some(pm) = self.hooks.marks.last_mut().and_then(|m| m.prof.as_mut())
            && pm.nesting == nesting
        {
            pm.child_steps += steps;
        }
    }

    /// The VM unwound to frame depth `depth` (a failure, a trial's
    /// end): the calls above it end now, before the memo table restores
    /// the peak. Called only while [`Hooks::calls`].
    #[cold]
    #[inline(never)]
    pub(crate) fn hook_call_unwind(&mut self, b: &dyn SemaBridge, depth: usize) {
        let now = Instant::now();
        let depth = depth as u32;
        while self.hooks.marks.last().is_some_and(|m| m.depth > depth) {
            self.end_call_mark(b, now);
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
        if let Some(pm) = m.prof {
            let peak = self.mem.peak.saturating_sub(pm.live0);
            self.mem.peak = self.mem.peak.max(pm.peak_saved);
            let s = self.spent(pm, now, peak);
            if let Some(p) = self.hooks.prof.as_mut() {
                p.func_end(m.func, s);
            }
        }
        let Some(span) = m.span else { return };
        let decl = self.funcs[m.func as usize].decl;
        trace::end_at(
            span,
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

    /// What the call or evaluation begun at `pm` spent, ending `now`
    /// with a peak of `peak` bytes; charged to the enclosing mark's
    /// children.
    fn spent(&mut self, pm: ProfMark, now: Instant, peak: u64) -> Spent {
        let ns = u64::try_from(now.saturating_duration_since(pm.start).as_nanos()).unwrap_or(0);
        let steps = self.run.counters.steps.saturating_sub(pm.steps0);
        if let Some(up) = self.hooks.marks.last_mut().and_then(|m| m.prof.as_mut()) {
            up.child_ns += ns;
            if up.nesting == pm.nesting {
                up.child_steps += steps;
            }
        }
        Spent { steps, child_steps: pm.child_steps, ns, child_ns: pm.child_ns, peak }
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
        let start = self.hooks.prof.is_some().then(Instant::now);
        self.compile_func(b, func);
        if let (Some(t), Some(p)) = (start, self.hooks.prof.as_mut()) {
            p.compiles += 1;
            p.compile_ns += u64::try_from(t.elapsed().as_nanos()).unwrap_or(0);
        }
        if let Some(s) = span {
            let decl = self.funcs[func as usize].decl;
            let ast = b.ast();
            trace::end(s, || decl.map_or_else(String::new, |d| notes::qualified_name(ast, d)));
        }
    }

    /// Begins a reflection meta operation's `OvenMetaOp` event (and
    /// counts it for the profile).
    #[inline]
    pub(crate) fn hook_meta_begin(&mut self) -> Option<Span> {
        if !self.hooks.on {
            return None;
        }
        if let Some(p) = self.hooks.prof.as_mut() {
            p.meta_ops += 1;
        }
        trace::begin_on("OvenMetaOp")
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
