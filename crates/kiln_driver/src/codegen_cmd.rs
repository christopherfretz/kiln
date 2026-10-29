//! Code generation (`spec/driver.md` "Code generation", `spec/codegen.md`
//! §5): `-S -emit-llvm`, `-c -emit-llvm`, `-S`, `-c` and linking.
//!
//! Each source input goes through the preprocessor exactly as `-E` builds
//! it ([`Job::build_preprocessor`]), the parser and Sema, then
//! [`kiln_codegen::emit`]; the `-fconstexpr-report` is written on every
//! path. The IR is then written (`.ll`), or piped to `llvm-as` (`.bc`) or
//! `llc` (`.s`, `.o`); linking runs `clang++` over the objects and the
//! linker inputs, in command-line order. The external commands live in
//! [`crate::link`].

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Group};
use kiln_lex::Interner;
use kiln_parse::{ParseOptions, parse_preprocessed};
use kiln_pp::PicModel;
use kiln_sema::target::Target;
use kiln_source::{SourceManager, Span};

use crate::args::{Action, Args, Input, Lang, LinkOptKind};
use crate::link::Command;
use crate::pp_cmd::{Job, PpInputs, input_lang};
use crate::toolchain::{StdLib, Tool, Toolchain};
use crate::{Io, input_std, make_engine, print_version, report_output_error, write_output};

/// How far the codegen path goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// `-S -emit-llvm`: textual IR (`.ll`).
    IrText,
    /// `-c -emit-llvm`: bitcode (`.bc`, through `llvm-as`).
    Bitcode,
    /// `-S`: assembly (`.s`, through `llc`).
    Assembly,
    /// `-c`: an object file (`.o`, through `llc -filetype=obj`).
    Object,
    /// No `-c` / `-S`: objects, then `clang++` links them.
    Link,
}

impl Phase {
    /// The phase `args` asks for. `-S` wins over `-c`, as in clang;
    /// `-emit-llvm` alone is [`Phase::Link`] (and an error).
    pub fn of(args: &Args) -> Phase {
        let c = args.compile;
        match (args.action, c.assembly, c.object, c.emit_llvm) {
            (Action::Compile, true, _, true) => Phase::IrText,
            (Action::Compile, true, _, false) => Phase::Assembly,
            (Action::Compile, false, true, true) => Phase::Bitcode,
            (Action::Compile, false, true, false) => Phase::Object,
            _ => Phase::Link,
        }
    }

    /// The output suffix of a compile-only phase (`ll`, `bc`, `s`, `o`).
    fn suffix(self) -> &'static str {
        match self {
            Phase::IrText => "ll",
            Phase::Bitcode => "bc",
            Phase::Assembly => "s",
            Phase::Object | Phase::Link => "o",
        }
    }
}

/// True if `input` is compiled (a C or C++ source, or any file after
/// `-x`); everything else is a linker input, as in clang.
pub fn is_source(input: &Input) -> bool {
    if input.lang.is_some() || input.path == "-" {
        return true;
    }
    let ext = Path::new(&input.path).extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "c" | "i"
            | "cc"
            | "cp"
            | "cpp"
            | "CPP"
            | "cxx"
            | "c++"
            | "C"
            | "ii"
            | "cppm"
            | "ixx"
            | "h"
            | "hh"
            | "hpp"
            | "hxx"
            | "H"
            | "inc"
    )
}

/// The default output name of `input` for a compile-only phase: its file
/// stem in the current directory with the phase's suffix (`foo.o`;
/// standard input is `-.o`), as clang names it.
pub fn default_output(input: &str, phase: Phase) -> String {
    let stem = if input == "-" {
        "-".to_owned()
    } else {
        Path::new(input)
            .file_stem()
            .map_or_else(|| input.to_owned(), |s| s.to_string_lossy().into_owned())
    };
    format!("{stem}.{}", phase.suffix())
}

/// The `llc` optimization level for the driver's `-O` flag. `llc`
/// defaults to `-O2`, so the default (`-O0`) is always passed.
pub fn llc_opt_level(flag: Option<&str>) -> &'static str {
    match flag {
        None | Some("-O0") => "-O0",
        Some("-O" | "-O1" | "-Og") => "-O1",
        Some("-O2" | "-Os" | "-Oz") => "-O2",
        Some(_) => "-O3",
    }
}

/// The identification string of kiln's modules (`!llvm.ident`).
pub fn ident() -> String {
    format!("kiln version {}", env!("CARGO_PKG_VERSION"))
}

/// One source input through the frontend and IR generation.
#[derive(Debug)]
pub struct CodegenJob<'j>(pub Job<'j>);

impl CodegenJob<'_> {
    /// Runs the job: diagnostics (and the `N errors generated.` summary)
    /// to standard error. Returns the module's text, or `None` after an
    /// error (the error count is the second element).
    pub fn run(self) -> (Option<String>, u32) {
        let job = &self.0;
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let mut diag_opts = job.diag_opts.clone();
        diag_opts.pending.clear();
        let engine = make_engine(&sm, &diag_opts, job.stderr.clone(), None);
        let mut ir = None;
        if let Some(inputs) = job.pp_inputs(&sm, &engine) {
            ir = self.lower(&sm, &engine, &interner, inputs);
        }
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        let errors = engine.error_count();
        (if errors == 0 { ir } else { None }, errors)
    }

    fn lower<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
    ) -> Option<String> {
        let job = &self.0;
        let a = job.args;
        let Ok(target) = Target::from_triple(job.toolchain.target.triple) else {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
                .arg(job.toolchain.target.triple)
                .emit(engine);
            return None;
        };
        let (pp, _deps) = job.build_preprocessor(sm, engine, interner, inputs, true)?;
        let popts = ParseOptions {
            std: job.std.lex,
            exceptions: a.predef.exceptions.unwrap_or(true),
            rtti: a.predef.rtti.unwrap_or(true),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            ..ParseOptions::default()
        };
        let limits = a.constexpr.limits();
        let (mut tu, _pp) =
            parse_preprocessed(pp, Arc::clone(interner), engine, target.clone(), &popts, limits);
        let mut ir = None;
        if engine.error_count() == 0 {
            let opts = CodegenOptions {
                source_filename: if job.path == "-" { "-".to_owned() } else { job.path.to_owned() },
                exceptions: a.predef.exceptions.unwrap_or(true),
                ident: ident(),
            };
            match emit(&mut tu.ast, &mut tu.oven, &target, &opts) {
                Ok(m) => ir = Some(m.to_string()),
                Err(e) => {
                    let span = e.loc.map_or(Span::INVALID, Span::empty);
                    Diag::new(DiagId::cg_error, span).arg(&e.msg).emit(engine);
                }
            }
        }
        // After IR generation too: it can evaluate more constants.
        a.constexpr.write_report(&tu.oven, sm, engine);
        if !tu.inst_report.is_empty() {
            engine.flush();
            let mut err = job.stderr.clone();
            let _ = err.write_all(tu.inst_report.as_bytes());
        }
        ir
    }
}

/// The driver's warning options for its own diagnostics: the `-W` options
/// naming groups kiln knows (unknown ones are reported by each input's
/// engine, not again here).
fn driver_diag_options(opts: &DiagOptions) -> DiagOptions {
    let mut o = opts.clone();
    o.pending.clear();
    o.remarks.clear();
    o.warnings.retain(|w| {
        let w = w.strip_prefix("no-").unwrap_or(w);
        let w = w.strip_prefix("error=").unwrap_or(w);
        matches!(w, "error" | "everything") || Group::from_name(w).is_some()
    });
    o
}

/// One step of the plan.
#[derive(Debug)]
enum Step<'a> {
    /// Run the frontend on an input; the IR goes to `dest`.
    Frontend { input: &'a Input, dest: Dest },
    /// An external command (`llc`, `llvm-as`, `clang++`), with the IR of
    /// the preceding frontend step on its standard input if `piped`.
    External { cmd: Command, piped: bool },
}

/// Where a frontend step's IR goes.
#[derive(Debug, PartialEq, Eq)]
enum Dest {
    /// Written to this file (`-` is standard output).
    File(String),
    /// Piped to the next step.
    Pipe,
}

/// Runs the codegen path over `inputs` (which exist); returns the exit
/// status.
pub fn compile(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&Input],
    stdin_text: Option<&str>,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> i32 {
    let phase = Phase::of(args);
    let dsm = SourceManager::new();
    let drv = make_engine(&dsm, &driver_diag_options(diag_opts), io.stderr.clone(), Some("kiln"));
    if phase == Phase::Link && args.compile.emit_llvm {
        Diag::new(DiagId::drv_emit_llvm_link, Span::INVALID).emit(&drv);
        drv.flush();
        return 1;
    }
    let sources = inputs.iter().filter(|i| is_source(i)).count();
    if phase != Phase::Link {
        if sources > 1 && args.output.is_some() {
            Diag::new(DiagId::drv_out_file_multiple, Span::INVALID).emit(&drv);
            drv.flush();
            return 1;
        }
        warn_unused_link_inputs(args, inputs, &drv);
    }
    let temp = TempDir::new();
    let plan = plan(args, tc, inputs, phase, &temp);
    if args.print_commands {
        print_version(&mut io.stderr, tc, false);
        let mut err = io.stderr.clone();
        for step in &plan {
            let _ = err.write_all(render(step, true).as_bytes());
        }
        let _ = err.flush();
        drv.flush();
        return i32::from(drv.error_count() > 0);
    }
    let status = execute(args, tc, &plan, stdin_text, diag_opts, &drv, io, &temp);
    temp.remove();
    drv.flush();
    status.max(i32::from(drv.error_count() > 0))
}

/// `<x>: 'linker' input unused` and `argument unused during compilation`
/// for a compile-only run, in command-line order.
fn warn_unused_link_inputs(args: &Args, inputs: &[&Input], drv: &DiagEngine<'_>) {
    let report_opts = |pos: usize| {
        for o in args.link.iter().filter(|o| o.pos == pos) {
            match o.kind {
                LinkOptKind::Input => {
                    Diag::new(DiagId::drv_linker_input_unused, Span::INVALID).arg(&o.arg).emit(drv);
                }
                LinkOptKind::Option => {
                    Diag::new(DiagId::drv_argument_unused, Span::INVALID).arg(&o.arg).emit(drv);
                }
                LinkOptKind::Silent => {}
            }
        }
    };
    // `inputs` holds only the existing inputs; positions count all of them.
    for (pos, input) in args.inputs.iter().enumerate() {
        report_opts(pos);
        if inputs.iter().any(|i| std::ptr::eq(*i, input)) && !is_source(input) {
            Diag::new(DiagId::drv_linker_input_unused, Span::INVALID).arg(&input.path).emit(drv);
        }
    }
    report_opts(args.inputs.len());
}

/// The temporary directory of a link's objects, created on first use and
/// removed (by its own path) afterwards.
#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let name = format!("kiln-{}-{nanos:09}", std::process::id());
        TempDir { path: std::env::temp_dir().join(name) }
    }

    fn file(&self, name: &str) -> String {
        self.path.join(name).display().to_string()
    }

    fn create(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.path)
    }

    fn remove(&self) {
        if self.path.is_dir() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// The steps for `inputs`, in order.
fn plan<'a>(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&'a Input],
    phase: Phase,
    temp: &TempDir,
) -> Vec<Step<'a>> {
    let mut steps = Vec::new();
    let mut link_inputs: Vec<(usize, String)> = Vec::new();
    for (n, input) in inputs.iter().enumerate() {
        let pos = args.inputs.iter().position(|i| std::ptr::eq(i, *input)).unwrap_or(n);
        if !is_source(input) {
            link_inputs.push((pos, input.path.clone()));
            continue;
        }
        let out = || args.output.clone().unwrap_or_else(|| default_output(&input.path, phase));
        match phase {
            Phase::IrText => {
                steps.push(Step::Frontend { input, dest: Dest::File(out()) });
            }
            Phase::Bitcode => {
                steps.push(Step::Frontend { input, dest: Dest::Pipe });
                let mut cmd = Command::new(tc, Tool::LlvmAs);
                cmd.arg("-o").arg(out()).arg("-");
                steps.push(Step::External { cmd, piped: true });
            }
            Phase::Assembly | Phase::Object => {
                steps.push(Step::Frontend { input, dest: Dest::Pipe });
                steps.push(Step::External { cmd: llc(args, tc, phase, out()), piped: true });
            }
            Phase::Link => {
                let stem = default_output(&input.path, Phase::Object);
                let obj = temp.file(&format!("{n}-{stem}"));
                steps.push(Step::Frontend { input, dest: Dest::Pipe });
                steps.push(Step::External { cmd: llc(args, tc, phase, obj.clone()), piped: true });
                link_inputs.push((pos, obj));
            }
        }
    }
    if phase == Phase::Link {
        steps.push(Step::External { cmd: link_command(args, tc, &link_inputs), piped: false });
    }
    steps
}

/// `llc` for an assembly or object output.
fn llc(args: &Args, tc: &Toolchain, phase: Phase, out: String) -> Command {
    let mut cmd = Command::new(tc, Tool::Llc);
    cmd.arg(llc_opt_level(args.opt_flag.as_deref()));
    let pic = args.predef.pic.unwrap_or(PicModel::Pie(2));
    cmd.arg(if pic == PicModel::None {
        "-relocation-model=static"
    } else {
        "-relocation-model=pic"
    });
    if phase != Phase::Assembly {
        cmd.arg("-filetype=obj");
    }
    cmd.arg("-o").arg(out).arg("-");
    cmd
}

/// `clang++` over the objects and linker inputs (`(position, path)`),
/// with the linker options in command-line order.
fn link_command(args: &Args, tc: &Toolchain, inputs: &[(usize, String)]) -> Command {
    let mut cmd = Command::new(tc, Tool::Linker);
    if args.target.is_some() {
        cmd.arg(format!("--target={}", tc.target.triple));
    }
    if let Some(s) = &args.sysroot {
        cmd.arg(format!("--sysroot={s}"));
    }
    if let Some(o) = &args.opt_flag {
        cmd.arg(o.clone());
    }
    if args.predef.pthread {
        cmd.arg("-pthread");
    }
    match args.stdlib {
        Some(StdLib::LibCxx) => {
            cmd.arg("-stdlib=libc++");
        }
        Some(StdLib::LibStdCxx) => {
            cmd.arg("-stdlib=libstdc++");
        }
        None => {}
    }
    let opts_at = |cmd: &mut Command, pos: usize| {
        for o in args.link.iter().filter(|o| o.pos == pos) {
            cmd.arg(o.arg.clone());
        }
    };
    for pos in 0..=args.inputs.len() {
        opts_at(&mut cmd, pos);
        for (_, path) in inputs.iter().filter(|(p, _)| *p == pos) {
            cmd.arg(path.clone());
        }
    }
    cmd.arg("-o").arg(args.output.clone().unwrap_or_else(|| "a.out".to_owned()));
    cmd
}

/// A step as `-###` (`quote_all`) or `-v` prints it.
fn render(step: &Step<'_>, quote_all: bool) -> String {
    match step {
        Step::Frontend { input, dest } => {
            let to = match dest {
                Dest::File(f) => f.as_str(),
                Dest::Pipe => "|",
            };
            format!(" (in-process) frontend \"{}\" -> \"{to}\"\n", input.path)
        }
        Step::External { cmd, .. } => cmd.render(quote_all),
    }
}

/// Runs the plan; returns the exit status.
#[allow(clippy::too_many_arguments)]
fn execute(
    args: &Args,
    tc: &Toolchain,
    plan: &[Step<'_>],
    stdin_text: Option<&str>,
    diag_opts: &DiagOptions,
    drv: &DiagEngine<'_>,
    io: &mut Io,
    temp: &TempDir,
) -> i32 {
    let mut failed = false;
    let mut ir: Option<String> = None;
    let mut skip_piped = false;
    for step in plan {
        if args.verbose {
            let mut err = io.stderr.clone();
            let _ = err.write_all(render(step, false).as_bytes());
        }
        match step {
            Step::Frontend { input, dest } => {
                ir = frontend(args, tc, input, stdin_text, diag_opts, io);
                skip_piped = ir.is_none();
                failed |= ir.is_none();
                if let (Some(text), Dest::File(f)) = (&ir, dest)
                    && let Err(e) = write_output(Some(f), text, &mut io.stdout)
                {
                    report_output_error(diag_opts, &io.stderr, f, &e);
                    failed = true;
                }
            }
            Step::External { cmd, piped: true } => {
                if skip_piped {
                    continue;
                }
                if cmd.tool == Tool::Llc
                    && Phase::of(args) == Phase::Link
                    && let Err(e) = temp.create()
                {
                    let dir = temp.path.display().to_string();
                    report_output_error(diag_opts, &io.stderr, &dir, &e.to_string());
                    failed = true;
                    continue;
                }
                failed |= !cmd.run(ir.as_deref(), &mut io.stdout, &io.stderr, drv);
            }
            Step::External { cmd, piped: false } => {
                // The link step: only when everything before it worked.
                if !failed {
                    failed |= !cmd.run(None, &mut io.stdout, &io.stderr, drv);
                }
            }
        }
    }
    i32::from(failed)
}

/// The frontend on one source input; `None` after an error.
fn frontend(
    args: &Args,
    tc: &Toolchain,
    input: &Input,
    stdin_text: Option<&str>,
    diag_opts: &DiagOptions,
    io: &Io,
) -> Option<String> {
    let lang = input_lang(input.lang, &input.path);
    let report = |d: Diag| {
        let sm = SourceManager::new();
        let e = make_engine(&sm, diag_opts, io.stderr.clone(), None);
        d.emit(&e);
        e.flush();
    };
    let std = match input_std(args, lang) {
        Ok(s) => s,
        Err(d) => {
            report(*d);
            return None;
        }
    };
    if lang == Lang::C {
        report(Diag::new(DiagId::drv_not_implemented, Span::INVALID).arg("compiling C"));
        return None;
    }
    let job = Job {
        args,
        toolchain: tc,
        std,
        path: &input.path,
        stdin_text: (input.path == "-").then(|| stdin_text.unwrap_or_default().to_owned()),
        diag_opts,
        stderr: io.stderr.clone(),
    };
    CodegenJob(job).run().0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Args {
        Args::parse(&v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn phases() {
        assert_eq!(Phase::of(&args(&["a.cpp"])), Phase::Link);
        assert_eq!(Phase::of(&args(&["-c", "a.cpp"])), Phase::Object);
        assert_eq!(Phase::of(&args(&["-S", "a.cpp"])), Phase::Assembly);
        assert_eq!(Phase::of(&args(&["-c", "-S", "a.cpp"])), Phase::Assembly);
        assert_eq!(Phase::of(&args(&["-S", "-emit-llvm", "a.cpp"])), Phase::IrText);
        assert_eq!(Phase::of(&args(&["-emit-llvm", "-c", "a.cpp"])), Phase::Bitcode);
        assert_eq!(Phase::of(&args(&["-emit-llvm", "a.cpp"])), Phase::Link);
    }

    #[test]
    fn output_names() {
        assert_eq!(default_output("dir/foo.cpp", Phase::Object), "foo.o");
        assert_eq!(default_output("foo.cc", Phase::Assembly), "foo.s");
        assert_eq!(default_output("foo.cpp", Phase::IrText), "foo.ll");
        assert_eq!(default_output("foo.cpp", Phase::Bitcode), "foo.bc");
        assert_eq!(default_output("-", Phase::Object), "-.o");
    }

    #[test]
    fn opt_levels() {
        assert_eq!(llc_opt_level(None), "-O0");
        assert_eq!(llc_opt_level(Some("-Og")), "-O1");
        assert_eq!(llc_opt_level(Some("-Os")), "-O2");
        assert_eq!(llc_opt_level(Some("-Ofast")), "-O3");
    }

    #[test]
    fn sources_and_linker_inputs() {
        let a = args(&["a.cpp", "b.o", "-x", "c++", "c.o", "-", "libx.a"]);
        let kinds: Vec<bool> = a.inputs.iter().map(is_source).collect();
        assert_eq!(kinds, [true, false, true, true, true]);
    }

    #[test]
    fn link_order() {
        let a = args(&["-lm", "a.o", "-L/x", "b.o", "-Wl,-z,now", "-static", "-O2"]);
        let tc = Toolchain::new(crate::toolchain::Target::host(), None, None);
        let inputs: Vec<(usize, String)> =
            a.inputs.iter().enumerate().map(|(i, x)| (i, x.path.clone())).collect();
        let cmd = link_command(&a, &tc, &inputs);
        assert_eq!(
            cmd.args,
            ["-O2", "-lm", "a.o", "-L/x", "b.o", "-Wl,-z,now", "-static", "-o", "a.out"]
        );
    }
}
