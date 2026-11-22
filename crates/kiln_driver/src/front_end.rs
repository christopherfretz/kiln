//! Kiln's front end as a library call (WP-2120 for tooling clients, KF1): one
//! translation unit parsed exactly as `kiln -fsyntax-only` parses it, and
//! a closure called with the parsed unit while everything it borrows is
//! still alive. See `spec/driver.md` "Front-end library entry".
//!
//! The command line goes through the driver's own parser, checks,
//! toolchain, PCH probe and job ([`crate::syntax_cmd::SyntaxJob`]); the
//! entry adds only what a library needs: no file written (no `-o`, no
//! dependency file, no module file, no statistics log, no
//! `-fconstexpr-report=`), diagnostics counted and the first error kept,
//! and a panic returned as [`FrontEndError::Panicked`].
use std::cell::{Cell, RefCell};
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use kiln_ast::Ast;
use kiln_diag::{
    Diag, DiagEngine, DiagId, DiagOptions, DiagSink, Diagnostic, Format, JsonSink, Level, TextSink,
};
use kiln_parse::{ParseOptions, ParsedTu};
use kiln_pp::{LookupLog, Preprocessor, SearchEvent};
use kiln_sema::Oven;
use kiln_sema::stack_guard::StackGuard;
use kiln_sema::target::Target;
use kiln_source::symbol::Interner;
use kiln_source::{FileId, SourceManager, Span};

use crate::args::{Action, Args, ModuleHeaderJob};
use crate::env::Environment;
use crate::pp_cmd::Job;
use crate::syntax_cmd::SyntaxJob;
use crate::toolchain::LangStd;
use crate::{Engines, EntryRules, SharedWriter};

/// How one run behaves. `Default` is: diagnostics discarded, no lookup
/// log, a thread of its own, the PCH probe on, the process environment,
/// no preprocessing record.
#[non_exhaustive]
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct FrontEndOptions {
    /// Diagnostics are also rendered here, as text or JSON as the command
    /// line says, followed by the summary line as `kiln` prints it.
    /// `None` renders nothing.
    pub stderr: Option<SharedWriter>,
    /// Record the header lookups ([`LookupLog`]).
    pub record_lookups: bool,
    /// Run on a new thread whose stack is the command line's
    /// `-fkiln-stack-size=` (else Kiln's default), with the stack guard
    /// installed, as the `kiln` executable does. `false`: run on the
    /// calling thread, which has done both.
    pub own_thread: bool,
    /// clang's PCH probe: `-include <h>` with `<h>.pch` (or `.gch`) next
    /// to it becomes `-include-pch`, which the entry refuses. `false`:
    /// no probe, the header is included as written.
    pub pch_probe: bool,
    /// Where the driver reads its environment variables (`KILN_*`,
    /// `PATH`, `SOURCE_DATE_EPOCH`, `HOME`, `XDG_CACHE_HOME`): the
    /// process's, or a fixed set ([`Environment::fixed`]), which the
    /// process environment then never adds to.
    pub environment: Environment,
    /// Keep the preprocessing record (`PpOptions::pp_record`, spec/pp.md
    /// "Preprocessing record"): the closure reads it as
    /// `fe.pp.record()`. Off by default (WP-2110).
    pub pp_record: bool,
}

impl Default for FrontEndOptions {
    fn default() -> FrontEndOptions {
        FrontEndOptions {
            stderr: None,
            record_lookups: false,
            own_thread: true,
            pch_probe: true,
            environment: Environment::process(),
            pp_record: false,
        }
    }
}

impl std::fmt::Debug for FrontEndOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrontEndOptions")
            .field("stderr", &self.stderr.is_some())
            .field("record_lookups", &self.record_lookups)
            .field("own_thread", &self.own_thread)
            .field("pch_probe", &self.pch_probe)
            .field("environment", &self.environment)
            .field("pp_record", &self.pp_record)
            .finish()
    }
}

/// What the closure sees: the state of one translation unit when its AST
/// is final. References only, valid inside the call.
#[non_exhaustive]
pub struct FrontEnd<'r, 'sm> {
    /// The unit's source manager: every file read, every macro expansion.
    pub sm: &'sm SourceManager,
    /// The interner of the unit's identifiers.
    pub interner: &'r Arc<Interner>,
    /// The job's diagnostics engine (what it reported so far; more may be
    /// emitted through it).
    pub diags: &'r DiagEngine<'sm>,
    /// The AST.
    pub ast: &'r Ast,
    /// The constant evaluator's state.
    pub oven: &'r Oven,
    /// The preprocessor at the end of the unit (its macro table, the
    /// files it entered).
    pub pp: &'r Preprocessor<'sm, 'r>,
    /// The target.
    pub target: &'r Target,
    /// The language standard of the input.
    pub lang: &'r LangStd,
    /// The parser's options, as the job set them.
    pub parse_options: &'r ParseOptions,
    /// The main file, once entered.
    pub main_file: Option<FileId>,
    /// Tokens the preprocessor delivered to the parser, the end of file
    /// not counted ([`ParsedTu::tokens`]: after a fatal error, the ones
    /// before it plus the one the preprocessor returned with it).
    pub tokens: u64,
    /// Paths the result depends on that are not files the preprocessor
    /// entered, in the order first met, each once.
    pub consulted: &'r [Consulted],
    /// The lookup log, when `record_lookups` was set.
    pub lookups: Option<&'r LookupLog>,
}

impl std::fmt::Debug for FrontEnd<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrontEnd")
            .field("files", &self.sm.file_count())
            .field("main_file", &self.main_file)
            .field("tokens", &self.tokens)
            .field("consulted", &self.consulted)
            .field("lookups", &self.lookups.is_some())
            .finish_non_exhaustive()
    }
}

/// A path the unit depends on without having entered it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Consulted {
    /// How it was consulted.
    pub kind: ConsultedKind,
    /// The path as header search found it.
    pub path: String,
    /// What the front end read there; `None` when it read nothing (a
    /// `__has_include` hit whose buffer is not loaded).
    pub content: Option<Content>,
}

/// The bytes of a [`Consulted`] path as the front end read them, by size
/// and digest, so a caller need not read the file again to know what the
/// unit saw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Content {
    /// The size in bytes.
    pub size: u64,
    /// The 128-bit FNV-1a hash of the bytes ([`content_digest`]): a
    /// change detector, not a cryptographic digest.
    pub digest: u128,
}

impl Content {
    /// The content of `bytes`.
    pub fn of(bytes: &[u8]) -> Content {
        Content {
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            digest: content_digest(bytes),
        }
    }
}

/// The 128-bit FNV-1a hash of `bytes` ([`Content::digest`]).
pub fn content_digest(bytes: &[u8]) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    bytes.iter().fold(OFFSET, |h, &b| (h ^ u128::from(b)).wrapping_mul(PRIME))
}

/// How a [`Consulted`] path was consulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConsultedKind {
    /// Found by `__has_include` or `__has_embed` and not otherwise read.
    Probe,
    /// The resource of an `#embed`.
    Embed,
}

/// The outcome of [`run_front_end`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontEndReport<R> {
    /// The closure's result. `None`: no parse happened (the driver
    /// refused the command line, or the job stopped before parsing).
    pub value: Option<R>,
    /// Errors of the driver and of the job together (fatal errors
    /// included).
    pub errors: u32,
    /// Warnings of the driver and of the job together.
    pub warnings: u32,
    /// A fatal error was reported.
    pub fatal: bool,
    /// The part of `errors` reported before any job ran.
    pub driver_errors: u32,
    /// The first diagnostic of level error or fatal, in the order emitted.
    pub first_error: Option<FirstError>,
}

/// The first error of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstError {
    /// Its id; `None` for a free-text diagnostic.
    pub id: Option<DiagId>,
    /// The message as the text sink prints it after `error: ` (with the
    /// ` [option]` suffix when the command line shows options).
    pub message: String,
    /// Presumed position as the text sink prints it; `None` for a
    /// diagnostic without a location.
    pub at: Option<Position>,
}

/// A presumed position: file and line after `#line`, the column in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Position {
    /// The presumed file name.
    pub file: String,
    /// The presumed line, from 1.
    pub line: u32,
    /// The column, from 1, in bytes.
    pub column: u32,
}

/// Why [`run_front_end`] returned no report.
#[derive(Debug)]
#[non_exhaustive]
pub enum FrontEndError {
    /// The front end or the closure panicked; the payload as text.
    Panicked(String),
    /// The thread could not be started.
    Thread(std::io::Error),
    /// The command line does not name exactly one source input.
    NotOneInput,
    /// The command line's action is not `-fsyntax-only`.
    NotSyntaxOnly,
}

impl std::fmt::Display for FrontEndError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrontEndError::Panicked(m) => write!(f, "the front end panicked: {m}"),
            FrontEndError::Thread(e) => write!(f, "cannot create the front-end thread: {e}"),
            FrontEndError::NotOneInput => f.write_str("the command line has not one source input"),
            FrontEndError::NotSyntaxOnly => f.write_str("the command line is not -fsyntax-only"),
        }
    }
}

impl std::error::Error for FrontEndError {}

/// Parses the one input of `command_line` (the arguments after the
/// program name, `@file`s expanded) as `kiln -fsyntax-only` would and
/// calls `f` when the AST is final. `argv0` picks the driver mode as the
/// executable's name does (`kiln++` is C++).
///
/// # Errors
///
/// [`FrontEndError::NotSyntaxOnly`] / [`FrontEndError::NotOneInput`] for
/// a command line that is not one `-fsyntax-only` job;
/// [`FrontEndError::Panicked`] when the front end or `f` panicked;
/// [`FrontEndError::Thread`] when `own_thread` could not start one. A
/// command line the driver rejects is not an error: the report says so.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the options belong to the call (the entry's signature), so that later ones may own an overlay"
)]
pub fn run_front_end<R: Send>(
    argv0: &str,
    command_line: &[String],
    options: FrontEndOptions,
    f: impl FnOnce(&FrontEnd<'_, '_>) -> R + Send,
) -> Result<FrontEndReport<R>, FrontEndError> {
    let command_line = crate::rsp::expand(command_line);
    if !options.own_thread {
        return panic::catch_unwind(AssertUnwindSafe(|| {
            run_here(argv0, &command_line, &options, f)
        }))
        .unwrap_or_else(|p| Err(FrontEndError::Panicked(payload_text(&*p))));
    }
    let size = crate::templ_opts::stack_size_of(&command_line);
    std::thread::scope(|s| {
        let h = std::thread::Builder::new()
            .name("kiln-front-end".into())
            .stack_size(size)
            .spawn_scoped(s, || {
                StackGuard::install(size);
                run_here(argv0, &command_line, &options, f)
            })
            .map_err(FrontEndError::Thread)?;
        h.join().unwrap_or_else(|p| Err(FrontEndError::Panicked(payload_text(&*p))))
    })
}

/// A panic payload as text: a `&str` or `String` message, else a stand-in.
fn payload_text(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "a panic payload that is not text".to_owned()
    }
}

/// The library entry's part of [`crate::pp_cmd::Job::build_preprocessor_with`]:
/// whether to record lookups, and where header search's events go.
pub(crate) struct PpHooks {
    pub(crate) record_lookups: bool,
    /// `FrontEndOptions::pp_record`.
    pub(crate) pp_record: bool,
    pub(crate) events: SearchEvents,
}

/// One of header search's events as the entry keeps it: the kind, the
/// path, the file `__has_include` found, and the bytes an `#embed` or
/// `__has_embed` read.
pub(crate) type SearchEvents =
    Rc<RefCell<Vec<(ConsultedKind, String, Option<FileId>, Option<Content>)>>>;

/// Keeps one search event (the observer of
/// [`crate::pp_cmd::Job::configure_preprocessor_with`]).
pub(crate) fn on_search_event(events: &SearchEvents, e: SearchEvent<'_>) {
    let ev = match e {
        SearchEvent::HasInclude { file, path, .. } => {
            (ConsultedKind::Probe, path.to_owned(), Some(file), None)
        }
        SearchEvent::Embed { path, query_only, bytes } => {
            let kind = if query_only { ConsultedKind::Probe } else { ConsultedKind::Embed };
            (kind, path.to_owned(), None, Some(Content::of(bytes)))
        }
    };
    events.borrow_mut().push(ev);
}

/// What [`crate::syntax_cmd::SyntaxJob::after_parse`] hands the entry.
pub(crate) struct Unit<'u, 'sm> {
    pub(crate) sm: &'sm SourceManager,
    pub(crate) interner: &'u Arc<Interner>,
    pub(crate) engine: &'u DiagEngine<'sm>,
    pub(crate) tu: &'u ParsedTu,
    pub(crate) target: &'u Target,
    pub(crate) lang: &'u LangStd,
    pub(crate) popts: &'u ParseOptions,
    pub(crate) main: Option<FileId>,
}

/// The entry's hooks in the job: the preprocessor's, and the closure.
pub(crate) struct Hooks<'h> {
    pub(crate) pp: PpHooks,
    see: &'h mut dyn FnMut(&FrontEnd<'_, '_>),
}

impl Hooks<'_> {
    /// Calls the closure on `unit` and `pp`, the lookup log taken from
    /// `pp` when one was asked for.
    pub(crate) fn see<'sm>(&mut self, unit: &Unit<'_, 'sm>, pp: &mut Preprocessor<'sm, '_>) {
        let lookups = if self.pp.record_lookups { pp.take_lookup_log() } else { None };
        let consulted = consulted(unit.sm, pp, &self.pp.events.borrow());
        let fe = FrontEnd {
            sm: unit.sm,
            interner: unit.interner,
            diags: unit.engine,
            ast: &unit.tu.ast,
            oven: &unit.tu.oven,
            pp,
            target: unit.target,
            lang: unit.lang,
            parse_options: unit.popts,
            main_file: unit.main,
            tokens: unit.tu.tokens,
            consulted: &consulted,
            lookups: lookups.as_ref(),
        };
        (self.see)(&fe);
    }
}

/// The consulted paths of `events`: in the order first met, each once
/// (an `#embed` makes an earlier probe of the same path an embed),
/// without the files the preprocessor entered. A `__has_include` hit's
/// content is its buffer's bytes when the source manager has them.
fn consulted(
    sm: &SourceManager,
    pp: &Preprocessor<'_, '_>,
    events: &[(ConsultedKind, String, Option<FileId>, Option<Content>)],
) -> Vec<Consulted> {
    let mut out: Vec<Consulted> = Vec::new();
    for (kind, path, file, content) in events {
        let entered =
            file.or_else(|| sm.loaded_file(Path::new(path))).is_some_and(|f| pp.was_entered(f));
        if entered {
            continue;
        }
        let content =
            content.or_else(|| file.and_then(|f| sm.original_bytes(f)).map(|b| Content::of(&b)));
        match out.iter_mut().find(|c| &c.path == path) {
            Some(c) => {
                if *kind == ConsultedKind::Embed {
                    c.kind = ConsultedKind::Embed;
                }
                c.content = c.content.or(content);
            }
            None => out.push(Consulted { kind: *kind, path: path.clone(), content }),
        }
    }
    // A path entered after it was first probed is not consulted either.
    out.retain(|c| !sm.loaded_file(Path::new(&c.path)).is_some_and(|f| pp.was_entered(f)));
    out
}

/// The entry's [`Engines`]: every engine's sink is a [`Tee`] (the text or
/// JSON sink over `options.stderr`, if any, and the first error), and
/// each engine's totals are added up when it is done.
struct Host {
    out: Option<SharedWriter>,
    first: Arc<Mutex<Option<FirstError>>>,
    errors: Cell<u32>,
    warnings: Cell<u32>,
    fatal: Cell<bool>,
}

impl Host {
    fn new(out: Option<SharedWriter>) -> Host {
        Host {
            out,
            first: Arc::new(Mutex::new(None)),
            errors: Cell::new(0),
            warnings: Cell::new(0),
            fatal: Cell::new(false),
        }
    }

    fn first_error(&self) -> Option<FirstError> {
        self.first.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl Engines for Host {
    fn make<'s>(
        &self,
        sm: &'s SourceManager,
        opts: &DiagOptions,
        prefix: Option<&str>,
    ) -> DiagEngine<'s> {
        // As `crate::make_engine`: JSON has no prefix.
        let inner: Option<Box<dyn DiagSink + Send>> = self.out.as_ref().map(|w| {
            let w = w.clone();
            match (opts.format, prefix) {
                (Format::Json, _) => Box::new(JsonSink::new(w, opts)) as Box<dyn DiagSink + Send>,
                (Format::Text, None) => Box::new(TextSink::new(w, opts)),
                (Format::Text, Some(p)) => Box::new(TextSink::new(w, opts).with_prefix(p)),
            }
        });
        let tee = Tee { inner, first: Arc::clone(&self.first), show_option: opts.show_option };
        DiagEngine::new(sm, opts, tee)
    }

    fn done(&self, e: &DiagEngine<'_>) {
        self.errors.set(self.errors.get() + e.error_count());
        self.warnings.set(self.warnings.get() + e.warning_count());
        self.fatal.set(self.fatal.get() || e.has_fatal());
    }
}

/// A sink that forwards to `inner` and keeps the first error.
struct Tee {
    inner: Option<Box<dyn DiagSink + Send>>,
    first: Arc<Mutex<Option<FirstError>>>,
    show_option: bool,
}

impl DiagSink for Tee {
    fn emit(&mut self, sm: &SourceManager, d: &Diagnostic) {
        if matches!(d.level, Level::Error | Level::Fatal) {
            let mut first = self.first.lock().unwrap_or_else(PoisonError::into_inner);
            if first.is_none() {
                let mut message = d.message.clone();
                if self.show_option
                    && let Some(o) = &d.option
                {
                    message.push_str(" [");
                    message.push_str(o);
                    message.push(']');
                }
                let at = kiln_diag::text_position(sm, d).map(|(file, line, column)| Position {
                    file: file.to_owned(),
                    line,
                    column,
                });
                *first = Some(FirstError { id: d.id, message, at });
            }
        }
        if let Some(s) = &mut self.inner {
            s.emit(sm, d);
        }
    }

    fn flush(&mut self) {
        if let Some(s) = &mut self.inner {
            s.flush();
        }
    }
}

/// The command line is one `-fsyntax-only` job on one source input.
fn structural_checks(args: &Args) -> Result<(), FrontEndError> {
    if args.action != Action::SyntaxOnly
        || args.help
        || args.probe.any()
        || args.print_resource_dir
        || args.print_commands
        || args.deps.only.is_some()
    {
        return Err(FrontEndError::NotSyntaxOnly);
    }
    let [input] = args.inputs.as_slice() else { return Err(FrontEndError::NotOneInput) };
    if input.path == "-" || input.asm.is_some() {
        return Err(FrontEndError::NotOneInput);
    }
    if args.module_header.is_some()
        && !matches!(args.module_header_job(input), ModuleHeaderJob::Build)
    {
        return Err(FrontEndError::NotSyntaxOnly);
    }
    Ok(())
}

/// The arguments of a library entry's command line: the driver's parse,
/// in `env`, writing nothing outside the call.
pub(crate) fn entry_args(argv0: &str, command_line: &[String], env: &Environment) -> Args {
    let mut args = Args::parse_as(argv0, command_line);
    args.env = env.clone();
    args.no_side_outputs = true;
    args
}

/// [`run_front_end`] on the current thread, panics not caught.
fn run_here<R>(
    argv0: &str,
    command_line: &[String],
    options: &FrontEndOptions,
    f: impl FnOnce(&FrontEnd<'_, '_>) -> R,
) -> Result<FrontEndReport<R>, FrontEndError> {
    let args = entry_args(argv0, command_line, &options.environment);
    structural_checks(&args)?;
    crate::crash_for_testing(&args);
    let host = Host::new(options.stderr.clone());
    let mut f = Some(f);
    let mut value = None;
    let mut see = |fe: &FrontEnd<'_, '_>| {
        if let Some(f) = f.take() {
            value = Some(f(fe));
        }
    };
    let stderr = options.stderr.clone().unwrap_or_else(|| SharedWriter::new(std::io::sink()));
    let rules = EntryRules { pch_probe: options.pch_probe };
    let mut driver_errors = None;
    prepare(&args, rules, &host, &stderr, |job| {
        driver_errors = Some(host.errors.get());
        let events = SearchEvents::default();
        let pp = PpHooks {
            record_lookups: options.record_lookups,
            pp_record: options.pp_record,
            events,
        };
        let mut hooks = Hooks { pp, see: &mut see };
        SyntaxJob(job).run_hooked(&host, &mut hooks);
    });
    Ok(FrontEndReport {
        value,
        errors: host.errors.get(),
        warnings: host.warnings.get(),
        fatal: host.fatal.get(),
        driver_errors: driver_errors.unwrap_or_else(|| host.errors.get()),
        first_error: host.first_error(),
    })
}

/// The driver's part of `kiln -fsyntax-only` for the first input of a
/// library entry's command line (spec/driver.md "Front-end library
/// entry"), in the stages [`crate::run_as`] and its jobs share with it:
/// the command line's and the toolchain's checks, the inputs' checks and
/// the driver's warnings ([`crate::input_stage`]), the PCH probe and the
/// host-call check ([`crate::jobs_stage`]), the input's standard
/// ([`crate::job_std`]) and a header unit's arguments; then `run` with
/// its job, whose reports go to `stderr`. `None` when the driver stopped
/// before the job (every error reported through `engines`). `-verify` is
/// refused with the driver's checks, `-include-pch` with the PCH probe.
pub(crate) fn prepare<R>(
    args: &Args,
    rules: EntryRules,
    engines: &dyn Engines,
    stderr: &SharedWriter,
    run: impl FnOnce(Job<'_>) -> R,
) -> Option<R> {
    let diag_opts = crate::diag_options(args);
    let dsm = SourceManager::new();
    let drv = engines.make(&dsm, &crate::driver_options(&diag_opts), Some(&args.program));
    crate::command_line_checks(args, &drv);
    let tc = crate::toolchain_of(args, &drv);
    crate::toolchain_checks(args, &tc, &drv);
    // An explicitly named LLVM that is not usable stops everything
    // (spec/driver.md "LLVM discovery").
    if tc.llvm.as_ref().is_err_and(crate::llvm::LlvmError::is_explicit) {
        drv.flush();
        engines.done(&drv);
        return None;
    }
    if args.verbose {
        crate::print_version(&mut stderr.clone(), &tc, true);
    }
    let inputs = crate::existing_inputs(args, &drv);
    let refuse = |drv: &DiagEngine<'_>| {
        if args.verify.is_some() {
            Diag::new(DiagId::drv_front_end_unsupported, Span::INVALID).arg("-verify").emit(drv);
        }
    };
    if !crate::input_stage(args, &tc, &inputs, &diag_opts, drv, engines, &refuse) {
        return None;
    }
    let job_args =
        crate::jobs_stage(args, &tc, &inputs, &diag_opts, engines, stderr, Some(rules)).ok()?;
    let args: &Args = job_args.as_ref().unwrap_or(args);
    let input = *inputs.first()?;
    let std = crate::job_std(args, input, &diag_opts, engines)?;
    let unit_args = crate::header_unit_args(args, input);
    let job = Job {
        args: unit_args.as_ref().unwrap_or(args),
        toolchain: &tc,
        std,
        path: &input.path,
        stdin_text: None,
        diag_opts: &diag_opts,
        stderr: stderr.clone(),
        asm: false,
    };
    Some(run(job))
}

/// The configuration the driver builds for a `-fsyntax-only` job, for a
/// caller that runs a front end of its own (an in-memory one) exactly as
/// the driver configures it ([`std_config`]).
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct JobConfig {
    /// The language standard.
    pub lang: LangStd,
    /// The target, with the driver's adjustments (`-f[no-]signed-char`,
    /// `-fnew-alignment=`, 128-bit atomics).
    pub target: Target,
    /// The parser's options ([`kiln_parse::parse_preprocessed`]).
    pub parse_options: ParseOptions,
    /// The preprocessor's options, the predefines buffer included, as for
    /// a preprocessor that runs sema. `modules` is `None`: the driver's
    /// module options name module files, directories and the module cache
    /// of a compile, which an in-memory front end sets up itself.
    pub pp_options: kiln_pp::PpOptions,
    /// The `#if` evaluator ([`kiln_pp::Preprocessor::set_if_evaluator`]).
    pub if_eval: kiln_pp::IfEval,
    /// The feature queries, with kiln's staging of `__has_builtin`
    /// ([`kiln_pp::Preprocessor::set_feature_queries`]).
    pub features: crate::pp_cmd::StagedFeatures,
    /// The header search list (`-I`, the toolchain's directories, the
    /// standard library's and the resource directory's).
    pub search: kiln_pp::SearchList,
}

/// The configuration of `kiln++ -fsyntax-only -std=<std>` on a C++ source
/// with no other flag ([`JobConfig`]): the parser's options, the
/// preprocessor's options and its configuration as the driver builds
/// them, for the host target and the toolchain `options.environment`
/// locates. No file is read but the toolchain's (the predefines). The
/// report's value is `None` when the driver rejected `<std>` or the
/// toolchain (the report says why); `options.own_thread` and
/// `options.record_lookups` do not apply.
pub fn std_config(std: &str, options: &FrontEndOptions) -> FrontEndReport<JobConfig> {
    let cl = ["-fsyntax-only".to_owned(), format!("-std={std}"), "std-config.cpp".to_owned()];
    let args = entry_args("kiln++", &cl, &options.environment);
    let host = Host::new(options.stderr.clone());
    let diag_opts = crate::diag_options(&args);
    let dsm = SourceManager::new();
    let drv = host.make(&dsm, &crate::driver_options(&diag_opts), Some(&args.program));
    crate::command_line_checks(&args, &drv);
    let tc = crate::toolchain_of(&args, &drv);
    crate::toolchain_checks(&args, &tc, &drv);
    drv.flush();
    host.done(&drv);
    let driver_errors = host.errors.get();
    let mut value = None;
    if let (0, [input]) = (driver_errors, args.inputs.as_slice())
        && let Some(lang) = crate::job_std(&args, input, &diag_opts, &host)
    {
        let stderr = options.stderr.clone().unwrap_or_else(|| SharedWriter::new(std::io::sink()));
        let job = Job {
            args: &args,
            toolchain: &tc,
            std: lang,
            path: &input.path,
            stdin_text: None,
            diag_opts: &diag_opts,
            stderr,
            asm: false,
        };
        let sm = SourceManager::new();
        let engine = host.make(&sm, &diag_opts, None);
        value = config_of(job, &engine);
        engine.flush();
        host.done(&engine);
    }
    FrontEndReport {
        value,
        errors: host.errors.get(),
        warnings: host.warnings.get(),
        fatal: host.fatal.get(),
        driver_errors,
        first_error: host.first_error(),
    }
}

/// The configuration of `job` as [`crate::pp_cmd::Job::configure_preprocessor`]
/// and [`SyntaxJob::parse`] build it; `None` once an error is reported
/// to `engine`.
fn config_of(job: Job<'_>, engine: &DiagEngine<'_>) -> Option<JobConfig> {
    let predefines = job.predefines(engine)?;
    let features = job.clang_features().map(crate::pp_cmd::StagedFeatures::new);
    let Some(features) = features else {
        Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
            .arg(job.toolchain.target.triple)
            .emit(engine);
        return None;
    };
    let pp_options = job.pp_options_for(predefines, true);
    let if_eval = job.if_evaluator();
    let search = job.search_list();
    let lang = job.std;
    let sj = SyntaxJob(job);
    let target = sj.target(engine)?;
    let parse_options = sj.parse_options();
    Some(JobConfig { lang, target, parse_options, pp_options, if_eval, features, search })
}
