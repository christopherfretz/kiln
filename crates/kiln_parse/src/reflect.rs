//! Reflection syntax, `-freflection` only (WP-267 spike; P2996R13
//! [expr.reflect], [basic.splice]; spec/parse.md "Reflection tokens"):
//! the reflect operator `^^` and the type splice `[: r :]`. Without the
//! flag the lexer never forms `^^`, `[:` or `:]`, so none of this runs.

use kiln_lex::{Punct, TokKind};

use crate::keywords::Kw;
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
        let ahead = self.reflect_lookahead();
        if ahead.is_none()
            && self.is_cxx_type_id_at(self.pos(), TypeIdContext::InTrailingReturnType).0
        {
            let ty = self.parse_type_id()?;
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_type(ty, loc, end));
        }
        let operand = self.tok().span.lo;
        let qual = self.parse_nested_name_nns().ok()?;
        let Some(name) = self.ident() else {
            // An operator-, conversion-function-id or destructor name
            // (/7): consumed whole, reflected later.
            if self.at_kw(Kw::Operator) {
                self.unsupported("reflection of an operator or conversion function name", operand);
                self.parse_operator_function_name()?;
                return None;
            }
            if self.at(Punct::Tilde) {
                self.unsupported("reflection of a destructor name", operand);
                self.bump();
                if self.ident().is_some() {
                    self.bump();
                }
                return None;
            }
            let bad = self.s.reflect_bad_operand(operand, loc);
            // A literal or parenthesized operand is consumed whole, so the
            // rest of the expression parses ([expr.reflect]/1 has no such
            // form; `^^(x)` and `^^42` are one error each).
            if qual.is_none()
                && (self.at(Punct::LParen)
                    || matches!(
                        self.tok().kind,
                        TokKind::PpNumber
                            | TokKind::CharLit(_)
                            | TokKind::StringLit(_)
                            | TokKind::RawStringLit(_)
                    ))
            {
                let _ = self.parse_primary_expr();
            }
            return Some(bad);
        };
        let name_loc = self.bump().span.lo;
        // `^^S::S` names the constructors ([class.qual]/2): an overload
        // set, never the injected-class-name.
        if ahead == Some(ReflectAhead::Constructor) {
            return Some(self.s.reflect_overload_set(name_loc, loc));
        }
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

    /// At `name` / `nns name` not followed by `<`: whether it names a
    /// template itself (its reflection, never a type-id) or, as
    /// `nns C::C`, the constructors of class `C`.
    fn reflect_lookahead(&self) -> Option<ReflectAhead> {
        let mut i = self.pos();
        let mut ctx = None;
        if Self::punct_of(self.tok_at(i)) == Some(Punct::ColonColon) {
            ctx = Some(self.s.ast.tu);
            i += 1;
        }
        loop {
            let TokKind::Identifier(name) = self.tok_at(i).kind else { return None };
            let next = Self::punct_of(self.tok_at(i + 1));
            if next == Some(Punct::ColonColon) {
                // Only plain namespace / class qualifiers are followed.
                ctx = Some(self.s.lookup_nested_ctx(name, ctx)?);
                i += 2;
                continue;
            }
            if next == Some(Punct::Less) {
                return None;
            }
            if let Some(c) = ctx
                && self.s.is_class_named(c, name)
            {
                return Some(ReflectAhead::Constructor);
            }
            return self.s.names_template(name, ctx).then_some(ReflectAhead::Template);
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

/// What [`Parser::reflect_lookahead`] found after `^^`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReflectAhead {
    /// A template-name not followed by `<`.
    Template,
    /// `nns C::C`.
    Constructor,
}
