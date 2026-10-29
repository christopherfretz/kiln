//! Shared helpers for the parser's per-rule tests: parse a source string
//! through [`kiln_parse::parse_file`] and return the diagnostics and the AST
//! dump as strings.

#![allow(dead_code, reason = "each test binary uses a subset")]

use std::sync::Arc;

use kiln_ast::dump::{DumpOptions, dump};
use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, Level, MemorySink};
use kiln_parse::{ParseOptions, parse_file};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The result of parsing one source string.
pub struct Parsed {
    /// Each diagnostic (and each attached note) as `L:C level: message`.
    pub diags: Vec<String>,
    /// The AST dump, without ids or locations.
    pub dump: String,
}

impl Parsed {
    /// The error and warning lines only (no notes).
    pub fn problems(&self) -> Vec<&str> {
        self.diags.iter().filter(|d| !d.contains(" note: ")).map(String::as_str).collect()
    }

    /// Asserts that no diagnostic was produced.
    #[track_caller]
    pub fn assert_clean(&self) {
        assert!(self.diags.is_empty(), "unexpected diagnostics:\n{}", self.diags.join("\n"));
    }

    /// Asserts that some diagnostic line contains `needle`.
    #[track_caller]
    pub fn assert_diag(&self, needle: &str) {
        assert!(
            self.diags.iter().any(|d| d.contains(needle)),
            "no diagnostic containing {needle:?}; got:\n{}",
            self.diags.join("\n")
        );
    }

    /// Asserts that the dump contains `needle`.
    #[track_caller]
    pub fn assert_dump(&self, needle: &str) {
        assert!(self.dump.contains(needle), "dump lacks {needle:?}:\n{}", self.dump);
    }
}

fn level(l: Level) -> &'static str {
    match l {
        Level::Note => "note",
        Level::Remark => "remark",
        Level::Warning => "warning",
        Level::Error => "error",
        Level::Fatal => "fatal",
    }
}

fn render(sm: &SourceManager, d: &Diagnostic, out: &mut Vec<String>) {
    let pos = sm
        .line_col(d.span.lo)
        .map_or_else(|| "-".to_owned(), |lc| format!("{}:{}", lc.line, lc.col));
    out.push(format!("{pos} {}: {}", level(d.level), d.message));
    for n in &d.notes {
        render(sm, n, out);
    }
}

/// Parses `src` as `t.cpp` for x86-64.
pub fn parse(src: &str) -> Parsed {
    parse_with(src, ParseOptions::default())
}

/// Parses `src` as `t.cpp` for x86-64 with `opts`.
pub fn parse_with(src: &str, opts: ParseOptions) -> Parsed {
    parse_dump(src, opts, false)
}

/// As [`parse`], with implicit declarations (implicit special members) in
/// the dump.
pub fn parse_implicit(src: &str) -> Parsed {
    parse_dump(src, ParseOptions::default(), true)
}

fn parse_dump(src: &str, opts: ParseOptions, implicit: bool) -> Parsed {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::X86_64);
    let ast = parse_file(&sm, &interner, file, &engine, target, &opts);
    let opts = DumpOptions {
        sources: Some(&sm),
        main_file: Some(file),
        implicit,
        ..DumpOptions::default()
    };
    let mut buf = Vec::new();
    dump(&ast, &opts, &mut buf).expect("dump to memory");
    let mut diags = Vec::new();
    for d in sink.take() {
        render(&sm, &d, &mut diags);
    }
    Parsed { diags, dump: String::from_utf8(buf).expect("utf-8 dump") }
}
