//! Evaluation sessions opened from a driver command line
//! (spec/eval-session.md §2): the translation unit is parsed as
//! `-fsyntax-only` parses it (include paths, predefines, the resource
//! directory, `-std=`, `--target=`, `-W` options and constexpr limits
//! apply), with `__builtin_kiln_host_call` visible, and the finished
//! front end is handed to a closure as a [`SessionFrontEnd`].

use std::io::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::{
    Diag, DiagEngine, DiagId, DiagOptions, DiagRecord, DiagSink, Diagnostic, Format, Level,
};
use kiln_lex::Interner;
use kiln_parse::{EvalSession, HostCalls, HostError, HostOut, OpenError};
pub use kiln_sema::stack_guard::StackPhase;
use kiln_source::{FileId, SourceManager, Span};

use crate::args::{Action, Lang};
use crate::front_end::{Consulted, Host};
use crate::pp_cmd::Job;
use crate::syntax_cmd::SyntaxJob;
use crate::{Io, codegen_cmd, make_engine};

/// The finished front end of a translation unit with its session
/// (spec/eval-session.md §2). Fields may be added.
#[derive(Debug)]
#[non_exhaustive]
pub struct SessionFrontEnd<'a, 'sm> {
    /// The session.
    pub session: EvalSession<'a, 'sm>,
    /// The source manager (for diagnostics' locations).
    pub sources: &'sm SourceManager,
    /// The main file.
    pub main: Option<FileId>,
    /// The command line's diagnostic options (the output format, the
    /// display settings, the clang level), which [`SessionFrontEnd::render`]
    /// renders with (WP-2324).
    pub diag_options: DiagOptions,
    /// The translation unit's own diagnostics (warnings and remarks: a
    /// unit with an error opens no session), as the engine's sink received
    /// them and printed them on standard error (WP-2324).
    pub tu_diagnostics: Vec<Diagnostic>,
    /// The paths the unit depends on that are in no buffer of
    /// [`SessionFrontEnd::sources`] it entered: `#embed` resources and
    /// `__has_include` / `__has_embed` hits, with what was read there
    /// (size and digest), in the order first met, each once; the list
    /// `FrontEnd::consulted` gives for the same command line (WP-2412).
    pub consulted: Vec<Consulted>,
}

impl SessionFrontEnd<'_, '_> {
    /// `records` (a run's `RunReport::diagnostics`, or a refusal's)
    /// rendered against [`SessionFrontEnd::sources`] exactly as the driver
    /// renders diagnostics on standard error for this command line, in
    /// its format (text, or JSON under `-fdiagnostics-format=json`), each
    /// note and call-stack frame under its diagnostic
    /// (spec/eval-session.md §7). [`kiln_diag::render_diagnostics`] with
    /// [`SessionFrontEnd::diag_options`]; to render another form, call it
    /// with changed options.
    #[must_use]
    pub fn render(&self, records: &[Diagnostic]) -> String {
        kiln_diag::render_diagnostics(self.sources, &self.diag_options, records)
    }

    /// `records` as data ([`kiln_diag::diag_records`] with
    /// [`SessionFrontEnd::diag_options`], spec/diag.md §10.4): per
    /// diagnostic its id, severity, message, group, option, caret
    /// location (buffer and offset, presumed file, line and column) and
    /// notes, the content of its `-fdiagnostics-format=json` object,
    /// usable after the session ends (WP-2827).
    #[must_use]
    pub fn records(&self, records: &[Diagnostic]) -> Vec<DiagRecord> {
        kiln_diag::diag_records(self.sources, &self.diag_options, records)
    }
}

/// Why [`with_eval_session`] opened no session (spec/eval-session.md §2
/// "Refusal").
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionRefused {
    /// The command line does not name exactly one C++ source input, or
    /// the driver rejected it before the translation unit's job ran
    /// (reported on standard error).
    CommandLine {
        /// The driver's first error as the text form prints it after
        /// `error: `, or why the command line names no one C++ source.
        message: String,
    },
    /// The translation unit reported errors and no fatal error other
    /// than the error limit's (reported on standard error, then
    /// `drv_eval_session_tu_errors`).
    #[non_exhaustive]
    TuHasErrors {
        /// How many (the error limit's fatal error counts as one).
        errors: u32,
        /// Every diagnostic the unit reported, warnings included, as the
        /// engine's sink received them. Their locations are the unit's
        /// source manager's, which ends with the call: after it returns,
        /// use [`SessionRefused::rendered`] (WP-2332).
        diagnostics: Vec<Diagnostic>,
        /// `diagnostics` rendered as the driver printed them on standard
        /// error, in the command line's format (text or JSON), without
        /// the summary line: usable after the call returns (WP-2332).
        rendered: String,
        /// `diagnostics` as data, usable after the call returns
        /// ([`SessionFrontEnd::records`]; spec/eval-session.md §2,
        /// WP-2827).
        records: Vec<DiagRecord>,
    },
    /// The front end stopped: a fatal error other than the error limit's
    /// (a header not found, an internal limit such as the include depth),
    /// or no tree was built (reported on standard error).
    #[non_exhaustive]
    Stopped {
        /// Every diagnostic the unit reported, the fatal one last (their
        /// locations as [`SessionRefused::TuHasErrors`]'s).
        diagnostics: Vec<Diagnostic>,
        /// `diagnostics` rendered as for [`SessionRefused::TuHasErrors`].
        rendered: String,
        /// `diagnostics` as data, as for [`SessionRefused::TuHasErrors`].
        records: Vec<DiagRecord>,
        /// The native-stack guard tripped (`-fkiln-stack-size`), in this
        /// phase: the fatal error is `sema_stack_exhausted` (WP-2827).
        /// `None`: another fatal error stopped the unit.
        stack_exhausted: Option<StackPhase>,
    },
    /// The front end panicked (the payload as text): the panic is caught,
    /// as `run_front_end`'s `FrontEndError::Panicked` is, and the closure
    /// called with this. A panic of the closure itself is not caught.
    Panicked(String),
}

impl SessionRefused {
    /// What the refusal reported, as text a client may keep after
    /// [`with_eval_session`] returns (WP-2332): the rendered diagnostics
    /// of a unit with errors or a stopped one, the message of a command
    /// line, the payload of a panic.
    #[must_use]
    pub fn rendered(&self) -> &str {
        match self {
            SessionRefused::CommandLine { message } | SessionRefused::Panicked(message) => message,
            SessionRefused::TuHasErrors { rendered, .. }
            | SessionRefused::Stopped { rendered, .. } => rendered,
        }
    }

    /// The unit's diagnostics as data (spec/eval-session.md §2,
    /// WP-2827): those of a unit with errors or a stopped one, empty for
    /// a command line or a panic.
    #[must_use]
    pub fn records(&self) -> &[DiagRecord] {
        match self {
            SessionRefused::TuHasErrors { records, .. }
            | SessionRefused::Stopped { records, .. } => records,
            SessionRefused::CommandLine { .. } | SessionRefused::Panicked(_) => &[],
        }
    }

    /// The phase in which the native-stack guard stopped the unit, if it
    /// did (WP-2827): a client tells a stack trip, the host's limit,
    /// from an error of the program without reading the rendered text.
    #[must_use]
    pub fn stack_exhausted(&self) -> Option<StackPhase> {
        match self {
            SessionRefused::Stopped { stack_exhausted, .. } => *stack_exhausted,
            _ => None,
        }
    }
}

/// The stack-guard phase of a stopped unit's `diagnostics`: its
/// `sema_stack_exhausted`'s, if it has one.
fn stack_phase(diagnostics: &[Diagnostic]) -> Option<StackPhase> {
    diagnostics.iter().find_map(|d| StackPhase::of_record(d.id, &d.message))
}

/// The message of [`SessionRefused::CommandLine`] for a command line
/// without one C++ source input.
const NOT_ONE_SOURCE: &str = "the command line does not name exactly one C++ source input";

/// The translation unit's handler of a session opened here: every call
/// fails (runs get their own handler).
#[derive(Debug, Default)]
struct Placeholder;

impl HostCalls for Placeholder {
    fn call(
        &mut self,
        _op: u32,
        _ints: &[u64],
        _bytes: &[u8],
        _out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        Err(HostError::new("host calls are served only in evaluation-session runs"))
    }
}

/// The translation unit's sink: the driver's ([`kiln_diag::format_sink`])
/// keeping a copy of every record for a refusal.
struct Recorder {
    inner: Box<dyn DiagSink + Send>,
    records: Arc<Mutex<Vec<Diagnostic>>>,
}

impl DiagSink for Recorder {
    fn emit(&mut self, sm: &SourceManager, d: &Diagnostic) {
        self.records.lock().unwrap_or_else(PoisonError::into_inner).push(d.clone());
        self.inner.emit(sm, d);
    }

    fn flush(&mut self) {
        self.inner.flush();
    }
}

/// Parses the one C++ source input of `command_line` (the driver's
/// arguments after `argv0`, words already split: an `@file` word is an
/// ordinary word, never a response file, so every option that reaches the
/// driver is one of the words given) as `-fsyntax-only` would, opens an
/// evaluation session on it and calls `f` with the front end, or with
/// why there is none ([`SessionRefused`]). The command line goes through
/// the driver's stages as the front-end library entry's does
/// ([`crate::front_end::prepare`], the PCH probe off), whatever action it
/// names. The translation unit's diagnostics go to `io`'s standard error
/// as the driver prints them; runs' diagnostics are returned in their
/// reports. A panic of the front end calls `f` with
/// [`SessionRefused::Panicked`]; one of `f` propagates.
pub fn with_eval_session<R>(
    argv0: &str,
    command_line: &[String],
    io: Io,
    f: impl for<'a, 'sm> FnOnce(Result<SessionFrontEnd<'a, 'sm>, SessionRefused>) -> R,
) -> R {
    let stderr = io.stderr;
    // The words as given: no `@file` is read (spec/eval-session.md §2,
    // WP-2411).
    let mut args = crate::front_end::entry_args(
        argv0,
        command_line,
        &crate::env::Environment::process(),
        &kiln_source::WorkDir::process(),
    );
    args.action = Action::SyntaxOnly;
    args.host_call_test_handler = false;
    args.host_calls_visible = true;
    let one_cxx_source = match args.inputs.as_slice() {
        [i] => i.path != "-" && codegen_cmd::is_source(i) && args.input_lang(i) == Lang::Cxx,
        _ => false,
    };
    if !one_cxx_source {
        return f(Err(SessionRefused::CommandLine { message: NOT_ONE_SOURCE.to_owned() }));
    }
    // `f` is taken only to be called: still here after a panic, the panic
    // was the front end's.
    let mut f = Some(f);
    let rules = crate::EntryRules { pch_probe: false };
    let host = Host::new(Some(stderr.clone()));
    let opened = panic::catch_unwind(AssertUnwindSafe(|| {
        crate::crash_for_testing(&args);
        crate::front_end::prepare(&args, rules, None, &host, &stderr, |job| {
            open(job, &stderr, &mut f)
        })
    }));
    kiln_source::activity::set_crash_for_testing(None);
    let opened = match opened {
        Ok(r) => r.flatten(),
        Err(payload) => match f.take() {
            Some(f) => {
                let text = crate::front_end::payload_text(&*payload);
                return f(Err(SessionRefused::Panicked(text)));
            }
            None => panic::resume_unwind(payload),
        },
    };
    match (opened, f.take()) {
        (Some(r), _) => r,
        (None, Some(f)) => {
            let message = host.first_error().map_or_else(
                || "the driver stopped before the translation unit's job".to_owned(),
                |e| e.message,
            );
            f(Err(SessionRefused::CommandLine { message }))
        }
        // `f` is taken only by the job, which returns its result.
        (None, None) => unreachable!("the session's closure was called and its result lost"),
    }
}

/// The job of [`with_eval_session`]: the parse, the session opened, `f`
/// (taken from its slot) called. `None` only with an empty slot.
fn open<R, F>(job: Job<'_>, stderr: &crate::SharedWriter, f: &mut Option<F>) -> Option<R>
where
    F: for<'a, 'sm> FnOnce(Result<SessionFrontEnd<'a, 'sm>, SessionRefused>) -> R,
{
    let diag_opts = job.diag_opts.clone();
    let path = job.path.to_owned();
    let program = job.args.program.clone();
    let sm = SourceManager::new();
    let interner = Arc::new(Interner::new());
    let records = Arc::new(Mutex::new(Vec::new()));
    let sink = Recorder {
        inner: kiln_diag::format_sink(stderr.clone(), &diag_opts, None),
        records: Arc::clone(&records),
    };
    let engine = DiagEngine::new(&sm, &diag_opts, sink);
    let parsed = crate::crash::catching(&path, None, &sm, || {
        SyntaxJob(job).parse_retained(&sm, &engine, &interner, Box::new(Placeholder))
    });
    // A crash phase asked for is the front end's: runs do not crash.
    kiln_source::activity::set_crash_for_testing(None);
    let summary = |engine: &DiagEngine<'_>| {
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == Format::Text
        {
            let mut err = stderr.clone();
            let _ = writeln!(err, "{s}");
        }
    };
    let take = || std::mem::take(&mut *records.lock().unwrap_or_else(PoisonError::into_inner));
    let Some((sema, main, consulted)) = parsed else {
        summary(&engine);
        let diagnostics = take();
        let rendered = kiln_diag::render_diagnostics(&sm, &diag_opts, &diagnostics);
        let records = kiln_diag::diag_records(&sm, &diag_opts, &diagnostics);
        let stack_exhausted = stack_phase(&diagnostics);
        let refused = SessionRefused::Stopped { diagnostics, rendered, records, stack_exhausted };
        return f.take().map(|f| f(Err(refused)));
    };
    match EvalSession::open(sema) {
        Ok(session) => {
            engine.flush();
            let fe = SessionFrontEnd {
                session,
                sources: &sm,
                main,
                diag_options: diag_opts.clone(),
                tu_diagnostics: take(),
                consulted,
            };
            f.take().map(|f| f(Ok(fe)))
        }
        Err(e) => {
            summary(&engine);
            let errors = match e {
                OpenError::TuHasErrors { errors } => errors,
                _ => 1,
            };
            let dsm = SourceManager::new();
            let drv = make_engine(
                &dsm,
                &codegen_cmd::driver_diag_options(&diag_opts),
                stderr.clone(),
                Some(&program),
            );
            Diag::new(DiagId::drv_eval_session_tu_errors, Span::INVALID)
                .arg(errors.to_string())
                .emit(&drv);
            drv.flush();
            let diagnostics = take();
            // The text the driver printed for them, which outlives `sm`
            // (WP-2332).
            let rendered = kiln_diag::render_diagnostics(&sm, &diag_opts, &diagnostics);
            // The same records as data (WP-2827).
            let records = kiln_diag::diag_records(&sm, &diag_opts, &diagnostics);
            // The error limit's fatal error stops a unit that has errors;
            // any other stops the front end.
            let stopped = diagnostics
                .iter()
                .any(|d| d.level == Level::Fatal && d.id != Some(DiagId::diag_too_many_errors));
            let refused = if stopped {
                let stack_exhausted = stack_phase(&diagnostics);
                SessionRefused::Stopped { diagnostics, rendered, records, stack_exhausted }
            } else {
                SessionRefused::TuHasErrors { errors, diagnostics, rendered, records }
            };
            f.take().map(|f| f(Err(refused)))
        }
    }
}
