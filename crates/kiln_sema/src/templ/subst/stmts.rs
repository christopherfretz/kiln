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
        if self.check_stack(|_| loc).is_err() || self.probe_done() {
            return self.sema.act_on_null_stmt(loc);
        }
        let (init_failures, errors) = (self.init_failures, self.sema.diags.error_count());
        let body_errors = self.sema.inst.body_errors;
        match self.stmt_node(s) {
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
                // lambda body (clang rebuilds the lambda). A statement
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
                let e = self.sema.recover_err(&[], loc);
                self.sema.builder().expr_stmt(e)
            }
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
            // clang `TransformAttributedStmt`: each assumption is
            // substituted and built again (`BuildCXXAssumeExpr`, so the
            // instantiation gets its own conversion and `-Wassume`); the
            // other statement attributes (likelihood, fallthrough) are
            // kept as they are, already checked in the definition.
            StmtKind::AttributedStmt => {
                use kiln_ast::ids::RawId;
                let list = self.sema.ast.stmts.stmt_attrs(s);
                let attrs: Vec<kiln_ast::AttrId> = self.sema.ast.lists.iter(list).collect();
                let mut kept = Vec::with_capacity(attrs.len());
                for a in attrs {
                    let node = *self.sema.ast.attrs.get(a);
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
                Ok(self.sema.builder().attributed_stmt(&kept, sub, loc))
            }
            StmtKind::CXXTryStmt => self.try_stmt(s),
            k => self.unimplemented(&format!("substitution into {k:?}"), loc),
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
        let name = dname.as_ident().ok_or(SubstFail)?;
        let nns = self.sema.ast.using_qualifier(d).ok_or(SubstFail)?;
        let nns = self.nns(nns, loc)?;
        // The `using` keyword, where the declaration's notes point.
        let using_loc = self.sema.ast.decls.cold.begin_loc.get(d).copied().unwrap_or(loc);
        if self.nns_still_dependent(nns) {
            // Still dependent (a generic lambda's body inside a function
            // template specialization): the local stays unresolved, its
            // uses dependent, until the lambda's own instantiation.
            let h = kiln_ast::builder::DeclHead::new(self.sema.ctx, dname, loc);
            let x = self.sema.builder().unresolved_using_value(h, nns, false);
            if using_loc != loc {
                let _ = self.sema.ast.decls.cold.begin_loc.insert(x, using_loc);
            }
            self.sema.push_local(name, x);
            return Ok(x);
        }
        let ctx = self.nns_context(nns, loc)?;
        // A using-declaration whose lookup finds nothing survives in the
        // pattern only with `using_if_exists` (the qualifier is not
        // dependent, so the lookup is the definition's). A dependent one
        // (`UnresolvedUsingValue`) is checked now.
        let if_exists = self.sema.kind_of(d) == DeclKind::Using
            && self.sema.lookup_qualified(ctx, name, crate::lookup::LookupKind::All).is_empty();
        let q = crate::using::Qualifier { ctx, nns, loc };
        let r = self.sema.act_on_using_declarator(using_loc, Some(q), dname, loc, if_exists, None);
        if r.is_none() && self.sema.kind_of(d) == DeclKind::UnresolvedUsingValue {
            self.failed_usings.push(d);
        }
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
            DeclKind::Using | DeclKind::UnresolvedUsingValue => self.local_using_decl(d, loc),
            // A block-scope using-directive ([namespace.udir]): the
            // nominated namespace is never dependent; redone in the
            // instantiation's block scope for its dependent lookups.
            DeclKind::UsingDirective => {
                let ns = self.sema.ast.using_target(d).ok_or(SubstFail)?;
                Ok(self.sema.act_on_using_directive(ns, loc))
            }
            DeclKind::CXXRecord => Ok(self.local_class(d, None)),
            DeclKind::Enum => {
                let x = self.enum_decl(d)?;
                self.locals.insert(d, super::DeclOrPack::Decl(x));
                Ok(x)
            }
            k => self.unimplemented(&format!("local {k:?} declarations in templates"), loc),
        }
    }
}
