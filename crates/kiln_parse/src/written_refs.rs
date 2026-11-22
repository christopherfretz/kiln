//! The parser side of the written-reference table (spec/ast.md §18,
//! WP-1930): an id-expression or member access notes the span of the
//! name it consumed ([`Parser::note_wname`]) and records it once its
//! expression is built ([`Parser::with_written_name`]). Callers test
//! [`kiln_sema::Sema::records_written_refs`] first, so recording off
//! costs one branch per site.

use kiln_ast::written_refs::{RefQualifier, RefRole};
use kiln_source::{Loc, Span};

use crate::parser::{PExpr, Parser};

impl Parser<'_, '_> {
    /// The name being parsed began at `lo` and ended at the last token
    /// consumed, after qualifier `q`.
    #[inline]
    pub(crate) fn note_wname(&mut self, lo: Loc, q: RefQualifier) {
        if self.s.records_written_refs() {
            self.wname = Some((Span::new(lo, self.prev_end()), q));
        }
    }

    /// Runs `f` (an id-expression or member access parse) and records the
    /// name it noted with the expression it built; the site of an
    /// enclosing parse is kept across it.
    pub(crate) fn with_written_name(
        &mut self,
        role: RefRole,
        f: impl FnOnce(&mut Self) -> PExpr,
    ) -> PExpr {
        let outer = self.wname.take();
        let r = f(self);
        if let Some((range, q)) = self.wname.take() {
            self.s.note_written_name(r, range, role, q);
        }
        self.wname = outer;
        r
    }
}
