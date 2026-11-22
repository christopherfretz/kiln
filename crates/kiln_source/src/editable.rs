//! Editable file ranges (spec/source.md "Editable file ranges"): which
//! characters of a file a source range written through macro expansions
//! stands for, when editing them edits exactly that range (clang
//! `Lexer::makeFileCharRange`, `Lexer::isAtStartOfMacroExpansion`,
//! `Lexer::isAtEndOfMacroExpansion`, `edit::Commit::canInsert`).
//!
//! Spans end exclusively, so "the last token ends its expansion" is "the
//! end is the last location of the entry", and the file location it maps
//! to is the (exclusive) end of the invocation. Every query is read-only
//! and safe on any location: unknown and invalid locations give `None`.

use crate::loc::{FileId, Loc, Span};
use crate::manager::{ExpansionInfo, ExpansionKind, SourceManager};

/// Bound on expansion-chain walks. Every step moves to an older entry
/// (`add_expansion` only links to locations already allocated), so a
/// chain is finite; the bound only guards a malformed table.
const MAX_DEPTH: u32 = 4096;

impl SourceManager {
    /// clang `SourceManager::isAtStartOfImmediateMacroExpansion`: the start
    /// of the invocation (or parameter) `loc`'s entry replaced, when `loc`
    /// is the first location of the entry and, for an argument, the
    /// argument does not continue from the previous entry.
    fn at_immediate_start(&self, loc: Loc) -> Option<Loc> {
        let e = self.expansion_info(loc)?;
        if loc != e.base {
            return None;
        }
        if e.kind == ExpansionKind::MacroArg && e.base.to_raw() > 0 {
            // The argument continues from the previous entry.
            let prev = Loc::from_raw(e.base.to_raw() - 1);
            if self.expansion_info(prev).is_some_and(|p| p.expansion.lo == e.expansion.lo) {
                return None;
            }
        }
        Some(e.expansion.lo)
    }

    /// clang `SourceManager::isAtEndOfImmediateMacroExpansion`, on an
    /// exclusive token end.
    fn at_immediate_end(&self, end: Loc) -> Option<Loc> {
        let e = self.expansion_info(end)?;
        let last = e.base.with_offset(e.len);
        if end != last {
            return None;
        }
        if e.kind == ExpansionKind::MacroArg {
            // The argument continues in the next entry (the last entry
            // of a full location space has none).
            let next = last.to_raw().checked_add(1).map(Loc::from_raw);
            if next
                .and_then(|n| self.expansion_info(n))
                .is_some_and(|n| n.expansion.lo == e.expansion.lo)
            {
                return None;
            }
        }
        Some(e.expansion.hi)
    }

    /// clang `Lexer::isAtStartOfMacroExpansion`: the file location of the
    /// outermost invocation that `loc` begins, when `loc` is the first
    /// location of every expansion it is in (an argument begins its
    /// parameter's expansion unless it continues from the previous entry).
    /// Identity for a file location; `None` when some expansion starts
    /// before `loc`, and for unknown macro locations.
    pub fn file_loc_at_macro_start(&self, mut loc: Loc) -> Option<Loc> {
        for _ in 0..MAX_DEPTH {
            if !loc.is_macro() {
                return Some(loc);
            }
            loc = self.at_immediate_start(loc)?;
        }
        None
    }

    /// clang `Lexer::isAtEndOfMacroExpansion` on an exclusive end: the file
    /// end of the outermost invocation whose last token ends at `end`,
    /// when `end` is the last location of every expansion it is in.
    /// Identity for a file location; `None` when some expansion continues
    /// after `end`, and for unknown macro locations.
    pub fn file_loc_at_macro_end(&self, mut end: Loc) -> Option<Loc> {
        for _ in 0..MAX_DEPTH {
            if !end.is_macro() {
                return Some(end);
            }
            end = self.at_immediate_end(end)?;
        }
        None
    }

    /// clang `edit::Commit::canInsert`: where an insertion at `at` goes in
    /// the file. A start of an expansion maps to the invocation, a
    /// macro-argument location to where the argument is written.
    fn insert_file_loc(&self, at: Loc) -> Option<Loc> {
        let mut loc = self.file_loc_at_macro_start(at).unwrap_or(at);
        for _ in 0..MAX_DEPTH {
            match self.expansion_info(loc) {
                Some(ExpansionInfo { kind: ExpansionKind::MacroArg, .. }) => {
                    loc = self.immediate_spelling_loc(loc);
                }
                _ => break,
            }
        }
        self.file_loc_at_macro_start(loc)
    }

    /// clang `Lexer::makeFileCharRange` on the character range `[lo, hi)`,
    /// before the final same-file check.
    fn file_char_range(&self, mut lo: Loc, mut hi: Loc) -> Option<Span> {
        for _ in 0..MAX_DEPTH {
            match (lo.is_macro(), hi.is_macro()) {
                (false, false) => return Some(Span::new(lo, hi)),
                (true, false) => return Some(Span::new(self.file_loc_at_macro_start(lo)?, hi)),
                (false, true) => return Some(Span::new(lo, self.file_loc_at_macro_end(hi)?)),
                (true, true) => {}
            }
            if let (Some(a), Some(b)) =
                (self.file_loc_at_macro_start(lo), self.file_loc_at_macro_end(hi))
            {
                return Some(Span::new(a, b));
            }
            // A range inside one macro argument is its spelling.
            let (b, e) = (self.expansion_info(lo)?, self.expansion_info(hi)?);
            if b.kind != ExpansionKind::MacroArg
                || e.kind != ExpansionKind::MacroArg
                || b.expansion.lo != e.expansion.lo
            {
                return None;
            }
            (lo, hi) = (self.immediate_spelling_loc(lo), self.immediate_spelling_loc(hi));
        }
        None
    }

    /// The file characters that editing `range` edits, as a span of file
    /// locations in one buffer range (an inclusion range for an included
    /// file), or `None` when the range is not editable as one file range
    /// (spec/source.md "Editable file ranges").
    ///
    /// A non-empty range maps by clang's `makeFileCharRange`: a macro begin
    /// must be the first location of every expansion it is in and maps to
    /// the start of the outermost invocation, a macro end must be the last
    /// location of every expansion it is in and maps to the end of the
    /// outermost invocation, and failing that a range whose two ends lie
    /// in one use of one macro argument maps to where the argument is
    /// spelled (and is mapped again). An empty range is an insertion point
    /// (clang `edit::Commit::canInsert`). The result must then lie in one
    /// buffer range, in order.
    pub fn editable_range(&self, range: Span) -> Option<Span> {
        let span = if range.lo == range.hi {
            Span::empty(self.insert_file_loc(range.lo)?)
        } else {
            self.file_char_range(range.lo, range.hi)?
        };
        // clang `makeRangeFromFileLocs`: one file, begin before end.
        self.same_entry_distance(span.lo, span.hi)?;
        (span.lo.is_file() && self.offset_of(span.lo).is_some()).then_some(span)
    }

    /// [`Self::editable_range`] as `(file, begin, end)` byte offsets in
    /// the buffer's text (`begin <= end`; equal for an insertion point).
    pub fn editable_file_range(&self, range: Span) -> Option<(FileId, u32, u32)> {
        let span = self.editable_range(range)?;
        let (file, lo) = self.offset_of(span.lo)?;
        let (_, hi) = self.offset_of(span.hi)?;
        Some((file, lo, hi))
    }
}
