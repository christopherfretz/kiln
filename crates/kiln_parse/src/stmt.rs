//! Statements ([stmt]; spec/parse.md §6).

use kiln_ast::decl::InitStyle;
use kiln_ast::stmt::IfKind;
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;
use kiln_lex::Punct;
use kiln_sema::{Declarator, DeclaratorContext, ScopeKind};
use kiln_source::Loc;

use crate::keywords::Kw;
use crate::parser::Parser;

/// A parsed condition: the condition variable, if one was declared, and
/// the (unconverted) condition expression.
type Cond = (Option<DeclId>, ExprId);

impl Parser<'_, '_> {
    /// compound-statement ([stmt.block]). A function body shares the
    /// function's scope (`new_scope` false). Returns the statement and the
    /// location of the `}`.
    pub(crate) fn parse_compound(&mut self, new_scope: bool) -> (StmtId, Loc) {
        let lbrace = self.bump().span.lo;
        if new_scope {
            self.s.push_scope(ScopeKind::Block);
        }
        let mut stmts = Vec::new();
        while !self.at(Punct::RBrace) && !self.at_eof() {
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        let rbrace = self.expect_closing(Punct::RBrace, Punct::LBrace, lbrace);
        if new_scope {
            self.s.pop_scope();
        }
        (self.s.act_on_compound(&stmts, lbrace, rbrace), rbrace)
    }

    /// A sub-statement in its own block scope ([stmt.pre]/1); a dropped
    /// statement becomes a null statement. A compound statement is that
    /// block itself, so it sits directly in the control statement's scope
    /// (clang's `InnerScope`, skipped before a `{`).
    fn parse_sub_statement(&mut self) -> StmtId {
        let loc = self.loc();
        let implicit = !self.at(Punct::LBrace);
        if implicit {
            self.s.push_scope(ScopeKind::Block);
        }
        let s = self.parse_statement();
        if implicit {
            self.s.pop_scope();
        }
        s.unwrap_or_else(|| self.s.act_on_null_stmt(loc))
    }

    /// A labeled statement's sub-statement: a label at the end of a block
    /// labels a null statement (C++23 [stmt.label]).
    fn parse_labeled_sub(&mut self) -> StmtId {
        let loc = self.loc();
        if self.at(Punct::RBrace) {
            return self.s.act_on_null_stmt(loc);
        }
        self.parse_statement().unwrap_or_else(|| self.s.act_on_null_stmt(loc))
    }

    /// statement ([stmt.pre]). `None` if the statement was dropped after a
    /// syntax error.
    pub fn parse_statement(&mut self) -> Option<StmtId> {
        if self.at_cxx11_attr() {
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
        }
        let loc = self.loc();
        match Self::punct_of(self.tok()) {
            Some(Punct::LBrace) => return Some(self.parse_compound(true).0),
            Some(Punct::Semi) => {
                self.bump();
                return Some(self.s.act_on_null_stmt(loc));
            }
            _ => {}
        }
        if let Some(k) = self.kw() {
            match k {
                Kw::If => return self.parse_if(),
                Kw::Switch => return self.parse_switch(),
                Kw::While => return self.parse_while(),
                Kw::Do => return self.parse_do(),
                Kw::For => return self.parse_for(),
                Kw::Case => return self.parse_case(),
                Kw::Default if self.peek_is(1, Punct::Colon) => {
                    self.bump();
                    self.bump();
                    self.s.act_on_default_label(loc);
                    let sub = self.parse_labeled_sub();
                    return Some(self.s.act_on_default(sub, loc));
                }
                Kw::Break => {
                    self.bump();
                    let s = self.s.act_on_break(loc);
                    self.expect_semi_after_stmt("break");
                    return Some(s);
                }
                Kw::Continue => {
                    self.bump();
                    let s = self.s.act_on_continue(loc);
                    self.expect_semi_after_stmt("continue");
                    return Some(s);
                }
                Kw::Return => return self.parse_return(),
                Kw::Goto => return self.parse_goto(),
                Kw::Try | Kw::CoReturn | Kw::Asm => {
                    self.unsupported(&format!("'{}' statements", k.spelling()), loc);
                    self.skip_statement();
                    return None;
                }
                Kw::StaticAssert => {
                    let d = self.parse_static_assert()?;
                    return Some(self.s.act_on_decl_stmt(&[d], loc));
                }
                Kw::Using => {
                    let ds = self.parse_using();
                    if ds.is_empty() {
                        return None;
                    }
                    return Some(self.s.act_on_decl_stmt(&ds, loc));
                }
                Kw::Namespace
                    if self.ident_of(self.peek(1)).is_some() && self.peek_is(2, Punct::Equal) =>
                {
                    self.bump();
                    let name = self.ident()?;
                    let nl = self.bump().span.lo;
                    self.bump();
                    let d = self.parse_namespace_alias(name, nl)?;
                    return Some(self.s.act_on_decl_stmt(&[d], loc));
                }
                Kw::Namespace | Kw::Template => {
                    self.unsupported(&format!("'{}' here", k.spelling()), loc);
                    self.skip_statement();
                    return None;
                }
                _ => {}
            }
        }
        if let Some(name) = self.ident()
            && self.peek_is(1, Punct::Colon)
        {
            self.bump();
            self.bump();
            self.skip_attributes();
            let l = self.s.act_on_label(name, loc);
            let sub = self.parse_labeled_sub();
            return Some(self.s.act_on_label_stmt(l, sub, loc));
        }
        if self.is_cxx_declaration_statement(false) {
            let decls = self.parse_simple_declaration(DeclaratorContext::Block, false);
            if decls.is_empty() {
                return None;
            }
            return Some(self.s.act_on_decl_stmt(&decls, loc));
        }
        self.parse_expression_statement()
    }

    /// `goto identifier ;` ([stmt.goto]).
    fn parse_goto(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        if self.at(Punct::Star) {
            self.unsupported("computed goto", self.loc());
            self.skip_to_semi();
            return None;
        }
        let Some(name) = self.ident() else {
            let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
            self.emit(d);
            self.skip_to_semi();
            return None;
        };
        let nl = self.bump().span.lo;
        let s = self.s.act_on_goto(name, nl, loc);
        self.expect_semi_after_stmt("goto");
        Some(s)
    }

    /// Skips a statement kiln does not parse.
    fn skip_statement(&mut self) {
        self.skip_unsupported_declaration();
    }

    /// expression-statement ([stmt.expr]).
    fn parse_expression_statement(&mut self) -> Option<StmtId> {
        let Some(e) = self.parse_expression() else {
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
            return None;
        };
        let s = self.s.act_on_expr_stmt(e);
        if self.eat(Punct::Semi).is_none() {
            self.s.error(DiagId::parse_expected_semi_after_expr, self.expected_loc());
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
        Some(s)
    }

    /// Expects the `;` ending a `what` statement.
    fn expect_semi_after_stmt(&mut self, what: &str) {
        if self.eat(Punct::Semi).is_none() {
            let d =
                self.s.diag(DiagId::parse_expected_semi_after_stmt, self.expected_loc()).arg(what);
            self.emit(d);
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
    }

    /// `( [init-statement] condition )` for `if` and `switch`. Returns
    /// the init-statement and the condition expression.
    fn parse_paren_condition(&mut self, what: &str) -> Option<(Option<StmtId>, Cond)> {
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg(format!("'{what}'"))
                .arg("'('");
            self.emit(d);
            self.skip_until(&[Punct::RBrace], true, false);
            return None;
        };
        let mut init = None;
        if let Some((_, Punct::Semi)) = self.scan_for(&[Punct::Semi, Punct::RParen]) {
            init = self.parse_statement();
        }
        let Some(c) = self.parse_condition() else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some((init, c))
    }

    /// condition ([stmt.pre]/1): an expression, or a declaration of a
    /// single variable with a brace-or-equal-initializer. Returns the
    /// condition variable (if any) and the condition expression -- for a
    /// declaration, a reference to the variable -- not yet converted.
    fn parse_condition(&mut self) -> Option<Cond> {
        if !self.is_cxx_condition_declaration() {
            return self.parse_expression().map(|e| (None, e));
        }
        let ctx = DeclaratorContext::Condition;
        let ds = self.parse_decl_specifiers(ctx);
        if ds.is_empty() {
            self.error_here(DiagId::parse_expected_expression);
            return None;
        }
        let mut d = Declarator::new(ctx, self.loc());
        d.direct_init = true;
        self.parse_declarator(&mut d);
        let base = self.s.type_from_declspec_at(&ds, d.name_loc());
        if self.at_attr() {
            self.parse_attributes(&mut d.attrs);
        }
        let Some((_, name_loc)) = d.name else {
            if !d.invalid {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            return None;
        };
        d.init_style = match Self::punct_of(self.tok()) {
            Some(Punct::Equal) => Some(InitStyle::C),
            Some(Punct::LBrace) => Some(InitStyle::List),
            Some(Punct::LParen) => {
                self.s.error(DiagId::parse_expected_init_in_condition_lparen, name_loc);
                return None;
            }
            _ => {
                // clang reports at the declarator's name.
                self.s.error(DiagId::parse_expected_init_in_condition, name_loc);
                return None;
            }
        };
        let v = self.s.act_on_declarator(&ds, base, &d);
        let Ok(Some(init)) = self.parse_initializer(d.init_style) else {
            return None;
        };
        let v = v?;
        self.s.act_on_init(v, init);
        Some(self.s.act_on_condition_var(v, name_loc))
    }

    /// if-statement ([stmt.if]): `if constexpr? ( init-statement?
    /// condition ) statement (else statement)?` and `if !? consteval
    /// compound-statement (else statement)?`.
    fn parse_if(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let constexpr = self.eat_kw(Kw::Constexpr).is_some();
        let negated = self.eat(Punct::Exclaim).is_some();
        if !constexpr && self.at_kw(Kw::Consteval) {
            return self.parse_if_consteval(loc, negated);
        }
        if (constexpr || negated) && !self.at(Punct::LParen) {
            // clang: at the current token, then skip to the next `;`.
            let d = self.s.diag(DiagId::parse_expected_after, self.loc()).arg("'if'").arg("'('");
            self.emit(d);
            self.skip_until(&[Punct::Semi], false, true);
            return None;
        }
        self.s.push_scope(ScopeKind::Control);
        let Some((init, (var, c))) = self.parse_paren_condition("if") else {
            self.s.pop_scope();
            self.skip_if_rest_after_cond();
            return None;
        };
        let (kind, cond) = if constexpr {
            (IfKind::Constexpr, self.s.act_on_constexpr_condition(c))
        } else {
            (IfKind::Ordinary, self.s.act_on_condition(c))
        };
        let then = self.parse_sub_statement();
        let els =
            if self.eat_kw(Kw::Else).is_some() { Some(self.parse_sub_statement()) } else { None };
        self.s.pop_scope();
        Some(self.s.act_on_if(kind, init, var, Some(cond), then, els, loc))
    }

    /// `if !? consteval compound-statement (else statement)?`
    /// ([stmt.if]/4), at `consteval`. Both sub-statements must be compound
    /// statements (checked after parsing them, as clang does); the one
    /// that runs during constant evaluation is an immediate function
    /// context ([expr.const]/16).
    fn parse_if_consteval(&mut self, loc: Loc, negated: bool) -> Option<StmtId> {
        let consteval_loc = self.bump().span.lo;
        let then_ok = self.at(Punct::LBrace);
        if !negated {
            self.s.enter_immediate_context();
        }
        let then = self.parse_sub_statement();
        if !negated {
            self.s.exit_immediate_context();
        }
        let mut else_bad = None;
        let els = if let Some(else_loc) = self.eat_kw(Kw::Else) {
            if !self.at(Punct::LBrace) {
                else_bad = Some(else_loc);
            }
            if negated {
                self.s.enter_immediate_context();
            }
            let e = self.parse_sub_statement();
            if negated {
                self.s.exit_immediate_context();
            }
            Some(e)
        } else {
            None
        };
        let bad = if then_ok {
            else_bad.map(|l| (l, "else"))
        } else {
            Some((consteval_loc, "consteval"))
        };
        if let Some((at, what)) = bad {
            let d = self.s.diag(DiagId::parse_expected_after, at).arg(what).arg("{");
            self.emit(d);
            return None;
        }
        let kind = if negated { IfKind::NotConsteval } else { IfKind::Consteval };
        Some(self.s.act_on_if(kind, None, None, None, then, els, loc))
    }

    /// Recovery after an unsupported `if` head: skip the condition and
    /// the sub-statements.
    fn skip_if_rest(&mut self) {
        while !self.at(Punct::LParen) && !self.at_eof() && !self.at(Punct::LBrace) {
            self.bump();
        }
        if self.eat(Punct::LParen).is_some() {
            self.skip_until(&[Punct::RParen], false, true);
        }
        self.skip_if_rest_after_cond();
    }

    fn skip_if_rest_after_cond(&mut self) {
        self.s.push_scope(ScopeKind::Block);
        self.skip_sub_statement();
        if self.eat_kw(Kw::Else).is_some() {
            self.skip_sub_statement();
        }
        self.s.pop_scope();
    }

    /// Skips one statement without building it.
    fn skip_sub_statement(&mut self) {
        if self.at(Punct::LBrace) {
            self.bump();
            self.skip_until(&[Punct::RBrace], false, true);
        } else if self.at_kw(Kw::If) {
            self.bump();
            self.skip_if_rest();
        } else {
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
    }

    /// switch-statement ([stmt.switch]).
    fn parse_switch(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        self.s.push_scope(ScopeKind::Control);
        let Some((init, (var, c))) = self.parse_paren_condition("switch") else {
            self.skip_sub_statement();
            self.s.pop_scope();
            return None;
        };
        let cond = self.s.act_on_start_switch(c, loc);
        let body = self.parse_sub_statement();
        let s = self.s.act_on_finish_switch(init, var, cond, body, loc);
        self.s.pop_scope();
        Some(s)
    }

    /// `case constant-expression :` ([stmt.label]).
    fn parse_case(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let Some(v) = self.parse_constant_expr() else {
            self.skip_until(&[Punct::Colon, Punct::RBrace], true, false);
            self.eat(Punct::Colon);
            return None;
        };
        if self.at(Punct::Ellipsis) {
            self.unsupported("case ranges", self.loc());
            self.skip_until(&[Punct::Colon], true, true);
            return None;
        }
        let v = self.s.act_on_case_value(v, loc);
        if self.eat(Punct::Colon).is_none() {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'case'")
                .arg("':'");
            self.emit(d);
        }
        let sub = self.parse_labeled_sub();
        Some(self.s.act_on_case(v, sub, loc))
    }

    /// A loop condition `( condition )`; `do` takes only an expression.
    fn parse_loop_condition(&mut self, what: &str, allow_decl: bool) -> Option<Cond> {
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg(format!("'{what}'"))
                .arg("'('");
            self.emit(d);
            return None;
        };
        let c = if allow_decl {
            self.parse_condition()
        } else {
            self.parse_expression().map(|e| (None, e))
        };
        let Some((var, c)) = c else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some((var, self.s.act_on_condition(c)))
    }

    /// while-statement ([stmt.while]).
    fn parse_while(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        self.s.push_scope(ScopeKind::Control);
        let cond = self.parse_loop_condition("while", true);
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        self.s.pop_scope();
        let (var, cond) = cond?;
        Some(self.s.act_on_while(var, cond, body, loc))
    }

    /// do-statement ([stmt.do]).
    fn parse_do(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        if self.eat_kw(Kw::While).is_none() {
            self.error_here(DiagId::parse_expected_while);
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
            return None;
        }
        let cond = self.parse_loop_condition("while", false);
        let Some((_, cond)) = cond else {
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
            return None;
        };
        let s = self.s.act_on_do(body, cond, loc);
        self.expect_semi_after_stmt("do/while");
        Some(s)
    }

    /// for-statement ([stmt.for]); range-based for is not supported.
    fn parse_for(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'for'")
                .arg("'('");
            self.emit(d);
            self.skip_until(&[Punct::RBrace], true, false);
            return None;
        };
        if let Some((_, Punct::Colon)) = self.scan_for(&[Punct::Semi, Punct::Colon, Punct::RParen])
        {
            self.unsupported("range-based for", loc);
            self.skip_until(&[Punct::RParen], false, true);
            self.skip_sub_statement();
            return None;
        }
        self.s.push_scope(ScopeKind::Control);
        let r = self.parse_for_rest(open, loc);
        self.s.pop_scope();
        r
    }

    fn parse_for_rest(&mut self, open: Loc, loc: Loc) -> Option<StmtId> {
        let init = if self.at(Punct::Semi) {
            self.bump();
            None
        } else if self.is_cxx_declaration_statement(true) {
            let l = self.loc();
            let decls = self.parse_simple_declaration(DeclaratorContext::ForInit, false);
            Some(self.s.act_on_decl_stmt(&decls, l))
        } else {
            self.parse_expression_statement()
        };
        let (mut var, mut cond) = (None, None);
        if !self.at(Punct::Semi) {
            let Some((v, c)) = self.parse_condition() else {
                self.skip_until(&[Punct::RParen], false, true);
                self.skip_sub_statement();
                return None;
            };
            var = v;
            cond = Some(self.s.act_on_condition(c));
        }
        if self.eat(Punct::Semi).is_none() {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("for-statement condition")
                .arg("';'");
            self.emit(d);
            self.skip_until(&[Punct::RParen], false, true);
            self.skip_sub_statement();
            return None;
        }
        let mut inc = None;
        if !self.at(Punct::RParen) {
            let Some(e) = self.parse_expression() else {
                self.skip_until(&[Punct::RParen], false, true);
                self.skip_sub_statement();
                return None;
            };
            inc = Some(e);
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        Some(self.s.act_on_for(init, var, cond, inc, body, loc))
    }

    /// return-statement ([stmt.return]).
    fn parse_return(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let mut value = None;
        if self.at(Punct::LBrace) {
            let Ok(l) = self.parse_init_list() else {
                self.skip_to_semi();
                return None;
            };
            let s = self.s.act_on_return_list(&l, loc);
            self.expect_semi_after_stmt("return");
            return Some(s);
        }
        if !self.at(Punct::Semi) {
            let Some(e) = self.parse_expression() else {
                self.skip_until(&[Punct::RBrace], true, false);
                self.eat(Punct::Semi);
                return None;
            };
            value = Some(e);
        }
        let s = self.s.act_on_return(value, loc);
        self.expect_semi_after_stmt("return");
        Some(s)
    }
}
