//! Splices (`-freflection`; spec/reflection.md §5; [basic.splice],
//! [expr.prim.splice], [expr.ref], [dcl.type.splice],
//! [expr.prim.id.qual]).
//!
//! A splice-specifier `[: r :]` designates the entity of the reflection
//! `r`, a converted constant expression of type `std::meta::info`.
//! [`Sema::splice_entity`] is the shared operand check: it reads a
//! frozen operand (a substituted `info` template argument, a
//! `^^operand`, a `constexpr` variable already evaluated) without an
//! Oven run (the fast path, counted in `Sema::splice_fast`) and
//! evaluates anything else. Each position then checks the designated
//! entity fits it: an expression ([`Sema::act_on_splice_expr`]), a
//! member of the object's class ([`Sema::act_on_member_splice`]), a
//! scope ([`Sema::act_on_nested_name_splice`]), a namespace
//! ([`Sema::act_on_splice_namespace`]) or a type
//! ([`Sema::act_on_splice_type`], `reflect.rs`). A dependent operand
//! builds a dependent `CXXSpliceExpr`, `Splice` type or `Nns::Splice`
//! that `templ/subst` resolves.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{DeclKind, VarEvalState};
use kiln_ast::expr::ExprKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::Nns;
use kiln_ast::{DeclId, ExprId, NnsId, ReflectionId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;

/// The outcome of checking a splice operand.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Spliced {
    /// The operand is value- or type-dependent: resolved at substitution.
    Dependent,
    /// The entity the operand's reflection designates.
    Entity(Reflectee),
    /// The operand is invalid (diagnosed).
    Error,
}

impl Sema<'_, '_> {
    /// The number of splices whose operand was read as a frozen constant
    /// without an Oven evaluation (the splice fast path).
    pub fn splice_fast_count(&self) -> u64 {
        self.splice_fast
    }

    /// The entity splice operand `e` designates ([basic.splice]/1): `e`
    /// must be a converted constant expression of type
    /// `std::meta::info`.
    pub(crate) fn splice_entity(&mut self, e: ExprId) -> Spliced {
        if self.expr_has_error(e) {
            return Spliced::Error;
        }
        let ety = self.ast.exprs.get(e).ty();
        if self.is_dependent_type(ety) || self.expr_is_value_dependent(e) {
            return Spliced::Dependent;
        }
        let at = self.begin_loc(e);
        if !self.is_meta_info(ety) {
            let d = self.diag(DiagId::sema_splice_not_info, at).arg(self.ty_arg(ety));
            self.emit(d);
            return Spliced::Error;
        }
        if let Some(id) = self.splice_frozen_id(e) {
            self.splice_fast += 1;
            return Spliced::Entity(self.ast.reflections.get(id));
        }
        let e = crate::conv::decay(self, e);
        let v = match self.eval_const(e, EvalContext::new(EvalKind::Other, at)) {
            Ok(v) => v,
            Err(f) => {
                let d = self.diag(DiagId::sema_splice_not_constant, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                return Spliced::Error;
            }
        };
        match self.ast.consts.scalar(v) {
            Some(Scalar::Reflection { id, .. }) => Spliced::Entity(self.ast.reflections.get(id)),
            _ => Spliced::Entity(Reflectee::Null),
        }
    }

    /// The reflection a frozen splice operand holds, read without an Oven
    /// run: a `^^operand` (also as the replacement of a substituted
    /// `info` template parameter), a constant expression whose value is
    /// recorded, or a `constexpr` variable of type `std::meta::info`
    /// whose initializer is already evaluated.
    fn splice_frozen_id(&self, e: ExprId) -> Option<ReflectionId> {
        let mut e = e;
        for _ in 0..64 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => e = self.ast.exprs.operand(e),
                ExprKind::SubstNonTypeTemplateParmExpr => e = self.ast.exprs.subst_replacement(e),
                ExprKind::ConstantExpr => match self.ast.exprs.constant_value(e) {
                    Some(v) => return self.scalar_reflection(v),
                    None => e = self.ast.exprs.operand(e),
                },
                ExprKind::CXXReflectExpr => return self.ast.exprs.reflect_value(e),
                ExprKind::DeclRefExpr => {
                    let d = self.ast.exprs.decl(e);
                    let v = self.ast.decls.var(d)?;
                    if !v.spec.is_constexpr() {
                        return None;
                    }
                    let ty = self.ast.decls.get(d).ty()?;
                    if !self.is_meta_info(ty) {
                        return None;
                    }
                    let VarEvalState::Value(c) = v.eval.state() else { return None };
                    return self.scalar_reflection(c);
                }
                _ => return None,
            }
        }
        None
    }

    fn scalar_reflection(&self, v: kiln_ast::ConstValueId) -> Option<ReflectionId> {
        match self.ast.consts.scalar(v)? {
            Scalar::Reflection { id, .. } => Some(id),
            _ => None,
        }
    }

    /// The class type an entity of class `c` is named through in a splice
    /// qualifier (`C::`), kept on the reference so that `&[: m :]` forms a
    /// pointer to member.
    fn splice_class_nns(&mut self, d: DeclId) -> Option<NnsId> {
        let c = self.declaring_class(d)?;
        let ty = self.tag_type(c);
        Some(self.ast.types.nns(Nns::Type { prefix: None, ty, template_kw: false }))
    }

    /// The splice-expression `[: e :]` ([expr.prim.splice]/2): a
    /// variable, structured binding, function, enumerator, value or
    /// object; a non-static member only names it without an object (the
    /// operand of `&`, or an unevaluated operand).
    pub fn act_on_splice_expr(&mut self, e: ExprId, open: Loc, close: Loc) -> ExprId {
        let r = match self.splice_entity(e) {
            Spliced::Error => return self.recover_err(&[e], open),
            Spliced::Dependent => return self.builder().splice_expr(e, None, open, close),
            Spliced::Entity(r) => r,
        };
        let at = self.begin_loc(e);
        match r {
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Field | DeclKind::IndirectField => {
                    let nns = self.splice_class_nns(d);
                    self.member_ref_without_object(d, open, None, nns)
                }
                DeclKind::CXXConstructor | DeclKind::CXXDestructor => {
                    self.splice_special_member(d, at);
                    self.recover_err(&[e], open)
                }
                DeclKind::CXXMethod | DeclKind::CXXConversion if !self.is_static_method(d) => {
                    let nns = self.splice_class_nns(d);
                    self.member_ref_without_object(d, open, None, nns)
                }
                DeclKind::Var
                | DeclKind::ParmVar
                | DeclKind::Binding
                | DeclKind::EnumConstant
                | DeclKind::Function
                | DeclKind::CXXMethod => self.build_decl_ref(d, open),
                _ => self.splice_misfit_expr(e, at, open),
            },
            Reflectee::Value(v) | Reflectee::Object(v) => {
                let ty = self.ast.consts.ty(v);
                if let Some(x) = self.nttp_value_expr(ty, v, open) {
                    return x;
                }
                self.unsupported("splice of a value or object reflection", open);
                self.recover_err(&[e], open)
            }
            _ => self.splice_misfit_expr(e, at, open),
        }
    }

    /// [expr.prim.splice]/2: a splice-expression designating a
    /// constructor or destructor is ill-formed.
    fn splice_special_member(&mut self, d: DeclId, at: Loc) {
        let which = u8::from(self.kind_of(d) == DeclKind::CXXDestructor);
        self.emit(self.diag(DiagId::sema_splice_special_member, at).arg(which));
    }

    fn splice_misfit_expr(&mut self, e: ExprId, at: Loc, open: Loc) -> ExprId {
        self.error(DiagId::sema_splice_not_expression, at);
        self.recover_err(&[e], open)
    }

    /// The class member access `base.[: e :]` / `base->[: e :]`
    /// ([expr.ref]/1, P2996 and P3293): `e` designates a member of the
    /// object's class or of one of its bases, or a direct base-specifier
    /// (the base class subobject). Splices are not access-checked
    /// (P2996 [expr.ref]; as GCC). `open` is the `[:`, `op_loc` the
    /// `.`/`->`, `close` the `:]`.
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub fn act_on_member_splice(
        &mut self,
        base: ExprId,
        e: ExprId,
        open: Loc,
        close: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        if self.is_err_expr(base) {
            return self.recover_err(&[base, e], open);
        }
        let base = self.resolve_single_overload_candidate(base);
        let r = match self.splice_entity(e) {
            Spliced::Error => return self.recover_err(&[base, e], open),
            Spliced::Dependent => {
                return self.builder().splice_expr(e, Some((base, arrow)), open, close);
            }
            Spliced::Entity(r) => r,
        };
        if self.expr_is_type_dependent(base) {
            return self.builder().splice_expr(e, Some((base, arrow)), open, close);
        }
        let (base, arrow) = match self.member_arrow_base(base, open, op_loc, arrow) {
            Ok(x) => x,
            Err(x) => return x,
        };
        let (b, obj_ty, def) = match self.member_object(base, open, op_loc, arrow) {
            Ok(x) => x,
            Err(x) => return x,
        };
        let at = self.begin_loc(e);
        match r {
            Reflectee::Decl(d)
                if matches!(
                    self.kind_of(d),
                    DeclKind::CXXConstructor | DeclKind::CXXDestructor
                ) =>
            {
                self.splice_special_member(d, at);
                self.recover_err(&[b], open)
            }
            Reflectee::Decl(d)
                if self.declaring_class(d).is_some_and(|c| self.qualifier_names_base(c, def)) =>
            {
                // Not a qualified-id: a virtual function is called
                // virtually ([expr.call]/2; the [dcl.contract.func]
                // example's `x1ref.[:^^X1::f:]()`).
                self.member_of_object(b, obj_ty, def, arrow, d, None, None, open)
            }
            Reflectee::BaseSpec(derived, i) if self.qualifier_names_base(derived, def) => {
                let Some(dd) = self.ast.definition(derived) else {
                    return self.recover_err(&[b], open);
                };
                let bt = self.ast.decls.bases(dd, &self.ast.lists).nth(i as usize).map(|s| s.ty);
                let Some(target) = bt.and_then(|t| self.ast.types.as_record(t)) else {
                    return self.recover_err(&[b], open);
                };
                let target = self.ast.first(target);
                let q = self.ast.types.effective_quals(obj_ty);
                self.object_to_class_as(
                    b,
                    arrow,
                    def,
                    q,
                    (target, bt),
                    q,
                    kiln_ast::expr::CastKind::UncheckedDerivedToBase,
                )
            }
            _ => {
                let d = self.diag(DiagId::sema_splice_not_member, at).arg(self.ty_arg(obj_ty));
                self.emit(d);
                self.recover_err(&[b, e], open)
            }
        }
    }

    /// The splice-scope-specifier `[: e :]::` ([expr.prim.id.qual]/1):
    /// a namespace, namespace alias, class or enumeration; the qualifier
    /// context and the nested-name-specifier, or a dependent marker
    /// context with `Nns::Splice` while `e` is dependent.
    pub fn act_on_nested_name_splice(&mut self, e: ExprId, loc: Loc) -> Option<(DeclId, NnsId)> {
        let r = match self.splice_entity(e) {
            Spliced::Error => return None,
            Spliced::Dependent => {
                let n = self.ast.types.nns(Nns::Splice(e));
                return Some((self.decltype_scope_marker(), n));
            }
            Spliced::Entity(r) => r,
        };
        match r {
            Reflectee::Namespace(ns) => {
                let ns = self.ast.first(ns);
                Some((ns, self.ast.types.nns(Nns::Namespace { prefix: None, ns })))
            }
            Reflectee::NamespaceAlias(alias) => {
                let ns = self.ast.first(self.ast.resolve_alias(alias));
                Some((ns, self.ast.types.nns(Nns::NamespaceAlias { prefix: None, alias, ns })))
            }
            Reflectee::Type(ty)
                if self.ast.types.as_record(ty).is_some()
                    || self.ast.types.as_enum(ty).is_some() =>
            {
                self.act_on_nested_name_decltype(ty, loc)
            }
            _ => {
                let at = self.begin_loc(e);
                self.error(DiagId::sema_splice_not_scope, at);
                None
            }
        }
    }

    /// The namespace `using namespace [: e :];` nominates
    /// ([namespace.udir]/1): `e` designates a namespace or namespace
    /// alias other than the global namespace, and is not dependent.
    /// `None` when invalid (diagnosed).
    pub fn act_on_splice_namespace(&mut self, e: ExprId) -> Option<DeclId> {
        match self.splice_entity(e) {
            Spliced::Error => None,
            // [namespace.udir]/1: the splice-specifier shall not be
            // dependent and shall not designate the global namespace.
            Spliced::Dependent => {
                self.error(DiagId::sema_splice_dependent_namespace, self.begin_loc(e));
                None
            }
            Spliced::Entity(Reflectee::Namespace(ns)) => self.splice_nominated(e, ns),
            Spliced::Entity(Reflectee::NamespaceAlias(a)) => {
                let ns = self.ast.resolve_alias(a);
                self.splice_nominated(e, ns)
            }
            Spliced::Entity(_) => {
                let at = self.begin_loc(e);
                self.error(DiagId::sema_splice_not_namespace, at);
                None
            }
        }
    }

    fn splice_nominated(&mut self, e: ExprId, ns: DeclId) -> Option<DeclId> {
        let ns = self.ast.first(ns);
        if ns == self.ast.first(self.ast.tu) {
            self.error(DiagId::sema_splice_global_namespace, self.begin_loc(e));
            return None;
        }
        Some(ns)
    }
}
