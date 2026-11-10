//! Reflection syntax, `-freflection` only (WP-267 spike; P2996R13
//! [expr.reflect], [basic.splice]; spec/parse.md "Reflection tokens"):
//! the reflect operator `^^` and the type splice `[: r :]`. Without the
//! flag the lexer never forms `^^`, `[:` or `:]`, so none of this runs.

use kiln_lex::{Punct, TokKind};

use crate::keywords::Kw;
use crate::parser::{PExpr, Parser, is_template_class};
use crate::tentative::TypeIdContext;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_sema::DeclSite;
use kiln_sema::templ::actions::TemplateIdRef;
use kiln_sema::using::Qualifier;
use kiln_source::Loc;

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
            let ty = self.parse_type_id_into(None, DeclSite::Reflect)?;
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_type(ty, loc, end));
        }
        let operand = self.tok().span.lo;
        let qual = self.parse_nested_name_nns().ok()?;
        // `^^T::template m<U>` ([temp.names]/5): after a dependent
        // qualifier only.
        let dependent = qual.filter(|q| self.s.is_dependent_ctx(q.ctx));
        let template_kw = if dependent.is_some() { self.eat_kw(Kw::Template) } else { None };
        let Some(name) = self.ident() else {
            return self.parse_reflect_special_name(loc, operand, qual, dependent, template_kw);
        };
        let name_loc = self.bump().span.lo;
        // `^^S::S` names the constructors ([class.qual]/2): an overload
        // set, never the injected-class-name.
        if ahead == Some(ReflectAhead::Constructor) {
            return Some(self.s.reflect_overload_set(name_loc, loc));
        }
        if let Some(q) = dependent {
            let targs = self.reflect_dependent_targs(template_kw).ok()?;
            let end = self.prev_end();
            let name = kiln_ast::names::DeclName::ident(name);
            return Some(self.s.act_on_reflect_dependent(
                (q.nns, q.loc),
                name,
                targs,
                template_kw,
                name_loc,
                loc,
                end,
            ));
        }
        if self.at(Punct::Less) {
            let class = self.s.classify_name(name, qual.map(|q| q.ctx));
            if is_template_class(class) {
                let mut r = self.parse_template_id_rest(class, name, name_loc).ok()?;
                // The whole overload set the name finds, not just the
                // template classification picked ([over.over]/3).
                if let TemplateIdRef::Overload(e) = r {
                    let e = self.s.widen_function_template_id(e, name, qual.map(|q| q.ctx));
                    if let Some(q) = qual {
                        self.s.set_template_id_begin(e, operand, name_loc);
                        self.s.set_template_id_qualifier(e, q.nns);
                    }
                    r = TemplateIdRef::Overload(e);
                } else if let (TemplateIdRef::Value(e), Some(q)) = (r, qual) {
                    // A variable template-id keeps its qualifier as an
                    // expression's does: `^^ns::v<T>` and `^^v<T>` are
                    // different dependent operands, in the signature's
                    // profile and its mangling (WP-1250).
                    self.s.set_template_id_begin(e, operand, name_loc);
                    self.s.set_template_id_qualifier(e, q.nns);
                }
                let end = self.prev_end();
                return Some(self.s.act_on_reflect_template_id(r, name_loc, loc, end));
            }
        }
        let end = self.prev_end();
        Some(self.s.act_on_reflect_name(qual, name, name_loc, loc, end))
    }

    /// After `^^ nns`, an operand that is not an identifier: an operator-,
    /// literal-operator- or conversion-function-id, an operator template-id,
    /// or a destructor name ([expr.reflect]/7), else the bad-operand error.
    fn parse_reflect_special_name(
        &mut self,
        loc: Loc,
        operand: Loc,
        qual: Option<Qualifier>,
        dependent: Option<Qualifier>,
        template_kw: Option<Loc>,
    ) -> PExpr {
        if self.at_kw(Kw::Operator) {
            let nl = self.loc();
            let name = self.parse_operator_function_name()?;
            if let Some(q) = dependent {
                let targs = self.reflect_dependent_targs(template_kw).ok()?;
                let end = self.prev_end();
                return Some(self.s.act_on_reflect_dependent(
                    (q.nns, q.loc),
                    name,
                    targs,
                    template_kw,
                    nl,
                    loc,
                    end,
                ));
            }
            // `^^ns::operator== <int>`, `^^operator""_x<'1'>`: an
            // operator- or literal-operator-id template-id ([temp.names]/1)
            // names the specialization its arguments select, as an
            // identifier template-id does (/7, [over.over]/3).
            if self.at_operator_template_args(name, qual, None) {
                let (args, _, _) = self.parse_template_arg_list()?;
                let ctx = qual.map(|q| q.ctx);
                let e = self.s.act_on_operator_template_id(name, &args, nl, ctx, false);
                if let Some(q) = qual {
                    self.s.set_template_id_begin(e, operand, nl);
                    self.s.set_template_id_qualifier(e, q.nns);
                }
                let end = self.prev_end();
                let r = TemplateIdRef::Overload(e);
                return Some(self.s.act_on_reflect_template_id(r, nl, loc, end));
            }
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_dn(qual, name, nl, loc, end));
        }
        if self.at(Punct::Tilde) {
            self.bump();
            let Some(name) = self.ident() else {
                self.error_here(DiagId::parse_destructor_tilde_identifier);
                return None;
            };
            let nl = self.bump().span.lo;
            let end = self.prev_end();
            return Some(self.s.act_on_reflect_destructor(qual, name, nl, loc, end));
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
        Some(bad)
    }

    /// The template arguments after `^^T::template m` (none without the
    /// `template` keyword: `<` is then the less-than operator).
    fn reflect_dependent_targs(
        &mut self,
        template_kw: Option<Loc>,
    ) -> Result<Option<kiln_ast::ArgListId>, ()> {
        if template_kw.is_none() || !self.at(Punct::Less) {
            return Ok(None);
        }
        let (args, _, _) = self.parse_template_arg_list().ok_or(())?;
        Ok(Some(self.s.ast.types.arg_list(&args)))
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

    /// The token index past the splice-specialization-specifier
    /// `[: r :] < args >` at token `i` (past `[: r :]` when no `<`
    /// follows): lookahead for a type position.
    pub(crate) fn splice_spec_end(&self, i: usize) -> Option<usize> {
        let after = self.splice_end(i)?;
        if Self::punct_of(self.tok_at(after)) != Some(Punct::Less) {
            return Some(after);
        }
        self.skip_template_args(after).map(|(end, _)| end + 1)
    }

    /// `[: constant-expression :]` in a type position
    /// ([basic.splice], [dcl.type.splice]): the type the operand
    /// reflects, at `[:`. A `<` after it opens a template argument
    /// list (a type-only context or after `typename`, P2996
    /// [temp.names]/3): the specialization named.
    pub(crate) fn parse_splice_type(&mut self) -> Option<QualType> {
        let open = self.loc();
        let (e, close) = self.parse_splice_specifier()?;
        if self.at(Punct::Less) {
            let (args, _, r) = self.parse_template_arg_list()?;
            return self.s.act_on_splice_template_type(e, &args, open, r);
        }
        self.s.act_on_splice_type(e, open, close)
    }

    /// The splice-specifier `[: constant-expression :]` at `[:`
    /// ([basic.splice]): its operand and the `:]` location. `None` after
    /// an error (the `:]` is consumed when present).
    pub(crate) fn parse_splice_specifier(&mut self) -> Option<(ExprId, Loc)> {
        let open = self.bump().span.lo;
        let e = self.with_greater_op(true, Self::parse_constant_expr);
        let close = self.expect_closing(Punct::SpliceClose, Punct::SpliceOpen, open);
        Some((e?, close))
    }

    /// [`Parser::parse_splice_specifier`]'s operand alone.
    pub(crate) fn parse_splice_operand(&mut self) -> Option<ExprId> {
        self.parse_splice_specifier().map(|(e, _)| e)
    }

    /// The splice-expression `[: r :]` at `[:` ([expr.prim.splice]); a
    /// `<` after a frozen operand that designates a template opens its
    /// argument list ([`kiln_sema::Sema::splice_names_template`]).
    pub(crate) fn parse_splice_expr(&mut self) -> Option<ExprId> {
        let open = self.loc();
        let (e, close) = self.parse_splice_specifier()?;
        if self.at(Punct::Less) && self.s.splice_names_template(e) {
            return self.parse_splice_template_args(e, open, open);
        }
        Some(self.s.act_on_splice_expr(e, open, close))
    }

    /// The splice-expression `template [: r :]` or
    /// `template [: r :] < args >` at `template` ([expr.prim.splice]/3,
    /// P2996): a function template's overload set, or the
    /// specialization of a variable template, concept or function
    /// template the template-id names.
    pub(crate) fn parse_template_splice_expr(&mut self) -> Option<ExprId> {
        let begin = self.bump().span.lo;
        let open = self.loc();
        let (e, close) = self.parse_splice_specifier()?;
        if self.at(Punct::Less) {
            return self.parse_splice_template_args(e, open, begin);
        }
        Some(self.s.act_on_splice_expr(e, open, close))
    }

    /// The `< args >` of the splice template-id whose splice operand is
    /// `e` (`open` its `[:`, `begin` its first token: `template` or
    /// `[:`): an overload set not called, directly or as the whole of a
    /// parenthesized callee (`(template [:r:]<A>)(x)`,
    /// [`Parser::template_id_is_callee`]), resolves to its single
    /// specialization ([over.over]).
    fn parse_splice_template_args(&mut self, e: ExprId, open: Loc, begin: Loc) -> Option<ExprId> {
        let (args, _, r) = self.parse_template_arg_list()?;
        let x = self.s.act_on_splice_template_expr(e, &args, open, r);
        Some(if self.template_id_is_callee(begin) {
            x
        } else {
            self.s.resolve_single_template_id(x)
        })
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
