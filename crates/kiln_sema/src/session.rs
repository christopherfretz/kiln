//! Evaluation sessions (spec/eval-session.md): a finished translation
//! unit kept to evaluate `constexpr` and `consteval` entry functions of
//! it many times, each run isolated from every other, for tooling
//! clients.
//!
//! The session owns the finished [`Sema`] (§1). A run
//! ([`EvalSession::run`]) is a manifestly constant-evaluated call of the
//! entry at the end of the translation unit (§4: an immediate invocation
//! of an immediate entry, the initializer of a notional `constexpr`
//! variable for a `constexpr` one), evaluated by the Oven
//! ([`Oven::session_run`]) through a bridge that carries the run's
//! host-call handler and the diagnostic replay of §7.

use std::fmt;

use kiln_ast::decl::{ConstexprKind, DeclKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType};
use kiln_diag::{DiagEngine, DiagId, Diagnostic};
pub use kiln_oven::{CANCEL_INTERVAL, CancelToken, RELEASE_FLOOR, RunBudget, RunError, RunValue};
use kiln_oven::{HostCalls, MetaOp, NestedCost, Oven, SemaBridge, SessUnit};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::oven_bridge::Bridge;
use crate::templ::stack::{FrameKind, InstFrame};

/// A finished translation unit kept to run entries (spec/eval-session.md
/// §2). `Send`, not `Sync` (§9).
pub struct EvalSession<'a, 'sm> {
    sema: Sema<'a, 'sm>,
    /// The records of §5.1 and §7 (in `sema` while a run is running).
    replay: Box<Replay>,
    tu_host: Option<Box<dyn HostCalls>>,
}

impl fmt::Debug for EvalSession<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvalSession")
            .field("replay", &self.replay.records.len())
            .field("tu_host", &self.tu_host.is_some())
            .finish_non_exhaustive()
    }
}

/// A session is `Send` (spec/eval-session.md §9).
const _: fn() = || {
    fn send<T: Send>() {}
    send::<EvalSession<'static, 'static>>();
    send::<RunReport>();
};

/// Why [`EvalSession::open`] refused (spec/eval-session.md §2).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenError {
    /// The translation unit reported errors.
    TuHasErrors {
        /// How many.
        errors: u32,
    },
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::TuHasErrors { errors } => write!(
                f,
                "cannot open an evaluation session: the translation unit has {errors} error(s)"
            ),
        }
    }
}

impl std::error::Error for OpenError {}

/// A function a session may run (spec/eval-session.md §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entry {
    decl: DeclId,
}

impl Entry {
    /// The entry's declaration.
    #[must_use]
    pub fn decl(self) -> DeclId {
        self.decl
    }
}

/// Why a function is not an entry (spec/eval-session.md §3).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntryError {
    /// No declaration has the name.
    NotFound {
        /// The name looked up.
        name: String,
    },
    /// The name is an overload set.
    Overloaded {
        /// The name looked up.
        name: String,
        /// How many functions it names.
        count: usize,
    },
    /// The declaration is not a function (a function template, a
    /// variable, a type, ...).
    NotAFunction,
    /// The function is neither `constexpr` nor `consteval` (nor an
    /// escalated immediate function).
    NotConstexpr,
    /// The function has parameters (an implicit object parameter
    /// included) or is variadic.
    HasParameters,
    /// The function returns neither `void` nor an integral type.
    WrongReturnType,
    /// The function is not defined.
    Undefined,
}

impl fmt::Display for EntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryError::NotFound { name } => write!(f, "no declaration named '{name}'"),
            EntryError::Overloaded { name, count } => {
                write!(f, "'{name}' names {count} overloaded functions")
            }
            EntryError::NotAFunction => f.write_str("the entry is not a function"),
            EntryError::NotConstexpr => {
                f.write_str("the entry is neither a constexpr nor a consteval function")
            }
            EntryError::HasParameters => f.write_str("the entry has parameters"),
            EntryError::WrongReturnType => {
                f.write_str("the entry returns neither void nor an integral type")
            }
            EntryError::Undefined => f.write_str("the entry is not defined"),
        }
    }
}

impl std::error::Error for EntryError {}

/// What a run produced (spec/eval-session.md §4).
#[derive(Clone, Debug)]
pub struct RunReport {
    /// The entry's value, or how the run failed.
    pub result: Result<RunValue, RunError>,
    /// Steps the run executed itself (§4).
    pub steps: u64,
    /// Steps of the nested work the run was charged (§5.1): with
    /// [`RunReport::steps`], what the step budget was checked against.
    pub nested_steps: u64,
    /// Handler calls the run made.
    pub host_calls: u64,
    /// The run's peak live frame and heap bytes, its charged nested
    /// work's included.
    pub peak_heap: u64,
    /// Function calls the run made itself.
    pub calls: u64,
    /// Function calls of the nested work the run was charged.
    pub nested_calls: u64,
    /// The run's diagnostics, in order (§7).
    pub diagnostics: Vec<Diagnostic>,
}

/// What a session holds (spec/eval-session.md §10): heap bytes reserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptMemory {
    /// The AST (`Ast::memory`, reserved bytes).
    pub ast: u64,
    /// Sema's own tables (`Sema::memory`).
    pub sema: u64,
    /// The Oven (`Oven::footprint`).
    pub oven: u64,
    /// The recorded diagnostics of §7.
    pub replay: u64,
    /// Results in the memoization cache.
    pub memo_entries: usize,
}

impl KeptMemory {
    /// Every byte counted.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.ast + self.sema + self.oven + self.replay
    }
}

/// A unit of nested work a run records (spec/eval-session.md §5.1, §7).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    /// The compilation of the Oven's function (by index).
    Func(u32),
    /// The evaluation of the initializer of the Oven's global (by index).
    Var(u32),
    /// The instantiation of a variable's initializer (a static data
    /// member's, a variable template specialization's).
    Init(DeclId),
    /// The instantiation of a member function's definition.
    Inst(DeclId),
    /// A reflection intrinsic with its operands.
    Meta(MetaOp, Box<[u128]>),
    /// A `noexcept` query.
    Nothrow(DeclId),
}

impl From<SessUnit> for Key {
    fn from(u: SessUnit) -> Self {
        match u {
            SessUnit::Func(f) => Key::Func(f),
            SessUnit::Var(g) => Key::Var(g),
        }
    }
}

/// One recorded piece: diagnostics, or what a nested key recorded, with
/// the context notes it had there.
#[derive(Clone, Debug)]
enum Seg {
    Diags(Vec<Diagnostic>),
    Child(Key, Vec<Diagnostic>),
}

/// The instantiation frame a unit opens (its context note names where it
/// was required, which is where a later reach requires it).
#[derive(Clone, Copy, Debug)]
pub(crate) struct UnitFrame {
    kind: FrameKind,
    entity: DeclId,
    template: DeclId,
    sfinae: Option<bool>,
}

/// What a unit recorded: its diagnostics (with the units nested in it
/// that reported, in order), the context notes they had, its own nested
/// evaluations' cost and the units it reached (charged with it).
#[derive(Clone, Debug, Default)]
struct Rec {
    segs: Vec<Seg>,
    ctx: Vec<Diagnostic>,
    frame: Option<UnitFrame>,
    own: NestedCost,
    kids: Vec<Key>,
}

/// A recording in progress: from capture index `from`, in context `ctx`,
/// with the nested keys recorded or replayed inside it (key, their
/// context, capture range), the units it reached and its own cost so far.
#[derive(Debug)]
struct Window {
    from: usize,
    ctx: Vec<Diagnostic>,
    frame: Option<UnitFrame>,
    children: Vec<(Key, Vec<Diagnostic>, usize, usize)>,
    kids: Vec<Key>,
    own: NestedCost,
}

impl Window {
    /// Whether nothing was recorded or reached in it so far.
    fn is_empty(&self) -> bool {
        self.children.is_empty() && self.kids.is_empty() && self.own.is_zero()
    }
}

/// What semantic work did in runs, for later runs: its diagnostics,
/// replayed, and its cost, charged (spec/eval-session.md §5.1, §7).
#[derive(Debug, Default)]
pub(crate) struct Replay {
    records: FxHashMap<Key, Rec>,
    /// The keys whose diagnostics were reported in the running run.
    done: FxHashSet<Key>,
    /// The keys whose cost was charged to the running run.
    charged: FxHashSet<Key>,
    windows: Vec<Window>,
    /// Charges made with no unit open and no Oven at hand, for the Oven to
    /// take (`SemaBridge::session_settle`).
    loose: NestedCost,
    /// Diagnostics recorded (for [`KeptMemory::replay`]).
    count: usize,
    /// The reflection intrinsics with a record (a call of another one
    /// that reports nothing computes no key).
    meta_ops: FxHashSet<MetaOp>,
    /// Content words in the records' keys (for [`KeptMemory::replay`]).
    key_words: usize,
}

/// A run's state on the bridge.
pub(crate) struct RunCtx<'r> {
    pub(crate) host: &'r mut (dyn HostCalls + 'r),
}

/// `notes` with the last run of `from` in them replaced by `to` (nothing
/// changes when `from` is empty or not there).
fn recontext(notes: &[Diagnostic], from: &[Diagnostic], to: &[Diagnostic]) -> Vec<Diagnostic> {
    let same = |a: &Diagnostic, b: &Diagnostic| a.message == b.message && a.span == b.span;
    if from.is_empty() || notes.len() < from.len() {
        return notes.to_vec();
    }
    let Some(at) = (0..=notes.len() - from.len())
        .rev()
        .find(|&i| notes[i..i + from.len()].iter().zip(from).all(|(a, b)| same(a, b)))
    else {
        return notes.to_vec();
    };
    let mut out = notes[..at].to_vec();
    out.extend_from_slice(to);
    out.extend_from_slice(&notes[at + from.len()..]);
    out
}

impl Replay {
    /// Reports the diagnostics of `k` in context `target` (its recorded
    /// one when `None`), if it has any and they were not reported in this
    /// run yet; an open window records it as a child either way.
    fn replay(&mut self, diags: &DiagEngine<'_>, k: &Key, target: Option<Vec<Diagnostic>>) {
        let Some(r) = self.records.get(k) else { return };
        if r.segs.is_empty() {
            return;
        }
        let target = target.unwrap_or_else(|| r.ctx.clone());
        let start = diags.captured_len();
        if self.done.insert(k.clone()) {
            let segs = r.segs.clone();
            let from = (r.ctx != target).then(|| r.ctx.clone());
            for s in &segs {
                match (s, &from) {
                    (Seg::Diags(v), None) => diags.capture_replay(v),
                    (Seg::Diags(v), Some(from)) => {
                        let v: Vec<Diagnostic> = v
                            .iter()
                            .map(|d| Diagnostic {
                                notes: recontext(&d.notes, from, &target),
                                ..d.clone()
                            })
                            .collect();
                        diags.capture_replay(&v);
                    }
                    (Seg::Child(c, edge), None) => self.replay(diags, c, Some(edge.clone())),
                    (Seg::Child(c, edge), Some(from)) => {
                        self.replay(diags, c, Some(recontext(edge, from, &target)));
                    }
                }
            }
        }
        let end = diags.captured_len();
        if let Some(w) = self.windows.last_mut() {
            w.children.push((k.clone(), target, start, end));
        }
    }

    /// The run reaches recorded unit `k` in context `target`: its
    /// diagnostics are replayed; inside an open window it is that unit's
    /// (charged with it), else its cost is what the run is charged now.
    fn reach(
        &mut self,
        diags: &DiagEngine<'_>,
        k: &Key,
        target: Option<Vec<Diagnostic>>,
    ) -> NestedCost {
        if !self.records.contains_key(k) {
            return NestedCost::ZERO;
        }
        self.replay(diags, k, target);
        self.place(k.clone())
    }

    /// The cost of `k` and every unit it reached not charged in this run
    /// yet (each once).
    fn charge(&mut self, k: &Key) -> NestedCost {
        let mut total = NestedCost::ZERO;
        let mut todo = vec![k.clone()];
        while let Some(k) = todo.pop() {
            if !self.charged.insert(k.clone()) {
                continue;
            }
            if let Some(r) = self.records.get(&k) {
                total.add(r.own);
                todo.extend(r.kids.iter().rev().cloned());
            }
        }
        total
    }

    /// Opens a window at capture index `from` in context `ctx`.
    fn begin(&mut self, from: usize, ctx: Vec<Diagnostic>, frame: Option<UnitFrame>) {
        self.windows.push(Window {
            from,
            ctx,
            frame,
            children: Vec::new(),
            kids: Vec::new(),
            own: NestedCost::ZERO,
        });
    }

    /// Closes the innermost window (begun at `mark`) as unit `key`:
    /// whether `key` is recorded, and what the run is to be charged now.
    /// A unit recorded in an earlier run (a query asked again) keeps its
    /// record's cost; new diagnostics replace its old ones, else those are
    /// replayed.
    fn end(&mut self, diags: &DiagEngine<'_>, key: Key, mark: usize) -> (bool, NestedCost) {
        let Some(w) = self.windows.pop() else { return (false, NestedCost::ZERO) };
        debug_assert_eq!(w.from, mark, "session windows nest");
        let end = diags.captured_len();
        let mut segs = Vec::new();
        if end > w.from || !w.children.is_empty() {
            let all = diags.captured_from(w.from);
            let mut at = w.from;
            for (c, ctx, s, e) in w.children {
                if s > at {
                    segs.push(Seg::Diags(all[at - w.from..s - w.from].to_vec()));
                }
                segs.push(Seg::Child(c, ctx));
                at = e.max(at);
            }
            if end > at {
                segs.push(Seg::Diags(all[at - w.from..].to_vec()));
            }
        }
        let fresh_diags = end > w.from;
        if let Some(r) = self.records.get_mut(&key) {
            if fresh_diags {
                r.segs = segs;
                r.ctx.clone_from(&w.ctx);
                self.done.insert(key.clone());
                if let Some(p) = self.windows.last_mut() {
                    p.children.push((key.clone(), w.ctx, w.from, end));
                }
            } else {
                self.replay(diags, &key, Some(w.ctx));
            }
            return (true, self.place(key));
        }
        if segs.is_empty() && w.own.is_zero() && w.kids.is_empty() {
            return (false, NestedCost::ZERO);
        }
        self.count +=
            segs.iter().map(|s| if let Seg::Diags(v) = s { v.len() } else { 0 }).sum::<usize>();
        if let Key::Meta(op, words) = &key {
            self.meta_ops.insert(*op);
            self.key_words += words.len();
        }
        if let Some(p) = self.windows.last_mut() {
            p.children.push((key.clone(), w.ctx.clone(), w.from, end));
        }
        self.records.insert(
            key.clone(),
            Rec { segs, ctx: w.ctx, frame: w.frame, own: w.own, kids: w.kids },
        );
        self.done.insert(key.clone());
        (true, self.place(key))
    }

    /// Recorded unit `key` was reached: the open window's, or charged now.
    fn place(&mut self, key: Key) -> NestedCost {
        if let Some(p) = self.windows.last_mut() {
            if !p.kids.contains(&key) {
                p.kids.push(key);
            }
            return NestedCost::ZERO;
        }
        self.charge(&key)
    }

    /// A nested evaluation cost `c`: the open window's own, or (`Some`)
    /// the run's now.
    fn nested(&mut self, c: NestedCost) -> Option<NestedCost> {
        match self.windows.last_mut() {
            Some(w) => {
                w.own.add(c);
                None
            }
            None => Some(c),
        }
    }
}

impl Sema<'_, '_> {
    /// The context notes a diagnostic emitted now would have.
    fn sess_context(&self) -> Vec<Diagnostic> {
        if self.inst.stack.is_empty() {
            return Vec::new();
        }
        // Computing the notes marks them printed (`last_emitted`); the
        // real diagnostics that follow must still print theirs.
        let printed = self.inst.stack.last_emitted.get();
        self.inst.stack.last_emitted.set(0);
        let notes = self.inst_context_notes();
        self.inst.stack.last_emitted.set(printed);
        self.diags.note_records(notes)
    }

    /// Opens a recording window in a session's run (§7) for a unit whose
    /// instantiation frame is the innermost one; its mark.
    pub(crate) fn sess_unit_begin(&mut self) -> Option<usize> {
        self.eval_replay.as_ref()?;
        let ctx = self.sess_context();
        let frame = self.inst.stack.frames.last().map(|f| UnitFrame {
            kind: f.kind,
            entity: f.entity,
            template: f.template,
            sfinae: f.sfinae,
        });
        let from = self.diags.captured_len();
        self.eval_replay.as_deref_mut()?.begin(from, ctx, frame);
        Some(from)
    }

    /// Closes the window `mark` opened as unit `key`, charging the run
    /// what it is to be charged now.
    pub(crate) fn sess_window_end(&mut self, key: Key, mark: Option<usize>) {
        let Some(mark) = mark else { return };
        let diags = self.diags;
        let Some(rp) = self.eval_replay.as_deref_mut() else { return };
        let (recorded, c) = rp.end(diags, key, mark);
        if recorded {
            self.sess_impure();
        }
        self.sess_charge(c);
    }

    /// The run reaches unit `key` (recorded in an earlier run, or not),
    /// required at `poi`: its diagnostics are reported with the context
    /// notes they would have now.
    pub(crate) fn sess_reach_at(&mut self, key: &Key, poi: Loc) {
        let Some(frame) =
            self.eval_replay.as_deref().and_then(|rp| rp.records.get(key)).map(|r| r.frame)
        else {
            return;
        };
        let target = match frame {
            Some(f) => {
                self.inst.stack.push(InstFrame {
                    kind: f.kind,
                    entity: f.entity,
                    template: f.template,
                    poi,
                    sfinae: f.sfinae,
                    args: None,
                    start: None,
                    start_bytes: 0,
                    start_nodes: 0,
                    child_ns: 0,
                    trace: None,
                });
                let ctx = self.sess_context();
                let _ = self.inst.stack.pop();
                Some(ctx)
            }
            None => None,
        };
        let diags = self.diags;
        let Some(rp) = self.eval_replay.as_deref_mut() else { return };
        let c = rp.reach(diags, key, target);
        self.sess_impure();
        self.sess_charge(c);
    }

    /// Whether unit `key` has a record (in a session's run).
    pub(crate) fn sess_recorded(&self, key: &Key) -> bool {
        self.eval_replay.as_deref().is_some_and(|rp| rp.records.contains_key(key))
    }

    /// The calls in progress reached recorded work: not memoized.
    fn sess_impure(&mut self) {
        if let Some(o) = self.oven.as_deref_mut() {
            o.mark_impure();
        }
    }

    /// Charges the run `c`: to the Oven when it is lent here, else kept
    /// for the Oven to take.
    fn sess_charge(&mut self, c: NestedCost) {
        if c.is_zero() {
            return;
        }
        if let Some(o) = self.oven.as_deref_mut() {
            o.session_charge(c);
        } else if let Some(rp) = self.eval_replay.as_deref_mut() {
            rp.loose.add(c);
        }
    }
}

impl Bridge<'_, '_, '_> {
    /// Opens a recording window (§7); its mark.
    pub(crate) fn window_begin(&mut self) -> usize {
        let from = self.sema.diags.captured_len();
        if self.sema.eval_replay.is_some() {
            let ctx = self.sema.sess_context();
            if let Some(rp) = self.sema.eval_replay.as_deref_mut() {
                rp.begin(from, ctx, None);
            }
        }
        from
    }

    /// Closes the window `mark` opened as unit `key`: whether it is
    /// recorded, and what the run is to be charged now.
    pub(crate) fn window_end(&mut self, key: Key, mark: usize) -> (bool, NestedCost) {
        let diags = self.sema.diags;
        match self.sema.eval_replay.as_deref_mut() {
            Some(rp) => rp.end(diags, key, mark),
            None => (false, NestedCost::ZERO),
        }
    }

    /// The run reaches recorded unit `key` (§5.1): what it is to be
    /// charged now.
    pub(crate) fn reach(&mut self, key: &Key) -> NestedCost {
        let diags = self.sema.diags;
        // Reported in the context of the reach when they were recorded in
        // one (not at a run's own level).
        let target = self
            .sema
            .eval_replay
            .as_deref()
            .and_then(|rp| rp.records.get(key))
            .filter(|r| !r.ctx.is_empty())
            .map(|_| self.sema.sess_context());
        match self.sema.eval_replay.as_deref_mut() {
            Some(rp) => rp.reach(diags, key, target),
            None => NestedCost::ZERO,
        }
    }

    /// A nested evaluation cost `c` (§5.1): the open unit's, or the run's
    /// now.
    pub(crate) fn nested_cost(&mut self, c: NestedCost) -> Option<NestedCost> {
        match self.sema.eval_replay.as_deref_mut() {
            Some(rp) => rp.nested(c),
            None => Some(c),
        }
    }

    /// The charges kept for the Oven.
    pub(crate) fn settle(&mut self) -> NestedCost {
        self.sema
            .eval_replay
            .as_deref_mut()
            .map_or(NestedCost::ZERO, |rp| std::mem::take(&mut rp.loose))
    }

    /// `f` (semantic work a bridge callback does) recorded under `key`
    /// in a run (§5.1): what it reports and costs is recorded; when it
    /// was recorded in an earlier run, that is replayed and charged.
    /// Either makes the calls in progress impure, so no memoized result
    /// skips it in a later run.
    pub(crate) fn tracked<R>(
        &mut self,
        key: Key,
        oven: &mut Oven,
        f: impl FnOnce(&mut Self, &mut Oven) -> R,
    ) -> R {
        self.tracked_by(None, |_, _| key, oven, f)
    }

    /// [`Bridge::tracked`] with the key computed after `f`, by `key`, and
    /// only when it is needed: always, unless `op` is a reflection
    /// intrinsic that has no record and whose call left nothing to
    /// record (no diagnostics, no nested work, no unit reached): no key
    /// of the op can be known then.
    pub(crate) fn tracked_by<R>(
        &mut self,
        op: Option<MetaOp>,
        key: impl FnOnce(&mut Self, &mut Oven) -> Key,
        oven: &mut Oven,
        f: impl FnOnce(&mut Self, &mut Oven) -> R,
    ) -> R {
        if self.sema.eval_replay.is_none() {
            return f(self, oven);
        }
        let mark = self.window_begin();
        let r = f(self, oven);
        let quiet = self.sema.diags.captured_len() <= mark;
        if let (Some(op), true) = (op, quiet)
            && let Some(rp) = self.sema.eval_replay.as_deref_mut()
            && !rp.meta_ops.contains(&op)
            && rp.windows.last().is_some_and(Window::is_empty)
        {
            let w = rp.windows.pop();
            debug_assert!(w.is_some_and(|w| w.from == mark), "session windows nest");
            return r;
        }
        let key = key(self, oven);
        // A known record is replayed and charged by `window_end` when the
        // window recorded nothing new (`Replay::end`).
        let (recorded, c) = self.window_end(key, mark);
        if recorded {
            oven.mark_impure();
        }
        oven.session_charge(c);
        r
    }
}

impl<'a, 'sm> EvalSession<'a, 'sm> {
    /// Opens a session on the finished translation unit `sema`
    /// (`kiln_parse::parse_preprocessed_retained`).
    ///
    /// # Errors
    /// [`OpenError::TuHasErrors`] when the translation unit reported an
    /// error.
    pub fn open(mut sema: Sema<'a, 'sm>) -> Result<Self, OpenError> {
        let errors = sema.diags.error_count();
        if errors != 0 || sema.diags.has_fatal() {
            return Err(OpenError::TuHasErrors { errors: errors.max(1) });
        }
        let oven = oven_mut(&mut sema);
        let tu_host = oven.take_host_calls();
        oven.session_begin();
        Ok(EvalSession { sema, replay: Box::default(), tu_host })
    }

    /// The translation unit's state (read only).
    #[must_use]
    pub fn sema(&self) -> &Sema<'a, 'sm> {
        &self.sema
    }

    /// The handler the translation unit was parsed with, taken out of the
    /// Oven at [`EvalSession::open`] (runs never use it).
    pub fn take_tu_host_calls(&mut self) -> Option<Box<dyn HostCalls>> {
        self.tu_host.take()
    }

    /// Closes the session, returning the translation unit's state.
    #[must_use]
    pub fn into_sema(mut self) -> Sema<'a, 'sm> {
        let tu_host = self.tu_host.take();
        let oven = oven_mut(&mut self.sema);
        oven.session_end();
        if tu_host.is_some() {
            let _ = oven.set_host_calls(tu_host);
        }
        self.sema
    }

    /// Function `f` as an entry (spec/eval-session.md §3).
    ///
    /// # Errors
    /// Why it is not one.
    pub fn entry(&self, f: DeclId) -> Result<Entry, EntryError> {
        let s = &self.sema;
        let ast = &s.ast;
        let kind = s.kind_of(f);
        match kind {
            DeclKind::Function => {}
            DeclKind::CXXMethod if s.is_static_method(f) => {}
            DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => return Err(EntryError::HasParameters),
            _ => return Err(EntryError::NotAFunction),
        }
        let Some(fd) = ast.decls.function(ast.latest(f)) else {
            return Err(EntryError::NotAFunction);
        };
        // A `constexpr` or `consteval` function (§3); one escalated to
        // immediate was declared `constexpr`.
        if fd.spec.constexpr_kind() == ConstexprKind::None && !s.is_immediate_fn(f) {
            return Err(EntryError::NotConstexpr);
        }
        let Some(fty) = ast.decls.get(f).ty() else { return Err(EntryError::NotAFunction) };
        let TypeView::Function(p) = ast.types.canonical_view(fty) else {
            return Err(EntryError::NotAFunction);
        };
        if p.num_params() != 0 || p.variadic() {
            return Err(EntryError::HasParameters);
        }
        if !self.entry_return_ok(p.ret()) {
            return Err(EntryError::WrongReturnType);
        }
        if s.defined_body(f).is_none() && ast.instantiated_from(f).is_none() {
            return Err(EntryError::Undefined);
        }
        Ok(Entry { decl: f })
    }

    /// `void`, `bool`, a character or integer type (`__int128` included;
    /// no enumeration, no `_BitInt`).
    fn entry_return_ok(&self, q: QualType) -> bool {
        let types = &self.sema.ast.types;
        if types.is_void(q) {
            return true;
        }
        types.is_integral(q) && !matches!(types.canonical_view(q), TypeView::BitInt { .. })
    }

    /// The function named by qualified name `name` (`f`, `ns::f`,
    /// `::ns::C::f`) as an entry (spec/eval-session.md §3).
    ///
    /// # Errors
    /// Why there is no such entry.
    pub fn entry_named(&self, name: &str) -> Result<Entry, EntryError> {
        let s = &self.sema;
        let not_found = || EntryError::NotFound { name: name.to_owned() };
        let path = name.trim().strip_prefix("::").unwrap_or(name.trim());
        let parts: Vec<&str> = path.split("::").map(str::trim).collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err(not_found());
        }
        let Some((last, scopes)) = parts.split_last() else { return Err(not_found()) };
        let sym = |p: &str| -> Symbol { s.ast.symbols().intern(p) };
        let mut ctx = s.ast.tu;
        for p in scopes {
            let found = s.lookup_qualified(ctx, sym(p), LookupKind::Nested);
            let [d] = found.decls.as_slice() else { return Err(not_found()) };
            ctx = match s.kind_of(*d) {
                DeclKind::Namespace => s.ast.first(*d),
                DeclKind::NamespaceAlias => s.ast.resolve_alias(*d),
                DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization => {
                    s.ast.definition(*d).unwrap_or(*d)
                }
                // A type alias of a class (`using C = T<int>;`).
                DeclKind::Typedef | DeclKind::TypeAlias => {
                    let Some(r) = s.ast.decls.get(*d).ty().and_then(|q| s.ast.types.as_record(q))
                    else {
                        return Err(not_found());
                    };
                    s.ast.definition(r).unwrap_or(r)
                }
                _ => return Err(not_found()),
            };
        }
        let found = s.lookup_qualified(ctx, sym(last), LookupKind::Ordinary);
        match found.decls.as_slice() {
            [] => Err(not_found()),
            [d] => self.entry(*d),
            ds => {
                let fns = ds
                    .iter()
                    .filter(|&&d| {
                        matches!(
                            s.kind_of(d),
                            DeclKind::Function
                                | DeclKind::CXXMethod
                                | DeclKind::FunctionTemplate
                                | DeclKind::CXXConversion
                        )
                    })
                    .count();
                if fns == ds.len() {
                    Err(EntryError::Overloaded { name: name.to_owned(), count: fns })
                } else {
                    Err(EntryError::NotAFunction)
                }
            }
        }
    }

    /// Runs `entry` once (spec/eval-session.md §4): host calls reach
    /// `host`, the run stops at `budget` or when `cancel` is cancelled.
    /// The session is usable after any outcome.
    pub fn run(
        &mut self,
        entry: Entry,
        host: &mut dyn HostCalls,
        budget: RunBudget,
        cancel: &CancelToken,
    ) -> RunReport {
        let f = entry.decl;
        let at = self.sema.ast.decls.get(f).loc();
        let diags = self.sema.diags;
        let mark = diags.capture_begin();
        let errors = diags.error_count();
        self.replay.done.clear();
        self.replay.charged.clear();
        self.replay.windows.clear();
        self.replay.loose = NestedCost::ZERO;
        self.sema.eval_replay = Some(std::mem::take(&mut self.replay));
        let fatal = self.sema.inst.fatal;
        let saved_loc = self.sema.inst.eval_loc.replace(at);
        let mut oven =
            self.sema.oven.take().unwrap_or_else(|| {
                unreachable!("an evaluation session's Oven is out between runs")
            });
        oven.set_clang_compat(self.sema.clang_compat);
        let year = Bridge { sema: &mut self.sema, run: None }.cxx_year();
        oven.set_cxx_year(year);
        let out = {
            let mut ctx = RunCtx { host };
            let mut b = Bridge { sema: &mut self.sema, run: Some(&mut ctx) };
            oven.session_run(&mut b, f, at, budget, cancel)
        };
        self.sema.oven = Some(oven);
        if let Some(rp) = self.sema.eval_replay.take() {
            self.replay = rp;
        }
        self.sema.inst.eval_loc = saved_loc;
        self.sema.inst.fatal = fatal;
        self.sema.inst.meta_calls.clear();
        let result = match out.result {
            // An error reported by semantic work the run needed (an
            // instantiation's) fails the run: it is its diagnostic (§4).
            Ok(_) if diags.error_count() > errors => Err(RunError::Failed),
            Ok(v) => Ok(v),
            Err((kind, fail)) => {
                self.report_failed_run(f, at, fail);
                Err(kind)
            }
        };
        let diagnostics = diags.capture_end(mark);
        RunReport {
            result,
            steps: out.steps,
            nested_steps: out.nested_steps,
            host_calls: out.host_calls,
            peak_heap: out.peak_heap,
            calls: out.calls,
            nested_calls: out.nested_calls,
            diagnostics,
        }
    }

    /// The diagnostic of a run of `f` that failed (spec/eval-session.md
    /// §7): an immediate entry's run is an immediate invocation, reported
    /// as one (with why an escalated function is immediate); a
    /// `constexpr` entry's names the entry, with the notes clang gives for
    /// `constexpr R v = f();`. Whether `f` is immediate is read now: a
    /// specialization the run instantiated may have escalated in it.
    fn report_failed_run(&mut self, f: DeclId, at: Loc, fail: kiln_oven::EvalFailure) {
        let s = &mut self.sema;
        let name = s.qualified_decl_name(f);
        let msg = if s.is_immediate_fn(f) {
            let consteval = s.is_consteval_kind(f);
            let mut msg = s
                .diag(DiagId::sema_immediate_call_non_constant, at)
                .arg(u8::from(consteval))
                .arg(name)
                .allow_duplicate();
            if !consteval && let Some(note) = s.escalation_reason_note(f) {
                msg = msg.with_note(note);
            }
            msg
        } else {
            s.diag(DiagId::sema_session_entry_non_constant, at).arg(name).allow_duplicate()
        };
        s.emit(Sema::with_failure_notes(msg, fail));
    }

    /// What the session holds now (spec/eval-session.md §10).
    #[must_use]
    pub fn kept(&self) -> KeptMemory {
        let oven = self.sema.oven();
        KeptMemory {
            ast: self.sema.ast.memory().reserved_bytes(),
            sema: self.sema.memory().reserved_bytes(),
            oven: oven.footprint() as u64,
            replay: (self.replay.count * size_of::<Diagnostic>()
                + self.replay.records.capacity() * size_of::<(Key, Rec)>()
                + self.replay.key_words * size_of::<u128>()) as u64,
            memo_entries: oven.memo_len(),
        }
    }

    /// Drops every memoized result (spec/eval-session.md §5); how many
    /// were dropped.
    pub fn clear_memo(&mut self) -> usize {
        oven_mut(&mut self.sema).clear_memo()
    }
}

/// The Oven of `sema` (at rest between runs).
fn oven_mut<'s>(sema: &'s mut Sema<'_, '_>) -> &'s mut Oven {
    match sema.oven.as_deref_mut() {
        Some(o) => o,
        None => unreachable!("the Oven is out outside an evaluation"),
    }
}
