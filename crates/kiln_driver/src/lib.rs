//! The `kiln` driver: a clang-compatible command line over the frontend,
//! one binary under two names as clang and clang++ (`kiln`, `kiln++`;
//! spec/driver.md "Driver modes").
//! See `spec/driver.md` and `docs/ARCHITECTURE.md` ("Driver").
//!
//! Preprocessing (`-E`, `-M`, `-MM`), `-fsyntax-only [-fdump-ast]`
//! (WP-048's parser) and code generation (`-S -emit-llvm`, `-S`, `-c`,
//! linking; WP-052b, [`codegen_cmd`]) are implemented.
#![forbid(unsafe_code)]

pub mod args;
pub mod codegen_cmd;
pub mod codegen_flags;
pub mod constexpr_opts;
pub mod crash;
pub mod debug_flags;
pub mod deps;
pub mod features;
pub mod fp_flags;
pub mod link;
pub mod pp_cmd;
pub mod rsp;
pub mod sanitize;
pub mod syntax_cmd;
pub mod templ_opts;
pub mod time;
pub mod toolchain;
pub mod version;
pub mod x86_targets;

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Format, JsonSink, TextSink};
use kiln_source::{SourceManager, Span};

use crate::args::{Action, ArgError, Args, Lang};
use crate::pp_cmd::{Job, JobResult};
use crate::syntax_cmd::SyntaxJob;
use crate::toolchain::{GccSearch, LangStd, Target, Toolchain};

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
OVERVIEW: kiln, a C++26 frontend emitting LLVM IR

USAGE: kiln [options] file...

kiln++ compiles every input as C++. kiln picks by extension: .c, .h and
.i are C (preprocessing only so far), everything else is C++. -x <lang>
overrides both.

Implemented: -E -P -C -CC -dM -o -x -std= -D -U -I -isystem -iquote
-idirafter -cxx-isystem -nostdinc -nostdinc++ -nostdlibinc -nobuiltininc
-isysroot --sysroot -resource-dir -stdlib= -f[no-]experimental-library
--gcc-toolchain= --gcc-install-dir=
--target= -march= -mcpu= -target-feature --embed-dir= -M -MM
-MD -MMD -MF -MT -MQ -MP -W... -w -pedantic -ferror-limit=
-fdiagnostics-format= -f[no-]color-diagnostics -fkiln-reproducible-time
-fkiln-source-path-style=<clang|gcc> -fkiln-reflection-mangling=<kiln|gcc>
-fsyntax-only -fdump-ast[=...] -fdump-ast-filter= -verify[=<prefixes>]
-verify-ignore-unexpected[=<levels>] -Xclang <arg> -fconstexpr-steps=
-fconstexpr-depth= -fconstexpr-memory= -fconstexpr-backtrace-limit=
-fconstexpr-report= -ftemplate-depth= -ftemplate-backtrace-limit=
-fkiln-instantiation-report[=all|json] -fkiln-stack-size= -fkiln-dump-cfg
-fkiln-cfg-stats -fkiln-cfg-max-elems= -fkiln-outline-expansions=
-fkiln-memory-report -c -S -emit-llvm
-O<n> -g (ignored) -l -L -Wl, -fuse-ld= -static -nostdlib -shared -rdynamic -pie -no-pie -pthread -###
-v --version.
See spec/driver.md.
";

/// `-fkiln-memory-report` (spec/driver.md): one line per row of `r`,
/// `memory: <name> count=N live=N reserved=N`, then `memory: total
/// live=N reserved=N` ([`MemReport::live_bytes`], which leaves out the
/// derived `instantiation.*` rows). A measurement aid, not a stable
/// format.
pub(crate) fn write_memory_report(r: &kiln_ast::mem::MemReport, out: &mut impl Write) {
    use std::fmt::Write as _;
    let mut s = String::new();
    for row in &r.rows {
        let _ = writeln!(
            s,
            "memory: {} count={} live={} reserved={}",
            row.name, row.count, row.live_bytes, row.reserved_bytes
        );
    }
    let _ = writeln!(s, "memory: total live={} reserved={}", r.live_bytes(), r.reserved_bytes());
    let _ = out.write_all(s.as_bytes());
}

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
        ArgError::InvalidInt(a, v) => {
            Diag::new(DiagId::drv_invalid_int_value, Span::INVALID).arg(a).arg(v)
        }
        ArgError::InvalidFlagValue(a, v) => {
            Diag::new(DiagId::drv_invalid_value, Span::INVALID).arg(a).arg(v)
        }
        ArgError::InvalidVerifyPrefix(p) => Diag::new(DiagId::drv_invalid_value, Span::INVALID)
            .arg("-verify=")
            .arg(p)
            .note(DiagId::drv_verify_prefix_spelling, Span::INVALID),
        ArgError::UnsupportedValue(o, v) => {
            Diag::new(DiagId::drv_unsupported_option_argument, Span::INVALID).arg(o).arg(v)
        }
        ArgError::UnsupportedForTarget(o, t) => {
            Diag::new(DiagId::drv_unsupported_opt_for_target, Span::INVALID).arg(o).arg(t)
        }
        ArgError::UnsupportedByKiln(a) => {
            Diag::new(DiagId::drv_unsupported_flag, Span::INVALID).arg(a)
        }
        ArgError::InvalidArgumentToOption(v, o) => {
            Diag::new(DiagId::drv_invalid_argument_to_option, Span::INVALID).arg(v).arg(o)
        }
        ArgError::NotAllowedWith(a, b) => {
            Diag::new(DiagId::drv_argument_not_allowed_with, Span::INVALID).arg(a).arg(b)
        }
        ArgError::NoSuchFile(f) => Diag::new(DiagId::drv_no_such_file, Span::INVALID).arg(f),
        ArgError::PatchableEntryOffset => {
            Diag::new(DiagId::drv_unsupported_fpatchable_function_entry_argument, Span::INVALID)
        }
    };
    d.emit(engine);
}

pub(crate) fn print_version(w: &mut dyn Write, tc: &Toolchain, verbose: bool) {
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
        for c in &tc.gcc_candidates {
            let _ = writeln!(w, "Found candidate GCC installation: {c}");
        }
        let _ = write!(
            w,
            "Selected GCC installation: {}\n\
             Candidate multilib: .;@m64\nSelected multilib: .;@m64\n",
            g.path
        );
    }
}

/// The standard of an input in `lang`, or the error to report (also the
/// per-input `-cc1` error for an invalid `-fvisibility=` value).
pub(crate) fn input_std(args: &Args, lang: Lang) -> Result<LangStd, Box<Diag>> {
    let cplusplus = lang == Lang::Cxx;
    if let Some(a) = &args.invalid_visibility {
        let v = a.strip_prefix("-fvisibility=").unwrap_or(a);
        return Err(Box::new(Diag::new(DiagId::drv_invalid_value, Span::INVALID).arg(a).arg(v)));
    }
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
pub(crate) fn write_output(
    path: Option<&str>,
    text: &str,
    stdout: &mut dyn Write,
) -> Result<(), String> {
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
    let not_impl = (args.print_commands
        && matches!(action, Action::Preprocess | Action::SyntaxOnly))
    .then_some("-### with -E or -fsyntax-only")
    .or(args.print.defines_too.then_some("-dD"))
    .or(args.print.includes_too.then_some("-dI"))
    .or((action != Action::SyntaxOnly
        && (args.dump_ast.is_some() || args.dump_ast_filter.is_some()))
    .then_some("-fdump-ast without -fsyntax-only"))
    .or((action != Action::SyntaxOnly && args.dump_record_layouts)
        .then_some("-fdump-record-layouts without -fsyntax-only"))
    .or((action != Action::SyntaxOnly && args.dump_vtable_layouts)
        .then_some("-fdump-vtable-layouts without -fsyntax-only"))
    .or((action != Action::SyntaxOnly && args.dump_cfg)
        .then_some("-fkiln-dump-cfg without -fsyntax-only"))
    .or((action != Action::SyntaxOnly && args.cfg_stats)
        .then_some("-fkiln-cfg-stats without -fsyntax-only"))
    .or((action != Action::SyntaxOnly && args.verify.is_some())
        .then_some("-verify without -fsyntax-only"));
    if let Some(what) = not_impl {
        Diag::new(DiagId::drv_not_implemented, Span::INVALID).arg(what).emit(drv);
    }
    // WP-813: clang's `XRayArgs` checks.
    for d in args.codegen.xray_errors() {
        d.emit(drv);
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

/// One `-fsyntax-only` input (C is not implemented yet).
#[allow(clippy::too_many_arguments)]
fn syntax_only(
    args: &Args,
    tc: &Toolchain,
    std: LangStd,
    lang: Lang,
    path: &str,
    stdin_text: Option<String>,
    diag_opts: &DiagOptions,
    io: &Io,
) -> JobResult {
    if lang == Lang::C {
        let sm = SourceManager::new();
        let e = make_engine(&sm, diag_opts, io.stderr.clone(), None);
        Diag::new(DiagId::drv_not_implemented, Span::INVALID).arg("-fsyntax-only for C").emit(&e);
        e.flush();
        return JobResult { errors: 1, ..JobResult::default() };
    }
    let job =
        Job { args, toolchain: tc, std, path, stdin_text, diag_opts, stderr: io.stderr.clone() };
    SyntaxJob(job).run()
}

/// The driver's warnings about the command line, with its `-W` options
/// applied, before any job runs: clang's deprecated `treating 'c' input as
/// 'c++' when in C++ mode` once per promoted input (spec/driver.md "Input
/// languages"), then `-Woption-ignored` for `-m[no-]outline-atomics` (the
/// last one written) on a non-`AArch64` target, once per source input, as
/// clang warns once per compile job. `true` when `-Werror` made one an
/// error (clang then runs no job).
fn driver_warnings(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&args::Input],
    diag_opts: &DiagOptions,
    stderr: &SharedWriter,
) -> bool {
    let promoted = args.promoted_inputs(inputs);
    let ignored = args.outline_atomics.filter(|_| !tc.target.is_aarch64());
    // WP-813: XRay's options without `-fxray-instrument` (clang claims
    // them only with it).
    let mut unused_xray =
        if args.quiet_unused_arguments { Vec::new() } else { args.codegen.unused_xray_args() };
    // WP-846: clang claims `-msign-return-address=` on `AArch64` only.
    if !args.quiet_unused_arguments && !tc.target.is_aarch64() {
        unused_xray.extend(args.codegen.sign_return_address_written.iter().cloned());
    }
    if promoted.is_empty() && ignored.is_none() && unused_xray.is_empty() {
        return false;
    }
    let sm = SourceManager::new();
    let e = make_engine(
        &sm,
        &codegen_cmd::driver_diag_options(diag_opts),
        stderr.clone(),
        Some(&args.program),
    );
    for (_, (from, to)) in promoted {
        Diag::new(DiagId::drv_treating_input_as_cxx, Span::INVALID).arg(from).arg(to).emit(&e);
    }
    if let Some(on) = ignored {
        let flag = if on { "-moutline-atomics" } else { "-mno-outline-atomics" };
        for _ in inputs.iter().filter(|i| codegen_cmd::is_source(i)) {
            Diag::new(DiagId::drv_option_ignored, Span::INVALID)
                .arg(tc.target.arch)
                .arg(flag)
                .emit(&e);
        }
    }
    for x in unused_xray {
        Diag::new(DiagId::drv_argument_unused, Span::INVALID).arg(x).emit(&e);
    }
    e.flush();
    e.error_count() > 0
}

/// The inputs that exist (`-` is standard input); the others are errors.
fn existing_inputs<'a>(args: &'a Args, drv: &DiagEngine<'_>) -> Vec<&'a args::Input> {
    args.inputs
        .iter()
        .filter(|i| {
            let ok = i.path == "-" || std::path::Path::new(&i.path).exists();
            if !ok {
                Diag::new(DiagId::drv_no_such_file, Span::INVALID).arg(&i.path).emit(drv);
            }
            ok
        })
        .collect()
}

/// The toolchain of `args` (`--target`, `--sysroot`, `-resource-dir`,
/// `--gcc-install-dir` / `--gcc-toolchain`), reporting a bad target or GCC
/// directory to `drv`. Shared with the crash reproducer ([`crash`]).
pub(crate) fn toolchain_of(args: &Args, drv: &DiagEngine<'_>) -> Toolchain {
    let target = match &args.target {
        None => Target::host(),
        Some(t) => Target::parse(t).unwrap_or_else(|| {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID).arg(t).emit(drv);
            Target::host()
        }),
    };
    let mut tc = Toolchain::new(target, args.sysroot.as_deref(), args.resource_dir.as_deref());
    let gcc_search = match (&args.gcc_install_dir, &args.gcc_toolchain) {
        (Some(d), _) => GccSearch::InstallDir(d),
        (None, Some(d)) => GccSearch::Toolchain(d),
        (None, None) => GccSearch::Default,
    };
    if gcc_search != GccSearch::Default
        && let Err(dir) = tc.select_gcc(gcc_search)
    {
        Diag::new(DiagId::drv_invalid_gcc_install_dir, Span::INVALID).arg(dir).emit(drv);
    }
    tc
}

/// Runs the driver on `argv` (without the program name) as `kiln++`: every
/// input is C++ unless `-x` or `--driver-mode=gcc` says otherwise. Returns
/// the exit code: 0 on success, 1 on any error (clang's convention).
pub fn run(command_line: &[String], io: Io) -> i32 {
    run_as("kiln++", command_line, io)
}

/// Runs the driver on `argv` (without the program name) for the program
/// `argv0`, whose basename is the driver diagnostics' prefix and whose
/// name picks the driver mode (`kiln` or `kiln++`; spec/driver.md "Driver
/// modes"). Returns the exit code as [`run`].
#[allow(clippy::too_many_lines, reason = "the driver's phases in order, one dispatch per action")]
pub fn run_as(argv0: &str, command_line: &[String], mut io: Io) -> i32 {
    let command_line = rsp::expand(command_line);
    let args = Args::parse_as(argv0, &command_line);
    // The hidden `-fkiln-crash-for-testing=` (spec/driver.md "Internal
    // compiler errors"); per thread, so set (or cleared) on every run.
    kiln_source::activity::set_crash_for_testing(args.crash_for_testing);
    if args.crash_for_testing == Some(kiln_source::activity::Phase::Driver) {
        // Entering the requested phase panics: a crash outside any input.
        drop(kiln_source::activity::enter(
            kiln_source::activity::Phase::Driver,
            kiln_source::Loc::INVALID,
        ));
    }
    let mut diag_opts = DiagOptions::from_args(&args.diag_args);
    diag_opts.clang_compat = args.predef.clang_compat.major();
    let dsm = SourceManager::new();
    let mut drv_opts = diag_opts.clone();
    drv_opts.warnings.clear();
    drv_opts.remarks.clear();
    let drv = make_engine(&dsm, &drv_opts, io.stderr.clone(), Some(&args.program));
    for e in &args.errors {
        emit_arg_error(e, &drv);
    }
    for f in &args.frameworks {
        Diag::new(DiagId::drv_framework_dir_ignored, Span::INVALID).arg(f).emit(&drv);
    }
    if !args.quiet_unused_arguments {
        for f in &args.time_trace {
            Diag::new(DiagId::drv_time_trace_ignored, Span::INVALID).arg(f).emit(&drv);
        }
    }
    let tc = toolchain_of(&args, &drv);
    for e in &features::resolve(&args, &tc.target).1 {
        emit_arg_error(e, &drv);
    }
    // spec/driver.md "Sanitizers".
    for e in &args.sanitize(!tc.target.is_aarch64(), tc.target.triple).1 {
        emit_arg_error(e, &drv);
    }
    for f in &args.sanitize.ignorelists {
        if !std::path::Path::new(f).exists() {
            emit_arg_error(&ArgError::NoSuchFile(f.clone()), &drv);
        }
    }
    if args.help {
        let _ = io.stdout.write_all(HELP.as_bytes());
        return 0;
    }
    if args.print_resource_dir {
        // clang prints the directory and stops, whatever else is given.
        let _ = writeln!(io.stdout, "{}", tc.resource_dir);
        return 0;
    }
    if args.version {
        print_version(&mut io.stdout, &tc, false);
        let _ = io.stdout.flush();
    }
    if args.verbose {
        print_version(&mut io.stderr, &tc, true);
    }
    let inputs = existing_inputs(&args, &drv);
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
    if driver_warnings(&args, &tc, &inputs, &diag_opts, &io.stderr) {
        return 1;
    }

    let stdin_text = inputs.iter().any(|i| i.path == "-").then(|| read_stdin(&mut io.stdin));
    if matches!(args.action, Action::Compile | Action::Link) && args.deps.only.is_none() {
        return codegen_cmd::compile(
            &args,
            &tc,
            &inputs,
            stdin_text.as_deref(),
            &diag_opts,
            &mut io,
        );
    }

    let mut output = String::new();
    let mut errors = 0u32;
    let mut side_deps: Vec<(String, String)> = Vec::new();
    for input in inputs {
        let lang = args.input_lang(input);
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
        if args.action == Action::SyntaxOnly {
            let stdin = if input.path == "-" { stdin_text.clone() } else { None };
            let r = syntax_only(&args, &tc, std, lang, &input.path, stdin, &diag_opts, &io);
            errors += r.errors;
            output.push_str(&r.output);
            if let Some(list) = r.dep_list
                && !deps::write_side_file(&args, &input.path, &list.borrow(), &diag_opts, &mut io)
            {
                errors += 1;
            }
            continue;
        }
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
            let text = deps::deps_text(&args, &input.path, &files);
            match deps::dep_destination(&args, &input.path) {
                deps::DepDest::Main => output.push_str(&text),
                deps::DepDest::File(f) => side_deps.push((f, text)),
            }
        }
    }

    write_results(&args, &diag_opts, &mut io, &output, errors, side_deps)
}

pub(crate) fn report_output_error(opts: &DiagOptions, stderr: &SharedWriter, path: &str, e: &str) {
    let sm = SourceManager::new();
    let engine = make_engine(&sm, opts, stderr.clone(), None);
    Diag::new(DiagId::drv_cannot_open_output, Span::INVALID).arg(path).arg(e).emit(&engine);
    engine.flush();
}
