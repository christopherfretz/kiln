//! Reflection syntax, `-freflection` only (WP-267 spike; P2996R13
//! [expr.reflect], [basic.splice]; spec/parse.md "Reflection tokens"):
//! the reflect operator `^^` and the type splice `[: r :]`. Without the
//! flag the lexer never forms `^^`, `[:` or `:]`, so none of this runs.

use kiln_lex::{Punct, TokKind};

use crate::parser::{PExpr, Parser};
use crate::tentative::TypeIdContext;
use kiln_ast::QualType;

impl Parser<'_, '_> {
    /// `^^ type-id` / `^^ qualified-namespace-specifier` /
    /// `^^ id-expression` ([expr.reflect]/1), at `^^`. A type-id wins
    /// when the operand can be one ([expr.reflect]/2: `^^T` reflects the
    /// type).
    pub(crate) fn parse_reflect_expr(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        // An operand that can be a type-id is one (as a trailing return
        // type: an ambiguous declarator such as `void (*)(int)` tips to it).
        if self.is_cxx_type_id_at(self.pos(), TypeIdContext::InTrailingReturnType).0 {
            let ty = self.parse_type_id()?;
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_type(ty, loc, end));
        }
        let qual = self.parse_nested_name_nns().ok()?;
        let Some(name) = self.ident() else {
            self.error_here(kiln_diag::DiagId::parse_expected_unqualified_id);
            return None;
        };
        let name_loc = self.bump().span.lo;
        if self.at(Punct::Less) {
            self.unsupported("reflection of a template-id", name_loc);
            return None;
        }
        let end = self.prev_end();
        Some(self.s.act_on_reflect_name(qual, name, name_loc, loc, end))
    }

    /// True at `[:` (only ever lexed under `-freflection`).
    pub(crate) fn at_splice(&self) -> bool {
        self.tok().kind == TokKind::Punct(Punct::SpliceOpen)
    }

    /// The token index just past the `:]` closing the `[:` at token
    /// index `i`, when `i` is a `[:` (lookahead; nested splices balance).
    pub(crate) fn splice_end(&self, i: usize) -> Option<usize> {
        if Self::punct_of(self.tok_at(i)) != Some(Punct::SpliceOpen) {
            return None;
        }
        let mut depth = 0usize;
        let mut j = i;
        loop {
            let t = self.tok_at(j);
            match Self::punct_of(t) {
                Some(Punct::SpliceOpen) => depth += 1,
                Some(Punct::SpliceClose) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j + 1);
                    }
                }
                _ if t.kind == TokKind::Eof => return None,
                _ => {}
            }
            j += 1;
        }
    }

    /// `[: constant-expression :]` in a type position
    /// ([basic.splice], [dcl.type.splice]): the type the operand
    /// reflects, at `[:`.
    pub(crate) fn parse_splice_type(&mut self) -> Option<QualType> {
        let open = self.bump().span.lo;
        let e = self.with_greater_op(true, Self::parse_constant_expr);
        let close = self.expect_closing(Punct::SpliceClose, Punct::SpliceOpen, open);
        let e = e?;
        self.s.act_on_splice_type(e, open, close)
    }
}
