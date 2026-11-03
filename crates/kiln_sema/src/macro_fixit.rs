//! Fix-its at macro locations (clang `Lexer::getLocForEndOfToken`,
//! `Lexer::makeFileCharRange`, `edit::Commit`; spec/sema.md §7.10.1).
//!
//! clang renders a fix-it only when it can be rewritten as an edit of the
//! file: a location inside a macro expansion maps to the file only when it
//! begins (or, for an end, finishes) every expansion it is in, or when a
//! whole range lies in one macro argument. A diagnostic whose fix-its do not
//! all map keeps its message and loses all of them (`mergeFixits`).
//!
//! Kiln spans end exclusively, so "the last token ends its expansion" is
//! "the end is the last location of the entry", and the file location is
//! the (exclusive) end of the invocation.

use kiln_diag::FixIt;
use kiln_source::{ExpansionInfo, ExpansionKind, Loc, Span};

use crate::Sema;

/// Bound on expansion-chain walks (each step moves to an older entry, so
/// the chain is finite; the bound only guards a malformed table).
const MAX_DEPTH: u32 = 4096;

impl Sema<'_, '_> {
    /// clang `Lexer::getLocForEndOfToken`: the end of the token at `at`, or
    /// `None` when `at` is a macro location whose token does not end its
    /// expansion (then the end of the outermost invocation).
    pub(crate) fn file_token_end(&self, at: Loc) -> Option<Loc> {
        self.macro_end_file_loc(self.token_end(at)?)
    }

    /// The fix-its `fixits` as file edits, or none at all when one of them
    /// cannot be mapped to the file (clang `mergeFixits` drops a commit that
    /// is not commitable).
    pub(crate) fn file_fixits(&self, fixits: Vec<FixIt>) -> Vec<FixIt> {
        let mut out = Vec::with_capacity(fixits.len());
        for f in fixits {
            let span = if f.span.lo == f.span.hi {
                self.insert_file_loc(f.span.lo).map(Span::empty)
            } else {
                self.file_char_range(f.span.lo, f.span.hi)
            };
            match span {
                Some(span) => out.push(FixIt { span, ..f }),
                None => return Vec::new(),
            }
        }
        out
    }

    /// The expansion entry of a macro location.
    fn entry(&self, loc: Loc) -> Option<ExpansionInfo> {
        self.diags.sm().expansion_info(loc)
    }

    /// clang `SourceManager::isAtStartOfImmediateMacroExpansion`.
    fn at_immediate_start(&self, loc: Loc) -> Option<Loc> {
        let e = self.entry(loc)?;
        if loc != e.base {
            return None;
        }
        if e.kind == ExpansionKind::MacroArg && e.base.to_raw() > 0 {
            // The argument continues from the previous entry.
            let prev = Loc::from_raw(e.base.to_raw() - 1);
            if self.entry(prev).is_some_and(|p| p.expansion.lo == e.expansion.lo) {
                return None;
            }
        }
        Some(e.expansion.lo)
    }

    /// clang `SourceManager::isAtEndOfImmediateMacroExpansion`, on an
    /// exclusive token end.
    fn at_immediate_end(&self, end: Loc) -> Option<Loc> {
        let e = self.entry(end)?;
        let last = e.base.with_offset(e.len);
        if end != last {
            return None;
        }
        if e.kind == ExpansionKind::MacroArg {
            // The argument continues in the next entry.
            let next = last.with_offset(1);
            if self.entry(next).is_some_and(|n| n.expansion.lo == e.expansion.lo) {
                return None;
            }
        }
        Some(e.expansion.hi)
    }

    /// clang `Lexer::isAtStartOfMacroExpansion`: the file location of the
    /// outermost invocation `loc` begins; identity for a file location.
    fn macro_start_file_loc(&self, mut loc: Loc) -> Option<Loc> {
        for _ in 0..MAX_DEPTH {
            if !loc.is_macro() {
                return Some(loc);
            }
            loc = self.at_immediate_start(loc)?;
        }
        None
    }

    /// clang `Lexer::isAtEndOfMacroExpansion`: the file end of the
    /// outermost invocation whose last token ends at `end`; identity for a
    /// file location.
    fn macro_end_file_loc(&self, mut end: Loc) -> Option<Loc> {
        for _ in 0..MAX_DEPTH {
            if !end.is_macro() {
                return Some(end);
            }
            end = self.at_immediate_end(end)?;
        }
        None
    }

    /// clang `edit::Commit::canInsert`: a start of an expansion maps to the
    /// invocation, a macro-argument location to where it is written.
    fn insert_file_loc(&self, at: Loc) -> Option<Loc> {
        let sm = self.diags.sm();
        let mut loc = self.macro_start_file_loc(at).unwrap_or(at);
        for _ in 0..MAX_DEPTH {
            match self.entry(loc) {
                Some(e) if e.kind == ExpansionKind::MacroArg => {
                    loc = sm.immediate_spelling_loc(loc);
                }
                _ => break,
            }
        }
        self.macro_start_file_loc(loc)
    }

    /// clang `Lexer::makeFileCharRange` on `[lo, hi)`.
    fn file_char_range(&self, mut lo: Loc, mut hi: Loc) -> Option<Span> {
        let sm = self.diags.sm();
        for _ in 0..MAX_DEPTH {
            match (lo.is_macro(), hi.is_macro()) {
                (false, false) => return Some(Span::new(lo, hi)),
                (true, false) => return Some(Span::new(self.macro_start_file_loc(lo)?, hi)),
                (false, true) => return Some(Span::new(lo, self.macro_end_file_loc(hi)?)),
                (true, true) => {}
            }
            if let (Some(a), Some(b)) = (self.macro_start_file_loc(lo), self.macro_end_file_loc(hi))
            {
                return Some(Span::new(a, b));
            }
            // A range inside one macro argument is its spelling.
            let (b, e) = (self.entry(lo)?, self.entry(hi)?);
            if b.kind != ExpansionKind::MacroArg
                || e.kind != ExpansionKind::MacroArg
                || b.expansion.lo != e.expansion.lo
            {
                return None;
            }
            (lo, hi) = (sm.immediate_spelling_loc(lo), sm.immediate_spelling_loc(hi));
        }
        None
    }
}
