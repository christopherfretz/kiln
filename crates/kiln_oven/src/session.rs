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

use kiln_ast::DeclId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::compile::{FState, RetKind};
use crate::host::HostMode;
use crate::ty::TyKind;
use crate::vm::Fault;
use crate::{Budget, EvalFailure, FailKind, Oven, SemaBridge};

/// A run checks its [`CancelToken`] at least every this many steps
/// (spec/eval-session.md §8).
pub const CANCEL_INTERVAL: u64 = 65_536;

/// After a run, the frame and heap spaces' buffers are released down to
/// this many bytes each (spec/eval-session.md §10).
pub const RELEASE_FLOOR: usize = 1 << 20;

/// The limits of one run (spec/eval-session.md §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunBudget {
    /// Steps the run may execute (spec/oven.md "Limits").
    pub steps: u64,
    /// Bytes of abstract memory (frame and heap) the run may hold.
    pub bytes: u64,
    /// Nested calls the run may make.
    pub depth: u32,
}

impl RunBudget {
    /// The program profile: 2^34 steps, 1 GiB, 16384 nested calls.
    pub const PROGRAM: RunBudget = RunBudget { steps: 1 << 34, bytes: 1 << 30, depth: 16_384 };

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

/// The value of a successful run: the entry's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunValue {
    /// A `void` entry.
    Void,
    /// A `bool` result.
    Bool(bool),
    /// A result of a signed integral type.
    Signed(i128),
    /// A result of an unsigned integral type (the character types by
    /// their signedness).
    Unsigned(u128),
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
    },
}

/// What [`Oven::session_run`] produced.
#[derive(Debug)]
pub struct RunOutcome {
    /// The value, or the kind of failure with its notes.
    pub result: Result<RunValue, (RunError, EvalFailure)>,
    /// Steps the run executed.
    pub steps: u64,
    /// Handler calls the run made.
    pub host_calls: u64,
    /// The run's peak live frame and heap bytes.
    pub peak_heap: u64,
    /// Function calls the run made.
    pub calls: u64,
}

/// The session state an Oven keeps while a session is open.
#[derive(Debug, Default)]
pub(crate) struct SessionState {
    /// The compiled call stub of each entry run so far.
    stubs: FxHashMap<DeclId, u32>,
    /// The functions whose compilation in a run reported diagnostics,
    /// with their real state: set back to a stub at the end of every
    /// run, so the next run's first call replays them (§7).
    noisy: FxHashMap<u32, FState>,
    /// The running run's state.
    pub(crate) arm: Option<Arm>,
}

/// A run in progress.
#[derive(Debug)]
pub(crate) struct Arm {
    cancel: CancelToken,
    /// The translation unit's budget and memory limit (nested
    /// evaluations run under them, §5).
    tu: Budget,
    tu_limit: u64,
    /// Nested evaluations open: no cancellation and no fault
    /// classification inside them.
    nested: u32,
    /// The kind of the run's last fault.
    fault: Option<RunError>,
}

/// What [`Oven::session_nested_begin`] saved.
pub(crate) struct NestedSaved {
    budget: Budget,
    limit: u64,
    step_stop: u64,
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
            for (idx, state) in s.noisy {
                self.funcs[idx as usize].state = state;
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

    /// Runs entry `f` (a `consteval` function without parameters
    /// returning `void` or an integral type; the caller checked) under
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
                let Some(s) = self.compile_call(b, f, at) else { return fail(RunError::Failed) };
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
            s.arm = Some(Arm { cancel: cancel.clone(), tu, tu_limit, nested: 0, fault: None });
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
            let v = match self.funcs[stub as usize].ret {
                RetKind::Scalar(ty) => {
                    let raw = self.regs[0];
                    match self.types.kind(ty) {
                        TyKind::Bool => RunValue::Bool(raw & 1 != 0),
                        TyKind::Int { width, signed: true } => RunValue::Signed(
                            crate::arith::canon(raw, u32::from(width), true) as i128,
                        ),
                        TyKind::Int { width, signed: false } => {
                            RunValue::Unsigned(crate::arith::canon(raw, u32::from(width), false))
                        }
                        _ => return Err(EvalFailure::silent(FailKind::NotConstant)),
                    }
                }
                _ => RunValue::Void,
            };
            match self.leak_notes(b) {
                None => Ok(v),
                Some(notes) => Err(EvalFailure { kind: FailKind::NotConstant, notes }),
            }
        });
        let c = self.end_outermost();
        self.host_leave(mode);
        self.inner = inner;
        self.budget = tu;
        self.mem.limit = tu_limit;
        self.step_stop = tu.steps;
        let arm = self.sess.as_mut().and_then(|s| s.arm.take());
        self.session_reset_noisy();
        self.release_spaces();
        let result = r.map_err(|e| {
            let k = arm.and_then(|a| a.fault).unwrap_or(RunError::Failed);
            (k, e)
        });
        RunOutcome {
            result,
            steps: c.steps,
            host_calls: self.host.calls - calls0,
            peak_heap: c.peak,
            calls: c.calls,
        }
    }

    /// [`step_over`] of this Oven.
    pub(crate) fn step_over(&mut self, need: u64) -> Result<(), bool> {
        step_over(&mut self.step_stop, self.budget.steps, self.sess.as_deref(), need)
    }

    /// Whether a host call of the running run is cancelled (checked
    /// before each one).
    pub(crate) fn host_cancelled(&self) -> bool {
        self.sess
            .as_ref()
            .and_then(|s| s.arm.as_ref())
            .is_some_and(|a| a.nested == 0 && a.cancel.is_cancelled())
    }

    /// Records the kind of fault `fault` of the run's outermost context
    /// (from `exec`, before it is rendered).
    #[cold]
    pub(crate) fn session_note_fault(&mut self, b: &dyn SemaBridge, fault: &Fault) {
        let Some(a) = self.sess.as_ref().and_then(|s| s.arm.as_ref()) else { return };
        if a.nested != 0 || self.ctx_depth != 1 {
            return;
        }
        let k = match fault {
            Fault::Cancelled => RunError::Cancelled,
            // The handler's own message, or what the call's check found.
            Fault::Host(m) if !m.is_empty() => RunError::Host {
                message: m.strip_prefix("host call failed: ").unwrap_or(m).to_owned(),
            },
            Fault::Uncaught { ty, what, .. } => RunError::Uncaught {
                type_name: self.tname(b.ast(), *ty),
                what: what.as_deref().map(str::to_owned),
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
        if let Some(a) = self.sess.as_mut().and_then(|s| s.arm.as_mut()) {
            a.nested -= 1;
        }
        self.budget = saved.budget;
        self.mem.limit = saved.limit;
        self.step_stop = saved.step_stop;
    }

    /// [`Oven::compile_func`] in a session: during a run, a function
    /// whose compilation reports diagnostics is remembered (and not
    /// inlined), and one set back to a stub replays them through the
    /// bridge and gets its real state back without compiling again (§7).
    /// Either makes the calls in progress impure, so no memoized result
    /// skips the replay in a later run.
    #[cold]
    #[inline(never)]
    pub(crate) fn session_compile(&mut self, b: &mut dyn SemaBridge, idx: u32) {
        let armed = self.sess.as_ref().is_some_and(|s| s.arm.is_some());
        if !armed {
            self.compile_func_body(b, idx);
            return;
        }
        let real = self.sess.as_ref().and_then(|s| s.noisy.get(&idx).copied());
        if let Some(state) = real
            && self.funcs[idx as usize].state == FState::Stub
        {
            b.session_replay(idx);
            self.funcs[idx as usize].state = state;
            self.run.impure += 1;
            return;
        }
        let w = b.session_window_begin();
        self.compile_func_body(b, idx);
        if b.session_window_end(idx, w) {
            let state = self.funcs[idx as usize].state;
            self.funcs[idx as usize].inline = false;
            if let Some(s) = self.sess.as_mut() {
                s.noisy.insert(idx, state);
            }
            self.run.impure += 1;
        }
    }

    /// Sets every function with recorded diagnostics back to a stub.
    fn session_reset_noisy(&mut self) {
        let Some(s) = self.sess.as_mut() else { return };
        for (&idx, real) in &mut s.noisy {
            let f = &mut self.funcs[idx as usize];
            if f.state != FState::Stub {
                *real = f.state;
            }
            f.state = FState::Stub;
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
/// `step_stop` (`limit` the step budget). `Err(true)`: the run is
/// cancelled; `Err(false)`: the budget is exceeded; `Ok`: `step_stop`
/// moved on.
#[cold]
#[inline(never)]
pub(crate) fn step_over(
    step_stop: &mut u64,
    limit: u64,
    sess: Option<&SessionState>,
    need: u64,
) -> Result<(), bool> {
    if need > limit {
        return Err(false);
    }
    match sess.and_then(|s| s.arm.as_ref()) {
        Some(a) if a.nested == 0 => {
            if a.cancel.is_cancelled() {
                return Err(true);
            }
            *step_stop = limit.min(need.saturating_add(CANCEL_INTERVAL));
        }
        _ => *step_stop = limit,
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
