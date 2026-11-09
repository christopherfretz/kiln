//! `-fsyntax-only [-fdump-ast[=opts]] [-fdump-ast-filter=s]
//! [-fdump-record-layouts[-complete]] [-fdump-vtable-layouts]` for one
//! input: the preprocessor exactly as `-E` builds it
//! ([`Job::build_preprocessor`]), the parser and Sema on its token stream, then the optional record-layout, vtable and
//! AST dumps (`spec/driver.md` "Syntax checking", `spec/ast-dump.md`,
//! `spec/abi.md` §8).

use std::cell::RefCell;
use std::io::Write as _;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use kiln_ast::dump::{DumpOptions, Filter, dump};
use kiln_diag::verify::{FileLookup, Verifier, VerifyOptions};
use kiln_diag::{Diag, DiagEngine, DiagId, MemorySink};
use kiln_lex::Interner;
use kiln_parse::{ParseOptions, parse_preprocessed};
use kiln_sema::target::Target;
use kiln_source::{FileId, Loc, SourceManager, Span};

use crate::make_engine;
use crate::pp_cmd::{Job, JobResult, PpInputs};

/// One input to parse: the same inputs as a preprocessing [`Job`].
#[derive(Debug)]
pub struct SyntaxJob<'j>(pub Job<'j>);

/// The dump options of `-fdump-ast=<opts>` (`ids`, `locs`, `canonical`,
/// `implicit`, `all-files`), or the unknown option.
pub fn dump_options<'a>(
    opts: &str,
    filter: Option<String>,
    sm: &'a SourceManager,
    main: FileId,
) -> Result<DumpOptions<'a>, String> {
    let mut d = DumpOptions {
        filter: filter.map(|name| Filter { name: Some(name), kind: None }),
        sources: Some(sm),
        main_file: Some(main),
        ..DumpOptions::default()
    };
    for o in opts.split(',').filter(|o| !o.is_empty()) {
        match o {
            "ids" => d.ids = true,
            "locs" => d.locs = true,
            "canonical" => d.canonical_types = true,
            "implicit" => d.implicit = true,
            "all-files" => d.all_files = true,
            _ => return Err(o.to_owned()),
        }
    }
    Ok(d)
}

impl SyntaxJob<'_> {
    /// Runs the job: diagnostics (and the `N errors generated.` summary)
    /// to standard error, the AST dump (if asked for) as the output.
    pub fn run(self) -> JobResult {
        use std::io::Write as _;
        let job = &self.0;
        if let Some(v) = &job.args.verify {
            return self.run_verify(v.clone());
        }
        let sm = SourceManager::new();
        // The preprocessor and the AST share the interner; it outlives
        // the engine.
        let interner = Arc::new(Interner::new());
        let engine = make_engine(&sm, job.diag_opts, job.stderr.clone(), None);
        let mut result = JobResult::default();
        // WP-813: a `-cc1` command line clang rejects runs no job.
        let x86 = !job.toolchain.target.is_aarch64();
        let invocation = job.args.codegen.invocation_errors(x86, job.toolchain.target.triple);
        if !invocation.is_empty() {
            for e in &invocation {
                e.emit(&engine);
            }
            engine.flush();
            result.errors = engine.error_count();
            return result;
        }
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                self.parse(&sm, &engine, &interner, inputs, None, &mut result);
            }
        });
        engine.flush();
        if let Some(s) = engine.summary()
            && job.diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        result.errors = engine.error_count();
        result
    }

    /// `-verify` (spec/driver.md "-verify", spec/diag.md §11): the
    /// diagnostics are buffered, the comments scanned as the
    /// preprocessor lexes them, and only the verifier's report printed.
    fn run_verify(self, opts: VerifyOptions) -> JobResult {
        let job = &self.0;
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let sink = MemorySink::new();
        let engine = DiagEngine::new(&sm, job.diag_opts, sink.clone());
        let mut result = JobResult::default();
        let mut verifier = None;
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                let dirs: Vec<String> = inputs.list.dirs().iter().map(|d| d.path.clone()).collect();
                let v = Verifier::new(&sm, opts.clone(), inputs.main.file(), quoted_lookup(dirs));
                let v = Rc::new(RefCell::new(v));
                self.parse(&sm, &engine, &interner, inputs, Some(&v), &mut result);
                verifier = Some(v);
            }
        });
        let verifier = verifier.unwrap_or_else(|| {
            Rc::new(RefCell::new(Verifier::new(&sm, opts, None, Box::new(|_, _, _| None))))
        });
        verifier.borrow_mut().finish(&engine);
        engine.flush();
        let report = verifier.borrow().check(&sink.take());
        let out = make_engine(&sm, job.diag_opts, job.stderr.clone(), None);
        report.emit(&out);
        out.flush();
        if let Some(s) = report.summary()
            && job.diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        result.errors = report.problems;
        result
    }

    /// `-fdump-ast`'s options, or `Err` once the reason is diagnosed.
    fn dump_options<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        main: Option<FileId>,
    ) -> Result<Option<DumpOptions<'sm>>, ()> {
        let a = self.0.args;
        Ok(match (&a.dump_ast, main) {
            (Some(_), None) => {
                Diag::new(DiagId::drv_not_implemented, Span::INVALID)
                    .arg("-fdump-ast of a module file")
                    .emit(engine);
                return Err(());
            }
            (Some(o), Some(main)) => match dump_options(o, a.dump_ast_filter.clone(), sm, main) {
                Ok(d) => {
                    Some(DumpOptions { written_tags: a.predef.clang_compat.major() >= 22, ..d })
                }
                Err(bad) => {
                    Diag::new(DiagId::drv_invalid_value, Span::INVALID)
                        .arg(format!("-fdump-ast={o}"))
                        .arg(bad)
                        .emit(engine);
                    return Err(());
                }
            },
            (None, _) => None,
        })
    }

    fn parse<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
        verify: Option<&Rc<RefCell<Verifier<'sm>>>>,
        result: &mut JobResult,
    ) {
        let job = &self.0;
        let a = job.args;
        let Ok(dump_opts) = self.dump_options(sm, engine, inputs.main.file()) else {
            return;
        };
        let Ok(mut target) = Target::from_triple(job.toolchain.target.triple) else {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
                .arg(job.toolchain.target.triple)
                .emit(engine);
            return;
        };
        if let Some(signed) = a.predef.char_signed {
            target.types.char_signed = signed;
        }
        let x86 = !job.toolchain.target.is_aarch64();
        // WP-630: `cx16` makes 128-bit atomics lock-free (clang
        // `X86TargetInfo::setMaxAtomicWidth`).
        if crate::features::resolve(a, &job.toolchain.target).0.x86_cx16() {
            target.types.max_atomic_inline_bytes = 16;
        }
        let Some((mut pp, deps)) = job.build_preprocessor(sm, engine, interner, inputs, true)
        else {
            return;
        };
        if let Some(v) = verify {
            let v = Rc::clone(v);
            pp.set_comment_handler(Box::new(move |c| v.borrow_mut().handle_comment(c, engine)));
        }
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        let popts = ParseOptions {
            std: job.std.lex,
            gnu: job.std.gnu,
            exceptions: a.predef.exceptions.unwrap_or(true),
            rtti: a.predef.rtti.unwrap_or(true),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            reflection: a.predef.reflection_for(&job.std),
            coro_aligned_allocation: a.predef.coro_aligned_allocation,
            lax_vector_conversions: a.lax_vector_conversions,
            clang_compat: a.predef.clang_compat.major(),
            gnu_lib_builtins: a.stdlib.unwrap_or_default() == crate::toolchain::StdLib::LibStdCxx,
            no_builtin: a.no_builtin(),
            no_builtin_fns: a.no_builtin_fns(),
            sized_deallocation: a.predef.sized_deallocation.unwrap_or(true),
            char8: a.predef.char8_t,
            math_errno: a.fp_model().opts.math_errno,
            sve: feats.sve,
            sve_vector_bits: feats.sve_vector_bits,
            cfg_dump: a.dump_cfg,
            cfg_stats: a.cfg_stats,
            cfg_max_elems: a.cfg_max_elems.unwrap_or(kiln_sema::cfg::DEFAULT_MAX_ELEMS),
            permissive: a.permissive,
            contract_semantic: a.contract_semantic.unwrap_or_default(),
            cf_protection_branch: a.codegen.cf_protection_branch(x86),
            record_carried: a.bmi_kind != Some(1) && job.module_output_path().is_some(),
            ..ParseOptions::default()
        };
        let limits = a.constexpr.limits();
        let (tu, mut pp) =
            parse_preprocessed(pp, Arc::clone(interner), engine, target.clone(), &popts, limits);
        if engine.error_count() == 0 {
            job.write_module_file(&mut pp, engine, tu.carried.as_ref());
        }
        if verify.is_some() || deps.is_some() {
            // The parser stops at a fatal error; clang's preprocessor lexes
            // on to the end (its diagnostics suppressed), so every comment
            // still reaches the verifier (spec/driver.md "-verify") and
            // every header the `-MD` list (spec/driver.md "Dependency
            // output").
            while !pp.next().is_eof() {}
        }
        drop(pp);
        result.dep_list = deps;
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
        result.output.push_str(&tu.cfg_dump);
        let ast = tu.ast;
        if a.dump_record_layouts {
            // clang prints each class as Sema completes it, before any
            // AST dump at the end of the translation unit.
            let layouts = kiln_abi::dump_all_record_layouts(&ast, &target, Some(sm));
            result.output.push_str(&layouts);
        }
        if a.dump_vtable_layouts {
            result.output.push_str(&kiln_abi::vtable::dump_all_vtable_layouts(&ast, &target));
        }
        if let Some(d) = dump_opts {
            let mut out: Vec<u8> = Vec::new();
            if dump(&ast, &d, &mut out).is_ok() {
                result.output.push_str(&String::from_utf8_lossy(&out));
            }
        }
    }
}

/// clang `Preprocessor::LookupFile` for a quoted name, as `-verify`'s
/// `@<file>:<line>` uses it: the directory of the file holding the
/// directive, then the search list; an absolute name as is.
fn quoted_lookup<'a>(dirs: Vec<String>) -> FileLookup<'a> {
    Box::new(move |sm: &SourceManager, at: Loc, name: &str| {
        let path = Path::new(name);
        if path.is_absolute() {
            return sm.load_file(path).ok();
        }
        let here = sm
            .offset_of(at)
            .map(|(f, _)| Path::new(sm.file_name(f)).parent().unwrap_or(Path::new("")).to_owned());
        here.into_iter()
            .chain(dirs.iter().map(|d| Path::new(d).to_owned()))
            .map(|d| d.join(path))
            .find(|p| p.is_file())
            .and_then(|p| sm.load_file(&p).ok())
    })
}
