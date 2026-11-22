//! Fix-its at macro locations (clang `Lexer::getLocForEndOfToken`,
//! `Lexer::makeFileCharRange`, `edit::Commit`; spec/sema.md §7.10.1).
//!
//! clang renders a fix-it only when it can be rewritten as an edit of the
//! file: a location inside a macro expansion maps to the file only when it
//! begins (or, for an end, finishes) every expansion it is in, or when a
//! whole range lies in one macro argument. A diagnostic whose fix-its do not
//! all map keeps its message and loses all of them (`mergeFixits`).
//!
//! The mapping is the source manager's editable-range query
//! (`SourceManager::editable_range`, spec/source.md "Editable file
//! ranges"), which tooling clients use too, so the two never drift.

use kiln_diag::FixIt;
use kiln_source::Loc;

use crate::Sema;

impl Sema<'_, '_> {
    /// clang `Lexer::getLocForEndOfToken`: the end of the token at `at`, or
    /// `None` when `at` is a macro location whose token does not end its
    /// expansion (then the end of the outermost invocation).
    pub(crate) fn file_token_end(&self, at: Loc) -> Option<Loc> {
        self.diags.sm().file_loc_at_macro_end(self.token_end(at)?)
    }

    /// The fix-its `fixits` as file edits, or none at all when one of them
    /// cannot be mapped to the file (clang `mergeFixits` drops a commit that
    /// is not commitable).
    pub(crate) fn file_fixits(&self, fixits: Vec<FixIt>) -> Vec<FixIt> {
        let sm = self.diags.sm();
        let mut out = Vec::with_capacity(fixits.len());
        for f in fixits {
            match sm.editable_range(f.span) {
                Some(span) => out.push(FixIt { span, ..f }),
                None => return Vec::new(),
            }
        }
        out
    }
}
