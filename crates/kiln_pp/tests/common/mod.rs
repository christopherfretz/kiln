//! Shared helpers for the `kiln_pp` integration tests.
//!
//! Goldens come from `clang++-19 -std=c++26 -E -P` (plus `-I` / `-isystem`
//! / `--embed-dir` for the in-memory headers), with the aarch64 Linux
//! predefines that clang itself uses.
#![allow(dead_code, reason = "each test binary uses a subset")]

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std};
use kiln_pp::{MemoryResolver, PpOptions, Preprocessor, PrintOptions, print_preprocessed};
use kiln_source::SourceManager;

/// clang 19's predefined macros for `-std=c++26 --target=aarch64-unknown-linux-gnu`.
pub const PREDEFINES: &str =
    include_str!("../../../../resource/predefines/aarch64-unknown-linux-gnu.c++26.txt");

/// The result of one preprocessor run.
#[derive(Debug)]
pub struct Out {
    /// The printed output.
    pub text: String,
    /// `line:col: level: message` for each diagnostic (notes excluded),
    /// located at the expansion location of its primary span.
    pub diags: Vec<String>,
}

/// One run: `src` as the main file `t.cpp`, headers from `r`, options
/// adjusted by `cfg`.
pub fn run_with(
    src: &str,
    r: MemoryResolver,
    print: PrintOptions,
    cfg: impl FnOnce(&mut PpOptions),
) -> Out {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut opts = PpOptions::new(Std::Cxx26);
    PREDEFINES.clone_into(&mut opts.predefines);
    opts.reproducible_time = Some(("Jan  1 2000".to_owned(), "00:00:00".to_owned()));
    cfg(&mut opts);
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
    pp.set_include_resolver(Box::new(r));
    pp.enter_main_file(main);
    let mut text = String::new();
    print_preprocessed(&mut pp, print, &mut text);
    drop(pp);
    let diags = sink
        .take()
        .iter()
        .map(|d| {
            let at = sm
                .presumed_line_col(d.span.lo)
                .map_or_else(String::new, |p| format!("{}:{}: ", p.line, p.col));
            format!("{at}{}: {}", d.level.text_name(), d.message)
        })
        .collect();
    Out { text, diags }
}

/// `-E -P` of `src` with headers from `r`.
pub fn pp_files(src: &str, r: MemoryResolver) -> Out {
    run_with(src, r, PrintOptions::default(), |_| {})
}

/// `-E -P` of `src`.
pub fn pp(src: &str) -> Out {
    pp_files(src, MemoryResolver::new())
}

/// The `-E -P` text of `src`.
pub fn pp_dump(src: &str) -> String {
    pp(src).text
}

/// Asserts `-E -P` of `src` is `want` and there are no diagnostics.
#[track_caller]
pub fn check(src: &str, want: &str) {
    let o = pp(src);
    assert_eq!(o.text, want, "output of:\n{src}");
    assert!(o.diags.is_empty(), "unexpected diagnostics for:\n{src}\n{:#?}", o.diags);
}

/// Asserts `-E -P` of `src` is `want` and the diagnostics are `diags`.
#[track_caller]
pub fn check_diags(src: &str, want: &str, diags: &[&str]) {
    let o = pp(src);
    assert_eq!(o.text, want, "output of:\n{src}");
    assert_eq!(o.diags, diags, "diagnostics of:\n{src}");
}

/// Asserts the run `o` printed `want` with diagnostics `diags`.
#[track_caller]
pub fn expect(o: &Out, want: &str, diags: &[&str]) {
    assert_eq!(o.text, want, "output");
    assert_eq!(o.diags, diags, "diagnostics");
}
