//! new-expressions and delete-expressions ([expr.new], [expr.delete];
//! spec/parse.md §5.9): clang's `ParseCXXNewExpression`,
//! `ParseDirectNewDeclarator` and `ParseCXXDeleteExpression`.

use kiln_ast::ExprId;
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std, TokFlags};
use kiln_sema::DeclaratorContext;
use kiln_sema::declspec::{Chunk, Declarator};
use kiln_sema::new_delete::{NewInit, NewPlacement, NewTypeId};
use kiln_source::Loc;

use crate::keywords::Kw;
use crate::parser::{PExpr, Parser};

impl Parser<'_, '_> {
    /// True at `::new` / `::delete`.
    pub(crate) fn at_global_new_delete(&self) -> bool {
        self.at(Punct::ColonColon) && matches!(self.kw_of(self.peek(1)), Some(Kw::New | Kw::Delete))
    }

    /// A new- or delete-expression, at `::`, `new` or `delete`, followed
    /// by any (ill-formed) postfix suffix: a new-expression is a
    /// unary-expression, so `new int*[n]` cannot be followed by `[`
    /// (clang's `AllowSuffix = false`).
    pub(crate) fn parse_new_delete_expr(&mut self) -> PExpr {
        let start = self.loc();
        let global = self.eat(Punct::ColonColon).is_some();
        let e = if self.at_kw(Kw::New) {
            self.parse_new_expr(global, start)?
        } else {
            self.parse_delete_expr(global, start)?
        };
        let t = self.tok();
        let spelling = match Self::punct_of(t) {
            Some(Punct::LSquare) => "'['",
            Some(Punct::LParen) => "'('",
            Some(Punct::PlusPlus) => "'++'",
            Some(Punct::MinusMinus) => "'--'",
            Some(Punct::Period) => "'.'",
            Some(Punct::Arrow) => "'->'",
            _ => return Some(e),
        };
        let line_start = t.flags.contains(TokFlags::START_OF_LINE);
        if line_start && !matches!(Self::punct_of(t), Some(Punct::Period | Punct::Arrow)) {
            // "expected ';'" is the better diagnostic: the caller's.
            return Some(e);
        }
        let d =
            self.s.diag(DiagId::parse_postfix_after_unary_requires_parens, t.span.lo).arg(spelling);
        self.emit(d);
        self.parse_postfix_suffix(e)
    }

    /// `new` new-placement(opt) new-type-id new-initializer(opt), or
    /// `new` new-placement(opt) `( type-id )` new-initializer(opt).
    fn parse_new_expr(&mut self, global: bool, start: Loc) -> PExpr {
        self.bump();
        let mut placement = None;
        let tid = if self.at(Punct::LParen) {
            let open = self.loc();
            if self.is_type_id_start(self.pos() + 1) {
                self.bump();
                let tid = self.parse_paren_new_type_id(open)?;
                Some(tid)
            } else {
                self.bump();
                let mut args = Vec::new();
                if !self.at(Punct::RParen) {
                    args = self.parse_new_expr_list()?;
                }
                let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
                placement = Some(NewPlacement { args, lparen: open, rparen });
                None
            }
        } else {
            None
        };
        let tid = match tid {
            Some(t) => t,
            None if placement.is_some() && self.at(Punct::LParen) => {
                let open = self.bump().span.lo;
                self.parse_paren_new_type_id(open)?
            }
            None => self.parse_new_type_id()?,
        };
        let mut end = self.tok_at(self.pos().saturating_sub(1)).span.lo;
        let init = if self.at(Punct::LParen) {
            let open = self.bump().span.lo;
            let mut args = Vec::new();
            if !self.at(Punct::RParen) {
                args = self.parse_new_expr_list()?;
            }
            let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
            end = rparen;
            NewInit::Paren(args, open, rparen)
        } else if self.at(Punct::LBrace) {
            let Ok(list) = self.parse_init_list() else {
                return Some(self.s.act_on_error_expr(&[], start));
            };
            end = list.rbrace;
            NewInit::List(list)
        } else {
            NewInit::None
        };
        Some(self.s.act_on_cxx_new(global, start, placement, tid, init, end))
    }

    /// expression-list in a new-placement or new-initializer; `None`
    /// after a syntax error (the tokens up to the `)` are skipped).
    fn parse_new_expr_list(&mut self) -> Option<Vec<ExprId>> {
        let mut args = Vec::new();
        loop {
            if self.at(Punct::LBrace) {
                self.unsupported("braced-init-list arguments", self.loc());
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            }
            let Some(a) = self.parse_assignment_expr() else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            args.push(a);
            if self.eat(Punct::Comma).is_none() {
                return Some(args);
            }
        }
    }

    /// `( type-id )` after the `(`: an ordinary abstract declarator (its
    /// first array bound, if any, is still the allocated array's bound).
    fn parse_paren_new_type_id(&mut self, open: Loc) -> Option<NewTypeId> {
        let loc = self.loc();
        let ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        if !ds.has_type_specifier() {
            self.error_here(DiagId::parse_expected_type);
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        }
        let mut d = Declarator::new(DeclaratorContext::TypeName, self.loc());
        self.parse_declarator(&mut d);
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        let base = self.s.type_from_declspec(&ds);
        let loc = ds.type_loc.unwrap_or(loc);
        Some(NewTypeId { base, declarator: d, loc, parenthesized: true })
    }

    /// new-type-id: type-specifier-seq new-declarator(opt).
    fn parse_new_type_id(&mut self) -> Option<NewTypeId> {
        let loc = self.loc();
        let ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        if !ds.has_type_specifier() {
            self.error_here(DiagId::parse_expected_type);
            return None;
        }
        let mut d = Declarator::new(DeclaratorContext::TypeName, self.loc());
        self.parse_new_declarator(&mut d);
        let base = self.s.type_from_declspec(&ds);
        let loc = ds.type_loc.unwrap_or(loc);
        Some(NewTypeId { base, declarator: d, loc, parenthesized: false })
    }

    /// new-declarator: ptr-operator new-declarator(opt) |
    /// noptr-new-declarator. The ptr-operators are greedy: `new int*[n]`
    /// allocates `n` pointers.
    fn parse_new_declarator(&mut self, d: &mut Declarator) {
        let loc = self.loc();
        match Self::punct_of(self.tok()) {
            Some(Punct::Star) => {
                self.bump();
                let quals = self.parse_ptr_cv();
                self.parse_new_declarator(d);
                d.chunks.push(Chunk::Pointer { quals, loc });
            }
            Some(Punct::Amp) => {
                self.bump();
                self.skip_attributes();
                self.parse_new_declarator(d);
                d.chunks.push(Chunk::LRef { loc });
            }
            Some(Punct::AmpAmp) => {
                self.bump();
                self.skip_attributes();
                self.parse_new_declarator(d);
                d.chunks.push(Chunk::RRef { loc });
            }
            _ => self.parse_direct_new_declarator(d),
        }
    }

    /// noptr-new-declarator: `[` expression(opt) `]` followed by
    /// `[` constant-expression `]`s.
    fn parse_direct_new_declarator(&mut self, d: &mut Declarator) {
        let mut first = true;
        while self.at(Punct::LSquare) && !self.at_cxx11_attr() {
            let open = self.bump().span.lo;
            let size = if first {
                if self.at(Punct::RSquare) {
                    None
                } else {
                    self.parse_expression().or_else(|| fail(d))
                }
            } else {
                self.parse_constant_expr().or_else(|| fail(d))
            };
            if d.invalid {
                self.skip_until(&[Punct::RSquare], true, false);
                return;
            }
            first = false;
            self.expect_closing(Punct::RSquare, Punct::LSquare, open);
            self.skip_attributes();
            d.chunks.push(Chunk::Array { size, loc: open });
        }
    }

    /// `delete` cast-expression or `delete [ ]` cast-expression.
    fn parse_delete_expr(&mut self, global: bool, start: Loc) -> PExpr {
        self.bump();
        let mut array = false;
        if self.at(Punct::LSquare) && self.peek_is(1, Punct::RSquare) {
            // [expr.delete]/1: `delete []` is array delete, unless a lambda
            // follows (clang's lookahead).
            let next = self.peek(2);
            if matches!(Self::punct_of(next), Some(Punct::LBrace | Punct::Less | Punct::LParen)) {
                self.unsupported("lambda expressions", self.loc());
                return None;
            }
            let open = self.bump().span.lo;
            self.expect_closing(Punct::RSquare, Punct::LSquare, open);
            array = true;
        }
        let operand = self.parse_cast_expr()?;
        let cxx26 = self.std >= Std::Cxx26;
        Some(self.s.act_on_cxx_delete(global, array, start, operand, cxx26))
    }
}

/// Marks `d` invalid (a bound failed to parse).
fn fail(d: &mut Declarator) -> Option<ExprId> {
    d.invalid = true;
    None
}

impl Parser<'_, '_> {
    /// The operator-function-ids `operator new`, `operator delete`,
    /// `operator new[]` and `operator delete[]` ([over.oper.general]) as a
    /// declarator-id, at `operator`.
    pub(crate) fn parse_new_delete_operator_id(&mut self, d: &mut Declarator) {
        use kiln_ast::names::OverloadedOperator as O;
        let loc = self.bump().span.lo;
        let is_new = self.at_kw(Kw::New);
        self.bump();
        let array = self.at(Punct::LSquare) && self.peek_is(1, Punct::RSquare);
        if array {
            self.bump();
            self.bump();
        }
        d.op_name = Some(match (is_new, array) {
            (true, false) => O::New,
            (true, true) => O::ArrayNew,
            (false, false) => O::Delete,
            (false, true) => O::ArrayDelete,
        });
        d.name = Some((self.s.intern("operator"), loc));
    }
}
