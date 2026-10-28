//! Shared test helpers: source fixtures and a capturing engine.
#![allow(dead_code)]

use kiln_diag::{DiagEngine, DiagOptions, JsonSink, SharedBuffer, TextSink};
use kiln_source::{ExpansionKind, FileId, Loc, SourceManager, Span};

/// A virtual buffer named `name` with contents `text`.
pub fn source(name: &str, text: &str) -> (SourceManager, FileId) {
    let sm = SourceManager::new();
    let f = sm.add_virtual(name, text.to_string()).unwrap();
    (sm, f)
}

/// Decoded byte offset of 1-based `line`/`col` in `file`.
pub fn offset(sm: &SourceManager, file: FileId, line: u32, col: u32) -> u32 {
    let text = sm.text(file);
    let lt = sm.line_text(file, line).expect("line exists");
    (lt.as_ptr().addr() - text.as_ptr().addr()) as u32 + col - 1
}

/// The file location of `line:col`.
pub fn at(sm: &SourceManager, file: FileId, line: u32, col: u32) -> Loc {
    sm.loc(file, offset(sm, file, line, col))
}

/// The span of `len` bytes at `line:col`.
pub fn sp(sm: &SourceManager, file: FileId, line: u32, col: u32, len: u32) -> Span {
    let lo = at(sm, file, line, col);
    Span::new(lo, lo.with_offset(len))
}

/// `base + a .. base + b`.
pub fn sub(base: Loc, a: u32, b: u32) -> Span {
    Span::new(base.with_offset(a), base.with_offset(b))
}

/// A macro-body expansion: `len` bytes spelled at `spelling`, replacing
/// `expansion`, invoked through the name token `name`.
pub fn mac(sm: &SourceManager, spelling: Loc, len: u32, expansion: Span, name: Span) -> Loc {
    sm.add_expansion(spelling, len, expansion, ExpansionKind::Macro { name }).unwrap()
}

/// A macro-argument expansion: `len` bytes spelled at `spelling`,
/// substituted for the parameter token `param`.
pub fn arg(sm: &SourceManager, spelling: Loc, len: u32, param: Span) -> Loc {
    sm.add_expansion(spelling, len, param, ExpansionKind::MacroArg).unwrap()
}

/// Runs `f` against an engine with a text sink and returns everything
/// written, followed by clang's summary line.
pub fn text(sm: &SourceManager, args: &[&str], f: impl FnOnce(&DiagEngine<'_>)) -> String {
    let opts = DiagOptions::from_args(args);
    let buf = SharedBuffer::new();
    let engine = DiagEngine::new(sm, &opts, TextSink::new(buf.clone(), &opts));
    f(&engine);
    let mut out = buf.contents();
    if let Some(s) = engine.summary() {
        out.push_str(&s);
        out.push('\n');
    }
    out
}

/// Runs `f` against an engine with a JSON sink and returns the lines.
pub fn json(sm: &SourceManager, args: &[&str], f: impl FnOnce(&DiagEngine<'_>)) -> Vec<String> {
    let opts = DiagOptions::from_args(args);
    let buf = SharedBuffer::new();
    let engine = DiagEngine::new(sm, &opts, JsonSink::new(buf.clone(), &opts));
    f(&engine);
    buf.contents().lines().map(str::to_string).collect()
}

/// Asserts equality and prints both sides readably on failure.
#[track_caller]
pub fn assert_text(got: &str, want: &str) {
    assert!(
        got == want,
        "rendered text differs\n--- got ---\n{got}\n--- want ---\n{want}\n--- got (debug) ---\n{got:?}\n--- want (debug) ---\n{want:?}"
    );
}
