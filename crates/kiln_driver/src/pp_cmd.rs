//! `-E` (and `-M` / `-MM`) for one input: the predefines, the header search
//! list, the preprocessor, the printer and the dependency list
//! (`spec/driver.md` "Preprocessing").

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use kiln_ast::attr::AttrSyntax;
use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions};
use kiln_lex::Interner;
use kiln_lex::TokKind;
use kiln_pp::{
    ClangFeatures, DirGroup, HeaderSearch, IfEval, PicModel, PpOptions, PredefConfig, Preprocessor,
    PrintOptions, RealFs, SearchDirSpec, SearchList, build_predefines, print_macros,
    print_preprocessed,
};
use kiln_source::{FileId, SourceManager, Span};

use crate::args::Args;
use crate::deps::{DepCallbacks, DepList, on_search_event};
use crate::toolchain::{LangStd, SystemIncludes, Toolchain};
use crate::{SharedWriter, make_engine, time};

/// What one input produced.
#[derive(Debug, Default)]
pub struct JobResult {
    /// The preprocessed text (or the `-M` dependency text).
    pub output: String,
    /// The dependency list, when dependencies were requested.
    pub deps: Option<Vec<String>>,
    /// `-fsyntax-only`'s list for `-MD` / `-MMD`, complete once the
    /// preprocessor reached the end of the input (the driver writes it).
    pub dep_list: Option<SharedDeps>,
    /// Errors reported.
    pub errors: u32,
}

/// The dependency list a preprocessor's callbacks fill (`-M`, `-MD`).
pub(crate) type SharedDeps = Rc<RefCell<DepList>>;

/// What [`Job::build_preprocessor`] needs besides the job.
#[derive(Debug)]
pub(crate) struct PpInputs {
    /// The header search list.
    pub list: SearchList,
    /// The main file.
    pub main: FileId,
    /// The predefines buffer (`<built-in>`).
    pub predefines: String,
}

/// One input to preprocess.
#[derive(Debug)]
pub struct Job<'j> {
    /// The command line.
    pub args: &'j Args,
    /// The toolchain.
    pub toolchain: &'j Toolchain,
    /// The standard.
    pub std: LangStd,
    /// The input path (`-` for standard input).
    pub path: &'j str,
    /// Standard input's text, for `-`.
    pub stdin_text: Option<String>,
    /// Diagnostic options.
    pub diag_opts: &'j DiagOptions,
    /// Standard error.
    pub stderr: SharedWriter,
}

impl Job<'_> {
    /// The full search list specification: the user's directories, then the
    /// toolchain's.
    pub fn search_specs(
        args: &Args,
        tc: &Toolchain,
        header_sysroot: &str,
        cplusplus: bool,
    ) -> Vec<SearchDirSpec> {
        let mut specs: Vec<SearchDirSpec> = args
            .dirs
            .iter()
            .map(|(g, d)| {
                // `-I=dir` is relative to the sysroot (clang `AddPath` with
                // `IsSysrootSpecified`).
                let path = match d.strip_prefix('=') {
                    Some(rest) if !header_sysroot.is_empty() => format!("{header_sysroot}{rest}"),
                    _ => d.clone(),
                };
                SearchDirSpec::new(path, *g)
            })
            .collect();
        let sys = SystemIncludes {
            nostdinc: args.nostdinc,
            nostdincxx: args.nostdincxx,
            nostdlibinc: args.nostdlibinc,
            nobuiltininc: args.nobuiltininc,
            stdlib: args.stdlib.unwrap_or_default(),
        };
        specs.extend(tc.system_include_specs(&sys, cplusplus));
        // `-cxx-isystem` only applies to C++.
        if !cplusplus {
            specs.retain(|s| s.group != DirGroup::CxxSystem);
        }
        specs
    }

    fn predefines(&self, engine: &DiagEngine<'_>) -> Option<String> {
        let path = self.toolchain.predefines_path(&self.std);
        let base = match std::fs::read_to_string(&path) {
            Ok(b) => b,
            Err(e) => {
                Diag::new(DiagId::drv_cannot_read_predefines, Span::INVALID)
                    .arg(path.display())
                    .arg(e)
                    .emit(engine);
                return None;
            }
        };
        let p = &self.args.predef;
        let cplusplus = self.std.cplusplus;
        let mut cfg = PredefConfig::new(cplusplus);
        cfg.gnu = self.std.gnu;
        cfg.aarch64 = self.toolchain.target.is_aarch64();
        if cfg.aarch64 {
            // Errors were reported by the driver's own `resolve` pass.
            let f = crate::features::resolve(self.args, &self.toolchain.target).0;
            cfg.arm = kiln_pp::ArmTarget {
                arch: f.arch,
                features: f.enabled(),
                sve_vector_bits: f.sve_vector_bits,
                branch_protection: (self.args.codegen.branch_protection(true))
                    .unwrap_or_default()
                    .macros()
                    .into_iter()
                    .map(|(n, v)| (n.to_owned(), v))
                    .collect(),
            };
        } else if let Some(cpu) = crate::features::resolve(self.args, &self.toolchain.target).0.cpu
        {
            // WP-630: the `-march=<cpu>` macros.
            cfg.x86_macros = crate::features::x86_cpu_macros(&cpu);
        }
        // Errors were reported by the driver's own `resolve` pass.
        cfg.sanitize_macros = self
            .args
            .sanitize(!cfg.aarch64, self.toolchain.target.triple)
            .0
            .macros(p.clang_compat.major() >= 22);
        cfg.opt = p.opt;
        cfg.no_inline = p.no_inline;
        cfg.implicit_includes.clone_from(&self.args.includes);
        cfg.exceptions = p.exceptions.unwrap_or(cplusplus);
        cfg.rtti = p.rtti.unwrap_or(cplusplus);
        cfg.char8_t = p.char8_t;
        cfg.char_signed = p.char_signed;
        cfg.pic = p.pic.unwrap_or(PicModel::Pie(2));
        let fp = self.args.fp_model();
        cfg.fast_math = fp.fast_math;
        cfg.finite_math = fp.opts.nnan && fp.opts.ninf;
        cfg.no_math_errno = !fp.opts.math_errno;
        cfg.unsafe_math = fp.unsafe_math;
        cfg.freestanding = p.freestanding.unwrap_or(false);
        cfg.pthread = p.pthread;
        cfg.threadsafe_statics = p.threadsafe_statics.unwrap_or(true);
        cfg.sized_deallocation = p.sized_deallocation.unwrap_or(true);
        cfg.kiln_version = crate::version::triple();
        cfg.version_macro = Some(format!("kiln {}", crate::version::VERSION));
        cfg.clang_compat = p.clang_compat;
        cfg.reflection = p.reflection_for(&self.std);
        cfg.std_float_aliases =
            self.args.stdlib.unwrap_or_default() != crate::toolchain::StdLib::LibCxx;
        Some(build_predefines(&base, &cfg, &self.args.macros))
    }

    fn pp_options(&self, predefines: String) -> PpOptions {
        let a = self.args;
        let mut o = PpOptions::new(self.std.lex);
        o.trigraphs = a.trigraphs.unwrap_or(
            !(self.std.gnu || self.std.cplusplus && self.std.lex >= kiln_lex::Std::Cxx17),
        );
        o.dollar_in_identifiers = a.dollars.unwrap_or(true);
        o.keep_comments = a.print.comments || a.print.macro_comments;
        o.keep_macro_comments = a.print.macro_comments;
        o.reflection = a.predef.reflection_for(&self.std);
        o.char8 = a.predef.char8_t;
        o.predefines = predefines;
        if let Some(epoch) = time::reproducible_epoch(a.reproducible_time) {
            let (date, t, stamp) = time::spellings(epoch);
            o.reproducible_time = Some((date, t));
            o.timestamp = Some(stamp);
        }
        o
    }

    fn load_main(&self, sm: &SourceManager, engine: &DiagEngine<'_>) -> Option<FileId> {
        let r = match &self.stdin_text {
            Some(t) => sm.add_virtual("<stdin>", t.clone()).map_err(|e| e.to_string()),
            None => sm.load_file(Path::new(self.path)).map_err(|e| e.to_string()),
        };
        match r {
            Ok(f) => Some(f),
            Err(e) => {
                Diag::new(DiagId::drv_cannot_read_input, Span::INVALID)
                    .arg(self.path)
                    .arg(e)
                    .emit(engine);
                None
            }
        }
    }

    /// Runs the job. Diagnostics (and the `N errors generated.` summary)
    /// go to standard error.
    pub fn run(self) -> JobResult {
        let sm = SourceManager::new();
        let interner = Interner::new();
        let mut diag_opts = self.diag_opts.clone();
        diag_opts.pending.clear();
        if self.args.deps.only.is_some() {
            // `-M` / `-MM` imply `-w`.
            diag_opts.ignore_warnings = true;
        }
        let engine = make_engine(&sm, &diag_opts, self.stderr.clone(), None);
        let mut result = JobResult::default();
        if let Some(inputs) = self.pp_inputs(&sm, &engine) {
            self.preprocess(&sm, &engine, &interner, inputs, &mut result);
        }
        engine.flush();
        if let Some(s) = engine.summary()
            && diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = self.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        result.errors = engine.error_count();
        result
    }

    /// The search list (printed for `-v`), the main file and the
    /// predefines of this input; `None` after reporting an error.
    pub(crate) fn pp_inputs(
        &self,
        sm: &SourceManager,
        engine: &DiagEngine<'_>,
    ) -> Option<PpInputs> {
        let cplusplus = self.std.cplusplus;
        let header_sysroot =
            self.args.isysroot.as_deref().or(self.args.sysroot.as_deref()).unwrap_or("");
        let specs = Self::search_specs(self.args, self.toolchain, header_sysroot, cplusplus);
        let list = SearchList::build(&specs, cplusplus, &mut RealFs);
        if self.args.verbose {
            let mut err = self.stderr.clone();
            let _ = write!(
                err,
                "kiln -cc1 version {} default target {}\n{}",
                crate::version::VERSION,
                self.toolchain.target.triple,
                list.verbose_text()
            );
        }
        sm.set_macro_prefix_map(&self.args.macro_prefix_map);
        sm.set_source_path_style(self.args.source_path_style);
        let main = self.load_main(sm, engine);
        let predefines = self.predefines(engine);
        Some(PpInputs { list, main: main?, predefines: predefines? })
    }

    /// The preprocessor of this input, exactly as `-E` configures it
    /// (header search, `#if` evaluation, `__has_feature` and friends,
    /// dependency collection), with the main file entered. Shared by
    /// `-E`, `-M` and `-fsyntax-only` (spec/driver.md). `None` after
    /// reporting an error. `staged` applies kiln's builtin staging to
    /// `__has_builtin` (spec/sema-templates.md §9, Q10): on for the
    /// modes that run sema, off for `-E`/`-M`, whose output stays
    /// token-identical to clang's.
    pub(crate) fn build_preprocessor<'a, 'd>(
        &self,
        sm: &'a SourceManager,
        engine: &'d DiagEngine<'a>,
        interner: &'a Interner,
        inputs: PpInputs,
        staged: bool,
    ) -> Option<(Preprocessor<'a, 'd>, Option<SharedDeps>)> {
        let a = self.args;
        let PpInputs { list, main, predefines } = inputs;
        let mut search = HeaderSearch::new(RealFs, list).with_embed_dirs(a.embed_dirs.clone());
        let deps = DepList::for_args(a).map(|l| Rc::new(RefCell::new(l)));
        if let Some(d) = &deps {
            let d2 = Rc::clone(d);
            search.set_observer(Box::new(move |_, e| on_search_event(&d2, e)));
        }
        let mut opts = self.pp_options(predefines);
        opts.parser_pragmas = staged;
        if !staged {
            // `-E -dD`/`-dI` print the directives in place (`-dM` and
            // `-M` print no text); `-M -MG` lists a missing header
            // instead of reporting it.
            let printing = a.deps.only.is_none() && !a.print.macros_only;
            opts.dump_defines = printing && a.print.defines_too;
            opts.dump_includes = printing && a.print.includes_too;
            opts.suppress_include_not_found = a.deps.missing_generated && a.deps.only.is_some();
        }
        // Modules for the modes that run sema, from C++20 (spec/modules.md
        // §2); `-E`/`-M` keep the directives as text.
        if staged && self.std.cplusplus && self.std.lex >= kiln_lex::Std::Cxx20 {
            opts.modules = Some(kiln_pp::ModuleOptions {
                module_files: a
                    .module_files
                    .iter()
                    .map(|(n, p)| (n.clone(), std::path::PathBuf::from(p)))
                    .collect(),
                prebuilt_paths: a.prebuilt_module_paths.iter().map(Into::into).collect(),
                std_dir: Some(self.toolchain.libcxx_module_dir()),
            });
        }
        let mut pp = Preprocessor::new(sm, engine, interner, opts);
        pp.set_include_resolver(Box::new(search));
        let triple = self.toolchain.target.triple;
        let mut ifeval = IfEval::for_triple(triple);
        if let Some(signed) = a.predef.char_signed {
            let mut params = ifeval.params();
            params.char_signed = signed;
            ifeval = IfEval::new(params);
        }
        pp.set_if_evaluator(Box::new(ifeval));
        // `from_args` sees the GNU dialect only through `-std=`; the
        // effective dialect (clang's default is GNU) goes last so it wins.
        // WP-675: `__has_feature(address_sanitizer)` and friends answer
        // from the resolved `-fsanitize=` set, not the raw arguments.
        let mut fargs: Vec<String> = a
            .argv
            .iter()
            .filter(|s| !s.starts_with("-fsanitize=") && !s.starts_with("-fno-sanitize="))
            .cloned()
            .collect();
        fargs.extend(a.sanitize(!self.toolchain.target.is_aarch64(), triple).0.feature_args());
        fargs.push(if self.std.gnu { "-std=gnu++" } else { "-std=c++" }.to_owned());
        let mut features = ClangFeatures::from_args(triple, self.std.lex, &fargs)
            .map(|f| f.with_compat(a.predef.clang_compat));
        if self.toolchain.target.is_aarch64() {
            // WP-558: the NEON builtins answer from the enabled features.
            let enabled = crate::features::resolve(a, &self.toolchain.target).0.enabled();
            features = features.map(|f| {
                f.with_target_builtins(kiln_pp::TargetBuiltins {
                    prefix: kiln_ast::decl::builtin::neon::PREFIX,
                    required: kiln_ast::decl::builtin::neon::required_features,
                    enabled,
                })
            });
        } else {
            // WP-630: and the x86 ones.
            let enabled = crate::features::resolve(a, &self.toolchain.target).0.x86_enabled();
            features = features.map(|f| {
                f.with_target_builtins(kiln_pp::TargetBuiltins {
                    prefix: kiln_ast::decl::builtin::x86::PREFIX,
                    required: kiln_ast::decl::builtin::x86::required_features,
                    enabled,
                })
            });
        }
        let Some(features) = features else {
            // No silent fallback to `NoFeatures` (unreachable today:
            // `Target` only has triples the tables cover).
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID).arg(triple).emit(engine);
            return None;
        };
        if staged {
            let clang19 = features.clone().with_compat(kiln_pp::ClangCompat::Clang19);
            pp.set_feature_queries(Box::new(StagedFeatures(features, clang19)));
        } else {
            pp.set_feature_queries(Box::new(features));
        }
        if let Some(d) = &deps {
            pp.set_callbacks(Box::new(DepCallbacks::new(Rc::clone(d), sm)));
        }
        pp.enter_main_file(main);
        Some((pp, deps))
    }

    fn preprocess<'a>(
        &self,
        sm: &'a SourceManager,
        engine: &DiagEngine<'a>,
        interner: &'a Interner,
        inputs: PpInputs,
        result: &mut JobResult,
    ) {
        let a = self.args;
        let Some((mut pp, deps)) = self.build_preprocessor(sm, engine, interner, inputs, false)
        else {
            return;
        };
        if a.deps.only.is_some() {
            while pp.next().kind != TokKind::Eof {}
        } else if a.print.macros_only {
            print_macros(&mut pp, &mut result.output);
        } else {
            let opts = PrintOptions {
                line_markers: !a.print.no_line_markers,
                line_directives: false,
                ucn_as_utf8: a.predef.clang_compat.major() >= 22,
            };
            print_preprocessed(&mut pp, opts, &mut result.output);
        }
        drop(pp);
        result.deps = deps.map(|d| d.borrow().files().to_vec());
    }
}

/// The `-MD` / `-MMD` list of a mode that runs the parser, once `pp` is
/// done: the parser stops at a fatal error, so the preprocessor lexes on
/// to the end (the engine drops its diagnostics) and the list has every
/// header (spec/driver.md "Dependency output").
pub(crate) fn finish_deps(
    mut pp: Preprocessor<'_, '_>,
    deps: Option<SharedDeps>,
) -> Option<SharedDeps> {
    if deps.is_some() {
        while pp.next().kind != TokKind::Eof {}
    }
    deps
}

/// WP-805: the library builtins in the reserved `__` namespace that clang
/// answers `__has_builtin` for but declares only through a header (glibc's
/// `__sigsetjmp`, `__finite`, `__exp10`, Apple libm's `__sinpi` family,
/// `std::__addressof`, `<ia32intrin.h>`'s `__rdtsc`). Kiln calls the
/// declared function, so they keep clang's answer. Sorted.
const RESERVED_LIBRARY_BUILTINS: &[&str] = &[
    "__addressof",
    "__cospi",
    "__cospif",
    "__exp10",
    "__exp10f",
    "__finite",
    "__finitef",
    "__finitel",
    "__rdtsc",
    "__sigsetjmp",
    "__sinpi",
    "__sinpif",
    "__tanpi",
    "__tanpif",
];

/// clang 19's feature answers with kiln's builtin staging applied
/// (spec/sema-templates.md §13.3): `__has_builtin` and
/// `__has_constexpr_builtin` answer 0 for a trait, transform, builtin
/// template or `__builtin_*` function kiln does not implement yet, so
/// libc++ takes its library fallback instead of reaching
/// `sema_unimplemented_template_feature`. A `__builtin_*` name only
/// clang 22 has (the second field, the clang 19 answers, says 0) that kiln
/// neither declares nor stages answers 0 too, so a guarded use such as
/// `#if __has_builtin(__builtin_infer_alloc_token)` is skipped rather
/// than reaching "use of undeclared identifier".
///
/// WP-805 (spec/sema.md §4.5): the answers are truthful. `__has_builtin`
/// answers 0 for a builtin function kiln does not declare, and for one
/// it declares but cannot lower ([`kiln_codegen::UNLOWERED_BUILTINS`]).
/// `__has_attribute`, `__has_cpp_attribute` and
/// `__has_declspec_attribute` answer 0 for a class-A attribute kiln
/// rejects ([`kiln_sema::attr_triage`]). `-E` keeps clang's raw answers.
struct StagedFeatures(ClangFeatures, ClangFeatures);

impl StagedFeatures {
    /// Whether `name` is a `__builtin_*` function clang 19 lacks that kiln
    /// neither declares nor lists in its staging table.
    fn undeclared_clang22(&self, name: &str) -> bool {
        name.starts_with("__builtin_")
            && kiln_pp::FeatureQueries::has_builtin(&self.1, name) == 0
            && kiln_ast::decl::builtin::by_name(name).is_none()
            && kiln_sema::templ::builtins::table::lookup(name).is_none()
    }

    /// Whether `name`, which clang answers `__has_builtin` for, is a
    /// builtin kiln neither handles (a keyword, a preprocessor builtin, a
    /// staged sema builtin, a declared function) nor lowers. A library
    /// builtin outside the reserved `__` namespace (`malloc`,
    /// `std::move`, `_Exit`) is a call to the function its header
    /// declares, so it keeps clang's answer.
    fn unavailable_builtin(&self, name: &str) -> bool {
        let compat = self.0.compat().major();
        let declared = kiln_ast::decl::builtin::by_name_at(name, compat).is_some()
            || kiln_ast::decl::builtin::lib_by_name(name).is_some();
        if declared {
            return kiln_codegen::is_unlowered_builtin(name);
        }
        name.starts_with("__")
            && RESERVED_LIBRARY_BUILTINS.binary_search(&name).is_err()
            && !kiln_pp::FeatureQueries::is_keyword(&self.0, name)
            && !kiln_pp::Builtin::ALL.iter().any(|&(n, _)| n == name)
            && kiln_sema::templ::builtins::table::lookup(name).is_none()
    }

    /// Whether attribute `name` written with `syntax` and `scope` is one
    /// kiln rejects as class A (and so feature-tests as absent).
    fn unsupported_attribute(&self, syntax: AttrSyntax, scope: Option<&str>, name: &str) -> bool {
        let scope = scope.map(|s| match s {
            "__gnu__" => "gnu",
            "_Clang" => "clang",
            s => s,
        });
        // Normalized as `Sema::act_on_attribute` normalizes it (clang
        // `normalizeAttrName`: `__clang_arm_builtin_alias` and `__const`
        // keep their leading `__`).
        let name = if kiln_ast::attr::attr_name_normalizes(syntax, scope) {
            kiln_ast::attr::normalize_attr_name(name)
        } else {
            name
        };
        kiln_sema::attr_triage::attribute_support(
            name,
            syntax,
            scope,
            self.0.compat().major(),
            self.0.is_cxx26(),
            self.0.is_x86_64(),
        ) == kiln_sema::attr_triage::Support::Unsupported
    }
}

impl kiln_pp::FeatureQueries for StagedFeatures {
    fn has_feature(&self, name: &str) -> bool {
        self.0.has_feature(name)
    }

    fn has_extension(&self, name: &str) -> bool {
        self.0.has_extension(name)
    }

    fn has_builtin(&self, name: &str) -> u32 {
        let v = self.0.has_builtin(name);
        match kiln_sema::templ::builtins::table::has_builtin_staged(name) {
            Some(false) => 0,
            // A GCC builtin clang lacks (WP-484: `__builtin_is_string_literal`).
            Some(true) if v == 0 && kiln_sema::templ::builtins::table::gcc_only(name) => 1,
            None if v != 0 && self.undeclared_clang22(name) => 0,
            None if v != 0 && self.unavailable_builtin(name) => 0,
            _ => v,
        }
    }

    fn has_constexpr_builtin(&self, name: &str) -> bool {
        self.0.has_constexpr_builtin(name)
            && kiln_sema::templ::builtins::table::has_constexpr_builtin_staged(name) != Some(false)
            && !self.undeclared_clang22(name)
    }

    fn has_attribute(&self, name: &str) -> u32 {
        let v = self.0.has_attribute(name);
        if v != 0 && self.unsupported_attribute(AttrSyntax::Gnu, None, name) { 0 } else { v }
    }

    fn has_cpp_attribute(&self, scope: Option<&str>, name: &str) -> u32 {
        let v = self.0.has_cpp_attribute(scope, name);
        if v != 0 && self.unsupported_attribute(AttrSyntax::Cxx11, scope, name) { 0 } else { v }
    }

    fn has_declspec_attribute(&self, name: &str) -> bool {
        self.0.has_declspec_attribute(name)
            && !self.unsupported_attribute(AttrSyntax::Declspec, None, name)
    }

    fn is_keyword(&self, name: &str) -> bool {
        self.0.is_keyword(name)
    }

    fn is_target(&self, query: kiln_pp::TargetQuery, name: &str) -> bool {
        self.0.is_target(query, name)
    }

    fn flt_eval_method(&self) -> i32 {
        self.0.flt_eval_method()
    }
}
