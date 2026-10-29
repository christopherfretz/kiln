//! The diagnostics engine (`spec/diag.md` section "Engine"): applies the
//! warning policy, deduplicates, enforces the error limit, counts, and
//! hands [`Diagnostic`] records to a [`DiagSink`].

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use kiln_source::{SourceManager, Span};

use crate::diag::{Diag, Diagnostic, Level};
use crate::ids::DiagId;
use crate::json::JsonSink;
use crate::options::{DiagOptions, Format};
use crate::policy::{Policy, Severity};
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
    seen: HashSet<(DiagId, Span)>,
}

/// The diagnostics engine. Shared by reference (`&DiagEngine` is
/// `Send + Sync`); every method takes `&self`.
pub struct DiagEngine<'sm> {
    sm: &'sm SourceManager,
    policy: Policy,
    error_limit: u32,
    state: Mutex<State<'sm>>,
    errors: AtomicU32,
    warnings: AtomicU32,
    fatal: AtomicBool,
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
            state: Mutex::new(State { sink: Box::new(sink), seen: HashSet::new() }),
            errors: AtomicU32::new(0),
            warnings: AtomicU32::new(0),
            fatal: AtomicBool::new(false),
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

    /// The severity `id` would be emitted with (`Ignored` for notes).
    pub fn severity(&self, id: DiagId) -> Severity {
        self.policy.severity(id)
    }

    /// Whether warnings located in system headers are dropped: true
    /// unless `-Wsystem-headers` (clang `getSuppressSystemWarnings`).
    pub fn suppress_system_warnings(&self) -> bool {
        self.policy.suppress_system
    }

    /// Whether `id` located at `loc` would be emitted: enabled
    /// ([`Self::is_enabled`]) and not suppressed in a system header (§5.4).
    pub fn is_enabled_at(&self, id: DiagId, loc: kiln_source::Loc) -> bool {
        self.is_enabled(id) && !self.suppressed_in_system_header(id, loc)
    }

    /// clang's system-header rule (the tail of `getDiagnosticSeverity`):
    /// a diagnostic that does not show in system headers, whose location's
    /// expansion is in one, is ignored unless `-Wsystem-headers`.
    fn suppressed_in_system_header(&self, id: DiagId, loc: kiln_source::Loc) -> bool {
        self.policy.suppress_system
            && !id.shows_in_system_header()
            && loc.is_valid()
            && self.sm.is_in_system_header(loc)
    }

    /// Whether `id` would be emitted at all. Use it to skip expensive
    /// checks for disabled warnings.
    pub fn is_enabled(&self, id: DiagId) -> bool {
        self.severity(id) != Severity::Ignored
    }

    /// Errors emitted so far (fatal errors included).
    pub fn error_count(&self) -> u32 {
        self.errors.load(Ordering::Acquire)
    }

    /// Warnings emitted so far.
    pub fn warning_count(&self) -> u32 {
        self.warnings.load(Ordering::Acquire)
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

    /// Applies policy to `d` and sinks it. Same as [`Diag::emit`].
    pub fn emit(&self, d: Diag) {
        let Some(id) = d.id else {
            debug_assert!(false, "free-text diagnostics are notes only");
            return;
        };
        if self.has_fatal() {
            return;
        }
        let level = match self.policy.severity(id) {
            Severity::Ignored => return,
            Severity::Remark => Level::Remark,
            Severity::Warning => Level::Warning,
            Severity::Error => Level::Error,
            Severity::Fatal => Level::Fatal,
        };
        if self.suppressed_in_system_header(id, d.span.lo) {
            return;
        }
        let mut st = self.lock();
        // Re-check under the lock: another thread may have gone fatal.
        if self.has_fatal() {
            return;
        }
        if !d.allow_duplicate && d.span.is_valid() && !st.seen.insert((id, d.span)) {
            return;
        }
        let record = if level == Level::Error
            && self.error_limit != 0
            && self.error_count() >= self.error_limit
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
            }
            Level::Fatal => {
                self.errors.fetch_add(1, Ordering::AcqRel);
                self.fatal.store(true, Ordering::Release);
            }
            Level::Warning => {
                self.warnings.fetch_add(1, Ordering::AcqRel);
            }
            Level::Remark | Level::Note => {}
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
