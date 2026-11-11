//! The `kiln` driver: a clang-compatible command line over the frontend,
//! one binary under two names as clang and clang++ (`kiln`, `kiln++`;
//! spec/driver.md "Driver modes").
//! See `spec/driver.md` and `docs/ARCHITECTURE.md` ("Driver").
//!
//! Preprocessing (`-E`, `-M`, `-MM`), `-fsyntax-only [-fdump-ast]`
//! (WP-048's parser) and code generation (`-S -emit-llvm`, `-S`, `-c`,
//! linking; WP-052b, [`codegen_cmd`]) are implemented.
#![forbid(unsafe_code)]

mod aarch64_cpus;
mod aarch64_extensions;
mod aarch64_levels;
pub mod args;
mod cdb;
pub mod codegen_cmd;
pub mod codegen_flags;
pub mod constexpr_opts;
pub mod coverage_flags;
pub mod crash;
pub mod debug_flags;
pub mod deps;
pub mod features;
pub mod fp_flags;
mod help;
pub mod link;
pub mod llvm;
pub mod module_cache;
mod pch;
mod pch_image;
pub mod pp_cmd;
pub mod rsp;
pub mod sanitize;
pub mod scan_deps;
pub mod syntax_cmd;
pub mod templ_opts;
pub mod time;
pub mod time_trace;
pub mod toolchain;
pub mod version;
pub mod x86_features;
pub mod x86_targets;

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, PoisonError};

use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Format, JsonSink, TextSink};
use kiln_source::{SourceManager, Span};

use crate::args::{Action, ArgError, Args, Lang, LinkOptKind};
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

pub(crate) fn emit_arg_error(e: &ArgError, engine: &DiagEngine<'_>) {
    let d = match e {
        ArgError::Unknown(a) => Diag::new(DiagId::drv_unknown_argument, Span::INVALID).arg(a),
        ArgError::UnknownSuggest(a, s) => {
            Diag::new(DiagId::drv_unknown_argument_suggest, Span::INVALID).arg(a).arg(s)
        }
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
        ArgError::OnlyAllowedWith(a, b) => {
            Diag::new(DiagId::drv_argument_only_allowed_with, Span::INVALID).arg(a).arg(b)
        }
        ArgError::NotAllowedWith(a, b) => {
            Diag::new(DiagId::drv_argument_not_allowed_with, Span::INVALID).arg(a).arg(b)
        }
        ArgError::NoSuchFile(f) => Diag::new(DiagId::drv_no_such_file, Span::INVALID).arg(f),
        ArgError::MalformedIgnorelist(m) => {
            Diag::new(DiagId::drv_malformed_sanitizer_ignorelist, Span::INVALID).arg(m)
        }
        ArgError::MemtagStackRequiresMte => {
            Diag::new(DiagId::drv_stack_tagging_requires_hardware_feature, Span::INVALID)
        }
        ArgError::PatchableEntryOffset => {
            Diag::new(DiagId::drv_unsupported_fpatchable_function_entry_argument, Span::INVALID)
        }
    };
    d.emit(engine);
}

/// clang's `HandleImmediateArgs`: a toolchain probe prints its answer
/// and stops, whatever else is given; the first in clang's order wins
/// (spec/driver.md "Toolchain probes"). `None` when no probe was given.
fn immediate(args: &Args, tc: &Toolchain, io: &mut Io, drv: &DiagEngine<'_>) -> Option<i32> {
    let p = &args.probe;
    if !p.any() && !args.print_resource_dir {
        return None;
    }
    let mut line = |s: &str| {
        let _ = writeln!(io.stdout, "{s}");
        let _ = io.stdout.flush();
        Some(0)
    };
    if p.dumpmachine {
        return line(tc.target.triple);
    }
    if p.dumpversion {
        // The clang compatibility version, as `__clang_major__` says:
        // probes compare it with clang/GCC thresholds.
        return line(args.predef.clang_compat.version());
    }
    if p.search_dirs {
        return Some(delegate(args, tc, io, drv, &["-print-search-dirs".to_owned()]));
    }
    if args.print_resource_dir {
        return line(&tc.resource_dir);
    }
    if let Some(name) = &p.file_name {
        // kiln's own resource directory first (`include`, ...), then
        // what the linking clang++ finds (`libc.so`, `crtbegin.o`).
        let own = std::path::Path::new(&tc.resource_dir).join(name);
        if !name.is_empty() && own.exists() {
            return line(&own.display().to_string());
        }
        // libc++'s module manifest, for build systems' `import std;`
        // (CMake's `CXX_MODULE_STD`; spec/driver.md "Module cache").
        if name == "libc++.modules.json"
            && let Some(json) = tc.libcxx_modules_json()
        {
            return line(&json.display().to_string());
        }
        return Some(delegate(args, tc, io, drv, &[format!("-print-file-name={name}")]));
    }
    if let Some(name) = &p.prog_name {
        return Some(delegate(args, tc, io, drv, &[format!("-print-prog-name={name}")]));
    }
    if p.libgcc_file_name {
        return Some(delegate(args, tc, io, drv, &["-print-libgcc-file-name".to_owned()]));
    }
    if p.target_triple {
        return line(tc.target.triple);
    }
    if p.multiarch {
        return line(tc.target.multiarch);
    }
    // `-print-sysroot` (GCC's): the `--sysroot`, else an empty line.
    line(args.sysroot.as_deref().unwrap_or(""))
}

/// Asks the linking clang++ (with the toolchain flags a link passes it)
/// and relays its answer: the honest one for the libraries and programs
/// a kiln link uses.
fn delegate(
    args: &Args,
    tc: &Toolchain,
    io: &mut Io,
    drv: &DiagEngine<'_>,
    probe: &[String],
) -> i32 {
    let mut cmd = link::Command::new(tc, toolchain::Tool::Linker);
    for a in codegen_cmd::toolchain_link_args(args, tc) {
        cmd.arg(a);
    }
    for a in probe {
        cmd.arg(a.clone());
    }
    let ok = cmd.run(None, &mut io.stdout, &io.stderr, drv);
    drv.flush();
    i32::from(!ok)
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
        version::VERSION,
        tc.target.triple
    );
    // WP-1016: the selected LLVM (spec/driver.md "LLVM discovery").
    let _ = match &tc.llvm {
        Ok(i) => writeln!(w, "LLVM backend: {} ({})", i.major, i.dir.display()),
        Err(_) => writeln!(w, "LLVM backend: none (kiln supports LLVM {})", llvm::supported_text()),
    };
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
    } else if args.action == Action::Precompile {
        // `-o` names the module file, written by the job.
        None
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

/// One `-fsyntax-only` input.
#[allow(clippy::too_many_arguments)]
pub(crate) fn syntax_only(
    args: &Args,
    tc: &Toolchain,
    std: LangStd,
    path: &str,
    stdin_text: Option<String>,
    diag_opts: &DiagOptions,
    io: &Io,
) -> JobResult {
    let job =
        Job { args, toolchain: tc, std, path, stdin_text, diag_opts, stderr: io.stderr.clone() };
    SyntaxJob(job).run()
}

/// The driver's warnings about the command line, with its `-W` options
/// applied, before any job runs: clang's deprecated `treating 'c' input as
/// 'c++' when in C++ mode` once per promoted input (spec/driver.md "Input
/// languages"), then `-Woption-ignored` for `-m[no-]outline-atomics` (the
/// last one written) on a non-`AArch64` target, once per source input, as
/// clang warns once per compile job, and the x86 feature flags' warnings
/// (WP-1000: `argument unused during compilation` for the ones clang
/// claims on `AArch64`, `the clang compiler does not support '-m3dnow'` on
/// x86-64). `true` when `-Werror` made one an error (clang then runs no
/// job).
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
    // WP-1115: the `-ftime-trace` family without a compiler job that
    // writes the trace (`-fsyntax-only`), or the modifiers without
    // `-ftime-trace`.
    if !args.quiet_unused_arguments {
        unused_xray.extend(time_trace::unused(args).iter().cloned());
        // WP-1044: the driver's separate `-ftrapv-handler <name>`.
        unused_xray.extend(args.codegen.trapv_handler_separate.iter().cloned());
    }
    // WP-1000: the x86 feature flags clang claims on `AArch64` without an
    // error (`-msse4`, `-msse4.2`, `-m[no-]3dnow[a]`), and on x86 its
    // warning for the dropped `-m3dnow` / `-m3dnowa`.
    let v22 = args.predef.clang_compat == kiln_pp::ClangCompat::Clang22;
    let mut no_3dnow: Vec<&String> = Vec::new();
    for a in args.x86_feature_flags.iter().filter(|a| !x86_features::unknown_at(a, v22)) {
        if tc.target.is_aarch64() {
            if !args.quiet_unused_arguments && x86_features::non_x86_spelling(a, v22).is_none() {
                unused_xray.push(a.clone());
            }
        } else if x86_features::is_3dnow(a) && !a.starts_with("-mno-") {
            no_3dnow.push(a);
        }
    }
    if promoted.is_empty() && ignored.is_none() && unused_xray.is_empty() && no_3dnow.is_empty() {
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
    for _ in inputs.iter().filter(|i| codegen_cmd::is_source(i)) {
        for a in &no_3dnow {
            Diag::new(DiagId::drv_clang_unsupported, Span::INVALID).arg(*a).emit(&e);
        }
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
/// `--gcc-install-dir` / `--gcc-toolchain`, `--llvm-dir`), reporting a bad
/// target, GCC directory or explicitly named LLVM to `drv`. Shared with the crash reproducer ([`crash`]).
pub(crate) fn toolchain_of(args: &Args, drv: &DiagEngine<'_>) -> Toolchain {
    let target = match &args.target {
        None => Target::host(),
        Some(t) => Target::parse(t).unwrap_or_else(|| {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID).arg(t).emit(drv);
            Target::host()
        }),
    };
    let mut tc = Toolchain::new(
        target,
        args.sysroot.as_deref(),
        args.resource_dir.as_deref(),
        args.llvm_dir.as_deref(),
    );
    if let Err(e) = &tc.llvm
        && e.is_explicit()
    {
        e.emit(drv);
    }
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
    // `kiln-scan-deps` and its `clang-scan-deps*` aliases (spec/modules.md
    // §11.9).
    if scan_deps::is_scanner_name(args::program_basename(argv0)) {
        return scan_deps::run(argv0, &command_line, io);
    }
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
    // spec/driver.md "Time trace": clang checks the granularity only when
    // a compiler job writes the trace.
    if let Err((flag, value)) = time_trace::settings(&args) {
        Diag::new(DiagId::drv_invalid_int_value, Span::INVALID).arg(flag).arg(value).emit(&drv);
    }
    let tc = toolchain_of(&args, &drv);
    let (target_features, feature_errors) = features::resolve(&args, &tc.target);
    for e in &feature_errors {
        emit_arg_error(e, &drv);
    }
    // spec/driver.md "Sanitizers".
    let (sanitize, sanitize_errors) = args.sanitize(!tc.target.is_aarch64(), tc.target.triple);
    // WP-1151: clang's `hasTargetFeatureMTE`, by `-fclang-compat=`.
    let mte = target_features.memtag_hw;
    for e in &sanitize_errors {
        emit_arg_error(e, &drv);
    }
    for f in &args.sanitize.ignorelists {
        if !std::path::Path::new(f).exists() {
            emit_arg_error(&ArgError::NoSuchFile(f.clone()), &drv);
        }
    }
    // WP-1032: the lists LLVM's `SpecialCaseList` rejects.
    for e in &args.sanitize.ignorelist_errors(sanitize.opts.kinds, &tc.resource_dir, args.clang22())
    {
        emit_arg_error(e, &drv);
    }
    // WP-1138: clang checks the target feature after the option errors.
    for e in &sanitize.hardware_errors(tc.target.is_aarch64(), mte) {
        emit_arg_error(e, &drv);
    }
    if args.help {
        let _ = io.stdout.write_all(help::HELP.as_bytes());
        return 0;
    }
    // An explicitly named LLVM that is not usable stops everything, before
    // any probe delegates to it (spec/driver.md "LLVM discovery").
    if tc.llvm.as_ref().is_err_and(llvm::LlvmError::is_explicit) {
        drv.flush();
        return 1;
    }
    if let Some(rc) = immediate(&args, &tc, &mut io, &drv) {
        return rc;
    }
    if args.version {
        print_version(&mut io.stdout, &tc, false);
        let _ = io.stdout.flush();
    }
    if args.verbose {
        print_version(&mut io.stderr, &tc, true);
    }
    let inputs = existing_inputs(&args, &drv);
    // clang counts linker inputs (`-l`, `-Wl,`, `-Xlinker`) as inputs:
    // `clang++ -Wl,--version` runs the linker alone (Meson's linker probe,
    // WP-920).
    let linker_inputs =
        args.link.iter().any(|l| matches!(l.kind, LinkOptKind::Input | LinkOptKind::Xlinker));
    if inputs.is_empty() && !(linker_inputs && args.inputs.is_empty()) {
        if (args.version || args.verbose) && args.inputs.is_empty() && drv.error_count() == 0 {
            return 0;
        }
        Diag::new(DiagId::drv_no_input_files, Span::INVALID).emit(&drv);
    }
    if drv.error_count() == 0 {
        check_action(&args, &drv);
    }
    if drv.error_count() == 0 && args.deps.missing_generated && args.deps.only.is_none() {
        // clang `Clang::ConstructJob`: `-MG` only lists headers for `-M`/`-MM`.
        Diag::new(DiagId::drv_mg_requires_m_or_mm, Span::INVALID).emit(&drv);
    }
    drv.flush();
    if drv.error_count() > 0 {
        return 1;
    }
    drop(drv);
    if driver_warnings(&args, &tc, &inputs, &diag_opts, &io.stderr) {
        return 1;
    }
    // WP-1037: clang's driver turns the first `-include <h>` into
    // `-include-pch <h>.pch` (or `.gch`) when there is one; it probes once
    // per job that preprocesses (not for `.i`/`.ii`, objects, module files).
    let jobs = inputs
        .iter()
        .filter(|i| codegen_cmd::is_source(i) || args.is_header_input(i))
        .filter(|i| {
            i.lang.is_some()
                || !matches!(
                    std::path::Path::new(&i.path).extension().and_then(|e| e.to_str()),
                    Some("i" | "ii")
                )
        })
        .count();
    let Ok(probed) = pch::probe(&args, jobs, &diag_opts, &io.stderr) else {
        return 1;
    };
    let args: &Args = probed.as_ref().unwrap_or(&args);
    // `-include-pch` (spec/driver.md "Precompiled headers", WP-1023): the
    // checked PCH replays as `-include`s.
    let Ok(pch_args) = pch::consumer(args, &tc, &inputs, &diag_opts, &io.stderr) else {
        return 1;
    };
    let args: &Args = pch_args.as_ref().unwrap_or(args);

    if args.action == Action::Precompile && inputs.len() > 1 && args.output.is_some() {
        let sm = SourceManager::new();
        let e = make_engine(
            &sm,
            &codegen_cmd::driver_diag_options(&diag_opts),
            io.stderr.clone(),
            Some(&args.program),
        );
        Diag::new(DiagId::drv_out_file_multiple, Span::INVALID).emit(&e);
        e.flush();
        return 1;
    }
    // Headers (and every input with `-emit-pch`) are precompiled; the
    // other inputs go on (WP-1023).
    let all_inputs = inputs.len();
    let (producers, inputs): (Vec<&args::Input>, Vec<&args::Input>) =
        inputs.into_iter().partition(|i| pch::is_producer(args, i));
    if !producers.is_empty() {
        let errors = pch::produce(args, &tc, &producers, all_inputs, &diag_opts, &mut io);
        if errors > 0 || inputs.is_empty() {
            return i32::from(errors > 0);
        }
    }
    let stdin_text = inputs.iter().any(|i| i.path == "-").then(|| read_stdin(&mut io.stdin));
    if matches!(args.action, Action::Compile | Action::Link) && args.deps.only.is_none() {
        return codegen_cmd::compile(
            args,
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
    if args.deps.cdb_fragment.is_some() {
        // `-E`/`-M` write to `-o` or standard output; `-fsyntax-only`
        // has no output.
        let out = (args.action != Action::SyntaxOnly)
            .then(|| args.output.clone().unwrap_or_else(|| "-".to_owned()));
        let jobs: Vec<_> =
            inputs.iter().map(|&input| cdb::CdbJob { input, output: out.clone() }).collect();
        let sm = SourceManager::new();
        let e = make_engine(&sm, &diag_opts, io.stderr.clone(), Some(&args.program));
        if !cdb::write(args, &tc, &jobs, &e) {
            errors += 1;
        }
        e.flush();
    }
    for input in inputs {
        let lang = args.input_lang(input);
        let std = match input_std(args, lang) {
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
        if matches!(args.action, Action::SyntaxOnly | Action::Precompile) {
            let stdin = if input.path == "-" { stdin_text.clone() } else { None };
            // spec/driver.md "Time trace": `--precompile` traces its job
            // (`Frontend`, then the module file's `WriteAST`), named
            // after the module file.
            let trace = time_trace::settings(args).ok().flatten().map(|s| {
                let out = pp_cmd::precompile_output(args, &input.path);
                time_trace::InputTrace::start(s, time_trace::path(args, None, &input.path, &out))
            });
            let r = {
                let _frontend = trace.as_ref().map(|_| kiln_source::trace::Scope::new("Frontend"));
                syntax_only(args, &tc, std, &input.path, stdin, &diag_opts, &io)
            };
            if let Some(mut t) = trace {
                t.failed = r.errors > 0;
                if let Err((path, e)) = t.finish(&args.program) {
                    // clang: reported, the exit status unchanged.
                    report_output_error(&diag_opts, &io.stderr, &path, &e);
                }
            }
            errors += r.errors;
            output.push_str(&r.output);
            if let Some(list) = r.dep_list
                && !deps::write_side_file(args, &input.path, &list.borrow(), &diag_opts, &mut io)
            {
                errors += 1;
            }
            continue;
        }
        let job = Job {
            args,
            toolchain: &tc,
            std,
            path: &input.path,
            stdin_text: if input.path == "-" { stdin_text.clone() } else { None },
            diag_opts: &diag_opts,
            stderr: io.stderr.clone(),
        };
        // spec/driver.md "Time trace": `-E` / `-M` trace the
        // preprocessor job alone (`ExecuteCompiler`), as clang.
        let trace = time_trace::settings(args).ok().flatten().map(|s| {
            let out = args.output.as_deref().unwrap_or("-");
            let mut t =
                time_trace::InputTrace::start(s, time_trace::path(args, None, &input.path, out));
            t.no_sources = true;
            t
        });
        let r = job.run();
        errors += r.errors;
        if let Some(mut t) = trace {
            t.failed = r.errors > 0;
            if let Err((path, e)) = t.finish(&args.program) {
                // clang: reported, the exit status unchanged.
                report_output_error(&diag_opts, &io.stderr, &path, &e);
            }
        }
        output.push_str(&r.output);
        if let Some(files) = r.deps {
            let text = deps::deps_text(args, &input.path, &files);
            match deps::dep_destination(args, &input.path) {
                deps::DepDest::Main => output.push_str(&text),
                deps::DepDest::File(f) => side_deps.push((f, text)),
            }
        }
    }

    write_results(args, &diag_opts, &mut io, &output, errors, side_deps)
}

pub(crate) fn report_output_error(opts: &DiagOptions, stderr: &SharedWriter, path: &str, e: &str) {
    let sm = SourceManager::new();
    let engine = make_engine(&sm, opts, stderr.clone(), None);
    Diag::new(DiagId::drv_cannot_open_output, Span::INVALID).arg(path).arg(e).emit(&engine);
    engine.flush();
}
