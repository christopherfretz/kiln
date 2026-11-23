//! Evaluation sessions (spec/eval-session.md): a finished translation
//! unit kept to evaluate `consteval` entry functions of it many times,
//! each run isolated from every other, for tooling clients.
//!
//! The session owns the finished [`Sema`] (§1). A run
//! ([`EvalSession::run`]) is an immediate invocation of the entry at the
//! end of the translation unit, evaluated by the Oven
//! ([`Oven::session_run`]) through a bridge that carries the run's
//! host-call handler and the diagnostic replay of §7.

use std::fmt;

use kiln_ast::decl::{ConstexprKind, DeclKind};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType};
use kiln_diag::{DiagEngine, DiagId, Diagnostic};
pub use kiln_oven::{CANCEL_INTERVAL, CancelToken, RELEASE_FLOOR, RunBudget, RunError, RunValue};
use kiln_oven::{HostCalls, MetaOp, Oven, SemaBridge};
use kiln_source::fx::{FxHashMap, FxHashSet};
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::oven_bridge::Bridge;

/// A finished translation unit kept to run entries (spec/eval-session.md
/// §2). `Send`, not `Sync` (§9).
pub struct EvalSession<'a, 'sm> {
    sema: Sema<'a, 'sm>,
    replay: Replay,
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
    /// The function is not declared `consteval`.
    NotConsteval,
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
            EntryError::NotConsteval => f.write_str("the entry is not a consteval function"),
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
    /// Steps the run executed.
    pub steps: u64,
    /// Handler calls the run made.
    pub host_calls: u64,
    /// The run's peak live frame and heap bytes.
    pub peak_heap: u64,
    /// Function calls the run made.
    pub calls: u64,
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

/// What a recorded diagnostic belongs to (spec/eval-session.md §7).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    /// The compilation of the Oven's function (by index).
    Func(u32),
    /// A reflection intrinsic with its operands.
    Meta(MetaOp, Box<[u128]>),
    /// A `noexcept` query.
    Nothrow(DeclId),
}

/// One recorded piece: diagnostics, or what a nested key recorded.
#[derive(Clone, Debug)]
enum Seg {
    Diags(Vec<Diagnostic>),
    Child(Key),
}

/// A recording in progress: from capture index `from`, with the nested
/// keys recorded or replayed inside it (key, capture range).
#[derive(Debug)]
struct Window {
    from: usize,
    children: Vec<(Key, usize, usize)>,
}

/// The diagnostics semantic work reported in runs, replayed by later
/// runs (spec/eval-session.md §7).
#[derive(Debug, Default)]
pub(crate) struct Replay {
    records: FxHashMap<Key, Vec<Seg>>,
    /// The keys reported in the running run.
    done: FxHashSet<Key>,
    windows: Vec<Window>,
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
    replay: &'r mut Replay,
}

impl Replay {
    /// Reports the diagnostics of `k`, if it has any and they were not
    /// reported in this run yet.
    fn replay(&mut self, diags: &DiagEngine<'_>, k: &Key) {
        if !self.records.contains_key(k) || !self.done.insert(k.clone()) {
            return;
        }
        let start = diags.captured_len();
        let segs = self.records.get(k).cloned().unwrap_or_default();
        for s in &segs {
            match s {
                Seg::Diags(v) => diags.capture_replay(v),
                Seg::Child(c) => self.replay(diags, c),
            }
        }
        let end = diags.captured_len();
        if end > start
            && let Some(w) = self.windows.last_mut()
        {
            w.children.push((k.clone(), start, end));
        }
    }
}

impl Bridge<'_, '_, '_> {
    /// Opens a recording window (§7); its mark.
    pub(crate) fn window_begin(&mut self) -> usize {
        let Some(run) = self.run.as_mut() else { return 0 };
        let from = self.sema.diags.captured_len();
        run.replay.windows.push(Window { from, children: Vec::new() });
        from
    }

    /// Closes the window `mark` opened, recording its diagnostics under
    /// `key`: whether there were any.
    pub(crate) fn window_end(&mut self, key: Key, mark: usize) -> bool {
        let Some(run) = self.run.as_mut() else { return false };
        let Some(w) = run.replay.windows.pop() else { return false };
        debug_assert_eq!(w.from, mark, "session windows nest");
        let diags = self.sema.diags;
        let end = diags.captured_len();
        if end <= w.from {
            return false;
        }
        let all = diags.captured_from(w.from);
        let mut segs = Vec::new();
        let mut at = w.from;
        for (c, s, e) in w.children {
            if s > at {
                segs.push(Seg::Diags(all[at - w.from..s - w.from].to_vec()));
            }
            segs.push(Seg::Child(c));
            at = e;
        }
        if end > at {
            segs.push(Seg::Diags(all[at - w.from..].to_vec()));
        }
        let rp = &mut *run.replay;
        rp.count +=
            segs.iter().map(|s| if let Seg::Diags(v) = s { v.len() } else { 0 }).sum::<usize>();
        if let Key::Meta(op, words) = &key {
            rp.meta_ops.insert(*op);
            if !rp.records.contains_key(&key) {
                rp.key_words += words.len();
            }
        }
        rp.records.insert(key.clone(), segs);
        rp.done.insert(key.clone());
        if let Some(p) = rp.windows.last_mut() {
            p.children.push((key, w.from, end));
        }
        true
    }

    /// Reports the recorded diagnostics of `key` (§7).
    pub(crate) fn replay(&mut self, key: &Key) {
        let diags = self.sema.diags;
        if let Some(run) = self.run.as_mut() {
            run.replay.replay(diags, key);
        }
    }

    /// `f` (semantic work a bridge callback does) recorded under `key`
    /// in a run: what it reports is recorded; when it reports nothing
    /// but reported something in an earlier run, that is replayed. Either
    /// makes the calls in progress impure, so no memoized result skips
    /// it in a later run.
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
    /// intrinsic that reported nothing and has no record (no key of the
    /// op can be known then).
    pub(crate) fn tracked_by<R>(
        &mut self,
        op: Option<MetaOp>,
        key: impl FnOnce(&mut Self, &mut Oven) -> Key,
        oven: &mut Oven,
        f: impl FnOnce(&mut Self, &mut Oven) -> R,
    ) -> R {
        if self.run.is_none() {
            return f(self, oven);
        }
        let mark = self.window_begin();
        let r = f(self, oven);
        let quiet = self.sema.diags.captured_len() <= mark;
        if let (Some(op), true) = (op, quiet)
            && let Some(run) = self.run.as_mut()
            && !run.replay.meta_ops.contains(&op)
        {
            let w = run.replay.windows.pop();
            debug_assert!(w.is_some_and(|w| w.from == mark), "session windows nest");
            return r;
        }
        let key = key(self, oven);
        let known = self.run.as_ref().is_some_and(|run| run.replay.records.contains_key(&key));
        let impure = if known {
            if !self.window_end(key.clone(), mark) {
                self.replay(&key);
            }
            true
        } else {
            self.window_end(key, mark)
        };
        if impure {
            oven.mark_impure();
        }
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
        Ok(EvalSession { sema, replay: Replay::default(), tu_host })
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
        if fd.spec.constexpr_kind() != ConstexprKind::Consteval {
            return Err(EntryError::NotConsteval);
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
        self.replay.windows.clear();
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
            let mut ctx = RunCtx { host, replay: &mut self.replay };
            let mut b = Bridge { sema: &mut self.sema, run: Some(&mut ctx) };
            oven.session_run(&mut b, f, at, budget, cancel)
        };
        self.sema.oven = Some(oven);
        self.sema.inst.eval_loc = saved_loc;
        self.sema.inst.fatal = fatal;
        self.sema.inst.meta_calls.clear();
        let result = match out.result {
            // An error reported by semantic work the run needed (an
            // instantiation's) fails the run: it is its diagnostic (§4).
            Ok(_) if diags.error_count() > errors => Err(RunError::Failed),
            Ok(v) => Ok(v),
            Err((kind, fail)) => {
                let consteval = u8::from(true);
                let msg = self
                    .sema
                    .diag(DiagId::sema_immediate_call_non_constant, at)
                    .arg(consteval)
                    .arg(self.sema.qualified_decl_name(f))
                    .allow_duplicate();
                self.sema.emit(Sema::with_failure_notes(msg, fail));
                Err(kind)
            }
        };
        let diagnostics = diags.capture_end(mark);
        RunReport {
            result,
            steps: out.steps,
            host_calls: out.host_calls,
            peak_heap: out.peak_heap,
            calls: out.calls,
            diagnostics,
        }
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
                + self.replay.records.capacity() * size_of::<(Key, Vec<Seg>)>()
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
