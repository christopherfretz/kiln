//! Recursive-descent C++26 parser driving semantic analysis.
//!
//! See `docs/ARCHITECTURE.md` and `spec/parse.md` for the contract
//! this crate implements. The parser owns the grammar and the token
//! stream; [`kiln_sema::Sema`] owns scopes, lookup and the typed AST
//! (clang's Parser/Sema split).
#![forbid(unsafe_code)]

mod asm;
mod attr;
mod class;
mod consteval_block;
mod contracts;
mod decl;
mod expr;
mod fp_pragma;
pub mod keywords;
mod lambda;
mod loop_hint;
mod modules;
mod new_delete;
mod parser;
mod pragma;
mod reflect;
mod stmt;
mod templ;
mod tentative;
pub mod tokens;

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::mem::MemReport;
use kiln_ast::types::{TypeContext, TypeOptions};
use kiln_diag::DiagEngine;
use kiln_lex::Std;
use kiln_lex::literal::{EncodingParams, LiteralOptions};
use kiln_pp::{PpOptions, Preprocessor};
use kiln_sema::target::Target;
use kiln_sema::{Budget, Oven, ProfileLevel, Sema};
use kiln_source::activity::{self, Phase};
use kiln_source::symbol::Interner;
use kiln_source::{FileId, Loc, SourceManager};

pub use parser::{PExpr, Parser};
pub use tentative::TypeIdContext;
pub use tokens::{Event, LoopHintInfo, TokenSource};

/// Options for [`parse_file`], [`parse_tu`] and [`parse_preprocessed`].
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct ParseOptions {
    /// The language standard (in C mode, the C++ standard sharing C's
    /// lexical rules; see [`Self::c`]).
    pub std: Std,
    /// The C standard when the input is C; `None` is C++
    /// ([`kiln_sema::Sema::c`]; spec/driver.md "Language mode").
    pub c: Option<kiln_lex::CStd>,
    /// Keep every token of the translation unit instead of a sliding
    /// window (the pre-WP-055 whole-file buffer; for measurements only).
    pub keep_tokens: bool,
    /// C++ exceptions are enabled (`-f[no-]exceptions`): `throw` is
    /// an error otherwise ([except], clang's `err_exceptions_disabled`).
    pub exceptions: bool,
    /// Run-time type information is enabled (`-f[no-]rtti`): `typeid`
    /// is an error otherwise (clang's `err_no_typeid_with_fno_rtti`).
    pub rtti: bool,
    /// A GNU dialect (`-std=gnu++NN`, [`kiln_sema::Sema::gnu`]).
    pub gnu: bool,
    /// GNU89 inline semantics in C ([`kiln_sema::Sema::gnu89_inline`]).
    pub gnu89_inline: bool,
    /// `-ftemplate-depth=` / `-ftemplate-backtrace-limit=`.
    pub inst_limits: kiln_sema::templ::stack::InstLimits,
    /// `-fkiln-instantiation-report[=all|json]`.
    pub inst_report: kiln_sema::templ::report::ReportFormat,
    /// `-freflection` (P2996 spike): `^^`, type splices and the kiln
    /// reflection intrinsics ([`kiln_sema::Sema::reflection`]).
    pub reflection: bool,
    /// `-fcoro-aligned-allocation` ([`kiln_sema::Sema::coro_aligned_allocation`]).
    pub coro_aligned_allocation: bool,
    /// `-flax-vector-conversions=` ([`kiln_sema::Sema::lax_vector_conversions`]).
    pub lax_vector_conversions: kiln_sema::LaxVectorConversions,
    /// `-fclang-compat=<major>`: the clang release whose spellings kiln
    /// reproduces where they differ (spec/pp.md "clang compatibility
    /// level"; [`kiln_sema::Sema::clang_compat`]).
    pub clang_compat: u32,
    /// [`kiln_sema::Sema::cf_protection_branch`].
    pub cf_protection_branch: bool,
    /// GCC's implicit library builtins (`-stdlib=libstdc++`;
    /// [`kiln_sema::Sema::gnu_lib_builtins`]).
    pub gnu_lib_builtins: bool,
    /// `-fno-builtin` / `-ffreestanding` ([`kiln_sema::Sema::no_builtin`]).
    pub no_builtin: bool,
    /// The `-fno-builtin-<name>` names ([`kiln_sema::Sema::no_builtin_fns`]).
    pub no_builtin_fns: Vec<String>,
    /// `-ffreestanding` ([`kiln_sema::Sema::freestanding`]).
    pub freestanding: bool,
    /// `-f[no-]sized-deallocation` ([`kiln_sema::Sema::sized_deallocation`]).
    pub sized_deallocation: bool,
    /// `-f[no-]char8_t` ([`kiln_sema::Sema::char8`]; `None`: by `std`).
    pub char8: Option<bool>,
    /// `-f[no-]math-errno` ([`kiln_sema::Sema::math_errno`]).
    pub math_errno: bool,
    /// The `AArch64` `sve` target feature (`-march=armv8-a+sve`;
    /// [`kiln_sema::Sema::sve`]).
    pub sve: bool,
    /// `-msve-vector-bits=N` ([`kiln_sema::Sema::sve_vector_bits`]).
    pub sve_vector_bits: Option<u32>,
    /// The `AArch64` CPU names ([`kiln_sema::Sema::aarch64_cpu_valid`]).
    pub aarch64_cpu_valid: fn(&str, bool) -> bool,
    /// `-fkiln-dump-cfg` ([`kiln_sema::Sema::cfg_dump`]).
    pub cfg_dump: bool,
    /// `-fkiln-cfg-stats` ([`kiln_sema::Sema::cfg_stats`]).
    pub cfg_stats: bool,
    /// `-fkiln-cfg-max-elems=` ([`kiln_sema::Sema::cfg_max_elems`]).
    pub cfg_max_elems: usize,
    /// `-fpermissive` ([`kiln_sema::Sema::permissive`]).
    pub permissive: bool,
    /// `-fcontract-evaluation-semantic=` ([`kiln_ast::Ast::contract_semantic`];
    /// spec/contracts.md §6).
    pub contract_semantic: kiln_ast::stmt::ContractSemantic,
    /// No deduction pre-filters ([`kiln_sema::Sema::full_deduction`]; a
    /// test hook).
    #[doc(hidden)]
    pub full_deduction: bool,
    /// No redeclaration index ([`kiln_sema::Sema::full_redecl_scan`]; a
    /// test hook).
    #[doc(hidden)]
    pub full_redecl_scan: bool,
    /// Record the main unit's constant results for a kind-2 module file
    /// ([`ParsedTu::carried`], spec/modules.md §11.11).
    pub record_carried: bool,
    /// Record the main unit's declaration image for a kind-2 module file
    /// ([`ParsedTu::image`], spec/modules.md §11.11 "Part 2a").
    pub record_image: bool,
    /// Count why the declaration image cannot carry each top-level
    /// declaration ([`ParsedTu::image_census`], spec/modules.md §11.11
    /// "Fallback census").
    pub image_census: bool,
    /// The main file is a header being precompiled (`-emit-pch` with
    /// `-fkiln-pch-image`): record its declaration image
    /// ([`ParsedTu::pch_image`], WP-1036). With [`Self::image_census`],
    /// the census is the header's.
    pub record_pch: bool,
    /// A precompiled header's declarations, built into the unit before
    /// its first token in place of replaying the header (WP-1036).
    pub pch: Option<PchLoad>,
}

/// What [`ParseOptions::pch`] builds: the decoded image, the source
/// manager's buffers for its file table (same order) and the header's
/// sema pragma state.
#[derive(Clone, Debug)]
pub struct PchLoad {
    /// The precompiled header's path (for the corrupt-file error).
    pub path: String,
    pub decls: Arc<kiln_sema::PchDecls>,
    pub files: Vec<FileId>,
    /// The predefines line replay includes the header from.
    pub root: Loc,
    pub pragmas: kiln_sema::PragmaState,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions {
            std: Std::Cxx26,
            c: None,
            keep_tokens: false,
            record_carried: false,
            record_image: false,
            image_census: false,
            record_pch: false,
            pch: None,
            exceptions: true,
            rtti: true,
            gnu: false,
            gnu89_inline: false,
            inst_limits: kiln_sema::templ::stack::InstLimits::default(),
            inst_report: kiln_sema::templ::report::ReportFormat::Off,
            reflection: false,
            coro_aligned_allocation: false,
            lax_vector_conversions: kiln_sema::LaxVectorConversions::All,
            clang_compat: 22,
            cf_protection_branch: false,
            gnu_lib_builtins: false,
            no_builtin: false,
            no_builtin_fns: Vec::new(),
            freestanding: false,
            sized_deallocation: true,
            char8: None,
            math_errno: true,
            sve: false,
            sve_vector_bits: None,
            aarch64_cpu_valid: |_, _| true,
            cfg_dump: false,
            cfg_stats: false,
            cfg_max_elems: kiln_sema::cfg::DEFAULT_MAX_ELEMS,
            permissive: false,
            contract_semantic: kiln_ast::stmt::ContractSemantic::Enforce,
            full_deduction: false,
            full_redecl_scan: false,
        }
    }
}

/// The literal-encoding parameters of `target`.
/// [`Sema::clang_constexpr_builtin`] from the preprocessor's
/// `__has_builtin` / `__has_constexpr_builtin` tables (WP-945).
fn clang_constexpr_builtin(name: &str, q: kiln_sema::BuiltinQuery) -> Option<bool> {
    use kiln_pp::FeatureQueries;
    let triple = if q.x86_64 { "x86_64-unknown-linux-gnu" } else { "aarch64-unknown-linux-gnu" };
    let gnu = [kiln_pp::FeatureFlag::GnuMode];
    let flags: &[kiln_pp::FeatureFlag] = if q.gnu { &gnu } else { &[] };
    let compat = if q.clang_compat < 22 {
        kiln_pp::ClangCompat::Clang19
    } else {
        kiln_pp::ClangCompat::Clang22
    };
    let f = kiln_pp::ClangFeatures::new(triple, q.std, flags)?.with_compat(compat);
    (f.has_builtin(name) != 0).then(|| f.has_constexpr_builtin(name))
}

fn encoding_params(target: &Target) -> EncodingParams {
    EncodingParams {
        char_signed: target.types.char_signed,
        wchar_bits: 32,
        wchar_signed: target.types.wchar_signed,
    }
}

/// Constant-evaluation settings of a translation unit: the driver's
/// `-fconstexpr-steps=`, `-fconstexpr-depth=`, `-fconstexpr-memory=`,
/// `-fconstexpr-backtrace-limit=`, `-fconstexpr-stats` and
/// `-fconstexpr-profile` (`spec/oven.md` "Integration", "Profile
/// counters").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstexprLimits {
    /// The budget of each top-level evaluation.
    pub budget: Budget,
    /// Call frames a failure's backtrace shows (0: all).
    pub backtrace_limit: usize,
    /// What the Oven's profile records.
    pub profile: ProfileLevel,
}

impl Default for ConstexprLimits {
    /// clang's defaults (steps 1048576, depth 512, backtrace 10) and
    /// kiln's 256 MiB memory budget.
    fn default() -> Self {
        ConstexprLimits {
            budget: Budget::default(),
            backtrace_limit: 10,
            profile: ProfileLevel::Off,
        }
    }
}

/// A parsed translation unit: its AST and the Oven that evaluated its
/// constant expressions (whose [`Oven::stats`] feed
/// `-fconstexpr-report=`).
#[derive(Debug)]
pub struct ParsedTu {
    /// The AST.
    pub ast: Ast,
    /// The TU's constant evaluator.
    pub oven: Oven,
    /// The template instantiation report (spec/sema-templates.md
    /// §11.3), empty unless [`ParseOptions::inst_report`] asked for it.
    pub inst_report: String,
    /// The `-fkiln-dump-cfg` dumps and `-fkiln-cfg-stats` lines, empty
    /// unless [`ParseOptions::cfg_dump`] or [`ParseOptions::cfg_stats`]
    /// asked for them.
    pub cfg_dump: String,
    /// Sema's memory rows at the end of the translation unit
    /// ([`kiln_sema::Sema::memory`]; sema itself is gone).
    pub sema_memory: MemReport,
    /// The main unit's carried constant results, when
    /// [`ParseOptions::record_carried`] (spec/modules.md §11.11).
    pub carried: Option<kiln_sema::CarriedConstants>,
    /// Constant evaluations answered from a kind-2 module file's carried
    /// results instead of being run.
    pub carried_hits: u64,
    /// The main unit's declaration image, when
    /// [`ParseOptions::record_image`] (spec/modules.md §11.11 "Part 2a").
    pub image: Option<kiln_sema::ModuleImage>,
    /// The fallback census, when [`ParseOptions::image_census`] and the
    /// main unit is a module unit.
    pub image_census: Option<kiln_sema::ImageCensus>,
    /// The precompiled header's declaration image, when
    /// [`ParseOptions::record_pch`] (WP-1036).
    pub pch_image: Option<kiln_sema::ModuleImage>,
    /// Sema's pragma state at the end of the translation unit, for the
    /// preprocessor-state record of a PCH (spec/modules.md §11.13).
    pub pragmas: kiln_sema::PragmaState,
}

impl ParsedTu {
    /// The memory report of the translation unit
    /// (`-fkiln-memory-report`, spec/driver.md): every row of
    /// [`Ast::memory`] (the arenas, the type context as `types.*`, the
    /// `journal.*` rows, `instantiation.retained`), then
    /// [`ParsedTu::sema_memory`], then [`Oven::memory`], each in its
    /// producer's order. The rows are the producers' own.
    #[must_use]
    pub fn memory(&self) -> MemReport {
        let mut r = self.ast.memory();
        r.rows.extend(self.sema_memory.rows.iter().cloned());
        r.rows.extend(self.oven.memory().rows);
        r
    }
}

/// Parses and analyses the translation unit whose main file is `file`,
/// reporting to `diags`, and returns its AST: [`parse_tu`] with the
/// default [`ConstexprLimits`].
///
/// `interner` must outlive `diags` (the preprocessor and the AST share
/// it).
#[must_use]
pub fn parse_file<'sm>(
    sm: &'sm SourceManager,
    interner: &'sm Arc<Interner>,
    file: FileId,
    diags: &DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
) -> Ast {
    parse_tu(sm, interner, file, diags, target, opts, ConstexprLimits::default()).ast
}

/// [`parse_file`] with explicit constant-evaluation limits, also returning
/// the TU's Oven. The file is preprocessed with [`PpOptions::new`] (no
/// predefined macros, no include paths): a driver builds its own
/// [`Preprocessor`] and calls [`parse_preprocessed`].
#[must_use]
pub fn parse_tu<'sm>(
    sm: &'sm SourceManager,
    interner: &'sm Arc<Interner>,
    file: FileId,
    diags: &DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> ParsedTu {
    let mut ppo = PpOptions::new(opts.std);
    ppo.char8 = opts.char8;
    ppo.reflection = opts.reflection;
    ppo.parser_pragmas = true;
    // Module units parse from C++20 on (spec/modules.md §2); no module
    // sources are configured, so an import is "module not found".
    if opts.std >= Std::Cxx20 {
        ppo.modules = Some(kiln_pp::ModuleOptions::default());
    }
    let mut pp = Preprocessor::new(sm, diags, interner, ppo);
    pp.enter_main_file(file);
    parse_preprocessed(pp, Arc::clone(interner), diags, target, opts, limits).0
}

/// Sets `s`'s language and option state from `opts`.
fn configure(s: &mut Sema<'_, '_>, opts: &ParseOptions) {
    s.exceptions = opts.exceptions;
    s.rtti = opts.rtti;
    s.reflection = opts.reflection;
    s.coro_aligned_allocation = opts.coro_aligned_allocation;
    s.lax_vector_conversions = opts.lax_vector_conversions;
    s.gnu = opts.gnu;
    s.gnu89_inline = opts.gnu89_inline;
    s.c = opts.c;
    s.clang_compat = opts.clang_compat;
    s.cf_protection_branch = opts.cf_protection_branch;
    s.ast.types.dtst_first_spelling = opts.clang_compat < 22;
    s.gnu_lib_builtins = opts.gnu_lib_builtins;
    s.no_builtin = opts.no_builtin;
    s.no_builtin_fns.clone_from(&opts.no_builtin_fns);
    s.freestanding = opts.freestanding;
    s.sized_deallocation = opts.sized_deallocation;
    s.char8 = opts.char8;
    s.lib_builtin_name = kiln_pp::is_library_builtin;
    s.clang_constexpr_builtin = clang_constexpr_builtin;
    s.math_errno = opts.math_errno;
    s.sve = opts.sve;
    s.sve_vector_bits = opts.sve_vector_bits;
    s.aarch64_cpu_valid = opts.aarch64_cpu_valid;
    s.cfg_dump = opts.cfg_dump;
    s.cfg_stats = opts.cfg_stats;
    s.cfg_max_elems = opts.cfg_max_elems;
    s.permissive = opts.permissive;
    s.ast.contract_semantic = opts.contract_semantic;
    s.ast.lang_c = opts.c.is_some();
    s.full_deduction = opts.full_deduction;
    s.full_redecl_scan = opts.full_redecl_scan;
    s.range_lifetime_extension = opts.std >= Std::Cxx23;
    s.inst.limits = opts.inst_limits;
    s.inst.report.format = opts.inst_report;
}

/// The literal options of a translation unit under `opts` for `target`.
fn literal_options(opts: &ParseOptions, target: &Target) -> LiteralOptions {
    LiteralOptions {
        char8: opts.char8,
        clang19_escapes: opts.clang_compat < 22,
        c: opts.c,
        ..LiteralOptions::new(opts.std, encoding_params(target))
    }
}

/// A new [`Sema`] over an empty AST, configured from `opts` and `limits`:
/// the start of every translation unit, and the object a snapshot is
/// restored into ([`Sema::snap_restore`], spec/parse.md "Snapshots").
#[must_use]
pub fn new_sema<'a, 'sm>(
    interner: Arc<Interner>,
    diags: &'a DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> Sema<'a, 'sm> {
    // Before C++17 the exception specification is not part of the
    // function type ([except.spec], pre-P0012): canonical function types
    // drop it, so type identity, mangling and `typeid` ignore `noexcept`
    // (spec/types.md §5.4).
    let type_opts = TypeOptions { noexcept_in_type: opts.std >= Std::Cxx17 };
    let ast = Ast::new(interner, TypeContext::with_options(target.types.clone(), type_opts));
    let mut s = Sema::for_target(ast, diags, target);
    s.set_constexpr_limits(limits.budget, limits.backtrace_limit);
    if limits.profile != ProfileLevel::Off {
        s.set_constexpr_profile(limits.profile);
    }
    configure(&mut s, opts);
    s
}

/// Parses and analyses the translation unit `pp` produces (its main file
/// entered), reporting to `diags` and evaluating constant expressions
/// under `limits`. `interner` must be the preprocessor's. Returns the
/// parsed TU and the preprocessor (for its macro table and dependency
/// callbacks).
///
/// # Panics
///
/// If `interner` is not `pp`'s interner.
#[must_use]
pub fn parse_preprocessed<'a, 'sm>(
    pp: Preprocessor<'sm, 'a>,
    interner: Arc<Interner>,
    diags: &'a DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> (ParsedTu, Preprocessor<'sm, 'a>) {
    assert!(
        std::ptr::eq(Arc::as_ptr(&interner), std::ptr::from_ref(pp.interner())),
        "the AST and the preprocessor share an interner"
    );
    let lit_opts = literal_options(opts, &target);
    let mut s = new_sema(interner, diags, target, opts, limits);
    if opts.record_image {
        s.record_module_image();
    }
    if opts.record_pch {
        s.record_pch_decl_keys();
    }
    if opts.record_carried {
        s.record_carried_constants();
    }
    let mut pp = pp;
    if let Some(c) = pp.take_main_carried() {
        s.install_carried_constants(&c.path, &c.bytes, Loc::INVALID);
    }
    let main_id = pp.main_file();
    if let Some(l) = &opts.pch {
        s.install_pragma_state(&l.pragmas);
        if let Err(why) = s.load_pch_image(&l.decls, &l.files, l.root) {
            kiln_diag::Diag::new(kiln_diag::DiagId::drv_pch_corrupt, kiln_source::Span::INVALID)
                .arg(format!("{why} in kiln PCH '{}'", l.path))
                .emit(diags);
        }
    }
    let main = main_id.map(|f| diags.sm().file_name(f).to_owned()).unwrap_or_default();
    let mut src = TokenSource::new(pp, diags);
    src.keep_all(opts.keep_tokens);
    let mut p = Parser::new(s, src, lit_opts);
    // The ICE breadcrumb (spec/driver.md "Internal compiler errors").
    let phase = activity::enter(Phase::Parse, Loc::INVALID);
    if opts.pch.is_none() {
        p.warn_if_empty_c_translation_unit();
    }
    p.parse_translation_unit();
    drop(phase);
    kiln_snap::timing::timed_part("finish unit", || finish_tu(p, main_id, &main, opts))
}

/// The end of [`parse_preprocessed`] and [`continue_preprocessed`]: the
/// parser's last events, then the parts of a [`ParsedTu`].
fn finish_tu<'a, 'sm>(
    p: Parser<'a, 'sm>,
    main_id: Option<FileId>,
    main: &str,
    opts: &ParseOptions,
) -> (ParsedTu, Preprocessor<'sm, 'a>) {
    let (s, src) = p.finish();
    let name = main.rsplit('/').next().unwrap_or(main);
    let inst_report = s.instantiation_report(name);
    let mut s = s;
    let cfg_dump = std::mem::take(&mut s.cfg_out);
    let sema_memory = s.memory();
    let carried = s.take_carried_section();
    let image = s.take_module_image();
    let pch_image = main_id.filter(|_| opts.record_pch).map(|m| s.take_pch_image(m));
    let image_census = match main_id {
        Some(m) if opts.image_census && opts.record_pch => Some(s.pch_image_census(m)),
        _ if opts.image_census => s.image_census(),
        _ => None,
    };
    let carried_hits = s.carried_hits();
    let pragmas = s.pragma_state();
    let (ast, oven) = s.into_parts();
    let tu = ParsedTu {
        ast,
        oven,
        inst_report,
        cfg_dump,
        sema_memory,
        carried,
        carried_hits,
        image,
        image_census,
        pch_image,
        pragmas,
    };
    (tu, src.into_preprocessor())
}

/// A prefix parsed to its end by [`parse_prefix`]: the semantic state to
/// snapshot or continue, the preprocessor, and whether the state is at
/// rest for a snapshot.
pub struct ParsedPrefix<'a, 'sm> {
    /// The semantic state at the end of the prefix, before any
    /// end-of-translation-unit work.
    pub sema: Sema<'a, 'sm>,
    /// The preprocessor, at the end of the prefix's main file.
    pub pp: Preprocessor<'sm, 'a>,
    /// `Ok` when the parser and [`Sema::snapshot_ready`] are at rest;
    /// otherwise the first thing that is not (a prefix ending inside a
    /// namespace, a linkage specification or a class; an option a
    /// snapshot does not carry).
    pub ready: Result<(), String>,
}

impl std::fmt::Debug for ParsedPrefix<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedPrefix").field("ready", &self.ready).finish_non_exhaustive()
    }
}

/// Entry point (a) of a snapshot (spec/parse.md "Snapshots"): parses the
/// translation unit `pp` produces as a *prefix*, to the end of its main
/// file, without the end-of-translation-unit work (pending
/// instantiations, the end-of-TU checks), and hands back the parts.
/// `opts` must not record a module image, a PCH or carried constants, nor
/// load a PCH: [`ParsedPrefix::ready`] refuses those.
///
/// # Panics
///
/// If `interner` is not `pp`'s interner.
#[must_use]
pub fn parse_prefix<'a, 'sm>(
    pp: Preprocessor<'sm, 'a>,
    interner: Arc<Interner>,
    diags: &'a DiagEngine<'sm>,
    target: Target,
    opts: &ParseOptions,
    limits: ConstexprLimits,
) -> ParsedPrefix<'a, 'sm> {
    assert!(
        std::ptr::eq(Arc::as_ptr(&interner), std::ptr::from_ref(pp.interner())),
        "the AST and the preprocessor share an interner"
    );
    let refused = if opts.record_image {
        Some("a module image is being recorded")
    } else if opts.record_pch {
        Some("a PCH image is being recorded")
    } else if opts.record_carried {
        Some("carried constants are being recorded")
    } else if opts.pch.is_some() {
        Some("a PCH is loaded")
    } else {
        None
    };
    let lit_opts = literal_options(opts, &target);
    let mut s = new_sema(interner, diags, target, opts, limits);
    // The keys a module-tracking parse would record (spec/modules.md §6),
    // for a consumer that imports a module (WP-1395).
    s.record_prefix_keys();
    let mut src = TokenSource::new(pp, diags);
    src.keep_all(opts.keep_tokens);
    let mut p = Parser::new(s, src, lit_opts);
    let phase = activity::enter(Phase::Parse, Loc::INVALID);
    p.parse_top_level_declarations();
    p.end_prefix();
    p.s.end_prefix_keys();
    drop(phase);
    let ready = match refused {
        Some(why) => Err(why.to_owned()),
        None => p.s.snapshot_ready().and_then(|()| p.snapshot_ready()),
    };
    let (sema, src) = p.finish();
    ParsedPrefix { sema, pp: src.into_preprocessor(), ready }
}

/// Entry point (b) of a snapshot (spec/parse.md "Snapshots"): continues
/// `sema` (the end of a prefix from [`parse_prefix`], or a snapshot of
/// one restored into [`new_sema`]) with the translation unit `pp`
/// produces (its main file entered), to the end, end-of-TU work
/// included. `opts` and `target` must be the prefix's.
///
/// # Panics
///
/// If `sema`'s interner is not `pp`'s.
#[must_use]
pub fn continue_preprocessed<'a, 'sm>(
    sema: Sema<'a, 'sm>,
    pp: Preprocessor<'sm, 'a>,
    target: &Target,
    opts: &ParseOptions,
) -> (ParsedTu, Preprocessor<'sm, 'a>) {
    assert!(
        std::ptr::eq(std::ptr::from_ref(sema.ast.symbols()), std::ptr::from_ref(pp.interner())),
        "the AST and the preprocessor share an interner"
    );
    let diags = sema.diags;
    let lit_opts = literal_options(opts, target);
    let main_id = pp.main_file();
    let main = main_id.map(|f| diags.sm().file_name(f).to_owned()).unwrap_or_default();
    let mut src = TokenSource::new(pp, diags);
    src.keep_all(opts.keep_tokens);
    let mut p = Parser::new(sema, src, lit_opts);
    let phase = activity::enter(Phase::Parse, Loc::INVALID);
    p.parse_translation_unit();
    drop(phase);
    kiln_snap::timing::timed_part("finish unit", || finish_tu(p, main_id, &main, opts))
}
