//! Substitution into statements (spec/sema-templates.md §5.5): every
//! statement of an instantiated function body is rebuilt through the
//! actions the parser calls, in the scopes the parser opens, so that the
//! checks of the actions (case values, `break` placement, return
//! conversions, NRVO) run on the instantiation.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprDeps, ExprKind};
use kiln_ast::stmt::{IfKind, StmtKind};
use kiln_ast::{DeclId, ExprId, StmtId};
use kiln_diag::DiagId;
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
        if self.check_stack(|_| loc).is_err() || self.probe_done() {
            return self.sema.act_on_null_stmt(loc);
        }
        let (init_failures, errors) = (self.init_failures, self.sema.diags.error_count());
        let body_errors = self.sema.inst.body_errors;
        let mark = self.sema.failure_mark();
        match self.stmt_node(s) {
            // A `return` whose operand does not convert to the return type
            // (an error reported here, not in a nested instantiation) is
            // `StmtError` in clang's `BuildReturnStmt`: the body is
            // invalid, and an enclosing `if` stops at it (WP-826).
            Ok(x) if self.lambda_bodies == 0 && self.failed_return(x, errors, body_errors) => {
                self.stmt_failed = true;
                x
            }
            Ok(x) => x,
            // The failure was diagnosed (an error in the instantiation, or
            // a `RecoveryExpr` of the pattern): the statement becomes an
            // erroneous one, so a constant evaluation that reaches it fails
            // without a follow-on note (dropping it would report, e.g.,
            // "control reached end of constexpr function" for a `return`).
            Err(SubstFail) if self.sema.has_errors() => {
                // An error reported in this statement makes the body
                // invalid (clang: the statement is `StmtError`), except
                // in a declaration statement whose variable's initializer
                // failed (clang keeps the invalid variable) and in a
                // lambda body (there the lambda-expression fails, and with
                // it the enclosing statement, §5.5). A statement
                // that only names an invalid entity is a `RecoveryExpr`
                // in clang, not an error.
                let decl_init = self.sema.ast.stmts.get(s).kind() == StmtKind::DeclStmt
                    && self.init_failures != init_failures;
                // Not counting the errors of another body instantiated
                // meanwhile (`return s();` instantiating a failing `s`).
                let nested = self.sema.inst.body_errors.saturating_sub(body_errors);
                let reported = self.sema.diags.error_count().saturating_sub(errors) > nested;
                if reported && !decl_init && self.lambda_bodies == 0 {
                    self.stmt_failed = true;
                }
                // clang 22 drops a `return` whose operand is an error of
                // the pattern (its definition has no such statement): a
                // constant evaluation goes on past it (spec/sema-templates.md
                // §5.5).
                if !reported && self.sema.clang_compat >= 22 && self.erroneous_return(s) {
                    return self.sema.act_on_null_stmt(loc);
                }
                // A `return` whose operand fails without a new error (it
                // names a variable whose initializer failed, or, under
                // clang 19, is an error of the pattern) in a function whose
                // return type has a placeholder: clang's `DeduceAutoType`
                // answers `AlreadyDiagnosed` for an operand that
                // `containsErrors()` and `BuildReturnStmt` marks the
                // function invalid, so its type is not deduced `void` from
                // what is left and a call to it does not cascade. In a
                // lambda body the lambda-expression fails instead
                // (spec/sema-templates.md §5.5; WP-434).
                if !reported && !self.probe_done() && self.placeholder_return(s) {
                    if self.lambda_bodies == 0 {
                        self.stmt_failed = true;
                    } else {
                        self.lambda_return_failed = true;
                    }
                }
                let e = self.sema.recover_err(&[], loc);
                self.sema.builder().expr_stmt(e)
            }
            // Nothing diagnosed: a kiln bug, made an internal error rather
            // than missing code (§4.6 "No silent drop", WP-905). A pack
            // probe that stopped once it found its length fails quietly
            // by design.
            Err(SubstFail) => {
                if self.sema.failed_silently_since(mark) && !self.probe_done() {
                    self.stmt_dropped(loc);
                }
                self.sema.act_on_null_stmt(loc)
            }
        }
    }

    /// The internal error for a statement at `loc` that failed to
    /// instantiate with nothing diagnosed (§4.6).
    fn stmt_dropped(&mut self, loc: Loc) {
        let in_ = self.ctx_arg(self.sema.ctx);
        let d = self.sema.diag(DiagId::sema_inst_part_dropped, loc).arg("a statement").arg(in_);
        self.sema.emit(d);
    }

    /// Whether substituted statement `x` is a `return` whose operand
    /// failed to convert, with an error reported since `errors` that is
    /// not one of a nested instantiation (`body_errors` before).
    fn failed_return(&self, x: StmtId, errors: u32, body_errors: u32) -> bool {
        let ast = &self.sema.ast;
        ast.stmts.get(x).kind() == StmtKind::ReturnStmt
            && ast.stmts.return_value(x).is_some_and(|v| self.sema.is_error_expr(v))
            && self.sema.diags.error_count().saturating_sub(errors)
                > self.sema.inst.body_errors.saturating_sub(body_errors)
    }

    /// Whether pattern statement `s` is a `return` whose operand has an
    /// error of the definition (clang's `containsErrors()`).
    fn erroneous_return(&self, s: StmtId) -> bool {
        let ast = &self.sema.ast;
        ast.stmts.get(s).kind() == StmtKind::ReturnStmt
            && ast
                .stmts
                .return_value(s)
                .is_some_and(|v| ast.exprs.is_erroneous_operand(v, &ast.lists))
    }

    /// Whether pattern statement `s` is a `return` with an operand in a
    /// function (or lambda call operator) whose declared return type has
    /// a placeholder, deduced by an earlier `return` or not.
    fn placeholder_return(&self, s: StmtId) -> bool {
        let ast = &self.sema.ast;
        ast.stmts.get(s).kind() == StmtKind::ReturnStmt
            && ast.stmts.return_value(s).is_some()
            && self.sema.func.as_ref().is_some_and(|f| f.auto_ret.is_some())
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
            let decl = self.sema.ast.stmts.get(x).kind() == StmtKind::DeclStmt;
            out.push(self.stmt(x));
            if std::mem::take(&mut self.invalid_decl_stmt) && decl {
                break;
            }
        }
        let fp = self.sema.ast.stmts.compound_fp(s);
        self.sema.ast.builder().compound_fp(&out, n.loc(), rb, fp)
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
        Ok((None, self.recovering_expr(cond, loc)?))
    }

    /// The condition expression of a (not `constexpr`) `if`, `while`,
    /// `for` or `switch`, or a `for` increment. One that fails without a
    /// new error (it names a local whose initializer failed) is an
    /// erroneous expression, and the statement is still built: clang's
    /// reference to such a variable is a `RecoveryExpr`, the condition
    /// over it is valid, and the sub-statements are instantiated and
    /// diagnosed (spec/sema-templates.md §5.5, WP-826).
    fn recovering_expr(&mut self, e: Option<ExprId>, loc: Loc) -> SubstResult<Option<ExprId>> {
        let errors = self.sema.diags.error_count();
        match self.opt_expr(e) {
            Err(SubstFail)
                if self.sema.diags.error_count() == errors
                    && self.sema.has_errors()
                    && !self.sema.diags.has_fatal()
                    && !self.probe_done() =>
            {
                let l = e.map_or(loc, |e| self.sema.ast.exprs.get(e).loc());
                Ok(Some(self.sema.recover_err(&[], l)))
            }
            r => r,
        }
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
                let last = self.stmt_expr_result == Some(e);
                let e = self.expr(e)?;
                if last {
                    return Ok(self.sema.act_on_stmt_expr_result(e));
                }
                Ok(self.sema.act_on_expr_stmt(e))
            }
            StmtKind::DeclStmt => {
                let ds: Vec<DeclId> = self
                    .sema
                    .ast
                    .lists
                    .iter::<DeclId>(self.sema.ast.stmts.decl_stmt_decls(s))
                    .collect();
                // clang's `SubstDecl`: an invalid pattern declaration
                // (`Ts x;`, [temp.variadic]/12) does not instantiate, and
                // its failed statement ends the enclosing compound
                // statement (`TransformCompoundStmt`), silently.
                if ds.iter().any(|&d| self.sema.ast.decls.get(d).has(DeclFlags::INVALID)) {
                    self.failed_locals.extend_from_slice(&ds);
                    self.invalid_decl_stmt = true;
                    return Err(SubstFail);
                }
                let mut out = Vec::with_capacity(ds.len());
                for (i, &d) in ds.iter().enumerate() {
                    match self.local_decl(d) {
                        Ok(x) => out.push(x),
                        Err(f) => {
                            // This declaration and the ones after it are
                            // invalid; a later use is no new error. As
                            // clang's `TransformCompoundStmt`, a declaration
                            // that does not instantiate at all (a
                            // `static_assert` whose condition is invalid, a
                            // variable whose type does not substitute; not
                            // one with an invalid initializer) also ends the
                            // enclosing compound statement (WP-418).
                            let none = match self.sema.kind_of(d) {
                                DeclKind::StaticAssert | DeclKind::ConstevalBlock => true,
                                DeclKind::Var => !self.locals.contains_key(&d),
                                _ => false,
                            };
                            self.failed_locals.extend_from_slice(&ds[i..]);
                            self.invalid_decl_stmt |= none;
                            return Err(f);
                        }
                    }
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
                // `return {...};`: the braced list, as written, or as the
                // analyzed pattern holds it (a list-initializing
                // construction, WP-067b; a `std::initializer_list` over its
                // backing array).
                if let Some(b) = v.and_then(|v| self.braced_source(v)) {
                    let l = self.braced_list(b)?;
                    return Ok(self.sema.act_on_return_list(&l, loc));
                }
                let v = self.opt_expr(v)?;
                Ok(self.sema.act_on_return(v, loc))
            }
            StmtKind::IfStmt => self.if_stmt(s),
            StmtKind::WhileStmt => {
                let l = *self.sema.ast.stmts.loop_data(s);
                self.sema.push_scope(ScopeKind::Control);
                // The loop is open from its condition (spec/sema.md §7.4a).
                self.sema.act_on_start_loop();
                let r = (|| {
                    let (var, c) = self.condition(l.cond_var, l.cond, loc)?;
                    let c = self.sema.act_on_condition_at(c.ok_or(SubstFail)?, loc);
                    let body = l.body.map(|b| self.sub_stmt(b));
                    let body = body.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
                    Ok((var, c, body))
                })();
                self.sema.act_on_end_loop();
                self.sema.pop_scope();
                let (var, c, body) = r?;
                Ok(self.sema.act_on_while(var, c, body, loc))
            }
            StmtKind::DoStmt => {
                let l = *self.sema.ast.stmts.loop_data(s);
                // The loop is open through its condition (spec/sema.md
                // §7.4a).
                self.sema.act_on_start_loop();
                let r = (|| {
                    let body = l.body.map(|b| self.sub_stmt(b));
                    let body = body.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
                    let c = self.expr(l.cond.ok_or(SubstFail)?)?;
                    Ok((body, self.sema.act_on_condition_at(c, loc)))
                })();
                self.sema.act_on_end_loop();
                let (body, c) = r?;
                Ok(self.sema.act_on_do(body, c, loc, self.sema.ast.stmts.do_rparen(s)))
            }
            StmtKind::ForStmt => {
                let l = *self.sema.ast.stmts.loop_data(s);
                self.sema.push_scope(ScopeKind::Control);
                let init = self.opt_stmt(l.init);
                // The loop is open from its condition (spec/sema.md §7.4a).
                self.sema.act_on_start_loop();
                let r = (|| {
                    let (var, c) = self.condition(l.cond_var, l.cond, loc)?;
                    let c = c.map(|c| self.sema.act_on_condition_at(c, loc));
                    let inc = self.recovering_expr(l.inc, loc)?;
                    let body = l.body.map(|b| self.sub_stmt(b));
                    let body = body.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
                    Ok(self.sema.act_on_for(init, var, c, inc, body, loc))
                })();
                self.sema.act_on_end_loop();
                self.sema.pop_scope();
                r
            }
            StmtKind::CXXForRangeStmt => self.range_for(s),
            StmtKind::CXXExpansionStmt => self.expansion_stmt(s),
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
                let v = self.expr(c.lhs)?;
                let (v, rhs) = match c.rhs {
                    Some(hi) => {
                        let hi = self.expr(hi)?;
                        let (v, hi) = self.sema.act_on_case_range(v, hi, loc);
                        (v, Some(hi))
                    }
                    None => (self.sema.act_on_case_value(v, loc), None),
                };
                let sub = self.stmt(c.sub);
                Ok(self.sema.act_on_case(v, rhs, sub, loc))
            }
            StmtKind::DefaultStmt => {
                self.sema.act_on_default_label(loc);
                let sub = self.sema.ast.stmts.default_sub(s);
                let sub = self.stmt(sub);
                Ok(self.sema.act_on_default(sub, loc))
            }
            StmtKind::BreakStmt => Ok(self.sema.act_on_break(loc)),
            StmtKind::ContinueStmt => Ok(self.sema.act_on_continue(loc)),
            StmtKind::GCCAsmStmt => self.asm_stmt(s),
            // Labels are rebuilt by name in the instantiation (clang
            // `TransformLabelStmt` / `TransformGotoStmt`, whose
            // `TransformDecl` makes a fresh `LabelDecl`).
            StmtKind::LabelStmt => {
                let (name, at) = self.label_name(DeclId::new(n.a()))?;
                let d = self.sema.act_on_label(name, at);
                let sub = self.stmt(StmtId::new(n.b()));
                Ok(self.sema.act_on_label_stmt(d, sub, loc))
            }
            StmtKind::GotoStmt => {
                let (name, at) = self.label_name(DeclId::new(n.a()))?;
                Ok(self.sema.act_on_goto(name, at, loc))
            }
            // clang `TransformIndirectGotoStmt`: the target is substituted
            // (its conversion to `const void*` peeled) and converted again.
            StmtKind::IndirectGotoStmt => {
                let star = self.sema.ast.stmts.indirect_goto_star(s);
                let e = self.expr(self.sema.ast.stmts.indirect_goto_target(s))?;
                self.sema.act_on_indirect_goto(e, loc, star).ok_or(SubstFail)
            }
            // clang `TransformAttributedStmt`: each assumption is
            // substituted and built again (`BuildCXXAssumeExpr`, so the
            // instantiation gets its own conversion and `-Wassume`); the
            // other statement attributes (likelihood, fallthrough, a loop
            // hint without a dependent value) are kept as they are,
            // already checked in the definition.
            StmtKind::AttributedStmt => {
                use kiln_ast::ids::RawId;
                let list = self.sema.ast.stmts.stmt_attrs(s);
                let attrs: Vec<kiln_ast::AttrId> = self.sema.ast.lists.iter(list).collect();
                let mut kept = Vec::with_capacity(attrs.len());
                for a in attrs {
                    let node = *self.sema.ast.attrs.get(a);
                    // A loop hint's dependent value (clang
                    // `TransformLoopHintAttr`).
                    if node.kind == kiln_ast::attr::AttrKind::LOOP_HINT {
                        match ExprId::from_raw(node.a) {
                            Some(e) if self.sema.expr_is_value_dependent(e) => {
                                let e = self.expr(e)?;
                                kept.push(self.sema.instantiated_loop_hint(a, e));
                            }
                            _ => kept.push(a),
                        }
                        continue;
                    }
                    if node.kind != kiln_ast::attr::AttrKind::ASSUME {
                        kept.push(a);
                        continue;
                    }
                    let Some(e) = ExprId::from_raw(node.a) else { continue };
                    let e = self.expr(e)?;
                    if let Some(c) = self.sema.build_cxx_assume_expr(e) {
                        kept.push(self.sema.assume_attr(node, c));
                    }
                }
                let sub = self.stmt(StmtId::new(n.b()));
                if kept.is_empty() {
                    return Ok(sub);
                }
                // `musttail` is checked again on the instantiated return
                // (clang `checkAndRewriteMustTailAttr` from
                // `BuildAttributedStmt`; WP-809).
                for &a in &kept {
                    let node = *self.sema.ast.attrs.get(a);
                    if node.kind != kiln_ast::attr::AttrKind::MUSTTAIL
                        || self.sema.ast.stmts.get(sub).kind() != StmtKind::ReturnStmt
                    {
                        continue;
                    }
                    if !self.sema.check_musttail(sub, &node) {
                        return Ok(sub);
                    }
                    if let Some(f) = self.sema.func.as_mut() {
                        f.has_musttail = true;
                    }
                }
                Ok(self.sema.builder().attributed_stmt(&kept, sub, loc))
            }
            StmtKind::CXXTryStmt => self.try_stmt(s),
            // [stmt.return.coroutine] (clang `TransformCoreturnStmt`).
            StmtKind::CoreturnStmt => {
                let v = self.sema.ast.stmts.coreturn_operand(s);
                let v = match v.and_then(|v| self.braced_source(v)) {
                    Some(b) => {
                        let l = self.braced_list(b)?;
                        Some(self.sema.act_on_braced_arg(l))
                    }
                    None => self.opt_expr(v)?,
                };
                Ok(self.sema.build_coreturn(v, loc))
            }
            // The user's body again: the instantiation builds its own
            // coroutine parts at its end.
            StmtKind::CoroutineBodyStmt => {
                let b = self.sema.ast.stmts.coroutine_body(s).body;
                Ok(self.stmt(b))
            }
            StmtKind::ContractAssertStmt => self.contract_stmt(s, None),
            k @ StmtKind::CXXCatchStmt => {
                self.unimplemented(&format!("substitution into {k:?}"), loc)
            }
        }
    }

    /// A try-block (clang `TransformCXXTryStmt`): the compound statement
    /// in a try scope, each handler, then `act_on_try`, whose handler-order
    /// checks run on the instantiated types.
    fn try_stmt(&mut self, s: StmtId) -> SubstResult<StmtId> {
        let loc = self.sema.ast.stmts.get(s).loc();
        let block = self.sema.ast.stmts.try_block(s);
        let hs: Vec<StmtId> =
            self.sema.ast.lists.iter::<StmtId>(self.sema.ast.stmts.try_handlers(s)).collect();
        self.sema.push_try_scope();
        let block = self.body(block);
        self.sema.pop_scope();
        let mut handlers = Vec::with_capacity(hs.len());
        for h in hs {
            handlers.push(self.handler(h)?);
        }
        self.sema.act_on_try(loc, block, &handlers).ok_or(SubstFail)
    }

    /// A handler (clang `TransformCXXCatchStmt`): the exception
    /// declaration is rebuilt from its substituted type through
    /// `act_on_exception_decl` (decay, the incomplete-type and abstract
    /// checks, the copy-initialization from the exception object), in the
    /// handler's scope; a pattern whose declaration is invalid fails.
    fn handler(&mut self, h: StmtId) -> SubstResult<StmtId> {
        let loc = self.sema.ast.stmts.get(h).loc();
        let var = self.sema.ast.stmts.catch_var(h);
        let block = self.sema.ast.stmts.catch_block(h);
        self.sema.push_scope(ScopeKind::Control);
        let v = match var {
            Some(d) => match self.exception_decl(d) {
                Ok(v) => v,
                Err(e) => {
                    self.sema.pop_scope();
                    return Err(e);
                }
            },
            None => None,
        };
        let block = self.stmt(block);
        self.sema.pop_scope();
        Ok(self.sema.act_on_catch(v, block, loc))
    }

    fn exception_decl(&mut self, d: DeclId) -> SubstResult<Option<DeclId>> {
        use crate::declspec::{DeclSpec, Declarator, TypeSpecKind};
        use kiln_ast::decl::DeclFlags;
        let n = *self.sema.ast.decls.get(d);
        if n.has(DeclFlags::INVALID) {
            return Err(SubstFail);
        }
        let loc = n.loc();
        let ty = self.named_decl_ty(d, self.decl_ty(d), loc)?;
        let type_loc = self.sema.handler_type_locs.get(&d).copied().unwrap_or(loc);
        let mut ds = DeclSpec::new(type_loc);
        ds.base = TypeSpecKind::Named;
        ds.named = Some(ty);
        ds.type_loc = Some(type_loc);
        let mut dr = Declarator::new(DeclaratorContext::Block, loc);
        if let Some(name) = n.name().as_ident()
            && !n.name().is_empty()
        {
            dr.name = Some((name, loc));
        }
        let v = self.sema.act_on_exception_decl(&ds, ty, &dr);
        if let Some(v) = v {
            self.locals.insert(d, super::DeclOrPack::Decl(v));
        }
        Ok(v)
    }

    /// A GNU asm statement (clang `TransformGCCAsmStmt`): the operands
    /// are substituted and the statement is checked again.
    fn asm_stmt(&mut self, s: StmtId) -> SubstResult<StmtId> {
        let loc = self.sema.ast.stmts.get(s).loc();
        let data = *self.sema.ast.stmts.asm_data(s);
        let bits = self.sema.ast.stmts.asm_bits(s);
        let ops = self.sema.ast.stmts.asm_operands(s).to_vec();
        let mut new = Vec::with_capacity(ops.len());
        for op in ops {
            let expr = self.expr(op.expr)?;
            new.push(kiln_ast::stmt::AsmOperand { expr, ..op });
        }
        let clobbers: Vec<_> = self
            .sema
            .ast
            .lists
            .iter::<kiln_ast::StringLitId>(data.clobbers)
            .map(|c| (c, data.string_loc))
            .collect();
        let (outs, ins) = new.split_at(data.num_outputs as usize);
        let string = (data.string, data.string_loc);
        let pattern: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(data.labels).collect();
        let mut labels: Vec<(DeclId, Loc)> = Vec::with_capacity(pattern.len());
        for l in pattern {
            let (name, at) = self.label_name(l)?;
            labels.push((self.sema.act_on_asm_goto_label(name, at), at));
        }
        self.sema
            .act_on_gcc_asm_goto_stmt(loc, bits, string, outs, ins, &clobbers, &labels, data.rparen)
            .ok_or(SubstFail)
    }

    /// The name and location of the pattern's label declaration `l`.
    fn label_name(&self, l: DeclId) -> SubstResult<(kiln_source::symbol::Symbol, Loc)> {
        let n = self.sema.ast.decls.get(l);
        Ok((n.name().as_ident().ok_or(SubstFail)?, n.loc()))
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
            // [stmt.if]/4: the branch taken in a constant evaluation is an
            // immediate function context ([expr.const]/16).
            let sub = |me: &mut Self, st: Option<StmtId>, immediate: bool| {
                st.map(|x| {
                    if immediate {
                        me.sema.enter_immediate_context();
                    }
                    let r = me.sub_stmt(x);
                    if immediate {
                        me.sema.exit_immediate_context();
                    }
                    r
                })
            };
            let positive = kind == IfKind::Consteval;
            let then = sub(self, d.then, positive);
            let then = then.unwrap_or_else(|| self.sema.act_on_null_stmt(loc));
            let els = sub(self, d.els, !positive);
            return Ok(self.sema.act_on_if(kind, None, None, None, then, els, loc));
        }
        self.sema.push_scope(ScopeKind::Control);
        let r = (|| {
            let init = self.opt_stmt(d.init);
            // clang's `constexpr` condition over a `RecoveryExpr` is not a
            // constant expression: the statement fails, its branches are
            // not instantiated.
            let (var, c) = if kind == IfKind::Constexpr && d.cond_var.is_none() {
                (None, self.opt_expr(d.cond)?)
            } else {
                self.condition(d.cond_var, d.cond, loc)?
            };
            let c = c.ok_or(SubstFail)?;
            if kind == IfKind::Constexpr {
                let c = self.sema.act_on_constexpr_condition(c, loc);
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
                // clang keeps a discarded branch as an empty compound
                // over the written branch's begin and end, which the
                // coverage mapping's skipped region spans (WP-1083).
                let empty = |me: &mut Self, at: Option<StmtId>| {
                    let (l, r) = at.map_or((loc, loc), |x| {
                        (me.sema.ast.stmts.get(x).loc(), me.sema.uninit_stmt_end(x))
                    });
                    me.sema.act_on_compound(&[], l, r)
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
            let c = self.sema.act_on_condition_at(c, loc);
            // clang's `TransformIfStmt` stops at a `then` branch that is
            // `StmtError`: the `else` branch of a statement whose `then`
            // branch failed is not instantiated (WP-826).
            let failed = std::mem::take(&mut self.stmt_failed);
            let then = match d.then {
                Some(t) => self.sub_stmt(t),
                None => self.sema.act_on_null_stmt(loc),
            };
            let then_failed = self.stmt_failed;
            self.stmt_failed |= failed;
            if then_failed {
                return Err(SubstFail);
            }
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
            // `for (x : {a, b})`: the braced list, which the analyzed
            // pattern holds as a `std::initializer_list` over its backing
            // array once it is not type-dependent.
            let range = if let Some(b) = self.braced_source(range_init) {
                self.braced_list(b).map(crate::RangeInit::List)
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
            self.remap_binding_pack(lv);
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
        let dname = self.sema.ast.decls.get(d).name();
        let pnns = self.sema.ast.using_qualifier(d).ok_or(SubstFail)?;
        // Qualifier errors and the qualifier's notes are located where
        // the nested-name-specifier begins, not at the name.
        let qloc = self.qualifier_begin(pnns, loc);
        let nns = self.nns(pnns, qloc)?;
        // The `using` keyword, where the declaration's notes point.
        let using_loc = self.sema.ast.decls.cold.begin_loc.get(d).copied().unwrap_or(loc);
        if self.nns_still_dependent(nns) {
            // Still dependent (a generic lambda's body inside a function
            // template specialization): the local stays unresolved, its
            // uses dependent, until the lambda's own instantiation. (An
            // operator name never has a dependent block-scope declarator.)
            let name = dname.as_ident().ok_or(SubstFail)?;
            let h = kiln_ast::builder::DeclHead::new(self.sema.ctx, dname, loc);
            let x = self.sema.builder().unresolved_using_value(h, nns, false);
            if using_loc != loc {
                let _ = self.sema.ast.decls.cold.begin_loc.insert(x, using_loc);
            }
            self.sema.push_local(name, x);
            return Ok(x);
        }
        let ctx = self.nns_context(nns, qloc)?;
        // A using-declaration whose lookup finds nothing survives in the
        // pattern only with `using_if_exists` (the qualifier is not
        // dependent, so the lookup is the definition's). A dependent one
        // (`UnresolvedUsingValue`) is checked now.
        let if_exists = self.sema.kind_of(d) == DeclKind::Using
            && self.sema.lookup_qualified_dn(ctx, dname, crate::lookup::LookupKind::All).is_empty();
        let q = crate::using::Qualifier { ctx, nns, loc: qloc };
        let r = self.sema.act_on_using_declarator(using_loc, Some(q), dname, loc, if_exists, None);
        r.ok_or(SubstFail)
    }

    /// A declaration of a declaration statement.
    fn local_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        match self.sema.kind_of(d) {
            DeclKind::Var if self.sema.ast.decls.decomp_bindings(d).is_some() => {
                self.decomp_decl(d, DeclaratorContext::Block)
            }
            DeclKind::Var if self.is_anonymous_object(d) => self.local_anonymous_object(d),
            DeclKind::Var => self.var_decl(d, DeclaratorContext::Block),
            DeclKind::Typedef | DeclKind::TypeAlias => {
                let x = self.typedef_decl_in(d, DeclaratorContext::Block)?;
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            DeclKind::StaticAssert => self.static_assert_decl(d).map(|x| x.0),
            DeclKind::ConstevalBlock => self.consteval_block_decl(d).map(|x| x.0),
            // GNU `__label__ name;`: redeclared in the instantiation's
            // block, where the substituted `goto`s and labels find it by
            // name (clang `VisitLabelDecl`).
            DeclKind::Label => {
                let (name, at) = self.label_name(d)?;
                Ok(self.sema.act_on_local_label(name, at))
            }
            DeclKind::Using | DeclKind::UnresolvedUsingValue => self.local_using_decl(d, loc),
            // A block-scope using-directive ([namespace.udir]): the
            // nominated namespace is never dependent; redone in the
            // instantiation's block scope for its dependent lookups.
            DeclKind::UsingDirective => {
                let ns = self.sema.ast.using_target(d).ok_or(SubstFail)?;
                Ok(self.sema.act_on_using_directive(ns, loc))
            }
            // A block-scope using-enum-declaration ([enum.udecl]; clang
            // `TemplateDeclInstantiator::VisitUsingEnumDecl`): the named
            // enumeration is mapped to the instantiation's (a local or
            // member enumeration of the pattern) and its enumerators are
            // redeclared in the instantiation's block scope.
            DeclKind::UsingEnum => {
                let e = self.sema.ast.using_target(d).ok_or(SubstFail)?;
                let e = self.find_instantiated_decl(e, loc)?;
                let written = self.sema.decl_name(e);
                Ok(self.sema.using_enum_of(e, &written, loc))
            }
            DeclKind::CXXRecord => Ok(self.local_class(d, None)),
            DeclKind::Enum => {
                let x = self.enum_decl(d)?;
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            // A block-scope function declaration ([basic.scope.block],
            // [temp.inst]/3): declared again with the substituted
            // signature in the instantiation's block, where a local
            // class's friend finds it ([class.friend]/11).
            DeclKind::Function => {
                let x = self.function_decl(d, DeclaratorContext::Block, None)?;
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            // A block-scope namespace alias ([namespace.alias]; clang
            // `TemplateDeclInstantiator::VisitNamespaceAliasDecl`): its
            // target is never dependent, so the alias is redeclared in the
            // instantiation's block scope with the same (or the mapped
            // local alias) target.
            DeclKind::NamespaceAlias => {
                let dname = self.sema.ast.decls.get(d).name();
                let name = dname.as_ident().ok_or(SubstFail)?;
                let t = self.sema.ast.using_target(d).ok_or(SubstFail)?;
                let t = match self.locals.get(&t) {
                    Some(super::DeclOrPack::Decl(x)) => *x,
                    _ => t,
                };
                let h = kiln_ast::builder::DeclHead::new(self.sema.ctx, dname, loc);
                let x = self.sema.builder().namespace_alias(h, t, None);
                self.sema.push_local(name, x);
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            k => self.unimplemented(&format!("local {k:?} declarations in templates"), loc),
        }
    }
}
