//! The `kiln` driver: a clang-compatible command line over the frontend.
//! See `spec/driver.md` and `docs/ARCHITECTURE.md` ("Driver").
//!
//! Only preprocessing (`-E`, `-M`, `-MM`) is implemented; `-fsyntax-only`
//! (WP-048), `-c`, `-S` and linking report "not implemented yet".
#![forbid(unsafe_code)]

pub mod args;
pub mod deps;
pub mod pp_cmd;
mod stub_features;
pub mod time;
pub mod toolchain;

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Format, JsonSink, TextSink};
use kiln_source::{SourceManager, Span};

use crate::args::{Action, ArgError, Args, Lang};
use crate::pp_cmd::{Job, input_lang};
use crate::toolchain::{LangStd, Target, Toolchain};

/// A [`Write`]r shared between the driver and its diagnostic engines.
#[derive(Clone)]
pub struct SharedWriter(Arc<Mutex<Box<dyn Write + Send>>>);

impl SharedWriter {
    /// Shares `w`.
    pub fn new(w: impl Write + Send + 'static) -> SharedWriter {
        SharedWriter(Arc::new(Mutex::new(Box::new(w))))
    }
}

impl std::fmt::Debug for SharedWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedWriter")
    }
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).flush()
    }
}

/// The driver's standard streams.
pub struct Io {
    /// Standard output.
    pub stdout: Box<dyn Write>,
    /// Standard error.
    pub stderr: SharedWriter,
    /// Standard input.
    pub stdin: Box<dyn Read>,
}

impl std::fmt::Debug for Io {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Io")
    }
}

impl Io {
    /// The process's streams.
    pub fn std() -> Io {
        Io {
            stdout: Box::new(io::stdout()),
            stderr: SharedWriter::new(io::stderr()),
            stdin: Box::new(io::stdin()),
        }
    }
}

/// `(major, minor)` of kiln's version (`__KILN_MAJOR__`, `__KILN_MINOR__`).
pub fn version_pair() -> (u32, u32) {
    let mut it = env!("CARGO_PKG_VERSION").split('.').map(|p| p.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

/// A diagnostic engine writing to `stderr` in `opts.format`, with
/// `prefix: ` before each text diagnostic (the driver's own diagnostics).
pub fn make_engine<'sm>(
    sm: &'sm SourceManager,
    opts: &DiagOptions,
    stderr: SharedWriter,
    prefix: Option<&str>,
) -> DiagEngine<'sm> {
    match (opts.format, prefix) {
        (Format::Json, _) => DiagEngine::new(sm, opts, JsonSink::new(stderr, opts)),
        (Format::Text, None) => DiagEngine::new(sm, opts, TextSink::new(stderr, opts)),
        (Format::Text, Some(p)) => {
            DiagEngine::new(sm, opts, TextSink::new(stderr, opts).with_prefix(p))
        }
    }
}

const HELP: &str = "\
OVERVIEW: kiln, a C++26 frontend (preprocessing only so far)

USAGE: kiln [options] file...

Implemented: -E -P -C -CC -dM -o -x -std= -D -U -I -isystem -iquote
-idirafter -cxx-isystem -nostdinc -nostdinc++ -nostdlibinc -nobuiltininc
-isysroot --sysroot -resource-dir -stdlib= --target= --embed-dir= -M -MM
-MD -MMD -MF -MT -MQ -MP -W... -w -pedantic -ferror-limit=
-fdiagnostics-format= -f[no-]color-diagnostics -fkiln-reproducible-time
-v --version. See spec/driver.md.
";

fn emit_arg_error(e: &ArgError, engine: &DiagEngine<'_>) {
    let d = match e {
        ArgError::Unknown(a) => Diag::new(DiagId::drv_unknown_argument, Span::INVALID).arg(a),
        ArgError::MissingValue(o) => {
            Diag::new(DiagId::drv_missing_argument, Span::INVALID).arg(o).arg(1)
        }
        ArgError::InvalidValue(_, v) => {
            Diag::new(DiagId::drv_language_not_recognized, Span::INVALID).arg(v)
        }
        ArgError::InvalidStdlib(a) => {
            Diag::new(DiagId::drv_invalid_stdlib_name, Span::INVALID).arg(a)
        }
    };
    d.emit(engine);
}

fn print_version(w: &mut dyn Write, tc: &Toolchain, verbose: bool) {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.display().to_string()))
        .unwrap_or_default();
    let _ = write!(
        w,
        "kiln version {} (C++26 frontend, LLVM IR backend)\nTarget: {}\nThread model: posix\n\
         InstalledDir: {dir}\n",
        env!("CARGO_PKG_VERSION"),
        tc.target.triple
    );
    if verbose && let Some(g) = &tc.gcc {
        let _ = write!(
            w,
            "Found candidate GCC installation: {0}\nSelected GCC installation: {0}\n\
             Candidate multilib: .;@m64\nSelected multilib: .;@m64\n",
            g.path
        );
    }
}

/// The standard of an input in `lang`, or the error to report.
fn input_std(args: &Args, lang: Lang) -> Result<LangStd, Box<Diag>> {
    let cplusplus = lang == Lang::Cxx;
    let Some(s) = &args.std else { return Ok(LangStd::default_for(cplusplus)) };
    let std = LangStd::parse(s).ok_or_else(|| {
        Box::new(
            Diag::new(DiagId::drv_invalid_value, Span::INVALID).arg(format!("-std={s}")).arg(s),
        )
    })?;
    if std.cplusplus != cplusplus {
        return Err(Box::new(
            Diag::new(DiagId::drv_argument_not_allowed_with, Span::INVALID)
                .arg(format!("-std={s}"))
                .arg(if cplusplus { "C++" } else { "C" }),
        ));
    }
    Ok(std)
}

/// Writes `text` to `path` (`-` is standard output).
fn write_output(path: Option<&str>, text: &str, stdout: &mut dyn Write) -> Result<(), String> {
    match path {
        None | Some("-") => stdout
            .write_all(text.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|e| e.to_string()),
        Some(p) => std::fs::write(p, text).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                "No such file or directory".to_owned()
            } else {
                e.to_string()
            }
        }),
    }
}

/// Reports the actions and options kiln does not implement yet.
fn check_action(args: &Args, drv: &DiagEngine<'_>) {
    let mut action = args.action;
    if action == Action::Link && args.deps.only.is_some() {
        action = Action::Preprocess;
    }
    let not_impl = match action {
        Action::Preprocess => None,
        Action::SyntaxOnly => Some("-fsyntax-only"),
        Action::Compile => Some("compiling (-c, -S, -emit-llvm)"),
        Action::Link => Some("linking (kiln only preprocesses so far: pass -E)"),
    };
    let not_impl = not_impl
        .or(args.print.defines_too.then_some("-dD"))
        .or(args.print.includes_too.then_some("-dI"));
    if let Some(what) = not_impl {
        Diag::new(DiagId::drv_not_implemented, Span::INVALID).arg(what).emit(drv);
    }
}

/// Writes the main output and the side dependency files; returns the exit
/// status.
fn write_results(
    args: &Args,
    diag_opts: &DiagOptions,
    io: &mut Io,
    output: &str,
    errors: u32,
    side_deps: Vec<(String, String)>,
) -> i32 {
    let mut status = i32::from(errors > 0);
    let out_path = if args.deps.only.is_some() {
        args.deps.file.as_deref().or(args.output.as_deref())
    } else {
        args.output.as_deref()
    };
    let to_file = out_path.is_some_and(|p| p != "-");
    if !(errors > 0 && to_file)
        && let Err(e) = write_output(out_path, output, &mut io.stdout)
    {
        report_output_error(diag_opts, &io.stderr, out_path.unwrap_or("-"), &e);
        status = 1;
    }
    if errors == 0 {
        for (f, text) in side_deps {
            if let Err(e) = write_output(Some(&f), &text, &mut io.stdout) {
                report_output_error(diag_opts, &io.stderr, &f, &e);
                status = 1;
            }
        }
    }
    status
}

/// All of standard input as text (lossily decoded).
fn read_stdin(stdin: &mut dyn Read) -> String {
    let mut bytes = Vec::new();
    match stdin.read_to_end(&mut bytes) {
        Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

/// Runs the driver on `argv` (without the program name). Returns the exit
/// code: 0 on success, 1 on any error (clang's convention).
pub fn run(command_line: &[String], mut io: Io) -> i32 {
    let args = Args::parse(command_line);
    let diag_opts = DiagOptions::from_args(&args.diag_args);
    let dsm = SourceManager::new();
    let mut drv_opts = diag_opts.clone();
    drv_opts.warnings.clear();
    drv_opts.remarks.clear();
    let drv = make_engine(&dsm, &drv_opts, io.stderr.clone(), Some("kiln"));
    for e in &args.errors {
        emit_arg_error(e, &drv);
    }
    for f in &args.frameworks {
        Diag::new(DiagId::drv_framework_dir_ignored, Span::INVALID).arg(f).emit(&drv);
    }
    let target = match &args.target {
        None => Target::host(),
        Some(t) => Target::parse(t).unwrap_or_else(|| {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID).arg(t).emit(&drv);
            Target::host()
        }),
    };
    let tc = Toolchain::new(target, args.sysroot.as_deref(), args.resource_dir.as_deref());
    if args.help {
        let _ = io.stdout.write_all(HELP.as_bytes());
        return 0;
    }
    if args.version {
        print_version(&mut io.stdout, &tc, false);
        let _ = io.stdout.flush();
    }
    if args.verbose {
        print_version(&mut io.stderr, &tc, true);
    }
    let inputs: Vec<_> = args
        .inputs
        .iter()
        .filter(|i| {
            let ok = i.path == "-" || std::path::Path::new(&i.path).exists();
            if !ok {
                Diag::new(DiagId::drv_no_such_file, Span::INVALID).arg(&i.path).emit(&drv);
            }
            ok
        })
        .collect();
    if inputs.is_empty() {
        if (args.version || args.verbose) && args.inputs.is_empty() && drv.error_count() == 0 {
            return 0;
        }
        Diag::new(DiagId::drv_no_input_files, Span::INVALID).emit(&drv);
    }
    if drv.error_count() == 0 {
        check_action(&args, &drv);
    }
    drv.flush();
    if drv.error_count() > 0 {
        return 1;
    }
    drop(drv);

    let stdin_text = inputs.iter().any(|i| i.path == "-").then(|| read_stdin(&mut io.stdin));

    let mut output = String::new();
    let mut errors = 0u32;
    let mut side_deps: Vec<(String, String)> = Vec::new();
    for input in inputs {
        let lang = input_lang(input.lang, &input.path);
        let std = match input_std(&args, lang) {
            Ok(s) => s,
            Err(d) => {
                let sm = SourceManager::new();
                let e = make_engine(&sm, &diag_opts, io.stderr.clone(), None);
                (*d).emit(&e);
                e.flush();
                errors += 1;
                continue;
            }
        };
        let job = Job {
            args: &args,
            toolchain: &tc,
            std,
            path: &input.path,
            stdin_text: if input.path == "-" { stdin_text.clone() } else { None },
            diag_opts: &diag_opts,
            stderr: io.stderr.clone(),
        };
        let r = job.run();
        errors += r.errors;
        output.push_str(&r.output);
        if let Some(files) = r.deps {
            let text = deps_text(&args, &input.path, &files);
            match dep_destination(&args, &input.path) {
                DepDest::Main => output.push_str(&text),
                DepDest::File(f) => side_deps.push((f, text)),
            }
        }
    }

    write_results(&args, &diag_opts, &mut io, &output, errors, side_deps)
}

fn report_output_error(opts: &DiagOptions, stderr: &SharedWriter, path: &str, e: &str) {
    let sm = SourceManager::new();
    let engine = make_engine(&sm, opts, stderr.clone(), None);
    Diag::new(DiagId::drv_cannot_open_output, Span::INVALID).arg(path).arg(e).emit(&engine);
    engine.flush();
}

enum DepDest {
    /// With the main output (`-M`, `-MM`, `-MF -`).
    Main,
    /// A file.
    File(String),
}

fn dep_destination(args: &Args, input: &str) -> DepDest {
    if args.deps.only.is_some() {
        return DepDest::Main;
    }
    match args.deps.file.as_deref() {
        Some("-") => DepDest::Main,
        Some(f) => DepDest::File(f.to_owned()),
        None => DepDest::File(deps::default_dep_file(args.output.as_deref(), input)),
    }
}

fn deps_text(args: &Args, input: &str, files: &[String]) -> String {
    let targets: Vec<String> = if args.deps.targets.is_empty() {
        let t = match args.output.as_deref() {
            Some(o) if args.deps.only.is_none() => o.to_owned(),
            _ => deps::default_target(input),
        };
        vec![deps::quote_target(&t)]
    } else {
        args.deps
            .targets
            .iter()
            .map(|(t, quote)| if *quote { deps::quote_target(t) } else { t.clone() })
            .collect()
    };
    deps::render(&targets, files, args.deps.phony)
}
