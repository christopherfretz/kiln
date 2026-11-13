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

use kiln_codegen::{AutoVarInit, CodegenOptions, RelocModel, Visibility};
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions, Group};
use kiln_lex::Interner;
use kiln_llvm::Module;
use kiln_parse::{ParseOptions, parse_preprocessed};
use kiln_pp::{ClangCompat, PicModel};
use kiln_sema::target::Target;
use kiln_source::{SourceManager, Span};

use crate::args::{Action, Args, AsmKind, DriverMode, Input, LinkOptKind};
use crate::link::Command;
use crate::pp_cmd::{Job, PpInputs, SharedDeps, SharedSkips};
use crate::time_trace::{self, InputTrace};
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

/// True if `input` is compiled (a C or C++ source, an assembly input, or
/// any file after `-x`); everything else is a linker input, as in clang.
pub fn is_source(input: &Input) -> bool {
    if input.lang.is_some() || input.asm.is_some() || input.path == "-" {
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
            | "pcm"
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

/// The `-gsplit-dwarf` `.dwo` file of `input` (clang
/// `SplitDebugName`): the output with its extension replaced, or for a
/// link `<output>-<stem>.dwo` (`a-<stem>.dwo` without `-o`).
pub fn dwo_name(args: &Args, input: &str) -> String {
    let stem = |p: &str| {
        Path::new(p).file_stem().map_or_else(|| p.to_owned(), |s| s.to_string_lossy().into_owned())
    };
    match (Phase::of(args), &args.output) {
        (Phase::Link, o) => {
            let o = o.as_deref().unwrap_or("a");
            let base = Path::new(o).with_extension("");
            format!("{}-{}.dwo", base.display(), stem(input))
        }
        (_, Some(o)) => Path::new(o).with_extension("dwo").display().to_string(),
        (_, None) => format!("{}.dwo", stem(input)),
    }
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
    format!("kiln version {}", crate::version::VERSION)
}

/// The [`CodegenOptions`] fields set by the driver's codegen flags
/// (spec/driver.md "Codegen flags"), the rest defaulted; `cpu` is the
/// resolved `-march=` CPU ([`crate::features::TargetFeatures::cpu`]).
fn codegen_flag_options(a: &Args, cpu: Option<String>, x86: bool) -> CodegenOptions {
    let f = &a.codegen;
    CodegenOptions {
        stack_protector: f.stack_protector(),
        unwind_tables: f.unwind_tables(a.predef.freestanding.unwrap_or(false)),
        no_plt: f.no_plt,
        common: f.common,
        static_destructors: f.static_destructors,
        wrapv: f.wrapv(),
        trapv: f.trapv,
        trapv_handler: f.trapv_handler(),
        wrapv_pointer: f.wrapv_pointer(a.predef.clang_compat == ClangCompat::Clang22),
        null_pointer_is_valid: f.null_pointer_is_valid(),
        no_builtin: a.no_builtin(),
        no_builtin_fns: a.no_builtin_fns(),
        target_cpu: cpu,
        tune_cpu: f.mtune.clone().filter(|t| t != "native"),
        no_inline: a.predef.no_inline,
        code_model: f.code_model,
        hardening: f.hardening(x86),
        reflection_mangling: a.reflection_mangling,
        outline_expansions: a
            .outline_expansions
            .unwrap_or(kiln_codegen::OUTLINE_EXPANSIONS_DEFAULT),
        sanitize: a.sanitize(x86, "").0.opts,
        lto: f.lto(),
        // `opt --thinlto-bc` adds a thin unit's `EnableSplitLTOUnit` itself.
        lto_module_flags: !(a.disable_llvm_passes
            || (f.lto == Some(true)
                && matches!(Phase::of(a), Phase::Object | Phase::Bitcode | Phase::Link))),
        ..CodegenOptions::default()
    }
}

/// One source input through the frontend and IR generation.
#[derive(Debug)]
pub struct CodegenJob<'j>(pub Job<'j>);

impl CodegenJob<'_> {
    /// Runs the job: diagnostics (and the `N errors generated.` summary)
    /// to standard error. Returns the module, or `None` after an error
    /// (the error count is the second element), and the `-MD` / `-MMD`
    /// dependency list, complete whatever the errors (the third). The
    /// caller prints the module (`print_ir`): by then the AST, the
    /// Oven, the preprocessor and the source manager are gone, so the
    /// print's peak holds the module and its text only (WP-633, WP-1366).
    pub fn run(self) -> (Option<Module>, u32, Option<SharedDeps>) {
        let job = &self.0;
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let mut diag_opts = job.diag_opts.clone();
        diag_opts.pending.clear();
        let engine = make_engine(&sm, &diag_opts, job.stderr.clone(), None);
        // WP-813: a `-cc1` command line clang rejects runs no job.
        let x86 = !job.toolchain.target.is_aarch64();
        let invocation = job.args.codegen.invocation_errors(x86, job.toolchain.target.triple);
        if !invocation.is_empty() {
            for e in &invocation {
                e.emit(&engine);
            }
            engine.flush();
            return (None, engine.error_count(), None);
        }
        if let Some(s) = &job.args.pch.snapshot_use {
            match self.run_restored(s, &diag_opts) {
                Ok(r) => return r,
                Err(why) => {
                    let why = format!("not used ({why}); the header is replayed");
                    crate::pch_snapshot::note(job.args, &job.stderr, &s.pch, &why);
                }
            }
        }
        let mut module = None;
        let mut deps = None;
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                module = self.lower(&sm, &engine, &interner, inputs, &mut deps, None);
            }
        });
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        let errors = engine.error_count();
        // After an error (a `-Werror` codegen warning, say) the module is
        // dropped unprinted: nothing is written for a failed compile.
        (if errors == 0 { module } else { None }, errors, deps)
    }

    /// EXPERIMENTAL (WP-1383): [`Self::run`] with the `-include-pch`
    /// file's snapshot restored in place of a replay of its header
    /// (`crate::pch_snapshot`); `Err` (nothing reported) when it cannot
    /// be restored, and the caller replays.
    #[allow(clippy::type_complexity)]
    fn run_restored(
        &self,
        snap: &crate::args::PchSnapshotUse,
        diag_opts: &DiagOptions,
    ) -> Result<(Option<Module>, u32, Option<SharedDeps>), String> {
        let job = &self.0;
        let (sm, interner, mut restore) = crate::pch_snapshot::open(snap)?;
        let mut engine = make_engine(&sm, diag_opts, job.stderr.clone(), None);
        engine.snap_restore(&mut restore.reader).map_err(|e| e.to_string())?;
        restore.timer.mark("DiagEngine");
        let mut module = None;
        let mut deps = None;
        let mut failed = None;
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                let mut restore = Some(restore);
                module = self.lower(&sm, &engine, &interner, inputs, &mut deps, restore.as_mut());
                failed = restore.and_then(|r| r.failed);
            }
        });
        if let Some(why) = failed {
            return Err(why);
        }
        crate::pch_snapshot::prefix_reported(job.args, &job.stderr, &snap.pch, &engine);
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        let errors = engine.error_count();
        Ok((if errors == 0 { module } else { None }, errors, deps))
    }

    /// The parser's options of the job (`pch`: a declaration image to
    /// load).
    fn parse_options(&self, pch: Option<kiln_parse::PchLoad>) -> ParseOptions {
        let job = &self.0;
        let a = job.args;
        let x86 = !job.toolchain.target.is_aarch64();
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        ParseOptions {
            exceptions: a.predef.exceptions.unwrap_or(job.std.cplusplus),
            rtti: a.predef.rtti.unwrap_or(job.std.cplusplus),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            reflection: a.predef.reflection_for(&job.std),
            coro_aligned_allocation: a.predef.coro_aligned_allocation,
            lax_vector_conversions: a.lax_vector_conversions,
            clang_compat: a.predef.clang_compat.major(),
            gnu_lib_builtins: a.stdlib.unwrap_or_default() == StdLib::LibStdCxx,
            no_builtin: a.no_builtin(),
            no_builtin_fns: a.no_builtin_fns(),
            freestanding: a.predef.freestanding.unwrap_or(false),
            sized_deallocation: a.predef.sized_deallocation.unwrap_or(true),
            char8: a.predef.char8_t,
            math_errno: a.fp_model().opts.math_errno,
            sve: feats.sve,
            sve_vector_bits: feats.sve_vector_bits,
            aarch64_cpu_valid: crate::features::aarch64_attr_cpu_valid,
            cfg_max_elems: a.cfg_max_elems.unwrap_or(kiln_sema::cfg::DEFAULT_MAX_ELEMS),
            permissive: a.permissive,
            contract_semantic: a.contract_semantic.unwrap_or_default(),
            cf_protection_branch: a.codegen.cf_protection_branch(x86),
            record_carried: a.bmi_kind != Some(1) && job.module_output_path().is_some(),
            record_image: a.bmi_image
                && a.bmi_kind != Some(1)
                && job.module_output_path().is_some(),
            image_census: a.bmi_image_census,
            gnu89_inline: a.predef.gnu89_inline_for(&job.std),
            pch,
            ..job.std.parse_options()
        }
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
        restore: Option<&mut crate::pch_snapshot::Restore<'_>>,
    ) -> Option<Module> {
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
        // WP-813: the flags this target rejects end the `-cc1` job.
        let x86 = !job.toolchain.target.is_aarch64();
        let target_errors = a.codegen.target_errors(x86);
        if !target_errors.is_empty() {
            for e in &target_errors {
                e.emit(engine);
            }
            return None;
        }
        // WP-630: `cx16` makes 128-bit atomics lock-free (clang
        // `X86TargetInfo::setMaxAtomicWidth`).
        if crate::features::resolve(a, &job.toolchain.target).0.x86_cx16() {
            target.types.max_atomic_inline_bytes = 16;
        }
        let mut inputs = inputs;
        let skipped = a.coverage.mapping().then(SharedSkips::default);
        inputs.skipped.clone_from(&skipped);
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        let (mut tu, mut pp, deps, main_id) = if let Some(restore) = restore {
            // EXPERIMENTAL (WP-1383): the prefix's state restored.
            let (sema, pp, deps, popts) =
                job.restore_front(sm, engine, interner, inputs, restore, &target, |o| {
                    self.parse_options(o)
                })?;
            let main_id = pp.main_file();
            let (tu, pp) = kiln_parse::continue_preprocessed(sema, pp, &target, &popts);
            restore.timer.mark("continue");
            restore.timer.report();
            (tu, pp, deps, main_id)
        } else {
            let (pp, deps) = job.build_preprocessor(sm, engine, interner, inputs, true)?;
            let main_id = pp.main_file();
            let pch = crate::pch_image::sema_load(a, &pp, sm, engine).ok()?;
            let popts = self.parse_options(pch);
            let limits = a.constexpr.limits();
            let (tu, pp) = parse_preprocessed(
                pp,
                Arc::clone(interner),
                engine,
                target.clone(),
                &popts,
                limits,
            );
            (tu, pp, deps, main_id)
        };
        job.note_image_imports(&pp);
        if engine.error_count() == 0 {
            job.write_module_file(&mut pp, engine, tu.carried.as_ref(), tu.image.as_ref());
        }
        crate::pch_image::finish(a, &mut pp, sm, interner, engine, None);
        *deps_out = crate::pp_cmd::finish_deps(pp, deps);
        let mut module = None;
        if engine.error_count() == 0 {
            let fp = a.fp_model();
            let mut opts = CodegenOptions {
                source_filename: if job.path == "-" { "-".to_owned() } else { job.path.to_owned() },
                c: job.std.c,
                gnu89_inline: a.predef.gnu89_inline_for(&job.std),
                exceptions: a.predef.exceptions.unwrap_or(job.std.cplusplus),
                rtti: a.predef.rtti.unwrap_or(job.std.cplusplus),
                ident: ident(),
                outline_atomics: a
                    .outline_atomics
                    .or_else(|| job.toolchain.aarch64_outline_atomics_default().then_some(true)),
                target_features: feats.extra.clone(),
                target_cpu_features: feats.x86_cpu_features.clone(),
                target_features_written: feats.x86_written.clone(),
                target_attr: Some(if job.toolchain.target.is_aarch64() {
                    crate::features::target_attr_parser as kiln_codegen::TargetAttrParser
                } else {
                    crate::features::x86_target_attr_features
                }),
                // clang 19's `vscale_range` also needs NEON (its `SveMode`).
                sve: feats.sve
                    && (a.predef.clang_compat == ClangCompat::Clang22
                        || feats.enabled().iter().any(|f| f == "neon")),
                sve_vector_bits: feats.sve_vector_bits,
                complex_range: a.complex_range(),
                fp: fp.opts,
                fp_denormal: fp.denormal,
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
                debug: kiln_codegen::DebugOptions {
                    main_file: if job.path == "-" { String::new() } else { job.path.to_owned() },
                    main_id,
                    language: kiln_codegen::DebugOptions::language_for(
                        job.std.cplusplus,
                        job.std.lex >= kiln_lex::Std::Cxx11,
                        job.std.lex >= kiln_lex::Std::Cxx14,
                    ),
                    ..a.debug.options(
                        &std::env::current_dir()
                            .map(|d| d.display().to_string())
                            .unwrap_or_default(),
                        a.debug.split().then(|| dwo_name(a, job.path)),
                    )
                },
                ..codegen_flag_options(a, feats.cpu.clone(), x86)
            };
            // WP-941: coverage, and the debug locations gcov needs (clang
            // `LocTrackingOnly` when no `-g` is given).
            opts.coverage = coverage_options(a, job.path);
            if let Some(s) = &skipped {
                let s = s.take();
                opts.coverage.skipped = s.skipped;
                opts.coverage.expanded = s.expanded;
            }
            if opts.coverage.gcov.is_some() && opts.debug.kind == kiln_codegen::DebugInfoKind::None
            {
                opts.debug.kind = kiln_codegen::DebugInfoKind::LocTrackingOnly;
            }
            // UBSan's `shift-base` depends on the language (WP-675).
            opts.sanitize.cplusplus = job.std.cplusplus;
            opts.sanitize.cxx20 = job.std.cplusplus && job.std.lex >= kiln_lex::Std::Cxx20;
            // WP-1032: the ignorelists the driver validated.
            opts.ignorelist = a.sanitize.ignorelist(
                opts.sanitize.kinds,
                &job.toolchain.resource_dir,
                opts.clang_compat >= 22,
            );
            // Sema sets the level on each evaluation it runs; a unit that
            // ran none still folds at its level (WP-416).
            tu.oven.set_clang_compat(opts.clang_compat);
            // The ICE breadcrumb (spec/driver.md "Internal compiler errors").
            let phase = kiln_source::activity::enter(
                kiln_source::activity::Phase::Codegen,
                kiln_source::Loc::INVALID,
            );
            // spec/driver.md "Time trace": `EmitIR` is the module's
            // construction, `PrintIR` (the caller's) its text.
            let emit = kiln_source::trace::Phase::new("EmitIR");
            let (r, warnings) =
                kiln_codegen::emit_with_debug(&mut tu.ast, &mut tu.oven, &target, &opts, Some(sm));
            drop(emit);
            drop(phase);
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
        job.write_image_reports(sm, tu.image_census.as_ref(), engine);
        a.constexpr.write_report(&tu.oven, sm, engine);
        let mut err = job.stderr.clone();
        a.constexpr.write_profile(&tu.oven, &tu.ast, sm, main_id, engine, &mut err);
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
        // IR text for a big unit: the module leaves `lower` only once the
        // AST is gone, and the caller prints it after `run` has dropped the
        // preprocessor and the source manager too, so the peak holds two
        // of the three, not all of them (WP-633, WP-1366).
        drop(tu);
        module
    }
}

/// Prints a module the codegen job returned, consuming it (its text and
/// the module are never kept side by side past the print), under the
/// `PrintIR` time-trace event (spec/driver.md "Time trace", WP-1366).
pub(crate) fn print_ir(module: Module) -> String {
    let print = kiln_source::trace::Phase::new("PrintIR");
    let text = module.to_text();
    drop(print);
    drop(module);
    text
}

/// The codegen coverage options of the compile of `input` (WP-941).
fn coverage_options(a: &Args, input: &str) -> kiln_codegen::CoverageOptions {
    let c = &a.coverage;
    let cwd = std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default();
    let gcov = c.gcov().then(|| {
        let (notes, data) =
            c.gcov_files(input, a.output.as_deref(), Phase::of(a) == Phase::Link, &cwd);
        (
            if c.gcov_notes() { notes } else { String::new() },
            if c.gcov_data() { data } else { String::new() },
        )
    });
    kiln_codegen::CoverageOptions {
        gcov,
        instr: c.instr_generate.is_some(),
        // clang's InstrProfiling pass creates the filename variable, so
        // `-disable-llvm-passes` IR has none.
        instr_path: c
            .instr_generate
            .clone()
            .flatten()
            .filter(|p| !p.is_empty() && !a.disable_llvm_passes),
        mapping: c.mapping(),
        comp_dir: c.comp_dir.clone().unwrap_or(cwd),
        prefix_map: c.prefix_map.clone(),
        skipped: Vec::new(),
        expanded: Vec::new(),
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
    /// Run the frontend on an input; the IR goes to `dest`. With `hod`
    /// (`-fkiln-backend=hod`, spec/driver.md "Back-end selection") the
    /// module goes to that command first, when the route predicate
    /// holds ([`hod_takes`]); the `opt` runs and the next `llc` step are
    /// the fallback.
    Frontend { input: &'a Input, dest: Dest, hod: Option<Command> },
    /// An external command (`llc`, `llvm-as`, `clang++`), with the IR of
    /// the preceding frontend step on its standard input if `piped`.
    External { cmd: Command, piped: bool },
    /// Preprocess an assembler-with-cpp input (`.S`) into `dest` (`-` is
    /// standard output), as `-E` would (WP-1331).
    Preprocess { input: &'a Input, dest: String },
    /// Assemble (`clang++ -c -x assembler`); skipped when `after_pp` and
    /// the preceding [`Step::Preprocess`] failed.
    Assemble { cmd: Command, after_pp: bool },
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
            warn_unused_link_inputs(args, inputs, phase, &drv);
        }
    }
    if check_asm_opts(args, phase, sources, &drv) {
        drv.flush();
        return 1;
    }
    // spec/driver.md "Back-end selection": an executable is required
    // only where the hod route is planned.
    let hod = if hod_planned(args, tc) {
        match crate::hod::find(args, tc) {
            Ok(p) => Some(p),
            Err(searched) => {
                Diag::new(DiagId::drv_hod_not_found, Span::INVALID).arg(searched).emit(&drv);
                drv.flush();
                return 1;
            }
        }
    } else {
        None
    };
    let temp = TempDir::new();
    let plan = plan(args, tc, inputs, phase, &temp, hod.as_deref());
    if args.deps.cdb_fragment.is_some() {
        let jobs: Vec<_> = inputs
            .iter()
            .enumerate()
            .filter(|(_, i)| is_source(i))
            .map(|(n, &input)| {
                // As `plan`: a link compiles each source to a temporary
                // object.
                let output = if phase == Phase::Link {
                    temp.file(&format!("{n}-{}", default_output(&input.path, Phase::Object)))
                } else {
                    args.output.clone().unwrap_or_else(|| default_output(&input.path, phase))
                };
                crate::cdb::CdbJob { input, output: Some(output) }
            })
            .collect();
        crate::cdb::write(args, tc, &jobs, &drv);
    }
    if args.print_commands {
        print_version(&mut io.stderr, tc, false);
        let mut err = io.stderr.clone();
        for step in &plan {
            let _ = err.write_all(render(step, true).as_bytes());
            // The `opt` runs `execute` makes after the frontend, as `-v`
            // prints them (spec/driver.md "`-###`").
            if let Step::Frontend { hod, .. } = step {
                for cmd in planned_ir_passes(args, tc).unwrap_or_default() {
                    let _ = err.write_all(cmd.render(true).as_bytes());
                }
                // The hod run, then a marker: the next `llc` is its
                // fallback (spec/driver.md "Back-end selection").
                if let Some(h) = hod {
                    let _ = err.write_all(h.render(true).as_bytes());
                    let _ = err.write_all(HOD_FALLBACK_MARK.as_bytes());
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

/// The `-Wa,` / `-Xassembler` checks (spec/driver.md "Assembler
/// options", WP-1288): clang checks the values only when a source is
/// assembled (`-c`, or a link); `-S` and `-emit-llvm` never look at them.
/// Without a source they are unused: `-Wa,` always, `-Xassembler` only
/// under `-c` (clang's link job claims it silently). Returns true on an
/// error.
fn check_asm_opts(args: &Args, phase: Phase, sources: usize, drv: &DiagEngine<'_>) -> bool {
    if args.asm.is_empty() || !matches!(phase, Phase::Object | Phase::Link) {
        return false;
    }
    if sources > 0 {
        let (errors, _) = args.asm_check();
        for e in &errors {
            crate::emit_arg_error(e, drv);
        }
        return !errors.is_empty();
    }
    if !args.quiet_unused_arguments {
        for o in args.asm.iter().filter(|o| phase == Phase::Object || o.opt == "-Wa,") {
            Diag::new(DiagId::drv_argument_unused, Span::INVALID).arg(&o.arg).emit(drv);
        }
    }
    false
}

/// `<x>: 'linker' input unused` and `argument unused during compilation`
/// for a compile-only run, in command-line order.
fn warn_unused_link_inputs(args: &Args, inputs: &[&Input], phase: Phase, drv: &DiagEngine<'_>) {
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
                LinkOptKind::Xlinker | LinkOptKind::Rpath => {
                    let opt = o.kind.separate().unwrap_or_default();
                    Diag::new(DiagId::drv_linker_input_unused, Span::INVALID)
                        .arg(format!("{opt} {}", o.arg))
                        .emit(drv);
                }
            }
        }
    };
    // `inputs` holds only the existing inputs; positions count all of them.
    for (pos, input) in args.inputs.iter().enumerate() {
        report_opts(pos);
        let existing = inputs.iter().any(|i| std::ptr::eq(*i, input));
        if existing && !is_source(input) {
            Diag::new(DiagId::drv_linker_input_unused, Span::INVALID).arg(&input.path).emit(drv);
        }
        // `-S` (and `-S -emit-llvm`) stop before a plain `.s` is
        // assembled: clang `'assembler' input unused`.
        if existing
            && input.asm == Some(AsmKind::Plain)
            && matches!(phase, Phase::IrText | Phase::Assembly)
        {
            Diag::new(DiagId::drv_input_unused, Span::INVALID)
                .arg(&input.path)
                .arg(AsmKind::Plain.type_name())
                .emit(drv);
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

    /// Six hex digits unique to this run and input `n`, as the random
    /// part of clang's temporary file names (`x-1a2b3c.o`).
    fn suffix(&self, n: usize) -> String {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.path.hash(&mut h);
        n.hash(&mut h);
        format!("{:06x}", h.finish() & 0xff_ffff)
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

/// The steps for `inputs`, in order. `hod`: the hod executable, when
/// the hod route is planned ([`hod_planned`]).
fn plan<'a>(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&'a Input],
    phase: Phase,
    temp: &TempDir,
    hod: Option<&Path>,
) -> Vec<Step<'a>> {
    let mut steps = Vec::new();
    let mut link_inputs: Vec<(usize, String)> = Vec::new();
    for (n, input) in inputs.iter().enumerate() {
        let pos = args.inputs.iter().position(|i| std::ptr::eq(i, *input)).unwrap_or(n);
        if !is_source(input) {
            link_inputs.push((pos, input.path.clone()));
            continue;
        }
        if let Some(kind) = input.asm {
            if let Some(obj) = plan_asm(args, tc, input, kind, phase, temp, n, &mut steps) {
                link_inputs.push((pos, obj));
            }
            continue;
        }
        let out = || args.output.clone().unwrap_or_else(|| default_output(&input.path, phase));
        match phase {
            Phase::IrText => {
                steps.push(Step::Frontend { input, dest: Dest::File(out()), hod: None });
            }
            // WP-873: an LTO object is the pre-link pipeline's bitcode.
            Phase::Bitcode | Phase::Object if args.codegen.lto.is_some() => {
                steps.push(Step::Frontend { input, dest: Dest::Pipe, hod: None });
                steps.push(Step::External { cmd: lto_writer(args, tc, out()), piped: true });
            }
            Phase::Assembly if args.codegen.lto.is_some() => {
                steps.push(Step::Frontend { input, dest: Dest::File(out()), hod: None });
            }
            Phase::Bitcode => {
                steps.push(Step::Frontend { input, dest: Dest::Pipe, hod: None });
                let mut cmd = Command::new(tc, Tool::LlvmAs);
                cmd.arg("-o").arg(out()).arg("-");
                steps.push(Step::External { cmd, piped: true });
            }
            Phase::Assembly | Phase::Object => {
                let cmd = llc(args, tc, phase, out(), &input.path);
                let hod =
                    hod.filter(|_| phase == Phase::Object).map(|h| crate::hod::command(h, &cmd));
                steps.push(Step::Frontend { input, dest: Dest::Pipe, hod });
                steps.push(Step::External { cmd, piped: true });
                // `-gz`: `llc` cannot compress, so the object's debug
                // sections are compressed in place after it.
                if phase == Phase::Object
                    && args.debug.kind() != kiln_codegen::DebugInfoKind::None
                    && let Some(z) = args.debug.compression()
                {
                    let mut cmd = Command::new(tc, Tool::Objcopy);
                    cmd.arg(format!("--compress-debug-sections={}", z.name())).arg(out());
                    steps.push(Step::External { cmd, piped: false });
                }
            }
            Phase::Link => {
                let stem = default_output(&input.path, Phase::Object);
                let obj = temp.file(&format!("{n}-{stem}"));
                let cmd = if args.codegen.lto.is_some() {
                    lto_writer(args, tc, obj.clone())
                } else {
                    llc(args, tc, phase, obj.clone(), &input.path)
                };
                let hod =
                    hod.filter(|_| cmd.tool == Tool::Llc).map(|h| crate::hod::command(h, &cmd));
                steps.push(Step::Frontend { input, dest: Dest::Pipe, hod });
                steps.push(Step::External { cmd, piped: true });
                link_inputs.push((pos, obj));
            }
        }
    }
    if phase == Phase::Link {
        steps.push(Step::External { cmd: link_command(args, tc, &link_inputs), piped: false });
    }
    steps
}

/// The steps of an assembly input (spec/driver.md "Assembler inputs",
/// WP-1331), as clang's phases: a `.S` is preprocessed (to its output
/// under `-S` and `-S -emit-llvm`, standard output by default, else to a
/// temporary `.s`), then assembled to the object (`-c`, also with
/// `-emit-llvm` or `-flto`) or, for a link, a temporary object, which is
/// returned. A plain `.s` has no step under `-S` (`'assembler' input
/// unused`, [`warn_unused_link_inputs`]).
#[allow(clippy::too_many_arguments)]
fn plan_asm<'a>(
    args: &Args,
    tc: &Toolchain,
    input: &'a Input,
    kind: AsmKind,
    phase: Phase,
    temp: &TempDir,
    n: usize,
    steps: &mut Vec<Step<'a>>,
) -> Option<String> {
    let stem = Path::new(&input.path)
        .file_stem()
        .map_or_else(|| input.path.clone(), |s| s.to_string_lossy().into_owned());
    if matches!(phase, Phase::IrText | Phase::Assembly) {
        if kind == AsmKind::Cpp {
            let dest = args.output.clone().unwrap_or_else(|| "-".to_owned());
            steps.push(Step::Preprocess { input, dest });
        }
        return None;
    }
    let source = if kind == AsmKind::Cpp {
        let s = temp.file(&format!("{n}-{stem}.s"));
        steps.push(Step::Preprocess { input, dest: s.clone() });
        s
    } else {
        input.path.clone()
    };
    let (out, linked) = if phase == Phase::Link {
        (temp.file(&format!("{n}-{stem}.o")), true)
    } else {
        (args.output.clone().unwrap_or_else(|| default_output(&input.path, Phase::Object)), false)
    };
    let cmd = assembler(args, tc, &source, out.clone());
    steps.push(Step::Assemble { cmd, after_pp: kind == AsmKind::Cpp });
    linked.then_some(out)
}

/// The integrated assembler on `source` (the input, or its preprocessed
/// text), writing the object `out`: `clang++ -c -x assembler` of the
/// selected LLVM installation, with the target, `-march=` / `-mcpu=`,
/// the `-I` directories (`.include`), `-g`, `-gz`, `-fno-pic` and the
/// `-Wa,` / `-Xassembler` options, as clang's `cc1as` job receives them.
fn assembler(args: &Args, tc: &Toolchain, source: &str, out: String) -> Command {
    let mut cmd = Command::new(tc, Tool::Assembler);
    cmd.arg(format!("--target={}", tc.target.triple));
    if let Some(m) = &args.march {
        cmd.arg(format!("-march={m}"));
    }
    if let Some(m) = &args.mcpu {
        cmd.arg(format!("-mcpu={m}"));
    }
    for (g, d) in &args.dirs {
        if *g == kiln_pp::DirGroup::Angled {
            cmd.arg(format!("-I{d}"));
        }
    }
    if let Some(v) = args.debug.dwarf_version() {
        cmd.arg("-g").arg(format!("-gdwarf-{v}"));
    }
    if let Some(z) = args.debug.compression() {
        cmd.arg(format!("-gz={}", z.name()));
    }
    if args.predef.pic == Some(PicModel::None) {
        cmd.arg("-fno-pic");
    }
    for o in &args.asm {
        if o.opt == "-Wa," {
            cmd.arg(o.arg.clone());
        } else {
            for v in &o.values {
                cmd.arg("-Xassembler").arg(v.clone());
            }
        }
    }
    cmd.arg("-Qunused-arguments").arg("-c").arg("-x").arg("assembler").arg(source);
    cmd.arg("-o").arg(out);
    cmd
}

/// The module text before its metadata section, which LLVM's printer
/// (and kiln's) writes after every declaration and attribute group: the
/// checks of [`ir_passes`] look only at those, and a `-g` module's
/// metadata is most of its text (WP-817). A top-level line opening with
/// `!` starts the section; no other line does (instructions are
/// indented and a string's newline is escaped).
pub(crate) fn code_part(ir: &str) -> &str {
    if ir.starts_with('!') {
        return "";
    }
    ir.find("\n!").map_or(ir, |i| &ir[..=i])
}

/// Whether the IR text needs the coroutine lowering: it declares an
/// `llvm.coro.*` intrinsic (clang's `CoroConditionalWrapper` test,
/// `coro::declaresAnyIntrinsic`). Every `presplitcoroutine` function
/// calls `llvm.coro.id`; a `__builtin_coro_noop` alone needs it too
/// (spec/driver.md "IR optimization").
pub fn needs_coro_lowering(ir: &str) -> bool {
    ir.lines().any(|l| l.starts_with("declare ") && l.contains(" @llvm.coro."))
}

/// Whether the IR has an `alwaysinline` attribute group: on a definition
/// (an `always_inline` function) or on a call site (kiln marks the calls
/// of a `[[clang::always_inline]]` statement and those inside a
/// `flatten` function, `kiln_codegen` `stmt_call_attrs`). clang's O0
/// pipeline (`AlwaysInlinerPass`) inlines every such call and every
/// call to such a function, deleting a discardable one left unused,
/// before `-emit-llvm` writes the module.
pub fn needs_always_inline(ir: &str) -> bool {
    ir.lines().any(|l| {
        l.starts_with("attributes #") && l.split_ascii_whitespace().any(|w| w == "alwaysinline")
    })
}

/// The `opt` runs between the frontend and the IR's consumer, if any:
/// at `-O1` and above the default pipeline of the level (which places
/// `CoroEarly`, `CoroSplit`, `CoroElide` and `CoroCleanup` where clang's does);
/// at `-O0` clang's whole O0 pipeline when the module has an
/// `alwaysinline` definition or call site (`AlwaysInlinerPass`, then
/// the coroutine lowering), else clang's O0 coroutine lowering (`CoroConditionalWrapper` in
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
    if args.codegen.lto.is_some() {
        return lto_passes(args, tc);
    }
    if args.disable_llvm_passes {
        return None;
    }
    let ir = code_part(ir);
    let passes = match opt_pipeline(args.opt_flag.as_deref()) {
        Some(p) => p,
        // WP-675: the sanitizer passes run at every level.
        None if sanitizer_passes(args, tc).is_some() => "default<O0>",
        // WP-941: so do the coverage passes.
        None if coverage_passes(args).is_some() => "default<O0>",
        None if needs_always_inline(ir) => "default<O0>",
        // WP-813: the O0 pipeline's `ee-instrument` turns the entry and
        // exit attributes into `__cyg_profile_func_*` calls.
        None if ir.contains("\"instrument-function-entry") => "default<O0>",
        None if needs_coro_lowering(ir) => "coro-early,cgscc(coro-split),coro-cleanup,globaldce",
        None => return None,
    };
    Some(opt_commands(args, tc, passes))
}

/// The conditions that make [`ir_passes`] schedule `opt` for the IR
/// text `ir`, each by its stable name (spec/driver.md "Compile
/// statistics log"): every one that holds, in this order, any one of
/// which alone schedules it; empty when `opt` does not run. Only the
/// statistics log asks (it walks the text once more).
pub(crate) fn opt_triggers(args: &Args, tc: &Toolchain, ir: &str) -> Vec<&'static str> {
    if args.disable_llvm_passes {
        return Vec::new();
    }
    let ir = code_part(ir);
    let checks = [
        ("opt-level", opt_pipeline(args.opt_flag.as_deref()).is_some()),
        ("lto", args.codegen.lto.is_some()),
        ("sanitizer", sanitizer_passes(args, tc).is_some()),
        ("coverage", coverage_passes(args).is_some()),
        ("alwaysinline", needs_always_inline(ir)),
        ("instrument-entry", ir.contains("\"instrument-function-entry")),
        ("coro", needs_coro_lowering(ir)),
    ];
    checks.into_iter().filter(|(_, on)| *on).map(|(n, _)| n).collect()
}

/// The static half of the hod route's predicate (spec/driver.md
/// "Back-end selection"): the back end is hod, the output an object
/// (`-c`, or the objects of a link), at `-O0`, without `-flto`, a
/// sanitizer, coverage or instrumentation (`-finstrument-functions*`,
/// `-fxray-instrument`, `-fpatchable-function-entry=`). Every other run
/// takes the LLVM route untouched; `-fkiln-backend=hod` is a request for
/// the cases hod can take, not an error elsewhere.
fn hod_planned(args: &Args, tc: &Toolchain) -> bool {
    let c = &args.codegen;
    crate::hod::selected(args).unwrap_or_default() == crate::hod::Backend::Hod
        && matches!(Phase::of(args), Phase::Object | Phase::Link)
        && c.lto.is_none()
        && opt_pipeline(args.opt_flag.as_deref()).is_none()
        && !args.sanitize(!tc.target.is_aarch64(), tc.target.triple).0.opts.any()
        && coverage_passes(args).is_none()
        && !c.instrument_functions
        && !c.instrument_functions_after_inlining
        && !c.xray_instrument
        && c.patchable_function_entry.is_none()
}

/// The dynamic half of the hod route's predicate: the `opt` trigger
/// set of the module ([`opt_triggers`], read from the text that goes to
/// hod's standard input) is empty or exactly {`alwaysinline`}. A
/// coroutine module (`coro`) or an instrumented one takes the LLVM route.
fn hod_takes(args: &Args, tc: &Toolchain, ir: &str) -> bool {
    opt_triggers(args, tc, ir).iter().all(|t| *t == "alwaysinline")
}

/// `-###`'s line between the hod command and the `llc` command that is
/// its fallback.
const HOD_FALLBACK_MARK: &str =
    " (fallback if hod declines or fails: opt as the module needs, then llc)\n";

/// The `opt` runs `-###` prints after a frontend step: those of an `-O1`
/// or higher level, whose pipeline does not depend on the IR, and those
/// of a sanitizer build. Otherwise at `-O0` the runs depend on what the
/// IR declares ([`ir_passes`]) and are not shown (spec/driver.md "IR
/// optimization").
fn planned_ir_passes(args: &Args, tc: &Toolchain) -> Option<Vec<Command>> {
    if args.codegen.lto.is_some() {
        return lto_passes(args, tc);
    }
    if args.disable_llvm_passes {
        return None;
    }
    opt_pipeline(args.opt_flag.as_deref())
        .or_else(|| sanitizer_passes(args, tc).map(|_| "default<O0>"))
        .or_else(|| coverage_passes(args).map(|_| "default<O0>"))
        .map(|p| opt_commands(args, tc, p))
}

/// WP-873: under `-flto[=thin]` clang runs the level's LTO pre-link
/// pipeline (`buildLTOPreLinkDefaultPipeline` /
/// `buildThinLTOPreLinkDefaultPipeline`, at `-O0` too), and a textual
/// output carries the module summary (`PrintModulePass` with
/// `EmitLTOSummary`). For a bitcode output the summary is written by
/// [`lto_writer`]. `-disable-llvm-passes` runs no pipeline and writes no
/// summary.
fn lto_passes(args: &Args, tc: &Toolchain) -> Option<Vec<Command>> {
    let text = matches!(Phase::of(args), Phase::IrText | Phase::Assembly);
    if args.disable_llvm_passes {
        return None;
    }
    let level = opt_pipeline(args.opt_flag.as_deref()).unwrap_or("default<O0>");
    let prefix = if args.codegen.lto == Some(true) { "thinlto-pre-link<" } else { "lto-pre-link<" };
    let passes = level.replacen("default<", prefix, 1);
    let mut cmds = opt_commands(args, tc, &passes);
    if text && let Some(last) = cmds.last_mut() {
        // Before the trailing `-o - -`.
        let at = last.args.len().saturating_sub(3);
        last.args.insert(at, "-module-summary".to_owned());
    }
    Some(cmds)
}

/// WP-873: the `opt` run writing an LTO object (`-c -flto`, or one
/// before an LTO link) from the pre-link pipeline's bitcode: full LTO's
/// with its module summary (`BitcodeWriterPass` with `EmitLTOSummary`),
/// thin LTO's through `ThinLTOBitcodeWriterPass` (`--thinlto-bc`), as
/// clang's `-flto` `-c`. Under `-disable-llvm-passes`, `llvm-as`.
fn lto_writer(args: &Args, tc: &Toolchain, out: String) -> Command {
    if args.disable_llvm_passes {
        let mut cmd = Command::new(tc, Tool::LlvmAs);
        cmd.arg("-o").arg(out).arg("-");
        return cmd;
    }
    let mut cmd = Command::new(tc, Tool::Opt);
    let keep_names = args.sanitize(!tc.target.is_aarch64(), tc.target.triple).0.opts.has(
        kiln_ast::sanitize::SanitizerSet(
            kiln_ast::sanitize::SanitizerSet::ADDRESS.0
                | kiln_ast::sanitize::SanitizerSet::MEMORY.0,
        ),
    );
    if !keep_names {
        cmd.arg("-discard-value-names");
    }
    if args.codegen.lto == Some(true) {
        cmd.arg("--thinlto-bc");
        if args.codegen.split_lto_unit == Some(true) {
            cmd.arg("--thinlto-split-lto-unit");
        }
    } else {
        cmd.arg("-module-summary");
    }
    cmd.arg("-o").arg(out).arg("-");
    cmd
}

/// clang's `addSanitizers` (WP-675, docs/design/sanitizers.md §1.3): the
/// `OptimizerLast` passes and their `cl::opt` flags.
fn sanitizer_passes(args: &Args, tc: &Toolchain) -> Option<(String, Vec<String>)> {
    args.sanitize(!tc.target.is_aarch64(), tc.target.triple)
        .0
        .optimizer_last(opt_level(args.opt_flag.as_deref()))
}

/// clang's coverage passes (WP-941, spec/codegen.md "Instrumentation"):
/// `GCOVProfilerPass` then `InstrProfilingLoweringPass` at the pipeline
/// start extension point, and their `cl::opt` flags.
fn coverage_passes(args: &Args) -> Option<(String, Vec<String>)> {
    let c = &args.coverage;
    let mut passes = Vec::new();
    let mut flags = Vec::new();
    if c.gcov() {
        passes.push("insert-gcov-profiling");
        if c.atomic {
            flags.push("--gcov-atomic-counter".to_owned());
        }
    }
    if c.instr_generate.is_some() {
        passes.push("instrprof");
        if c.atomic {
            flags.push("--instrprof-atomic-counter-update-all".to_owned());
        }
    }
    (!passes.is_empty()).then(|| (passes.join(","), flags))
}

/// The `opt` run of `passes`, then for a textual or bitcode output the
/// run that discards local value names ([`ir_passes`]). With a sanitizer
/// the instrumentation joins the pipeline at its last extension point;
/// under address or memory clang keeps the value names (its
/// `-discard-value-names` is off), so there is no second run.
fn opt_commands(args: &Args, tc: &Toolchain, passes: &str) -> Vec<Command> {
    let mut run = Command::new(tc, Tool::Opt);
    run.arg(format!("-passes={passes}"));
    if let Some((ep, cl)) = coverage_passes(args) {
        run.arg(format!("--passes-ep-pipeline-start={ep}"));
        for c in cl {
            run.arg(c);
        }
    }
    let san = sanitizer_passes(args, tc);
    if let Some((ep, cl)) = &san {
        run.arg(format!("--passes-ep-optimizer-last={ep}"));
        for c in cl {
            run.arg(c.clone());
        }
    }
    let keep_names = args.sanitize(!tc.target.is_aarch64(), tc.target.triple).0.opts.has(
        kiln_ast::sanitize::SanitizerSet(
            kiln_ast::sanitize::SanitizerSet::ADDRESS.0
                | kiln_ast::sanitize::SanitizerSet::MEMORY.0,
        ),
    );
    run.arg("-o").arg("-").arg("-");
    if matches!(Phase::of(args), Phase::Object | Phase::Link)
        || (args.codegen.lto.is_some() && Phase::of(args) == Phase::Bitcode)
    {
        return vec![run];
    }
    if keep_names {
        run.arg("-S");
        return vec![run];
    }
    let mut names = Command::new(tc, Tool::Opt);
    names.arg("-discard-value-names").arg("-S").arg("-o").arg("-").arg("-");
    vec![run, names]
}

/// `llc` for an assembly or object output.
fn llc(args: &Args, tc: &Toolchain, phase: Phase, out: String, input: &str) -> Command {
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
    // clang `AllowFPOpFusion`: `-ffp-contract=fast` (not
    // `fast-honor-pragmas`) fuses across statements in the backend too;
    // the other fast-math options reach it as function attributes. LLVM
    // 22's `llc` warns that the X86 backend ignores `-fp-contract` (it
    // fuses on the IR's `contract` flags, which every FP operation carries
    // under `-ffp-contract=fast`, as with clang), so it is not passed
    // there; the object code is the same (WP-1016).
    let x86_ignores_fusion = !tc.target.is_aarch64() && tc.llvm_major() >= 22;
    if args.fp_model().backend_fuse && !x86_ignores_fusion {
        cmd.arg("-fp-contract=fast");
    }
    // The module's `Code Model` flag says the same; explicit, as clang
    // hands the model to the target machine.
    if let Some(cm) = args.codegen.code_model {
        cmd.arg(format!("-code-model={}", cm.name()));
    }
    // clang `addX86AlignBranchArgs` (WP-961): the assembler pads branches
    // so none crosses or ends on a 32-byte boundary. Off x86 the option is
    // a driver error (`features::resolve`).
    if args.codegen.branches_within_32b && !tc.target.is_aarch64() {
        cmd.arg("-x86-branches-within-32B-boundaries");
    }
    // spec/driver.md "Debug information": clang's debugger tuning, and
    // `-gsplit-dwarf`'s `.dwo` file.
    if args.debug.kind() != kiln_codegen::DebugInfoKind::None {
        cmd.arg("-debugger-tune=gdb");
        if args.debug.split() {
            let dwo = dwo_name(args, input);
            cmd.arg(format!("-split-dwarf-file={dwo}"));
            if phase != Phase::Assembly {
                cmd.arg(format!("-split-dwarf-output={dwo}"));
            }
        }
    }
    cmd.arg("-o").arg(out).arg("-");
    cmd
}

/// `clang++` over the objects and linker inputs (`(position, path)`),
/// with the linker options in command-line order.
/// The flags that pick the linking clang++'s target and installation
/// (also passed with the `-print-*` probes it answers, WP-920).
pub(crate) fn toolchain_link_args(args: &Args, tc: &Toolchain) -> Vec<String> {
    let mut v = Vec::new();
    if args.target.is_some() {
        v.push(format!("--target={}", tc.target.triple));
    }
    if let Some(s) = &args.sysroot {
        v.push(format!("--sysroot={s}"));
    }
    if let Some(d) = &args.gcc_toolchain {
        v.push(format!("--gcc-toolchain={d}"));
    }
    if let Some(d) = &args.gcc_install_dir {
        v.push(format!("--gcc-install-dir={d}"));
    } else if let Some(g) = tc.gcc.as_ref().filter(|_| tc.gcc_from_driver) {
        // The headers came from `toolchain::DEFAULT_GCC`'s installation;
        // link against the same one rather than clang++'s own choice.
        v.push(format!("--gcc-install-dir={}", g.path));
    }
    v
}

fn link_command(args: &Args, tc: &Toolchain, inputs: &[(usize, String)]) -> Command {
    let mut cmd = Command::new(tc, Tool::Linker);
    // WP-1282: the C driver (`kiln`, `--driver-mode=gcc`) links as `clang`
    // does, without the C++ standard library (no `-lstdc++ -lm`).
    if args.mode == DriverMode::Gcc {
        cmd.arg("--driver-mode=gcc");
    }
    // `-v` reaches the linking `clang++` too, so its own `-v` lines (the
    // linker's command line) follow: build systems read the implicit link
    // directories and libraries from them (CMake's compiler ABI probe).
    if args.verbose {
        cmd.arg("-v");
    }
    for a in toolchain_link_args(args, tc) {
        cmd.arg(a);
    }
    if let Some(o) = &args.opt_flag {
        cmd.arg(o.clone());
    }
    // clang adds GCC's `crtfastmath.o` (flush denormals) for the last
    // fast-math flag as for `-Ofast` (spec/driver.md "Floating-point
    // flags").
    if let Some(f) = args.fp.fast_math_runtime_flag() {
        cmd.arg(f.to_owned());
    }
    if args.predef.pthread {
        cmd.arg("-pthread");
    }
    // `-gz` reaches the linker as `--compress-debug-sections=`.
    if let Some(z) = args.debug.compression() {
        cmd.arg(format!("-gz={}", z.name()));
    }
    // WP-873: the linker's LTO plugin (or lld's LTO) is clang++'s choice.
    match args.codegen.lto {
        Some(true) => {
            cmd.arg("-flto=thin");
        }
        Some(false) => {
            cmd.arg("-flto");
        }
        None => {}
    }
    // WP-941: clang links the profile runtime for coverage.
    for f in args.coverage.link_flags() {
        cmd.arg(f);
    }
    // WP-813: clang links the XRay runtime for `-fxray-instrument`.
    if args.codegen.xray_instrument {
        cmd.arg("-fxray-instrument");
    }
    // WP-675: the linking clang++ picks the sanitizer runtimes.
    if let Some(l) = args.sanitize(!tc.target.is_aarch64(), tc.target.triple).0.link_list() {
        cmd.arg(l);
        match args.sanitize.shared_libsan {
            Some(true) => {
                cmd.arg("-shared-libsan");
            }
            Some(false) => {
                cmd.arg("-static-libsan");
            }
            None => {}
        }
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
            if let Some(opt) = o.kind.separate() {
                cmd.arg(opt);
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
        Step::Frontend { input, dest, .. } => {
            let to = match dest {
                Dest::File(f) => f.as_str(),
                Dest::Pipe => "|",
            };
            format!(" (in-process) frontend \"{}\" -> \"{to}\"\n", input.path)
        }
        Step::External { cmd, .. } | Step::Assemble { cmd, .. } => cmd.render(quote_all),
        Step::Preprocess { input, dest } => {
            format!(" (in-process) preprocessor \"{}\" -> \"{dest}\"\n", input.path)
        }
    }
}

/// Starts `input`'s trace (spec/driver.md "Time trace"): its path is
/// derived from the job's output. When linking, that is clang's
/// temporary object, `<stem>-XXXXXX.o` (`-ftime-trace=<dir>` names the
/// trace after it, so two inputs, or two runs, never share one); `n`
/// is the input's position, which keeps the suffix unique per input.
fn start_trace(
    args: &Args,
    s: time_trace::Settings,
    input: &str,
    temp: &TempDir,
    n: usize,
) -> InputTrace {
    let phase = Phase::of(args);
    let output = match phase {
        Phase::Link => {
            let obj = default_output(input, Phase::Object);
            let stem = obj.strip_suffix(".o").unwrap_or(&obj);
            format!("{stem}-{}.o", temp.suffix(n))
        }
        _ => args.output.clone().unwrap_or_else(|| default_output(input, phase)),
    };
    InputTrace::start(s, time_trace::path(args, Some(phase), input, &output))
}

/// Runs the plan; returns the exit status.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
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
    // hod wrote this input's object: its `llc` step is skipped.
    let mut hod_done = false;
    // spec/driver.md "Time trace": one trace per source input, from its
    // frontend step to its last step before the next input or the link.
    let trace_settings = time_trace::settings(args).ok().flatten();
    let mut trace: Option<InputTrace> = None;
    // spec/driver.md "Compile statistics log": one record per source
    // input and one for the link, each ended where its trace ends.
    let stats_log = crate::stats::log_path();
    let mut stats: Option<crate::stats::Job> = None;
    let end_stats = |stats: &mut Option<crate::stats::Job>| {
        if let (Some(j), Some(log)) = (stats.take(), &stats_log) {
            j.finish(args, log);
        }
    };
    let mut traced_children = 0usize;
    let mut traced_inputs = 0usize;
    let mut child = |trace: &mut Option<InputTrace>, cmd: &Command| -> Command {
        let Some(t) = trace.as_mut() else { return cmd.clone() };
        if !matches!(cmd.tool, Tool::Llc | Tool::Opt) || temp.create().is_err() {
            return cmd.clone();
        }
        traced_children += 1;
        t.child(cmd, temp.file(&format!("time-trace-{traced_children}.json")))
    };
    for step in plan {
        if hod_done && matches!(step, Step::External { cmd, piped: true } if cmd.tool == Tool::Llc)
        {
            hod_done = false;
            continue;
        }
        if args.verbose {
            let mut err = io.stderr.clone();
            let _ = err.write_all(render(step, false).as_bytes());
        }
        match step {
            Step::Frontend { input, dest, hod } => {
                hod_done = false;
                end_trace(trace.take(), args, diag_opts, io);
                end_stats(&mut stats);
                stats = stats_log.as_ref().map(|_| crate::stats::Job::start(Some(&input.path)));
                traced_inputs += 1;
                trace =
                    trace_settings.map(|s| start_trace(args, s, &input.path, temp, traced_inputs));
                let text = {
                    let _frontend = kiln_source::trace::Phase::new("Frontend");
                    frontend(args, tc, input, stdin_text, diag_opts, io)
                };
                if let Some(t) = trace.as_mut() {
                    t.failed |= text.is_none();
                    t.begin_backend();
                }
                if let Some(j) = stats.as_mut() {
                    j.failed |= text.is_none();
                    if let Some(t) = &text {
                        j.module(args, tc, t);
                    }
                }
                // spec/driver.md "Back-end selection": hod first, on the
                // module before `opt`.
                if let (Some(h), Some(t)) = (hod, &text)
                    && hod_takes(args, tc, t)
                {
                    match run_hod(h, t, args, &mut stats, drv, io, temp) {
                        HodStep::Done => {
                            hod_done = true;
                            skip_piped = false;
                            ir = None;
                            continue;
                        }
                        HodStep::Fallback => {}
                        HodStep::Failed => {
                            if let Some(t) = trace.as_mut() {
                                t.failed = true;
                            }
                            if let Some(j) = stats.as_mut() {
                                j.failed = true;
                            }
                            failed = true;
                            skip_piped = true;
                            ir = None;
                            continue;
                        }
                    }
                }
                // The IR-level passes run before the IR is written or
                // piped on (spec/driver.md "IR optimization").
                let Some(cmds) = text.as_deref().and_then(|t| ir_passes(args, tc, t)) else {
                    // The frontend's own text is written as is: no UTF-8
                    // re-validation of a (`-g`: mostly metadata) module.
                    if let (Some(t), Dest::File(f)) = (&text, dest)
                        && let Err(e) = write_output(Some(f), t, &mut io.stdout)
                    {
                        report_output_error(diag_opts, &io.stderr, f, &e);
                        failed = true;
                    }
                    skip_piped = text.is_none();
                    failed |= text.is_none();
                    ir = text.map(String::into_bytes);
                    continue;
                };
                // Each run's output replaces its input: the frontend's
                // text is not kept alongside a copy (WP-633).
                let mut data = text.map(String::into_bytes).unwrap_or_default();
                let mut ok = true;
                for cmd in &cmds {
                    if args.verbose {
                        let mut err = io.stderr.clone();
                        let _ = err.write_all(cmd.render(false).as_bytes());
                    }
                    let mut out = Vec::new();
                    let cmd = child(&mut trace, cmd);
                    ok = InputTrace::run_child(&cmd, || {
                        cmd.run(Some(&data), &mut out, &io.stderr, drv)
                    });
                    if !ok {
                        break;
                    }
                    data = out;
                }
                if let Some(t) = trace.as_mut() {
                    t.failed |= !ok;
                }
                if let Some(j) = stats.as_mut() {
                    j.failed |= !ok;
                }
                ir = ok.then_some(data);
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
            Step::Preprocess { input, dest } => {
                end_trace(trace.take(), args, diag_opts, io);
                end_stats(&mut stats);
                let ok = preprocess_asm(args, tc, input, dest, stdin_text, diag_opts, io, temp);
                skip_piped = !ok;
                failed |= !ok;
            }
            Step::Assemble { cmd, after_pp } => {
                end_trace(trace.take(), args, diag_opts, io);
                end_stats(&mut stats);
                if *after_pp && skip_piped {
                    continue;
                }
                if Phase::of(args) == Phase::Link
                    && let Err(e) = temp.create()
                {
                    let dir = temp.path.display().to_string();
                    report_output_error(diag_opts, &io.stderr, &dir, &e.to_string());
                    failed = true;
                    continue;
                }
                failed |= !cmd.run(None, &mut io.stdout, &io.stderr, drv);
            }
            Step::External { cmd, piped: true } => {
                if skip_piped {
                    continue;
                }
                if matches!(cmd.tool, Tool::Llc | Tool::Opt | Tool::LlvmAs)
                    && Phase::of(args) == Phase::Link
                    && let Err(e) = temp.create()
                {
                    let dir = temp.path.display().to_string();
                    report_output_error(diag_opts, &io.stderr, &dir, &e.to_string());
                    failed = true;
                    continue;
                }
                let cmd = child(&mut trace, cmd);
                let ok = InputTrace::run_child(&cmd, || {
                    cmd.run(ir.as_deref(), &mut io.stdout, &io.stderr, drv)
                });
                if let Some(t) = trace.as_mut() {
                    t.failed |= !ok;
                }
                if let Some(j) = stats.as_mut() {
                    j.failed |= !ok;
                    if cmd.tool == Tool::Llc {
                        j.backend.get_or_insert("llvm");
                    }
                }
                failed |= !ok;
            }
            Step::External { cmd, piped: false } if !matches!(cmd.tool, Tool::Linker) => {
                // `-gz`'s in-place `llvm-objcopy`: part of the input.
                if !failed {
                    let ok = InputTrace::run_child(cmd, || {
                        cmd.run(None, &mut io.stdout, &io.stderr, drv)
                    });
                    if let Some(t) = trace.as_mut() {
                        t.failed |= !ok;
                    }
                    if let Some(j) = stats.as_mut() {
                        j.failed |= !ok;
                    }
                    failed |= !ok;
                }
            }
            Step::External { cmd, piped: false } => {
                end_trace(trace.take(), args, diag_opts, io);
                end_stats(&mut stats);
                // The link step: only when everything before it worked.
                if !failed {
                    let mut link = stats_log.as_ref().map(|_| crate::stats::Job::start(None));
                    let ok = InputTrace::run_child(cmd, || {
                        cmd.run(None, &mut io.stdout, &io.stderr, drv)
                    });
                    if let Some(j) = link.as_mut() {
                        j.failed = !ok;
                    }
                    end_stats(&mut link);
                    failed |= !ok;
                }
            }
        }
    }
    end_trace(trace.take(), args, diag_opts, io);
    end_stats(&mut stats);
    i32::from(failed)
}

/// What [`run_hod`] leaves to the caller.
enum HodStep {
    /// The object is written: skip `opt` and `llc`.
    Done,
    /// Take today's route (`opt` as the IR needs, then `llc`).
    Fallback,
    /// A hod failure under `KILN_HOD_STRICT=1`: reported, the input
    /// failed.
    Failed,
}

/// Runs hod (`cmd`, made by [`crate::hod::command`]) on the module text
/// `ir` (spec/driver.md "Back-end selection"): status 0 is done; status
/// 2 (declined) falls back silently (`-v`: one line naming the reason);
/// any other status or a signal warns and falls back, or is an error
/// with `KILN_HOD_STRICT=1`. A non-zero status never leaves an `-o`
/// file behind.
#[allow(clippy::too_many_arguments)]
fn run_hod(
    cmd: &Command,
    ir: &str,
    args: &Args,
    stats: &mut Option<crate::stats::Job>,
    drv: &DiagEngine<'_>,
    io: &mut Io,
    temp: &TempDir,
) -> HodStep {
    use crate::hod::Outcome;
    // A link's objects go to its temporary directory; if it cannot be
    // made, the `llc` step reports it.
    if Phase::of(args) == Phase::Link && temp.create().is_err() {
        return HodStep::Fallback;
    }
    let obj = crate::hod::output(cmd).filter(|o| *o != "-");
    if args.verbose {
        let mut err = io.stderr.clone();
        let _ = err.write_all(cmd.render(false).as_bytes());
    }
    let (outcome, out) = InputTrace::run_child(cmd, || crate::hod::run(cmd, ir.as_bytes()));
    if let Some(j) = stats.as_mut() {
        j.backend = Some(outcome.backend());
        j.hod_reason = outcome.reason().map(str::to_owned);
    }
    if outcome == Outcome::Compiled {
        if obj.is_none() {
            let _ = io.stdout.write_all(&out);
            let _ = io.stdout.flush();
        }
        return HodStep::Done;
    }
    if let Some(o) = obj
        && Path::new(o).exists()
    {
        let _ = std::fs::remove_file(o);
    }
    match outcome {
        Outcome::Compiled => HodStep::Done,
        Outcome::Declined(reason) => {
            if args.verbose {
                let mut err = io.stderr.clone();
                let _ = writeln!(err, " (hod declined '{reason}'; falling back to llc)");
            }
            HodStep::Fallback
        }
        Outcome::Failed { status, message } => {
            let message = if message.is_empty() { "no message".to_owned() } else { message };
            if crate::hod::strict() {
                Diag::new(DiagId::drv_hod_failed_strict, Span::INVALID)
                    .arg(status)
                    .arg(message)
                    .emit(drv);
                return HodStep::Failed;
            }
            Diag::new(DiagId::drv_hod_failed, Span::INVALID).arg(status).arg(message).emit(drv);
            HodStep::Fallback
        }
    }
}

/// Finishes an input's time trace and writes it. A trace that cannot
/// be written is reported as clang's `unable to open output file` and
/// leaves the exit status alone (clang 19 / 22 `cc1_main`: the error is
/// printed after the compile, which already succeeded).
fn end_trace(trace: Option<InputTrace>, args: &Args, diag_opts: &DiagOptions, io: &Io) {
    let Some(t) = trace else { return };
    if let Err((path, e)) = t.finish(&args.program) {
        report_output_error(diag_opts, &io.stderr, &path, &e);
    }
}

/// A [`Step::Preprocess`]: the `-E` job of the assembler-with-cpp input
/// written to `dest`, and its `-MD` / `-MMD` file. False after an error.
#[allow(clippy::too_many_arguments)]
fn preprocess_asm(
    args: &Args,
    tc: &Toolchain,
    input: &Input,
    dest: &str,
    stdin_text: Option<&str>,
    diag_opts: &DiagOptions,
    io: &mut Io,
    temp: &TempDir,
) -> bool {
    let std = match input_std(args, args.input_lang(input)) {
        Ok(s) => s,
        Err(d) => {
            let sm = SourceManager::new();
            let e = make_engine(&sm, diag_opts, io.stderr.clone(), None);
            d.emit(&e);
            e.flush();
            return false;
        }
    };
    let job = Job {
        args,
        toolchain: tc,
        std,
        path: &input.path,
        stdin_text: (input.path == "-").then(|| stdin_text.unwrap_or_default().to_owned()),
        diag_opts,
        stderr: io.stderr.clone(),
        asm: true,
    };
    let r = job.run();
    if r.errors > 0 {
        return false;
    }
    if Path::new(dest).starts_with(&temp.path)
        && let Err(e) = temp.create()
    {
        report_output_error(
            diag_opts,
            &io.stderr,
            &temp.path.display().to_string(),
            &e.to_string(),
        );
        return false;
    }
    if let Err(e) = write_output(Some(dest), &r.output, &mut io.stdout) {
        report_output_error(diag_opts, &io.stderr, dest, &e);
        return false;
    }
    if let Some(files) = r.deps {
        let text = crate::deps::deps_text(args, &input.path, &files);
        let path = match crate::deps::dep_destination(args, &input.path) {
            crate::deps::DepDest::Main => "-".to_owned(),
            crate::deps::DepDest::File(f) => f,
        };
        if let Err(e) = write_output(Some(&path), &text, &mut io.stdout) {
            report_output_error(diag_opts, &io.stderr, &path, &e);
            return false;
        }
    }
    true
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
    let job = Job {
        args,
        toolchain: tc,
        std,
        path: &input.path,
        stdin_text: (input.path == "-").then(|| stdin_text.unwrap_or_default().to_owned()),
        diag_opts,
        stderr: io.stderr.clone(),
        asm: false,
    };
    // `-fkiln-pch-snapshot=check` (WP-1392): compared first, then
    // compiled by replay.
    let snap_check = crate::pch_snapshot::check_replay(args);
    let snap_diff = snap_check.as_ref().and_then(|_| {
        crate::pch_snapshot::check_difference(
            args,
            tc,
            std,
            &input.path,
            job.stdin_text.as_deref(),
            diag_opts,
        )
    });
    let job = match &snap_check {
        Some(replay) => Job { args: replay, ..job },
        None => job,
    };
    let checked = crate::pch_check::codegen(
        args,
        &crate::pch_check::Input {
            tc,
            std,
            path: &input.path,
            stdin_text: job.stdin_text.clone(),
            diag_opts,
            stderr: &io.stderr,
        },
    );
    let (mut ir, deps) = if let Some(r) = checked {
        r
    } else {
        let (module, _, deps) = CodegenJob(job).run();
        (module.map(print_ir), deps)
    };
    if let Some(d) = snap_diff {
        crate::pch_snapshot::report_mismatch(args, diag_opts, &io.stderr, &d);
        ir = None;
    }
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
    fn code_part_stops_at_metadata() {
        let ir = "define void @f() #0 !dbg !5 {\n  ret void, !dbg !7\n}\n\nattributes #0 = { alwaysinline }\n\n!llvm.dbg.cu = !{!0}\n!0 = !{}\n";
        let head = code_part(ir);
        assert!(head.ends_with("alwaysinline }\n\n"));
        assert!(needs_always_inline(head));
        assert_eq!(code_part("!0 = !{}\n"), "");
        assert_eq!(code_part("declare void @g()\n"), "declare void @g()\n");
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

    /// WP-1282: `kiln` links as `clang`, `kiln++` as `clang++`.
    #[test]
    fn link_c_driver_mode() {
        let tc = searched_toolchain();
        let c = Args::parse_as("kiln", &["a.o".to_owned()]);
        let cmd = link_command(&c, &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(cmd.args, ["--driver-mode=gcc", "a.o", "-o", "a.out"]);
        let cxx = Args::parse_as("kiln", &["--driver-mode=g++".to_owned(), "a.o".to_owned()]);
        let cmd = link_command(&cxx, &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(cmd.args, ["a.o", "-o", "a.out"]);
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
            ..Toolchain::new(crate::toolchain::Target::host(), None, None, None)
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
