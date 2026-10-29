//! Substitution into statements (spec/sema-templates.md §5.5): every
//! statement of an instantiated function body is rebuilt through the
//! actions the parser calls, in the scopes the parser opens, so that the
//! checks of the actions (case values, `break` placement, return
//! conversions, NRVO) run on the instantiation.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_source::Loc;

use super::{SubstFail, SubstResult, Substituter};
use crate::declspec::DeclaratorContext;
use crate::scope::ScopeKind;

impl Substituter<'_, '_, '_> {
    /// Substitutes into statement `s`. A statement that fails is replaced
    /// by a null statement (the error is diagnosed) so the rest of the
    /// body is still checked, as clang does.
    pub fn stmt(&mut self, s: StmtId) -> StmtId {
        let loc = self.sema.ast.stmts.get(s).loc();
        match self.stmt_node(s) {
            Ok(x) => x,
            Err(SubstFail) => self.sema.act_on_null_stmt(loc),
        }
    }

    /// A sub-statement: a statement that is not compound gets a block
    /// scope of its own ([stmt.pre]/2).
    fn sub_stmt(&mut self, s: StmtId) -> StmtId {
        let compound = self.sema.ast.stmts.get(s).kind() == StmtKind::CompoundStmt;
        if !compound {
            self.sema.push_scope(ScopeKind::Block);
        }
        let r = self.stmt(s);
        if !compound {
            self.sema.pop_scope();
        }
        r
    }

    /// The body of a function: a compound statement whose scope is the
    /// function's own (opened by `act_on_start_function_def`).
    pub(crate) fn body(&mut self, s: StmtId) -> StmtId {
        let n = *self.sema.ast.stmts.get(s);
        if n.kind() != StmtKind::CompoundStmt {
            return self.stmt(s);
        }
        let items: Vec<StmtId> =
            self.sema.ast.lists.iter::<StmtId>(self.sema.ast.stmts.compound_body(s)).collect();
        let rb = self.sema.ast.stmts.rbrace(s);
        let mut out = Vec::with_capacity(items.len());
        for x in items {
            out.push(self.stmt(x));
        }
        self.sema.act_on_compound(&out, n.loc(), rb)
    }

    fn opt_stmt(&mut self, s: Option<StmtId>) -> Option<StmtId> {
        s.map(|s| self.stmt(s))
    }

    /// A condition: a condition variable, or an expression.
    fn condition(
        &mut self,
        var: Option<DeclId>,
        cond: Option<ExprId>,
        loc: Loc,
    ) -> SubstResult<(Option<DeclId>, Option<ExprId>)> {
        if let Some(v) = var {
            let v2 = self.var_decl(v, DeclaratorContext::Condition)?;
            let (v3, c) = self.sema.act_on_condition_var(v2, loc);
            return Ok((v3, Some(c)));
        }
        Ok((None, self.opt_expr(cond)?))
    }

    #[allow(clippy::too_many_lines, reason = "one flat match over the statement kinds")]
    fn stmt_node(&mut self, s: StmtId) -> SubstResult<StmtId> {
        let n = *self.sema.ast.stmts.get(s);
        let loc = n.loc();
        match n.kind() {
            StmtKind::CompoundStmt => {
                self.sema.push_scope(ScopeKind::Block);
                let r = self.body(s);
                self.sema.pop_scope();
                Ok(r)
            }
            StmtKind::NullStmt => Ok(self.sema.act_on_null_stmt(loc)),
            StmtKind::ExprStmt => {
                let e = self.sema.ast.stmts.expr_stmt_expr(s);
                let e = self.expr(e)?;
                Ok(self.sema.act_on_expr_stmt(e))
            }
            StmtKind::DeclStmt => {
                let ds: Vec<DeclId> = self
                    .sema
                    .ast
                    .lists
                    .iter::<DeclId>(self.sema.ast.stmts.decl_stmt_decls(s))
                    .collect();
                let mut out = Vec::with_capacity(ds.len());
                for &d in &ds {
                    out.push(self.local_decl(d)?);
                }
                if out.len() > 1 {
                    // [dcl.spec.auto.general]/8 with the substituted
                    // initializers' deductions.
                    let items: Vec<_> = ds
                        .iter()
                        .zip(&out)
                        .map(|(&d, &v)| (v, self.sema.group_placeholder(d)))
                        .collect();
                    self.sema.check_auto_group(&items);
                }
                Ok(self.sema.act_on_decl_stmt(&out, loc))
            }
            StmtKind::ReturnStmt => {
                let v = self.sema.ast.stmts.return_value(s);
                if let Some(v) = v
                    && self.sema.ast.exprs.get(v).kind() == ExprKind::InitListExpr
                {
                    let l = self.init_list(v)?;
                    return Ok(self.sema.act_on_return_list(&l, loc));
                }
                let v = self.opt_expr(v)?;
                Ok(self.sema.act_on_return(v, loc))
            }
            StmtKind::IfStmt => self.if_stmt(s),
            StmtKind::WhileStmt => {
                let l = *self.sema.ast.stmts.loop_data(s);
                self.sema.push_scope(ScopeKind::Control);
                let r = (|| {
                    let (var, c) = self.condition(l.cond_var, l.cond, loc)?;
                    let c = self.sema.act_on_condition(c.ok_or(SubstFail)?);
                    self.sema.act_on_start_loop();
                    let body = l.body.map(|b| self.sub_stmt(b));
                    self.sema.act_on_end_loop();
                    let body = body.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
                    Ok(self.sema.act_on_while(var, c, body, loc))
                })();
                self.sema.pop_scope();
                r
            }
            StmtKind::DoStmt => {
                let l = *self.sema.ast.stmts.loop_data(s);
                self.sema.act_on_start_loop();
                let body = l.body.map(|b| self.sub_stmt(b));
                self.sema.act_on_end_loop();
                let body = body.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
                let c = self.expr(l.cond.ok_or(SubstFail)?)?;
                let c = self.sema.act_on_condition(c);
                Ok(self.sema.act_on_do(body, c, loc))
            }
            StmtKind::ForStmt => {
                let l = *self.sema.ast.stmts.loop_data(s);
                self.sema.push_scope(ScopeKind::Control);
                let r = (|| {
                    let init = self.opt_stmt(l.init);
                    let (var, c) = self.condition(l.cond_var, l.cond, loc)?;
                    let c = c.map(|c| self.sema.act_on_condition(c));
                    let inc = self.opt_expr(l.inc)?;
                    self.sema.act_on_start_loop();
                    let body = l.body.map(|b| self.sub_stmt(b));
                    self.sema.act_on_end_loop();
                    let body = body.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
                    Ok(self.sema.act_on_for(init, var, c, inc, body, loc))
                })();
                self.sema.pop_scope();
                r
            }
            StmtKind::SwitchStmt => {
                let sw = *self.sema.ast.stmts.switch_data(s);
                self.sema.push_scope(ScopeKind::Control);
                let r = (|| {
                    let init = self.opt_stmt(sw.init);
                    let (var, c) = self.condition(sw.cond_var, Some(sw.cond), loc)?;
                    let c = self.sema.act_on_start_switch(c.ok_or(SubstFail)?, loc);
                    let body = self.sub_stmt(sw.body);
                    Ok(self.sema.act_on_finish_switch(init, var, c, body, loc))
                })();
                self.sema.pop_scope();
                r
            }
            StmtKind::CaseStmt => {
                let c = *self.sema.ast.stmts.case_data(s);
                if c.rhs.is_some() {
                    return self.unimplemented("case ranges in templates", loc);
                }
                let v = self.expr(c.lhs)?;
                let v = self.sema.act_on_case_value(v, loc);
                let sub = self.stmt(c.sub);
                Ok(self.sema.act_on_case(v, sub, loc))
            }
            StmtKind::DefaultStmt => {
                self.sema.act_on_default_label(loc);
                let sub = self.sema.ast.stmts.default_sub(s);
                let sub = self.stmt(sub);
                Ok(self.sema.act_on_default(sub, loc))
            }
            StmtKind::BreakStmt => Ok(self.sema.act_on_break(loc)),
            StmtKind::ContinueStmt => Ok(self.sema.act_on_continue(loc)),
            k => self.unimplemented(&format!("substitution into {k:?}"), loc),
        }
    }

    /// `if` ([stmt.if]); for `if constexpr` only the taken branch is
    /// instantiated, the other becomes an empty compound statement as in
    /// clang (§5.5, [stmt.if]/2).
    fn if_stmt(&mut self, s: StmtId) -> SubstResult<StmtId> {
        let n = *self.sema.ast.stmts.get(s);
        let loc = n.loc();
        let kind = self.sema.ast.stmts.if_kind(s);
        let d = *self.sema.ast.stmts.if_data(s);
        if matches!(kind, IfKind::Consteval | IfKind::NotConsteval) {
            return self.unimplemented("if consteval in templates", loc);
        }
        self.sema.push_scope(ScopeKind::Control);
        let r = (|| {
            let init = self.opt_stmt(d.init);
            let (var, c) = self.condition(d.cond_var, d.cond, loc)?;
            let c = c.ok_or(SubstFail)?;
            if kind == IfKind::Constexpr {
                let c = self.sema.act_on_constexpr_condition(c);
                let taken = self
                    .sema
                    .ast
                    .exprs
                    .constant_value(c)
                    .and_then(|v| self.sema.const_int(v))
                    .map(|v| v != 0);
                let Some(taken) = taken else { return Err(SubstFail) };
                let empty = |me: &mut Self, at: Option<StmtId>| {
                    let l = at.map_or(loc, |x| me.sema.ast.stmts.get(x).loc());
                    me.sema.act_on_compound(&[], l, l)
                };
                let then = match d.then {
                    Some(t) if taken => self.sub_stmt(t),
                    t => empty(self, t),
                };
                let els = match d.els {
                    Some(e) if !taken => Some(self.sub_stmt(e)),
                    Some(e) => Some(empty(self, Some(e))),
                    None => None,
                };
                return Ok(self.sema.act_on_if(kind, init, var, Some(c), then, els, loc));
            }
            let c = self.sema.act_on_condition(c);
            let then = match d.then {
                Some(t) => self.sub_stmt(t),
                None => self.sema.act_on_null_stmt(loc),
            };
            let els = d.els.map(|e| self.sub_stmt(e));
            Ok(self.sema.act_on_if(kind, init, var, Some(c), then, els, loc))
        })();
        self.sema.pop_scope();
        r
    }

    /// A declaration of a declaration statement.
    fn local_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        match self.sema.kind_of(d) {
            DeclKind::Var => self.var_decl(d, DeclaratorContext::Block),
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let x = self.typedef_decl_in(d, DeclaratorContext::Block)?;
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            DeclKind::StaticAssert => self.static_assert_decl(d),
            k => self.unimplemented(&format!("local {k:?} declarations in templates"), loc),
        }
    }
}
