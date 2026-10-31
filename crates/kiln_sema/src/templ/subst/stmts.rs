//! Substitution into statements (spec/sema-templates.md §5.5): every
//! statement of an instantiated function body is rebuilt through the
//! actions the parser calls, in the scopes the parser opens, so that the
//! checks of the actions (case values, `break` placement, return
//! conversions, NRVO) run on the instantiation.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{ExprDeps, ExprKind};
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
        if self.check_stack(|_| loc).is_err() {
            return self.sema.act_on_null_stmt(loc);
        }
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
            let v2 = if self.sema.ast.decls.decomp_bindings(v).is_some() {
                self.decomp_decl(v, DeclaratorContext::Condition)?
            } else {
                self.var_decl(v, DeclaratorContext::Condition)?
            };
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
                // `return {...};` of a class type: a list-initializing
                // construction (WP-067b) goes back to its braced list.
                if let Some(v) = v {
                    let c = self.peel_initializer(v);
                    let ex = &self.sema.ast.exprs;
                    if ex.get(c).kind() == ExprKind::CXXConstructExpr && ex.construct_info(c).list {
                        let at = ex.get(c).loc();
                        let end = ex.end_loc(c).unwrap_or(at);
                        let l = self.construct_list(c, at, end)?;
                        return Ok(self.sema.act_on_return_list(&l, loc));
                    }
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
            StmtKind::CXXForRangeStmt => self.range_for(s),
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
                if self.sema.ast.exprs.get(c).kind() != ExprKind::ConstantExpr {
                    // Still value-dependent (an outer level is not yet
                    // substituted): both branches are kept, as written.
                    if self.sema.ast.exprs.get(c).deps().intersects(ExprDeps::VALUE) {
                        let then = match d.then {
                            Some(t) => self.sub_stmt(t),
                            None => self.sema.act_on_null_stmt(loc),
                        };
                        let els = d.els.map(|e| self.sub_stmt(e));
                        return Ok(self.sema.act_on_if(kind, init, var, Some(c), then, els, loc));
                    }
                    // Invalid (diagnosed).
                    return Err(SubstFail);
                }
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

    /// A range-based `for` ([stmt.ranged]; clang
    /// `TransformCXXForRangeStmt`): the for-range-initializer and the
    /// loop variable are substituted, and the desugaring is rebuilt
    /// (spec/sema-templates.md §5.5).
    fn range_for(&mut self, s: StmtId) -> SubstResult<StmtId> {
        let d = *self.sema.ast.stmts.range_for_data(s);
        let loc = self.sema.ast.stmts.get(s).loc();
        let range_init = self.sema.ast.decls.var(d.range).and_then(|v| v.init).ok_or(SubstFail)?;
        let range_name = self.sema.ast.decls.get(d.range).name().as_ident().ok_or(SubstFail)?;
        // `__begin`/`__end` keep the pattern's numbering when the pattern
        // has them, else they are numbered where the instantiation runs.
        let depth = match d.begin {
            Some(b) => {
                let n = self.sema.ast.decls.get(b).name();
                let digits = n.as_ident().map(|x| self.sema.ast.symbols().as_str(x).to_owned());
                digits
                    .and_then(|x| x.strip_prefix("__begin").and_then(|n| n.parse().ok()))
                    .unwrap_or(0)
            }
            None => self.sema.range_for_inst_depth(),
        };
        self.sema.push_scope(ScopeKind::Control);
        let r = (|| {
            let init = self.opt_stmt(d.init);
            let collect = self.sema.begin_for_range_init(self.sema.range_lifetime_extension);
            let range = if self.sema.ast.exprs.get(range_init).kind() == ExprKind::InitListExpr {
                self.init_list(range_init).map(crate::RangeInit::List)
            } else {
                self.expr(range_init).map(crate::RangeInit::Expr)
            };
            let temps = self.sema.end_for_range_init(collect);
            let range = range?;
            let lv = d.loop_var;
            let (ds, v) = if self.sema.ast.decls.decomp_bindings(lv).is_some() {
                self.decomp_declare(lv, DeclaratorContext::ForInit)?
            } else {
                let vloc = self.sema.ast.decls.get(lv).loc();
                let t = self.decl_ty(lv);
                let ty = self.named_decl_ty(lv, t, vloc)?;
                let ds = self.pattern_declspec(lv, ty);
                let dr = self.pattern_declarator(lv, DeclaratorContext::ForInit)?;
                let Some(v) = self.sema.act_on_declarator(&ds, ty, &dr) else {
                    return Err(SubstFail);
                };
                (ds, v)
            };
            self.locals.insert(lv, super::DeclOrPack::Decl(v));
            if !self.sema.act_on_for_range_decl(&ds, v) {
                return Err(SubstFail);
            }
            let head = self
                .sema
                .instantiate_for_range(init, range_name, range, temps, v, d.colon, loc, depth);
            self.sema.act_on_start_loop();
            let body = self.sub_stmt(d.body);
            self.sema.act_on_end_loop();
            let head = head.ok_or(SubstFail)?;
            Ok(self.sema.act_on_finish_for_range(head, body))
        })();
        self.sema.pop_scope();
        r
    }

    /// A block-scope using-declaration ([namespace.udecl]; clang
    /// `TemplateDeclInstantiator::VisitUsingDecl`): the qualifier is
    /// substituted and the declarator is redone in the instantiation's
    /// block scope, introducing its shadows there.
    fn local_using_decl(&mut self, d: DeclId, loc: Loc) -> SubstResult<DeclId> {
        let name = self.sema.ast.decls.get(d).name().as_ident().ok_or(SubstFail)?;
        let nns = self.sema.ast.using_qualifier(d).ok_or(SubstFail)?;
        let nns = self.nns(nns, loc)?;
        if self.nns_still_dependent(nns) {
            return self.unimplemented("dependent using-declarations in templates", loc);
        }
        let ctx = self.nns_context(nns, loc)?;
        // A using-declaration whose lookup finds nothing survives in the
        // pattern only with `using_if_exists` (the qualifier is not
        // dependent, so the lookup is the definition's).
        let if_exists =
            self.sema.lookup_qualified(ctx, name, crate::lookup::LookupKind::All).is_empty();
        let q = crate::using::Qualifier { ctx, nns, loc };
        self.sema.act_on_using_declarator(loc, Some(q), name, loc, if_exists).ok_or(SubstFail)
    }

    /// A declaration of a declaration statement.
    fn local_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        match self.sema.kind_of(d) {
            DeclKind::Var if self.sema.ast.decls.decomp_bindings(d).is_some() => {
                self.decomp_decl(d, DeclaratorContext::Block)
            }
            DeclKind::Var => self.var_decl(d, DeclaratorContext::Block),
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let x = self.typedef_decl_in(d, DeclaratorContext::Block)?;
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            DeclKind::StaticAssert => self.static_assert_decl(d),
            DeclKind::Using => self.local_using_decl(d, loc),
            k => self.unimplemented(&format!("local {k:?} declarations in templates"), loc),
        }
    }
}
