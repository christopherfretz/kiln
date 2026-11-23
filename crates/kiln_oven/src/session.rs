//! The Oven's side of an evaluation session (spec/eval-session.md): the
//! per-run budget, the cancellation token, a run's value and outcome,
//! and the run itself ([`Oven::session_run`]), which evaluates a call of
//! an entry function against the retained state of a finished
//! translation unit and leaves nothing a later run can observe.
//!
//! Outside a session nothing here runs: the dispatch's step check
//! compares against [`Oven`]'s `step_stop`, which equals the step budget
//! then, and the compile path tests one `Option` ([`Oven::compile_func`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kiln_ast::{ConstValueId, DeclId};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::compile::{FState, RetKind};
use crate::failure::Note;
use crate::host::HostMode;
use crate::mem::Ptr;
use crate::ty::{OTy, TyKind};
use crate::value::Freeze;
use crate::vm::Fault;
use crate::{Budget, EvalFailure, FailKind, Oven, SemaBridge};

/// A run checks its [`CancelToken`] at least every this many steps
/// (spec/eval-session.md §8).
pub const CANCEL_INTERVAL: u64 = 65_536;

/// After a run, the frame and heap spaces' buffers are released down to
/// this many bytes each (spec/eval-session.md §10).
pub const RELEASE_FLOOR: usize = 1 << 20;

/// At most this many [`RunError::Uncaught`] scalars.
pub const UNCAUGHT_SCALARS: usize = 4;

/// The note of a host call refused in semantic work a run triggered
/// (spec/eval-session.md §6), in the author's terms (WP-2325).
pub(crate) const NESTED_HOST_NOTE: &str = "a host call cannot be evaluated here: this expression \
     is evaluated when the code is instantiated, not when it runs";

/// The limits of one run (spec/eval-session.md §8). A client that builds
/// it literally writes `..RunBudget::PROGRAM` for the fields it leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunBudget {
    /// Steps the run may execute (spec/oven.md "Limits").
    pub steps: u64,
    /// Bytes of abstract memory (frame and heap) the run may hold.
    pub bytes: u64,
    /// Nested calls the run may make.
    pub depth: u32,
    /// Host bytes the result's tree may take (spec/eval-session.md §4.1).
    pub result_bytes: u64,
}

impl RunBudget {
    /// The program profile: 2^34 steps, 1 GiB, 16384 nested calls, a
    /// 64 MiB result.
    pub const PROGRAM: RunBudget =
        RunBudget { steps: 1 << 34, bytes: 1 << 30, depth: 16_384, result_bytes: 64 << 20 };

    /// This budget with `steps` steps.
    #[must_use]
    pub fn with_steps(self, steps: u64) -> Self {
        RunBudget { steps, ..self }
    }

    /// This budget with `bytes` bytes of abstract memory.
    #[must_use]
    pub fn with_bytes(self, bytes: u64) -> Self {
        RunBudget { bytes, ..self }
    }

    /// This budget with `depth` nested calls.
    #[must_use]
    pub fn with_depth(self, depth: u32) -> Self {
        RunBudget { depth, ..self }
    }

    /// This budget with a result of at most `result_bytes` host bytes.
    #[must_use]
    pub fn with_result_bytes(self, result_bytes: u64) -> Self {
        RunBudget { result_bytes, ..self }
    }
}

impl Default for RunBudget {
    fn default() -> Self {
        RunBudget::PROGRAM
    }
}

/// Stops a run from any thread (spec/eval-session.md §8). Clones share
/// one flag.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels: a run given this token (or a clone) stops at its next
    /// check, and every later run given it fails at its first.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether [`CancelToken::cancel`] was called since the last reset.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Makes the token usable again.
    pub fn reset(&self) {
        self.0.store(false, Ordering::Release);
    }
}

/// What a successful run gives the session (spec/eval-session.md §4.1):
/// a `void` or integral result read from the stub's register, or a value
/// the session read through [`SemaBridge::session_result`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunRaw {
    /// A `void` entry.
    Void,
    /// A `bool` result.
    Bool(bool),
    /// A result of a signed integral type.
    Signed(i128),
    /// A result of an unsigned integral type (the character types by
    /// their signedness).
    Unsigned(u128),
    /// Any other result: frozen and read by the session.
    Read,
}

/// Why a frozen result cannot cross to a session's client
/// ([`SemaBridge::session_result`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultReadError {
    /// The tree would exceed the budget's `result_bytes` (the cap).
    TooLarge(u64),
    /// The value has no form for the client (an internal limit; the
    /// note's text).
    Unreadable(String),
}

/// How a run failed (spec/eval-session.md §8). Every kind comes with its
/// diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RunError {
    /// Not a constant expression.
    Failed,
    /// The budget's steps were exceeded.
    StepBudget,
    /// The budget's bytes of abstract memory were exceeded.
    MemoryBudget,
    /// The budget's nested calls were exceeded.
    DepthLimit,
    /// The run's [`CancelToken`] was cancelled.
    Cancelled,
    /// A host call failed: its handler returned an error, or its
    /// arguments failed a check (the note's text).
    Host {
        /// The failure's note.
        message: String,
    },
    /// An exception left the entry.
    Uncaught {
        /// The exception's type.
        type_name: String,
        /// Its `what()`, when it derives from `std::exception`.
        what: Option<String>,
        /// The values of the thrown object's first [`UNCAUGHT_SCALARS`]
        /// non-static data members of integral or enumeration type, in
        /// declaration order, a base class's before the class's own; a
        /// signed value in two's complement (sign-extended to 64 bits). Empty
        /// for an object not of class type (spec/eval-session.md §8,
        /// WP-2324).
        scalars: Vec<u64>,
    },
    /// The result's tree would exceed the budget's `result_bytes`
    /// (spec/eval-session.md §4.1).
    ResultTooLarge,
}

/// What a unit of nested work cost (spec/eval-session.md §5.1): its
/// nested evaluations' steps and calls summed, their peak bytes and call
/// depth the largest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NestedCost {
    /// Steps executed.
    pub steps: u64,
    /// Function calls made.
    pub calls: u64,
    /// Peak live frame and heap bytes.
    pub peak: u64,
    /// Peak address space.
    pub arena: u64,
    /// Nested call depth.
    pub depth: u32,
}

impl NestedCost {
    /// Nothing.
    pub const ZERO: NestedCost = NestedCost { steps: 0, calls: 0, peak: 0, arena: 0, depth: 0 };

    /// Whether this costs nothing.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == NestedCost::ZERO
    }

    /// Adds `o`: steps and calls sum, the peaks and depth take the larger.
    pub fn add(&mut self, o: NestedCost) {
        self.steps = self.steps.saturating_add(o.steps);
        self.calls = self.calls.saturating_add(o.calls);
        self.peak = self.peak.max(o.peak);
        self.arena = self.arena.max(o.arena);
        self.depth = self.depth.max(o.depth);
    }
}

/// A unit of nested work the Oven sees (spec/eval-session.md §5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SessUnit {
    /// The compilation of the Oven's function (by index).
    Func(u32),
    /// The evaluation of the initializer of the Oven's global (by index).
    Var(u32),
}

/// What [`Oven::session_run`] produced.
#[derive(Debug)]
pub struct RunOutcome {
    /// The value, or the kind of failure with its notes.
    pub result: Result<RunRaw, (RunError, EvalFailure)>,
    /// Steps the run executed itself.
    pub steps: u64,
    /// Steps of the nested work the run was charged (§5.1).
    pub nested_steps: u64,
    /// Calls of the nested work the run was charged.
    pub nested_calls: u64,
    /// Handler calls the run made.
    pub host_calls: u64,
    /// The run's peak live frame and heap bytes.
    pub peak_heap: u64,
    /// Function calls the run made itself.
    pub calls: u64,
}

/// The session state an Oven keeps while a session is open.
#[derive(Debug, Default)]
pub(crate) struct SessionState {
    /// The compiled call stub of each entry run so far.
    stubs: FxHashMap<DeclId, u32>,
    /// The functions whose compilation in a run was recorded (it
    /// reported diagnostics or did nested work, §5.1), with their real
    /// state: they stay stubs while the session is open, so every call
    /// reaches the record and is impure (no memoized result skips it).
    tracked: FxHashMap<u32, FState>,
    /// The globals whose initializer's evaluation in a run was recorded,
    /// with their object: unresolved again after every run, so the next
    /// run's first read reaches the record.
    kept_vars: FxHashMap<u32, u64>,
    /// The tracked functions reached by the running run outside nested
    /// work (a later call there needs no reach).
    reached: FxHashSet<u32>,
    /// The running run's state.
    pub(crate) arm: Option<Arm>,
}

/// Why the next step check stops a run that was charged past its budget
/// (spec/eval-session.md §5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stop {
    /// The step budget (or the run's charge over it).
    Limit,
    /// The cancellation token.
    Cancelled,
    /// Charged nested work's bytes over the budget.
    Memory,
    /// Charged nested work's depth over the budget.
    Depth,
}

/// A run in progress.
#[derive(Debug)]
pub(crate) struct Arm {
    cancel: CancelToken,
    /// The translation unit's budget and memory limit (nested
    /// evaluations run under them, §5).
    tu: Budget,
    tu_limit: u64,
    /// The run's own budget.
    run: RunBudget,
    /// Nested evaluations open: no cancellation and no fault
    /// classification inside them.
    nested: u32,
    /// The kind of the run's last fault.
    fault: Option<RunError>,
    /// The nested work charged so far (§5.1).
    nested_steps: u64,
    nested_calls: u64,
    /// The run's peak with its charged nested work's.
    peak: u64,
    /// Charges made while a nested evaluation is open, applied when the
    /// last one ends.
    pending: NestedCost,
    /// The charge put the run over its budget: the next step check stops.
    over: Option<Stop>,
}

/// What [`Oven::session_nested_begin`] saved.
pub(crate) struct NestedSaved {
    budget: Budget,
    limit: u64,
    step_stop: u64,
}

/// What [`Oven::session_var_begin`] saved.
pub(crate) struct VarSaved {
    nested: NestedSaved,
    mark: usize,
    depth_base: u32,
    maxd: u32,
    peak: u64,
    live0: u64,
    arena_peak: u64,
    arena0: u64,
}

impl Oven {
    /// Opens a session on this Oven (spec/eval-session.md): later calls
    /// may run entries. Without one, [`Oven::session_run`] fails.
    pub fn session_begin(&mut self) {
        if self.sess.is_none() {
            self.sess = Some(Box::default());
        }
    }

    /// Closes the session: every function set back to a stub gets its
    /// real state again.
    pub fn session_end(&mut self) {
        if let Some(s) = self.sess.take() {
            for (idx, state) in s.tracked {
                self.funcs[idx as usize].state = state;
            }
            for (g, p) in s.kept_vars {
                self.globals[g as usize].state = crate::compile::GState::Resolved(p);
            }
        }
    }

    /// Whether a session is open.
    pub fn in_session(&self) -> bool {
        self.sess.is_some()
    }

    /// Drops every memoized result; how many were dropped.
    pub fn clear_memo(&mut self) -> usize {
        self.memo.clear()
    }

    /// How many memoized results the cache holds.
    pub fn memo_len(&self) -> usize {
        self.memo.entries()
    }

    /// Runs entry `f` (a `constexpr` or `consteval` function without
    /// parameters returning `void` or an integral type; the caller
    /// checked) as a manifestly constant-evaluated call that is not
    /// plainly constant-evaluated (spec/eval-session.md §4) under
    /// `budget`, stopping when `cancel` is cancelled. `at` is the
    /// location the call is reported at. Host calls reach `b`'s handler
    /// (`SemaBridge::host_calls`).
    pub fn session_run(
        &mut self,
        b: &mut dyn SemaBridge,
        f: DeclId,
        at: Loc,
        budget: RunBudget,
        cancel: &CancelToken,
    ) -> RunOutcome {
        let fail = |k: RunError| RunOutcome {
            result: Err((k, EvalFailure::silent(FailKind::NotConstant))),
            steps: 0,
            nested_steps: 0,
            nested_calls: 0,
            host_calls: 0,
            peak_heap: 0,
            calls: 0,
        };
        if self.ctx_depth != 0 || self.sess.is_none() {
            return fail(RunError::Failed);
        }
        let _phase = kiln_source::activity::enter(kiln_source::activity::Phase::Oven, at);
        self.types.refresh(b);
        self.mp_refresh(b);
        let stub = match self.sess.as_ref().and_then(|s| s.stubs.get(&f)) {
            Some(&s) => s,
            None => {
                let Some(s) = self.compile_call(b, f, at, true) else {
                    return fail(RunError::Failed);
                };
                if let Some(st) = self.sess.as_mut() {
                    st.stubs.insert(f, s);
                }
                s
            }
        };
        let tu = self.budget;
        let tu_limit = self.mem.limit;
        self.budget.steps = budget.steps;
        self.budget.bytes = budget.bytes;
        self.budget.depth = budget.depth;
        self.mem.limit = budget.bytes;
        if let Some(s) = self.sess.as_mut() {
            s.arm = Some(Arm {
                cancel: cancel.clone(),
                tu,
                tu_limit,
                run: budget,
                nested: 0,
                fault: None,
                nested_steps: 0,
                nested_calls: 0,
                peak: 0,
                pending: NestedCost::ZERO,
                over: None,
            });
            s.reached.clear();
        }
        let inner = std::mem::take(&mut self.inner);
        let calls0 = self.host.calls;
        let mode = self.host_enter(HostMode::Allowed);
        self.begin_outermost(false, true, true);
        // The first step checks the token.
        self.step_stop = 0;
        self.run.object = None;
        self.let_pending.clear();
        let r = self.exec(b, stub, 0).and_then(|()| {
            // Charged past the budget after the last step check.
            let loose = b.session_settle();
            self.session_charge(loose);
            if let Some(stop) = self.sess.as_ref().and_then(|s| s.arm.as_ref()).and_then(|a| a.over)
            {
                return Err(self.session_over_failure(b, stop, at));
            }
            self.session_result(b, f, stub, at)
        });
        let c = self.end_outermost();
        self.host_leave(mode);
        self.inner = inner;
        self.budget = tu;
        self.mem.limit = tu_limit;
        self.step_stop = tu.steps;
        let arm = self.sess.as_mut().and_then(|s| s.arm.take());
        self.session_reset_vars();
        self.release_spaces();
        let (nested_steps, nested_calls, peak) =
            arm.as_ref().map_or((0, 0, 0), |a| (a.nested_steps, a.nested_calls, a.peak));
        let result = r.map_err(|e| {
            let k = arm.and_then(|a| a.fault).unwrap_or(RunError::Failed);
            (k, e)
        });
        RunOutcome {
            result,
            steps: c.steps,
            nested_steps,
            nested_calls,
            host_calls: self.host.calls - calls0,
            peak_heap: c.peak.max(peak),
            calls: c.calls,
        }
    }

    /// The value of the run of entry `f` whose stub `stub` returned
    /// (spec/eval-session.md §4.1): a `void` or integral result from the
    /// register; any other frozen (the checks of `constexpr R v = f();`)
    /// and read by the session; then the leak check and, for a class
    /// result of a `constexpr` entry, its constant destruction (as
    /// [`Oven::eval_at`] for the variable).
    fn session_result(
        &mut self,
        b: &mut dyn SemaBridge,
        f: DeclId,
        stub: u32,
        at: Loc,
    ) -> Result<RunRaw, EvalFailure> {
        let integral = {
            let ast = b.ast();
            ast.decls.get(f).ty().and_then(|t| match ast.types.canonical_view(t) {
                kiln_ast::types::TypeView::Function(p) => Some(ast.types.is_integral(p.ret())),
                _ => None,
            })
        } == Some(true);
        let ret = self.funcs[stub as usize].ret;
        let raw = self.regs[0];
        let notes = |f: Freeze| match f {
            Freeze::Fault(k) => EvalFailure::silent(k),
            Freeze::Notes(notes) => EvalFailure { kind: FailKind::NotConstant, notes },
        };
        let v = match ret {
            RetKind::Scalar(ty) if integral => match self.types.kind(ty) {
                TyKind::Bool => RunRaw::Bool(raw & 1 != 0),
                TyKind::Int { width, signed: true } => {
                    RunRaw::Signed(crate::arith::canon(raw, u32::from(width), true) as i128)
                }
                TyKind::Int { width, signed: false } => {
                    RunRaw::Unsigned(crate::arith::canon(raw, u32::from(width), false))
                }
                _ => return Err(EvalFailure::silent(FailKind::NotConstant)),
            },
            RetKind::Scalar(_) | RetKind::Sret(_) => {
                let mark = b.consts().mark();
                let cv = match ret {
                    RetKind::Scalar(ty) => self.freeze_scalar(b, ty, raw, at),
                    _ => self.freeze_object(b, raw as u64, at),
                };
                match cv {
                    Ok(cv) => self.session_read(b, cv, mark, at)?,
                    Err(e) => {
                        // What a refused freeze interned is dropped too.
                        b.consts().truncate(mark);
                        return Err(notes(e));
                    }
                }
            }
            RetKind::Void => RunRaw::Void,
        };
        if let Some(notes) = self.leak_notes(b) {
            return Err(EvalFailure { kind: FailKind::NotConstant, notes });
        }
        // [dcl.constexpr]/6: the notional variable has constant
        // destruction ([expr.const.defns]/3.1).
        if let RetKind::Sret(ty) = ret
            && b.session_destroys_result(f)
            && let Some(df) = self.compile_destroy(b, f, ty)
        {
            self.regs[0] = raw;
            self.exec(b, df, 0)
                .map_err(|e| EvalFailure { kind: FailKind::Destruction, notes: e.notes })?;
        }
        Ok(v)
    }

    /// Hands frozen result `cv` (above `mark`) to the session to read.
    fn session_read(
        &mut self,
        b: &mut dyn SemaBridge,
        cv: ConstValueId,
        mark: kiln_ast::consts::ConstMark,
        at: Loc,
    ) -> Result<RunRaw, EvalFailure> {
        match b.session_result(cv, mark) {
            Ok(()) => Ok(RunRaw::Read),
            Err(e) => {
                let msg = match e {
                    ResultReadError::TooLarge(cap) => {
                        if let Some(a) = self.sess.as_mut().and_then(|s| s.arm.as_mut()) {
                            a.fault = Some(RunError::ResultTooLarge);
                        }
                        format!(
                            "the result needs more than {cap} bytes on the host (the run's result budget)"
                        )
                    }
                    ResultReadError::Unreadable(m) => m,
                };
                Err(EvalFailure { kind: FailKind::NotConstant, notes: vec![Note::new(at, msg)] })
            }
        }
    }

    /// [`step_over`] of this Oven.
    pub(crate) fn step_over(&mut self, need: u64) -> Result<(), Stop> {
        step_over(&mut self.step_stop, self.budget.steps, self.sess.as_deref(), need)
    }

    /// Whether nested work of a session's run is running (its host calls
    /// are refused, §6).
    pub(crate) fn session_in_nested(&self) -> bool {
        self.sess.as_ref().and_then(|s| s.arm.as_ref()).is_some_and(|a| a.nested != 0)
    }

    /// Whether a host call of the running run is cancelled (checked
    /// before each one).
    pub(crate) fn host_cancelled(&self) -> bool {
        self.sess
            .as_ref()
            .and_then(|s| s.arm.as_ref())
            .is_some_and(|a| a.nested == 0 && a.cancel.is_cancelled())
    }

    /// Whether the running evaluation is a session run's own (its
    /// outermost context, not nested work).
    pub(crate) fn session_run_outermost(&self) -> bool {
        self.ctx_depth == 1
            && self.sess.as_ref().and_then(|s| s.arm.as_ref()).is_some_and(|a| a.nested == 0)
    }

    /// Records the kind of fault `fault` of the run's outermost context
    /// (from `exec`, before it is rendered).
    #[cold]
    pub(crate) fn session_note_fault(&mut self, b: &dyn SemaBridge, fault: &Fault) {
        if !self.session_run_outermost() {
            return;
        }
        let k = match fault {
            Fault::Cancelled => RunError::Cancelled,
            // The handler's own message, or what the call's check found.
            Fault::Host(m) if !m.is_empty() => RunError::Host {
                message: m.strip_prefix("host call failed: ").unwrap_or(m).to_owned(),
            },
            Fault::Uncaught { obj, ty, what } => RunError::Uncaught {
                type_name: self.tname(b.ast(), *ty),
                what: what.as_deref().map(str::to_owned),
                scalars: self.uncaught_scalars(b.ast(), *obj, *ty),
            },
            f => match f.kind() {
                FailKind::StepLimit => RunError::StepBudget,
                FailKind::DepthLimit => RunError::DepthLimit,
                FailKind::MemoryLimit => RunError::MemoryBudget,
                _ => RunError::Failed,
            },
        };
        if let Some(a) = self.sess.as_mut().and_then(|s| s.arm.as_mut()) {
            a.fault = Some(k);
        }
    }

    /// [`RunError::Uncaught`]'s scalars of the exception object `obj` of
    /// type `ty` (read before the failure is rendered, while the object
    /// lives): none unless `ty` is a class that is not a union.
    fn uncaught_scalars(&self, ast: &kiln_ast::Ast, obj: u64, ty: OTy) -> Vec<u64> {
        let mut out = Vec::new();
        if matches!(self.types.kind(ty), TyKind::Record { union: false, .. }) {
            self.scalars_into(ast, obj, ty, &mut out);
        }
        out
    }

    /// Appends the integral and enumeration members of the object of class
    /// type `t` at `raw` to `out`, its direct non-virtual bases' first (in
    /// base-specifier order, each the same way), until `out` holds
    /// [`UNCAUGHT_SCALARS`]. A bit-field counts with its value; an unnamed
    /// one is no member and does not; a virtual base, a member of class,
    /// array, pointer or floating type, or a union member is skipped. A
    /// member whose value is not initialized ends the list. `false` once
    /// the list is complete.
    fn scalars_into(&self, ast: &kiln_ast::Ast, raw: u64, t: OTy, out: &mut Vec<u64>) -> bool {
        let base_off = u64::from(Ptr::decode(raw).off);
        for f in self.types.fields(t) {
            if out.len() >= UNCAUGHT_SCALARS {
                return false;
            }
            let at = Ptr::with_off(raw, base_off + f.offset, false);
            if f.base {
                if matches!(self.types.kind(f.ty), TyKind::Record { union: false, .. })
                    && !self.scalars_into(ast, at, f.ty, out)
                {
                    return false;
                }
                continue;
            }
            let (width, signed) = match self.types.kind(f.ty) {
                TyKind::Bool => (1, false),
                TyKind::Int { width, signed } => (u32::from(width), signed),
                _ => continue,
            };
            let v = match f.width {
                Some(w) => {
                    if w == 0 || ast.decls.get(f.decl).name().is_empty() {
                        continue;
                    }
                    let n = u64::from((f.bit + w).div_ceil(8));
                    let bits = self.mem.check(at, n, false).ok().and_then(|(st, abs, _)| {
                        let sp = self.mem.space(st);
                        sp.is_init(abs, n).then(|| sp.read(abs, n))
                    });
                    let Some(bits) = bits else { return false };
                    let m = if w >= 128 { u128::MAX } else { (1u128 << w) - 1 };
                    crate::arith::canon((bits >> f.bit) & m, w, signed)
                }
                None => match self.load(at, f.ty, crate::kbc::ak::READ) {
                    Ok(v) => crate::arith::canon(v, width, signed),
                    Err(_) => return false,
                },
            };
            // The low 64 bits of the two's-complement value.
            out.push(v as u64);
        }
        out.len() < UNCAUGHT_SCALARS
    }

    /// A nested evaluation starts inside a run: it runs under the
    /// translation unit's limits, counted from zero, not cancellable
    /// (§5). `None` outside a run.
    pub(crate) fn session_nested_begin(&mut self) -> Option<NestedSaved> {
        let a = self.sess.as_mut()?.arm.as_mut()?;
        a.nested += 1;
        let saved =
            NestedSaved { budget: self.budget, limit: self.mem.limit, step_stop: self.step_stop };
        self.budget = a.tu;
        self.mem.limit = self.mem.arena().saturating_add(a.tu_limit);
        self.step_stop = self.budget.steps;
        Some(saved)
    }

    /// Ends what [`Oven::session_nested_begin`] started.
    pub(crate) fn session_nested_end(&mut self, saved: NestedSaved) {
        let mut pending = NestedCost::ZERO;
        if let Some(a) = self.sess.as_mut().and_then(|s| s.arm.as_mut()) {
            a.nested -= 1;
            if a.nested == 0 {
                pending = std::mem::take(&mut a.pending);
            }
        }
        self.budget = saved.budget;
        self.mem.limit = saved.limit;
        self.step_stop = saved.step_stop;
        self.session_charge(pending);
    }

    /// Charges the running run nested work's cost `c` (spec/eval-session.md
    /// §5.1): its steps count against the run's step budget, its bytes and
    /// depth on top of the run's live ones. A charge that puts the run
    /// over its budget stops it at its next step check. Inside a nested
    /// evaluation the charge waits until the last one ends. Nothing
    /// outside a run.
    pub fn session_charge(&mut self, c: NestedCost) {
        if c.is_zero() {
            return;
        }
        let live = self.mem.live_bytes();
        let arena = self.mem.arena();
        let active = self.active_calls();
        let own = self.run.counters.steps;
        let Some(a) = self.sess.as_mut().and_then(|s| s.arm.as_mut()) else { return };
        if a.nested != 0 {
            a.pending.add(c);
            return;
        }
        a.nested_steps = a.nested_steps.saturating_add(c.steps);
        a.nested_calls = a.nested_calls.saturating_add(c.calls);
        a.peak = a.peak.max(live.saturating_add(c.peak));
        // The run's own steps may use what its nested work left.
        self.budget.steps = a.run.steps.saturating_sub(a.nested_steps);
        let over = if own > self.budget.steps || a.nested_steps > a.run.steps {
            Some(Stop::Limit)
        } else if arena.saturating_add(c.arena) > a.run.bytes {
            Some(Stop::Memory)
        } else if active.saturating_add(c.depth) > a.run.depth {
            Some(Stop::Depth)
        } else {
            None
        };
        if over.is_some() && a.over.is_none() {
            a.over = over;
        }
        if a.over.is_some() {
            self.step_stop = 0;
        } else {
            self.step_stop = self.step_stop.min(self.budget.steps);
        }
    }

    /// The failure of a run that ended charged past its budget (`stop`),
    /// reported at `at`.
    #[cold]
    fn session_over_failure(&mut self, b: &dyn SemaBridge, stop: Stop, at: Loc) -> EvalFailure {
        let f = match stop {
            Stop::Memory => Fault::Memory,
            Stop::Depth => Fault::Depth,
            Stop::Limit | Stop::Cancelled => Fault::Steps(at),
        };
        self.session_note_fault_at_top(b, &f);
        let kind = f.kind();
        let msg = match stop {
            Stop::Memory => format!(
                "constexpr evaluation exceeded the memory limit of {} bytes",
                self.budget.bytes
            ),
            Stop::Depth => format!(
                "constexpr evaluation exceeded maximum depth of {} calls",
                self.budget.depth
            ),
            Stop::Limit | Stop::Cancelled => {
                "constexpr evaluation hit maximum step limit; possible infinite loop?".to_owned()
            }
        };
        EvalFailure { kind, notes: vec![Note::new(at, msg)] }
    }

    /// [`Oven::session_note_fault`] at the run's own level.
    fn session_note_fault_at_top(&mut self, b: &dyn SemaBridge, f: &Fault) {
        let depth = std::mem::replace(&mut self.ctx_depth, 1);
        self.session_note_fault(b, f);
        self.ctx_depth = depth;
    }

    /// The state of function `idx` as its compilation left it (a tracked
    /// function's is kept aside while it stays a stub, §5.1).
    pub(crate) fn func_state(&self, idx: u32) -> FState {
        let s = self.funcs[idx as usize].state;
        if s == FState::Stub
            && let Some(&r) = self.sess.as_ref().and_then(|x| x.tracked.get(&idx))
        {
            return r;
        }
        s
    }

    /// A read of global `g` in a run whose initializer's evaluation an
    /// earlier run recorded: reaches the record (§5.1) and resolves `g`
    /// to its kept object. `None` when `g` has no record.
    #[cold]
    pub(crate) fn session_kept_var(&mut self, b: &mut dyn SemaBridge, g: u32) -> Option<u64> {
        let s = self.sess.as_ref()?;
        s.arm.as_ref()?;
        let p = *s.kept_vars.get(&g)?;
        self.globals[g as usize].state = crate::compile::GState::Resolved(p);
        self.run.impure += 1;
        let c = b.session_reach(SessUnit::Var(g));
        self.session_charge(c);
        Some(p)
    }

    /// The evaluation of global `g`'s initializer in a run starts
    /// (`eval_var_init`): it runs as nested work (§5.1), under the
    /// translation unit's limits, recorded under its own unit. `None`
    /// outside a run.
    pub(crate) fn session_var_begin(&mut self, b: &mut dyn SemaBridge) -> Option<VarSaved> {
        let nested = self.session_nested_begin()?;
        let mark = b.session_window_begin();
        let saved = VarSaved {
            nested,
            mark,
            depth_base: self.run.depth_base,
            maxd: self.run.maxd,
            peak: self.mem.peak,
            live0: self.mem.live_bytes(),
            arena_peak: self.mem.arena_peak,
            arena0: self.mem.arena(),
        };
        self.run.depth_base = 0;
        self.run.maxd = 0;
        self.mem.peak = saved.live0;
        self.mem.arena_peak = saved.arena0;
        Some(saved)
    }

    /// Ends what [`Oven::session_var_begin`] started, the initializer's
    /// evaluation having made object `p`, counted `counters` and peaked at
    /// `peak` live bytes.
    pub(crate) fn session_var_end(
        &mut self,
        b: &mut dyn SemaBridge,
        g: u32,
        p: u64,
        (counters, peak): (crate::vm::Counters, u64),
        saved: VarSaved,
    ) {
        let cost = NestedCost {
            steps: counters.steps,
            calls: counters.calls,
            peak: peak.saturating_sub(saved.live0),
            arena: self.mem.arena_peak.saturating_sub(saved.arena0),
            depth: self.run.maxd,
        };
        self.run.depth_base = saved.depth_base;
        self.run.maxd = saved.maxd;
        self.mem.peak = saved.peak;
        self.mem.arena_peak = saved.arena_peak;
        // Its cost is its unit's own.
        let loose = b.session_nested_cost(cost).unwrap_or(NestedCost::ZERO);
        let (recorded, charge) = b.session_window_end(SessUnit::Var(g), saved.mark);
        if recorded {
            if let Some(s) = self.sess.as_mut() {
                s.kept_vars.insert(g, p);
            }
            self.run.impure += 1;
        }
        self.session_nested_end(saved.nested);
        self.session_charge(loose);
        self.session_charge(charge);
    }

    /// [`Oven::compile_func`] in a session: during a run, a function
    /// whose compilation is recorded (it reported diagnostics or did nested
    /// work, §5.1) stays a stub, is not inlined, and every later call of it
    /// in any run reaches the record (its diagnostics replayed, its cost
    /// charged once per run) and makes the calls in progress impure, so no
    /// memoized result skips the reach.
    #[cold]
    #[inline(never)]
    pub(crate) fn session_compile(&mut self, b: &mut dyn SemaBridge, idx: u32) {
        let Some(s) = self.sess.as_ref() else { return };
        let real = s.tracked.get(&idx).copied();
        if s.arm.is_none() {
            if real.is_none() {
                self.compile_func_body(b, idx);
            }
            return;
        }
        // A declared function is retried at every call; its compilation is
        // recorded again.
        if let Some(r) = real
            && r != FState::Undefined
        {
            self.run.impure += 1;
            let first = self.sess.as_mut().is_some_and(|s| {
                s.reached.insert(idx) || s.arm.as_ref().is_some_and(|a| a.nested != 0)
            });
            if first {
                let c = b.session_reach(SessUnit::Func(idx));
                self.session_charge(c);
            }
            return;
        }
        let w = b.session_window_begin();
        self.compile_func_body(b, idx);
        let (recorded, charge) = b.session_window_end(SessUnit::Func(idx), w);
        if recorded || real.is_some() {
            let state = self.funcs[idx as usize].state;
            self.funcs[idx as usize].inline = false;
            self.funcs[idx as usize].state = FState::Stub;
            if let Some(s) = self.sess.as_mut() {
                s.tracked.insert(idx, state);
                s.reached.insert(idx);
            }
            self.run.impure += 1;
        }
        self.session_charge(charge);
    }

    /// Sets every global whose initializer's evaluation was recorded back
    /// to unresolved.
    fn session_reset_vars(&mut self) {
        let Some(s) = self.sess.as_ref() else { return };
        for &g in s.kept_vars.keys() {
            self.globals[g as usize].state = crate::compile::GState::Unresolved;
        }
    }

    /// Releases what a run's large structures left in the buffers.
    fn release_spaces(&mut self) {
        self.mem.release_buffers(RELEASE_FLOOR);
        let floor = RELEASE_FLOOR / size_of::<u128>();
        if self.regs.len() > floor {
            self.regs.truncate(floor);
            self.regs.shrink_to_fit();
        }
        trim(&mut self.stack, RELEASE_FLOOR);
        trim(&mut self.cleanups, RELEASE_FLOOR);
        trim(&mut self.cdtors, RELEASE_FLOOR);
    }
}

/// The step check's slow path ([`CANCEL_INTERVAL`]): `need` steps passed
/// `step_stop` (`limit` the step budget). `Err`: why the run stops (a
/// run's charge past its budget first, §5.1); `Ok`: `step_stop` moved on.
#[cold]
#[inline(never)]
pub(crate) fn step_over(
    step_stop: &mut u64,
    limit: u64,
    sess: Option<&SessionState>,
    need: u64,
) -> Result<(), Stop> {
    let arm = sess.and_then(|s| s.arm.as_ref()).filter(|a| a.nested == 0);
    if let Some(stop) = arm.and_then(|a| a.over) {
        return Err(stop);
    }
    if need > limit {
        return Err(Stop::Limit);
    }
    match arm {
        Some(a) => {
            if a.cancel.is_cancelled() {
                return Err(Stop::Cancelled);
            }
            *step_stop = limit.min(need.saturating_add(CANCEL_INTERVAL));
        }
        None => *step_stop = limit,
    }
    Ok(())
}

/// Shrinks empty `v`'s buffer to at most `bytes` bytes.
pub(crate) fn trim<T>(v: &mut Vec<T>, bytes: usize) {
    let cap = bytes / size_of::<T>().max(1);
    if v.capacity() > cap {
        v.shrink_to(cap.max(v.len()));
    }
}
