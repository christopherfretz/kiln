//! Substitution into expressions (spec/sema-templates.md §5.4): an
//! expression that is not type-, value- or instantiation-dependent and
//! has no unexpanded pack is shared with the pattern; otherwise the
//! implicit nodes semantic analysis added are stripped and the
//! expression is rebuilt through the Sema actions the parser calls.

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{CastKind, CharKind, ExprDeps, ExprFlags, ExprKind, Radix, ValueCategory};
use kiln_ast::ids::RawId;
use kiln_ast::types::{ArgView, BuiltinType, Nns};
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

    /// Substitutes into `e`.
    pub fn expr(&mut self, e: ExprId) -> SubstResult<ExprId> {
        if !self.needs_rebuild(e) {
            return Ok(e);
        }
        self.check_stack(|s| s.sema.ast.exprs.get(e).loc())?;
        let r = self.expr_node(e)?;
        if self.sema.is_err_expr(r) { Err(SubstFail) } else { Ok(r) }
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
                let i = self.find_instantiated_decl(d, loc)?;
                // The qualifier as written survives ([temp.inst]; clang's
                // `TransformDeclRefExpr` transforms the qualifier loc).
                let nns = match ex(self).decl_ref_extra(e).and_then(|x| x.qualifier) {
                    Some(q) => Some(self.nns(q, loc)?),
                    None => None,
                };
                let r = self.sema.build_decl_ref_qualified(i, loc, None, nns);
                if nns.is_some()
                    && let Some(b) = self.sema.ast.templates.expr_begin.get(e).copied()
                    && self.sema.ast.exprs.get(r).kind() == ExprKind::DeclRefExpr
                {
                    let _ = self.sema.ast.templates.expr_begin.insert(r, b);
                }
                Ok(r)
            }
            ExprKind::ParenExpr => {
                let o = ex(self).operand(e);
                let o = self.expr(o)?;
                Ok(self.sema.act_on_paren(o, loc, end))
            }
            ExprKind::UnaryOperator => {
                let op = ex(self).unary_op(e);
                let o = ex(self).operand(e);
                let o = self.expr(o)?;
                Ok(self.sema.act_on_unary(op, o, loc))
            }
            ExprKind::BinaryOperator => {
                let op = ex(self).binary_op(e);
                let [l, r] = ex(self).operands(e);
                let l = self.expr(l)?;
                let r = self.expr(r)?;
                Ok(self.sema.act_on_binary(op, l, r, loc))
            }
            ExprKind::CompoundAssignOperator => {
                let op = ex(self).binary_op(e);
                let l = ExprId::new(n.a());
                let r = ex(self).compound_assign(e).rhs;
                let l = self.expr(l)?;
                let r = self.expr(r)?;
                Ok(self.sema.act_on_binary(op, l, r, loc))
            }
            ExprKind::ConditionalOperator => {
                let c = ExprId::new(n.a());
                let [t, f] = ex(self).cond_arms(e);
                let c = self.expr(c)?;
                let t = self.expr(t)?;
                let f = self.expr(f)?;
                Ok(self.sema.act_on_conditional(c, t, f, loc))
            }
            ExprKind::ArraySubscriptExpr => {
                let [b, i] = ex(self).operands(e);
                let b = self.expr(b)?;
                let i = self.expr(i)?;
                Ok(self.sema.act_on_subscript(b, i, loc))
            }
            ExprKind::CallExpr | ExprKind::CXXMemberCallExpr => {
                let callee = ex(self).callee(e);
                let args = ex(self).call_args(e);
                let callee = self.callee(callee)?;
                let args = self.expr_list(args)?;
                Ok(self.sema.act_on_call(callee, &args, loc, end))
            }
            ExprKind::CXXOperatorCallExpr => {
                use crate::overload::oper::{binary_of, unary_of};
                use kiln_ast::names::OverloadedOperator as OO;
                let op = ex(self).operator_call_op(e);
                let args = ex(self).call_args(e);
                let args = self.expr_list(args)?;
                match (op, args.as_slice()) {
                    (OO::Call, [f, rest @ ..]) => Ok(self.sema.act_on_call(*f, rest, loc, end)),
                    (OO::Subscript, [b, rest @ ..]) => {
                        Ok(self.sema.act_on_subscript_args(*b, rest, loc, end))
                    }
                    (_, &[a]) | (OO::PlusPlus | OO::MinusMinus, &[a, _]) => {
                        match unary_of(op, args.len() == 2) {
                            Some(u) => Ok(self.sema.act_on_unary(u, a, loc)),
                            None => self.unimplemented("this overloaded operator", loc),
                        }
                    }
                    (_, &[l, r]) => match binary_of(op) {
                        Some(b) => Ok(self.sema.act_on_binary(b, l, r, loc)),
                        None => self.unimplemented("this overloaded operator", loc),
                    },
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
            ExprKind::UnresolvedLookupExpr => self.callee(e),
            ExprKind::MemberExpr => {
                let base = ex(self).operand(e);
                let m = ex(self).decl(e);
                let arrow = n.has(ExprFlags::ARROW);
                // `x->m` through `operator->`: the chain is formed again
                // from the substituted `x` ([over.ref]/1).
                let mut base = base;
                while arrow
                    && ex(self).get(base).kind() == ExprKind::CXXOperatorCallExpr
                    && ex(self).operator_call_op(base) == kiln_ast::names::OverloadedOperator::Arrow
                {
                    let l = ex(self).call_args(base);
                    match self.sema.ast.lists.iter::<ExprId>(l).next() {
                        Some(x) => base = x,
                        None => break,
                    }
                }
                let base = self.member_base(base)?;
                let Some(name) = self.sema.ast.decls.get(m).name().as_ident() else {
                    return self.unimplemented("member access to a special member", loc);
                };
                // A member template-id `x.f<args>` (§6.4).
                let targs = match self.sema.ovl.members.get(&e).and_then(|m| m.targs) {
                    Some(l) => Some(self.args(l, loc)?),
                    None => None,
                };
                let op = self.sema.ast.templates.member_op_loc.get(e).copied().unwrap_or(loc);
                Ok(self.sema.act_on_template_member(base, name, targs, false, loc, op, arrow))
            }
            ExprKind::CXXDependentScopeMemberExpr => {
                let Some(base) = ExprId::from_raw(n.a()) else {
                    return self.unimplemented("implicit dependent member access", loc);
                };
                let name = kiln_source::symbol::Symbol::from_index(n.b());
                let arrow = n.aux() & 1 != 0;
                let base = self.member_base(base)?;
                let targs = match self.sema.ast.templates.expr_targs.get(e).copied() {
                    Some(l) => Some(self.args(l, loc)?),
                    None => None,
                };
                let kw = n.aux() & 2 != 0;
                let op = self.sema.ast.templates.member_op_loc.get(e).copied().unwrap_or(loc);
                Ok(self.sema.act_on_template_member(base, name, targs, kw, loc, op, arrow))
            }
            ExprKind::DependentScopeDeclRefExpr => {
                let Some(nns) = kiln_ast::ids::NnsId::try_new(n.a()) else {
                    return Err(SubstFail);
                };
                let name = kiln_source::symbol::Symbol::from_index(n.b());
                let begin = self.sema.ast.templates.expr_begin.get(e).copied().unwrap_or(loc);
                let nns = self.nns(nns, begin)?;
                if let Nns::Type { ty, .. } = self.sema.ast.types.nns_view(nns)
                    && self.sema.is_dependent_type(ty)
                {
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
                    let t = self.member_template_named(nns, name, loc)?;
                    let l = self.args(l, loc)?;
                    let args = self.sema.ast.types.args(l).to_vec();
                    let tn = self.sema.template_name_for(t);
                    return match self.sema.act_template_id(tn, &args, loc, loc) {
                        Ok(
                            crate::templ::actions::TemplateIdRef::Value(x)
                            | crate::templ::actions::TemplateIdRef::Overload(x),
                        ) => Ok(x),
                        Ok(crate::templ::actions::TemplateIdRef::Type(_)) => {
                            self.unimplemented("a type template-id as an expression", loc)
                        }
                        Err(()) => Err(SubstFail),
                    };
                }
                let ctx = self.nns_context(nns, begin)?;
                let q = crate::using::Qualifier { ctx, nns, loc: begin };
                Ok(self.sema.act_on_id_expression(name, loc, Some(q)))
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
                if self.sema.ast.exprs.get(o).kind() == ExprKind::InitListExpr {
                    let l = self.init_list(o)?;
                    return Ok(self.sema.act_on_list_cast(ty, &l, loc));
                }
                let o = self.expr(o)?;
                Ok(self.sema.act_on_cast(style, ty, o, loc, end))
            }
            ExprKind::BuiltinBitCastExpr => {
                // WP-074: `__builtin_bit_cast(T, e)` (a dependent one keeps
                // `T` as its type and `e` unmaterialized).
                let ty = self.ty(n.ty(), loc)?;
                let o = self.expr(ex(self).operand(e))?;
                Ok(self.sema.act_on_builtin_bit_cast(ty, o, loc, end))
            }
            ExprKind::CXXUnresolvedConstructExpr => {
                let ty = self.ty(n.ty(), loc)?;
                let l = kiln_ast::ids::ListId::new(n.a());
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
                let args = self.expr_list(l)?;
                Ok(match args.as_slice() {
                    [] => self.sema.act_on_value_init_cast(ty, loc, end),
                    [a] => self.sema.act_on_cast(CastStyle::Functional, ty, *a, loc, end),
                    _ => self.sema.act_on_paren_list_cast(ty, &args, loc, end),
                })
            }
            ExprKind::UnaryExprOrTypeTraitExpr => {
                let kind = ex(self).trait_kind(e);
                if let Some(t) = ex(self).trait_type_arg(e) {
                    let t = self.ty(t, loc)?;
                    Ok(self.sema.act_on_trait_type(kind, t, loc, end))
                } else {
                    let o = ExprId::new(n.a());
                    let o = self.expr(o)?;
                    Ok(self.sema.act_on_trait_expr(kind, o, loc))
                }
            }
            // spec/sema-templates.md §9.1, §9.4: rebuilt through the
            // parser's builders (no deprecation warning on instantiation).
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
                Ok(self.sema.build_type_trait(t, &args, loc, end))
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
                let o = self.expr(o)?;
                Ok(self.sema.build_expression_trait(t, o, loc, end))
            }
            ExprKind::CXXNoexceptExpr => {
                let o = ex(self).noexcept_operand(e);
                let o = self.expr(o)?;
                Ok(self.sema.act_on_noexcept_expr(o, loc, end))
            }
            ExprKind::CXXThrowExpr => {
                let o = ex(self).throw_operand(e);
                let o = self.opt_expr(o)?;
                Ok(self.sema.act_on_throw(o, loc))
            }
            ExprKind::CXXFoldExpr => self.fold(e, loc, end),
            ExprKind::SizeOfPackExpr => self.size_of_pack(e, loc, end),
            ExprKind::PackIndexingExpr => self.pack_indexing_expr(e, loc, end),
            ExprKind::PackExpansionExpr
            | ExprKind::SubstNonTypeTemplateParmPackExpr
            | ExprKind::FunctionParmPackExpr => self.unimplemented("pack expansion", loc),
            ExprKind::ConceptSpecializationExpr => self.concept_id_expr(e, loc),
            ExprKind::RequiresExpr => self.requires_expr(e, loc),
            ExprKind::LambdaExpr => self.unimplemented("lambda expressions in templates", loc),
            ExprKind::CXXConstructExpr => self.construct_expr(e, loc),
            ExprKind::CXXTemporaryObjectExpr => {
                // `T(a, b)` / `T()` / `T{...}` ([expr.type.conv]/2): rebuilt
                // as written (clang `TransformCXXTemporaryObjectExpr`).
                let ty = self.ty(n.ty(), loc)?;
                if ex(self).construct_info(e).list {
                    let l = self.construct_list(e, loc, end)?;
                    return Ok(self.sema.act_on_list_cast(ty, &l, loc));
                }
                let args = self.expr_list(ex(self).construct_args(e))?;
                Ok(match args.as_slice() {
                    [] => self.sema.act_on_value_init_cast(ty, loc, end),
                    [a] => self.sema.act_on_cast(CastStyle::Functional, ty, *a, loc, end),
                    _ => self.sema.act_on_paren_list_cast(ty, &args, loc, end),
                })
            }
            _ => self.unimplemented(&format!("substitution into {k:?}"), loc),
        }
    }

    /// The object expression of a member access: an implicit `this` is
    /// rebuilt implicit.
    fn member_base(&mut self, base: ExprId) -> SubstResult<ExprId> {
        let b = *self.sema.ast.exprs.get(base);
        if b.kind() == ExprKind::CXXThisExpr && self.needs_rebuild(base) {
            let implicit = self.sema.ast.exprs.this_is_implicit(base);
            return Ok(self.this_expr(implicit, b.loc()));
        }
        self.expr(base)
    }

    fn this_expr(&mut self, implicit: bool, loc: Loc) -> ExprId {
        match self.sema.current_this() {
            Some(t) => self.sema.builder().this_expr(t, implicit, loc),
            None => self.sema.act_on_this(loc),
        }
    }

    /// A callee: an unresolved name is looked up again from the
    /// instantiation context, which also stands in for the
    /// argument-dependent lookup at the point of instantiation
    /// ([temp.dep.candidate]; spec §5.4 gap: one lookup, not two).
    fn callee(&mut self, c: ExprId) -> SubstResult<ExprId> {
        let n = *self.sema.ast.exprs.get(c);
        if n.kind() != ExprKind::UnresolvedLookupExpr {
            return self.expr(c);
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
        if !adl {
            // A qualified name: the decls were found in a non-dependent
            // scope; rebuild the reference as written.
            if let [d] = decls.as_slice() {
                let d = self.find_instantiated_decl(*d, n.loc())?;
                // `v<args>` naming a member variable template of the
                // current instantiation ([temp.dep.expr]/3).
                if self.sema.kind_of(d) == DeclKind::VarTemplate
                    && let Some(&targs) = self.sema.ast.templates.expr_targs.get(c)
                {
                    let l = self.args(targs, n.loc())?;
                    let args = self.sema.ast.types.args(l).to_vec();
                    let name = self.sema.template_name_for(d);
                    return match self.sema.act_template_id(name, &args, n.loc(), n.loc()) {
                        Ok(crate::templ::actions::TemplateIdRef::Value(e)) => Ok(e),
                        _ => Err(SubstFail),
                    };
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
            for &d in &decls {
                ds.push(self.find_instantiated_decl(d, n.loc())?);
            }
            let nns = match self.sema.ast.templates.expr_nns.get(c).copied() {
                Some(q) => Some(self.nns(q, n.loc())?),
                None => None,
            };
            return Ok(self.sema.builder().unresolved_lookup(
                name,
                &ds,
                false,
                nns,
                targs,
                n.loc(),
            ));
        }
        if decls.is_empty() {
            return Ok(self.sema.act_on_undeclared_call_name(name, n.loc()));
        }
        let r = self.sema.act_on_call_name(name, n.loc());
        if let Some(t) = targs {
            if self.sema.ast.exprs.get(r).kind() == ExprKind::UnresolvedLookupExpr {
                let _ = self.sema.ast.templates.expr_targs.insert(r, t);
            } else if self.sema.is_member_template_ref(r) {
                // An implicit member template-id `this->f<T>` (§6.4).
                return Ok(self.sema.act_on_member_template_args(r, t));
            }
        }
        Ok(r)
    }

    /// A reference to a non-type template parameter (§5.4): the
    /// argument as a literal wrapped in `SubstNonTypeTemplateParmExpr`.
    fn nttp_ref(&mut self, e: ExprId, p: DeclId, loc: Loc) -> SubstResult<ExprId> {
        let Some(info) = self.sema.ast.template_parm(p).copied() else { return Err(SubstFail) };
        let a = match self.lookup(info.depth, info.index) {
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
        let r = match a.view() {
            ArgView::Value { ty, value } => self.value_literal(ty, value, loc)?,
            ArgView::Expr { expr, .. } => expr,
            _ => return self.unimplemented("non-type template argument of this kind", loc),
        };
        // The parameter of the level's template (an instantiated member
        // template's own parameter, as clang).
        let shown = self
            .assoc(info.depth)
            .and_then(|a| self.sema.ast.template_info(a).copied())
            .and_then(|i| {
                let l = *self.sema.ast.templates.param_list(i.params);
                self.sema.ast.lists.iter::<DeclId>(l.params).nth(usize::from(info.index))
            })
            .filter(|&x| self.sema.kind_of(x) == DeclKind::NonTypeTemplateParm)
            .unwrap_or(p);
        Ok(self.sema.ast.builder().subst_nttp_expr(shown, r, loc))
    }

    /// The literal for a non-type template argument value
    /// (clang's `BuildExpressionFromNonTypeTemplateArgument`).
    fn value_literal(
        &mut self,
        ty: QualType,
        value: kiln_ast::ConstValueId,
        loc: Loc,
    ) -> SubstResult<ExprId> {
        let Some(v) = self.sema.const_int(value) else {
            // Class, pointer and null values (§6.10).
            if let Some(r) = self.sema.nttp_value_expr(ty, value, loc) {
                return Ok(r);
            }
            return self.unimplemented("non-type template argument of this type", loc);
        };
        let t = &self.sema.ast.types;
        if let Some(en) = t.as_enum(ty) {
            let it = self
                .sema
                .ast
                .decls
                .enumeration(en)
                .and_then(|d| d.int_type)
                .unwrap_or(QualType::INT);
            let lit = self.int_literal(it, v, loc);
            return Ok(self.sema.ast.builder().cast(
                ExprKind::CStyleCastExpr,
                CastKind::IntegralCast,
                lit,
                ty,
                ValueCategory::PRValue,
                None,
                loc,
                ExprFlags::NONE,
            ));
        }
        let kind = match t.builtin(ty) {
            Some(BuiltinType::Bool) => {
                return Ok(self.sema.ast.builder().bool_literal(ty, v != 0, loc));
            }
            Some(BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar) => {
                Some(CharKind::Ascii)
            }
            Some(BuiltinType::WChar) => Some(CharKind::Wide),
            Some(BuiltinType::Char8) => Some(CharKind::Utf8),
            Some(BuiltinType::Char16) => Some(CharKind::Utf16),
            Some(BuiltinType::Char32) => Some(CharKind::Utf32),
            Some(_) => None,
            None => return self.unimplemented("non-type template argument of this type", loc),
        };
        if let Some(kind) = kind {
            let u = u32::try_from(v & 0xffff_ffff).unwrap_or(0);
            return Ok(self.sema.ast.builder().char_literal(ty, kind, u, loc));
        }
        Ok(self.int_literal(ty, v, loc))
    }

    pub(crate) fn int_literal(&mut self, ty: QualType, v: i128, loc: Loc) -> ExprId {
        if let Ok(u) = u64::try_from(v) {
            self.sema.ast.builder().int_literal(ty, u, Radix::Decimal, loc)
        } else {
            let c = self.sema.int_const(ty, v);
            self.sema.ast.builder().wide_int_literal(ty, c, Radix::Decimal, loc)
        }
    }

    /// An initializer-clause of a braced list.
    fn init_value(&mut self, x: ExprId) -> SubstResult<InitItem> {
        let value = if self.sema.ast.exprs.get(x).kind() == ExprKind::InitListExpr {
            InitValue::List(self.init_list(x)?)
        } else {
            InitValue::Expr(self.expr(x)?)
        };
        Ok(InitItem { designator: Vec::new(), value })
    }

    /// The initializer-clauses `items` of a braced list, pack expansions
    /// expanded ([temp.variadic]/5).
    fn init_items(&mut self, items: &[ExprId]) -> SubstResult<Vec<InitItem>> {
        let mut out = Vec::with_capacity(items.len());
        for &x in items {
            if crate::templ::pack::expr_pattern(self.sema, x).is_some() {
                for y in self.expand_exprs(&[x])? {
                    out.push(InitItem { designator: Vec::new(), value: InitValue::Expr(y) });
                }
                continue;
            }
            out.push(self.init_value(x)?);
        }
        Ok(out)
    }

    /// A braced-init-list of the pattern (designators: a gap).
    pub(crate) fn init_list(&mut self, e: ExprId) -> SubstResult<InitList> {
        let n = *self.sema.ast.exprs.get(e);
        let l = self.sema.ast.exprs.init_list_inits(e);
        let items: Vec<ExprId> = self.sema.ast.lists.iter::<ExprId>(l).collect();
        let out = self.init_items(&items)?;
        let rbrace = self.sema.ast.exprs.end_loc(e).unwrap_or(n.loc());
        Ok(InitList { items: out, lbrace: n.loc(), rbrace })
    }

    /// A braced-init-list as the parser built it (a mem-initializer's).
    fn written_list(&mut self, l: &InitList) -> SubstResult<InitList> {
        let mut items = Vec::with_capacity(l.items.len());
        for it in &l.items {
            if !it.designator.is_empty() {
                return self.unimplemented("designated initializers in a template", l.lbrace);
            }
            let value = match &it.value {
                InitValue::Expr(e) if crate::templ::pack::expr_pattern(self.sema, *e).is_some() => {
                    for y in self.expand_exprs(&[*e])? {
                        items.push(InitItem { designator: Vec::new(), value: InitValue::Expr(y) });
                    }
                    continue;
                }
                InitValue::Expr(e) => InitValue::Expr(self.written_arg(*e)?),
                InitValue::List(n) => InitValue::List(self.written_list(n)?),
            };
            items.push(InitItem { designator: Vec::new(), value });
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

    /// Written arguments, pack expansions expanded.
    fn written_args(&mut self, v: &[ExprId]) -> SubstResult<Vec<ExprId>> {
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
        let list_construct =
            pk == ExprKind::CXXConstructExpr && self.sema.ast.exprs.construct_info(peeled).list;
        if pk == ExprKind::CXXConstructExpr && (style != InitStyle::C || list_construct) {
            let pn = *self.sema.ast.exprs.get(peeled);
            let end = self.sema.ast.exprs.end_loc(peeled);
            if list_construct {
                let l = self.construct_list(peeled, pn.loc(), end.unwrap_or(pn.loc()))?;
                return Ok(Some(Initializer::List(l)));
            }
            let args = self.expr_list(self.sema.ast.exprs.construct_args(peeled))?;
            // No parentheses: the variable was default-initialized.
            let Some(r) = end else { return Ok(None) };
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
        let n = *self.sema.ast.exprs.get(init);
        Ok(Some(match n.kind() {
            ExprKind::ParenListExpr => {
                let l = kiln_ast::ids::ListId::new(n.a());
                let args = self.expr_list(l)?;
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
                let e = self.expr(init)?;
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
            init = ex.operand(init);
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
    /// enclosing action redoes. Other forms appear only as initializers
    /// (see [`Self::initializer`]).
    fn construct_expr(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let ast = &self.sema.ast;
        let ex = &ast.exprs;
        if !ex.construct_info(e).list {
            let args: Vec<ExprId> = ast
                .lists
                .iter::<ExprId>(ex.construct_args(e))
                .take_while(|&a| ex.get(a).kind() != ExprKind::CXXDefaultArgExpr)
                .collect();
            if let [a] = args.as_slice() {
                return self.expr(*a);
            }
        }
        self.unimplemented("substitution into this CXXConstructExpr", loc)
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
