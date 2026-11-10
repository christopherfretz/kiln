//! The diagnostics engine (`spec/diag.md` section "Engine"): applies the
//! warning policy, deduplicates, enforces the error limit, counts, and
//! hands [`Diagnostic`] records to a [`DiagSink`].

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard};

use kiln_source::{Loc, SourceManager, Span};

use crate::diag::{Diag, Diagnostic, Level};
use crate::groups::Flavor;
use crate::ids::DiagId;
use crate::json::JsonSink;
use crate::options::{DiagOptions, Format};
use crate::policy::{Policy, Severity};
use crate::pragma::Pragmas;
use crate::text::TextSink;

/// Where emitted diagnostics go. Sinks are called under the engine's lock,
/// one diagnostic at a time, in emission order.
pub trait DiagSink {
    /// Receives one diagnostic (with its notes).
    fn emit(&mut self, sm: &SourceManager, d: &Diagnostic);

    /// Flushes buffered output.
    fn flush(&mut self) {}
}

struct State<'sm> {
    sink: Box<dyn DiagSink + Send + 'sm>,
    seen: HashSet<(DiagId, Span, u64)>,
}

/// The diagnostics engine. Shared by reference (`&DiagEngine` is
/// `Send + Sync`); every method takes `&self`.
pub struct DiagEngine<'sm> {
    sm: &'sm SourceManager,
    policy: Policy,
    error_limit: u32,
    /// `-fclang-compat=`: ids clang 22 reworded print clang 19's text
    /// below 22 ([`DiagId::for_compat`]).
    clang_compat: u32,
    state: Mutex<State<'sm>>,
    /// `#pragma clang diagnostic` state ([`crate::pragma`]).
    pragmas: RwLock<Pragmas>,
    /// Some pragma changed the state: severities depend on location.
    has_pragmas: AtomicBool,
    errors: AtomicU32,
    /// Errors that are errors by default (clang `UncompilableErrorOccurred`).
    uncompilable: AtomicU32,
    /// Errors of class error (clang `isUnrecoverable`).
    unrecoverable: AtomicU32,
    warnings: AtomicU32,
    fatal: AtomicBool,
    /// [`Self::hold`] depth: while positive, diagnostics wait in `held`.
    holding: AtomicU32,
    held: Mutex<Vec<HeldDiag>>,
    /// What the held diagnostics will count, in the order `errors`,
    /// `uncompilable`, `unrecoverable`, `warnings`.
    held_counts: [AtomicU32; 4],
}

/// A diagnostic [`DiagEngine::hold`] keeps back.
struct HeldDiag {
    diag: Diag,
    id: DiagId,
    level: Level,
    dropped: bool,
}

impl std::fmt::Debug for DiagEngine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiagEngine")
            .field("error_limit", &self.error_limit)
            .field("errors", &self.error_count())
            .field("warnings", &self.warning_count())
            .field("fatal", &self.has_fatal())
            .finish_non_exhaustive()
    }
}

impl<'sm> DiagEngine<'sm> {
    /// Builds an engine over `sm` with the policy of `opts`, writing to
    /// `sink`. Diagnostics about the options themselves (queued in
    /// `opts.pending`, then unknown `-W`/`-R` groups) are emitted at once.
    pub fn new(
        sm: &'sm SourceManager,
        opts: &DiagOptions,
        sink: impl DiagSink + Send + 'sm,
    ) -> DiagEngine<'sm> {
        let (policy, reports) = Policy::new(opts);
        let engine = DiagEngine {
            sm,
            policy,
            error_limit: opts.error_limit,
            clang_compat: opts.clang_compat,
            state: Mutex::new(State { sink: Box::new(sink), seen: HashSet::new() }),
            pragmas: RwLock::new(Pragmas::new()),
            has_pragmas: AtomicBool::new(false),
            errors: AtomicU32::new(0),
            uncompilable: AtomicU32::new(0),
            unrecoverable: AtomicU32::new(0),
            warnings: AtomicU32::new(0),
            fatal: AtomicBool::new(false),
            holding: AtomicU32::new(0),
            held: Mutex::new(Vec::new()),
            held_counts: Default::default(),
        };
        for od in opts.pending.iter().chain(&reports) {
            let mut d = Diag::new(od.id, Span::INVALID);
            d.args.clone_from(&od.args);
            engine.emit(d);
        }
        engine
    }

    /// An engine writing to standard error in `opts.format`.
    pub fn with_stderr(sm: &'sm SourceManager, opts: &DiagOptions) -> DiagEngine<'sm> {
        match opts.format {
            Format::Text => DiagEngine::new(sm, opts, TextSink::new(io::stderr(), opts)),
            Format::Json => DiagEngine::new(sm, opts, JsonSink::new(io::stderr(), opts)),
        }
    }

    /// The source manager diagnostics refer to.
    pub fn sm(&self) -> &'sm SourceManager {
        self.sm
    }

    /// The severity `id` would be emitted with by the command-line
    /// options alone (`Ignored` for notes); see [`Self::severity_at`].
    pub fn severity(&self, id: DiagId) -> Severity {
        self.policy.severity(id)
    }

    /// The severity `id` located at `loc` would be emitted with, under
    /// the diagnostic pragmas in effect there (an invalid location: the
    /// current state). Ignores the system-header rule.
    pub fn severity_at(&self, id: DiagId, loc: Loc) -> Severity {
        if !self.has_pragmas.load(Ordering::Acquire) {
            return self.policy.severity(id);
        }
        match self.pragmas().mapping_at(self.sm, id, loc) {
            Some(m) => self.policy.severity_with(id, m),
            None => self.policy.severity(id),
        }
    }

    fn pragmas(&self) -> RwLockReadGuard<'_, Pragmas> {
        self.pragmas.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn update_pragmas<T>(&self, f: impl FnOnce(&mut Pragmas) -> T) -> T {
        let mut p = self.pragmas.write().unwrap_or_else(PoisonError::into_inner);
        let r = f(&mut p);
        self.has_pragmas.store(!p.is_empty(), Ordering::Release);
        r
    }

    /// `#pragma clang diagnostic push` (or `GCC`) at `at`.
    pub fn pragma_push(&self, _at: Loc) {
        self.update_pragmas(Pragmas::push);
    }

    /// `#pragma clang diagnostic pop` at `at`; false if nothing was
    /// pushed ("pragma diagnostic pop could not pop").
    pub fn pragma_pop(&self, at: Loc) -> bool {
        let at = self.sm.expansion_loc(at);
        self.update_pragmas(|p| p.pop(at))
    }

    /// `#pragma clang diagnostic ignored/warning/error/fatal "-W<group>"`
    /// (`remark`: `"-R<group>"`) at `at`; `group` may be `everything`.
    /// False if the group is unknown.
    pub fn pragma_set(&self, at: Loc, remark: bool, group: &str, sev: Severity) -> bool {
        let at = self.sm.expansion_loc(at);
        let flavor = if remark { Flavor::Remark } else { Flavor::WarningOrError };
        self.update_pragmas(|p| p.set(&self.policy, at, flavor, group, sev))
    }

    /// The preprocessor left an included file, resuming its includer at
    /// `resume`: the diagnostic state at the end of the file continues
    /// there (a pragma in a header without `push` reaches the includer).
    pub fn pragma_file_exit(&self, resume: Loc) {
        if self.has_pragmas.load(Ordering::Acquire) {
            self.update_pragmas(|p| p.file_exit(self.sm, resume));
        }
    }

    /// Whether warnings located in system headers are dropped: true
    /// unless `-Wsystem-headers` (clang `getSuppressSystemWarnings`).
    pub fn suppress_system_warnings(&self) -> bool {
        self.policy.suppress_system
    }

    /// Whether `id` located at `loc` would be emitted: enabled
    /// ([`Self::is_enabled`]) and not suppressed in a system header (§5.4).
    pub fn is_enabled_at(&self, id: DiagId, loc: Loc) -> bool {
        self.severity_at(id, loc) != Severity::Ignored && !self.suppressed_in_system_header(id, loc)
    }

    /// clang's system-header rules (the tail of `getDiagnosticSeverity`),
    /// both off under `-Wsystem-headers`: a diagnostic that does not show
    /// in system headers, whose location's expansion is in one, is
    /// ignored; so is one that does not show in system macros, whose
    /// location is spelled in a macro from a system header (§5.4).
    fn suppressed_in_system_header(&self, id: DiagId, loc: Loc) -> bool {
        self.policy.suppress_system
            && loc.is_valid()
            && ((!id.shows_in_system_header() && self.sm.is_in_system_header(loc))
                || (!id.shows_in_system_macro() && self.sm.is_in_system_macro(loc)))
    }

    /// Whether `id` would be emitted at all: enabled by the options or
    /// by some diagnostic pragma. Use it to skip expensive checks for
    /// disabled warnings.
    pub fn is_enabled(&self, id: DiagId) -> bool {
        self.severity(id) != Severity::Ignored
            || (self.has_pragmas.load(Ordering::Acquire) && self.pragmas().enables(id))
    }

    /// Whether the last of `-W<group>` and `-Wno-<group>` on the command
    /// line was the positive one; `None` if neither was given. clang
    /// reads a few groups as language options (`-Winvalid-constexpr`
    /// sets `CheckConstexprFunctionBodies`), which `-Weverything`,
    /// `-Werror=` and pragmas do not change.
    pub fn group_flag(&self, group: &str) -> Option<bool> {
        self.policy.group_flag(group)
    }

    /// Errors emitted so far (fatal errors included).
    pub fn error_count(&self) -> u32 {
        self.errors.load(Ordering::Acquire) + self.held_count(0)
    }

    /// Errors emitted so far whose id is an error by default (fatal
    /// errors included): clang `hasUncompilableErrorOccurred`. A warning
    /// promoted by `-Werror` counts for the exit status
    /// ([`Self::error_count`]) but not here, so the analyses that stop
    /// after an error (sema's flow warnings) go on as for a warning
    /// (spec/diag.md §7.5).
    pub fn uncompilable_error_count(&self) -> u32 {
        self.uncompilable.load(Ordering::Acquire) + self.held_count(1)
    }

    /// Errors emitted so far whose id is of class error (fatal errors
    /// included): clang `DiagnosticErrorTrap::hasUnrecoverableErrorOccurred`,
    /// which also leaves out a warning that is an error by default.
    pub fn unrecoverable_error_count(&self) -> u32 {
        self.unrecoverable.load(Ordering::Acquire) + self.held_count(2)
    }

    /// Warnings emitted so far.
    pub fn warning_count(&self) -> u32 {
        self.warnings.load(Ordering::Acquire) + self.held_count(3)
    }

    fn held_count(&self, i: usize) -> u32 {
        self.held_counts[i].load(Ordering::Acquire)
    }

    /// Adds (or with `sub`, takes away) what a held diagnostic counts.
    fn count_held(&self, id: DiagId, level: Level, sub: bool) {
        let bump = |i: usize| {
            let c = &self.held_counts[i];
            if sub {
                c.fetch_sub(1, Ordering::AcqRel);
            } else {
                c.fetch_add(1, Ordering::AcqRel);
            }
        };
        match level {
            Level::Error => {
                bump(0);
                if id.is_default_error() {
                    bump(1);
                }
                if !id.is_warning_or_extension() {
                    bump(2);
                }
            }
            Level::Warning => bump(3),
            _ => {}
        }
    }

    /// Starts holding back every diagnostic but a fatal one until the
    /// matching [`Self::release`]: sema's speculative typo correction
    /// (spec/diag.md "Held diagnostics") decides which of them to drop
    /// ([`Self::drop_held`]). Held diagnostics count as emitted. Holds
    /// nest.
    pub fn hold(&self) {
        self.holding.fetch_add(1, Ordering::AcqRel);
    }

    /// How many diagnostics are held: the index the next held one gets.
    pub fn held_len(&self) -> usize {
        self.held.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// The held errors from index `from` on, not dropped, with their ids.
    pub fn held_errors_from(&self, from: usize) -> Vec<(usize, DiagId)> {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        let tail = held.get(from..).unwrap_or_default();
        let errors = tail.iter().enumerate().filter(|(_, h)| !h.dropped && h.level == Level::Error);
        errors.map(|(i, h)| (from + i, h.id)).collect()
    }

    /// Drops held diagnostic `i`: it will not be emitted or counted.
    pub fn drop_held(&self, i: usize) {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(h) = held.get_mut(i)
            && !h.dropped
        {
            h.dropped = true;
            self.count_held(h.id, h.level, true);
        }
    }

    /// Ends a [`Self::hold`]; the outermost emits the held diagnostics
    /// not dropped, in order.
    pub fn release(&self) {
        let prev =
            self.holding.try_update(Ordering::AcqRel, Ordering::Acquire, |d| d.checked_sub(1));
        match prev {
            Ok(1) => {}
            Ok(_) => return,
            Err(_) => {
                debug_assert!(false, "release without hold");
                return;
            }
        }
        let held = std::mem::take(&mut *self.held.lock().unwrap_or_else(PoisonError::into_inner));
        for h in held.into_iter().filter(|h| !h.dropped) {
            self.count_held(h.id, h.level, true);
            self.emit(h.diag);
        }
    }

    /// Whether a fatal error was emitted; every later diagnostic is dropped.
    pub fn has_fatal(&self) -> bool {
        self.fatal.load(Ordering::Acquire)
    }

    /// clang's closing line (`2 warnings and 1 error generated.`), or `None`
    /// when nothing was counted.
    pub fn summary(&self) -> Option<String> {
        let (w, e) = (self.warning_count(), self.error_count());
        if w == 0 && e == 0 {
            return None;
        }
        let mut s = String::new();
        if w > 0 {
            let _ = write!(s, "{w} warning{}", if w == 1 { "" } else { "s" });
        }
        if w > 0 && e > 0 {
            s.push_str(" and ");
        }
        if e > 0 {
            let _ = write!(s, "{e} error{}", if e == 1 { "" } else { "s" });
        }
        s.push_str(" generated.");
        Some(s)
    }

    /// Flushes the sink.
    pub fn flush(&self) {
        self.lock().sink.flush();
    }

    fn lock(&self) -> MutexGuard<'_, State<'sm>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether [`emit`](Self::emit) would print `d` now: it is not
    /// ignored, not suppressed in a system header, not a duplicate, and
    /// no fatal error has occurred. Lets a caller skip work (sema's
    /// instantiation-context notes, which print once per depth) for a
    /// diagnostic that will be dropped.
    pub fn would_emit(&self, d: &Diag) -> bool {
        let Some(id) = d.id else { return false };
        let id = id.for_compat(self.clang_compat);
        if self.has_fatal() || self.severity_at(id, d.span.lo) == Severity::Ignored {
            return false;
        }
        if self.suppressed_in_system_header(id, d.span.lo) {
            return false;
        }
        !Self::is_duplicate(&self.lock(), id, d)
    }

    /// The clang compatibility level diagnostics are worded for
    /// (`-fclang-compat=`, [`DiagOptions::clang_compat`]).
    pub fn clang_compat(&self) -> u32 {
        self.clang_compat
    }

    /// The deduplication rule (spec/diag.md §7.4): the key is
    /// `(id, span, context)`; a diagnostic with a context (inside a
    /// template instantiation) is also a duplicate of the same
    /// `(id, span)` reported with none (in the template definition).
    fn is_duplicate(st: &State<'_>, id: DiagId, d: &Diag) -> bool {
        if d.allow_duplicate || !d.span.is_valid() {
            return false;
        }
        st.seen.contains(&(id, d.span, d.context))
            || (d.context != 0 && !d.per_instantiation && st.seen.contains(&(id, d.span, 0)))
    }

    /// Applies policy to `d` and sinks it. Same as [`Diag::emit`].
    pub fn emit(&self, mut d: Diag) {
        d.set_compat(self.clang_compat);
        let Some(id) = d.id else {
            debug_assert!(false, "free-text diagnostics are notes only");
            return;
        };
        if self.has_fatal() {
            return;
        }
        let level = match self.severity_at(id, d.span.lo) {
            Severity::Ignored => return,
            Severity::Remark => Level::Remark,
            Severity::Warning => Level::Warning,
            Severity::Error => Level::Error,
            Severity::Fatal => Level::Fatal,
        };
        if self.suppressed_in_system_header(id, d.span.lo) {
            return;
        }
        if level != Level::Fatal && self.holding.load(Ordering::Acquire) > 0 {
            let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
            self.count_held(id, level, false);
            held.push(HeldDiag { diag: d, id, level, dropped: false });
            return;
        }
        let mut st = self.lock();
        // Re-check under the lock: another thread may have gone fatal.
        if self.has_fatal() {
            return;
        }
        if Self::is_duplicate(&st, id, &d) {
            return;
        }
        if !d.allow_duplicate && d.span.is_valid() {
            st.seen.insert((id, d.span, d.context));
        }
        let record = if level == Level::Error
            && self.error_limit != 0
            && self.errors.load(Ordering::Acquire) >= self.error_limit
        {
            Diagnostic {
                id: Some(DiagId::diag_too_many_errors),
                level: Level::Fatal,
                message: crate::template::render(DiagId::diag_too_many_errors.template(), &[]),
                span: Span::INVALID,
                labels: Vec::new(),
                fixits: Vec::new(),
                option: Some("-ferror-limit=".to_string()),
                notes: Vec::new(),
            }
        } else {
            Diagnostic {
                id: Some(id),
                level,
                message: d.message(),
                span: d.span,
                notes: d.notes.iter().map(Diagnostic::from_note).collect(),
                labels: d.labels,
                fixits: d.fixits,
                option: option_suffix(id, level),
            }
        };
        match record.level {
            Level::Error => {
                self.errors.fetch_add(1, Ordering::AcqRel);
                if id.is_default_error() {
                    self.uncompilable.fetch_add(1, Ordering::AcqRel);
                }
                if !id.is_warning_or_extension() {
                    self.unrecoverable.fetch_add(1, Ordering::AcqRel);
                }
            }
            Level::Fatal => {
                self.errors.fetch_add(1, Ordering::AcqRel);
                self.uncompilable.fetch_add(1, Ordering::AcqRel);
                self.unrecoverable.fetch_add(1, Ordering::AcqRel);
                self.fatal.store(true, Ordering::Release);
            }
            Level::Warning => {
                self.warnings.fetch_add(1, Ordering::AcqRel);
            }
            Level::Remark | Level::Note => {}
        }
        let error_notes = record.notes.iter().filter(|n| n.level == Level::Error).count();
        let error_notes = u32::try_from(error_notes).unwrap_or(u32::MAX);
        if error_notes != 0 {
            self.errors.fetch_add(error_notes, Ordering::AcqRel);
            self.uncompilable.fetch_add(error_notes, Ordering::AcqRel);
            self.unrecoverable.fetch_add(error_notes, Ordering::AcqRel);
        }
        st.sink.emit(self.sm, &record);
    }
}

/// clang's `printDiagnosticOptions`: what goes between the brackets after
/// the message.
fn option_suffix(id: DiagId, level: Level) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if level == Level::Error && id.is_warning_or_extension() && !id.is_default_error() {
        parts.push("-Werror".to_string());
    }
    if let Some(g) = id.display_group() {
        let flag = if level == Level::Remark { "-R" } else { "-W" };
        parts.push(format!("{flag}{}", g.name()));
    }
    if parts.is_empty() { None } else { Some(parts.join(",")) }
}

/// A sink that keeps records in memory. Clones share the same storage, so
/// keep one clone and give the other to the engine.
#[derive(Clone, Debug, Default)]
pub struct MemorySink {
    store: Arc<Mutex<Vec<Diagnostic>>>,
}

impl MemorySink {
    /// An empty sink.
    pub fn new() -> MemorySink {
        MemorySink::default()
    }

    /// Removes and returns the records received so far.
    pub fn take(&self) -> Vec<Diagnostic> {
        std::mem::take(&mut *self.store.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// A copy of the records received so far.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl DiagSink for MemorySink {
    fn emit(&mut self, _sm: &SourceManager, d: &Diagnostic) {
        self.store.lock().unwrap_or_else(PoisonError::into_inner).push(d.clone());
    }
}

/// A shareable in-memory byte buffer implementing [`Write`], for capturing
/// text or JSON output. Clones share the same buffer.
#[derive(Clone, Debug, Default)]
pub struct SharedBuffer {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl SharedBuffer {
    /// An empty buffer.
    pub fn new() -> SharedBuffer {
        SharedBuffer::default()
    }

    /// The bytes written so far.
    pub fn bytes(&self) -> Vec<u8> {
        self.buf.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The bytes written so far, as text (lossily decoded).
    pub fn contents(&self) -> String {
        String::from_utf8_lossy(&self.bytes()).into_owned()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
