//! Contract assertions (P2900; spec/contracts.md §2-§3): the
//! *function-contract-specifier-seq* of a function declarator (`pre` /
//! `post`, contextual keywords) and the `contract_assert` statement.
//!
//! A function's seq is saved as tokens where it is written and replayed
//! once the function is declared (its return type names the result name's
//! type, [dcl.contract.res]/1); a member function's is a complete-class
//! context, replayed with its default arguments and noexcept-specifier
//! ([class.mem.general]/7). A definition whose return type is deduced
//! replays it after its body ([dcl.contract.res]/2).

use kiln_ast::stmt::ContractKind;
use kiln_ast::{DeclId, StmtId};
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std, TokKind, Token};
use kiln_sema::ContractDeclKind;
use kiln_sema::declspec::Declarator;
use kiln_source::Loc;

use crate::class::LateToks;
use crate::expr::prec;
use crate::keywords::Kw;
use crate::parser::Parser;

impl Parser<'_, '_> {
    /// True at a *function-contract-specifier*: `pre` or `post` followed
    /// by `(` or an attribute-specifier `[[` (C++26).
    pub(crate) fn at_contract_spec(&self) -> bool {
        self.std >= Std::Cxx26
            && matches!(self.tok().kind,
                TokKind::Identifier(s) if s == self.kw.pre_ || s == self.kw.post_)
            && (self.peek_is(1, Punct::LParen)
                || (self.peek_is(1, Punct::LSquare) && self.peek_is(2, Punct::LSquare)))
    }

    /// Saves the *function-contract-specifier-seq* at the current token,
    /// if any, for a replay by [`Parser::replay_contract_specs`].
    pub(crate) fn capture_contract_specs(&mut self) -> Option<LateToks> {
        if !self.at_contract_spec() {
            return None;
        }
        let mut toks: Vec<Token> = Vec::new();
        while self.at_contract_spec() {
            self.take_into(&mut toks);
            while self.at(Punct::LSquare) && self.peek_is(1, Punct::LSquare) {
                self.take_into(&mut toks);
                if !self.capture_until(&[Punct::RSquare], &mut toks, true, true) {
                    break;
                }
            }
            if !self.at(Punct::LParen) {
                break;
            }
            self.take_into(&mut toks);
            if !self.capture_until(&[Punct::RParen], &mut toks, true, true) {
                break;
            }
        }
        let end = toks.last().map_or(self.loc(), |t| t.span.hi);
        Some(LateToks { toks, end })
    }

    /// Attributes after `pre`, `post` or `contract_assert`: parsed and
    /// ignored with a warning, as GCC does (`which`: 0 `contract_assert`,
    /// 1 a function contract specifier).
    fn contract_attrs(&mut self, which: u8) {
        if !self.at_cxx11_attr() {
            return;
        }
        let at = self.loc();
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        if !attrs.is_empty() {
            let d = self.s.diag(DiagId::parse_contract_attribute_ignored, at).arg(which);
            self.emit(d);
        }
    }

    /// `( conditional-expression )` of a contract assertion opened by
    /// `sc` at `loc`.
    fn parse_contract_predicate(&mut self, sc: kiln_sema::ContractScope, loc: Loc) -> StmtId {
        let typos = self.begin_typo_correction();
        let cond = self.with_greater_op(true, |p| p.parse_expr_prec(prec::COND));
        self.correct_typos(typos, cond.is_some());
        self.s.act_on_end_contract(sc, cond, loc)
    }

    /// The *function-contract-specifier-seq* of `f` at the current token
    /// ([dcl.contract.func]/1). `definition`: `f`'s body was seen or
    /// follows.
    pub(crate) fn parse_contract_specs(
        &mut self,
        f: Option<DeclId>,
        definition: bool,
    ) -> Vec<StmtId> {
        let mut out = Vec::new();
        while self.at_contract_spec() {
            let t = self.bump();
            let loc = t.span.lo;
            let kind = if t.kind == TokKind::Identifier(self.kw.pre_) {
                ContractKind::Pre
            } else {
                ContractKind::Post
            };
            self.contract_attrs(1);
            let Some(lp) = self.eat(Punct::LParen) else {
                self.expected_here("'('");
                break;
            };
            // result-name-introducer: attributed-identifier `:`
            // ([dcl.contract.res]/1).
            let mut result = None;
            if kind == ContractKind::Post
                && let Some(name) = self.ident()
                && (self.peek_is(1, Punct::Colon)
                    || (self.peek_is(1, Punct::LSquare) && self.peek_is(2, Punct::LSquare)))
            {
                let nloc = self.bump().span.lo;
                if self.at_cxx11_attr() {
                    let mut attrs = Vec::new();
                    self.parse_attributes(&mut attrs);
                }
                if self.eat(Punct::Colon).is_some() {
                    result = Some((name, nloc));
                } else {
                    self.expected_here("':'");
                }
            }
            let sc = self.s.act_on_start_contract(kind, f, result, definition);
            let s = self.parse_contract_predicate(sc, loc);
            self.expect_closing(Punct::RParen, Punct::LParen, lp);
            out.push(s);
        }
        out
    }

    /// Replays the seq `toks` saved after the declarator `d` of `f`
    /// (a namespace-scope or block-scope declaration, or an out-of-line
    /// member definition): the declarator's parameters, its qualifier's
    /// scope and `this` are in scope, as for its trailing
    /// requires-clause. `None` toks: the declaration has no seq (still
    /// checked against the earlier ones).
    pub(crate) fn replay_contract_specs(
        &mut self,
        toks: Option<LateToks>,
        d: &Declarator,
        f: Option<DeclId>,
        how: ContractDeclKind,
    ) {
        let Some(toks) = toks else {
            if f.is_some() {
                self.s.act_on_function_contracts(f, &[], how, d.loc);
            }
            return;
        };
        let at = toks.toks.first().map_or(d.loc, |t| t.span.lo);
        self.begin_replay(toks.toks, toks.end);
        let scope = self.s.act_on_start_trailing_requires(d);
        let list = self.parse_contract_specs(f, how == ContractDeclKind::Definition);
        self.s.act_on_end_trailing_requires(scope);
        if !self.at_eof() {
            self.expected_here("'pre' or 'post'");
        }
        self.finish_replay();
        self.s.act_on_function_contracts(f, &list, how, at);
    }

    /// Replays the saved seq of member function (or friend) `f` with its
    /// parameters and `this` in scope; the caller re-entered `f`'s
    /// parameter scope ([`kiln_sema::Sema::act_on_start_delayed_method`]).
    pub(crate) fn replay_member_contract_specs(
        &mut self,
        f: DeclId,
        toks: LateToks,
        how: ContractDeclKind,
    ) {
        let at = toks.toks.first().map_or(Loc::INVALID, |t| t.span.lo);
        self.begin_replay(toks.toks, toks.end);
        let this = self.s.act_on_start_delayed_exception_spec(f);
        let list = self.parse_contract_specs(Some(f), how == ContractDeclKind::Definition);
        self.s.act_on_end_this_scope(this);
        if !self.at_eof() {
            self.expected_here("'pre' or 'post'");
        }
        self.finish_replay();
        self.s.act_on_function_contracts(Some(f), &list, how, at);
    }

    /// Replays the saved seq of the call operator `op` of the lambda
    /// whose body is open.
    pub(crate) fn replay_lambda_contract_specs(&mut self, op: DeclId, toks: LateToks) {
        let at = toks.toks.first().map_or(Loc::INVALID, |t| t.span.lo);
        self.begin_replay(toks.toks, toks.end);
        let list = self.parse_contract_specs(Some(op), true);
        if !self.at_eof() {
            self.expected_here("'pre' or 'post'");
        }
        self.finish_replay();
        self.s.act_on_function_contracts(Some(op), &list, ContractDeclKind::Definition, at);
    }

    /// `contract_assert attribute-specifier-seq? ( conditional-expression ) ;`
    /// ([stmt.contract.assert]).
    pub(crate) fn parse_contract_assert(&mut self) -> Option<StmtId> {
        debug_assert!(self.at_kw(Kw::ContractAssert));
        let loc = self.bump().span.lo;
        self.contract_attrs(0);
        let Some(lp) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.loc())
                .arg("'contract_assert'")
                .arg("'('");
            self.emit(d);
            self.skip_to_semi();
            return None;
        };
        let sc = self.s.act_on_start_contract_assert();
        let s = self.parse_contract_predicate(sc, loc);
        self.expect_closing(Punct::RParen, Punct::LParen, lp);
        self.expect_semi_after_stmt("contract_assert");
        Some(s)
    }
}
