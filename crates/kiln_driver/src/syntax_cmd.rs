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
            if let Some(s) = &job.args.pch.snapshot_use {
                // spec/pch.md "Consumer": a `-verify` consumer
                // restores when the header reported nothing.
                match self.run_verify_inner(v.clone(), Some(s)) {
                    Ok(r) => return r,
                    Err(why) => {
                        let why = format!("not used ({why}); the header is replayed");
                        crate::pch_snapshot::note(job.args, &job.stderr, &s.pch, &why);
                    }
                }
            }
            return self.run_verify_inner(v.clone(), None).unwrap_or_default();
        }
        if let Some(s) = &job.args.pch.snapshot_use {
            match self.run_restored(s) {
                Ok(r) => {
                    crate::pch_snapshot::phase("teardown of the roots");
                    return r;
                }
                Err(why) => {
                    let why = format!("not used ({why}); the header is replayed");
                    crate::pch_snapshot::note(job.args, &job.stderr, &s.pch, &why);
                }
            }
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
                self.parse(&sm, &engine, &interner, inputs, None, &mut result, None);
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

    /// WP-1383: the job with the `-include-pch` file's
    /// snapshot restored in place of a replay of its header
    /// (`crate::pch_snapshot`); `Err` (nothing reported) when it cannot
    /// be restored, and the caller replays.
    pub(crate) fn run_restored(
        &self,
        snap: &crate::args::PchSnapshotUse,
    ) -> Result<JobResult, String> {
        use std::io::Write as _;
        let job = &self.0;
        crate::pch_snapshot::phase("job setup");
        let (sm, interner, mut restore) = crate::pch_snapshot::open(snap)?;
        let restored = sm.file_count();
        let mut engine = make_engine(&sm, job.diag_opts, job.stderr.clone(), None);
        engine.snap_restore(&mut restore.reader).map_err(|e| e.to_string())?;
        restore.timer.mark("DiagEngine");
        let mut result = JobResult::default();
        let x86 = !job.toolchain.target.is_aarch64();
        let invocation = job.args.codegen.invocation_errors(x86, job.toolchain.target.triple);
        if !invocation.is_empty() {
            return Err("a -cc1 invocation error".to_owned());
        }
        let mut failed = None;
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                let mut restore = Some(restore);
                self.parse(&sm, &engine, &interner, inputs, None, &mut result, restore.as_mut());
                crate::pch_snapshot::phase("reports and teardown of the AST");
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
            && job.diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = job.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        result.errors = engine.error_count();
        Ok(result)
    }

    /// WP-1383: the producer's snapshot pass. The job's
    /// input is an empty main file whose `-include`s are the producer's
    /// and the header; it is parsed to its end without the
    /// end-of-translation-unit work, closed, and every root written, in
    /// restore order. `Err`: why there is no snapshot.
    ///
    /// With `single` (WP-1393, `crate::pch_snapshot::produce`), the pass
    /// then goes on as a consumer with an empty main file would: the main
    /// file entered again and the end-of-translation-unit work done. The
    /// second result is then the header pass's own result, when this pass
    /// shows that the header pass would add nothing to it
    /// ([`crate::pch_snapshot::stands_in`]).
    pub(crate) fn snapshot(
        self,
        single: Option<&crate::pch_snapshot::Single<'_>>,
    ) -> (Result<crate::pch_snapshot::Roots, String>, crate::pch_snapshot::StandIn) {
        let job = &self.0;
        let sm = SourceManager::new();
        let interner = Arc::new(Interner::new());
        let engine = make_engine(&sm, job.diag_opts, job.stderr.clone(), None);
        let none = "the preprocessor could not be built";
        let mut out: (Result<crate::pch_snapshot::Roots, String>, crate::pch_snapshot::StandIn) =
            (Err(none.to_owned()), Err(none.to_owned()));
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                out = self.snapshot_parse(&sm, &engine, &interner, inputs, single);
            }
        });
        engine.flush();
        if out.1.is_ok()
            && let Some(single) = single
            && !single.quiet()
        {
            out.1 = Err(format!("the pass wrote {} byte(s) of diagnostics", single.written()));
        }
        out
    }

    fn snapshot_parse<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
        single: Option<&crate::pch_snapshot::Single<'_>>,
    ) -> (Result<crate::pch_snapshot::Roots, String>, crate::pch_snapshot::StandIn) {
        let (bytes, (p, deps, target, popts)) =
            match self.snapshot_encode(sm, engine, interner, inputs) {
                Ok(x) => x,
                Err(why) => {
                    let stand = Err(format!("no snapshot: {why}"));
                    return (Err(why), stand);
                }
            };
        let Some(single) = single else { return (Ok(bytes), Err("two passes".to_owned())) };
        engine.flush();
        if !single.quiet() {
            let why = format!("the prefix wrote {} byte(s) of diagnostics", single.written());
            return (Ok(bytes), Err(why));
        }
        let kiln_parse::ParsedPrefix { sema, mut pp, .. } = p;
        let Some(main) = pp.main_file() else {
            return (Ok(bytes), Err("the prefix has no main file".to_owned()));
        };
        if let Err(e) = pp.enter_next_main_file(main) {
            return (Ok(bytes), Err(format!("the main file cannot be entered again: {e}")));
        }
        let (tu, mut pp) = kiln_parse::continue_preprocessed(sema, pp, &target, &popts);
        // As `after_parse` with a dependency list: the preprocessor lexes
        // on to the end after a fatal error.
        while !pp.next().is_eof() {}
        drop(pp);
        engine.flush();
        let after = [
            (engine.error_count() > 0, "errors"),
            (!tu.inst_report.is_empty(), "an instantiation report"),
            (!tu.cfg_dump.is_empty(), "a CFG dump"),
        ];
        if let Some((_, what)) = after.iter().find(|a| a.0) {
            return (Ok(bytes), Err(format!("the end of the translation unit gave {what}")));
        }
        let r = crate::pch_snapshot::stands_in(single, sm, engine, deps);
        (Ok(bytes), r)
    }

    /// The snapshot pass's prefix and its encoding, with the parts a
    /// single pass goes on with.
    #[allow(clippy::type_complexity)]
    fn snapshot_encode<'sm, 'd>(
        &self,
        sm: &'sm SourceManager,
        engine: &'d DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
    ) -> Result<
        (
            crate::pch_snapshot::Roots,
            (
                kiln_parse::ParsedPrefix<'d, 'sm>,
                Option<crate::pp_cmd::SharedDeps>,
                Target,
                ParseOptions,
            ),
        ),
        String,
    > {
        let job = &self.0;
        let a = job.args;
        let Some(target) = self.target(engine) else {
            return Err("the target is not supported".to_owned());
        };
        let Some((pp, deps)) = job.build_preprocessor(sm, engine, interner, inputs, true) else {
            return Err("the preprocessor could not be built".to_owned());
        };
        let popts = self.parse_options();
        let limits = a.constexpr.limits();
        let started = std::time::Instant::now();
        let mut p = kiln_parse::parse_prefix(
            pp,
            Arc::clone(interner),
            engine,
            target.clone(),
            &popts,
            limits,
        );
        let parsed = started.elapsed();
        std::mem::replace(&mut p.ready, Ok(()))?;
        if engine.error_count() > 0 {
            return Err(format!("the prefix has {} error(s)", engine.error_count()));
        }
        p.pp.end_main_file()?;
        // The prefix's header lookups (WP-1403): a consumer with another
        // search list repeats them. A pass that did not record them gets
        // no snapshot.
        let Some(lookups) = p.pp.take_lookup_log() else {
            return Err("the prefix's header lookups were not recorded".to_owned());
        };
        let started = std::time::Instant::now();
        let mut w = kiln_snap::Writer::new();
        w.put(sm);
        w.put(interner);
        engine.snap_put(&mut w)?;
        p.pp.snap_put(&mut w)?;
        p.sema.snap_put(&mut w)?;
        let bytes = w.into_bytes();
        if crate::pch_snapshot::timing() {
            eprintln!(
                "snapshot-timing: producer prefix parse {:.2} ms, encode {:.2} ms, {} bytes",
                parsed.as_secs_f64() * 1000.0,
                started.elapsed().as_secs_f64() * 1000.0,
                bytes.len()
            );
        }
        Ok((crate::pch_snapshot::Roots { bytes, lookups }, (p, deps, target, popts)))
    }

    /// The target of the job's triple, or `None` once the error is
    /// reported.
    fn target(&self, engine: &DiagEngine<'_>) -> Option<Target> {
        let job = &self.0;
        let a = job.args;
        let Ok(mut target) = Target::from_triple(job.toolchain.target.triple) else {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
                .arg(job.toolchain.target.triple)
                .emit(engine);
            return None;
        };
        target.types.char_signed = a.predef.char_signed.unwrap_or(target.types.char_signed);
        // WP-630: `cx16` makes 128-bit atomics lock-free (clang
        // `X86TargetInfo::setMaxAtomicWidth`).
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        if feats.x86_cx16() {
            target.types.max_atomic_inline_bytes = 16;
        }
        Some(target)
    }

    /// The parser's options of the job.
    fn parse_options(&self) -> ParseOptions {
        let job = &self.0;
        let a = job.args;
        let x86 = !job.toolchain.target.is_aarch64();
        let feats = crate::features::resolve(a, &job.toolchain.target).0;
        let writes_kind2 = a.bmi_kind != Some(1) && job.module_output_path().is_some();
        ParseOptions {
            exceptions: a.predef.exceptions.unwrap_or(job.std.cplusplus),
            rtti: a.predef.rtti.unwrap_or(job.std.cplusplus),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            reflection: a.predef.reflection_for(&job.std),
            coro_aligned_allocation: a.predef.coro_aligned_allocation,
            lax_vector_conversions: a.lax_vector_conversions,
            clang_compat: a.predef.clang_compat.major(),
            gnu_lib_builtins: a.stdlib.unwrap_or_default() == crate::toolchain::StdLib::LibStdCxx,
            no_builtin: a.no_builtin(),
            no_builtin_fns: a.no_builtin_fns(),
            freestanding: a.predef.freestanding.unwrap_or(false),
            sized_deallocation: a.predef.sized_deallocation.unwrap_or(true),
            char8: a.predef.char8_t,
            math_errno: a.fp_model().opts.math_errno,
            sve: feats.sve,
            sve_vector_bits: feats.sve_vector_bits,
            aarch64_cpu_valid: crate::features::aarch64_attr_cpu_valid,
            cfg_dump: a.dump_cfg,
            cfg_stats: a.cfg_stats,
            cfg_max_elems: a.cfg_max_elems.unwrap_or(kiln_sema::cfg::DEFAULT_MAX_ELEMS),
            permissive: a.permissive,
            contract_semantic: a.contract_semantic.unwrap_or_default(),
            cf_protection_branch: a.codegen.cf_protection_branch(x86),
            record_carried: writes_kind2,
            record_image: a.bmi_image && writes_kind2,
            image_census: a.bmi_image_census,
            gnu89_inline: a.predef.gnu89_inline_for(&job.std),
            prefix: a.pch.producing || a.pch.record_lookups,
            ..job.std.parse_options()
        }
    }

    /// `-verify` (spec/driver.md "-verify", spec/diag.md §11): the
    /// diagnostics are buffered, the comments scanned as the
    /// preprocessor lexes them, and only the verifier's report printed.
    ///
    /// With `snap` (WP-1394) the `-include-pch` file's
    /// snapshot is restored instead of a replay of its header; `Err`
    /// (nothing reported) when it cannot be, or when the header reported
    /// diagnostics when it was built: those are not shown again, so a
    /// directive expecting one could not be satisfied, and the caller
    /// replays (spec/pch.md "Consumer"). Without `snap` it is
    /// always `Ok`.
    fn run_verify_inner(
        &self,
        opts: VerifyOptions,
        snap: Option<&crate::args::PchSnapshotUse>,
    ) -> Result<JobResult, String> {
        let job = &self.0;
        let sink = MemorySink::new();
        let (sm, interner, mut restore) = match snap {
            Some(s) => {
                let (sm, interner, restore) = crate::pch_snapshot::open(s)?;
                (sm, interner, Some(restore))
            }
            None => (SourceManager::new(), Arc::new(Interner::new()), None),
        };
        let mut engine = DiagEngine::new(&sm, job.diag_opts, sink.clone());
        if let Some(restore) = &mut restore {
            engine.snap_restore(&mut restore.reader).map_err(|e| e.to_string())?;
            restore.timer.mark("DiagEngine");
            let n = engine.prefix_reported();
            if n != 0 {
                return Err(format!(
                    "-verify: the header reported {n} diagnostic(s), which a restore does not show"
                ));
            }
        }
        let mut result = JobResult::default();
        let mut verifier = None;
        let mut failed = None;
        crate::crash::catching(job.path, job.stdin_text.as_deref(), &sm, || {
            if let Some(inputs) = job.pp_inputs(&sm, &engine) {
                let dirs: Vec<String> = inputs.list.dirs().iter().map(|d| d.path.clone()).collect();
                let v = Verifier::new(&sm, opts.clone(), inputs.main.file(), quoted_lookup(dirs));
                let v = Rc::new(RefCell::new(v));
                self.parse(
                    &sm,
                    &engine,
                    &interner,
                    inputs,
                    Some(&v),
                    &mut result,
                    restore.as_mut(),
                );
                failed = restore.take().and_then(|r| r.failed);
                verifier = Some(v);
            }
        });
        if let Some(why) = failed {
            return Err(why);
        }
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
        Ok(result)
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

    #[allow(clippy::too_many_lines, reason = "the job's phases, in order")]
    #[allow(clippy::too_many_arguments)]
    fn parse<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
        verify: Option<&Rc<RefCell<Verifier<'sm>>>>,
        result: &mut JobResult,
        restore: Option<&mut crate::pch_snapshot::Restore<'_>>,
    ) {
        let job = &self.0;
        let a = job.args;
        let Ok(dump_opts) = self.dump_options(sm, engine, inputs.main.file()) else {
            return;
        };
        let Some(target) = self.target(engine) else { return };
        if let Some(restore) = restore {
            // WP-1383: the prefix's state restored.
            let Some(parts) =
                job.restore_front(sm, engine, interner, inputs, restore, &target, || {
                    self.parse_options()
                })
            else {
                return;
            };
            let (sema, mut pp, deps, popts) = parts;
            if let Some(v) = verify {
                // Nothing is lexed yet: the main file's comments (and the
                // `-include`s' after the PCH) reach the verifier.
                let v = Rc::clone(v);
                pp.set_comment_handler(Box::new(move |c| v.borrow_mut().handle_comment(c, engine)));
            }
            let main_id = pp.main_file();
            let (tu, pp) = kiln_parse::continue_preprocessed(sema, pp, &target, &popts);
            restore.timer.mark("continue");
            restore.timer.report();
            crate::pch_snapshot::phase("restore and main file with end of unit");
            self.after_parse(sm, engine, tu, pp, deps, main_id, verify, dump_opts, &target, result);
            return;
        }
        let Some((mut pp, deps)) = job.build_preprocessor(sm, engine, interner, inputs, true)
        else {
            return;
        };
        if let Some(v) = verify {
            let v = Rc::clone(v);
            pp.set_comment_handler(Box::new(move |c| v.borrow_mut().handle_comment(c, engine)));
        }
        let popts = self.parse_options();
        let limits = a.constexpr.limits();
        let main_id = pp.main_file();
        let (tu, pp) =
            parse_preprocessed(pp, Arc::clone(interner), engine, target.clone(), &popts, limits);
        self.after_parse(sm, engine, tu, pp, deps, main_id, verify, dump_opts, &target, result);
    }

    /// The end of [`Self::parse`]: module and PCH output, reports and
    /// dumps of the parsed unit `tu`.
    #[allow(clippy::too_many_arguments)]
    fn after_parse<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        tu: kiln_parse::ParsedTu,
        mut pp: kiln_pp::Preprocessor<'sm, '_>,
        deps: Option<crate::pp_cmd::SharedDeps>,
        main_id: Option<FileId>,
        verify: Option<&Rc<RefCell<Verifier<'sm>>>>,
        dump_opts: Option<DumpOptions<'sm>>,
        target: &Target,
        result: &mut JobResult,
    ) {
        let job = &self.0;
        let a = job.args;
        job.note_image_imports(&pp);
        if engine.error_count() == 0 {
            job.write_module_file(&mut pp, engine, tu.carried.as_ref(), tu.image.as_ref());
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
        result.output.push_str(&tu.cfg_dump);
        let ast = tu.ast;
        #[cfg(test)]
        let ast = crate::snap_tests::observe(ast, sm, main_id);
        if a.dump_record_layouts {
            // clang prints each class as Sema completes it, before any
            // AST dump at the end of the translation unit.
            let layouts = kiln_abi::dump_all_record_layouts(&ast, target, Some(sm));
            result.output.push_str(&layouts);
        }
        if a.dump_vtable_layouts {
            result.output.push_str(&kiln_abi::vtable::dump_all_vtable_layouts(&ast, target));
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
