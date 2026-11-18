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
        code_model: f.code_model(!x86),
        large_data_threshold: f.large_data_threshold(x86),
        tls_model: f.tls_model(),
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

    /// WP-1383: [`Self::run`] with the `-include-pch`
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
        let restored = sm.file_count();
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
        crate::pch_snapshot::changed_files(&sm, restored, &snap.pch, &engine);
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

    /// The parser's options of the job.
    fn parse_options(&self) -> ParseOptions {
        let job = &self.0;
        let a = job.args;
        let x86 = !job.toolchain.target.is_aarch64();
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        ParseOptions {
            exceptions: a.predef.exceptions.unwrap_or(job.std.cplusplus),
            rtti: a.predef.rtti.unwrap_or(job.std.cplusplus),
            access_control: a.predef.access_control.unwrap_or(true),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            reflection: a.predef.reflection_for(&job.std),
            contracts: Some(a.predef.contracts_for(&job.std)),
            coro_aligned_allocation: a.predef.coro_aligned_allocation,
            lax_vector_conversions: a.lax_vector_conversions,
            clang_compat: a.predef.clang_compat.major(),
            gnu_lib_builtins: a.stdlib.unwrap_or_default() == StdLib::LibStdCxx,
            no_builtin: a.no_builtin(),
            no_builtin_fns: a.no_builtin_fns(),
            freestanding: a.predef.freestanding.unwrap_or(false),
            sized_deallocation: a.predef.sized_deallocation_for(&job.std),
            aligned_allocation: a.predef.aligned_allocation_for(&job.std),
            short_enums: a.predef.short_enums,
            char8: a.predef.char8_t,
            math_errno: a.fp_model().opts.math_errno,
            sve: feats.sve,
            sve_vector_bits: feats.sve_vector_bits,
            aarch64_cpu_valid: crate::features::aarch64_attr_cpu_valid,
            target_features: if x86 { Vec::new() } else { feats.extra.clone() },
            target_attr_features: crate::features::target_attr_feature_list,
            cfg_max_elems: a.cfg_max_elems.unwrap_or(kiln_sema::cfg::DEFAULT_MAX_ELEMS),
            permissive: a.permissive,
            range_for_ext_temps: a.range_for_ext_temps,
            contract_semantic: a.contract_semantic.unwrap_or_default(),
            contract_checks: a.contract_checks,
            cf_protection_branch: a.codegen.cf_protection_branch(x86),
            record_carried: a.bmi_kind != Some(1) && job.module_output_path().is_some(),
            record_image: a.bmi_image
                && a.bmi_kind != Some(1)
                && job.module_output_path().is_some(),
            image_census: a.bmi_image_census,
            gnu89_inline: a.predef.gnu89_inline_for(&job.std),
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
        // WP-1487: `-fnew-alignment=N` / `-faligned-new=N`.
        target.default_new_align = a.predef.new_alignment.unwrap_or(target.default_new_align);
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
            // WP-1383: the prefix's state restored.
            let (sema, pp, deps, popts) =
                job.restore_front(sm, engine, interner, inputs, restore, &target, || {
                    self.parse_options()
                })?;
            let main_id = pp.main_file();
            let (tu, pp) = kiln_parse::continue_preprocessed(sema, pp, &target, &popts);
            restore.timer.mark("continue");
            restore.timer.report();
            (tu, pp, deps, main_id)
        } else {
            let (pp, deps) = job.build_preprocessor(sm, engine, interner, inputs, true)?;
            let main_id = pp.main_file();
            let popts = self.parse_options();
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
        // WP-1557: read from the macro table at the end of the TU.
        let mason_meta = crate::mason_meta::options(a, &job.std, &pp);
        *deps_out = crate::pp_cmd::finish_deps(pp, deps);
        let mut module = None;
        if engine.error_count() == 0 {
            let fp = a.fp_model();
            let mut opts = CodegenOptions {
                source_filename: if job.path == "-" { "-".to_owned() } else { job.path.to_owned() },
                c: job.std.c,
                gnu89_inline: a.predef.gnu89_inline_for(&job.std),
                exceptions: a.predef.exceptions.unwrap_or(job.std.cplusplus),
                dynamic_exception_specs: job.std.cplusplus && job.std.lex < kiln_lex::Std::Cxx17,
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
                    flags: a.debug.record_command_line.then(|| recorded_command_line(a)),
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
            opts.elide_constructors = a.elide_constructors.unwrap_or(true);
            opts.bad_array_new_length = job.std.cplusplus
                && job.std.lex >= kiln_lex::Std::Cxx11
                && a.new_bound_check.unwrap_or(true);
            // WP-1651: GCC's unit-local `type_info` name marker, for
            // libstdc++'s `type_info` comparison.
            opts.rtti_local_name_marker = job.std.cplusplus
                && a.rtti_local_marker.unwrap_or(a.stdlib.unwrap_or_default() == StdLib::LibStdCxx);
            opts.check_new = a.check_new;
            opts.contract_checks = a.contract_checks;
            opts.mason_meta = mason_meta;
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
            let (r, warnings, mason) =
                kiln_codegen::emit_with_stats(&mut tu.ast, &mut tu.oven, &target, &opts, Some(sm));
            crate::stats::set_mason_defs(mason);
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

/// The command line `-grecord-command-line` records in the compile unit
/// (WP-1411), as clang's driver writes its `-dwarf-debug-flags`: the
/// executable's path, then every argument, each with a space or a
/// backslash escaped by a backslash. clang renders each argument from its
/// option table (`-DX` becomes `-D X`, `-grecord-gcc-switches`
/// `-grecord-command-line`); kiln writes the arguments as given.
fn recorded_command_line(args: &Args) -> String {
    fn escaped(out: &mut String, a: &str) {
        for c in a.chars() {
            if matches!(c, ' ' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
    }
    let exe =
        std::env::current_exe().map_or_else(|_| args.program.clone(), |e| e.display().to_string());
    let mut out = String::new();
    escaped(&mut out, &exe);
    // clang's driver puts the mode a `clang++` name implies on the
    // command line it records (the executable is `clang`, as `kiln++` is
    // `kiln`).
    if DriverMode::from_program_name(&args.program) == DriverMode::Gxx {
        out.push_str(" --driver-mode=g++");
    }
    for a in &args.argv {
        out.push(' ');
        escaped(&mut out, a);
    }
    out
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
    /// module goes to that route first, when the route predicate holds
    /// ([`hod_takes`]); the `opt` runs and the next `llc` step are the
    /// fallback.
    Frontend { input: &'a Input, dest: Dest, hod: Option<HodRoute> },
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

/// The hod route of a frontend step (spec/driver.md "Back-end selection").
#[derive(Debug)]
enum HodRoute {
    /// The external executable: this command ([`crate::hod::command`]).
    External(Command),
    /// The registered in-process back end ([`crate::backend_hook`]),
    /// given the arguments of this `llc` command (WP-1559).
    InProcess(Command),
}

/// What the hod route runs, when it is planned.
#[derive(Debug)]
enum HodTarget {
    /// The external executable found ([`crate::hod::find`]).
    Executable(PathBuf),
    /// The registered in-process back end.
    InProcess,
}

impl HodTarget {
    /// The route of an object job whose `llc` command is `llc`.
    fn route(&self, llc: &Command) -> HodRoute {
        match self {
            HodTarget::Executable(h) => HodRoute::External(crate::hod::command(h, llc)),
            HodTarget::InProcess => HodRoute::InProcess(llc.clone()),
        }
    }
}

/// `-###` (`quote_all`) and `-v`'s line for the in-process back end's
/// step over the job of `llc`: ` (in-process) hod` then `llc`'s
/// arguments, in the style of the frontend's line.
fn in_process_line(llc: &Command, quote_all: bool) -> String {
    let full = llc.render(quote_all);
    let program = Command { args: Vec::new(), ..llc.clone() }.render(quote_all);
    let args = full.strip_prefix(program.trim_end_matches('\n')).unwrap_or(&full);
    format!(" (in-process) hod{args}")
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
    if phase == Phase::Link
        && let Some(id) = unwindlib_error(args)
    {
        let arg = args.link.iter().rev().find(|o| is_unwindlib(&o.arg)).map(|o| o.arg.clone());
        let mut d = Diag::new(id, Span::INVALID);
        if id == DiagId::drv_invalid_unwindlib_name {
            d = d.arg(arg.unwrap_or_default());
        }
        d.emit(&drv);
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
    // only where the hod route is planned, and never when a back end is
    // registered in process (WP-1559: it replaces the executable).
    let hod = if hod_planned(args, tc) {
        if crate::backend_hook::registered().is_some() {
            Some(HodTarget::InProcess)
        } else {
            match crate::hod::find(args, tc) {
                Ok(p) => Some(HodTarget::Executable(p)),
                Err(searched) => {
                    Diag::new(DiagId::drv_hod_not_found, Span::INVALID).arg(searched).emit(&drv);
                    drv.flush();
                    return 1;
                }
            }
        }
    } else {
        None
    };
    let temp = TempDir::new();
    let plan = plan(args, tc, inputs, phase, &temp, hod.as_ref());
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
                    let line = match h {
                        HodRoute::External(c) => c.render(true),
                        HodRoute::InProcess(llc) => in_process_line(llc, true),
                    };
                    let _ = err.write_all(line.as_bytes());
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
        for o in args
            .asm
            .iter()
            .filter(|o| phase == Phase::Object || o.opt == "-Wa,")
            .filter(|o| !args.no_unused.quiet(&o.arg))
        {
            Diag::new(DiagId::drv_argument_unused, Span::INVALID).arg(&o.arg).emit(drv);
        }
    }
    false
}

fn is_unwindlib(arg: &str) -> bool {
    arg.starts_with("--unwindlib=") || arg.starts_with("-unwindlib=")
}

/// clang's check of the last `--unwindlib=` when linking (WP-1633):
/// `libgcc`, `platform` (libgcc on Linux) and `none` are accepted and
/// passed to the linking clang++; `libunwind` contradicts the runtime
/// library kiln links (libgcc, clang's default; kiln has no `--rtlib=`);
/// any other value is not a name.
fn unwindlib_error(args: &Args) -> Option<DiagId> {
    let o = args.link.iter().rev().find(|o| is_unwindlib(&o.arg))?;
    let v = o.arg.split_once('=').map_or("", |(_, v)| v);
    match v {
        "libgcc" | "platform" | "none" | "" => None,
        "libunwind" => Some(DiagId::drv_rtlib_requires_unwindlib),
        _ => Some(DiagId::drv_invalid_unwindlib_name),
    }
}

/// `<x>: 'linker' input unused` and `argument unused during compilation`
/// for a compile-only run, in command-line order.
fn warn_unused_link_inputs(args: &Args, inputs: &[&Input], phase: Phase, drv: &DiagEngine<'_>) {
    let report_opts = |pos: usize| {
        for (i, o) in args.link.iter().enumerate().filter(|(_, o)| o.pos == pos) {
            if o.kind == LinkOptKind::Silent || args.no_unused.link.contains(&i) {
                continue;
            }
            let text = match o.kind.separate() {
                Some(opt) => format!("{opt} {}", o.arg),
                None => o.arg.clone(),
            };
            let id = if o.kind.is_input() {
                DiagId::drv_linker_input_unused
            } else {
                DiagId::drv_argument_unused
            };
            Diag::new(id, Span::INVALID).arg(text).emit(drv);
        }
    };
    // `inputs` holds only the existing inputs; positions count all of them.
    for (pos, input) in args.inputs.iter().enumerate() {
        report_opts(pos);
        let existing = inputs.iter().any(|i| std::ptr::eq(*i, input));
        if args.no_unused.quiet(&input.path) {
            continue;
        }
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

/// The steps for `inputs`, in order. `hod`: what the hod route runs,
/// when it is planned ([`hod_planned`]).
fn plan<'a>(
    args: &Args,
    tc: &Toolchain,
    inputs: &[&'a Input],
    phase: Phase,
    temp: &TempDir,
    hod: Option<&HodTarget>,
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
                let hod = hod.filter(|_| phase == Phase::Object).map(|h| h.route(&cmd));
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
                let hod = hod.filter(|_| cmd.tool == Tool::Llc).map(|h| h.route(&cmd));
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
/// (spec/driver.md "IR optimization"). The declared name is the one
/// after the line's first ` @` (no return type or attribute before the
/// name holds one), and it is unquoted: a quoted name such as an asm
/// label's `@"x @llvm.coro.y"` is not an intrinsic (WP-1411).
pub fn needs_coro_lowering(ir: &str) -> bool {
    ir.lines().any(|l| {
        l.strip_prefix("declare ")
            .and_then(|r| r.split_once(" @"))
            .is_some_and(|(_, name)| name.starts_with("llvm.coro."))
    })
}

/// The attributes of an attribute group line `attributes #N = { ... }`
/// (any other line: none), one item each: a keyword (`alwaysinline`,
/// `memory(argmem: read)`, `allockind("alloc")`) or a string attribute
/// (`"key"` or `"key"="value"`), as printed. A quoted string is one
/// item whatever it holds (LLVM's printer escapes a `"` in it as `\22`,
/// so the next `"` closes it), and a keyword's parentheses are part of
/// it: a word inside a string value or an argument list is never an
/// item of its own (WP-1411).
pub(crate) fn group_attrs(line: &str) -> impl Iterator<Item = &str> {
    let body = line
        .strip_prefix("attributes #")
        .and_then(|r| r.split_once(" = { "))
        .and_then(|(_, r)| r.strip_suffix(" }").or_else(|| r.strip_suffix('}')))
        .unwrap_or("");
    let bytes = body.as_bytes();
    let mut at = 0;
    std::iter::from_fn(move || {
        while at < bytes.len() && bytes[at] == b' ' {
            at += 1;
        }
        if at >= bytes.len() {
            return None;
        }
        let start = at;
        let (mut quoted, mut depth) = (false, 0u32);
        while at < bytes.len() {
            match bytes[at] {
                b'"' => quoted = !quoted,
                b'(' if !quoted => depth += 1,
                b')' if !quoted => depth = depth.saturating_sub(1),
                b' ' if !quoted && depth == 0 => break,
                _ => {}
            }
            at += 1;
        }
        Some(&body[start..at])
    })
}

/// The key of a string attribute item of [`group_attrs`] (`"key"` or
/// `"key"="value"`); `None` for a keyword.
fn string_attr_key(item: &str) -> Option<&str> {
    item.strip_prefix('"').and_then(|r| r.split_once('"')).map(|(k, _)| k)
}

/// Whether the IR has an attribute group holding the attribute item
/// `pred` accepts (top-level `attributes #` lines only: a global's
/// initializer, a string constant included, is on an `@` line, and a
/// string's newline is escaped, so no constant reaches one).
fn any_group_attr(ir: &str, pred: impl Fn(&str) -> bool) -> bool {
    ir.lines().filter(|l| l.starts_with("attributes #")).any(|l| group_attrs(l).any(&pred))
}

/// The function attributes LLVM's `EntryExitInstrumenter` pass
/// (`ee-instrument`, in the `-O0` pipeline) reads.
const ENTRY_EXIT_ATTRS: [&str; 4] = [
    "instrument-function-entry",
    "instrument-function-entry-inlined",
    "instrument-function-exit",
    "instrument-function-exit-inlined",
];

/// Whether the IR has the entry or exit instrumentation attributes
/// (`-finstrument-functions*`, `-finstrument-function-entry-bare`) on
/// an attribute group, as a string attribute's key: a string constant
/// or another attribute's value with the same text does not count
/// (WP-1411).
pub fn needs_entry_instrumentation(ir: &str) -> bool {
    any_group_attr(ir, |a| string_attr_key(a).is_some_and(|k| ENTRY_EXIT_ATTRS.contains(&k)))
}

/// Whether the IR has an `alwaysinline` attribute group: on a definition
/// (an `always_inline` function) or on a call site (kiln marks the calls
/// of a `[[clang::always_inline]]` statement and those inside a
/// `flatten` function, `kiln_codegen` `stmt_call_attrs`). clang's O0
/// pipeline (`AlwaysInlinerPass`) inlines every such call and every
/// call to such a function, deleting a discardable one left unused,
/// before `-emit-llvm` writes the module.
/// The keyword must be an item of the group ([`group_attrs`]), not a
/// word of a string attribute's value (`"dontcall-error"="... alwaysinline
/// ..."`, WP-1411).
pub fn needs_always_inline(ir: &str) -> bool {
    any_group_attr(ir, |a| a == "alwaysinline")
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
///
/// The module's part of the decision is `facts` ([`IrFacts`]: read from
/// the module, WP-1559).
fn ir_passes(args: &Args, tc: &Toolchain, facts: IrFacts) -> Option<Vec<Command>> {
    if args.codegen.lto.is_some() {
        return lto_passes(args, tc);
    }
    if args.disable_llvm_passes {
        return None;
    }
    let passes = match opt_pipeline(args.opt_flag.as_deref()) {
        Some(p) => p,
        // WP-675: the sanitizer passes run at every level.
        None if sanitizer_passes(args, tc).is_some() => "default<O0>",
        // WP-941: so do the coverage passes.
        None if coverage_passes(args).is_some() => "default<O0>",
        None if facts.always_inline => "default<O0>",
        // WP-813: the O0 pipeline's `ee-instrument` turns the entry and
        // exit attributes into `__cyg_profile_func_*` calls.
        None if facts.entry_instrumentation => "default<O0>",
        None if facts.coro => "coro-early,cgscc(coro-split),coro-cleanup,globaldce",
        None => return None,
    };
    Some(opt_commands(args, tc, passes))
}

/// What a module holds that makes `opt` run at `-O0` (spec/driver.md
/// "IR optimization"; WP-1559): the module's half of [`ir_passes`] and
/// [`opt_triggers`], the options being the other. Computed from the
/// module before it is printed ([`IrFacts::of_module`], one pass over
/// its attribute table, function headers and call records: `kiln_llvm`
/// `query.rs`), and equal by contract to the scan of its printed text
/// ([`IrFacts::of_text`]), which the checked mode verifies
/// ([`check_opt_predicate`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct IrFacts {
    /// An `alwaysinline` function attribute group, on a definition or a
    /// call site ([`needs_always_inline`]).
    pub(crate) always_inline: bool,
    /// The entry or exit instrumentation attributes
    /// ([`needs_entry_instrumentation`]).
    pub(crate) entry_instrumentation: bool,
    /// A declared `llvm.coro.*` intrinsic ([`needs_coro_lowering`]).
    pub(crate) coro: bool,
}

impl IrFacts {
    /// The facts of the module text `ir` (the whole printed module).
    pub(crate) fn of_text(ir: &str) -> IrFacts {
        let ir = code_part(ir);
        IrFacts {
            always_inline: needs_always_inline(ir),
            entry_instrumentation: needs_entry_instrumentation(ir),
            coro: needs_coro_lowering(ir),
        }
    }

    /// The facts of `m`, without printing it.
    pub(crate) fn of_module(m: &Module) -> IrFacts {
        IrFacts {
            always_inline: m.has_fn_attr_group(|a| {
                *a == kiln_llvm::Attr::Flag(kiln_llvm::AttrKind::AlwaysInline)
            }),
            entry_instrumentation: m.has_fn_attr_key(&ENTRY_EXIT_ATTRS),
            coro: m.declares_prefixed("llvm.coro."),
        }
    }
}

/// `KILN_CHECK_OPT_PREDICATE=1`: the checked mode of the `opt` predicate
/// in a build without debug assertions.
pub const ENV_CHECK_OPT_PREDICATE: &str = "KILN_CHECK_OPT_PREDICATE";

/// Whether the checked mode of the `opt` predicate is on (spec/driver.md
/// "In-process back end", WP-1559): in a build with debug assertions
/// (the one `cargo xtask test` and `ci` run the corpus with, so every
/// corpus compile that prints a module is checked), or with
/// `KILN_CHECK_OPT_PREDICATE=1` (a release or harness-profile build: a
/// libc++ run, say).
pub(crate) fn check_opt_predicate() -> bool {
    cfg!(debug_assertions) || std::env::var_os(ENV_CHECK_OPT_PREDICATE).is_some_and(|v| v == "1")
}

/// Prints `module` ([`print_ir`]); in the checked mode, compares the
/// facts read from the module before the print (`facts`) with the scan
/// of the text, and panics on a difference (an internal compiler error:
/// the module predicate disagrees with the text scan it replaces).
pub(crate) fn print_checked(module: Module, facts: IrFacts) -> String {
    let text = print_ir(module);
    if check_opt_predicate() {
        let scanned = IrFacts::of_text(&text);
        assert!(
            scanned == facts,
            "the opt predicate read from the module ({facts:?}) differs from the scan of its \
             text ({scanned:?})"
        );
    }
    text
}

/// The conditions that make [`ir_passes`] schedule `opt` for a module
/// with `facts`, each by its stable name (spec/driver.md "Compile
/// statistics log"): every one that holds, in this order, any one of
/// which alone schedules it; empty when `opt` does not run. Only the
/// statistics log and the hod routes ask.
pub(crate) fn opt_triggers(args: &Args, tc: &Toolchain, facts: IrFacts) -> Vec<&'static str> {
    if args.disable_llvm_passes {
        return Vec::new();
    }
    let checks = [
        ("opt-level", opt_pipeline(args.opt_flag.as_deref()).is_some()),
        ("lto", args.codegen.lto.is_some()),
        ("sanitizer", sanitizer_passes(args, tc).is_some()),
        ("coverage", coverage_passes(args).is_some()),
        ("alwaysinline", facts.always_inline),
        ("instrument-entry", facts.entry_instrumentation),
        ("coro", facts.coro),
    ];
    checks.into_iter().filter(|(_, on)| *on).map(|(n, _)| n).collect()
}

/// The static half of the hod route's predicate (spec/driver.md
/// "Back-end selection"): the back end is hod and [`hod_static_skip`]
/// finds nothing. Every other run takes the LLVM route untouched;
/// `-fkiln-backend=hod` is a request for the cases hod can take, not an
/// error elsewhere.
fn hod_planned(args: &Args, tc: &Toolchain) -> bool {
    crate::hod::selected(args).unwrap_or_default() == crate::hod::Backend::Hod
        && hod_static_skip(args, tc).is_none()
}

/// The first condition of the run that keeps hod from being offered its
/// modules, by its `hod_skip` name (spec/driver.md "Compile statistics
/// log"), in this order: the output is not an object (`-c`, or the
/// objects of a link): `output-kind`; an `-O` level above 0:
/// `opt-level`; `-flto`: `lto`; a sanitizer: `sanitizer`; coverage:
/// `coverage`; `-finstrument-functions*`,
/// `-finstrument-function-entry-bare` (WP-1411: before the module, so
/// hod is not searched for), `-fxray-instrument` or
/// `-fpatchable-function-entry=`: `instrument`. `None`: the route is
/// planned.
fn hod_static_skip(args: &Args, tc: &Toolchain) -> Option<&'static str> {
    let c = &args.codegen;
    if !matches!(Phase::of(args), Phase::Object | Phase::Link) {
        Some("output-kind")
    } else if opt_pipeline(args.opt_flag.as_deref()).is_some() {
        Some("opt-level")
    } else if c.lto.is_some() {
        Some("lto")
    } else if args.sanitize(!tc.target.is_aarch64(), tc.target.triple).0.opts.any() {
        Some("sanitizer")
    } else if coverage_passes(args).is_some() {
        Some("coverage")
    } else if c.instrument_functions
        || c.instrument_functions_after_inlining
        || c.instrument_function_entry_bare
        || c.xray_instrument
        || c.patchable_function_entry.is_some()
    {
        Some("instrument")
    } else {
        None
    }
}

/// The `opt` triggers hod accepts in a module before `opt` (hod's
/// `spec/cli.md` "Pre-opt modules"): it compiles an `alwaysinline` call
/// out of line and a switch-resumed coroutine body once, with its frame
/// in the heap, in place of the always-inliner and the coroutine passes.
/// Both hod routes accept them (WP-1562): the external executable and the
/// in-process back end agree on every module.
const HOD_TRIGGERS: [&str; 2] = ["alwaysinline", "coro"];

/// The dynamic half of the hod route's predicate, the same on both
/// routes: the `opt` trigger set of the module ([`opt_triggers`], from
/// the module's [`IrFacts`]) is a subset of [`HOD_TRIGGERS`]. An
/// instrumented module (`instrument-entry`: after [`hod_static_skip`],
/// only an IR input carrying the attributes) takes the LLVM route. When
/// hod declines a module with triggers, the fallback runs `opt` as the
/// module needs ([`ir_passes`]), then `llc`.
fn hod_takes(args: &Args, tc: &Toolchain, facts: IrFacts) -> bool {
    opt_triggers(args, tc, facts).iter().all(|t| HOD_TRIGGERS.contains(t))
}

/// The statistics log's `hod_skip` of a job whose module has the `opt`
/// triggers `triggers` (empty without a module): `None` unless the back
/// end is hod and the job is not offered to hod; then the first static
/// reason ([`hod_static_skip`]), else the first trigger outside
/// [`HOD_TRIGGERS`], by its name.
pub(crate) fn hod_skip(
    args: &Args,
    tc: &Toolchain,
    triggers: &[&'static str],
) -> Option<&'static str> {
    if crate::hod::selected(args).unwrap_or_default() != crate::hod::Backend::Hod {
        return None;
    }
    hod_static_skip(args, tc)
        .or_else(|| triggers.iter().copied().find(|t| !HOD_TRIGGERS.contains(t)))
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
    if let Some(cm) = args.codegen.code_model(tc.target.is_aarch64()) {
        cmd.arg(format!("-code-model={}", cm.name()));
    }
    // clang `addX86AlignBranchArgs` (WP-961): the assembler pads branches
    // so none crosses or ends on a 32-byte boundary. Off x86 the option is
    // a driver error (`features::resolve`).
    if args.codegen.branches_within_32b && !tc.target.is_aarch64() {
        cmd.arg("-x86-branches-within-32B-boundaries");
    }
    // WP-1411: clang's `-cc1 -enable-tlsdesc` for x86-64's
    // `-mtls-dialect=gnu2` (`TargetOptions::EnableTLSDESC`; the IR is
    // the same). hod gets `llc`'s arguments and declines an option it
    // does not know, so such a module keeps the LLVM route.
    if args.codegen.tls_desc(!tc.target.is_aarch64()) {
        cmd.arg("-enable-tlsdesc");
    }
    // WP-1422: clang 19's `-mllvm -enable-machine-outliner[=never]` for
    // `-moutline` / `-mno-outline` (`addMachineOutlinerArgs`). LLVM adds
    // the outliner at optimization levels above `-O0` only (on `AArch64`
    // by default, for `minsize` functions), so at `-O0` the option changes
    // nothing and is left out: hod, which declines an `llc` option it does
    // not know, keeps such a module.
    if let Some(o) = args.codegen.outliner_arg(tc.target.is_aarch64())
        && llc_opt_level(args.opt_flag.as_deref()) != "-O0"
    {
        cmd.arg(o);
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
    // WP-1411: an LTO link generates the code; clang passes the TLS
    // dialect on as `-plugin-opt=-enable-tlsdesc`.
    if args.codegen.lto.is_some() && args.codegen.tls_desc(!tc.target.is_aarch64()) {
        cmd.arg("-mtls-dialect=gnu2");
    }
    // WP-1422: the LTO link's code generator gets the outliner setting
    // (clang turns it into `-plugin-opt=-enable-machine-outliner[=never]`;
    // `-moutline` off `AArch64` was already warned about and dropped).
    if args.codegen.lto.is_some() && args.codegen.outliner_arg(tc.target.is_aarch64()).is_some() {
        cmd.arg(if args.codegen.outline == Some(true) { "-moutline" } else { "-mno-outline" });
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
    if links_libstdcxx_exp(args, tc) {
        cmd.arg("-lstdc++exp");
    }
    cmd.arg("-o").arg(args.output.clone().unwrap_or_else(|| "a.out".to_owned()));
    cmd
}

/// Whether the link adds `-lstdc++exp` after the inputs, where GCC 16.1's
/// `g++` puts it (right before the `-lstdc++` the linking clang++ adds):
/// at C++26 and later (`-std=c++26`, `gnu++26`, `c++2c`, `gnu++2c`), and
/// below it when `-fcontracts` or `-fno-contracts` was given, as `g++`
/// does; only for the C++ driver with libstdc++, without `-nostdlib`,
/// `-nodefaultlibs` or `-nostdlib++`, and only when the selected GCC
/// installation has the archive (spec/driver.md "Link options"). The
/// archive holds the parts of libstdc++ whose ABI is not frozen (the
/// default `handle_contract_violation`, spec/contracts.md §6.3,
/// `<stacktrace>`, `std::text_encoding::environment`, `<debugging>`, the
/// Filesystem TS); as a static archive it adds nothing to a program that
/// references none of them. g++ 16.1 does not add it at a plain
/// `-std=c++26`; kiln deliberately does.
fn links_libstdcxx_exp(args: &Args, tc: &Toolchain) -> bool {
    let cxx26 = || {
        input_std(args, crate::args::Lang::Cxx)
            .is_ok_and(|s| s.cplusplus && s.lex >= kiln_lex::Std::Cxx26)
    };
    (args.predef.contracts_flag || cxx26())
        && args.mode != DriverMode::Gcc
        && args.stdlib.unwrap_or_default() == StdLib::LibStdCxx
        && !args
            .link
            .iter()
            .any(|o| matches!(o.arg.as_str(), "-nostdlib" | "-nodefaultlibs" | "-nostdlib++"))
        && tc.libstdcxx_exp_archive().is_some()
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
                let in_process = matches!(hod, Some(HodRoute::InProcess(_)));
                // The module's `opt` facts are read before it is printed
                // (WP-1559); the module is printed inside `Frontend`, as
                // before, unless the in-process back end gets it first.
                let (kept, mut text, facts) = {
                    let _frontend = kiln_source::trace::Phase::new("Frontend");
                    let module = frontend(args, tc, input, stdin_text, diag_opts, io);
                    let facts = module.as_ref().map(IrFacts::of_module);
                    match (module, facts) {
                        (Some(m), Some(f)) if in_process && hod_takes(args, tc, f) => {
                            (Some(m), None, facts)
                        }
                        (m, f) => (None, m.map(|m| print_checked(m, f.unwrap_or_default())), f),
                    }
                };
                let produced = kept.is_some() || text.is_some();
                let facts = facts.unwrap_or_default();
                if let Some(t) = trace.as_mut() {
                    t.failed |= !produced;
                    t.begin_backend();
                }
                if let Some(j) = stats.as_mut() {
                    j.failed |= !produced;
                    if produced {
                        j.triggers = opt_triggers(args, tc, facts);
                    }
                    j.counts = text.as_deref().map(crate::stats::IrCounts::of);
                    j.hod_skip = hod_skip(args, tc, &j.triggers);
                }
                // spec/driver.md "In-process back end": the registered
                // back end first, on the module before it is printed; on
                // any answer but done, the module is printed now and takes
                // the LLVM route below.
                if let (Some(HodRoute::InProcess(llc)), Some(m)) = (hod, kept) {
                    match run_in_process(llc, &m, args, &mut stats, diag_opts, drv, io, temp) {
                        HodStep::Done => {
                            hod_done = true;
                            skip_piped = false;
                            ir = None;
                            continue;
                        }
                        HodStep::Fallback => {
                            let t = print_checked(m, facts);
                            if let Some(j) = stats.as_mut() {
                                j.counts = Some(crate::stats::IrCounts::of(&t));
                            }
                            text = Some(t);
                        }
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
                // spec/driver.md "Back-end selection": hod first, on the
                // module before `opt`.
                if let (Some(HodRoute::External(h)), Some(t)) = (hod, &text)
                    && hod_takes(args, tc, facts)
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
                let Some(cmds) = text.as_ref().and_then(|_| ir_passes(args, tc, facts)) else {
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
/// any other status or a signal warns (`-Whod-fallback`, which a blanket
/// `-Werror` does not promote) and falls back, or is an error with
/// `KILN_HOD_STRICT=1` or `-Werror=hod-fallback`. A non-zero status never leaves an `-o`
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
    not_done(outcome, obj, args, drv, io)
}

/// The in-process back end's status in the `-Whod-fallback` warning.
const IN_PROCESS_STATUS: &str = "in process";

/// Hands the module `module` to the registered in-process back end
/// ([`crate::backend_hook`]) with the arguments of the job's `llc`
/// command `llc`, inside a contained-panic scope ([`crate::crash::contained`];
/// spec/driver.md "In-process back end", WP-1559). Done: the object is
/// written where `llc -o` would have written it. Declined, failed or
/// panicked (reason `internal-panic`): as hod's status 2, and 1
/// ([`run_hod`]), the `-o` file removed. The back end's time is the
/// `CodeGenPasses` event (detail `in-process`) and the statistics
/// log's `in-process-backend` child.
#[allow(clippy::too_many_arguments)]
fn run_in_process(
    llc: &Command,
    module: &Module,
    args: &Args,
    stats: &mut Option<crate::stats::Job>,
    diag_opts: &DiagOptions,
    drv: &DiagEngine<'_>,
    io: &mut Io,
    temp: &TempDir,
) -> HodStep {
    use crate::backend_hook::Answer;
    use crate::hod::Outcome;
    let Some(backend) = crate::backend_hook::registered() else { return HodStep::Fallback };
    if Phase::of(args) == Phase::Link && temp.create().is_err() {
        return HodStep::Fallback;
    }
    let obj = crate::hod::output(llc).filter(|o| *o != "-");
    if args.verbose {
        let mut err = io.stderr.clone();
        let _ = err.write_all(in_process_line(llc, false).as_bytes());
    }
    let mut out = Vec::new();
    let start = std::time::Instant::now();
    let answer = crate::crash::contained(|| backend.compile(module, &llc.args, &mut out));
    let end = std::time::Instant::now();
    kiln_source::trace::complete("CodeGenPasses", start, end, || "in-process".to_owned());
    kiln_source::trace::phase_add("in-process-backend", start, end);
    let (outcome, reason) = match answer {
        Ok(Answer::Done) => (Outcome::Compiled, None),
        Ok(Answer::Declined(r)) => (Outcome::Declined(r.clone()), Some(r)),
        Ok(Answer::Failed(m)) => {
            let reason = crate::hod::failure_reason(&format!("hod: error: {m}"))
                .unwrap_or("internal-error")
                .to_owned();
            (Outcome::Failed { status: IN_PROCESS_STATUS.to_owned(), message: m }, Some(reason))
        }
        Err(panic) => {
            let message = format!("internal-panic: {}", panic.lines().next().unwrap_or(""));
            let failed = Outcome::Failed { status: IN_PROCESS_STATUS.to_owned(), message };
            (failed, Some("internal-panic".to_owned()))
        }
    };
    if let Some(j) = stats.as_mut() {
        j.backend = Some(outcome.backend());
        j.hod_reason = reason;
        j.in_process = true;
    }
    if outcome == Outcome::Compiled {
        let written = match obj {
            Some(o) => std::fs::write(o, &out).map_err(|e| (o, e.to_string())),
            None => io
                .stdout
                .write_all(&out)
                .and_then(|()| io.stdout.flush())
                .map_err(|e| ("-", e.to_string())),
        };
        return match written {
            Ok(()) => HodStep::Done,
            Err((path, e)) => {
                report_output_error(diag_opts, &io.stderr, path, &e);
                HodStep::Failed
            }
        };
    }
    drop(out);
    not_done(outcome, obj, args, drv, io)
}

/// A hod answer other than done ([`run_hod`], [`run_in_process`]): the
/// `-o` file `obj` removed, then the decline's `-v` line, or the failure's
/// warning (an error under `KILN_HOD_STRICT=1` / `-Werror=hod-fallback`).
fn not_done(
    outcome: crate::hod::Outcome,
    obj: Option<&str>,
    args: &Args,
    drv: &DiagEngine<'_>,
    io: &mut Io,
) -> HodStep {
    use crate::hod::Outcome;
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
            // WP-1477: a blanket `-Werror` leaves it a warning (the
            // fallback's result is the output); `-Werror=hod-fallback`
            // makes it an error and the input fails, as strict mode.
            let before = drv.error_count();
            Diag::new(DiagId::drv_hod_failed, Span::INVALID).arg(status).arg(message).emit(drv);
            if drv.error_count() > before { HodStep::Failed } else { HodStep::Fallback }
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

/// The frontend on one source input: its module (the caller prints it,
/// [`print_checked`]); `None` after an error. Writes the input's `-MD` /
/// `-MMD` dependency file, errors or not
/// ([`crate::deps::write_side_file`]).
fn frontend(
    args: &Args,
    tc: &Toolchain,
    input: &Input,
    stdin_text: Option<&str>,
    diag_opts: &DiagOptions,
    io: &mut Io,
) -> Option<Module> {
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
    let (mut module, _, deps) = CodegenJob(job).run();
    if let Some(d) = snap_diff {
        crate::pch_snapshot::report_mismatch(args, diag_opts, &io.stderr, &d);
        module = None;
    }
    if let Some(list) = deps
        && !crate::deps::write_side_file(args, &input.path, &list.borrow(), diag_opts, io)
    {
        return None;
    }
    module
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
        // WP-1411: an asm label holding the text is a quoted name.
        assert!(!needs_coro_lowering("declare void @\"x @llvm.coro.id\"() #0\n"));
        assert!(!needs_coro_lowering("@s = constant [16 x i8] c\"declare @llvm.coro.\""));
    }

    /// WP-1411: the triggers read attribute groups item by item; the
    /// attribute names as string constants or as words of another
    /// attribute's value do not count.
    #[test]
    fn attribute_triggers_ignore_strings() {
        let items: Vec<&str> = group_attrs(
            "attributes #3 = { alwaysinline memory(argmem: read) \"a b\"=\"c d\" \"k\" }",
        )
        .collect();
        assert_eq!(items, ["alwaysinline", "memory(argmem: read)", "\"a b\"=\"c d\"", "\"k\""]);
        assert_eq!(group_attrs("define void @f() #0 {").count(), 0);
        let instr = "attributes #0 = { noinline \"instrument-function-entry-inlined\"=\"__cyg_profile_func_enter_bare\" }\n";
        assert!(needs_entry_instrumentation(instr));
        let instr =
            "attributes #0 = { \"instrument-function-entry\"=\"__cyg_profile_func_enter\" }\n";
        assert!(needs_entry_instrumentation(instr));
        for ir in [
            "@.str = private unnamed_addr constant [26 x i8] c\"instrument-function-entry\\00\", align 1\n",
            "@.str = private unnamed_addr constant [27 x i8] c\"\\22instrument-function-entry\\00\", align 1\n",
            "attributes #0 = { \"dontcall-error\"=\"\\22instrument-function-entry\" }\n",
            "attributes #0 = { \"dontcall-warn\"=\"no alwaysinline here\" noinline }\n",
            "  call void @f(ptr @\"instrument-function-entry\")\n",
        ] {
            assert!(!needs_entry_instrumentation(ir), "{ir}");
            assert!(!needs_always_inline(ir), "{ir}");
        }
        assert!(needs_always_inline("attributes #9 = { \"dontcall-warn\"=\"x\" alwaysinline }"));
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

    /// WP-1453: the link-mode options keep their place among the
    /// objects, `-l` and `-Wl,` options, as on clang's link line; the
    /// separate forms are passed as two arguments.
    #[test]
    fn link_order_link_mode() {
        let a = args(&[
            "-nodefaultlibs",
            "a.o",
            "-Wl,--start-group",
            "-lx",
            "b.o",
            "-ly",
            "-Wl,--end-group",
            "-nostartfiles",
            "-z",
            "defs",
            "-ufoo",
            "c.o",
            "-T",
            "x.ld",
            "-shared",
        ]);
        let tc = searched_toolchain();
        let inputs: Vec<(usize, String)> =
            a.inputs.iter().enumerate().map(|(i, x)| (i, x.path.clone())).collect();
        let cmd = link_command(&a, &tc, &inputs);
        assert_eq!(
            cmd.args,
            [
                "-nodefaultlibs",
                "a.o",
                "-Wl,--start-group",
                "-lx",
                "b.o",
                "-ly",
                "-Wl,--end-group",
                "-nostartfiles",
                "-z",
                "defs",
                "-u",
                "foo",
                "c.o",
                "-T",
                "x.ld",
                "-shared",
                "-o",
                "a.out"
            ]
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

    /// WP-1463 (spec/driver.md "Link options"): `-f[no-]contracts` adds
    /// `-lstdc++exp` after the inputs, as GCC 16.1's `g++` does, only with
    /// libstdc++, a GCC installation holding the archive and the default
    /// libraries; never in the C driver.
    #[test]
    fn link_libstdcxx_exp() {
        let temp = TempDir::new();
        let ver = temp.path.join("lib/gcc/x86_64-linux-gnu/16");
        std::fs::create_dir_all(&ver).unwrap();
        let lib64 = temp.path.join("lib64");
        std::fs::create_dir_all(&lib64).unwrap();
        let gcc = crate::toolchain::GccInstall {
            path: ver.display().to_string(),
            triple: "x86_64-linux-gnu".to_owned(),
            version: "16".to_owned(),
        };
        let tc = Toolchain { gcc: Some(gcc), ..searched_toolchain() };
        let link = |argv: &[&str]| link_command(&args(argv), &tc, &[(0, "a.o".to_owned())]).args;
        // No archive: nothing added, no diagnostic.
        assert_eq!(link(&["-fcontracts", "a.o"]), ["a.o", "-o", "a.out"]);
        std::fs::write(lib64.join("libstdc++exp.a"), b"!<arch>\n").unwrap();
        assert_eq!(
            link(&["-fcontracts", "a.o", "-lm"]),
            ["a.o", "-lm", "-lstdc++exp", "-o", "a.out"]
        );
        assert_eq!(link(&["-fno-contracts", "a.o"]), ["a.o", "-lstdc++exp", "-o", "a.out"]);
        assert_eq!(link(&["a.o"]), ["a.o", "-o", "a.out"]);
        for off in ["-nostdlib", "-nodefaultlibs", "-nostdlib++"] {
            assert_eq!(link(&["-fcontracts", off, "a.o"]), [off, "a.o", "-o", "a.out"]);
        }
        assert_eq!(
            link(&["-fcontracts", "-stdlib=libc++", "a.o"]),
            ["-stdlib=libc++", "a.o", "-o", "a.out"]
        );
        let c = Args::parse_as("kiln", &["-fcontracts".to_owned(), "a.o".to_owned()]);
        let cmd = link_command(&c, &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(cmd.args, ["--driver-mode=gcc", "a.o", "-o", "a.out"]);
        // WP-1485 item 10: at C++26 and later without the flag, once.
        for std in ["-std=c++26", "-std=gnu++26", "-std=c++2c", "-std=gnu++2c"] {
            assert_eq!(link(&[std, "a.o"]), ["a.o", "-lstdc++exp", "-o", "a.out"]);
        }
        assert_eq!(
            link(&["-std=c++26", "-fcontracts", "a.o"]),
            ["a.o", "-lstdc++exp", "-o", "a.out"]
        );
        assert_eq!(link(&["-std=c++23", "a.o"]), ["a.o", "-o", "a.out"]);
        assert_eq!(
            link(&["-std=c++23", "-fcontracts", "a.o"]),
            ["a.o", "-lstdc++exp", "-o", "a.out"]
        );
        for off in ["-nostdlib", "-nodefaultlibs", "-nostdlib++"] {
            assert_eq!(link(&["-std=c++26", off, "a.o"]), [off, "a.o", "-o", "a.out"]);
        }
        assert_eq!(
            link(&["-std=c++26", "-stdlib=libc++", "a.o"]),
            ["-stdlib=libc++", "a.o", "-o", "a.out"]
        );
        let c = Args::parse_as("kiln", &["-std=c23".to_owned(), "a.o".to_owned()]);
        let cmd = link_command(&c, &tc, &[(0, "a.o".to_owned())]);
        assert_eq!(cmd.args, ["--driver-mode=gcc", "a.o", "-o", "a.out"]);
        std::fs::remove_file(lib64.join("libstdc++exp.a")).unwrap();
        assert_eq!(link(&["-std=c++26", "a.o"]), ["a.o", "-o", "a.out"]);
        temp.remove();
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

    /// WP-1562: the hod route's predicate over every combination of the
    /// module's facts, under the arguments that decide it. The expected
    /// answers are the external route's before WP-1562, written out (the
    /// stock binary's decisions, bit for bit); the in-process route now
    /// shares them (one trigger set, no route argument).
    #[test]
    fn hod_predicate_is_the_external_routes() {
        let tc = Toolchain::new(crate::toolchain::Target::host(), None, None, None);
        let all_facts = (0..8u8).map(|b| IrFacts {
            always_inline: b & 1 != 0,
            entry_instrumentation: b & 2 != 0,
            coro: b & 4 != 0,
        });
        for facts in all_facts {
            let name = |f: bool, n: &'static str| f.then_some(n);
            let triggers: Vec<&str> = [
                name(facts.always_inline, "alwaysinline"),
                name(facts.entry_instrumentation, "instrument-entry"),
                name(facts.coro, "coro"),
            ]
            .into_iter()
            .flatten()
            .collect();
            // `-c` with hod: taken unless `instrument-entry` holds; the skip
            // names it, and only it.
            let a = args(&["-c", "-fkiln-backend=hod", "a.cpp"]);
            assert_eq!(opt_triggers(&a, &tc, facts), triggers, "{facts:?}");
            assert_eq!(hod_takes(&a, &tc, facts), !facts.entry_instrumentation, "{facts:?}");
            let skip = facts.entry_instrumentation.then_some("instrument-entry");
            assert_eq!(hod_skip(&a, &tc, &triggers), skip, "{facts:?}");
            // `-disable-llvm-passes`: no trigger, always taken.
            let a = args(&["-c", "-fkiln-backend=hod", "-Xclang", "-disable-llvm-passes", "a.cpp"]);
            assert!(hod_takes(&a, &tc, facts), "{facts:?}");
            // The static reasons come first, whatever the module has.
            for (extra, why) in [
                ("-O2", "opt-level"),
                ("-fsanitize=address", "sanitizer"),
                ("-S", "output-kind"),
                ("-flto", "lto"),
            ] {
                let a = args(&["-c", "-fkiln-backend=hod", extra, "a.cpp"]);
                assert_eq!(hod_skip(&a, &tc, &triggers), Some(why), "{extra} {facts:?}");
                assert!(!hod_planned(&a, &tc), "{extra}");
            }
            // The LLVM back end: no skip is recorded.
            for a in [args(&["-c", "a.cpp"]), args(&["-c", "-fkiln-backend=llvm", "a.cpp"])] {
                assert_eq!(hod_skip(&a, &tc, &triggers), None, "{facts:?}");
                assert!(!hod_planned(&a, &tc));
            }
        }
    }
}
