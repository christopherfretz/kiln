//! Splices (`-freflection`; spec/reflection.md §5; [basic.splice],
//! [expr.prim.splice], [expr.ref], [dcl.type.splice],
//! [expr.prim.id.qual]).
//!
//! A splice-specifier `[: r :]` designates the entity of the reflection
//! `r`, a converted constant expression of type `std::meta::info`.
//! [`Sema::splice_entity`] is the shared operand check: it reads a
//! frozen operand (a substituted `info` template argument, a
//! `^^operand`, a `constexpr` variable already evaluated) without an
//! Oven run (the fast path, counted in `Sema::splice_fast` and reported
//! as the Oven report's `splice_fast` total) and evaluates anything
//! else. Each position then checks the designated entity fits it: an
//! expression ([`Sema::act_on_splice_expr`]), a
//! member of the object's class ([`Sema::act_on_member_splice`]), a
//! scope ([`Sema::act_on_nested_name_splice`]), a namespace
//! ([`Sema::act_on_splice_namespace`]) or a type
//! ([`Sema::act_on_splice_type`], `reflect.rs`). A dependent operand
//! builds a dependent `CXXSpliceExpr`, `Splice` type or `Nns::Splice`
//! that `templ/subst` resolves. A splice-specialization-specifier
//! `[: r :] < args >` (P2996 [temp.names]/1) names a specialization of
//! the template `r` designates ([`Sema::act_on_splice_template_type`],
//! [`Sema::act_on_splice_template_expr`]); a dependent `r` is the
//! template name `TemplateName::Splice`, or a `CXXSpliceExpr` that
//! keeps the arguments in an expression.

use kiln_ast::consts::Scalar;
use kiln_ast::decl::{DeclKind, VarEvalState};
use kiln_ast::expr::ExprKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{Nns, TemplateArg, TemplateName};
use kiln_ast::{DeclId, ExprId, NnsId, QualType, ReflectionId, TemplateNameId};
use kiln_diag::DiagId;
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::templ::actions::TemplateIdRef;

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
        // A class operand converts to `info` through a conversion
        // function ([basic.splice]/1: a converted constant expression).
        let mut e = e;
        if self.ast.types.as_record(self.non_reference(ety)).is_some() {
            let info = kiln_ast::types::BuiltinType::MetaInfo.qual_type();
            if let Ok(c) =
                crate::conv::implicit_convert(self, e, info, crate::conv::ConvCtx::Initialization)
            {
                e = c;
            }
        }
        let ety = self.ast.exprs.get(e).ty();
        if !self.is_meta_info(ety) {
            let d = self.diag(DiagId::sema_splice_not_info, at).arg(self.ty_arg(ety));
            self.emit(d);
            return Spliced::Error;
        }
        self.splice_operand_not_odr_used(e);
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
    /// operand of `&`, or an unevaluated operand). With `template_kw`
    /// (`template [: e :]`, /3) it must designate a function template
    /// that is not a constructor template; without it, a template is
    /// ill-formed (/2.7: diagnosed, then taken as if `template` were
    /// written).
    pub fn act_on_splice_expr(
        &mut self,
        e: ExprId,
        template_kw: bool,
        open: Loc,
        close: Loc,
    ) -> ExprId {
        let r = match self.splice_entity(e) {
            Spliced::Error => return self.recover_err(&[e], open),
            Spliced::Dependent if template_kw => {
                return self.builder().template_splice_expr(e, None, open, close);
            }
            Spliced::Dependent => return self.builder().splice_expr(e, None, open, close),
            Spliced::Entity(r) => r,
        };
        let at = self.begin_loc(e);
        if !self.splice_template_kw_fits(r, template_kw, at) {
            return self.recover_err(&[e], open);
        }
        match r {
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Field | DeclKind::IndirectField => {
                    let Ok(d) = self.anon_member_as_indirect(d) else {
                        let name = self.decl_name(d);
                        self.emit(
                            self.diag(DiagId::sema_invalid_nonstatic_member_use, open).arg(name),
                        );
                        return self.recover_err(&[e], open);
                    };
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
                // In a contract predicate a variable or structured
                // binding declared outside the assertion is const here
                // as for an id-expression (P3598R0, /2.5;
                // `contract_constify`).
                DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding
                    if self.splice_local_across_lambda(d, open) =>
                {
                    self.recover_err(&[e], open)
                }
                DeclKind::Var
                | DeclKind::ParmVar
                | DeclKind::Binding
                | DeclKind::EnumConstant
                | DeclKind::Function
                | DeclKind::CXXMethod => self.build_decl_ref(d, open),
                _ => self.splice_misfit_expr(e, at, open),
            },
            // A value: a prvalue of it; an object: an lvalue naming it
            // ([expr.prim.splice]/2; `splice_object.rs`).
            Reflectee::Value(v) => {
                if let Some(x) = self.splice_value_expr(v, open) {
                    return x;
                }
                self.unsupported("splice of this value reflection", open);
                self.recover_err(&[e], open)
            }
            Reflectee::Object(p) => {
                if let Some(x) = self.splice_object_expr(p, open) {
                    return x;
                }
                let what = self.object_splice_unsupported(p);
                self.unsupported(what, open);
                self.recover_err(&[e], open)
            }
            // `template [: r :]` / `[: r :]` designating a function
            // template: its overload set, the templates' arguments
            // deduced by a call ([expr.prim.splice]/3).
            Reflectee::Template(n) => match self.ast.types.template_decl(n) {
                Some(d) if self.kind_of(d) == DeclKind::FunctionTemplate => {
                    let name = self.ast.decls.get(d).name();
                    self.builder().unresolved_lookup(name, &[d], false, None, None, open)
                }
                _ => self.splice_misfit_expr(e, at, open),
            },
            _ => self.splice_misfit_expr(e, at, open),
        }
    }

    /// [expr.prim.splice]/2.7 and /3 for a splice-expression without
    /// template arguments designating `r`: after `template` it must be a
    /// function template that is not a constructor template (else
    /// diagnosed, `false`); without `template`, a template is diagnosed
    /// (missing keyword) and then accepted for recovery.
    fn splice_template_kw_fits(&mut self, r: Reflectee, template_kw: bool, at: Loc) -> bool {
        let fn_template = match r {
            Reflectee::Template(n) => self.ast.types.template_decl(n).map(|d| {
                self.kind_of(d) == DeclKind::FunctionTemplate
                    && !self
                        .pattern_of(d)
                        .is_some_and(|p| self.kind_of(p) == DeclKind::CXXConstructor)
            }),
            _ => Some(false),
        };
        if template_kw {
            if fn_template == Some(false) {
                self.error(DiagId::sema_splice_not_function_template, at);
                return false;
            }
        } else if matches!(r, Reflectee::Template(_)) {
            self.error(DiagId::sema_splice_missing_template_kw, at);
            return fn_template == Some(true);
        }
        true
    }

    /// The missing-`template` error for frozen splice operand `e` that
    /// designates a template and is followed by `<` in an expression
    /// ([temp.names]/3, [expr.prim.splice]/2.7).
    pub fn missing_splice_template_kw(&mut self, e: ExprId) {
        self.error(DiagId::sema_splice_missing_template_kw, self.begin_loc(e));
    }

    /// Whether splice operand `e` is frozen ([`Self::splice_frozen_id`])
    /// and designates a template: a `<` after it then opens a template
    /// argument list even without `template` (as GCC 16.1; P2996
    /// [temp.names]/3 asks for `template` outside a type-only context).
    /// Reads no unfrozen operand, so it diagnoses nothing.
    pub fn splice_names_template(&self, e: ExprId) -> bool {
        if self.expr_has_error(e) {
            return false;
        }
        self.splice_frozen_id(e)
            .is_some_and(|id| matches!(self.ast.reflections.get(id), Reflectee::Template(_)))
    }

    /// The template the splice-specifier of a
    /// splice-specialization-specifier `[: e :] < args >` designates
    /// ([temp.names]/1, P2996): a dependent `e` is the template name
    /// `TemplateName::Splice`, resolved at substitution. `None` when
    /// invalid (diagnosed).
    /// With `want_type`, the template must be a class or alias template
    /// (or a template template parameter): the specialization is a
    /// type.
    pub(crate) fn splice_template_name(
        &mut self,
        e: ExprId,
        want_type: bool,
    ) -> Option<TemplateNameId> {
        match self.splice_entity(e) {
            Spliced::Error => None,
            Spliced::Dependent => {
                let profile = Some(self.dependent_profile(e));
                Some(self.ast.types.template_name(TemplateName::Splice { expr: e, profile }))
            }
            Spliced::Entity(Reflectee::Template(n)) => {
                let fits = !want_type
                    || self.ast.types.template_decl(n).is_none_or(|d| {
                        matches!(
                            self.kind_of(d),
                            DeclKind::ClassTemplate
                                | DeclKind::TypeAliasTemplate
                                | DeclKind::TemplateTemplateParm
                                | DeclKind::BuiltinTemplate
                        )
                    });
                if !fits {
                    self.error(DiagId::sema_splice_not_type, self.begin_loc(e));
                    return None;
                }
                Some(n)
            }
            Spliced::Entity(_) => {
                self.error(DiagId::sema_splice_not_template, self.begin_loc(e));
                None
            }
        }
    }

    /// The type `[: e :] < args >` names in a type position
    /// ([dcl.type.splice]/1, P2996): a class or alias template
    /// specialization (a dependent one while `e` is dependent). `open`
    /// is the `[:`, `r` the `>`.
    pub fn act_on_splice_template_type(
        &mut self,
        e: ExprId,
        args: &[TemplateArg],
        open: Loc,
        r: Loc,
    ) -> Option<QualType> {
        let n = self.splice_template_name(e, true)?;
        match self.act_template_id(n, args, open, r) {
            Ok(TemplateIdRef::Type(t)) => Some(t),
            Ok(_) => {
                self.error(DiagId::sema_splice_not_type, self.begin_loc(e));
                None
            }
            Err(()) => None,
        }
    }

    /// The splice-expression `template [: e :] < args >`
    /// ([expr.prim.splice]/3, P2996): a variable template
    /// specialization, a concept-id, or a function template's overload
    /// set with explicit arguments; a dependent `CXXSpliceExpr` keeping
    /// `args` while `e` is dependent. `open` is the `[:`, `r` the `>`.
    pub fn act_on_splice_template_expr(
        &mut self,
        e: ExprId,
        args: &[TemplateArg],
        open: Loc,
        r: Loc,
    ) -> ExprId {
        if let Spliced::Dependent = self.splice_entity_peek(e) {
            let list = self.ast.types.arg_list(args);
            return self.builder().splice_template_expr(e, list, open, r);
        }
        let Some(n) = self.splice_template_name(e, false) else {
            return self.recover_err(&[e], open);
        };
        match self.act_template_id(n, args, open, r) {
            Ok(TemplateIdRef::Value(x) | TemplateIdRef::Overload(x)) => x,
            Ok(TemplateIdRef::Type(_)) => {
                let at = self.begin_loc(e);
                self.splice_misfit_expr(e, at, open)
            }
            Err(()) => self.recover_err(&[e], open),
        }
    }

    /// [`Self::splice_entity`]'s dependence test alone (no evaluation,
    /// no diagnostic).
    fn splice_entity_peek(&self, e: ExprId) -> Spliced {
        if self.expr_has_error(e) {
            return Spliced::Error;
        }
        let ety = self.ast.exprs.get(e).ty();
        if self.is_dependent_type(ety) || self.expr_is_value_dependent(e) {
            return Spliced::Dependent;
        }
        Spliced::Entity(Reflectee::Null)
    }

    /// [expr.prim.splice]/2: a splice-expression designating a
    /// constructor or destructor is ill-formed.
    /// Field `d` of an anonymous union or struct as the member its
    /// innermost enclosing named class declares for it
    /// ([class.union.anon]/1: the `IndirectField` a member access names);
    /// `d` itself otherwise; `Err` for a member of an anonymous union at
    /// namespace or block scope, which is a member of no named class
    /// (CWG issue 705: no splice designates it).
    fn anon_member_as_indirect(&self, d: DeclId) -> Result<DeclId, ()> {
        if self.kind_of(d) != DeclKind::Field {
            return Ok(d);
        }
        let anon = |s: &Self, c: DeclId| {
            s.ast
                .decls
                .record(c)
                .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::ANONYMOUS))
        };
        let Some(mut c) = self.ast.decls.get(d).ctx() else { return Ok(d) };
        if !anon(self, c) {
            return Ok(d);
        }
        while anon(self, c) {
            let Some(p) = self.ast.decls.get(c).ctx() else { return Ok(d) };
            c = p;
        }
        if self.ast.decls.record(c).is_none() {
            return Err(());
        }
        let c = self.ast.definition(c).unwrap_or(c);
        Ok(self
            .ast
            .members(c)
            .find(|&m| {
                self.kind_of(m) == DeclKind::IndirectField
                    && self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(m)).last()
                        == Some(d)
            })
            .unwrap_or(d))
    }

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
        // A member function template (`x.template [:r:](a)` substituted):
        // its one-template overload set. Without `template` it is
        // ill-formed ([expr.prim.splice]/2.7; recovered as with it).
        if matches!(r, Reflectee::Template(_)) {
            self.error(DiagId::sema_splice_missing_template_kw, self.begin_loc(e));
            return self.act_on_member_template_splice(base, e, None, open, close, op_loc, arrow);
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
        let r = match r {
            Reflectee::Decl(d) => Reflectee::Decl(self.anon_member_as_indirect(d).unwrap_or(d)),
            r => r,
        };
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

    /// The class member access `base.template [: e :]` or
    /// `base.template [: e :] < args >` ([expr.ref]/1, [expr.prim.splice]/3,
    /// P2996): `e` designates a member function template or a static data
    /// member template of the object's class or of a base; without
    /// arguments it must be a function template, whose arguments the call
    /// deduces. The template is the one designated, not what its name
    /// finds in the object's class (a derived class's template of the
    /// same name hides nothing). In a template (or with a dependent object
    /// or `e`) the access is kept, with its arguments, for substitution.
    /// `open` is the `[:`.
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub fn act_on_member_template_splice(
        &mut self,
        base: ExprId,
        e: ExprId,
        args: Option<&[TemplateArg]>,
        open: Loc,
        close: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        if self.is_err_expr(base) {
            return self.recover_err(&[base, e], open);
        }
        // In a template the access is kept as written: substitution would
        // look the set's name up again in the object's class.
        let dependent = matches!(self.splice_entity_peek(e), Spliced::Dependent)
            || self.expr_is_type_dependent(base)
            || self.in_template();
        if dependent {
            let Some(args) = args else {
                return self.builder().template_splice_expr(e, Some((base, arrow)), open, close);
            };
            let list = self.ast.types.arg_list(args);
            return self.builder().member_splice_template_expr(e, (base, arrow), list, open, close);
        }
        let base = self.resolve_single_overload_candidate(base);
        let n = match self.splice_entity(e) {
            Spliced::Error => return self.recover_err(&[base, e], open),
            Spliced::Dependent => unreachable!("peeked"),
            Spliced::Entity(Reflectee::Template(n))
                if args.is_some()
                    || self
                        .ast
                        .types
                        .template_decl(n)
                        .is_some_and(|d| self.kind_of(d) == DeclKind::FunctionTemplate) =>
            {
                n
            }
            Spliced::Entity(_) => {
                let id = if args.is_some() {
                    DiagId::sema_splice_not_template
                } else {
                    DiagId::sema_splice_not_function_template
                };
                self.error(id, self.begin_loc(e));
                return self.recover_err(&[base, e], open);
            }
        };
        let (base, arrow) = match self.member_arrow_base(base, open, op_loc, arrow) {
            Ok(x) => x,
            Err(x) => return x,
        };
        let (b, obj_ty, def) = match self.member_object(base, open, op_loc, arrow) {
            Ok(x) => x,
            Err(x) => return x,
        };
        let at = self.begin_loc(e);
        let d = self.ast.types.template_decl(n);
        let member = d.and_then(|d| {
            let kind = self.kind_of(d);
            let c = self.declaring_class(d)?;
            (self.qualifier_names_base(c, def)
                && matches!(kind, DeclKind::FunctionTemplate | DeclKind::VarTemplate))
            .then_some((d, c))
        });
        let Some((d, naming)) = member else {
            let d = self.diag(DiagId::sema_splice_not_member, at).arg(self.ty_arg(obj_ty));
            self.emit(d);
            return self.recover_err(&[b, e], open);
        };
        let set = crate::overload::OvlMember {
            name: self.ast.decls.get(d).name(),
            decls: vec![d],
            found: vec![d],
            naming,
            obj_ty,
            def,
            base: b,
            arrow,
            nns: None,
            implicit: false,
            targs: None,
        };
        let x = self.build_member_overload(set, open);
        match args {
            Some(args) => {
                let l = self.ast.types.arg_list(args);
                self.act_on_member_template_args(x, l)
            }
            None => x,
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
