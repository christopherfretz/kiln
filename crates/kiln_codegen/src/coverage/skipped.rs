//! The skipped ranges of source-based coverage (WP-1003, spec/codegen.md
//! "Instrumentation"): clang's `CoverageSourceInfo`, which collects the
//! groups skipped by conditional directives (`SourceRangeSkipped`), the
//! comments (`HandleComment`) and the runs of empty lines
//! (`HandleEmptyline`) while the preprocessor runs, each with the
//! location of the token watched before it (`PrevTokLoc`, which also
//! decides whether it merges into the previous range) and after it
//! (`NextTokLoc`).
//!
//! kiln does not watch the preprocessor's tokens; it rescans each file
//! entry instead and models what clang's token watcher sees: every
//! token outside skipped groups except the `#` of a directive and the
//! name of an expanded macro, the end of each directive line (`eod`),
//! the directive names (and their `eod`s) inside skipped groups, and one
//! *foreign* token (spelled elsewhere, so never on the same line) for a
//! macro's expansion and for an included file's tokens.

use kiln_lex::{LexOptions, Lexer, TokFlags, TokKind};
use kiln_source::{FileId, Interner, Loc, SourceManager, Span};

/// What produced a [`SkippedRange`] (clang `SkippedRange::Kind`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SkipKind {
    PpIfElse,
    EmptyLine,
    Comment,
}

/// A watched token: its location, or `None` for a foreign one.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Tok {
    id: u32,
    pub loc: Option<Loc>,
}

/// clang `SkippedRange`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SkippedRange {
    pub kind: SkipKind,
    pub lo: Loc,
    pub hi: Loc,
    pub prev: Option<Tok>,
    pub next: Option<Tok>,
}

struct Scan {
    out: Vec<SkippedRange>,
    prev: Option<Tok>,
    ids: u32,
}

impl Scan {
    fn watch(&mut self, loc: Option<Loc>, eod: bool) {
        self.ids += 1;
        let t = Tok { id: self.ids, loc };
        self.prev = Some(t);
        if !eod
            && let Some(b) = self.out.last_mut()
            && b.next.is_none()
        {
            b.next = Some(t);
        }
    }

    /// clang `CoverageSourceInfo::AddSkippedRange` (both ends are in the
    /// scanned entry).
    fn add(&mut self, kind: SkipKind, lo: Loc, hi: Loc) {
        if let Some(b) = self.out.last_mut()
            && b.prev.map(|t| t.id) == self.prev.map(|t| t.id)
        {
            b.hi = hi;
            return;
        }
        self.out.push(SkippedRange { kind, lo, hi, prev: self.prev, next: None });
    }
}

/// The skipped ranges of the file entry starting at `base` (one
/// inclusion of `file`), in source order. `pp` are the skipped groups and
/// `macros` the macro invocations (with whether the macro is
/// function-like), both of the whole translation unit, sorted.
pub(crate) fn scan_entry(
    sm: &SourceManager,
    file: FileId,
    base: Loc,
    pp: &[Span],
    macros: &[(Span, bool)],
) -> Vec<SkippedRange> {
    let text = sm.text(file);
    let len = text.len() as u32;
    let end = base.with_offset(len);
    let in_entry = |l: Loc| base <= l && l <= end;
    let pp: Vec<Span> = pp.iter().copied().filter(|s| in_entry(s.lo)).collect();
    let macros: Vec<(Span, bool)> = macros.iter().copied().filter(|m| in_entry(m.0.lo)).collect();
    let interner = Interner::new();
    let opts = LexOptions { keep_comments: true, ..LexOptions::default() };
    let mut lx = Lexer::new(sm, &interner, file, base, opts);
    let bytes = text.as_bytes();
    let mut sc = Scan { out: Vec::new(), prev: None, ids: 0 };
    // The previous token's end (for the whitespace before the next).
    let mut ws_from = 0u32;
    let (mut pi, mut mi) = (0usize, 0usize);
    // The skipped group being scanned, and whether a directive line is.
    let mut in_pp: Option<Span> = None;
    let mut in_dir = false;
    let mut dir_name_next = false;
    let mut dir_is_include = false;
    // The function-like invocation whose arguments are being read, and
    // the object-like one whose name is being replaced.
    let mut in_macro: Option<Loc> = None;
    let off = |l: Loc| sm.same_entry_distance(base, l).unwrap_or(0);
    loop {
        let t = lx.next();
        let lo = off(t.span.lo);
        // The whitespace before the token: a run with two or more line
        // terminators is an empty-line range from just after the first
        // to the last (clang `Lexer::SkipWhitespace`).
        if lo > ws_from {
            let ws = &bytes[ws_from as usize..lo as usize];
            let first = ws.iter().position(|&c| c == b'\n');
            let last = ws.iter().rposition(|&c| c == b'\n');
            if let (Some(f), Some(l)) = (first, last)
                && f != l
            {
                sc.add(
                    SkipKind::EmptyLine,
                    base.with_offset(ws_from + f as u32 + 1),
                    base.with_offset(ws_from + l as u32),
                );
            }
        }
        ws_from = off(t.span.hi).max(lo);
        if t.kind == TokKind::Eof {
            if let Some(g) = in_pp.take() {
                sc.add(SkipKind::PpIfElse, g.lo, g.hi);
            }
            // What follows the entry is spelled elsewhere.
            sc.watch(None, false);
            break;
        }
        // A skipped group ends after the `eod` of the directive that
        // ended it.
        if let Some(g) = in_pp
            && t.span.lo > g.hi
        {
            sc.add(SkipKind::PpIfElse, g.lo, g.hi);
            in_pp = None;
        }
        if in_pp.is_none() && pi < pp.len() && t.span.lo >= pp[pi].lo {
            in_pp = Some(pp[pi]);
            pi += 1;
        }
        let skipping = in_pp.is_some();
        match t.kind {
            TokKind::Eod => {
                lx.set_in_directive(false);
                in_dir = false;
                sc.watch(Some(t.span.lo), true);
                if dir_is_include && !skipping {
                    // The included file's tokens.
                    sc.watch(None, false);
                }
                dir_is_include = false;
                if let Some(g) = in_pp
                    && t.span.lo >= g.hi
                {
                    sc.add(SkipKind::PpIfElse, g.lo, g.hi);
                    in_pp = None;
                }
                continue;
            }
            TokKind::Comment => {
                if !skipping {
                    sc.add(SkipKind::Comment, t.span.lo, t.span.hi);
                }
                continue;
            }
            _ => {}
        }
        if !in_dir
            && t.flags.contains(TokFlags::START_OF_LINE)
            && matches!(t.kind, TokKind::Punct(kiln_lex::Punct::Hash))
        {
            lx.set_in_directive(true);
            in_dir = true;
            dir_name_next = true;
            continue;
        }
        if dir_name_next {
            dir_name_next = false;
            let name = sm.span_text(t.span).unwrap_or_default();
            dir_is_include = matches!(name, "include" | "include_next" | "import");
            sc.watch(Some(t.span.lo), false);
            continue;
        }
        if skipping {
            continue;
        }
        // Macro invocations.
        while mi < macros.len() && macros[mi].0.lo < t.span.lo {
            mi += 1;
        }
        if let Some(e) = in_macro {
            if t.span.lo >= e {
                in_macro = None;
                sc.watch(None, false);
            } else {
                sc.watch(Some(t.span.lo), false);
                if t.span.hi >= e {
                    // The `)`: then the expansion.
                    in_macro = None;
                    sc.watch(None, false);
                }
                continue;
            }
        }
        if mi < macros.len() && macros[mi].0.lo == t.span.lo {
            let (r, fn_like) = macros[mi];
            mi += 1;
            if fn_like && r.hi > t.span.hi {
                in_macro = Some(r.hi);
            } else {
                sc.watch(None, false);
            }
            continue;
        }
        sc.watch(Some(t.span.lo), false);
    }
    sc.out
}
