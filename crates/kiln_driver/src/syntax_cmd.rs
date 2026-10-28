//! `-fsyntax-only [-fdump-ast[=opts]] [-fdump-ast-filter=s]` for one input:
//! WP-048's parser and Sema on the file, then the optional AST dump
//! (`spec/driver.md` "Syntax checking", `spec/ast-dump.md`).
//!
//! The parser still lexes the file itself (no `#include`, no macros) until
//! WP-055 feeds it `kiln_pp` tokens.

use kiln_ast::dump::{DumpOptions, Filter, dump};
use kiln_diag::{Diag, DiagId, DiagOptions};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::Target;
use kiln_source::{FileId, SourceManager, Span};

use crate::args::Args;
use crate::pp_cmd::JobResult;
use crate::toolchain::{LangStd, Toolchain};
use crate::{SharedWriter, make_engine};

/// One input to parse.
#[derive(Debug)]
pub struct SyntaxJob<'j> {
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
        let sm = SourceManager::new();
        let engine = make_engine(&sm, self.diag_opts, self.stderr.clone(), None);
        let mut result = JobResult::default();
        let loaded = match &self.stdin_text {
            Some(t) => sm.add_virtual("<stdin>", t.clone()).map_err(|e| e.to_string()),
            None => sm.load_file(std::path::Path::new(self.path)).map_err(|e| e.to_string()),
        };
        match loaded {
            Err(e) => {
                Diag::new(DiagId::drv_cannot_read_input, Span::INVALID)
                    .arg(self.path)
                    .arg(e)
                    .emit(&engine);
            }
            Ok(file) => self.parse(&sm, &engine, file, &mut result),
        }
        engine.flush();
        if let Some(s) = engine.summary()
            && self.diag_opts.format == kiln_diag::Format::Text
        {
            let mut err = self.stderr.clone();
            let _ = writeln!(err, "{s}");
        }
        result.errors = engine.error_count();
        result
    }

    fn parse<'sm>(
        &self,
        sm: &'sm SourceManager,
        engine: &kiln_diag::DiagEngine<'sm>,
        file: FileId,
        result: &mut JobResult,
    ) {
        let a = self.args;
        let dump_opts = match &a.dump_ast {
            Some(o) => match dump_options(o, a.dump_ast_filter.clone(), sm, file) {
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
        let Ok(target) = Target::from_triple(self.toolchain.target.triple) else {
            Diag::new(DiagId::drv_unsupported_target, Span::INVALID)
                .arg(self.toolchain.target.triple)
                .emit(engine);
            return;
        };
        let popts = ParseOptions { std: self.std.lex };
        let ast = parse_file(sm, file, engine, target, &popts);
        if let Some(d) = dump_opts {
            let mut out: Vec<u8> = Vec::new();
            if dump(&ast, &d, &mut out).is_ok() {
                result.output.push_str(&String::from_utf8_lossy(&out));
            }
        }
    }
}
