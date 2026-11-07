//! Substitution into expressions (spec/sema-templates.md §5.4): an
//! expression that is not type-, value- or instantiation-dependent and
//! has no unexpanded pack is shared with the pattern; otherwise the
//! implicit nodes semantic analysis added are stripped and the
//! expression is rebuilt through the Sema actions the parser calls.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{
    BinaryOp, CastKind, ExprDeps, ExprFlags, ExprKind, ExprNode, TypeidOperand, ValueCategory,
};
use kiln_ast::ids::{ArgListId, RawId};
use kiln_ast::names::{DeclName, DeclNameView};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{ArgView, BuiltinType, Nns, TemplateArg};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::{SubstFail, SubstResult, Substituter};
use crate::expr::CastStyle;
use crate::init::{InitItem, InitList, InitValue, Initializer};
use crate::templ::args::ArgLookup;

impl Substituter<'_, '_, '_> {
    /// Whether `e` must be rebuilt (§5.4). An expression with no type,
    /// value or instantiation dependence (references to decls of a
    /// dependent context are instantiation-dependent) and no unexpanded
    /// pack is shared with the pattern by id; this is the substituter's
    /// base case, so leaves need no rebuild arm (§3.3, §14 Q3).
    pub(crate) fn needs_rebuild(&self, e: ExprId) -> bool {
        let deps = self.sema.ast.exprs.get(e).deps();
        deps.intersects(ExprDeps::TYPE_VALUE_INST | ExprDeps::UNEXPANDED_PACK)
    }

    /// Substitutes into the unevaluated operand `e` of `sizeof`,
    /// `alignof`, `noexcept` or an expression trait ([expr.context]/1):
    /// rebuilt inside an unevaluated context, as the parser builds it, so
    /// the functions it names are not odr-used (and not instantiated).
    pub(crate) fn unevaluated_operand(&mut self, e: ExprId) -> SubstResult<ExprId> {
        self.sema.enter_unevaluated();
        let r = self.expr(e);
        self.sema.exit_unevaluated();
        r
    }

    /// Substitutes into `e`.
    pub fn expr(&mut self, e: ExprId) -> SubstResult<ExprId> {
        // A non-dependent overloaded member access (`M(1).f` called with
        // dependent arguments) is formed again too: resolving its call
        // consumes the overload set it records, so the pattern's node
        // cannot be shared between specializations (clang's
        // `TransformUnresolvedMemberExpr`).
        if !self.needs_rebuild(e)
            && self.sema.member_overload(e).is_none()
            && !self.dealiased_in_expansion(e)
        {
            if self.lambda_bodies > 0 {
                self.note_shared_captures(e);
            }
            self.sema.mark_shared_expr_referenced(e);
            self.sema.note_shared_consteval_only(e);
            self.sema.note_shared_throws(e);
            self.sema.reissue_designator_warnings(e);
            self.sema.note_shared_temporaries(e);
            return Ok(e);
        }
        self.check_stack(|s| s.sema.ast.exprs.get(e).loc())?;
        self.sema.subst_expr_depth += 1;
        let r = self.expr_node(e);
        self.sema.subst_expr_depth -= 1;
        let r = r?;
        self.sema.note_inst_consteval_only(e, r);
        if self.sema.is_err_expr(r) {
            return Err(SubstFail);
        }
        self.keep_template_kw(e, r);
        Ok(r)
    }

    /// A name written `N::template x<A>` that is still unresolved after
    /// substitution keeps its `template` keyword (clang's
    /// `TransformDependentScopeDeclRefExpr` / `TransformUnresolvedLookupExpr`
    /// pass the `TemplateKWLoc` on); a resolved `DeclRefExpr` does not
    /// (WP-308).
    fn keep_template_kw(&mut self, e: ExprId, r: ExprId) {
        if r != e
            && let Some(&kw) = self.sema.ast.templates.template_kw.get(e)
            && matches!(
                self.sema.ast.exprs.get(r).kind(),
                ExprKind::UnresolvedLookupExpr | ExprKind::DependentScopeDeclRefExpr
            )
        {
            let _ = self.sema.ast.templates.template_kw.insert(r, kw);
        }
    }

    /// Substitutes into an optional expression.
    pub fn opt_expr(&mut self, e: Option<ExprId>) -> SubstResult<Option<ExprId>> {
        e.map(|e| self.expr(e)).transpose()
    }

    /// Substitutes into the expressions of list `l` (default arguments
    /// of a call are dropped: the rebuilt call adds its own).
    fn expr_list(&mut self, l: kiln_ast::ids::ListId) -> SubstResult<Vec<ExprId>> {
        let mut v: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
        if let Some(i) =
            v.iter().position(|&x| self.sema.ast.exprs.get(x).kind() == ExprKind::CXXDefaultArgExpr)
        {
            v.truncate(i);
        }
        self.expand_exprs(&v)
    }

    /// The arguments of a call: a braced-init-list argument becomes a
    /// braced argument again ([`crate::Sema::act_on_braced_arg`]), which
    /// the call list-initializes its parameter from (a dependent call
    /// keeps it as written, [`crate::Sema::dependent_init_list`]).
    fn call_arg_list(&mut self, l: kiln_ast::ids::ListId) -> SubstResult<Vec<ExprId>> {
        let v: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
        if !v.iter().any(|&x| self.braced_source(x).is_some() || self.braced_pattern(x).is_some()) {
            return self.expr_list(l);
        }
        self.braced_args(v)
    }

    /// The rebuild of [`Self::call_arg_list`] for an argument list holding a
    /// braced argument (or a braced pack expansion), up to the first
    /// `CXXDefaultArgExpr`.
    fn braced_args(&mut self, v: Vec<ExprId>) -> SubstResult<Vec<ExprId>> {
        let mut out = Vec::with_capacity(v.len());
        for x in v {
            if self.sema.ast.exprs.get(x).kind() == ExprKind::CXXDefaultArgExpr {
                break;
            }
            if let Some(p) = self.braced_pattern(x) {
                let ellipsis = self.sema.ast.exprs.get(x).loc();
                match self.expand_pattern_at(Some(ellipsis), |s| s.init_list(p))? {
                    crate::templ::pack::Expansion::Elements(ls) => {
                        out.extend(ls.into_iter().map(|il| self.sema.act_on_braced_arg(il)));
                    }
                    crate::templ::pack::Expansion::Unexpanded(il) => {
                        let e = self.sema.act_on_braced_arg(il);
                        out.push(self.sema.act_pack_expansion(e, ellipsis));
                    }
                }
                continue;
            }
            match self.braced_source(x) {
                Some(b) => {
                    let il = self.braced_list(b)?;
                    out.push(self.sema.act_on_braced_arg(il));
                }
                None => out.extend(self.expand_exprs(&[x])?),
            }
        }
        Ok(out)
    }

    /// The braced-init-list pattern of `x` when `x` is the pack expansion
    /// `{a, b}...` ([temp.variadic]/5.4: the pattern of an
    /// initializer-list expansion is an *initializer-clause*).
    fn braced_pattern(&self, x: ExprId) -> Option<ExprId> {
        crate::templ::pack::expr_pattern(self.sema, x)
            .filter(|&p| self.sema.ast.exprs.get(p).kind() == ExprKind::InitListExpr)
    }

    /// The braced-init-list an analyzed operand `x` was initialized from
    /// ([dcl.init.list]/1), seen through the implicit nodes semantic
    /// analysis added: an `InitListExpr`, the backing array of a
    /// `std::initializer_list` (`CXXStdInitializerListExpr`,
    /// [dcl.init.list]/5) or a list-initializing `CXXConstructExpr`. The
    /// rebuilt operand is that list again, initialized anew (clang
    /// `TransformInitializer` reverting to the syntactic form).
    pub(crate) fn braced_source(&self, mut x: ExprId) -> Option<ExprId> {
        let ex = &self.sema.ast.exprs;
        loop {
            match ex.get(x).kind() {
                ExprKind::ExprWithCleanups
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::ImplicitCastExpr => x = ex.operand(x),
                ExprKind::CXXStdInitializerListExpr => x = ex.operand(ex.operand(x)),
                ExprKind::InitListExpr => return Some(x),
                ExprKind::CXXConstructExpr if ex.construct_info(x).list => return Some(x),
                _ => return None,
            }
        }
    }

    /// The braced-init-list of [`Self::braced_source`] result `b`.
    pub(crate) fn braced_list(&mut self, b: ExprId) -> SubstResult<InitList> {
        let ex = &self.sema.ast.exprs;
        if ex.get(b).kind() == ExprKind::CXXConstructExpr {
            let at = ex.get(b).loc();
            let end = ex.end_loc(b).unwrap_or(at);
            return self.construct_list(b, at, end);
        }
        self.init_list(b)
    }

    /// A parenthesized aggregate initialization `T(a, b)` ([dcl.init.general]
    /// /16.6.2.2, P0960) as an operand: formed again as the explicit type
    /// conversion of `ty` from the substituted explicit initializers
    /// ([expr.type.conv]/2), which redoes the initialization (the
    /// resolved one is not reused: an instantiation-dependent operand may
    /// convert differently).
    fn paren_list_init(
        &mut self,
        p: ExprId,
        ty: QualType,
        loc: Loc,
        end: Loc,
    ) -> SubstResult<ExprId> {
        let args = self.call_arg_list(self.sema.ast.exprs.paren_list_inits(p))?;
        Ok(match args.as_slice() {
            [a] if !self.sema.is_braced_arg(*a) => {
                self.sema.act_on_cast(CastStyle::Functional, ty, *a, loc, end)
            }
            _ => self.sema.act_on_paren_list_cast(ty, &args, loc, end),
        })
    }

    #[allow(clippy::too_many_lines, reason = "one arm per expression kind, as TreeTransform")]
    fn expr_node(&mut self, e: ExprId) -> SubstResult<ExprId> {
        fn ex<'x>(s: &'x Substituter<'_, '_, '_>) -> &'x kiln_ast::expr::ExprArena {
            &s.sema.ast.exprs
        }
        let n = *self.sema.ast.exprs.get(e);
        let loc = n.loc();
        let end = self.sema.ast.exprs.end_loc(e).unwrap_or(loc);
        let k = n.kind();
        match k {
            // An implicit user-defined conversion through a conversion
            // function is redone from its object (clang
            // `CastExpr::getSubExprAsWritten`): the call names a
            // specialization of a conversion function template by the
            // pattern's `operator T`, whose `T` is the member template's
            // own parameter, not the enclosing instantiation's
            // ([temp.deduct.conv]; WP-563).
            ExprKind::ImplicitCastExpr if let Some(o) = self.udc_object(e) => self.expr(o),
            // Implicit nodes are rebuilt by the actions (§5.4).
            ExprKind::ImplicitCastExpr
            | ExprKind::ConstantExpr
            | ExprKind::MaterializeTemporaryExpr
            | ExprKind::CXXBindTemporaryExpr
            | ExprKind::ExprWithCleanups => {
                let o = ex(self).operand(e);
                self.expr(o)
            }
            ExprKind::SubstNonTypeTemplateParmExpr => {
                let r = ex(self).subst_replacement(e);
                self.expr(r)
            }
            ExprKind::DeclRefExpr => {
                let d = ex(self).decl(e);
                if self.sema.kind_of(d) == DeclKind::NonTypeTemplateParm {
                    return self.nttp_ref(e, d, loc);
                }
                if let Some(r) = self.kept_parm_pack(e, d, loc) {
                    return Ok(r);
                }
                if let Some(q) = ex(self).decl_ref_extra(e).and_then(|x| x.qualifier)
                    && self.in_dependent_specialization(d, loc)?
                {
                    // `X<A...>::name` with the substituted class still
                    // dependent: looked up again later.
                    let nns = self.nns(q, loc)?;
                    let name = self.sema.ast.decls.get(d).name();
                    let begin = self.sema.ast.templates.expr_begin.get(e).copied().unwrap_or(loc);
                    return Ok(self.sema.act_dependent_scope_ref(nns, name, None, begin, loc));
                }
                if let Some(r) = self.respecialized_member_template(d, loc)? {
                    return Ok(r);
                }
                let i = self.find_instantiated_decl(d, loc)?;
                // The qualifier as written survives ([temp.inst]; clang's
                // `TransformDeclRefExpr` transforms the qualifier loc).
                let nns = match ex(self).decl_ref_extra(e).and_then(|x| x.qualifier) {
                    Some(q) => Some(self.nns(q, loc)?),
                    None => None,
                };
                let r = self.sema.build_decl_ref_qualified(i, loc, None, nns);
                if n.has(ExprFlags::IMMEDIATE_ESCALATING)
                    && self.sema.ast.exprs.get(r).kind() == ExprKind::DeclRefExpr
                {
                    self.sema.ast.exprs.set_immediate_escalating(r);
                }
                if nns.is_some()
                    && let Some(b) = self.sema.ast.templates.expr_begin.get(e).copied()
                    && self.sema.ast.exprs.get(r).kind() == ExprKind::DeclRefExpr
                {
                    let _ = self.sema.ast.templates.expr_begin.insert(r, b);
                }
                Ok(r)
            }
            // Named in the instantiated context (clang
            // `TransformPredefinedExpr`).
            ExprKind::SourceLocExpr => {
                let kind = ex(self).source_loc_kind(e);
                Ok(self.sema.act_on_source_loc_expr(kind, loc, end))
            }
            ExprKind::PredefinedExpr => {
                let kind = ex(self).predefined_kind(e);
                Ok(self.sema.act_on_predefined_expr(kind, loc))
            }
            ExprKind::ParenExpr => {
                let o = ex(self).operand(e);
                let o = self.expr(o)?;
                Ok(self.sema.act_on_paren(o, loc, end))
            }
            ExprKind::UnaryOperator => {
                let op = ex(self).unary_op(e);
                let o = ex(self).operand(e);
                // `&C::m` stays the qualified name, not `this->C::m`
                // (`Sema::act_on_id_expression_in`).
                let addr = op == kiln_ast::expr::UnaryOp::AddrOf
                    && self.sema.ast.exprs.get(o).kind() == ExprKind::DeclRefExpr
                    && ex(self).decl_ref_extra(o).is_some_and(|x| x.qualifier.is_some());
                let saved = std::mem::replace(&mut self.sema.address_operand, addr);
                let o = self.expr(o);
                self.sema.address_operand = saved;
                let o = o?;
                Ok(self.sema.act_on_unary(op, o, loc))
            }
            ExprKind::BinaryOperator => {
                let op = ex(self).binary_op(e);
                let [l, r] = ex(self).operands(e);
                self.binary(op, l, r, loc)
            }
            ExprKind::CompoundAssignOperator => {
                let op = ex(self).binary_op(e);
                let l = ExprId::new(n.a());
                let r = ex(self).compound_assign(e).rhs;
                self.binary(op, l, r, loc)
            }
            ExprKind::ConditionalOperator => {
                let c = ExprId::new(n.a());
                let [t, f] = ex(self).cond_arms(e);
                let c = self.expr(c)?;
                let t = self.expr(t)?;
                let f = self.expr(f)?;
                Ok(self.sema.act_on_conditional(c, t, f, loc))
            }
            // GNU `x ?: y` (clang `TransformBinaryConditionalOperator`):
            // the common operand and the false arm, built again.
            ExprKind::BinaryConditionalOperator => {
                let [common, _, _, f] =
                    self.sema.ast.exprs.binary_conditional_operands(e, &self.sema.ast.lists);
                let common = self.expr(common)?;
                let f = self.expr(f)?;
                Ok(self.sema.act_on_binary_conditional(common, f, loc, loc))
            }
            // GNU `({ ... })` (clang `TransformStmtExpr`): the body is
            // rebuilt in a block scope of its own and its last expression
            // statement yields the value again.
            ExprKind::StmtExpr => {
                let body = ex(self).stmt_expr_body(e);
                let rparen = self.sema.ast.exprs.end_loc(e).unwrap_or(loc);
                let scope = self.sema.act_on_start_stmt_expr();
                let result = self.sema.stmt_expr_result(body);
                let saved = std::mem::replace(&mut self.stmt_expr_result, result);
                self.sema.push_scope(crate::scope::ScopeKind::Block);
                let b = self.body(body);
                self.sema.pop_scope();
                self.stmt_expr_result = saved;
                Ok(self.sema.act_on_stmt_expr(scope, b, loc, rparen))
            }
            // GNU `&&label` (clang `TransformAddrLabelExpr`): the label is
            // looked up again by name in the instantiated body, without the
            // parser's extension warning.
            ExprKind::AddrLabelExpr => {
                let l = ex(self).addr_label(e);
                let name = self.sema.ast.decls.get(l).name().as_ident().ok_or(SubstFail)?;
                let at = self.sema.ast.exprs.end_loc(e).unwrap_or(loc);
                let l = self.sema.act_on_asm_goto_label(name, at);
                let ty = self.sema.ast.types.pointer_to(QualType::VOID);
                Ok(self.sema.builder().addr_label(l, ty, loc, at))
            }
            ExprKind::ArraySubscriptExpr => {
                let [b, i] = ex(self).operands(e);
                let b = self.expr(b)?;
                let i = self.expr(i)?;
                let r = self.sema.act_on_subscript(b, i, loc);
                if let Some(end) = self.sema.ast.exprs.end_loc(e) {
                    self.sema.builder().set_subscript_end(r, end);
                }
                Ok(r)
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => {
                let callee = ex(self).callee(e);
                let args = ex(self).call_args(e);
                let callee = self.callee(callee)?;
                let args = self.call_arg_list(args)?;
                // A call of an explicit object member function (`o.f(x)`
                // built as `f(o, x)`, P0847) is rebuilt as one.
                let named = self.sema.ignore_parens(callee);
                if ex(self).get(named).kind() == ExprKind::DeclRefExpr
                    && !args.is_empty()
                    && self.sema.has_explicit_object(ex(self).decl(named))
                {
                    let m = ex(self).decl(named);
                    let at = ex(self).get(named).loc();
                    let lparen = self.sema.begin_loc(callee);
                    return Ok(self.sema.rebuild_explicit_object_call(m, at, &args, lparen, end));
                }
                // clang `TransformCallExpr`: the "fake" `(` is where the
                // callee begins.
                let lparen = self.sema.begin_loc(callee);
                Ok(self.sema.act_on_call(callee, &args, lparen, end))
            }
            ExprKind::AtomicExpr => {
                // clang `TransformAtomicExpr`: the operands in source
                // order, rebuilt (and re-checked) as the builtin call.
                let op = ex(self).atomic_op(e);
                let subs: Vec<ExprId> =
                    self.sema.ast.lists.iter::<ExprId>(ex(self).atomic_subs(e)).collect();
                let Some(form) = kiln_ast::decl::builtin::atomic::form(op) else {
                    return self.unimplemented("this atomic operation", loc);
                };
                let mut args = Vec::with_capacity(subs.len());
                for &s in form.arg_to_sub() {
                    args.push(self.expr(subs[s])?);
                }
                match self.sema.builtin_atomic_op(loc, op, &args, end) {
                    Some(r) => Ok(r),
                    None => Err(SubstFail),
                }
            }
            ExprKind::CXXOperatorCallExpr => {
                use crate::overload::oper::{binary_of, unary_of};
                use kiln_ast::names::OverloadedOperator as OO;
                let op = ex(self).operator_call_op(e);
                let args = ex(self).call_args(e);
                // `x = {…}` through an assignment operator function
                // ([expr.assign]/8, [over.match.oper]): the list is
                // assigned again.
                if let Some(b) = binary_of(op).filter(|b| b.is_assignment())
                    && let [l, r] = self.sema.ast.lists.iter::<ExprId>(args).collect::<Vec<_>>()[..]
                    && let Some(src) = self.braced_source(r)
                {
                    let l = self.expr(l)?;
                    let il = self.braced_list(src)?;
                    return Ok(self.sema.act_on_assign_list(b, l, &il, loc));
                }
                // A braced argument of `f(…)` or `a[…]` ([expr.call]/7,
                // [over.sub]) is a braced argument again.
                let args = if matches!(op, OO::Call | OO::Subscript) {
                    self.call_arg_list(args)?
                } else {
                    self.expr_list(args)?
                };
                match (op, args.as_slice()) {
                    (OO::Call, [f, rest @ ..]) => {
                        let lparen = self.sema.begin_loc(*f);
                        Ok(self.sema.act_on_call(*f, rest, lparen, end))
                    }
                    (OO::Subscript, [b, rest @ ..]) => {
                        Ok(self.sema.act_on_subscript_args(*b, rest, loc, end))
                    }
                    // One link of a `->` chain outside a member access
                    // (the access itself re-forms the whole chain from the
                    // class object, [over.ref]/1): `operator->` selected
                    // again for the substituted object.
                    (OO::Arrow, &[a]) => self.sema.rebuild_operator_arrow(a, loc).ok_or(SubstFail),
                    (_, &[a]) | (OO::PlusPlus | OO::MinusMinus, &[a, _]) => {
                        match unary_of(op, args.len() == 2) {
                            Some(u) => Ok(self.sema.act_on_unary(u, a, loc)),
                            None => self.unimplemented("this overloaded operator", loc),
                        }
                    }
                    // Every binary operator function, `->*` and `,`
                    // included, names a built-in binary operator.
                    (_, &[l, r]) => match binary_of(op) {
                        Some(b) => Ok(self.sema.act_on_binary(b, l, r, loc)),
                        None => self.unimplemented("this overloaded operator", loc),
                    },
                    // Not reached by a resolved call: `new`/`delete`
                    // operator functions are never a `CXXOperatorCallExpr`
                    // (a new/delete-expression, or a call `operator new(n)`
                    // of a named function), and the `operator co_await` call
                    // of a `CoawaitExpr` is formed again from the operand as
                    // written ([expr.await]/3.3).
                    _ => self.unimplemented("this overloaded operator", loc),
                }
            }
            ExprKind::CXXRewrittenBinaryOperator => {
                let (op, l) = ex(self).rewritten_written(e);
                let v = self.expr_list(l)?;
                match v.as_slice() {
                    &[l, r] => Ok(self.sema.act_on_binary(op, l, r, loc)),
                    _ => self.unimplemented("this rewritten comparison", loc),
                }
            }
            ExprKind::UnresolvedLookupExpr if self.sema.var_ttp_ref(e).is_some() => {
                // `V<args>` with `V` a variable template template
                // parameter (P2841R7).
                let p = self.sema.var_ttp_ref(e).ok_or(SubstFail)?;
                self.ttp_template_id(e, p, loc).unwrap_or(Err(SubstFail))
            }
            ExprKind::UnresolvedLookupExpr => match self.local_using_ref(e, false) {
                Some(r) => Ok(r),
                // A function template-id not called (`&f<T>`, `auto *p =
                // f<T>;`) names its one specialization once the arguments
                // are substituted, as the parser resolves a non-dependent
                // one ([over.over]/3 via [temp.arg.explicit]/4).
                None if self.sema.ast.templates.expr_targs.get(e).is_some() => {
                    let r = self.callee(e)?;
                    Ok(self.sema.resolve_single_template_id(r))
                }
                None => self.callee(e),
            },
            ExprKind::MemberExpr => {
                let base = ex(self).operand(e);
                let m = ex(self).decl(e);
                let arrow = n.has(ExprFlags::ARROW);
                // `x->m` through `operator->`: the chain is formed again
                // from the substituted `x` ([over.ref]/1).
                let base = if arrow { self.sema.arrow_chain_object(base) } else { base };
                let (base, this_at) = self.member_base_deferred(base)?;
                let name = self.sema.ast.decls.get(m).name();
                if name.is_empty() && self.sema.kind_of(m) == DeclKind::Field {
                    // The unnamed object of an anonymous struct or union
                    // (`x.u` through an injected `IndirectField`): it has
                    // no name to look up.
                    let f = self.find_instantiated_decl(m, loc)?;
                    let bt = self.sema.ety(base);
                    let ot = if arrow { self.sema.pointee_sugared(bt).unwrap_or(bt) } else { bt };
                    let q = self.sema.ast.types.effective_quals(ot);
                    self.note_deferred_this(None, this_at);
                    return Ok(self.sema.field_access(base, f, q, arrow, loc));
                }
                if self.sema.kind_of(m) == DeclKind::CXXDestructor {
                    // `p->~C()` / `p->B::~B()` resolved in the pattern
                    // ([expr.ref]/4, [class.dtor]/16): formed again from
                    // the substituted object as a destructor access to
                    // the (instantiated) class the destructor belongs to.
                    let Some(cls) = self.sema.ast.decls.get(m).ctx() else {
                        return Err(SubstFail);
                    };
                    let cls = self.find_instantiated_decl(cls, loc)?;
                    let ty = self.sema.tag_type(cls);
                    let qualified = ex(self).decl_ref_extra(e).and_then(|x| x.qualifier).is_some();
                    let op = self.sema.ast.templates.member_op_loc.get(e).copied().unwrap_or(loc);
                    self.note_deferred_this(None, this_at);
                    return Ok(self
                        .sema
                        .rebuild_member_destructor(base, ty, qualified, loc, op, arrow));
                }
                let name = self.member_name(name, loc)?;
                // A member template-id `x.f<args>` (§6.4): the arguments
                // as written, kept on the resolved access too (the pattern
                // may have selected the specialization at the definition,
                // `throw_error<X>()` through an implicit `this` in a class
                // template). Arguments the pattern only deduced are not
                // written again: with them every member template of the
                // name would take them explicitly, and a substitution
                // failure in the overload set would become a hard error
                // (`ss.str("")` naming libc++'s `str<char[1]>(const
                // SAlloc&)` would instantiate `basic_string<char, traits,
                // char[1]>` for `str(const basic_string<C, T, SAlloc>&)`;
                // clang `TreeTransform::TransformMemberExpr` passes the
                // explicit arguments only when written).
                let written = ex(self).decl_ref_extra(e).and_then(|x| x.template_args);
                let targs = match self.sema.ovl.members.get(&e).and_then(|m| m.targs).or(written) {
                    Some(l) => Some(self.args(l, loc)?),
                    None => None,
                };
                // `this->B::f()` keeps its qualifier (no virtual call).
                let q = ex(self)
                    .decl_ref_extra(e)
                    .and_then(|x| x.qualifier)
                    .or_else(|| self.sema.ovl.members.get(&e).and_then(|m| m.nns));
                let qual = match q {
                    Some(q) => Some(self.member_qualifier(q, base, arrow, loc)?),
                    None => None,
                };
                let op = self.sema.ast.templates.member_op_loc.get(e).copied().unwrap_or(loc);
                self.sema.pattern_member_access = true;
                let r = self
                    .sema
                    .act_on_template_member_q(base, qual, name, targs, false, loc, op, arrow);
                self.sema.pattern_member_access = false;
                self.note_deferred_this(Some(r), this_at);
                Ok(r)
            }
            ExprKind::CXXPseudoDestructorExpr => {
                let (base, arrow, destroyed) = ex(self).pseudo_destructor(e);
                let qualified = ex(self).pseudo_destructor_qualified(e);
                let base = self.member_base(base)?;
                let op = self.sema.ast.templates.member_op_loc.get(e).copied().unwrap_or(loc);
                // `p->~N()` whose `N` named no type at the definition: it
                // is looked up now, in the object's class (clang
                // `TransformCXXPseudoDestructorExpr` → `getDestructorName`).
                if let Some(&name) = self.sema.ast.templates.pseudo_dtor_name.get(e) {
                    return Ok(self.sema.act_on_member_destructor_late(base, name, loc, op, arrow));
                }
                let destroyed = self.ty(destroyed, loc)?;
                Ok(self.sema.rebuild_member_destructor(base, destroyed, qualified, loc, op, arrow))
            }
            ExprKind::CXXDependentScopeMemberExpr => {
                let Some(base) = ExprId::from_raw(n.a()) else {
                    return self.implicit_dependent_member(e, loc);
                };
                let name = self.sema.ast.dependent_member_name(e);
                let name = self.member_name(name, loc)?;
                let arrow = n.aux() & 1 != 0;
                let (base, this_at) = self.member_base_deferred(base)?;
                let qual = match self.sema.ast.templates.expr_nns.get(e).copied() {
                    Some(q) => {
                        let at = self.sema.ast.templates.expr_begin.get(e).copied().unwrap_or(loc);
                        Some(self.member_qualifier(q, base, arrow, at)?)
                    }
                    None => None,
                };
                let targs = match self.sema.ast.templates.expr_targs.get(e).copied() {
                    Some(l) => Some(self.args(l, loc)?),
                    None => None,
                };
                let kw = n.aux() & 2 != 0;
                let op = self.sema.ast.templates.member_op_loc.get(e).copied().unwrap_or(loc);
                let r = self.sema.act_on_template_member_q(base, qual, name, targs, kw, loc, op, arrow);
                self.note_deferred_this(Some(r), this_at);
                Ok(r)
            }
            ExprKind::DependentScopeDeclRefExpr => {
                let callee = std::mem::take(&mut self.as_callee);
                let Some(nns) = kiln_ast::ids::NnsId::try_new(n.a()) else {
                    return Err(SubstFail);
                };
                let name = self.sema.ast.dependent_scope_ref_name(e);
                let name = self.member_name(name, loc)?;
                let begin = self.sema.ast.templates.expr_begin.get(e).copied().unwrap_or(loc);
                let nns = self.nns(nns, begin)?;
                if self.nns_still_dependent(nns) {
                    // Substituted with dependent arguments (an alias
                    // template named in a template): still dependent.
                    let targs = match self.sema.ast.templates.expr_targs.get(e).copied() {
                        Some(l) => Some(self.args(l, loc)?),
                        None => None,
                    };
                    return Ok(self.sema.act_dependent_scope_ref(nns, name, targs, begin, loc));
                }
                // `T::template x<args>` ([temp.names]/5, /6): the member
                // template the name finds after substitution.
                if let Some(l) = self.sema.ast.templates.expr_targs.get(e).copied() {
                    let Some(name) = name.as_ident() else {
                        return self.unimplemented("an operator function template-id", loc);
                    };
                    let t = self.member_template_named(nns, name, loc)?;
                    let l = self.args(l, loc)?;
                    let args = self.sema.ast.types.args(l).to_vec();
                    let tn = self.sema.template_name_for(t);
                    return match self.sema.act_template_id(tn, &args, loc, loc) {
                        Ok(crate::templ::actions::TemplateIdRef::Value(x)) => {
                            // Printed with its substituted qualifier.
                            self.sema.set_template_id_begin(x, begin, loc);
                            self.sema.set_template_id_qualifier(x, nns);
                            Ok(x)
                        }
                        // Not called, a function template-id names its
                        // one specialization, as for an unresolved
                        // lookup (above): `return D<I>::template f<F>;`
                        // with a deduced return type (libc++ variant's
                        // `__make_dispatch`, [over.over]/3,
                        // [temp.arg.explicit]/4).
                        Ok(crate::templ::actions::TemplateIdRef::Overload(x)) => {
                            // With the other overloads of the name and its
                            // qualifier, as the parser forms `C::f<A>`.
                            let ctx = self.nns_context(nns, begin)?;
                            let x = self.sema.widen_function_template_id(x, name, Some(ctx));
                            self.sema.set_template_id_begin(x, begin, loc);
                            self.sema.set_template_id_qualifier(x, nns);
                            Ok(if callee { x } else { self.sema.resolve_single_template_id(x) })
                        }
                        Ok(crate::templ::actions::TemplateIdRef::Type(_)) => {
                            self.unimplemented("a type template-id as an expression", loc)
                        }
                        Err(()) => Err(SubstFail),
                    };
                }
                let ctx = self.nns_context(nns, begin)?;
                let q = crate::using::Qualifier { ctx, nns, loc: begin };
                Ok(self.sema.act_on_id_expression_dn(name, loc, Some(q)))
            }
            ExprKind::CXXThisExpr => {
                let implicit = ex(self).this_is_implicit(e);
                Ok(self.this_expr(implicit, loc))
            }
            ExprKind::CStyleCastExpr
            | ExprKind::CXXFunctionalCastExpr
            | ExprKind::CXXStaticCastExpr
            | ExprKind::CXXConstCastExpr
            | ExprKind::CXXReinterpretCastExpr
            | ExprKind::CXXDynamicCastExpr => {
                let style = match k {
                    ExprKind::CStyleCastExpr => CastStyle::CStyle,
                    ExprKind::CXXFunctionalCastExpr => CastStyle::Functional,
                    ExprKind::CXXStaticCastExpr => CastStyle::Static,
                    ExprKind::CXXConstCastExpr => CastStyle::Const,
                    ExprKind::CXXReinterpretCastExpr => CastStyle::Reinterpret,
                    _ => CastStyle::Dynamic,
                };
                // The type as written: the cast's type, a reference for
                // a glvalue result ([expr.cast]/1).
                let t = n.ty();
                let written = match n.value_category() {
                    ValueCategory::LValue
                        if style != CastStyle::Functional
                            || self.sema.ast.types.is_reference(t) =>
                    {
                        self.sema.ast.types.lvalue_ref_to(t)
                    }
                    ValueCategory::XValue => self.sema.ast.types.rvalue_ref_to(t),
                    _ => t,
                };
                let written = if n.value_category() == ValueCategory::LValue
                    && style == CastStyle::Functional
                {
                    self.sema.ast.types.lvalue_ref_to(t)
                } else {
                    written
                };
                let ty = self.ty(written, loc)?;
                let o = ex(self).operand(e);
                // `A(a, b)` of an aggregate `A`: the no-op functional cast
                // over the parenthesized aggregate initialization.
                if style == CastStyle::Functional {
                    let mut p = o;
                    while ex(self).get(p).kind() == ExprKind::CXXBindTemporaryExpr {
                        p = ex(self).operand(p);
                    }
                    if ex(self).get(p).kind() == ExprKind::CXXParenListInitExpr {
                        return self.paren_list_init(p, ty, loc, end);
                    }
                }
                // `T{...}` of an aggregate with a non-trivial destructor
                // binds the list's temporary under the cast.
                let mut p = o;
                while ex(self).get(p).kind() == ExprKind::CXXBindTemporaryExpr {
                    p = ex(self).operand(p);
                }
                if self.sema.ast.exprs.get(p).kind() == ExprKind::InitListExpr {
                    let l = self.init_list(p)?;
                    return Ok(self.sema.act_on_list_cast(ty, &l, loc));
                }
                let o = self.expr(o)?;
                Ok(self.sema.act_on_cast(style, ty, o, loc, end))
            }
            ExprKind::CompoundLiteralExpr => {
                // WP-180: clang's `TransformCompoundLiteralExpr` rebuilds
                // through `BuildCompoundLiteralExpr` with the instantiated
                // type and list.
                // An analyzed literal of a non-dependent type over
                // instantiation-dependent elements goes back to its list:
                // a class's list construction (`(S){x, 1}`) or a single
                // element initializing the class ([dcl.init.list]/3.2).
                let o = ex(self).compound_literal_init(e);
                let p = self.peel_initializer(o);
                let ty = self.ty(n.ty(), loc)?;
                let l = match ex(self).get(p).kind() {
                    ExprKind::InitListExpr => self.init_list(p)?,
                    ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr
                        if ex(self).construct_info(p).list =>
                    {
                        self.construct_list(p, loc, end)?
                    }
                    _ => {
                        let x = self.expr(p)?;
                        let value = InitValue::Expr(x);
                        let items = vec![InitItem { designator: Vec::new(), value }];
                        InitList { items, lbrace: loc, rbrace: end }
                    }
                };
                Ok(self.sema.act_on_compound_literal(ty, &l, loc))
            }
            ExprKind::BuiltinBitCastExpr => {
                // WP-074: `__builtin_bit_cast(T, e)` (a dependent one keeps
                // `T` as its type and `e` unmaterialized).
                let ty = self.ty(n.ty(), loc)?;
                let o = self.expr(ex(self).operand(e))?;
                Ok(self.sema.act_on_builtin_bit_cast(ty, o, loc, end))
            }
            ExprKind::VAArgExpr => {
                // WP-171: clang's `TransformVAArgExpr` rebuilds through
                // `BuildVAArgExpr` (the checks run on the instantiated type).
                let (list, written, ty_loc) = ex(self).va_arg(e);
                let ty = self.ty(written, ty_loc)?;
                let list = self.expr(list)?;
                Ok(self.sema.act_on_va_arg(list, ty, ty_loc, loc, end))
            }
            // clang `TransformChooseExpr`: all three operands, then
            // `ActOnChooseExpr` again.
            ExprKind::ChooseExpr => {
                let [c, l, r] = ex(self).choose_operands(e);
                let c = self.expr(c)?;
                let l = self.expr(l)?;
                let r = self.expr(r)?;
                Ok(self.sema.act_on_choose_expr(loc, c, l, r, loc))
            }
            // clang `TransformExtVectorElementExpr`: the base, then the
            // member access again (`RebuildExtVectorElementExpr`).
            ExprKind::ExtVectorElementExpr => {
                let b = self.expr(ex(self).operand(e))?;
                let name = ex(self).ext_vector_accessor(e);
                let arrow = n.has(ExprFlags::ARROW);
                Ok(self.sema.act_on_member(b, name, loc, loc, arrow))
            }
            // clang `TransformOffsetOfExpr`: the operand type and the
            // index expressions, then `BuildBuiltinOffsetOf` again.
            ExprKind::OffsetOfExpr => {
                let Some((operand, comps)) = ex(self).offsetof_pattern(e) else {
                    return Ok(e);
                };
                let comps = comps.to_vec();
                let operand = self.ty(operand, loc)?;
                let mut out = Vec::with_capacity(comps.len());
                for c in comps {
                    out.push(match c {
                        crate::OffsetofComponent::Index { index, start, end } => {
                            let index = self.expr(index)?;
                            crate::OffsetofComponent::Index { index, start, end }
                        }
                        f @ crate::OffsetofComponent::Field { .. } => f,
                    });
                }
                Ok(self.sema.act_on_builtin_offsetof(loc, operand, &out, end))
            }
            ExprKind::ShuffleVectorExpr => {
                // clang `TransformShuffleVectorExpr`: the operands, rebuilt
                // (and re-checked) as the builtin call.
                let subs: Vec<ExprId> =
                    self.sema.ast.lists.iter::<ExprId>(ex(self).shuffle_subs(e)).collect();
                let mut args = Vec::with_capacity(subs.len());
                for s in subs {
                    args.push(self.expr(s)?);
                }
                Ok(self.sema.act_on_shufflevector(&args, loc, end))
            }
            ExprKind::ConvertVectorExpr => {
                let ty = self.ty(n.ty(), loc)?;
                let o = self.expr(ex(self).operand(e))?;
                Ok(self.sema.act_on_convert_vector(o, ty, loc, end))
            }
            ExprKind::CXXUnresolvedConstructExpr => {
                let ty = self.ty(n.ty(), loc)?;
                let l = kiln_ast::ids::ListId::new(n.a());
                if n.aux() != 0 && self.invalid_class_braced_cast(ty, loc) {
                    return Err(SubstFail);
                }
                if n.aux() != 0 {
                    let items: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
                    // `T{...}` keeps its braced list as the one argument.
                    if let [x] = items[..]
                        && self.sema.ast.exprs.get(x).kind() == ExprKind::InitListExpr
                    {
                        let il = self.init_list(x)?;
                        return Ok(self.sema.act_on_list_cast(ty, &il, loc));
                    }
                    let out = self.init_items(&items)?;
                    let il = InitList { items: out, lbrace: loc, rbrace: end };
                    return Ok(self.sema.act_on_list_cast(ty, &il, loc));
                }
                // `T(a, {b})`: a braced operand is a placeholder again.
                let args = self.call_arg_list(l)?;
                Ok(match args.as_slice() {
                    [] => self.sema.act_on_value_init_cast(ty, loc, end),
                    [a] if !self.sema.is_braced_arg(*a) => {
                        self.sema.act_on_cast(CastStyle::Functional, ty, *a, loc, end)
                    }
                    _ => self.sema.act_on_paren_list_cast(ty, &args, loc, end),
                })
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                let kind = ex(self).trait_kind(e);
                if let Some(t) = ex(self).trait_type_arg(e) {
                    let t = self.ty_written_after(t, loc)?;
                    Ok(self.sema.rebuild_trait_type(kind, t, loc, end))
                } else {
                    let o = ExprId::new(n.a());
                    let o = self.unevaluated_operand(o)?;
                    Ok(self.sema.act_on_trait_expr(kind, o, loc))
                }
            }
            // [expr.typeid]: rebuilt by the parser's action. As clang's
            // `TransformCXXTypeidExpr`, an expression operand is first
            // substituted as an unevaluated operand and substituted again,
            // potentially evaluated, if it turns out to be a glvalue of
            // polymorphic class type (/3).
            ExprKind::CXXTypeidExpr => {
                let operand = match ex(self).typeid_operand(e) {
                    TypeidOperand::Type(t) => Err(self.ty_written_after(t, loc)?),
                    TypeidOperand::Expr(o) => {
                        let u = self.unevaluated_operand(o)?;
                        if self.sema.is_polymorphic_glvalue(u) { Ok(self.expr(o)?) } else { Ok(u) }
                    }
                };
                Ok(self.sema.act_on_typeid(operand, loc, end))
            }
            // spec/sema-templates.md §9.1, §9.4: rebuilt through the
            // parser's builder, which warns about a deprecated spelling again
            // under the instantiation note (clang's `BuildTypeTrait`).
            ExprKind::TypeTraitExpr => {
                let t = ex(self).type_trait(e);
                let tys: Vec<QualType> =
                    self.sema.ast.lists.iter::<QualType>(ex(self).type_trait_args(e)).collect();
                let rhs = ex(self).type_trait_rhs_loc(e).unwrap_or(loc);
                let tys = self.expand_types(&tys, loc)?;
                let args: Vec<(QualType, Loc)> = tys
                    .iter()
                    .enumerate()
                    .map(|(i, &q)| (q, if i == 1 { rhs } else { loc }))
                    .collect();
                Ok(self.sema.act_type_trait(t, &args, loc, end))
            }
            // spec/reflection-repr.md §2: `^^T` reflects the substituted
            // type; a declaration of the pattern its instantiation; a
            // template template parameter its argument.
            ExprKind::CXXReflectExpr => match ex(self).reflect_operand(e) {
                Reflectee::Type(q) => {
                    let q = self.expansion_dealias(q);
                    let q = self.ty(q, loc)?;
                    Ok(self.sema.act_on_reflect_type(q, loc, end))
                }
                Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => {
                    let d = self.find_instantiated_decl(d, loc)?;
                    Ok(self.sema.reflect_entity(d, loc, loc, end))
                }
                Reflectee::Template(n) => {
                    let n = self.template_name(n, loc)?;
                    let n = self.sema.ast.types.canonical_template_name(n);
                    Ok(self.sema.make_reflect_expr(Reflectee::Template(n), loc, end))
                }
                _ => self.reflect_dependent_name(e, loc, end),
            },
            // spec/reflection.md §5: a dependent splice resolves once its
            // operand (and object) are substituted.
            ExprKind::CXXSpliceExpr => {
                let op = ex(self).splice_operand(e);
                let base = ex(self).splice_base(e);
                let targs = ex(self).splice_template_args(e);
                let op = self.expr(op)?;
                if let Some(list) = targs {
                    let list = self.args(list, loc)?;
                    let v = self.sema.ast.types.args(list).to_vec();
                    return Ok(self.sema.act_on_splice_template_expr(op, &v, loc, end));
                }
                Ok(match base {
                    None => self.sema.act_on_splice_expr(op, loc, end),
                    Some((b, arrow)) => {
                        let b = self.expr(b)?;
                        self.sema.act_on_member_splice(b, op, loc, end, loc, arrow)
                    }
                })
            }
            ExprKind::ArrayTypeTraitExpr => {
                let t = ex(self).type_trait(e);
                let q = ex(self).array_trait_type(e);
                let dim = ex(self).array_trait(e).dim;
                let q = self.ty(q, loc)?;
                let dim = self.opt_expr(dim)?;
                Ok(self.sema.build_array_type_trait(t, q, dim, loc, end))
            }
            ExprKind::ExpressionTraitExpr => {
                let t = ex(self).type_trait(e);
                let o = ex(self).expression_trait_operand(e);
                let o = self.unevaluated_operand(o)?;
                Ok(self.sema.build_expression_trait(t, o, loc, end))
            }
            ExprKind::CXXNoexceptExpr => {
                let o = ex(self).noexcept_operand(e);
                let o = self.unevaluated_operand(o)?;
                Ok(self.sema.act_on_noexcept_expr(o, loc, end))
            }
            ExprKind::CXXThrowExpr => {
                let o = ex(self).throw_operand(e);
                let o = self.opt_expr(o)?;
                Ok(self.sema.act_on_throw(o, loc))
            }
            // [expr.await], [expr.yield]: rebuilt from the operand as
            // written (clang `TransformDependentCoawaitExpr`,
            // `TransformCoyieldExpr`); the coroutine is built in the
            // instantiation's body.
            ExprKind::DependentCoawaitExpr | ExprKind::CoawaitExpr => {
                let o = ex(self).coawait_operand(e, &self.sema.ast.lists);
                let o = self.expr(o)?;
                Ok(self.sema.build_coawait(o, loc))
            }
            ExprKind::CoyieldExpr => {
                let o = ex(self).coawait_operand(e, &self.sema.ast.lists);
                let o = match self.braced_source(o) {
                    Some(b) => {
                        let l = self.braced_list(b)?;
                        self.sema.act_on_braced_arg(l)
                    }
                    None => self.expr(o)?,
                };
                Ok(self.sema.build_coyield(o, loc))
            }
            ExprKind::CXXFoldExpr => self.fold(e, loc, end),
            ExprKind::SizeOfPackExpr => self.size_of_pack(e, loc, end),
            ExprKind::PackIndexingExpr => self.pack_indexing_expr(e, loc, end),
            ExprKind::SubstNonTypeTemplateParmPackExpr => self.subst_nttp_pack(e, loc),
            ExprKind::FunctionParmPackExpr => self.subst_parm_pack(e, loc),
            ExprKind::PackExpansionExpr => self.unimplemented("pack expansion", loc),
            ExprKind::ConceptSpecializationExpr => self.concept_id_expr(e, loc),
            ExprKind::RequiresExpr => self.requires_expr(e, loc),
            ExprKind::LambdaExpr => self.lambda_expr(e, loc),
            ExprKind::CXXConstructExpr => self.construct_expr(e, loc),
            ExprKind::CXXTemporaryObjectExpr => {
                // `T(a, b)` / `T()` / `T{...}` ([expr.type.conv]/2): rebuilt
                // as written (clang `TransformCXXTemporaryObjectExpr`).
                let ty = self.ty(n.ty(), loc)?;
                if ex(self).construct_info(e).list {
                    let l = self.construct_list(e, loc, end)?;
                    return Ok(self.sema.act_on_list_cast(ty, &l, loc));
                }
                let args = self.call_arg_list(ex(self).construct_args(e))?;
                Ok(match args.as_slice() {
                    [] => self.sema.act_on_value_init_cast(ty, loc, end),
                    [a] if !self.sema.is_braced_arg(*a) => {
                        self.sema.act_on_cast(CastStyle::Functional, ty, *a, loc, end)
                    }
                    _ => self.sema.act_on_paren_list_cast(ty, &args, loc, end),
                })
            }
            ExprKind::CXXNewExpr => self.new_expr(e, loc, end),
            ExprKind::CXXDeleteExpr => {
                // [expr.delete], rebuilt from the operand as written (clang
                // `TransformCXXDeleteExpr`).
                let o = ex(self).delete_operand(e);
                let o = self.expr(o)?;
                let global = n.has(ExprFlags::GLOBAL);
                let array = n.has(ExprFlags::ARRAY_AS_WRITTEN);
                let cxx26 = self.sema.delete_cxx26;
                Ok(self.sema.act_on_cxx_delete(global, array, loc, o, cxx26))
            }
            ExprKind::RecoveryExpr => self.recovery_expr(e, loc),
            ExprKind::CXXExpansionSelectExpr => self.expansion_select(e),
            ExprKind::CXXParenListInitExpr => {
                let ty = self.ty(n.ty(), loc)?;
                self.paren_list_init(e, ty, loc, end)
            }
            // `std::initializer_list<E>` over its backing array
            // ([dcl.init.list]/5): the list initializes the (substituted)
            // `std::initializer_list` type again.
            ExprKind::CXXStdInitializerListExpr => {
                let ty = self.ty(n.ty(), loc)?;
                let b = self.braced_source(e).ok_or(SubstFail)?;
                let l = self.braced_list(b)?;
                Ok(self.sema.init_from_list_at(ty, &l, Some(l.lbrace)))
            }
            // A braced-init-list template argument (P2308R1): substituted
            // and kept as written until it initializes the parameter.
            ExprKind::InitListExpr if n.ty() == QualType::VOID => {
                let l = self.init_list(e)?;
                Ok(self.sema.act_on_braced_template_arg(&l))
            }
            _ => self.unimplemented(&format!("substitution into {k:?}"), loc),
        }
    }

    /// An erroneous expression of the pattern (spec/ast.md §5.3): the
    /// error was diagnosed when the pattern was parsed, so the
    /// instantiation is a fresh `RecoveryExpr` over the substituted
    /// children and is never diagnosed again (clang
    /// `TreeTransform::TransformRecoveryExpr`). Its `ERROR` dependence
    /// suppresses follow-on diagnostics in the enclosing expressions. The
    /// type is kept when it was known and substitutes cleanly; otherwise
    /// it is the error type (never dependent after instantiation).
    fn recovery_expr(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let n = self.sema.ast.exprs.get(e);
        let (ty, l, vk) = (n.ty(), kiln_ast::ids::ListId::new(n.a()), n.value_category());
        let kids: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
        let mut out = Vec::with_capacity(kids.len());
        for k in kids {
            out.push(self.expr(k)?);
        }
        let ty = if self.sema.is_error_type(ty)
            || self.sema.ast.types.builtin(ty) == Some(BuiltinType::Dependent)
        {
            QualType::ERROR
        } else {
            match self.ty(ty, loc) {
                Ok(t) if !self.sema.is_dependent_type(t) => t,
                _ => QualType::ERROR,
            }
        };
        Ok(self.sema.builder().recovery(Some(ty), vk, &out, loc))
    }

    /// A new-expression ([expr.new]) rebuilt from the allocated type, the
    /// first bound, the placement arguments and the initializer as
    /// written (clang `TransformCXXNewExpr`), then analyzed again by
    /// [`crate::Sema::act_on_cxx_new`]. A dependent one was kept as
    /// written; an analyzed one (a non-dependent type over
    /// instantiation-dependent operands, `new S[2]{{x}, {x, 1}}`) goes
    /// back to its written form: the placement arguments before their
    /// conversions and the initializer before its initialization
    /// ([`Self::new_initializer`]), so the allocation function is looked
    /// up and the initialization done anew.
    fn new_expr(&mut self, e: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        use crate::declspec::{Chunk, Declarator, DeclaratorContext};
        use crate::new_delete::{NewPlacement, NewTypeId};
        let n = *self.sema.ast.exprs.get(e);
        let x = *self.sema.ast.exprs.new_extra(e);
        let placement = match x.placement {
            // The allocation function's default arguments are added again.
            Some(l) => Some(NewPlacement { args: self.expr_list(l)?, lparen: loc, rparen: loc }),
            None => None,
        };
        let base = self.ty(x.allocated_ty, x.type_loc)?;
        let mut declarator = Declarator::new(DeclaratorContext::TypeName, x.type_loc);
        declarator.site = crate::declspec::DeclSite::New;
        if n.has(ExprFlags::ARRAY_FORM) {
            let size = self.opt_expr(x.array_size)?;
            declarator.chunks.push(Chunk::Array { size, loc: x.type_loc });
        }
        let tid = NewTypeId { base, declarator, loc: x.type_loc, parenthesized: false };
        let init = self.new_initializer(e, x.init, loc)?;
        let global = n.has(ExprFlags::GLOBAL);
        Ok(self.sema.act_on_cxx_new(global, loc, placement, tid, init, end))
    }

    /// The written new-initializer of new-expression `e` from its stored
    /// initializer `init` ([expr.new]/23): a dependent one is the
    /// `ParenListExpr` or braced list as written; an analyzed one is
    /// mapped back through [`Self::initializer`], its style (parentheses
    /// or braces) kept, a value-initialization `new T()` being the empty
    /// parentheses.
    fn new_initializer(
        &mut self,
        e: ExprId,
        init: Option<ExprId>,
        loc: Loc,
    ) -> SubstResult<crate::new_delete::NewInit> {
        use crate::new_delete::NewInit;
        use kiln_ast::decl::InitStyle;
        use kiln_ast::expr::NewInitStyle;
        let style = self.sema.ast.exprs.new_init_style(e);
        let braces = style == NewInitStyle::Braces;
        let empty = |braces: bool| {
            if braces {
                NewInit::List(InitList { items: Vec::new(), lbrace: loc, rbrace: loc })
            } else {
                NewInit::Paren(Vec::new(), loc, loc)
            }
        };
        let Some(i) = init.filter(|_| style != NewInitStyle::None) else {
            return Ok(if style == NewInitStyle::None { NewInit::None } else { empty(braces) });
        };
        let node = *self.sema.ast.exprs.get(i);
        match node.kind() {
            ExprKind::ParenListExpr => {
                let l = kiln_ast::ids::ListId::new(node.a());
                let args = self.call_arg_list(l)?;
                let r = self.sema.ast.exprs.end_loc(i).unwrap_or(node.loc());
                return Ok(NewInit::Paren(args, node.loc(), r));
            }
            ExprKind::InitListExpr if braces => return Ok(NewInit::List(self.init_list(i)?)),
            _ => {}
        }
        // `new T()`: the value-initialization of the analyzed form.
        if self.sema.ast.exprs.get(self.peel_initializer(i)).kind()
            == ExprKind::ImplicitValueInitExpr
        {
            return Ok(empty(braces));
        }
        let written = if braces { InitStyle::List } else { InitStyle::Call };
        Ok(match (self.initializer(i, written)?, braces) {
            (None, b) => empty(b),
            (Some(Initializer::List(l)), true) => NewInit::List(l),
            (Some(Initializer::Paren(v, l, r)), false) => NewInit::Paren(v, l, r),
            (Some(Initializer::Expr(x)), false) => NewInit::Paren(vec![x], loc, loc),
            (Some(Initializer::Expr(x)), true) => {
                let items = vec![InitItem { designator: Vec::new(), value: InitValue::Expr(x) }];
                NewInit::List(InitList { items, lbrace: loc, rbrace: loc })
            }
            // `new T[n](a, {b})` (P0960) analyzed into a list: the
            // elements as parenthesized arguments.
            (Some(Initializer::List(l)), false) => {
                let args = l
                    .items
                    .into_iter()
                    .map(|it| match it.value {
                        InitValue::Expr(x) => x,
                        InitValue::List(n) => self.sema.act_on_braced_arg(n),
                    })
                    .collect();
                NewInit::Paren(args, l.lbrace, l.rbrace)
            }
            (Some(Initializer::Paren(v, l, r)), true) => {
                let items = v
                    .into_iter()
                    .map(|x| InitItem { designator: Vec::new(), value: InitValue::Expr(x) })
                    .collect();
                NewInit::List(InitList { items, lbrace: l, rbrace: r })
            }
        })
    }

    /// The object expression of a member access: an implicit `this` is
    /// rebuilt implicit.
    /// The member name `name` of a member access, substituted: the target
    /// type of a conversion-function-id is rebuilt (`t.operator T()`,
    /// [temp.dep.type]); identifiers and operator names are kept.
    /// `^^nns name` with a dependent `nns` (spec/reflection-repr.md §2,
    /// WP-519): the entity the qualified-id names once `nns` is
    /// substituted, reflected as the parser reflects a non-dependent one
    /// (the name is not looked up again by its spelling: the
    /// `DependentScopeDeclRefExpr` keeps the `DeclName`).
    fn reflect_dependent_name(&mut self, r: ExprId, loc: Loc, end: Loc) -> SubstResult<ExprId> {
        let Some(e) = self.sema.ast.exprs.reflect_dependent_name(r) else { return Ok(r) };
        let Some(nns) = kiln_ast::ids::NnsId::try_new(self.sema.ast.exprs.get(e).a()) else {
            return Err(SubstFail);
        };
        let name = self.sema.ast.dependent_scope_ref_name(e);
        let name_loc = self.sema.ast.exprs.get(e).loc();
        let begin = self.sema.ast.templates.expr_begin.get(e).copied().unwrap_or(name_loc);
        let template_kw = self.sema.ast.templates.template_kw.get(e).copied();
        let targs = self.sema.ast.templates.expr_targs.get(e).copied();
        let nns = self.nns(nns, begin)?;
        // A destructor name keeps its (substituted) type to check against
        // the class the qualifier names.
        let (name, destroyed) = match self.sema.ast.names.view(name) {
            DeclNameView::Destructor(t) => {
                let t = self.ty(QualType::unqual(t), name_loc)?;
                let canon = self.sema.ast.types.canonical(t).ty();
                (self.sema.ast.names.destructor(canon), Some(t))
            }
            _ => (self.member_name(name, name_loc)?, None),
        };
        if self.nns_still_dependent(nns) {
            let targs = match targs {
                Some(l) => Some(self.args(l, loc)?),
                None => None,
            };
            return Ok(self.sema.act_on_reflect_dependent(
                (nns, begin),
                name,
                targs,
                template_kw,
                name_loc,
                loc,
                end,
            ));
        }
        // `^^T::template m<args>` ([temp.names]/5): the member template's
        // specialization, as `^^C::m<args>` reflects it.
        if let Some(l) = targs {
            let Some(id) = name.as_ident() else {
                return self.unimplemented("an operator function template-id", name_loc);
            };
            let t = self.member_template_named(nns, id, name_loc)?;
            let l = self.args(l, loc)?;
            let args = self.sema.ast.types.args(l).to_vec();
            let tn = self.sema.template_name_for(t);
            let mut res =
                self.sema.act_template_id(tn, &args, name_loc, name_loc).map_err(|()| SubstFail)?;
            if let crate::templ::actions::TemplateIdRef::Overload(x) = res {
                let ctx = self.nns_context(nns, begin)?;
                let x = self.sema.widen_function_template_id(x, id, Some(ctx));
                self.sema.set_template_id_begin(x, begin, name_loc);
                self.sema.set_template_id_qualifier(x, nns);
                res = crate::templ::actions::TemplateIdRef::Overload(x);
            }
            let ctx = self.nns_context(nns, begin)?;
            let r = self.sema.act_on_reflect_template_id(res, name_loc, loc, end);
            return Ok(self.sema.check_reflect_dependent_kind(r, ctx, name, true, name_loc, loc));
        }
        let ctx = self.nns_context(nns, begin)?;
        if let Some(t) = destroyed {
            let class = self.sema.ast.types.as_record(t);
            if class.map(|c| self.sema.ast.first(c)) != Some(self.sema.ast.first(ctx)) {
                return Ok(self.sema.reflect_bad_operand(name_loc, loc));
            }
            return Ok(self.sema.reflect_destructor_of(ctx, name_loc, loc, end));
        }
        let q = crate::using::Qualifier { ctx, nns, loc: begin };
        let r = self.sema.act_on_reflect_dn(Some(q), name, name_loc, loc, end);
        let template_kw = template_kw.is_some();
        Ok(self.sema.check_reflect_dependent_kind(r, ctx, name, template_kw, name_loc, loc))
    }

    fn member_name(&mut self, name: DeclName, loc: Loc) -> SubstResult<DeclName> {
        match self.sema.ast.names.view(name) {
            DeclNameView::Conversion(t) => {
                let t = self.ty(t, loc)?;
                let t = self.sema.ast.types.canonical(t);
                Ok(self.sema.ast.names.conversion(t))
            }
            DeclNameView::Destructor(_) | DeclNameView::Constructor(_) => {
                self.unimplemented("member access to a special member", loc)
            }
            _ => Ok(name),
        }
    }

    /// The substituted qualifier of a member access on the substituted
    /// object `base`: a leading dependent `name::` (not found where the
    /// template was defined) is looked up in the object's class now
    /// ([basic.lookup.qual.general]/2, WP-121).
    fn member_qualifier(
        &mut self,
        n: kiln_ast::ids::NnsId,
        base: ExprId,
        arrow: bool,
        loc: Loc,
    ) -> SubstResult<crate::using::Qualifier> {
        let (ctx, nns) = self.member_nns(n, base, arrow, loc)?;
        Ok(crate::using::Qualifier { ctx, nns, loc })
    }

    fn member_nns(
        &mut self,
        n: kiln_ast::ids::NnsId,
        base: ExprId,
        arrow: bool,
        loc: Loc,
    ) -> SubstResult<(DeclId, kiln_ast::ids::NnsId)> {
        match self.sema.ast.types.nns_view(n) {
            Nns::Identifier { prefix: None, name } => {
                let bt = self.sema.ety(base);
                let obj = if arrow { self.sema.ast.types.pointee(bt) } else { Some(bt) };
                let Some(rec) = obj.and_then(|o| self.sema.ast.types.as_record(o)) else {
                    let d = self
                        .sema
                        .diag(kiln_diag::DiagId::sema_undeclared_var_use, loc)
                        .arg(self.sema.ast.symbols().as_str(name).to_owned());
                    self.sema.emit(d);
                    return Err(SubstFail);
                };
                let def = self.sema.ast.definition(rec).unwrap_or(rec);
                let found = self.sema.lookup_in_class(def, name, crate::lookup::LookupKind::Nested);
                let Some((c, spelled)) = self.sema.nested_name_from_found(None, name, &found, loc)
                else {
                    return Err(SubstFail);
                };
                Ok((c, self.sema.nns_component(None, c, spelled)))
            }
            Nns::Identifier { prefix: Some(p), name } if self.rooted_in_member(p) => {
                let (pc, pn) = self.member_nns(p, base, arrow, loc)?;
                let Some((c, spelled)) = self.sema.act_on_nested_name_spelled(Some(pc), name, loc)
                else {
                    return Err(SubstFail);
                };
                Ok((c, self.sema.nns_component(Some(pn), c, spelled)))
            }
            _ => {
                let q = self.nns(n, loc)?;
                let c = self.nns_context(q, loc)?;
                // The injected-class-name of the class template being
                // instantiated (`this->S::f()` in `S<A>`) names the
                // pattern: map it to the instantiation (§5.6).
                let c = if self.sema.kind_of(c) == DeclKind::CXXRecord {
                    self.find_instantiated_decl(c, loc)?
                } else {
                    c
                };
                Ok((c, q))
            }
        }
    }

    /// True if specifier `n` starts with a dependent `name::` of a
    /// member access's object scope.
    fn rooted_in_member(&self, mut n: kiln_ast::ids::NnsId) -> bool {
        loop {
            match self.sema.ast.types.nns_view(n) {
                Nns::Identifier { prefix: None, .. } => return true,
                Nns::Identifier { prefix: Some(p), .. } => n = p,
                _ => return false,
            }
        }
    }

    fn member_base(&mut self, base: ExprId) -> SubstResult<ExprId> {
        let b = *self.sema.ast.exprs.get(base);
        if b.kind() == ExprKind::CXXThisExpr && self.needs_rebuild(base) {
            let implicit = self.sema.ast.exprs.this_is_implicit(base);
            return Ok(self.this_expr(implicit, b.loc()));
        }
        self.expr(base)
    }

    /// A member named by a dependent using-declarator with no object
    /// (`using B<T>::m; ... m`, `Sema::act_on_id_expression_dn`): the
    /// instantiated class now has the using-declaration's shadows, so
    /// the name is looked up again from the instantiation context and
    /// becomes an implicit member access or a static member reference
    /// (clang's `TransformUnresolvedLookupExpr` over the
    /// `UnresolvedUsingValueDecl`, then `BuildPossibleImplicitMemberExpr`).
    fn implicit_dependent_member(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let name = self.sema.ast.dependent_member_name(e);
        let name = self.member_name(name, loc)?;
        if self.sema.ast.templates.expr_targs.get(e).is_some() {
            return self.unimplemented("implicit dependent member template-id", loc);
        }
        let qual = match self.sema.ast.templates.expr_nns.get(e).copied() {
            Some(q) => {
                let at = self.sema.ast.templates.expr_begin.get(e).copied().unwrap_or(loc);
                let nns = self.nns(q, at)?;
                if self.nns_still_dependent(nns) {
                    return self.unimplemented("implicit dependent member access", loc);
                }
                let ctx = self.nns_context(nns, at)?;
                Some(crate::using::Qualifier { ctx, nns, loc: at })
            }
            None => None,
        };
        Ok(self.sema.act_on_id_expression_dn(name, loc, qual))
    }

    /// [`Self::member_base`], except that an implicit `this` is not yet
    /// captured: whether the access uses it is known only once the
    /// member is looked up again ([`Self::note_deferred_this`]). Returns
    /// the location of that deferred use.
    fn member_base_deferred(&mut self, base: ExprId) -> SubstResult<(ExprId, Option<Loc>)> {
        let b = *self.sema.ast.exprs.get(base);
        if b.kind() == ExprKind::CXXThisExpr
            && self.sema.ast.exprs.this_is_implicit(base)
            && let Some(t) = self.sema.current_this()
        {
            return Ok((self.sema.builder().this_expr(t, true, b.loc()), Some(b.loc())));
        }
        Ok((self.member_base(base)?, None))
    }

    /// The deferred implicit `this` at `at` of the rebuilt member access
    /// `r` (`None`: a field or destructor access, which uses it): the
    /// enclosing lambdas capture `this` unless `r` is an implicit member
    /// overload set with a static member, which uses `this` only once
    /// an instance member wins ([`Sema::finish_member_winner`]), or a
    /// static member named without an object. As clang's
    /// `TreeTransform`, which does not transform an implicit access's
    /// `this` and builds the member reference again
    /// (`BuildPossibleImplicitMemberExpr`; [expr.prim.lambda.capture]/8,
    /// [class.mfct.non.static]/2).
    fn note_deferred_this(&mut self, r: Option<ExprId>, at: Option<Loc>) {
        let Some(loc) = at else { return };
        if let Some(r) = r {
            if let Some(x) = self.sema.member_overload(r)
                && let Some(m) = self.sema.ovl.members.get(&x)
                && m.implicit
                && !m.decls.iter().all(|&d| {
                    let f = self.sema.templated_or_self(d);
                    self.sema.is_instance_member(f)
                })
            {
                return;
            }
            let x = self.sema.ignore_parens(r);
            if self.sema.ast.exprs.get(x).kind() == ExprKind::DeclRefExpr {
                return;
            }
        }
        self.sema.note_this_use(loc);
    }

    fn this_expr(&mut self, implicit: bool, loc: Loc) -> ExprId {
        match self.sema.current_this() {
            Some(t) => {
                self.sema.note_this_use(loc);
                self.sema.builder().this_expr(t, implicit, loc)
            }
            None => self.sema.act_on_this(loc),
        }
    }

    /// A non-ADL callee (`N::g`, `S::f<T>`): the decls were found in a
    /// non-dependent scope; the reference is rebuilt as written.
    fn qualified_callee(
        &mut self,
        c: ExprId,
        n: &ExprNode,
        name: DeclName,
        decls: &[DeclId],
        targs: Option<ArgListId>,
    ) -> SubstResult<ExprId> {
        if let (Some(&d), Some(q)) =
            (decls.first(), self.sema.ast.templates.expr_nns.get(c).copied())
            && self.in_dependent_specialization(d, n.loc())?
        {
            // A member of a class template pattern whose substituted class
            // is still dependent: `X<A...>::name`, looked up again later.
            let nns = self.nns(q, n.loc())?;
            let begin = self.sema.ast.templates.expr_begin.get(c).copied().unwrap_or(n.loc());
            return Ok(self.sema.act_dependent_scope_ref(nns, name, targs, begin, n.loc()));
        }
        if let [d] = decls {
            let d = self.find_instantiated_decl(*d, n.loc())?;
            // `v<args>` naming a member variable template of the
            // current instantiation ([temp.dep.expr]/3).
            if self.sema.kind_of(d) == DeclKind::VarTemplate
                && let Some(&targs) = self.sema.ast.templates.expr_targs.get(c)
            {
                let l = self.args(targs, n.loc())?;
                let args = self.sema.ast.types.args(l).to_vec();
                let name = self.sema.template_name_for(d);
                let Ok(crate::templ::actions::TemplateIdRef::Value(e)) =
                    self.sema.act_template_id(name, &args, n.loc(), n.loc())
                else {
                    return Err(SubstFail);
                };
                // `N::v<T>` keeps its qualifier (WP-300).
                if let Some(q) = self.sema.ast.templates.expr_nns.get(c).copied() {
                    let q = self.nns(q, n.loc())?;
                    self.sema.set_template_id_qualifier(e, q);
                }
                return Ok(e);
            }
            let nns = match self.sema.ast.templates.expr_nns.get(c).copied() {
                Some(q) => Some(self.nns(q, n.loc())?),
                None => None,
            };
            if self.sema.kind_of(d) == DeclKind::FunctionTemplate || targs.is_some() {
                return Ok(self.sema.builder().unresolved_lookup(
                    name,
                    &[d],
                    false,
                    nns,
                    targs,
                    n.loc(),
                ));
            }
            return Ok(self.sema.build_decl_ref_qualified(d, n.loc(), None, nns));
        }
        // An overload set: resolved again at the call.
        let mut ds = Vec::with_capacity(decls.len());
        for &d in decls {
            ds.push(self.find_instantiated_decl(d, n.loc())?);
        }
        let nns = match self.sema.ast.templates.expr_nns.get(c).copied() {
            Some(q) => Some(self.nns(q, n.loc())?),
            None => None,
        };
        Ok(self.sema.builder().unresolved_lookup(name, &ds, false, nns, targs, n.loc()))
    }

    /// A name `e` found as a block-scope dependent using-declarator
    /// (`using E<T>::a; ... a`): looked up again in the instantiation's
    /// block scope, which now holds the instantiated declaration's
    /// shadows (as a call's callee when `call`). A use of one whose
    /// instantiation failed is an error already reported (clang finds
    /// the invalid `UsingDecl`). `None` for any other name.
    fn local_using_ref(&mut self, e: ExprId, call: bool) -> Option<ExprId> {
        let decls = self.sema.ast.unresolved_decls(e);
        let &[d] = decls.as_slice() else { return None };
        if self.sema.kind_of(d) != DeclKind::UnresolvedUsingValue
            || !self.sema.is_block_scope_decl(d)
            || self.sema.ast.templates.expr_targs.get(e).is_some()
        {
            return None;
        }
        let loc = self.sema.ast.exprs.get(e).loc();
        if self.failed_locals.contains(&d) {
            return Some(self.sema.recover_err(&[], loc));
        }
        let name = self.sema.ast.unresolved_name(e);
        Some(if call {
            self.sema.act_on_call_name_dn(name, loc)
        } else {
            self.sema.act_on_id_expression_dn(name, loc, None)
        })
    }

    /// A callee: an unresolved name is looked up again from the
    /// instantiation context, which also stands in for the
    /// argument-dependent lookup at the point of instantiation
    /// ([temp.dep.candidate]; spec §5.4 gap: one lookup, not two).
    fn callee(&mut self, c: ExprId) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(c);
        // `(f)(x)` ([over.call.func]/3: no argument-dependent lookup, which
        // the name already records): the parenthesized name is a callee
        // too, looked up and access-checked from the instantiation context
        // (boost's `(on_capacity_overflow)(throw_on_overflow_t())`).
        if n.kind() == ExprKind::ParenExpr {
            let inner = self.sema.ast.exprs.operand(c);
            let named = self.sema.ignore_parens(inner);
            if self.sema.ast.exprs.get(named).kind() == ExprKind::UnresolvedLookupExpr {
                let end = self.sema.ast.exprs.end_loc(c).unwrap_or(n.loc());
                let o = self.callee(inner)?;
                return Ok(self.sema.act_on_paren(o, n.loc(), end));
            }
        }
        if n.kind() != ExprKind::UnresolvedLookupExpr {
            // A resolved callee rebuilt because of an argument (a
            // function-name-dependent default argument): the call is redone
            // on the function itself, so it adds its default arguments again.
            // A builtin's name (`BuiltinFnToFnPtr`) is called again as the
            // builtin: a custom-checked one (`__builtin_constant_p`) keeps
            // its own argument checks, not its declared type's ellipsis.
            let ex = &self.sema.ast.exprs;
            if n.kind() == ExprKind::ImplicitCastExpr
                && matches!(
                    ex.cast_kind(c),
                    CastKind::FunctionToPointerDecay | CastKind::BuiltinFnToFnPtr
                )
                && ex.get(ex.operand(c)).kind() == ExprKind::DeclRefExpr
            {
                let o = ex.operand(c);
                return self.expr(o);
            }
            if n.kind() == ExprKind::DependentScopeDeclRefExpr {
                // `T::template f<A>(x)`: the call deduces the rest.
                self.as_callee = true;
                let r = self.expr(c);
                self.as_callee = false;
                return r;
            }
            return self.expr(c);
        }
        if let Some(r) = self.local_using_ref(c, true) {
            return Ok(r);
        }
        let name = self.sema.ast.unresolved_name(c);
        let decls = self.sema.ast.unresolved_decls(c);
        let adl = n.aux() != 0;
        // Explicit template arguments (`f<T>(x)`): substituted, and kept
        // on the rebuilt name for the call's deduction (§6.1).
        let written_targs = self.sema.ast.templates.expr_targs.get(c).copied();
        let targs = match written_targs {
            Some(l) if !decls.iter().any(|&d| self.sema.kind_of(d) == DeclKind::VarTemplate) => {
                Some(self.args(l, n.loc())?)
            }
            _ => None,
        };
        if !adl || self.retained_static_members(&decls) {
            // `N::g(p)` keeps its written begin (the qualifier) for the
            // call-site notes that report it ([expr.const]).
            let begin = self.sema.ast.templates.expr_begin.get(c).copied();
            let r = self.qualified_callee(c, &n, name, &decls, targs);
            if let (Ok(r), Some(b)) = (&r, begin) {
                let _ = self.sema.ast.templates.expr_begin.insert(*r, b);
            }
            if let Ok(r) = r {
                self.keep_template_kw(c, r);
            }
            return r;
        }
        if decls.is_empty() {
            // `get<0>(c)` assumed a template-name ([temp.names]/3): ADL at
            // the call finds it, with the explicit arguments.
            if let Some(t) = targs {
                return Ok(self.sema.builder().unresolved_lookup(
                    name,
                    &[],
                    true,
                    None,
                    Some(t),
                    n.loc(),
                ));
            }
            return Ok(self.sema.act_on_undeclared_call_name_dn(name, n.loc()));
        }
        if decls.iter().all(|&d| self.namespace_scope_decl(d)) {
            // [temp.dep.candidate]/1 (§3.5): the declarations unqualified
            // lookup found at the definition, plus argument-dependent
            // lookup at the call. Looking the name up again from the
            // context the instantiation runs in would see that context's
            // names instead (a `swap` of the namespace that requires a
            // concept, hiding the poison pill beside the concept), or
            // none at all: a partial specialization's argument
            // `decltype(declval<T>().f())` is substituted where the
            // specialization is named, outside the template's namespace.
            // Explicit template arguments stay on the name for the call.
            let r = self.sema.builder().unresolved_lookup(name, &decls, true, None, targs, n.loc());
            if let Some(f) = self.sema.ovl.ule_found.get(&c).cloned() {
                self.sema.ovl.ule_found.insert(r, f);
            }
            return Ok(r);
        }
        let r = self.sema.act_on_call_name_dn(name, n.loc());
        if let Some(t) = targs {
            if self.sema.ast.exprs.get(r).kind() == ExprKind::UnresolvedLookupExpr {
                // Rebuilt with the arguments, so that dependent ones (an
                // outer level still being substituted) make the name, and
                // the call, type-dependent ([temp.dep.expr]/3).
                let rn = *self.sema.ast.exprs.get(r);
                let ds = self.sema.ast.unresolved_decls(r);
                let nns = self.sema.ast.templates.expr_nns.get(r).copied();
                return Ok(self.sema.builder().unresolved_lookup(
                    name,
                    &ds,
                    rn.aux() != 0,
                    nns,
                    Some(t),
                    rn.loc(),
                ));
            } else if self.sema.is_member_template_ref(r) {
                // An implicit member template-id `this->f<T>` (§6.4).
                return Ok(self.sema.act_on_member_template_args(r, t));
            }
        }
        Ok(r)
    }

    /// `d`, a specialization of a member function template of a class
    /// template pattern that the pattern already selected (`ok<int>()`
    /// in a requires-clause, resolved at the definition): the same
    /// specialization of the member template of the instantiated class
    /// (clang keeps the call dependent and resolves it then). `None`
    /// when `d` is no such specialization or its template stays.
    fn respecialized_member_template(
        &mut self,
        d: DeclId,
        loc: Loc,
    ) -> SubstResult<Option<ExprId>> {
        let Some((ft, args)) = self.sema.ast.spec_info(d).map(|(_, s)| (s.template, s.args)) else {
            return Ok(None);
        };
        if self.sema.kind_of(ft) != DeclKind::FunctionTemplate
            || !self
                .sema
                .ast
                .decls
                .get(ft)
                .flags()
                .contains(kiln_ast::decl::DeclFlags::DEPENDENT_CTX)
        {
            return Ok(None);
        }
        let it = self.find_instantiated_decl(ft, loc)?;
        if it == ft {
            return Ok(None);
        }
        let l = self.args(args, loc)?;
        if let Some(spec) = self.structural_member_spec(it, l, loc)? {
            return Ok(Some(self.sema.build_decl_ref_qualified(spec, loc, None, None)));
        }
        let ty = &self.sema.ast.types;
        let flat: Vec<TemplateArg> = ty
            .args(l)
            .iter()
            .flat_map(|&a| match a.view() {
                ArgView::Pack(p) => ty.args(p).to_vec(),
                _ => vec![a],
            })
            .collect();
        let tn = self.sema.template_name_for(it);
        match self.sema.act_template_id(tn, &flat, loc, loc) {
            Ok(crate::templ::actions::TemplateIdRef::Value(x)) => Ok(Some(x)),
            Ok(crate::templ::actions::TemplateIdRef::Overload(x)) => {
                Ok(Some(self.sema.resolve_single_template_id(x)))
            }
            _ => Err(SubstFail),
        }
    }

    /// The specialization of member function template `it` for `l`, the
    /// substituted arguments the pattern's specialization was formed with
    /// (one per parameter, each pack one `Pack`), when `it` has a
    /// parameter pack that is not its last parameter: re-reading the
    /// flattened arguments as explicit ones would give the pack every
    /// argument after it ([temp.arg.explicit]/9), so a trailing defaulted
    /// or deduced parameter would be swallowed. The deduction already made
    /// is kept instead ([temp.deduct.call]/1, [temp.param]/14; clang's
    /// `FindInstantiatedDecl` re-forms the specialization from its
    /// structured argument list). `None` (the template-id path) otherwise,
    /// or while the arguments stay dependent.
    fn structural_member_spec(
        &mut self,
        it: DeclId,
        l: ArgListId,
        loc: Loc,
    ) -> SubstResult<Option<DeclId>> {
        let params = self.sema.template_params(it);
        let is_pack = |s: &Self, p: DeclId| s.sema.ast.template_parm(p).is_some_and(|x| x.pack);
        let inner_pack =
            params.split_last().is_some_and(|(_, init)| init.iter().any(|&p| is_pack(self, p)));
        if !inner_pack {
            return Ok(None);
        }
        let ty = &self.sema.ast.types;
        let args = ty.args(l);
        let canonical = ty.canonical_args(l);
        if args.len() != params.len()
            || params
                .iter()
                .zip(args)
                .any(|(&p, a)| is_pack(self, p) != matches!(a.view(), ArgView::Pack(_)))
            || ty.arg_list_flags(canonical).intersects(
                kiln_ast::types::TypeFlags::DEPENDENT | kiln_ast::types::TypeFlags::UNEXPANDED_PACK,
            )
        {
            return Ok(None);
        }
        let conv = crate::templ::args::ConvertedArgs { sugared: l, canonical };
        self.sema.get_function_spec(it, &conv, loc).map(Some).map_err(|()| SubstFail)
    }

    /// Whether `decls` are static member functions (or function
    /// templates) of a class template pattern, named outside the class or
    /// while its own level is kept: the deduction guide of a constructor template
    /// ([over.match.class.deduct]/1.2) naming `ok<U...>()` in its
    /// requires-clause. They are kept as found (clang's
    /// `TransformUnresolvedLookupExpr` transforms each declaration), not
    /// looked up again from the guide's namespace scope, and are found in
    /// the deduced specialization when the guide's constraints are
    /// checked (libstdc++ tuple:980 `__disambiguating_constraint`).
    fn retained_static_members(&mut self, decls: &[DeclId]) -> bool {
        let Some(&d0) = decls.first() else { return false };
        let Some(parent) = self.sema.ast.decls.get(d0).ctx() else { return false };
        if !self.is_class_pattern(parent) {
            return false;
        }
        decls.iter().all(|&d| {
            let d = self.sema.ast.strip_shadow(d);
            let f = match self.sema.kind_of(d) {
                DeclKind::FunctionTemplate => self.sema.templated_function(d),
                _ => Some(d),
            };
            self.sema.ast.decls.get(d).ctx() == Some(parent)
                && f.is_some_and(|f| self.sema.is_static_method(f))
        })
    }

    /// Whether `d`, found by an unqualified lookup at a template's
    /// definition, is a namespace member (not a class member, which
    /// needs the instantiated class and an implicit `this`, nor a
    /// block-scope declaration, which needs the instantiated function).
    fn namespace_scope_decl(&self, d: DeclId) -> bool {
        let d = self.sema.ast.strip_shadow(d);
        if !matches!(self.sema.kind_of(d), DeclKind::Function | DeclKind::FunctionTemplate) {
            return false;
        }
        let mut c = self.sema.ast.decls.get(d).ctx();
        while let Some(x) = c {
            match self.sema.kind_of(x) {
                DeclKind::Namespace | DeclKind::TranslationUnit => return true,
                DeclKind::LinkageSpec | DeclKind::Export => c = self.sema.ast.decls.get(x).ctx(),
                _ => return false,
            }
        }
        false
    }

    /// A reference to a non-type template parameter (§5.4): the
    /// argument as a literal wrapped in `SubstNonTypeTemplateParmExpr`.
    fn nttp_ref(&mut self, e: ExprId, p: DeclId, loc: Loc) -> SubstResult<ExprId> {
        let Some(info) = self.sema.ast.template_parm(p).copied() else { return Err(SubstFail) };
        let a = match self.lookup(info.depth, info.index) {
            // Rebuilding a partially substituted expansion (§7.2): the
            // pack stays, naming its arguments.
            ArgLookup::Arg(a)
                if self.pack.keep
                    && self.pack_index.is_none()
                    && matches!(a.view(), ArgView::Pack(_)) =>
            {
                let ArgView::Pack(l) = a.view() else { return Err(SubstFail) };
                let ty = self
                    .sema
                    .ast
                    .types
                    .args(l)
                    .iter()
                    .find_map(|x| match x.view() {
                        ArgView::Value { ty, .. } => Some(ty),
                        ArgView::Expr { expr, .. } => Some(self.sema.ast.exprs.get(expr).ty()),
                        _ => None,
                    })
                    .or_else(|| self.sema.ast.decls.get(p).ty())
                    .ok_or(SubstFail)?;
                let shown = self.shown_nttp(info.depth, info.index).unwrap_or(p);
                return Ok(self.sema.ast.builder().subst_nttp_pack_expr(shown, l, ty, loc));
            }
            ArgLookup::Arg(a) => self.element(a, Some(p), loc)?,
            ArgLookup::Retain(d) if d == info.depth => return Ok(e),
            // A parameter of a member template redeclared one level
            // shallower ([temp.mem]; §4.6).
            ArgLookup::Retain(_) => {
                let np = self.find_instantiated_decl(p, loc)?;
                if np == p {
                    return self.unimplemented("member templates", loc);
                }
                return Ok(self.sema.build_decl_ref(np, loc));
            }
            ArgLookup::Missing => return Err(SubstFail),
        };
        let shown = self.shown_nttp(info.depth, info.index).unwrap_or(p);
        self.nttp_replacement(a, shown, loc)
    }

    /// The parameter of the level's template (an instantiated member
    /// template's own parameter, as clang) for `(depth, index)`.
    fn shown_nttp(&self, depth: u16, index: u16) -> Option<DeclId> {
        self.assoc(depth)
            .and_then(|a| self.sema.ast.template_info(a).copied())
            .and_then(|i| {
                let l = *self.sema.ast.templates.param_list(i.params);
                self.sema.ast.lists.iter::<DeclId>(l.params).nth(usize::from(index))
            })
            .filter(|&x| self.sema.kind_of(x) == DeclKind::NonTypeTemplateParm)
    }

    /// The argument `a` of the non-type parameter `shown`, wrapped in
    /// `SubstNonTypeTemplateParmExpr`.
    fn nttp_replacement(&mut self, a: TemplateArg, shown: DeclId, loc: Loc) -> SubstResult<ExprId> {
        let r = match a.view() {
            ArgView::Value { ty, value } => self.value_literal(ty, value, loc)?,
            ArgView::Expr { expr, .. } => expr,
            // A canonical value-dependent argument (a dependent default
            // argument converted for a partial substitution): its first
            // expression stands for it, as in `template_arg` (§5.1).
            ArgView::Profile(p) => self.sema.ast.types.profile_expr(p).ok_or(SubstFail)?,
            _ => return self.unimplemented("non-type template argument of this kind", loc),
        };
        Ok(self.sema.ast.builder().subst_nttp_expr(shown, r, loc))
    }

    /// A non-type parameter pack substituted earlier while its expansion
    /// stayed dependent (§7.2): its element for the current index, or
    /// itself while another partial expansion is rebuilt.
    fn subst_nttp_pack(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        if self.pack.keep && self.pack_index.is_none() {
            return Ok(e);
        }
        let p = self.sema.ast.exprs.subst_pack_parameter(e);
        let l = self.sema.ast.exprs.subst_pack_args(e);
        let n = u32::try_from(self.sema.ast.types.args(l).len()).unwrap_or(u32::MAX);
        // Unnamed: an outer pack in the length-mismatch diagnostic.
        let i = self.pack_reached(n, |_| crate::templ::pack::PackName::outer(), loc)?;
        let a = self.sema.ast.types.args(l)[i as usize];
        self.nttp_replacement(a, p, loc)
    }

    /// Rebuilding a partially substituted expansion (§7.2): a reference
    /// `e` to the function parameter pack `d`, instantiated as a list of
    /// parameters, stays a pack as a `FunctionParmPackExpr` naming them
    /// (clang's `TransformFunctionParmPackRefExpr` with no pack index).
    /// Each parameter is captured by the enclosing lambdas as a reference
    /// to it would be (clang's `MarkFunctionParmPackReferenced`). `None`
    /// outside keep mode or for any other declaration.
    fn kept_parm_pack(&mut self, e: ExprId, d: DeclId, loc: Loc) -> Option<ExprId> {
        if !self.pack.keep || self.pack_index.is_some() {
            return None;
        }
        let Some(&super::DeclOrPack::Pack(l)) = self.locals.get(&d) else { return None };
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l).collect();
        for p in params {
            let _ = self.sema.capture_at_decl_ref(p, loc);
        }
        let ty = self.sema.ast.exprs.get(e).ty();
        Some(self.sema.ast.builder().function_parm_pack_expr(d, l, ty, loc))
    }

    /// A function parameter pack kept while its expansion stayed
    /// dependent (§7.2): a reference to its parameter for the current
    /// index, or itself while another partial expansion is rebuilt.
    fn subst_parm_pack(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        if self.pack.keep && self.pack_index.is_none() {
            return Ok(e);
        }
        let d = self.sema.ast.exprs.parm_pack_parameter(e);
        let l = self.sema.ast.exprs.parm_pack_params(e);
        let n = u32::try_from(self.sema.ast.lists.len(l)).unwrap_or(u32::MAX);
        let i = self.pack_reached(
            n,
            |s| crate::templ::pack::PackName {
                name: s.sema.decl_name(d),
                depth: None,
                outer: true,
                partial: false,
            },
            loc,
        )?;
        let p = self.sema.ast.lists.at::<DeclId>(l, i as usize).ok_or(SubstFail)?;
        Ok(self.sema.build_decl_ref(p, loc))
    }

    /// The literal for a non-type template argument value
    /// (clang's `BuildExpressionFromNonTypeTemplateArgument`).
    fn value_literal(
        &mut self,
        ty: QualType,
        value: kiln_ast::ConstValueId,
        loc: Loc,
    ) -> SubstResult<ExprId> {
        match self.sema.nttp_arg_expr(ty, value, loc) {
            Some(e) => Ok(e),
            None => self.unimplemented("non-type template argument of this type", loc),
        }
    }

    pub(crate) fn int_literal(&mut self, ty: QualType, v: i128, loc: Loc) -> ExprId {
        self.sema.nttp_int_literal(ty, v, loc)
    }

    /// An initializer-clause of a braced list; a `DesignatedInitExpr`
    /// gives back its designators (clang `TransformDesignatedInitExpr`).
    fn init_value(&mut self, x: ExprId) -> SubstResult<InitItem> {
        let (designator, x) = if self.sema.ast.exprs.get(x).kind() == ExprKind::DesignatedInitExpr {
            let ds = self.sema.ast.exprs.designators(x).to_vec();
            let init = self.sema.ast.exprs.designated_init(x);
            (self.designators(&ds)?, init)
        } else {
            (Vec::new(), x)
        };
        // A nested braced-init-list, also when its element was analyzed
        // into a list-initializing construction (`S a[2] = {{x, 1}}`,
        // [dcl.init.aggr]/4.2 with [dcl.init.list]/3.7): the list again.
        let value = match self.braced_source(x) {
            Some(b) => InitValue::List(self.braced_list(b)?),
            None => InitValue::Expr(self.expr(x)?),
        };
        Ok(InitItem { designator, value })
    }

    /// The designators of a clause, array indices substituted.
    fn designators(
        &mut self,
        ds: &[kiln_ast::expr::Designator],
    ) -> SubstResult<Vec<crate::init::Designator>> {
        use crate::init::Designator as D;
        use kiln_ast::expr::Designator as A;
        let mut out = Vec::with_capacity(ds.len());
        for d in ds {
            out.push(match *d {
                A::Field { name, loc } => D::Field(name, loc),
                A::Index { index, loc } => D::Index(self.expr(index)?, loc),
                A::Range { first, last, loc, ellipsis } => {
                    D::Range(self.expr(first)?, self.expr(last)?, loc, ellipsis)
                }
            });
        }
        Ok(out)
    }

    /// The designators of a written clause, array indices substituted.
    fn written_designators(
        &mut self,
        ds: &[crate::init::Designator],
    ) -> SubstResult<Vec<crate::init::Designator>> {
        let v: Vec<kiln_ast::expr::Designator> =
            ds.iter().map(crate::init::Designator::to_ast).collect();
        self.designators(&v)
    }

    /// The initializer-clauses `items` of a braced list, pack expansions
    /// expanded ([temp.variadic]/5).
    fn init_items(&mut self, items: &[ExprId]) -> SubstResult<Vec<InitItem>> {
        let mut out = Vec::with_capacity(items.len());
        for &x in items {
            match crate::templ::pack::expr_pattern(self.sema, x) {
                Some(p)
                    if matches!(
                        self.sema.ast.exprs.get(p).kind(),
                        ExprKind::DesignatedInitExpr | ExprKind::InitListExpr
                    ) =>
                {
                    // `.m = e...`: one designated clause per element
                    // (clang expands the whole `DesignatedInitExpr`);
                    // `{a, b}...`: one braced-init-list per element
                    // ([temp.variadic]/5.4).
                    let ellipsis = self.sema.ast.exprs.get(x).loc();
                    match self.expand_pattern_at(Some(ellipsis), |s| s.init_value(p))? {
                        crate::templ::pack::Expansion::Elements(v) => out.extend(v),
                        crate::templ::pack::Expansion::Unexpanded(mut it) => {
                            it.value = InitValue::Expr(match it.value {
                                InitValue::Expr(y) => self.sema.act_pack_expansion(y, ellipsis),
                                InitValue::List(l) => {
                                    self.sema.act_pack_expansion_list(&l, ellipsis)
                                }
                            });
                            out.push(it);
                        }
                    }
                }
                Some(_) => {
                    for y in self.expand_exprs(&[x])? {
                        out.push(InitItem { designator: Vec::new(), value: InitValue::Expr(y) });
                    }
                }
                None => out.push(self.init_value(x)?),
            }
        }
        Ok(out)
    }

    /// A binary operator of the pattern. A braced right operand
    /// (`x = {...}`, [expr.assign]/8) is assigned again from the
    /// rebuilt list ([`crate::Sema::act_on_assign_list`]).
    fn binary(&mut self, op: BinaryOp, l: ExprId, r: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let l = self.expr(l)?;
        if op.is_assignment() && self.sema.ast.exprs.get(r).kind() == ExprKind::InitListExpr {
            let il = self.init_list(r)?;
            return Ok(self.sema.act_on_assign_list(op, l, &il, loc));
        }
        let r = self.expr(r)?;
        Ok(self.sema.act_on_binary(op, l, r, loc))
    }

    /// A braced-init-list of the pattern. A semantic list built from a
    /// designated list is rebuilt from its written form (spec/ast.md
    /// §5.6), as clang's `TransformInitListExpr` uses the syntactic form.
    pub(crate) fn init_list(&mut self, e: ExprId) -> SubstResult<InitList> {
        if let Some(w) = self.sema.ast.exprs.syntactic_list(e) {
            self.note_default_uses(e);
            return self.init_list(w);
        }
        let n = *self.sema.ast.exprs.get(e);
        let l = self.sema.ast.exprs.init_list_inits(e);
        let mut items: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
        // The default member initializers and value-initializations
        // filling the trailing fields are implicit (clang keeps them out
        // of the syntactic form): the rebuilt list fills them again, the
        // default member initializers in its own context (for the
        // source-location builtins in them). An `ImplicitValueInitExpr`
        // fed back as a written clause would initialize an array member
        // by brace elision from an expression of the array's own type
        // (`S s{};` with `char path[108]`, [dcl.init.aggr]/5; WP-613).
        // A written `{}` is an `InitListExpr`, never one of these; a
        // filler before a written clause only comes from designators,
        // whose list is rebuilt from its syntactic form above.
        while items.last().is_some_and(|&x| {
            matches!(
                self.sema.ast.exprs.get(x).kind(),
                ExprKind::CXXDefaultInitExpr | ExprKind::ImplicitValueInitExpr
            )
        }) {
            items.pop();
        }
        let out = self.init_items(&items)?;
        let rbrace = self.sema.ast.exprs.end_loc(e).unwrap_or(n.loc());
        // A default member initializer the semantic list used is used
        // again by the instantiation ([expr.const]/16).
        for &x in &items {
            if self.sema.ast.exprs.get(x).kind() == ExprKind::CXXDefaultInitExpr {
                let f = self.sema.ast.exprs.default_init_field(x);
                self.sema.note_default_use(x, f, rbrace);
            }
        }
        Ok(InitList { items: out, lbrace: n.loc(), rbrace })
    }

    /// [`Substituter::init_list`]'s default-member-initializer uses of
    /// the semantic list `e` (all of its elements, nested lists too).
    fn note_default_uses(&mut self, e: ExprId) {
        let rbrace = self.sema.ast.exprs.end_loc(e).unwrap_or(self.sema.ast.exprs.get(e).loc());
        let l = self.sema.ast.exprs.init_list_inits(e);
        let items: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
        for x in items {
            match self.sema.ast.exprs.get(x).kind() {
                ExprKind::CXXDefaultInitExpr => {
                    let f = self.sema.ast.exprs.default_init_field(x);
                    self.sema.note_default_use(x, f, rbrace);
                }
                ExprKind::InitListExpr => self.note_default_uses(x),
                _ => {}
            }
        }
    }

    /// A braced-init-list as the parser built it (a mem-initializer's).
    fn written_list(&mut self, l: &InitList) -> SubstResult<InitList> {
        /// The elements of the expansion `{a, b}...` (`e`), each a
        /// braced-init-list item with `designator`.
        fn out_lists(
            s: &mut Substituter<'_, '_, '_>,
            e: ExprId,
            designator: &[crate::init::Designator],
            items: &mut Vec<InitItem>,
        ) -> SubstResult<()> {
            let Some(p) = s.braced_pattern(e) else { return Ok(()) };
            let ellipsis = s.sema.ast.exprs.get(e).loc();
            match s.expand_pattern_at(Some(ellipsis), |s| s.init_list(p))? {
                crate::templ::pack::Expansion::Elements(ls) => {
                    for n in ls {
                        items.push(InitItem {
                            designator: designator.to_vec(),
                            value: InitValue::List(n),
                        });
                    }
                }
                crate::templ::pack::Expansion::Unexpanded(n) => {
                    let value = InitValue::Expr(s.sema.act_pack_expansion_list(&n, ellipsis));
                    items.push(InitItem { designator: designator.to_vec(), value });
                }
            }
            Ok(())
        }
        let mut items = Vec::with_capacity(l.items.len());
        for it in &l.items {
            let designator = self.written_designators(&it.designator)?;
            let value = match &it.value {
                InitValue::Expr(e) if self.braced_pattern(*e).is_some() => {
                    // `{a, b}...`: one braced-init-list per element.
                    out_lists(self, *e, &designator, &mut items)?;
                    continue;
                }
                InitValue::Expr(e) if crate::templ::pack::expr_pattern(self.sema, *e).is_some() => {
                    // `.m = e...` repeats the designators per element.
                    for y in self.expand_exprs(&[*e])? {
                        items.push(InitItem {
                            designator: designator.clone(),
                            value: InitValue::Expr(y),
                        });
                    }
                    continue;
                }
                InitValue::Expr(e) => InitValue::Expr(self.written_arg(*e)?),
                InitValue::List(n) => InitValue::List(self.written_list(n)?),
            };
            items.push(InitItem { designator, value });
        }
        Ok(InitList { items, lbrace: l.lbrace, rbrace: l.rbrace })
    }

    fn written_arg(&mut self, e: ExprId) -> SubstResult<ExprId> {
        if self.sema.ast.exprs.get(e).kind() == ExprKind::PackExpansionExpr {
            let loc = self.sema.ast.exprs.get(e).loc();
            return self.unimplemented("pack expansion", loc);
        }
        self.expr(e)
    }

    /// Written arguments, pack expansions expanded; a braced argument
    /// (`m(v, {v + 1})`, `m(a..., {a..., 7})`) is rebuilt as a braced
    /// argument as in a call ([class.base.init]/7, WP-445).
    fn written_args(&mut self, v: &[ExprId]) -> SubstResult<Vec<ExprId>> {
        if v.iter().any(|&x| self.braced_source(x).is_some() || self.braced_pattern(x).is_some()) {
            return self.braced_args(v.to_vec());
        }
        self.expand_exprs(v)
    }

    /// A mem-initializer of a constructor of the pattern, as the parser
    /// handed it to `act_on_mem_initializer` (clang
    /// `InstantiateMemInitializers`): the id's type and the initializer's
    /// expressions are substituted; the id is resolved again in the
    /// instantiated class.
    /// A pack expansion `B(args)...` gives one mem-initializer per
    /// element ([temp.variadic]/5).
    pub(crate) fn mem_initializer(
        &mut self,
        info: &crate::class::MemInitInfo,
    ) -> SubstResult<Vec<crate::class::MemInitInfo>> {
        if !info.pack_expansion {
            return Ok(vec![self.mem_initializer_one(info)?]);
        }
        match self.expand_pattern(|s| s.mem_initializer_one(info))? {
            crate::templ::pack::Expansion::Elements(v) => Ok(v),
            crate::templ::pack::Expansion::Unexpanded(_) => self
                .unimplemented("a mem-initializer pack expansion of an enclosing pack", info.loc),
        }
    }

    fn mem_initializer_one(
        &mut self,
        info: &crate::class::MemInitInfo,
    ) -> SubstResult<crate::class::MemInitInfo> {
        use crate::class::MemInitId;
        let id = match info.id {
            MemInitId::Ident(s) => MemInitId::Ident(s),
            MemInitId::Type(t) => MemInitId::Type(self.ty(t, info.loc)?),
        };
        let init = match &info.init {
            Initializer::Expr(e) => Initializer::Expr(self.written_arg(*e)?),
            Initializer::Paren(v, l, r) => Initializer::Paren(self.written_args(v)?, *l, *r),
            Initializer::List(l) => Initializer::List(self.written_list(l)?),
        };
        Ok(crate::class::MemInitInfo { id, loc: info.loc, init, pack_expansion: false })
    }

    /// The initializer of a variable of the pattern, in the form the
    /// parser hands to `act_on_init` (§5.6).
    /// `None`: a default-initialized variable (its pattern initializer is
    /// the implicit `CXXConstructExpr` of [dcl.init.general]/7).
    pub(crate) fn initializer(
        &mut self,
        init: ExprId,
        style: kiln_ast::decl::InitStyle,
    ) -> SubstResult<Option<Initializer>> {
        use kiln_ast::decl::InitStyle;
        // clang `TransformInitializer`: strip the implicit full-expression
        // and temporary nodes; a construction (or a parenthesized
        // aggregate initialization) other than copy-initialization goes
        // back to its parenthesized or braced list of arguments, and the
        // initialization is redone for the instantiated variable.
        let peeled = self.peel_initializer(init);
        let pk = self.sema.ast.exprs.get(peeled).kind();
        // An array decomposition's element-wise copy: redone from the
        // array (clang `TransformInitializer` takes its common
        // expression's source).
        if pk == ExprKind::ArrayInitLoopExpr {
            let ex = &self.sema.ast.exprs;
            let [common, _] = ex.array_init_loop(peeled);
            let src = ex.opaque_source(common).ok_or(SubstFail)?;
            let at = ex.get(src).loc();
            let e = self.expr(src)?;
            return Ok(Some(match style {
                InitStyle::Call | InitStyle::ParenList => Initializer::Paren(vec![e], at, at),
                InitStyle::List => Initializer::List(InitList {
                    items: vec![InitItem { designator: Vec::new(), value: InitValue::Expr(e) }],
                    lbrace: at,
                    rbrace: at,
                }),
                InitStyle::C => Initializer::Expr(e),
            }));
        }
        let list_construct =
            pk == ExprKind::CXXConstructExpr && self.sema.ast.exprs.construct_info(peeled).list;
        if pk == ExprKind::CXXConstructExpr && (style != InitStyle::C || list_construct) {
            let pn = *self.sema.ast.exprs.get(peeled);
            let end = self.sema.ast.exprs.end_loc(peeled);
            if list_construct {
                let l = self.construct_list(peeled, pn.loc(), end.unwrap_or(pn.loc()))?;
                return Ok(Some(Initializer::List(l)));
            }
            // A single element of the class (or a derived class) in braces
            // initializes the object from that element ([dcl.init.list]/3.2):
            // a construction with no stored braces, redone as a list.
            if style == InitStyle::List && self.written_construct_args(peeled) {
                let l = self.construct_list(peeled, pn.loc(), pn.loc())?;
                return Ok(Some(Initializer::List(l)));
            }
            // No parentheses and no written argument: the variable was
            // default-initialized.
            if end.is_none() && !self.written_construct_args(peeled) {
                return Ok(None);
            }
            // A braced argument (`P p({x, 1}, y)`) stays one: the
            // constructor's parameter is list-initialized from it again.
            let args = self.call_arg_list(self.sema.ast.exprs.construct_args(peeled))?;
            let r = end.unwrap_or(pn.loc());
            return Ok(Some(Initializer::Paren(args, pn.loc(), r)));
        }
        if pk == ExprKind::CXXParenListInitExpr && style != InitStyle::C {
            let pn = *self.sema.ast.exprs.get(peeled);
            let args = self.expr_list(self.sema.ast.exprs.paren_list_inits(peeled))?;
            let r = self.sema.ast.exprs.end_loc(peeled).unwrap_or(pn.loc());
            return Ok(Some(Initializer::Paren(args, pn.loc(), r)));
        }
        // `std::initializer_list<E> x = {…}` (WP-106): redone from the list.
        if pk == ExprKind::InitListExpr
            && self.sema.ast.types.is_array(self.sema.ety(peeled))
            && self.sema.ast.types.as_record(self.sema.ety(init)).is_some()
        {
            return Ok(Some(Initializer::List(self.init_list(peeled)?)));
        }
        // A list whose elements bind temporaries is under its
        // full-expression (`T a[] = {{1}}` with `T(const int&)`).
        let init = match self.sema.ast.exprs.get(init).kind() {
            ExprKind::ExprWithCleanups
                if matches!(
                    self.sema.ast.exprs.get(self.sema.ast.exprs.operand(init)).kind(),
                    ExprKind::InitListExpr | ExprKind::ParenListExpr
                ) =>
            {
                self.sema.ast.exprs.operand(init)
            }
            _ => init,
        };
        let n = *self.sema.ast.exprs.get(init);
        Ok(Some(match n.kind() {
            // A braced-init-list argument stays a braced argument, which
            // the constructor's parameter is list-initialized from
            // ([over.ics.list], `C c({1, 2}, a)` with `a` dependent).
            ExprKind::ParenListExpr => {
                let l = kiln_ast::ids::ListId::new(n.a());
                let args = self.call_arg_list(l)?;
                let r = self.sema.ast.exprs.end_loc(init).unwrap_or(n.loc());
                Initializer::Paren(args, n.loc(), r)
            }
            // `= {…}` and `{…}` alike: list-initialization ([dcl.init.list]/1).
            ExprKind::InitListExpr => Initializer::List(self.init_list(init)?),
            // `T x(args...)`: the one written argument is an expansion.
            ExprKind::PackExpansionExpr => {
                let args = self.expand_exprs(&[init])?;
                let r = self.sema.ast.exprs.end_loc(init).unwrap_or(n.loc());
                Initializer::Paren(args, n.loc(), r)
            }
            _ => {
                // clang `TransformInitializer` strips the full-expression
                // even of a non-dependent initializer shared with the
                // pattern: the instantiated variable's conversion goes
                // inside it (`template <class T> constexpr T v = f().m;`
                // keeps `f()`'s temporary alive through the copy of `m`).
                let src = match n.kind() {
                    ExprKind::ExprWithCleanups => self.sema.ast.exprs.operand(init),
                    _ => init,
                };
                let e = self.expr(src)?;
                // A braced element of an expansion-init-list: `decl =
                // {…};` ([stmt.expand]/5.1).
                if pk == ExprKind::CXXExpansionSelectExpr && self.is_braced_element(e) {
                    return Ok(Some(Initializer::List(self.init_list(e)?)));
                }
                match style {
                    InitStyle::Call | InitStyle::ParenList => {
                        let r = self.sema.ast.exprs.end_loc(init).unwrap_or(n.loc());
                        Initializer::Paren(vec![e], n.loc(), r)
                    }
                    _ => Initializer::Expr(e),
                }
            }
        }))
    }

    /// The object argument of the conversion function call an implicit
    /// `UserDefinedConversion` cast `e` wraps (seen through the
    /// temporaries around the call), or `None` for any other cast: the
    /// operand as written, whose conversion the enclosing action forms
    /// again (clang `CastExpr::getSubExprAsWritten`; WP-563).
    pub(crate) fn udc_object(&self, e: ExprId) -> Option<ExprId> {
        let ex = &self.sema.ast.exprs;
        if ex.get(e).kind() != ExprKind::ImplicitCastExpr
            || ex.cast_kind(e) != CastKind::UserDefinedConversion
        {
            return None;
        }
        let mut x = ex.operand(e);
        while matches!(
            ex.get(x).kind(),
            ExprKind::MaterializeTemporaryExpr | ExprKind::CXXBindTemporaryExpr
        ) {
            x = ex.operand(x);
        }
        match ex.get(x).kind() {
            ExprKind::CXXMemberCallExpr
                if ex.get(ex.callee(x)).kind() == ExprKind::MemberExpr
                    && self.sema.kind_of(ex.decl(ex.callee(x))) == DeclKind::CXXConversion =>
            {
                Some(ex.member_call_object(x))
            }
            _ => None,
        }
    }

    /// The initializer `init` without the implicit nodes clang's
    /// `TransformInitializer` strips: the full-expression, a
    /// materialization, temporary bindings and an implicit conversion.
    pub(crate) fn peel_initializer(&self, mut init: ExprId) -> ExprId {
        let ex = &self.sema.ast.exprs;
        if ex.get(init).kind() == ExprKind::ExprWithCleanups {
            init = ex.operand(init);
        }
        if ex.get(init).kind() == ExprKind::MaterializeTemporaryExpr {
            init = ex.operand(init);
        }
        while ex.get(init).kind() == ExprKind::CXXBindTemporaryExpr {
            init = ex.operand(init);
        }
        if ex.get(init).kind() == ExprKind::ImplicitCastExpr {
            init = self.udc_object(init).unwrap_or_else(|| ex.operand(init));
        }
        // `std::initializer_list` over its backing array (WP-106).
        if ex.get(init).kind() == ExprKind::CXXStdInitializerListExpr {
            init = ex.operand(ex.operand(init));
        }
        init
    }

    /// A `CXXConstructExpr` that must be rebuilt ([class.ctor], [dcl.init];
    /// spec/sema-templates.md §5.4). As an implicit node it is stripped:
    /// with one argument it is the conversion of that argument, which the
    /// enclosing action redoes. A list-initializing one is the
    /// copy-list-initialization of its (substituted) class from the list
    /// again ([dcl.init.list]/3.7; a braced initializer of an aggregate
    /// element, of a call argument or of a variable is rebuilt as a list by
    /// its owner, [`Self::braced_source`]). Other forms appear only as
    /// initializers (see [`Self::initializer`]).
    fn construct_expr(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let ast = &self.sema.ast;
        let ex = &ast.exprs;
        if ex.construct_info(e).list {
            let ty = self.ty(ex.get(e).ty(), loc)?;
            let end = self.sema.ast.exprs.end_loc(e).unwrap_or(loc);
            let l = self.construct_list(e, loc, end)?;
            return Ok(self.sema.init_from_list_at(ty, &l, Some(loc)));
        }
        let args: Vec<ExprId> = ast
            .lists
            .iter::<ExprId>(ex.construct_args(e))
            .take_while(|&a| ex.get(a).kind() != ExprKind::CXXDefaultArgExpr)
            .collect();
        if let [a] = args.as_slice() {
            return self.expr(*a);
        }
        // No or several written arguments: the constructor call is
        // direct-initialization of the (substituted) class from the
        // substituted arguments, redone ([dcl.init.general]/16.6.2; clang
        // `TransformCXXConstructExpr` rebuilds the call). With none, a
        // zero-initializing one is value-initialization
        // ([dcl.init.general]/9), else default-initialization.
        let (pty, zeroing, end) = (ex.get(e).ty(), ex.construct_info(e).zeroing, ex.end_loc(e));
        let ty = self.ty(pty, loc)?;
        let args = self.call_arg_list(self.sema.ast.exprs.construct_args(e))?;
        if args.is_empty() && zeroing {
            return Ok(self
                .sema
                .value_init_class(ty, loc)
                .unwrap_or_else(|| self.sema.recover(ty, &[], loc)));
        }
        let mut site = crate::class::ConstructSite::at(loc);
        site.rparen = end;
        Ok(self
            .sema
            .construct_class(ty, &args, crate::class::InitStyle::Direct, site)
            .unwrap_or_else(|| self.sema.recover(ty, &args, loc)))
    }

    /// Whether construct expression `e` has a written argument (one that
    /// is not a default argument).
    fn written_construct_args(&self, e: ExprId) -> bool {
        let ast = &self.sema.ast;
        ast.lists
            .iter::<ExprId>(ast.exprs.construct_args(e))
            .next()
            .is_some_and(|a| ast.exprs.get(a).kind() != ExprKind::CXXDefaultArgExpr)
    }

    /// The braced list of list-initializing construct expression `e`
    /// (clang `TransformInitializer` reverting it to syntactic form).
    pub(crate) fn construct_list(
        &mut self,
        e: ExprId,
        lbrace: Loc,
        rbrace: Loc,
    ) -> SubstResult<InitList> {
        let items: Vec<ExprId> = self
            .sema
            .ast
            .lists
            .iter::<ExprId>(self.sema.ast.exprs.construct_args(e))
            .take_while(|&a| self.sema.ast.exprs.get(a).kind() != ExprKind::CXXDefaultArgExpr)
            .collect();
        let out = self.init_items(&items)?;
        Ok(InitList { items: out, lbrace, rbrace })
    }
}

impl Substituter<'_, '_, '_> {
    /// Whether the substituted `T{...}` names a class whose definition is
    /// invalid (a failed `static_assert` in its instantiation, say): the
    /// expression is then dropped without a second diagnostic, as clang
    /// rebuilds no `CXXUnresolvedConstructExpr` list-initialization of an
    /// invalid class (`std::numbers::e_v<int>` reports only the
    /// `static_assert`, spec/sema.md §5.22).
    fn invalid_class_braced_cast(&mut self, ty: QualType, loc: Loc) -> bool {
        let Some(r) = self.sema.ast.types.as_record(ty) else { return false };
        let _ = self.sema.require_complete(ty, loc);
        let def = self.sema.ast.definition(r).unwrap_or(r);
        self.sema.ast.decls.get(def).has(kiln_ast::decl::DeclFlags::INVALID)
    }
}

impl Substituter<'_, '_, '_> {
    /// The operand of `^^` as GCC 16.1 instantiates it in the body of an
    /// expansion statement (spec/reflection.md §2.3): its tsubst treats
    /// that body as a template definition, finds no local specialization
    /// of a type alias declared outside every template, and drops one
    /// level of the alias (gcc/cp/pt.cc `tsubst`, "We don't have an
    /// instantiation yet, so drop the typedef"). So inside `template for`
    /// `^^std::meta::info` is `^^decltype(^^int)`, and code comparing a
    /// reflected value's `type_of` with it takes that branch. An alias
    /// declared in a template (a member of a class template, a local of
    /// a function template or of the body itself) is kept, as are alias
    /// template specializations and every alias outside such a body.
    /// Whether `e`, though not dependent, is a `^^` whose alias operand
    /// [`Self::expansion_dealias`] drops, so it is formed again. Only a
    /// `^^` reached as its own operand is: a non-dependent expression
    /// containing one deeper is still shared (GCC rebuilds that too).
    fn dealiased_in_expansion(&self, e: ExprId) -> bool {
        if self.expansions.is_empty()
            || self.sema.ast.exprs.get(e).kind() != ExprKind::CXXReflectExpr
        {
            return false;
        }
        match self.sema.ast.exprs.reflect_operand(e) {
            Reflectee::Type(q) => self.expansion_dealias(q) != q,
            _ => false,
        }
    }

    fn expansion_dealias(&self, q: QualType) -> QualType {
        if self.expansions.is_empty() {
            return q;
        }
        let ast = &self.sema.ast;
        let kiln_ast::types::TypeView::Typedef { decl, .. } = ast.types.view(q.ty()) else {
            return q;
        };
        if ast.decls.get(decl).has(kiln_ast::decl::DeclFlags::DEPENDENT_CTX) {
            return q;
        }
        // The aliased type as written (`using B = A;` drops to `A`).
        let Some(underlying) = ast.decls.typedef(decl).map(|t| t.underlying) else {
            return q;
        };
        QualType::new(underlying.ty(), underlying.quals_bits() | q.quals_bits())
    }
}
