//! Reflection syntax, `-freflection` only (WP-267 spike; P2996R13
//! [expr.reflect], [basic.splice]; spec/parse.md "Reflection tokens"):
//! the reflect operator `^^` and the type splice `[: r :]`. Without the
//! flag the lexer never forms `^^`, `[:` or `:]`, so none of this runs.

use kiln_lex::{Punct, TokKind};

use crate::parser::{PExpr, Parser, is_template_class};
use crate::tentative::TypeIdContext;
use kiln_ast::QualType;

impl Parser<'_, '_> {
    /// `^^ ::` / `^^ type-id` / `^^ reflection-name` / `^^ id-expression`
    /// ([expr.reflect]/1), at `^^`. A type-id wins when the operand can be
    /// one ([expr.reflect]/6); a template-name not followed by `<` names
    /// the template (/5); a template-id that is not a type names the
    /// variable or function specialization (/7).
    pub(crate) fn parse_reflect_expr(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        // `^^::` ([expr.reflect]/4): nothing that continues a qualified
        // name follows the `::`.
        if self.at(Punct::ColonColon)
            && !matches!(self.tok_at(self.pos() + 1).kind, TokKind::Identifier(_))
            && !matches!(Self::punct_of(self.tok_at(self.pos() + 1)), Some(Punct::Tilde))
        {
            self.bump();
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_global(loc, end));
        }
        // An operand that can be a type-id is one (as a trailing return
        // type: an ambiguous declarator such as `void (*)(int)` tips to it),
        // unless it is a bare template-name.
        if !self.at_bare_template_name()
            && self.is_cxx_type_id_at(self.pos(), TypeIdContext::InTrailingReturnType).0
        {
            let ty = self.parse_type_id()?;
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_type(ty, loc, end));
        }
        let operand = self.tok().span.lo;
        let qual = self.parse_nested_name_nns().ok()?;
        let Some(name) = self.ident() else {
            return Some(self.s.reflect_bad_operand(operand, loc));
        };
        let name_loc = self.bump().span.lo;
        if let Some(q) = qual
            && self.s.is_dependent_ctx(q.ctx)
        {
            self.unsupported("reflection of a dependent qualified name", name_loc);
            return None;
        }
        if self.at(Punct::Less) {
            let class = self.s.classify_name(name, qual.map(|q| q.ctx));
            if is_template_class(class) {
                let r = self.parse_template_id_rest(class, name, name_loc).ok()?;
                let end = self.prev_end();
                return Some(self.s.act_on_reflect_template_id(r, name_loc, loc, end));
            }
        }
        let end = self.prev_end();
        Some(self.s.act_on_reflect_name(qual, name, name_loc, loc, end))
    }

    /// True at `name` / `nns name` naming a template, not followed by `<`
    /// (the reflection of the template itself, never a type-id).
    fn at_bare_template_name(&self) -> bool {
        let mut i = self.pos();
        let mut ctx = None;
        if Self::punct_of(self.tok_at(i)) == Some(Punct::ColonColon) {
            ctx = Some(self.s.ast.tu);
            i += 1;
        }
        loop {
            let TokKind::Identifier(name) = self.tok_at(i).kind else { return false };
            let next = Self::punct_of(self.tok_at(i + 1));
            if next == Some(Punct::ColonColon) {
                // Only plain namespace / class qualifiers are followed.
                match self.s.lookup_nested_ctx(name, ctx) {
                    Some(c) => ctx = Some(c),
                    None => return false,
                }
                i += 2;
                continue;
            }
            if next == Some(Punct::Less) {
                return false;
            }
            return self.s.names_template(name, ctx);
        }
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
