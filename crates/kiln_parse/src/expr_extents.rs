//! Expression extents for tooling clients (spec/ast.md §21, WP-2327):
//! [`kiln_ast::expr_extent::WrittenExtents`] mapped to bytes of one file by the
//! rule of `kiln_lex::token_range` (spec/lex.md "Token ranges"), and the
//! `-fkiln-dump-expr-extents` dump (§21.4).

use std::fmt::Write as _;

use kiln_ast::Ast;
use kiln_ast::expr_extent::WrittenExtents;
use kiln_ast::ids::ExprId;
use kiln_lex::LexOptions;
use kiln_source::symbol::Interner;
use kiln_source::{FileId, Loc, SourceManager};

/// The extents of a finished unit's expressions: the tree, the source
/// manager its locations are in, and the lexer options of the unit (to
/// measure the last token).
pub struct ExprExtents<'a> {
    ast: &'a Ast,
    tree: WrittenExtents<'a>,
    sm: &'a SourceManager,
    interner: &'a Interner,
    opts: LexOptions,
}

impl std::fmt::Debug for ExprExtents<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExprExtents").finish_non_exhaustive()
    }
}

impl<'a> ExprExtents<'a> {
    /// The extents of `ast`, whose locations are in `sm` and whose
    /// identifiers `interner` holds, lexed with `opts` (the
    /// preprocessor's lexer options).
    pub fn new(
        ast: &'a Ast,
        sm: &'a SourceManager,
        interner: &'a Interner,
        opts: LexOptions,
    ) -> Self {
        ExprExtents { ast, tree: WrittenExtents::new(ast), sm, interner, opts }
    }

    /// The first and the last token of `e` as written (spec/ast.md §21),
    /// or `None`: [`WrittenExtents::get`] when the two tokens are the
    /// bytes of one file by the rule of `kiln_lex::token_range`, so a
    /// `Some` is always [`Self::written_range`]'s `Some`.
    pub fn written_extent(&self, e: ExprId) -> Option<(Loc, Loc)> {
        let (first, last) = self.tree.get(e)?;
        self.range(first, last).map(|_| (first, last))
    }

    /// [`Self::written_extent`] as bytes of one file: `(file, begin, end)`,
    /// `end` exclusive.
    pub fn written_range(&self, e: ExprId) -> Option<(FileId, u32, u32)> {
        let (first, last) = self.tree.get(e)?;
        self.range(first, last)
    }

    fn range(&self, first: Loc, last: Loc) -> Option<(FileId, u32, u32)> {
        let r = kiln_lex::token_range(self.sm, self.interner, first, last, self.opts)?;
        // A paste or stringizing buffer is no file a client can edit.
        (!self.sm.is_written_in_scratch_space(self.sm.loc(r.0, r.1))).then_some(r)
    }

    /// The `-fkiln-dump-expr-extents` dump (spec/ast.md §21.4): one line
    /// per expression node of every written body and initializer
    /// ([`kiln_ast::dump::written_exprs`]), in the walk's order,
    /// `<kind> TAB <file> TAB <begin> TAB <end>` with byte offsets, `end`
    /// exclusive. A `None` extent has `-` for `end`, and for `begin` the
    /// offset of the node's own location when that is file text (else
    /// `-`, with `-` for `file`). A `Some` that is not one file range
    /// (which [`Self::written_extent`] never gives) would print
    /// `<kind> TAB ! TAB <what> TAB -`.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for e in kiln_ast::dump::written_exprs(self.ast) {
            let kind = self.ast.exprs.get(e).kind();
            let _ = write!(out, "{kind:?}\t");
            if self.written_extent(e).is_some() {
                match self.written_range(e) {
                    Some((f, b, x)) => {
                        let _ = writeln!(out, "{}\t{b}\t{x}", self.sm.file_name(f));
                    }
                    None => out.push_str("!\tunmapped\t-\n"),
                }
            } else {
                let loc = self.ast.exprs.get(e).loc();
                match self.sm.offset_of(loc).filter(|_| loc.is_file()) {
                    Some((f, b)) => {
                        let _ = writeln!(out, "{}\t{b}\t-", self.sm.file_name(f));
                    }
                    None => out.push_str("-\t-\t-\n"),
                }
            }
        }
        out
    }
}
