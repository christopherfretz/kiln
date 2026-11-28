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
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use kiln_ast::Ast;
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, DiagSink, Diagnostic, Level};
use kiln_parse::{ParseOptions, ParsedTu, TokenTable};
use kiln_pp::{LookupLog, Preprocessor, SearchEvent};
use kiln_sema::Oven;
use kiln_sema::stack_guard::StackGuard;
use kiln_sema::target::Target;
use kiln_source::symbol::Interner;
use kiln_source::{FileId, Overlay, SourceManager, Span, WorkDir};

use crate::args::{Action, Args, ModuleHeaderJob};
use crate::env::Environment;
use crate::pp_cmd::Job;
use crate::syntax_cmd::SyntaxJob;
use crate::toolchain::LangStd;
use crate::{Engines, EntryRules, SharedWriter};

/// How one run behaves. `Default` is: diagnostics discarded, no lookup
/// log, a thread of its own, the PCH probe on, the process environment,
/// no preprocessing record, no written-reference table, no static-initialization
/// facts, no builtin-macro
/// literal table, no token table, the process's working directory (read
/// once), no overlay.
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
    /// Expand `@file` response files in the command line, as the `kiln`
    /// executable does (spec/driver.md "Response files"). `false`: the
    /// words are taken as given, an `@x` word is an ordinary word (an
    /// input named `@x`), so every option that reaches the driver is one
    /// of the words the caller passed (WP-2411). A caller that reads the
    /// files itself (from an edited tree, or to learn which it read)
    /// expands them with [`crate::rsp::expand_with`] and turns this off.
    pub response_files: bool,
    /// Where the driver reads its environment variables (`KILN_*`,
    /// `PATH`, `SOURCE_DATE_EPOCH`, `HOME`, `XDG_CACHE_HOME`): the
    /// process's, or a fixed set ([`Environment::fixed`]), which the
    /// process environment then never adds to.
    pub environment: Environment,
    /// Keep the preprocessing record (`PpOptions::pp_record`, spec/pp.md
    /// "Preprocessing record"): the closure reads it as
    /// `fe.pp.record()`. Off by default (WP-2110).
    pub pp_record: bool,
    /// Record the written-reference table (`ParseOptions::written_refs`,
    /// spec/ast.md §18), as `-fkiln-written-refs` on the command line
    /// does, and print nothing: the closure reads it as
    /// `fe.ast.written_refs`, with the content `-fkiln-dump-written-refs`
    /// prints for the same input. Off by default (WP-2320).
    pub written_refs: bool,
    /// Record the table's substitution keys too
    /// (`ParseOptions::written_ref_keys`, spec/ast.md §18.14), as
    /// `-fkiln-written-ref-keys` on the command line does; implies
    /// `written_refs`. Off by default (WP-3540).
    pub written_ref_keys: bool,
    /// Record each static variable's static-initialization fact
    /// (`ParseOptions::static_init_facts`, spec/ast.md §25), as
    /// `-fkiln-static-init-facts` on the command line does: the closure
    /// reads it as `fe.ast.static_init_fact(d)`. Off by default
    /// (WP-3650).
    pub static_init_facts: bool,
    /// Record the function-name span table (`ParseOptions::fn_names`,
    /// spec/ast.md §24), as `-fkiln-fn-names` on the command line does,
    /// and print nothing: the closure reads it as `fe.ast.fn_names`
    /// (WP-3280). Off by default.
    pub fn_names: bool,
    /// Record which builtin macro each literal came from (the
    /// builtin-macro literal table, spec/ast.md §22), as
    /// `-fkiln-builtin-literals` on the command line does: the closure
    /// reads it as `fe.ast.literal_builtin(e)`. Off by default (WP-2821).
    pub builtin_literals: bool,
    /// Record the token table (`ParseOptions::token_table`, spec/parse.md
    /// §2.6): every token the preprocessor delivered to the parser, a
    /// split `>>` as its two `>`s, and the query "does a token start
    /// here" per file. The closure reads it as `fe.token_table`. No
    /// command-line flag sets it. Off by default (WP-2825).
    pub token_table: bool,
    /// Keep the shared bytes each [`Consulted`] entry read
    /// (`Consulted::bytes`, spec/driver.md "Front-end library entry",
    /// WP-3692), without copying or re-reading the file. Off by default:
    /// when off, every `Consulted::bytes` is `None` and no extra
    /// allocation or copy is made.
    pub consulted_bytes: bool,
    /// The directory every relative path of the run opens against (the
    /// main input, `-I` and the other search directories, `-include`,
    /// `#embed`, module files, `@file`s), as clang's `-working-directory`
    /// (spec/driver.md "Working directory"): a value of the run, the
    /// process's current directory is never changed, so runs in different
    /// directories may run at the same time. `None`: the process's current
    /// directory, read once when the run starts. A relative directory is
    /// relative to the process's. A `-working-directory` of the command
    /// line wins, a relative one relative to this one. Names as written
    /// (`__FILE__`, diagnostics, `#line`) stay relative.
    pub working_directory: Option<PathBuf>,
    /// Replacement contents for files of the run (spec/source.md
    /// "Overlay", WP-2390): every read of an overlaid file (the main
    /// input, `#include` by any spelling, `-include`, `#embed`) gets the
    /// overlay's bytes, the run seeing what the same command sees over a
    /// tree in which those files hold them. The source manager reports
    /// such a buffer as `FileOrigin::Overlay`. A run whose overlay
    /// replaces an input of a module file the command line names is
    /// refused (`drv_front_end_overlay_input`). `None` (or an empty
    /// overlay): the disk.
    pub overlay: Option<Arc<Overlay>>,
}

impl Default for FrontEndOptions {
    fn default() -> FrontEndOptions {
        FrontEndOptions {
            stderr: None,
            record_lookups: false,
            own_thread: true,
            pch_probe: true,
            response_files: true,
            environment: Environment::process(),
            pp_record: false,
            written_refs: false,
            written_ref_keys: false,
            static_init_facts: false,
            fn_names: false,
            builtin_literals: false,
            token_table: false,
            consulted_bytes: false,
            working_directory: None,
            overlay: None,
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
            .field("response_files", &self.response_files)
            .field("environment", &self.environment)
            .field("pp_record", &self.pp_record)
            .field("written_refs", &self.written_refs)
            .field("written_ref_keys", &self.written_ref_keys)
            .field("static_init_facts", &self.static_init_facts)
            .field("fn_names", &self.fn_names)
            .field("builtin_literals", &self.builtin_literals)
            .field("token_table", &self.token_table)
            .field("consulted_bytes", &self.consulted_bytes)
            .field("working_directory", &self.working_directory)
            .field("overlay", &self.overlay)
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
    /// The token table, when `FrontEndOptions::token_table` was set
    /// ([`ParsedTu::token_table`], spec/parse.md §2.6): the tokens of
    /// this unit, not those of a PCH prefix or an imported module file.
    pub token_table: Option<&'r TokenTable>,
    /// Paths the result depends on: every `#embed` read (even when the
    /// preprocessor also entered the file) and every `__has_include` /
    /// `__has_embed` hit of a file the preprocessor did not enter, in the
    /// order met (reads of the same path with identical content merge into
    /// one, upgrading a probe to an embed; a differing content gets its
    /// own entry).
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
            .field("token_table", &self.token_table.map(TokenTable::len))
            .field("consulted", &self.consulted)
            .field("lookups", &self.lookups.is_some())
            .finish_non_exhaustive()
    }
}

/// A path the unit consulted: an `#embed` read, or a `__has_include` /
/// `__has_embed` hit of a file the preprocessor did not enter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Consulted {
    /// How it was consulted.
    pub kind: ConsultedKind,
    /// The path as header search found it.
    pub path: String,
    /// What the front end read there; `None` when it read nothing (a
    /// `__has_include` hit whose buffer is not loaded).
    pub content: Option<Content>,
    /// The shared bytes of this read when
    /// [`FrontEndOptions::consulted_bytes`] was set (`None` when off, or
    /// when `content` is `None`).
    pub bytes: Option<Arc<[u8]>>,
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
    /// The entries of `FrontEndOptions::overlay` whose loader
    /// ([`Overlay::replace_with`], [`Overlay::create_with`]) failed when
    /// this run read them, each with the loader's error text, in the
    /// order read. A file whose loader failed is not found in the run, so
    /// a run with any is void: its diagnostics are not the tree's.
    pub overlay_failures: Vec<(PathBuf, String)>,
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
    /// An `@file` of the command line exists but cannot be read, or names
    /// itself (`FrontEndOptions::response_files`; the driver prints this
    /// text and exits with 1, as clang's).
    ResponseFile(crate::rsp::RspError),
}

impl std::fmt::Display for FrontEndError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrontEndError::Panicked(m) => write!(f, "the front end panicked: {m}"),
            FrontEndError::Thread(e) => write!(f, "cannot create the front-end thread: {e}"),
            FrontEndError::NotOneInput => f.write_str("the command line has not one source input"),
            FrontEndError::NotSyntaxOnly => f.write_str("the command line is not -fsyntax-only"),
            FrontEndError::ResponseFile(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for FrontEndError {}

/// Parses the one input of `command_line` (the arguments after the
/// program name, `@file`s expanded unless `options.response_files` is
/// off) as `kiln -fsyntax-only` would and
/// calls `f` when the AST is final. `argv0` picks the driver mode as the
/// executable's name does (`kiln++` is C++).
///
/// # Errors
///
/// [`FrontEndError::NotSyntaxOnly`] / [`FrontEndError::NotOneInput`] for
/// a command line that is not one `-fsyntax-only` job;
/// [`FrontEndError::ResponseFile`] for an `@file` that cannot be read;
/// [`FrontEndError::Panicked`] when the front end or `f` panicked;
/// [`FrontEndError::Thread`] when `own_thread` could not start one. A
/// command line the driver rejects is not an error: the report says so.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the options belong to the call (the entry's signature): they own the overlay"
)]
pub fn run_front_end<R: Send>(
    argv0: &str,
    command_line: &[String],
    options: FrontEndOptions,
    f: impl FnOnce(&FrontEnd<'_, '_>) -> R + Send,
) -> Result<FrontEndReport<R>, FrontEndError> {
    let base = options.base_work_dir();
    let command_line = if options.response_files {
        crate::rsp::expand_checked(command_line, &base).map_err(FrontEndError::ResponseFile)?.words
    } else {
        command_line.to_vec()
    };
    if !options.own_thread {
        return panic::catch_unwind(AssertUnwindSafe(|| {
            run_here(argv0, &command_line, &options, &base, f)
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
                run_here(argv0, &command_line, &options, &base, f)
            })
            .map_err(FrontEndError::Thread)?;
        h.join().unwrap_or_else(|p| Err(FrontEndError::Panicked(payload_text(&*p))))
    })
}

/// A panic payload as text: a `&str` or `String` message, else a stand-in.
pub(crate) fn payload_text(p: &(dyn std::any::Any + Send)) -> String {
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
    /// `FrontEndOptions::consulted_bytes`.
    pub(crate) consulted_bytes: bool,
    pub(crate) events: SearchEvents,
    /// `FrontEndOptions::overlay`, never an empty one.
    pub(crate) overlay: Option<Arc<Overlay>>,
    /// Set by the job: the overlay's entries whose loader failed when the
    /// run read them (`FrontEndReport::overlay_failures`).
    pub(crate) overlay_failures: Vec<(PathBuf, String)>,
}

/// One of header search's events as the entry keeps it: the kind, the
/// path, the file `__has_include` found, the content an `#embed` or
/// `__has_embed` read, and (when `consulted_bytes` is on) its shared bytes.
pub(crate) struct RecordedSearchEvent {
    kind: ConsultedKind,
    path: String,
    file: Option<FileId>,
    content: Option<Content>,
    bytes: Option<Arc<[u8]>>,
}

/// Header search's recorded events for one run.
pub(crate) type SearchEvents = Rc<RefCell<Vec<RecordedSearchEvent>>>;

/// Keeps one search event (the observer of
/// [`crate::pp_cmd::Job::configure_preprocessor_with`]).
pub(crate) fn on_search_event(events: &SearchEvents, keep_bytes: bool, e: SearchEvent<'_>) {
    let ev = match e {
        SearchEvent::HasInclude { file, path, .. } => RecordedSearchEvent {
            kind: ConsultedKind::Probe,
            path: path.to_owned(),
            file: Some(file),
            content: None,
            bytes: None,
        },
        SearchEvent::Embed { path, query_only, bytes } => RecordedSearchEvent {
            kind: if query_only { ConsultedKind::Probe } else { ConsultedKind::Embed },
            path: path.to_owned(),
            file: None,
            content: Some(Content::of(bytes)),
            bytes: keep_bytes.then(|| Arc::clone(bytes)),
        },
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

/// The entry's hooks in the job: the preprocessor's, the parser's
/// token table, and the closure.
pub(crate) struct Hooks<'h> {
    pub(crate) pp: PpHooks,
    /// `FrontEndOptions::token_table`.
    pub(crate) token_table: bool,
    see: &'h mut dyn FnMut(&FrontEnd<'_, '_>),
}

impl Hooks<'_> {
    /// Calls the closure on `unit` and `pp`, the lookup log taken from
    /// `pp` when one was asked for.
    pub(crate) fn see<'sm>(&mut self, unit: &Unit<'_, 'sm>, pp: &mut Preprocessor<'sm, '_>) {
        let lookups = if self.pp.record_lookups { pp.take_lookup_log() } else { None };
        let consulted = consulted(unit.sm, pp, &self.pp.events.borrow(), self.pp.consulted_bytes);
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
            token_table: unit.tu.token_table.as_ref(),
            consulted: &consulted,
            lookups: lookups.as_ref(),
        };
        (self.see)(&fe);
    }
}

/// The consulted paths of `events` (WP-3692): every `#embed` read (even if
/// the preprocessor also entered the file) and every `__has_include` /
/// `__has_embed` hit of a file the preprocessor did not enter, in the order
/// met. Reads of the same path with identical content merge into one
/// (keeping first-met order and upgrading a probe to an embed); a read with
/// differing content gets its own entry. When `keep_bytes` is `true`, each
/// entry also carries the shared bytes of that read (`None` when `false`,
/// without allocating or copying).
pub(crate) fn consulted(
    sm: &SourceManager,
    pp: &Preprocessor<'_, '_>,
    events: &[RecordedSearchEvent],
    keep_bytes: bool,
) -> Vec<Consulted> {
    let mut out: Vec<Consulted> = Vec::new();
    for ev in events {
        if ev.kind == ConsultedKind::Probe {
            let entered = ev
                .file
                .or_else(|| sm.loaded_file(Path::new(&ev.path)))
                .is_some_and(|f| pp.was_entered(f));
            if entered {
                continue;
            }
        }
        let (content, bytes) = match ev.content {
            Some(c) => (Some(c), if keep_bytes { ev.bytes.clone() } else { None }),
            None => {
                if keep_bytes {
                    let b = ev.file.and_then(|f| sm.shared_original_bytes(f));
                    let c = b.as_deref().map(Content::of);
                    (c, b)
                } else {
                    let c = ev.file.and_then(|f| sm.original_bytes(f)).map(|b| Content::of(&b));
                    (c, None)
                }
            }
        };
        let existing = out.iter_mut().find(|c| {
            c.path == ev.path && (c.content == content || c.content.is_none() || content.is_none())
        });
        match existing {
            Some(c) => {
                if ev.kind == ConsultedKind::Embed {
                    c.kind = ConsultedKind::Embed;
                }
                if c.content.is_none() {
                    c.content = content;
                }
                if c.bytes.is_none() {
                    c.bytes = bytes;
                }
            }
            None => out.push(Consulted { kind: ev.kind, path: ev.path.clone(), content, bytes }),
        }
    }
    // A probe of a path entered after it was probed stays dropped; an
    // `#embed` is a separate read and stays reported.
    out.retain(|c| {
        c.kind == ConsultedKind::Embed
            || !sm.loaded_file(Path::new(&c.path)).is_some_and(|f| pp.was_entered(f))
    });
    out
}

/// The entry's [`Engines`]: every engine's sink is a [`Tee`] (the text or
/// JSON sink over `options.stderr`, if any, and the first error), and
/// each engine's totals are added up when it is done.
pub(crate) struct Host {
    out: Option<SharedWriter>,
    first: Arc<Mutex<Option<FirstError>>>,
    errors: Cell<u32>,
    warnings: Cell<u32>,
    fatal: Cell<bool>,
}

impl Host {
    pub(crate) fn new(out: Option<SharedWriter>) -> Host {
        Host {
            out,
            first: Arc::new(Mutex::new(None)),
            errors: Cell::new(0),
            warnings: Cell::new(0),
            fatal: Cell::new(false),
        }
    }

    pub(crate) fn first_error(&self) -> Option<FirstError> {
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
        // As `crate::make_engine`.
        let inner = self.out.as_ref().map(|w| kiln_diag::format_sink(w.clone(), opts, prefix));
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

impl FrontEndOptions {
    /// The run's working directory before the command line's
    /// `-working-directory`: [`FrontEndOptions::working_directory`], else
    /// the process's current directory, read now. Read once per run.
    fn base_work_dir(&self) -> WorkDir {
        match &self.working_directory {
            Some(d) => WorkDir::at(d).unwrap_or_else(|_| WorkDir::of_process()),
            None => WorkDir::of_process(),
        }
    }
}

/// The arguments of a library entry's command line: the driver's parse,
/// in `env`, writing nothing outside the call, its relative paths
/// resolved against `work_dir` (then the command line's
/// `-working-directory`).
pub(crate) fn entry_args(
    argv0: &str,
    command_line: &[String],
    env: &Environment,
    work_dir: &WorkDir,
) -> Args {
    let mut args = Args::parse_as(argv0, command_line);
    args.env = env.clone();
    args.no_side_outputs = true;
    args.resolve_work_dir(work_dir);
    args
}

/// [`run_front_end`] on the current thread, panics not caught.
fn run_here<R>(
    argv0: &str,
    command_line: &[String],
    options: &FrontEndOptions,
    base: &WorkDir,
    f: impl FnOnce(&FrontEnd<'_, '_>) -> R,
) -> Result<FrontEndReport<R>, FrontEndError> {
    let mut args = entry_args(argv0, command_line, &options.environment, base);
    // The command line's `-fkiln-written-refs`: the parser's option, and
    // the module cache's key of a module built in process (a module file
    // carries the table its producer recorded, spec/ast.md §18.10.2).
    args.written_refs |= options.written_refs || options.written_ref_keys;
    args.written_ref_keys |= options.written_ref_keys;
    args.static_init_facts |= options.static_init_facts;
    args.fn_names |= options.fn_names;
    args.builtin_literals |= options.builtin_literals;
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
    let mut overlay_failures = Vec::new();
    let overlay = options.overlay.as_ref().filter(|o| !o.is_empty());
    prepare(&args, rules, overlay.map(|o| &**o), &host, &stderr, |job| {
        driver_errors = Some(host.errors.get());
        let events = SearchEvents::default();
        let pp = PpHooks {
            record_lookups: options.record_lookups,
            pp_record: options.pp_record,
            consulted_bytes: options.consulted_bytes,
            events,
            overlay: overlay.cloned(),
            overlay_failures: Vec::new(),
        };
        let mut hooks = Hooks { pp, token_table: options.token_table, see: &mut see };
        SyntaxJob(job).run_hooked(&host, &mut hooks);
        overlay_failures = std::mem::take(&mut hooks.pp.overlay_failures);
    });
    Ok(FrontEndReport {
        value,
        errors: host.errors.get(),
        warnings: host.warnings.get(),
        fatal: host.fatal.get(),
        driver_errors: driver_errors.unwrap_or_else(|| host.errors.get()),
        first_error: host.first_error(),
        overlay_failures,
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
/// refused with the driver's checks, as is an `overlay` that replaces an
/// input of a module file of the command line; `-include-pch` with the
/// PCH probe.
pub(crate) fn prepare<R>(
    args: &Args,
    rules: EntryRules,
    overlay: Option<&Overlay>,
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
    let inputs = crate::existing_inputs(args, overlay, &drv);
    let refuse = |drv: &DiagEngine<'_>| {
        if args.verify.is_some() {
            Diag::new(DiagId::drv_front_end_unsupported, Span::INVALID).arg("-verify").emit(drv);
        }
        if let Some(o) = overlay {
            overlaid_module_inputs(args, o, drv);
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

/// Refuses an `overlay` that replaces or removes an input of a module
/// file of the command line (spec/driver.md "Front-end library entry",
/// WP-2390, WP-2391): `-fmodule-file=` (named or not) and a module-file
/// main input. A module file holds its inputs' text as it was built (and
/// checks them by size and hash against the disk), which the overlay's
/// bytes are not: one `drv_front_end_overlay_input` per overlaid input.
/// It records its inputs but not its lookups, so an overlay that creates
/// any file (which could shadow an input or answer a `__has_include`)
/// refuses every one: `drv_front_end_overlay_creates`, naming the first
/// created path. A module file that cannot be read is left to the job,
/// which diagnoses it.
fn overlaid_module_inputs(args: &Args, overlay: &Overlay, drv: &DiagEngine<'_>) {
    use kiln_pp::bmi;
    let mains = args
        .inputs
        .iter()
        .map(|i| i.path.as_str())
        .filter(|p| bmi::sniff(&args.fs_path(p)) == bmi::Sniffed::Kiln);
    let named = args.module_files.iter().map(|(_, p)| p.as_str());
    let nameless = args.nameless_module_files.iter().map(String::as_str);
    for path in mains.chain(named).chain(nameless) {
        let Ok(c) = bmi::read_file(&args.fs_path(path)) else { continue };
        if let Some(created) = overlay.created_paths().first() {
            Diag::new(DiagId::drv_front_end_overlay_creates, Span::INVALID)
                .arg(created.to_string_lossy())
                .arg(path)
                .emit(drv);
            continue;
        }
        for f in c.files.iter().filter(|f| overlay.contains(&args.fs_path(&f.path))) {
            Diag::new(DiagId::drv_front_end_overlay_input, Span::INVALID)
                .arg(&f.path)
                .arg(path)
                .emit(drv);
        }
    }
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
    let args = entry_args("kiln++", &cl, &options.environment, &options.base_work_dir());
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
        overlay_failures: Vec::new(),
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
