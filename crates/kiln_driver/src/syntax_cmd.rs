//! `-fsyntax-only [-fdump-ast[=opts]] [-fdump-ast-filter=s]
//! [-fdump-record-layouts[-complete]] [-fdump-vtable-layouts]` for one
//! input: the preprocessor exactly as `-E` builds it
//! ([`Job::build_preprocessor`]), the parser and Sema on its token stream, then the optional record-layout, vtable and
//! AST dumps (`spec/driver.md` "Syntax checking", `spec/ast-dump.md`,
//! `spec/abi.md` §8).

use std::io::Write as _;
use std::sync::Arc;

use kiln_ast::dump::{DumpOptions, Filter, dump};
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_lex::Interner;
use kiln_parse::{ParseOptions, parse_preprocessed};
use kiln_sema::target::Target;
use kiln_source::{FileId, SourceManager, Span};

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
        let sm = SourceManager::new();
        // The preprocessor and the AST share the interner; it outlives
        // the engine.
        let interner = Arc::new(Interner::new());
        let engine = make_engine(&sm, job.diag_opts, job.stderr.clone(), None);
        let mut result = JobResult::default();
        if let Some(inputs) = job.pp_inputs(&sm, &engine) {
            self.parse(&sm, &engine, &interner, inputs, &mut result);
        }
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

    fn parse<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &DiagEngine<'sm>,
        interner: &'sm Arc<Interner>,
        inputs: PpInputs,
        result: &mut JobResult,
    ) {
        let job = &self.0;
        let a = job.args;
        let dump_opts = match &a.dump_ast {
            Some(o) => match dump_options(o, a.dump_ast_filter.clone(), sm, inputs.main) {
                Ok(d) => Some(d),
                Err(bad) => {
                    Diag::new(DiagId::drv_invalid_value, Span::INVALID)
                        .arg(format!("-fdump-ast={o}"))
                        .arg(bad)
                        .emit(engine);
                    return;
                }
            },
            None => None,
        };
        let Ok(target) = Target::from_triple(job.toolchain.target.triple) else {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
                .arg(job.toolchain.target.triple)
                .emit(engine);
            return;
        };
        // `-MD` with `-fsyntax-only` is rejected before this (no deps).
        let Some((pp, _deps)) = job.build_preprocessor(sm, engine, interner, inputs) else {
            return;
        };
        let popts = ParseOptions {
            std: job.std.lex,
            exceptions: a.predef.exceptions.unwrap_or(true),
            rtti: a.predef.rtti.unwrap_or(true),
            inst_limits: a.templ.limits(),
            inst_report: a.templ.report,
            ..ParseOptions::default()
        };
        let limits = a.constexpr.limits();
        let (tu, _pp) =
            parse_preprocessed(pp, Arc::clone(interner), engine, target.clone(), &popts, limits);
        a.constexpr.write_report(&tu.oven, sm, engine);
        if !tu.inst_report.is_empty() {
            engine.flush();
            let mut err = job.stderr.clone();
            let _ = err.write_all(tu.inst_report.as_bytes());
        }
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
