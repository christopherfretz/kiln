//! Statements ([stmt]; spec/parse.md §6).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::InitStyle;
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std, TokKind};
use kiln_sema::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, EmptyBodyStmt, ExpansionInit, RangeInit,
    ScopeKind, TypeSpecKind,
};
use kiln_source::Loc;

use crate::fp_pragma::FpAt;
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
        self.parse_compound_in(new_scope, false)
    }

    /// A compound statement; `stmt_expr`: it is a statement expression's
    /// body, whose statements are parsed in the `InStmtExpr` context.
    pub(crate) fn parse_compound_in(&mut self, new_scope: bool, stmt_expr: bool) -> (StmtId, Loc) {
        let lbrace = self.bump().span.lo;
        if new_scope {
            self.s.push_scope(ScopeKind::Block);
        }
        // FP pragmas at the start apply to the compound statement
        // ([stmt.block]; C23 7.12.2/2), which restores the enclosing state
        // at its end.
        let fp_outer = self.s.fp_pragma_state();
        self.take_fp_pragmas(FpAt::Compound);
        let mut stmts = Vec::new();
        while !self.at(Punct::RBrace) && !self.at_eof() {
            self.in_stmt_expr = stmt_expr;
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        // Loop hints before the `}`: clang parses them and then finds no
        // statement.
        if let Some(h) = self.take_loop_hints() {
            let _ = self.loop_hint_attrs(h);
            self.expected_here("statement");
        }
        self.take_fp_pragmas(FpAt::Statement);
        let rbrace = self.expect_closing(Punct::RBrace, Punct::LBrace, lbrace);
        if new_scope {
            self.s.pop_scope();
        }
        let fp = self.s.fp_pragma_state();
        self.s.set_fp_pragma_state(fp_outer);
        let block = self.s.act_on_compound_fp(&stmts, lbrace, rbrace, fp);
        self.s.check_empty_loop_bodies(&stmts);
        (block, rbrace)
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
    fn parse_labeled_sub(&mut self, in_stmt_expr: bool) -> StmtId {
        let loc = self.loc();
        if self.at(Punct::RBrace) {
            return self.s.act_on_null_stmt(loc);
        }
        self.in_stmt_expr = in_stmt_expr;
        self.parse_statement().unwrap_or_else(|| self.s.act_on_null_stmt(loc))
    }

    /// statement ([stmt.pre]). `None` if the statement was dropped after a
    /// syntax error.
    pub fn parse_statement(&mut self) -> Option<StmtId> {
        if !self.stack_ok() {
            return None;
        }
        self.take_fp_pragmas(FpAt::Statement);
        // Loop-hint pragmas before the statement (clang
        // `ParsePragmaLoopHint`): attributes of it, before any `[[...]]`.
        let (mut hints, hints_at) = match self.take_loop_hints() {
            Some(h) => {
                let at = h[0].start;
                (self.loop_hint_attrs(h), at)
            }
            None => (Vec::new(), Loc::INVALID),
        };
        let s = self.parse_statement_hinted(&mut hints);
        if hints.is_empty() {
            return s;
        }
        // A statement with no other statement attributes.
        let s = s?;
        Some(self.s.act_on_attributed_stmt(&hints, s, hints_at, self.std))
    }

    /// A statement after its loop hints `hints`: one with statement
    /// attributes takes the hints into its `AttributedStmt` (clearing
    /// `hints`).
    fn parse_statement_hinted(&mut self, hints: &mut Vec<kiln_ast::AttrId>) -> Option<StmtId> {
        if self.at_cxx11_attr() || self.at_kw(Kw::Alignas) || self.at_gnu_attr_null_stmt() {
            let start = self.loc();
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            // Statement attributes are applied once the statement is
            // parsed (clang's `ProcessStmtAttributes`); `[[assume]]`,
            // `[[likely]]`, `[[unlikely]]` and `[[fallthrough]]` are kept
            // (spec/parse.md §6).
            if attrs.iter().any(|&a| {
                matches!(
                    self.s.ast.attrs.get(a).kind,
                    AttrKind::ASSUME
                        | AttrKind::LIKELY
                        | AttrKind::UNLIKELY
                        | AttrKind::FALLTHROUGH
                        | AttrKind::MUSTTAIL
                )
            }) {
                let sub = self.parse_statement_after_attrs()?;
                let mut all = std::mem::take(hints);
                all.extend_from_slice(&attrs);
                return Some(self.s.act_on_attributed_stmt(&all, sub, start, self.std));
            }
            // Otherwise they appertain to a declaration's entities; on any
            // other statement they are errors (clang
            // `ProcessStmtAttributes`).
            let n = attrs.len();
            self.stmt_attrs = attrs.clone();
            self.stmt_attrs_loc = Some(start);
            let s = self.parse_statement_after_attrs()?;
            let asm = match self.s.ast.stmts.get(s).kind() {
                StmtKind::DeclStmt => {
                    self.s.act_on_decl_stmt_attrs(s, &attrs);
                    return Some(s);
                }
                StmtKind::LabelStmt => {
                    self.s.reject_annotations_in(&attrs, Some("a label"));
                    return Some(s);
                }
                StmtKind::GCCAsmStmt => true,
                _ => false,
            };
            if n > 0 {
                self.s.act_on_stmt_decl_attrs(&attrs, asm);
            }
            // WP-813: `[[clang::noinline]]`, `[[clang::always_inline]]`,
            // `[[clang::nomerge]]` and `[[clang::code_align]]` are also
            // statement attributes (on a declaration they stay the
            // declaration's).
            let stmt_attrs: Vec<kiln_ast::AttrId> = attrs
                .iter()
                .copied()
                .filter(|&a| {
                    matches!(
                        self.s.ast.attrs.get(a).kind,
                        AttrKind::NOINLINE
                            | AttrKind::ALWAYS_INLINE
                            | AttrKind::NOMERGE
                            | AttrKind::CODE_ALIGN
                    )
                })
                .collect();
            if !asm && !stmt_attrs.is_empty() {
                let mut all = std::mem::take(hints);
                all.extend_from_slice(&stmt_attrs);
                return Some(self.s.act_on_attributed_stmt(&all, s, start, self.std));
            }
            return Some(s);
        }
        self.parse_statement_after_attrs()
    }

    /// A namespace-alias-definition at block scope, at `namespace`; it
    /// takes no attribute-specifier-seq ([dcl.pre]/5).
    fn parse_block_namespace_alias(&mut self, lead_loc: Option<Loc>, loc: Loc) -> Option<StmtId> {
        self.prohibit_attrs_at(lead_loc);
        self.bump();
        let name = self.ident()?;
        let nl = self.bump().span.lo;
        self.bump();
        let d = self.parse_namespace_alias(name, nl)?;
        Some(self.s.act_on_decl_stmt(&[d], loc))
    }

    /// A statement after its attribute-specifier-seq.
    fn parse_statement_after_attrs(&mut self) -> Option<StmtId> {
        let lead_attrs = std::mem::take(&mut self.stmt_attrs);
        let lead_loc = self.stmt_attrs_loc.take().filter(|_| !lead_attrs.is_empty());
        let in_stmt_expr = std::mem::take(&mut self.in_stmt_expr);
        self.skip_extension_before_using();
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
                    let sub = self.parse_labeled_sub(false);
                    return Some(self.s.act_on_default(sub, loc));
                }
                Kw::ContractAssert => return self.parse_contract_assert(),
                Kw::Break | Kw::Continue => return Some(self.parse_break_continue(k, loc)),
                Kw::Return => return self.parse_return(),
                Kw::Goto => return self.parse_goto(),
                Kw::Try => return self.parse_try_stmt(),
                Kw::Asm => return self.parse_asm_statement(),
                Kw::CoReturn => return self.parse_coreturn(),
                // [dcl.pre]/5: static_assert, a consteval block (P3289R1), an
                // alias or a namespace alias definition takes no attributes.
                Kw::StaticAssert => {
                    self.prohibit_attrs_at(lead_loc);
                    let d = self.parse_static_assert()?;
                    return Some(self.s.act_on_decl_stmt(&[d], loc));
                }
                Kw::Consteval if self.at_consteval_block() => {
                    return Some(self.parse_consteval_block_stmt(lead_loc, loc));
                }
                Kw::Using => {
                    let ds = self.parse_using_leading(&lead_attrs, lead_loc);
                    if ds.is_empty() {
                        return None;
                    }
                    return Some(self.s.act_on_decl_stmt(&ds, loc));
                }
                Kw::Namespace
                    if self.ident_of(self.peek(1)).is_some() && self.peek_is(2, Punct::Equal) =>
                {
                    return self.parse_block_namespace_alias(lead_loc, loc);
                }
                Kw::Template if self.kw_of(self.peek(1)) == Some(Kw::For) => {
                    return self.parse_expansion_stmt();
                }
                // `template [: r :] ...`: a splice template-id
                // ([expr.prim.splice], [expr.prim.id.qual]).
                Kw::Template if self.peek_is(1, Punct::SpliceOpen) => {}
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
            let sub = self.parse_labeled_sub(in_stmt_expr);
            return Some(self.s.act_on_label_stmt(l, sub, loc));
        }
        if self.skip_ambiguous_name_stmt(loc) || self.skip_failed_scope_stmt() {
            return None;
        }
        if self.is_cxx_declaration_statement(false) {
            self.lead_decl_attrs = lead_attrs;
            self.stmt_attrs_loc = lead_loc;
            let decls = self.parse_simple_declaration(DeclaratorContext::Block, false);
            if decls.is_empty() {
                return None;
            }
            return Some(self.s.act_on_decl_stmt(&decls, loc));
        }
        self.parse_expression_statement(in_stmt_expr)
    }

    /// An unqualified name whose lookup is ambiguous starts neither a
    /// declaration nor an expression: reported, and the statement
    /// skipped ([basic.lookup.general]/1; clang's `ClassifyName` error
    /// in `ParseStatementOrDeclaration`). True when skipped.
    fn skip_ambiguous_name_stmt(&mut self, loc: Loc) -> bool {
        let Some(name) = self.ident() else { return false };
        if self.peek_is(1, Punct::ColonColon) || !self.s.diagnose_ambiguous_name(name, None, loc) {
            return false;
        }
        self.skip_to_semi();
        true
    }

    /// A statement `Q::name ...` whose qualifier failed
    /// ([`Parser::at_failed_scope_declaration`]) is skipped silently.
    /// True when skipped.
    fn skip_failed_scope_stmt(&mut self) -> bool {
        if !self.at_failed_scope_declaration() {
            return false;
        }
        self.skip_to_semi();
        true
    }

    /// `goto identifier ;` ([stmt.goto]); GNU `goto * expression ;`
    /// (clang `ParseGotoStatement`: "use of GNU indirect-goto extension"
    /// at the `*`, the operand is a full-expression; an `IndirectGotoStmt`).
    fn parse_goto(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        if self.at(Punct::Star) {
            let star = self.bump().span.lo;
            let d = self.s.diag(DiagId::parse_gnu_indirect_goto, star);
            self.emit(d);
            let Some(e) = self.parse_expression() else {
                self.skip_to_semi();
                return None;
            };
            let s = self.s.act_on_indirect_goto(e, loc, star);
            self.expect_semi_after_stmt("goto");
            return s;
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
    /// `in_stmt_expr`: directly in a statement expression, where the
    /// statement before the closing `})` (null statements aside) yields
    /// the statement expression's value (clang's `handleExprStmt`).
    fn parse_expression_statement(&mut self, in_stmt_expr: bool) -> Option<StmtId> {
        let mark = self.s.begin_delayed_typos();
        let outer_invalid = std::mem::replace(&mut self.expr_invalid, false);
        let parsed = self.parse_expression();
        // clang's `ParseExprStatement`: an invalid expression skips to the
        // `;` (`a = g(S::nom x)` with no `;` reports no "expected ';'").
        let invalid = std::mem::replace(&mut self.expr_invalid, outer_invalid);
        let Some(e) = parsed.filter(|_| !invalid) else {
            self.s.end_delayed_typos(mark);
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
            return None;
        };
        let result = in_stmt_expr && self.ends_stmt_expr();
        if !self.eat_extraneous_before_semi() && self.eat(Punct::Semi).is_none() {
            // clang's `ExpectAndConsumeSemi`: nothing is skipped; the next
            // statement starts at the current token (`X<T>::m b;` goes on
            // with `b;`).
            let at = self.expected_loc();
            let d = self
                .s
                .diag(DiagId::parse_expected_semi_after_expr, at)
                .fixit(kiln_diag::FixIt::insert(at, ";"));
            self.emit(d);
        }
        // clang's `handleExprStmt` finishes the full-expression after the
        // `;` (its delayed typo corrections report then: `S::nom y;`).
        self.s.end_delayed_typos(mark);
        let s = if result { self.s.act_on_stmt_expr_result(e) } else { self.s.act_on_expr_stmt(e) };
        Some(s)
    }

    /// Whether the tokens from here are `;`s followed by `})`. Under
    /// `-fclang-compat=22` only the statement's own `;` may come before
    /// the `})`: a trailing null statement is the last statement, and the
    /// statement expression is `void` (clang 22; clang 19 skipped null
    /// statements).
    fn ends_stmt_expr(&self) -> bool {
        let max = if self.s.clang_compat >= 22 { 1 } else { usize::MAX };
        let mut n = 0;
        while n < max && self.peek_is(n, Punct::Semi) {
            n += 1;
        }
        self.peek_is(n, Punct::RBrace) && self.peek_is(n + 1, Punct::RParen)
    }

    /// clang's `ExpectAndConsumeSemi`: a stray `)` or `]` before the `;`
    /// is reported (with a removal fix-it) and taken with it
    /// (`f(u<int> 7; x);`, WP-399; `a[1]];`, WP-420).
    pub(crate) fn eat_extraneous_before_semi(&mut self) -> bool {
        let Some(p) = [Punct::RParen, Punct::RSquare].into_iter().find(|&p| self.at(p)) else {
            return false;
        };
        if !self.peek_is(1, Punct::Semi) {
            return false;
        }
        let text = if p == Punct::RParen { ")" } else { "]" };
        let d = self
            .s
            .diag(DiagId::parse_extraneous_token_before_semi, self.loc())
            .arg(text)
            .fixit(kiln_diag::FixIt::remove(self.tok().span));
        self.emit(d);
        self.bump();
        self.bump();
        true
    }

    /// `break;` or `continue;` (`k`) at `loc`.
    fn parse_break_continue(&mut self, k: Kw, loc: Loc) -> StmtId {
        self.bump();
        let s = if k == Kw::Break { self.s.act_on_break(loc) } else { self.s.act_on_continue(loc) };
        self.expect_semi_after_stmt(k.spelling());
        s
    }

    /// Expects the `;` ending a `what` statement.
    pub(crate) fn expect_semi_after_stmt(&mut self, what: &str) {
        if self.eat(Punct::Semi).is_none() {
            let at = self.expected_loc();
            let d = self
                .s
                .diag(DiagId::parse_expected_semi_after_stmt, at)
                .arg(what)
                .fixit(kiln_diag::FixIt::insert(at, ";"));
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
        if self.condition_has_init_statement() {
            init = if self.at_kw(Kw::Using) {
                self.parse_alias_init_statement()
            } else {
                self.parse_statement()
            };
        }
        let Some(c) = self.parse_condition() else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some((init, c))
    }

    /// An alias-declaration as the init-statement of an `if`, `switch` or
    /// `for` ([stmt.pre]/1, C++23 P2360), at `using` (clang
    /// `ParseAliasDeclarationInInitStatement`): a C++23 extension before
    /// C++23, reported at `using` once the declaration parsed.
    fn parse_alias_init_statement(&mut self) -> Option<StmtId> {
        let loc = self.loc();
        let ds = self.parse_using();
        if ds.is_empty() {
            return None;
        }
        if self.std < Std::Cxx23 {
            let w = self.s.diag(DiagId::parse_ext_alias_in_init_statement, loc);
            self.emit(w);
        }
        Some(self.s.act_on_decl_stmt(&ds, loc))
    }

    /// Whether a `;` comes before the `)` closing a condition, skipping
    /// nested brackets and braces (clang's `SkipUntil(r_paren, semi)` in
    /// `isCXXConditionDeclarationOrInitStatement`): then the condition
    /// has an init-statement, which may hold braces (`if (int n{1}; n)`,
    /// `if (auto g = [] { return 1; }; g())`).
    fn condition_has_init_statement(&self) -> bool {
        let mut depth = 0u32;
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            match t.kind {
                TokKind::Eof => return false,
                TokKind::Punct(Punct::LParen | Punct::LSquare | Punct::LBrace) => depth += 1,
                TokKind::Punct(Punct::RParen | Punct::RSquare | Punct::RBrace) => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                TokKind::Punct(Punct::Semi) if depth == 0 => return true,
                _ => {}
            }
            i += 1;
        }
    }

    /// condition ([stmt.pre]/1): an expression, or a declaration of a
    /// single variable with a brace-or-equal-initializer. Returns the
    /// condition variable (if any) and the condition expression -- for a
    /// declaration, a reference to the variable -- not yet converted.
    fn parse_condition(&mut self) -> Option<Cond> {
        if !self.is_cxx_condition_declaration() {
            // clang 19 corrects the condition's delayed typos when it
            // finishes the condition's full-expression (WP-399).
            let typos = (self.s.clang_compat < 22).then(|| self.s.begin_delayed_typos());
            let e = self.parse_expression();
            if let Some(m) = typos {
                if let Some(e) = e {
                    self.s.note_condition_typo(e);
                }
                self.s.end_delayed_typos(m);
            }
            return e.map(|e| (None, e));
        }
        let ctx = DeclaratorContext::Condition;
        let mut ds = self.parse_decl_specifiers(ctx);
        // [stmt.pre]/1: a type-specifier-seq, so no storage class.
        self.reject_type_name_storage(&mut ds);
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
        if !d.has_name() {
            if !d.invalid {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            return None;
        }
        let name_loc = d.name_loc();
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
        // clang 19 corrects a condition declaration's delayed typos in
        // `AddInitializerToDecl`, with the initialization as the filter,
        // as for any other initializer (WP-408).
        // An undeduced variable is corrected unfiltered before deduction.
        let typos = (self.s.clang_compat < 22).then(|| self.s.begin_delayed_typos());
        self.finish_condition_decl(v, d.init_style, name_loc, typos)
    }

    /// The initializer of condition variable `v` and the reference to it;
    /// closes the compat-19 correction scope `typos`.
    fn finish_condition_decl(
        &mut self,
        v: Option<DeclId>,
        style: Option<InitStyle>,
        name_loc: Loc,
        typos: Option<usize>,
    ) -> Option<Cond> {
        let init = self.parse_initializer(style, false);
        let typos = match (typos, v) {
            (Some(m), Some(v)) if self.s.has_undeduced_type(v) => {
                self.s.flush_delayed_typos(m);
                None
            }
            (t, _) => t,
        };
        let r = match (init, v) {
            (Ok(Some(init)), Some(v)) => {
                self.s.act_on_init(v, init);
                Some(self.s.act_on_condition_var(v, name_loc))
            }
            _ => None,
        };
        if let Some(m) = typos {
            self.s.end_delayed_typos(m);
        }
        r
    }

    /// try-block ([except.pre]/1): `try compound-statement handler-seq`,
    /// at `try` (clang `ParseCXXTryBlock`).
    fn parse_try_stmt(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        if !self.at(Punct::LBrace) {
            let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("'{'");
            self.emit(d);
            self.skip_statement();
            return None;
        }
        self.parse_try_block_common(loc)
    }

    /// The compound statement and handlers of a try-block or a
    /// function-try-block (clang `ParseCXXTryBlockCommon`), at `{`.
    pub(crate) fn parse_try_block_common(&mut self, loc: Loc) -> Option<StmtId> {
        self.s.push_try_scope();
        let (block, _) = self.parse_compound(false);
        self.s.pop_scope();
        if self.at_attr() {
            // clang `DiagnoseAndSkipCXX11Attributes`.
            self.skip_attributes();
        }
        if !self.at_kw(Kw::Catch) {
            self.error_here(DiagId::parse_expected_catch);
            return None;
        }
        let mut handlers = Vec::new();
        while self.at_kw(Kw::Catch) {
            if let Some(h) = self.parse_handler() {
                handlers.push(h);
            }
        }
        if handlers.is_empty() {
            return None;
        }
        self.s.act_on_try(loc, block, &handlers)
    }

    /// handler ([except.pre]/1): `catch ( exception-declaration )
    /// compound-statement`, at `catch` (clang `ParseCXXCatchBlock`). The
    /// exception variable lives in the handler's own scope, which the
    /// compound statement's outermost block may not redeclare
    /// ([basic.scope.block]/2).
    fn parse_handler(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let Some(lparen) = self.eat(Punct::LParen) else {
            let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("'('");
            self.emit(d);
            return None;
        };
        self.s.push_catch_scope();
        let var = if self.eat(Punct::Ellipsis).is_some() {
            None
        } else {
            let mut attrs = Vec::new();
            if self.at_attr() {
                self.parse_attributes(&mut attrs);
            }
            // The leading attributes appertain to the exception variable
            // ([except.pre]/1, [dcl.pre]/5).
            self.lead_decl_attrs = attrs;
            let ctx = DeclaratorContext::Block;
            let mut ds = self.parse_decl_specifiers(ctx);
            // A type-specifier-seq: no storage class.
            self.reject_type_name_storage(&mut ds);
            let mut d = Declarator::new(ctx, self.loc());
            self.catch_declarator = true;
            self.parse_declarator(&mut d);
            self.catch_declarator = false;
            self.set_abstract_loc(&mut d);
            let base = self.s.type_from_declspec_at(&ds, d.name_loc());
            self.s.act_on_exception_decl(&ds, base, &d)
        };
        if self.eat(Punct::RParen).is_none() {
            self.expect_closing(Punct::RParen, Punct::LParen, lparen);
            if !self.at(Punct::LBrace) {
                self.s.pop_scope();
                return None;
            }
        }
        if !self.at(Punct::LBrace) {
            let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("'{'");
            self.emit(d);
            self.s.pop_scope();
            return None;
        }
        let (block, _) = self.parse_compound(true);
        self.s.pop_scope();
        Some(self.s.act_on_catch(var, block, loc))
    }

    /// if-statement ([stmt.if]): `if constexpr? ( init-statement?
    /// condition ) statement (else statement)?` and `if !? consteval
    /// compound-statement (else statement)?`.
    fn parse_if(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let constexpr_tok = self.eat_kw(Kw::Constexpr);
        let constexpr = constexpr_tok.is_some();
        if let Some(cl) = constexpr_tok
            && self.std < Std::Cxx17
        {
            // clang (ext_constexpr_if): at the `constexpr` keyword.
            let w = self.s.diag(DiagId::parse_ext_constexpr_if, cl);
            self.emit(w);
        }
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
        // An `if constexpr` condition is manifestly constant-evaluated.
        if constexpr {
            self.s.enter_constant_evaluated();
        }
        let cond = self.parse_paren_condition("if");
        if constexpr {
            self.s.exit_constant_evaluated(None);
        }
        let Some((init, (var, c))) = cond else {
            self.s.pop_scope();
            self.skip_if_rest_after_cond();
            return None;
        };
        let rparen = self.prev_end();
        let (kind, cond) = if constexpr {
            (IfKind::Constexpr, self.s.act_on_constexpr_condition(c, loc))
        } else {
            (IfKind::Ordinary, self.s.act_on_condition_at(c, loc))
        };
        // The branch a non-dependent `if constexpr` condition discards
        // is a discarded statement ([stmt.if]/2).
        let taken = if constexpr { self.s.constexpr_condition_value(cond) } else { None };
        let then = self.parse_branch(taken == Some(false));
        let els = if self.eat_kw(Kw::Else).is_some() {
            Some(self.parse_branch(taken == Some(true)))
        } else {
            None
        };
        self.s.pop_scope();
        let s = self.s.act_on_if(kind, init, var, Some(cond), then, els, loc);
        if els.is_none() {
            self.s.check_empty_body(EmptyBodyStmt::If, rparen, then);
        }
        Some(s)
    }

    /// A sub-statement of an if-statement, `discarded` or not.
    fn parse_branch(&mut self, discarded: bool) -> StmtId {
        if !discarded {
            return self.parse_sub_statement();
        }
        self.s.enter_discarded_statement();
        let s = self.parse_sub_statement();
        self.s.exit_discarded_statement();
        s
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
        let rparen = self.prev_end();
        let cond = self.s.act_on_start_switch(c, loc);
        let body = self.parse_sub_statement();
        let s = self.s.act_on_finish_switch(init, var, cond, body, loc);
        self.s.check_empty_switch_body(cond, rparen, body);
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
        // GNU `case lo ... hi:`.
        let (v, rhs) = if let Some(dots) = self.eat(Punct::Ellipsis) {
            let d = self.s.diag(DiagId::parse_gnu_case_range, dots);
            self.emit(d);
            let Some(hi) = self.parse_constant_expr() else {
                self.skip_until(&[Punct::Colon, Punct::RBrace], true, false);
                self.eat(Punct::Colon);
                return None;
            };
            let (v, hi) = self.s.act_on_case_range(v, hi, loc);
            (v, Some(hi))
        } else {
            (self.s.act_on_case_value(v, loc), None)
        };
        if self.eat(Punct::Colon).is_none() {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'case'")
                .arg("':'");
            self.emit(d);
        }
        let sub = self.parse_labeled_sub(false);
        Some(self.s.act_on_case(v, rhs, sub, loc))
    }

    /// A loop condition `( condition )` of the statement whose keyword is
    /// at `kw`; `do` takes only an expression.
    fn parse_loop_condition(&mut self, what: &str, allow_decl: bool, kw: Loc) -> Option<Cond> {
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
        Some((var, self.s.act_on_condition_at(c, kw)))
    }

    /// while-statement ([stmt.while]).
    fn parse_while(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        self.s.push_scope(ScopeKind::Control);
        let cond = self.parse_loop_condition("while", true, loc);
        let rparen = self.prev_end();
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        self.s.pop_scope();
        let (var, cond) = cond?;
        let s = self.s.act_on_while(var, cond, body, loc);
        self.s.note_loop_body(EmptyBodyStmt::While, s, rparen, body);
        Some(s)
    }

    /// do-statement ([stmt.do]).
    fn parse_do(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        // clang's `ParseDoStatement` opens a scope (it numbers the
        // implicit variables of a nested range-based for).
        self.s.push_scope(ScopeKind::Control);
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        self.s.pop_scope();
        if self.eat_kw(Kw::While).is_none() {
            self.error_here(DiagId::parse_expected_while);
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
            return None;
        }
        let cond = self.parse_loop_condition("while", false, loc);
        let Some((_, cond)) = cond else {
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
            return None;
        };
        let s = self.s.act_on_do(body, cond, loc, self.prev_lo());
        self.expect_semi_after_stmt("do/while");
        Some(s)
    }

    /// for-statement ([stmt.for]) or range-based for ([stmt.ranged]).
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
        self.s.push_scope(ScopeKind::Control);
        let r = if self.at_for_range_head() {
            self.parse_for_range(open, loc, None)
        } else {
            self.parse_for_rest(open, loc)
        };
        self.s.pop_scope();
        r
    }

    /// True if the tokens from the current one up to the next top-level
    /// `;` or the closing `)` contain a top-level `:` (not the `:` of a
    /// conditional expression): a for-range-declaration follows
    /// (clang's `ForRangeInit` detection in `ParseForStatement`).
    fn at_for_range_head(&self) -> bool {
        let mut depth = 0u32;
        let mut questions = 0u32;
        let mut i = self.pos();
        loop {
            let p = match self.tok_at(i).kind {
                TokKind::Eof => return false,
                TokKind::Punct(p) => p,
                _ => {
                    i += 1;
                    continue;
                }
            };
            match p {
                Punct::LParen | Punct::LSquare | Punct::LBrace => depth += 1,
                Punct::RParen | Punct::RSquare | Punct::RBrace => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                Punct::Semi if depth == 0 => return false,
                Punct::Question if depth == 0 => questions += 1,
                Punct::Colon if depth == 0 => {
                    if questions == 0 {
                        return true;
                    }
                    questions -= 1;
                }
                _ => {}
            }
            i += 1;
        }
    }

    /// The rest of a range-based for statement ([stmt.ranged]) after
    /// `(` and the optional init-statement `init`: the
    /// for-range-declaration, `:`, the for-range-initializer, `)` and the
    /// body. The range is parsed before the loop variable is declared,
    /// as clang does (`for (auto x : x)` names an outer `x`).
    fn parse_for_range(&mut self, open: Loc, loc: Loc, init: Option<StmtId>) -> Option<StmtId> {
        let ctx = DeclaratorContext::ForInit;
        if let Some(name) = self.ident()
            && Self::punct_of(self.peek(1)) == Some(Punct::Colon)
        {
            // `for (x : r)` (clang `isForRangeIdentifier`): diagnosed after
            // the range, then declared as `auto&& x` (clang
            // `ActOnCXXForRangeIdentifier`).
            let nloc = self.bump().span.lo;
            let colon = self.bump().span.lo;
            let range = self.parse_for_range_initializer();
            self.s.error(DiagId::parse_for_range_identifier, nloc);
            let mut ds = DeclSpec::new(nloc);
            let _ = ds.set_base(TypeSpecKind::Auto, None, nloc);
            let mut d = Declarator::new(ctx, nloc);
            d.name = Some((name, nloc));
            d.chunks.push(Chunk::RRef { loc: nloc });
            d.init_style = Some(InitStyle::C);
            let base = self.s.type_from_declspec_at(&ds, nloc);
            let v = self.s.act_on_declarator(&ds, base, &d);
            let v = v.filter(|&v| self.s.act_on_for_range_decl(&ds, v));
            return self.finish_for_range(open, loc, init, v, range, Vec::new(), colon);
        }
        let ds = self.parse_decl_specifiers(ctx);
        if ds.is_empty() {
            // `for (expr : r)`: clang `err_for_range_expected_decl` at the
            // `:` after the expression.
            let _ = self.parse_expression();
            if self.at(Punct::Colon) {
                self.error_here(DiagId::parse_for_range_expected_decl);
            } else {
                let e = self.s.diag(DiagId::parse_expected, self.loc()).arg("';'");
                self.emit(e);
            }
            self.skip_until(&[Punct::RParen], false, true);
            self.parse_dead_loop_body();
            return None;
        }
        let mut d = Declarator::new(ctx, self.loc());
        self.parse_declarator(&mut d);
        let base = self.s.type_from_declspec_at(&ds, d.name_loc());
        if self.at_attr() {
            self.parse_attributes(&mut d.attrs);
        }
        if !d.has_name() || !self.at(Punct::Colon) {
            if !d.invalid {
                if d.has_name() {
                    let e = self.s.diag(DiagId::parse_expected, self.loc()).arg("';'");
                    self.emit(e);
                } else {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                }
            }
            self.skip_until(&[Punct::RParen], false, true);
            self.parse_dead_loop_body();
            return None;
        }
        let colon = self.bump().span.lo;
        let collect = self.s.begin_for_range_init(self.s.range_lifetime_extension);
        let range = self.parse_for_range_initializer();
        let temps = self.s.end_for_range_init(collect);
        d.init_style = Some(InitStyle::C);
        let v = self.s.act_on_declarator(&ds, base, &d);
        let v = v.filter(|&v| self.s.act_on_for_range_decl(&ds, v));
        self.finish_for_range(open, loc, init, v, range, temps, colon)
    }

    /// The for-range-initializer: an expression or a braced-init-list.
    fn parse_for_range_initializer(&mut self) -> Option<RangeInit> {
        if self.at(Punct::LBrace) {
            self.parse_init_list().ok().map(RangeInit::List)
        } else {
            self.parse_expression().map(RangeInit::Expr)
        }
    }

    /// The `)` and the body of a range-based for whose loop variable `v`
    /// (`None` after an error) and range were parsed.
    #[allow(clippy::too_many_arguments, reason = "the parsed parts of the head")]
    fn finish_for_range(
        &mut self,
        open: Loc,
        loc: Loc,
        init: Option<StmtId>,
        v: Option<DeclId>,
        range: Option<RangeInit>,
        temps: Vec<ExprId>,
        colon: Loc,
    ) -> Option<StmtId> {
        let Some(range) = range else {
            if let Some(v) = v {
                self.s.act_on_for_range_error(v);
            }
            self.skip_until(&[Punct::RParen], false, true);
            self.parse_dead_loop_body();
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        let rparen = self.prev_end();
        let head = v.and_then(|v| self.s.act_on_for_range(init, v, range, temps, colon, loc));
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        let s = head.map(|h| self.s.act_on_finish_for_range(h, body));
        if s.is_some() {
            self.s.check_empty_body(EmptyBodyStmt::RangeFor, rparen, body);
        }
        s
    }

    /// expansion-statement ([stmt.expand], P1306R5): `template for (
    /// init-statement_opt for-range-declaration : expansion-initializer )
    /// compound-statement`, the `template` keyword current. Any other
    /// statement as the body is an extension (spec/sema.md §7.6a).
    fn parse_expansion_stmt(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        if self.std < Std::Cxx26 {
            let w = self.s.diag(DiagId::parse_ext_expansion_statements, loc);
            self.emit(w);
        }
        self.bump();
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
        self.s.push_scope(ScopeKind::Control);
        let r = self.parse_expansion_rest(open, loc);
        self.s.pop_scope();
        r
    }

    /// The head and body of an expansion statement after its `(`.
    fn parse_expansion_rest(&mut self, open: Loc, loc: Loc) -> Option<StmtId> {
        let init = if self.at_for_range_head() {
            None
        } else {
            let init = if self.at(Punct::Semi) {
                self.bump();
                None
            } else if self.at_kw(Kw::Using) {
                self.parse_alias_init_statement()
            } else if self.is_cxx_declaration_statement(true) {
                let l = self.loc();
                let decls = self.parse_simple_declaration(DeclaratorContext::ForInit, false);
                Some(self.s.act_on_decl_stmt(&decls, l))
            } else {
                self.parse_expression_statement(false)
            };
            if !self.at_for_range_head() {
                let e = self.s.diag(DiagId::parse_expected, self.loc()).arg("':'");
                self.emit(e);
                self.skip_until(&[Punct::RParen], false, true);
                self.parse_dead_loop_body();
                return None;
            }
            init
        };
        let ctx = DeclaratorContext::ForInit;
        let ds = self.parse_decl_specifiers(ctx);
        if ds.is_empty() {
            let _ = self.parse_expression();
            if self.at(Punct::Colon) {
                self.error_here(DiagId::parse_for_range_expected_decl);
            } else {
                let e = self.s.diag(DiagId::parse_expected, self.loc()).arg("':'");
                self.emit(e);
            }
            self.skip_until(&[Punct::RParen], false, true);
            self.parse_dead_loop_body();
            return None;
        }
        let mut d = Declarator::new(ctx, self.loc());
        self.parse_declarator(&mut d);
        let base = self.s.type_from_declspec_at(&ds, d.name_loc());
        if self.at_attr() {
            self.parse_attributes(&mut d.attrs);
        }
        if !d.has_name() || !self.at(Punct::Colon) {
            if !d.invalid {
                if d.has_name() {
                    let e = self.s.diag(DiagId::parse_expected, self.loc()).arg("':'");
                    self.emit(e);
                } else {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                }
            }
            self.skip_until(&[Punct::RParen], false, true);
            self.parse_dead_loop_body();
            return None;
        }
        let colon = self.bump().span.lo;
        // An expansion-init-list `{ a, b }` is not a braced-init-list
        // ([stmt.expand]/1): its expressions are the elements.
        let range = if self.at(Punct::LBrace) {
            self.parse_init_list().ok().map(ExpansionInit::List)
        } else {
            self.parse_expression().map(ExpansionInit::Expr)
        };
        let Some(range) = range else {
            self.skip_until(&[Punct::RParen], false, true);
            self.parse_dead_loop_body();
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        d.init_style = Some(InitStyle::C);
        let head = self.s.act_on_expansion_head(init, &ds, base, &d, range, colon, loc);
        // [stmt.expand]/1: the body is a compound-statement; any other
        // statement is accepted as an extension (clang 23
        // `ext_expansion_stmt_body_not_compound_stmt`; GCC accepts it).
        let body_loc = self.loc();
        let attr = self.at(Punct::LSquare) || self.at_kw(Kw::Attribute);
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        let mut inner = body;
        while matches!(
            self.s.ast.stmts.get(inner).kind(),
            StmtKind::LabelStmt | StmtKind::AttributedStmt
        ) {
            inner = self.s.ast.stmts.sub_stmt(inner);
        }
        let ext = if self.s.ast.stmts.get(inner).kind() != StmtKind::CompoundStmt {
            Some(DiagId::parse_ext_expansion_stmt_body_not_compound_stmt)
        } else if attr {
            Some(DiagId::parse_ext_expansion_stmt_body_attr)
        } else {
            None
        };
        if let Some(id) = ext {
            let w = self.s.diag(id, body_loc);
            self.emit(w);
        }
        head.and_then(|h| self.s.act_on_finish_expansion(h, body))
    }

    /// The body of a loop whose head was in error: analysed (clang keeps
    /// parsing it) and dropped.
    fn parse_dead_loop_body(&mut self) {
        self.s.act_on_start_loop();
        let _ = self.parse_sub_statement();
        self.s.act_on_end_loop();
    }

    fn parse_for_rest(&mut self, open: Loc, loc: Loc) -> Option<StmtId> {
        let first = self.loc();
        let init = if self.at(Punct::Semi) {
            self.bump();
            None
        } else if self.at_kw(Kw::Using) {
            self.parse_alias_init_statement()
        } else if self.is_cxx_declaration_statement(true) {
            let l = self.loc();
            let decls = self.parse_simple_declaration(DeclaratorContext::ForInit, false);
            Some(self.s.act_on_decl_stmt(&decls, l))
        } else {
            self.parse_expression_statement(false)
        };
        if self.at_for_range_head() {
            if self.std < Std::Cxx20 {
                // clang reports at the start of the init-statement.
                let w = self.s.diag(DiagId::parse_ext_for_range_init_stmt, first);
                self.emit(w);
            }
            return self.parse_for_range(open, loc, init);
        }
        let (mut var, mut cond) = (None, None);
        if !self.at(Punct::Semi) {
            let Some((v, c)) = self.parse_condition() else {
                self.skip_until(&[Punct::RParen], false, true);
                self.skip_sub_statement();
                return None;
            };
            var = v;
            cond = Some(self.s.act_on_condition_at(c, loc));
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
        let rparen = self.prev_end();
        self.s.act_on_start_loop();
        let body = self.parse_sub_statement();
        self.s.act_on_end_loop();
        let s = self.s.act_on_for(init, var, cond, inc, body, loc);
        self.s.note_loop_body(EmptyBodyStmt::For, s, rparen, body);
        Some(s)
    }

    /// `co_return expr-or-braced-init-list? ;` ([stmt.return.coroutine];
    /// clang's `ParseReturnStatement` for `kw_co_return`).
    fn parse_coreturn(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let mut value = None;
        if self.at(Punct::LBrace) {
            let Ok(l) = self.parse_init_list() else {
                self.skip_to_semi();
                return None;
            };
            value = Some(self.s.act_on_braced_arg(l));
        } else if !self.at(Punct::Semi) {
            let Some(e) = self.parse_expression() else {
                self.skip_until(&[Punct::RBrace], true, false);
                self.eat(Punct::Semi);
                return None;
            };
            value = Some(e);
        }
        let s = self.s.act_on_coreturn(value, loc);
        self.expect_semi_after_stmt("co_return");
        Some(s)
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
            // clang 19's `ActOnReturnStmt` corrects the delayed typos of
            // the operand first, before the `;` and the return's own
            // checks (`return val<int>;` with `int val(int)` in `int f()`:
            // "did you mean '::val'?", then "cannot initialize return
            // object", WP-399).
            let typos = (self.s.clang_compat < 22).then(|| self.s.begin_delayed_typos());
            let parsed = self.parse_expression();
            if let Some(m) = typos {
                self.s.end_delayed_typos(m);
            }
            let Some(e) = parsed else {
                self.skip_until(&[Punct::RBrace], true, false);
                self.eat(Punct::Semi);
                return None;
            };
            // An uncorrected speculation leaves clang an `ExprError`
            // operand: no return checks.
            value = Some(if self.s.holds_uncorrected_typo(e) {
                self.s.act_on_error_expr(&[], self.s.expr_loc(e))
            } else {
                e
            });
        }
        let s = self.s.act_on_return(value, loc);
        self.expect_semi_after_stmt("return");
        Some(s)
    }
}
