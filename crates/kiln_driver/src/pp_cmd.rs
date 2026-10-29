//! `-E` (and `-M` / `-MM`) for one input: the predefines, the header search
//! list, the preprocessor, the printer and the dependency list
//! (`spec/driver.md` "Preprocessing").

use std::cell::RefCell;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use kiln_diag::{Diag, DiagEngine, DiagId, DiagOptions};
use kiln_lex::Interner;
use kiln_lex::TokKind;
use kiln_pp::{
    ClangFeatures, DirGroup, HeaderSearch, IfEval, PicModel, PpOptions, PredefConfig, Preprocessor,
    PrintOptions, RealFs, SearchDirSpec, SearchList, build_predefines, print_macros,
    print_preprocessed,
};
use kiln_source::{FileId, SourceManager, Span};

use crate::args::{Args, Lang};
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
        cfg.opt = p.opt;
        cfg.exceptions = p.exceptions.unwrap_or(cplusplus);
        cfg.rtti = p.rtti.unwrap_or(cplusplus);
        cfg.char8_t = p.char8_t;
        cfg.pic = p.pic.unwrap_or(PicModel::Pie(2));
        cfg.fast_math = p.fast_math.unwrap_or(p.ofast);
        cfg.freestanding = p.freestanding.unwrap_or(false);
        cfg.pthread = p.pthread;
        cfg.threadsafe_statics = p.threadsafe_statics.unwrap_or(true);
        cfg.sized_deallocation = p.sized_deallocation.unwrap_or(true);
        cfg.kiln_version = crate::version_pair();
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
                env!("CARGO_PKG_VERSION"),
                self.toolchain.target.triple,
                list.verbose_text()
            );
        }
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
        let deps = a
            .deps
            .headers()
            .map(|h| Rc::new(RefCell::new(DepList::new(h == crate::args::DepHeaders::All))));
        if let Some(d) = &deps {
            let d2 = Rc::clone(d);
            search.set_observer(Box::new(move |sm, e| on_search_event(&d2, sm, e)));
        }
        let mut pp = Preprocessor::new(sm, engine, interner, self.pp_options(predefines));
        pp.set_include_resolver(Box::new(search));
        let triple = self.toolchain.target.triple;
        pp.set_if_evaluator(Box::new(IfEval::for_triple(triple)));
        // `from_args` sees the GNU dialect only through `-std=`; the
        // effective dialect (clang's default is GNU) goes last so it wins.
        let mut fargs = a.argv.clone();
        fargs.push(if self.std.gnu { "-std=gnu++" } else { "-std=c++" }.to_owned());
        let Some(features) = ClangFeatures::from_args(triple, self.std.lex, &fargs) else {
            // No silent fallback to `NoFeatures` (unreachable today:
            // `Target` only has triples the tables cover).
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID).arg(triple).emit(engine);
            return None;
        };
        if staged {
            pp.set_feature_queries(Box::new(StagedFeatures(features)));
        } else {
            pp.set_feature_queries(Box::new(features));
        }
        if let Some(d) = &deps {
            pp.set_callbacks(Box::new(DepCallbacks { list: Rc::clone(d), sm }));
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
        let Some((mut pp, deps)) = self.build_preprocessor(sm, engine, interner, inputs, false) else {
            return;
        };
        if a.deps.only.is_some() {
            while pp.next().kind != TokKind::Eof {}
        } else if a.print.macros_only {
            print_macros(&mut pp, &mut result.output);
        } else {
            let opts =
                PrintOptions { line_markers: !a.print.no_line_markers, line_directives: false };
            print_preprocessed(&mut pp, opts, &mut result.output);
        }
        drop(pp);
        result.deps = deps.map(|d| d.borrow().files().to_vec());
    }
}

/// The language of an input.
pub fn input_lang(lang: Option<Lang>, path: &str) -> Lang {
    lang.unwrap_or_else(|| if path == "-" { Lang::Cxx } else { Lang::from_path(path) })
}

/// clang 19's feature answers with kiln's builtin staging applied
/// (spec/sema-templates.md §13.3): `__has_builtin` and
/// `__has_constexpr_builtin` answer 0 for a trait, transform, builtin
/// template or `__builtin_*` function kiln does not implement yet, so
/// libc++ takes its library fallback instead of reaching
/// `sema_unimplemented_template_feature`.
struct StagedFeatures(ClangFeatures);

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
            _ => v,
        }
    }

    fn has_constexpr_builtin(&self, name: &str) -> bool {
        self.0.has_constexpr_builtin(name)
            && kiln_sema::templ::builtins::table::has_constexpr_builtin_staged(name) != Some(false)
    }

    fn has_attribute(&self, name: &str) -> u32 {
        self.0.has_attribute(name)
    }

    fn has_cpp_attribute(&self, scope: Option<&str>, name: &str) -> u32 {
        self.0.has_cpp_attribute(scope, name)
    }

    fn has_declspec_attribute(&self, name: &str) -> bool {
        self.0.has_declspec_attribute(name)
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
