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

use kiln_codegen::{AutoVarInit, CodegenOptions, RelocModel, Visibility, emit_with_diags};
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Group};
use kiln_lex::Interner;
use kiln_parse::{ParseOptions, parse_preprocessed};
use kiln_pp::{ClangCompat, PicModel};
use kiln_sema::target::Target;
use kiln_source::{SourceManager, Span};

use crate::args::{Action, Args, Input, Lang, LinkOptKind};
use crate::link::Command;
use crate::pp_cmd::{Job, PpInputs, SharedDeps};
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

/// The `opt` pipeline of an `-O` flag (clang's `PassBuilder` default
/// pipelines): `None` at `-O0`, where only a coroutine module is lowered
/// (spec/driver.md "IR optimization").
pub fn opt_pipeline(flag: Option<&str>) -> Option<&'static str> {
    match flag {
        None | Some("-O0") => None,
        Some("-O" | "-O1" | "-Og") => Some("default<O1>"),
        Some("-O2") => Some("default<O2>"),
        Some("-Os") => Some("default<Os>"),
        Some("-Oz") => Some("default<Oz>"),
        Some(_) => Some("default<O3>"),
    }
}

/// clang's `CodeGenOpts.OptimizationLevel` for `-O<flag>`: 0 at `-O0`,
/// 1 at `-O`/`-O1`/`-Og`, 2 at `-O2`/`-Os`/`-Oz`, else 3.
/// [basic.indet]/1 (P2795R5): C++26 automatic variables start with their
/// erroneous value (spec/codegen.md §7r).
fn default_auto_var_init(std: &crate::toolchain::LangStd) -> AutoVarInit {
    if std.cplusplus && std.lex >= kiln_lex::Std::Cxx26 {
        AutoVarInit::Zero
    } else {
        AutoVarInit::Uninitialized
    }
}

pub fn opt_level(flag: Option<&str>) -> u8 {
    match flag {
        None | Some("-O0") => 0,
        Some("-O" | "-O1" | "-Og") => 1,
        Some("-O2" | "-Os" | "-Oz") => 2,
        Some(_) => 3,
    }
}

/// clang's `CodeGenOpts.OptimizeSize` for `-O<flag>`: 1 at `-Os`, 2 at
/// `-Oz`, else 0.
pub fn opt_size(flag: Option<&str>) -> u8 {
    match flag {
        Some("-Os") => 1,
        Some("-Oz") => 2,
        _ => 0,
    }
}

/// The identification string of kiln's modules (`!llvm.ident`).
pub fn ident() -> String {
    format!("kiln version {}", env!("CARGO_PKG_VERSION"))
}

/// The [`CodegenOptions`] fields set by the driver's codegen flags
/// (spec/driver.md "Codegen flags"), the rest defaulted; `cpu` is the
/// resolved `-march=` CPU ([`crate::features::TargetFeatures::cpu`]).
fn codegen_flag_options(a: &Args, cpu: Option<String>) -> CodegenOptions {
    let f = &a.codegen;
    CodegenOptions {
        stack_protector: f.stack_protector(),
        unwind_tables: f.unwind_tables(a.predef.freestanding.unwrap_or(false)),
        no_plt: f.no_plt,
        static_destructors: f.static_destructors,
        wrapv: f.wrapv(),
        wrapv_pointer: f.wrapv_pointer(a.predef.clang_compat == ClangCompat::Clang22),
        null_pointer_is_valid: f.null_pointer_is_valid(),
        target_cpu: cpu,
        tune_cpu: f.mtune.clone().filter(|t| t != "native"),
        no_inline: a.predef.no_inline,
        code_model: f.code_model,
        reflection_mangling: a.reflection_mangling,
        outline_expansions: a
            .outline_expansions
            .unwrap_or(kiln_codegen::OUTLINE_EXPANSIONS_DEFAULT),
        ..CodegenOptions::default()
    }
}

/// One source input through the frontend and IR generation.
#[derive(Debug)]
pub struct CodegenJob<'j>(pub Job<'j>);

impl CodegenJob<'_> {
    /// Runs the job: diagnostics (and the `N errors generated.` summary)
    /// to standard error. Returns the module's text, or `None` after an
    /// error (the error count is the second element), and the `-MD` /
    /// `-MMD` dependency list, complete whatever the errors (the third).
    pub fn run(self) -> (Option<String>, u32, Option<SharedDeps>) {
        let job = &self.0;
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let mut diag_opts = job.diag_opts.clone();
        diag_opts.pending.clear();
        let engine = make_engine(&sm, &diag_opts, job.stderr.clone(), None);
        let mut ir = None;
        let mut deps = None;
        if let Some(inputs) = job.pp_inputs(&sm, &engine) {
            ir = self.lower(&sm, &engine, &interner, inputs, &mut deps);
        }
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        let errors = engine.error_count();
        (if errors == 0 { ir } else { None }, errors, deps)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "target, parse and codegen options in one straight-line pipeline"
    )]
    fn lower<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
        deps_out: &mut Option<SharedDeps>,
    ) -> Option<String> {
        let job = &self.0;
        let a = job.args;
        let Ok(mut target) = Target::from_triple(job.toolchain.target.triple) else {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
                .arg(job.toolchain.target.triple)
                .emit(engine);
            return None;
        };
        if let Some(signed) = a.predef.char_signed {
            target.types.char_signed = signed;
        }
        // WP-630: `cx16` makes 128-bit atomics lock-free (clang
        // `X86TargetInfo::setMaxAtomicWidth`).
        if crate::features::resolve(a, &job.toolchain.target).0.x86_cx16() {
            target.types.max_atomic_inline_bytes = 16;
        }
        let (pp, deps) = job.build_preprocessor(sm, engine, interner, inputs, true)?;
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        let popts = ParseOptions {
            std: job.std.lex,
            gnu: job.std.gnu,
            exceptions: a.predef.exceptions.unwrap_or(true),
            rtti: a.predef.rtti.unwrap_or(true),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            reflection: a.predef.reflection,
            coro_aligned_allocation: a.predef.coro_aligned_allocation,
            lax_vector_conversions: a.lax_vector_conversions,
            clang_compat: a.predef.clang_compat.major(),
            gnu_lib_builtins: a.stdlib.unwrap_or_default() == StdLib::LibStdCxx,
            sve: feats.sve,
            cfg_max_elems: a.cfg_max_elems.unwrap_or(kiln_sema::cfg::DEFAULT_MAX_ELEMS),
            permissive: a.permissive,
            contract_semantic: a.contract_semantic.unwrap_or_default(),
            ..ParseOptions::default()
        };
        let limits = a.constexpr.limits();
        let (mut tu, pp) =
            parse_preprocessed(pp, Arc::clone(interner), engine, target.clone(), &popts, limits);
        *deps_out = crate::pp_cmd::finish_deps(pp, deps);
        let mut module = None;
        if engine.error_count() == 0 {
            let opts = CodegenOptions {
                source_filename: if job.path == "-" { "-".to_owned() } else { job.path.to_owned() },
                exceptions: a.predef.exceptions.unwrap_or(true),
                rtti: a.predef.rtti.unwrap_or(true),
                ident: ident(),
                outline_atomics: a.outline_atomics.unwrap_or(true),
                target_features: feats.extra.clone(),
                target_attr: Some(if job.toolchain.target.is_aarch64() {
                    crate::features::target_attr_features as kiln_codegen::TargetAttrParser
                } else {
                    crate::features::x86_target_attr_features
                }),
                // clang 19's `vscale_range` also needs NEON (its `SveMode`).
                sve: feats.sve
                    && (a.predef.clang_compat == ClangCompat::Clang22
                        || feats.enabled().iter().any(|f| f == "neon")),
                sve_vector_bits: feats.sve_vector_bits,
                complex_range: a.complex_range(),
                opt_level: opt_level(a.opt_flag.as_deref()),
                opt_size: opt_size(a.opt_flag.as_deref()),
                strict_aliasing: a.strict_aliasing.unwrap_or(true),
                clang_compat: a.predef.clang_compat.major(),
                llvm_major: job.toolchain.llvm_major(),
                // spec/contracts.md §6.3: kiln's default handler unless the
                // standard library ships its own (with its `<contracts>`).
                contract_default_handler: !job
                    .toolchain
                    .stdlib_ships_contracts(a.stdlib.unwrap_or_default()),
                omit_frame_pointer: a.omit_frame_pointer,
                omit_leaf_frame_pointer: a.omit_leaf_frame_pointer,
                auto_var_init: a.auto_var_init.unwrap_or(default_auto_var_init(&job.std)),
                auto_var_init_max_size: a
                    .auto_var_init_max_size
                    .unwrap_or(kiln_codegen::AUTO_VAR_INIT_MAX_SIZE),
                visibility: a.visibility.unwrap_or_default(),
                type_visibility: if a.visibility_ms_compat {
                    Visibility::Default
                } else {
                    a.visibility.unwrap_or_default()
                },
                inlines_hidden: a.visibility_inlines_hidden,
                reloc: match a.predef.pic.unwrap_or(PicModel::Pie(2)) {
                    PicModel::None => RelocModel::Static,
                    PicModel::Pic(l) => RelocModel::Pic(l),
                    PicModel::Pie(l) => RelocModel::Pie(l),
                },
                semantic_interposition: a.semantic_interposition,
                direct_access_external_data: a.direct_access_external_data,
                cxx_modules: job.std.cplusplus && job.std.lex >= kiln_lex::Std::Cxx20,
                ..codegen_flag_options(a, feats.cpu.clone())
            };
            // Sema sets the level on each evaluation it runs; a unit that
            // ran none still folds at its level (WP-416).
            tu.oven.set_clang_compat(opts.clang_compat);
            let (r, warnings) = emit_with_diags(&mut tu.ast, &mut tu.oven, &target, &opts);
            for w in warnings {
                w.emit(engine);
            }
            match r {
                Ok(m) => module = Some(m),
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
        if a.memory_report {
            engine.flush();
            crate::write_memory_report(&tu.memory(), &mut job.stderr.clone());
        }
        // The AST, the Oven and the module are each about as large as the
        // IR text for a big unit: print only once the AST is gone, so the
        // peak holds two of the three, not all of them (WP-633).
        drop(tu);
        module.map(|m| m.to_text())
    }
}

/// The driver's warning options for its own diagnostics: the `-W` options
/// naming groups kiln knows (unknown ones are reported by each input's
/// engine, not again here).
pub(crate) fn driver_diag_options(opts: &DiagOptions) -> DiagOptions {
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
    let drv =
        make_engine(&dsm, &driver_diag_options(diag_opts), io.stderr.clone(), Some(&args.program));
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
        if !args.quiet_unused_arguments {
            warn_unused_link_inputs(args, inputs, &drv);
        }
    }
    let temp = TempDir::new();
    let plan = plan(args, tc, inputs, phase, &temp);
    if args.print_commands {
        print_version(&mut io.stderr, tc, false);
        let mut err = io.stderr.clone();
        for step in &plan {
            let _ = err.write_all(render(step, true).as_bytes());
            // The `opt` runs `execute` makes after the frontend, as `-v`
            // prints them (spec/driver.md "`-###`").
            if matches!(step, Step::Frontend { .. }) {
                for cmd in planned_ir_passes(args, tc).unwrap_or_default() {
                    let _ = err.write_all(cmd.render(true).as_bytes());
                }
            }
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
                LinkOptKind::Xlinker => {
                    Diag::new(DiagId::drv_linker_input_unused, Span::INVALID)
                        .arg(format!("-Xlinker {}", o.arg))
                        .emit(drv);
                }
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

/// Whether the IR text needs the coroutine lowering: it declares an
/// `llvm.coro.*` intrinsic (clang's `CoroConditionalWrapper` test,
/// `coro::declaresAnyIntrinsic`). Every `presplitcoroutine` function
/// calls `llvm.coro.id`; a `__builtin_coro_noop` alone needs it too
/// (spec/driver.md "IR optimization").
pub fn needs_coro_lowering(ir: &str) -> bool {
    ir.lines().any(|l| l.starts_with("declare ") && l.contains(" @llvm.coro."))
}

/// Whether the IR defines an `always_inline` function (an attribute
/// group holds `alwaysinline`; kiln puts it on definitions only). clang's
/// O0 pipeline (`AlwaysInlinerPass`) inlines every call to one, deleting
/// a discardable one left unused, before `-emit-llvm` writes the module.
pub fn needs_always_inline(ir: &str) -> bool {
    ir.lines().any(|l| {
        l.starts_with("attributes #") && l.split_ascii_whitespace().any(|w| w == "alwaysinline")
    })
}

/// The `opt` runs between the frontend and the IR's consumer, if any:
/// at `-O1` and above the default pipeline of the level (which places
/// `CoroEarly`, `CoroSplit`, `CoroElide` and `CoroCleanup` where clang's does);
/// at `-O0` clang's whole O0 pipeline when the module defines an
/// `always_inline` function (`AlwaysInlinerPass`, then the coroutine
/// lowering), else clang's O0 coroutine lowering (`CoroConditionalWrapper` in
/// `buildO0DefaultPipeline`: `CoroEarly`, `CoroSplit`, `CoroCleanup`,
/// then `GlobalDCE`), only for a module that needs it. The first run
/// writes bitcode, which the second reads back discarding the local
/// value names the passes made, as clang's (release) pipeline does
/// (`opt` refuses `-discard-value-names` for textual input). For an
/// object (`-c`, or before linking) `llc` reads the first run's bitcode
/// directly: local value names do not reach an object, and the second
/// process and the text round trip were most of the `-O0`
/// always-inliner's cost (spec/codegen.md §6 "Always-inliner").
fn ir_passes(args: &Args, tc: &Toolchain, ir: &str) -> Option<Vec<Command>> {
    if args.disable_llvm_passes {
        return None;
    }
    let passes = match opt_pipeline(args.opt_flag.as_deref()) {
        Some(p) => p,
        None if needs_always_inline(ir) => "default<O0>",
        None if needs_coro_lowering(ir) => "coro-early,cgscc(coro-split),coro-cleanup,globaldce",
        None => return None,
    };
    Some(opt_commands(args, tc, passes))
}

/// The `opt` runs `-###` prints after a frontend step: those of an `-O1`
/// or higher level, whose pipeline does not depend on the IR. At `-O0`
/// the runs depend on what the IR declares ([`ir_passes`]) and are not
/// shown (spec/driver.md "IR optimization").
fn planned_ir_passes(args: &Args, tc: &Toolchain) -> Option<Vec<Command>> {
    if args.disable_llvm_passes {
        return None;
    }
    opt_pipeline(args.opt_flag.as_deref()).map(|p| opt_commands(args, tc, p))
}

/// The `opt` run of `passes`, then for a textual or bitcode output the
/// run that discards local value names ([`ir_passes`]).
fn opt_commands(args: &Args, tc: &Toolchain, passes: &str) -> Vec<Command> {
    let mut run = Command::new(tc, Tool::Opt);
    run.arg(format!("-passes={passes}")).arg("-o").arg("-").arg("-");
    if matches!(Phase::of(args), Phase::Object | Phase::Link) {
        return vec![run];
    }
    let mut names = Command::new(tc, Tool::Opt);
    names.arg("-discard-value-names").arg("-S").arg("-o").arg("-").arg("-");
    vec![run, names]
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
    // spec/driver.md "Codegen flags": clang's `-ffunction-sections` /
    // `-fdata-sections` and the `-march=native` / x86 `-march=<cpu>` CPU.
    if args.codegen.function_sections {
        cmd.arg("-function-sections");
    }
    if args.codegen.data_sections {
        cmd.arg("-data-sections");
    }
    if let Some(cpu) = crate::features::resolve(args, &tc.target).0.cpu {
        cmd.arg(format!("-mcpu={cpu}"));
    }
    // The module's `Code Model` flag says the same; explicit, as clang
    // hands the model to the target machine.
    if let Some(cm) = args.codegen.code_model {
        cmd.arg(format!("-code-model={}", cm.name()));
    }
    cmd.arg("-o").arg(out).arg("-");
    cmd
}

/// `clang++` over the objects and linker inputs (`(position, path)`),
/// with the linker options in command-line order.
fn link_command(args: &Args, tc: &Toolchain, inputs: &[(usize, String)]) -> Command {
    let mut cmd = Command::new(tc, Tool::Linker);
    // `-v` reaches the linking `clang++` too, so its own `-v` lines (the
    // linker's command line) follow: build systems read the implicit link
    // directories and libraries from them (CMake's compiler ABI probe).
    if args.verbose {
        cmd.arg("-v");
    }
    if args.target.is_some() {
        cmd.arg(format!("--target={}", tc.target.triple));
    }
    if let Some(s) = &args.sysroot {
        cmd.arg(format!("--sysroot={s}"));
    }
    if let Some(d) = &args.gcc_toolchain {
        cmd.arg(format!("--gcc-toolchain={d}"));
    }
    if let Some(d) = &args.gcc_install_dir {
        cmd.arg(format!("--gcc-install-dir={d}"));
    } else if let Some(g) = tc.gcc.as_ref().filter(|_| tc.gcc_from_driver) {
        // The headers came from `toolchain::DEFAULT_GCC`'s installation;
        // link against the same one rather than clang++'s own choice.
        cmd.arg(format!("--gcc-install-dir={}", g.path));
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
    if args.experimental_library {
        // clang adds `-lc++experimental` when the stdlib is libc++.
        cmd.arg("-fexperimental-library");
    }
    let opts_at = |cmd: &mut Command, pos: usize| {
        for o in args.link.iter().filter(|o| o.pos == pos) {
            if o.kind == LinkOptKind::Xlinker {
                cmd.arg("-Xlinker");
            }
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
    // The frontend's textual IR, or the bitcode of an `opt` run that
    // feeds `llc` directly (`ir_passes`).
    let mut ir: Option<Vec<u8>> = None;
    let mut skip_piped = false;
    for step in plan {
        if args.verbose {
            let mut err = io.stderr.clone();
            let _ = err.write_all(render(step, false).as_bytes());
        }
        match step {
            Step::Frontend { input, dest } => {
                ir = frontend(args, tc, input, stdin_text, diag_opts, io).map(String::into_bytes);
                // The IR-level passes run before the IR is written or
                // piped on (spec/driver.md "IR optimization").
                if let Some(text) = &ir
                    && let Some(cmds) = ir_passes(args, tc, std::str::from_utf8(text).unwrap_or(""))
                {
                    // Each run's output replaces its input: the frontend's
                    // text is not kept alongside a copy (WP-633).
                    let mut data = ir.take().unwrap_or_default();
                    let mut ok = true;
                    for cmd in &cmds {
                        if args.verbose {
                            let mut err = io.stderr.clone();
                            let _ = err.write_all(cmd.render(false).as_bytes());
                        }
                        let mut out = Vec::new();
                        ok = cmd.run(Some(&data), &mut out, &io.stderr, drv);
                        if !ok {
                            break;
                        }
                        data = out;
                    }
                    ir = ok.then_some(data);
                }
                skip_piped = ir.is_none();
                failed |= ir.is_none();
                if let (Some(text), Dest::File(f)) = (&ir, dest)
                    && let Err(e) =
                        write_output(Some(f), &String::from_utf8_lossy(text), &mut io.stdout)
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

/// The frontend on one source input; `None` after an error. Writes the
/// input's `-MD` / `-MMD` dependency file, errors or not
/// ([`crate::deps::write_side_file`]).
fn frontend(
    args: &Args,
    tc: &Toolchain,
    input: &Input,
    stdin_text: Option<&str>,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> Option<String> {
    let lang = args.input_lang(input);
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
    let (ir, _, deps) = CodegenJob(job).run();
    if let Some(list) = deps
        && !crate::deps::write_side_file(args, &input.path, &list.borrow(), diag_opts, io)
    {
        return None;
    }
    ir
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
    fn coro_lowering_detection() {
        assert!(needs_coro_lowering("declare token @llvm.coro.id(i32, ptr, ptr, ptr) #1\n"));
        assert!(needs_coro_lowering("declare ptr @llvm.coro.noop() #1\n"));
        assert!(!needs_coro_lowering("define void @\"llvm.coro.id\"() {\n}\n"));
        assert!(!needs_coro_lowering("declare void @f() #0\n"));
    }

    #[test]
    fn always_inline_detection() {
        assert!(needs_always_inline("attributes #1 = { alwaysinline mustprogress nounwind }\n"));
        assert!(!needs_always_inline("attributes #0 = { mustprogress noinline optnone }\n"));
        assert!(!needs_always_inline("define void @alwaysinline() #0 {\n}\n"));
    }

    #[test]
    fn opt_levels() {
        assert_eq!(llc_opt_level(None), "-O0");
        assert_eq!(llc_opt_level(Some("-Og")), "-O1");
        assert_eq!(llc_opt_level(Some("-Os")), "-O2");
        assert_eq!(llc_opt_level(Some("-Ofast")), "-O3");
        assert_eq!(opt_pipeline(None), None);
        assert_eq!(opt_pipeline(Some("-O0")), None);
        assert_eq!(opt_pipeline(Some("-O")), Some("default<O1>"));
        assert_eq!(opt_pipeline(Some("-O2")), Some("default<O2>"));
        assert_eq!(opt_pipeline(Some("-Oz")), Some("default<Oz>"));
        assert_eq!(opt_pipeline(Some("-O3")), Some("default<O3>"));
        assert_eq!(opt_level(None), 0);
        assert_eq!(opt_level(Some("-O0")), 0);
        assert_eq!(opt_level(Some("-O")), 1);
        assert_eq!(opt_level(Some("-Og")), 1);
        assert_eq!(opt_level(Some("-Oz")), 2);
        assert_eq!(opt_level(Some("-Ofast")), 3);
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
        let tc = searched_toolchain();
        let inputs: Vec<(usize, String)> =
            a.inputs.iter().enumerate().map(|(i, x)| (i, x.path.clone())).collect();
        let cmd = link_command(&a, &tc, &inputs);
        assert_eq!(
            cmd.args,
            ["-O2", "-lm", "a.o", "-L/x", "b.o", "-Wl,-z,now", "-static", "-o", "a.out"]
        );
    }

    #[test]
    fn link_experimental_library() {
        let a = args(&["-stdlib=libc++", "-fexperimental-library", "a.o"]);
        let tc = searched_toolchain();
        let cmd = link_command(&a, &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(cmd.args, ["-stdlib=libc++", "-fexperimental-library", "a.o", "-o", "a.out"]);
    }

    /// The host toolchain as if no `KILN_DEFAULT_GCC` were built in, so
    /// the expected link commands do not depend on the build environment.
    fn searched_toolchain() -> Toolchain {
        Toolchain {
            gcc_from_driver: false,
            ..Toolchain::new(crate::toolchain::Target::host(), None, None)
        }
    }

    #[test]
    fn link_default_gcc_install_dir() {
        let gcc = crate::toolchain::GccInstall {
            path: "/opt/gcc/lib/gcc/x86_64-redhat-linux/13".to_owned(),
            triple: "x86_64-redhat-linux".to_owned(),
            version: "13".to_owned(),
        };
        let tc = Toolchain { gcc: Some(gcc), gcc_from_driver: true, ..searched_toolchain() };
        let cmd = link_command(&args(&["a.o"]), &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(
            cmd.args,
            ["--gcc-install-dir=/opt/gcc/lib/gcc/x86_64-redhat-linux/13", "a.o", "-o", "a.out"]
        );
        // An explicit `--gcc-install-dir=` wins.
        let a = args(&["--gcc-install-dir=/x", "a.o"]);
        let cmd = link_command(&a, &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(cmd.args, ["--gcc-install-dir=/x", "a.o", "-o", "a.out"]);
    }
}
