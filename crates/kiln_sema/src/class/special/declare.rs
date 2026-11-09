//! Constructor, destructor, conversion-function and member `operator=`
//! declarators (spec/sema-class.md §4.7; clang's
//! `CheckConstructorDeclarator`, `CheckDestructorDeclarator`,
//! `CheckConversionDeclarator`, `CheckConstructor`) and the
//! `explicit` specifier ([dcl.fct.spec]/3).

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind, FnSpec};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncInfo, RefQual};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use super::SpecialKind;
use crate::Sema;
use crate::declspec::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, ParamInfo, SpecialName, StorageSpec,
};
use crate::lookup::LookupKind;

/// What a special declarator declares.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Ctor,
    Dtor,
    Conv(QualType),
    Assign,
}

impl Shape {
    /// The `%select` index of `constructor|destructor|conversion function`.
    fn sel(self) -> &'static str {
        match self {
            Shape::Ctor => "0",
            Shape::Dtor => "1",
            Shape::Conv(_) | Shape::Assign => "2",
        }
    }

    fn kind(self) -> DeclKind {
        match self {
            Shape::Ctor => DeclKind::CXXConstructor,
            Shape::Dtor => DeclKind::CXXDestructor,
            Shape::Conv(_) => DeclKind::CXXConversion,
            Shape::Assign => DeclKind::CXXMethod,
        }
    }
}

impl Sema<'_, '_> {
    /// Whether [`Sema::act_on_special_declarator`] declares `d`:
    /// constructors, destructors, conversion functions and member
    /// `operator=` (other operator functions and friend declarations of
    /// special members are WP-068b).
    pub(crate) fn handles_special_declarator(&self, ds: &DeclSpec, d: &Declarator) -> bool {
        if ds.friend.is_some() {
            return false;
        }
        match d.special {
            Some((
                SpecialName::Constructor(_)
                | SpecialName::Destructor(_)
                | SpecialName::Conversion(_),
                _,
            )) => true,
            Some((SpecialName::Operator(OverloadedOperator::Equal), _)) => {
                self.declarator_class(d).is_some()
            }
            _ => false,
        }
    }

    /// The class a declarator declares a member of: the class being
    /// defined, or the class named by its qualifier.
    fn declarator_class(&self, d: &Declarator) -> Option<DeclId> {
        let (ctx, _) = self.decl_contexts(d);
        (self.kind_of(ctx) == DeclKind::CXXRecord).then(|| self.ast.definition(ctx).unwrap_or(ctx))
    }

    /// The `explicit` specifier of a declarator ([dcl.fct.spec]/3; clang
    /// `ActOnFunctionDeclarator`, `DiagnoseFunctionSpecifiers`): only on
    /// a constructor or conversion function declared in its class.
    /// Returns whether the declaration is explicit (written, and
    /// `explicit(expr)` not known false).
    pub(crate) fn check_explicit_spec(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        function: bool,
        ctor_or_conv: bool,
    ) -> bool {
        let Some(loc) = ds.explicit else { return false };
        if !function {
            self.error(DiagId::sema_explicit_non_function, loc);
        } else if d.ctx != DeclaratorContext::Member {
            self.error(DiagId::sema_explicit_out_of_class, loc);
        } else if !ctor_or_conv {
            self.error(DiagId::sema_explicit_non_ctor_or_conv, loc);
        } else {
            return true;
        }
        false
    }

    /// Whether function `f` is explicit without a value-dependent
    /// `explicit(expr)` (clang `isNonDependentlyExplicit`): a template
    /// whose explicit-specifier depends on its parameters is resolved
    /// per specialization (§6.1 step 4).
    pub(crate) fn nondependently_explicit(&self, f: DeclId) -> bool {
        self.fn_spec(f).is_explicit()
            && self.ast.decls.cold.explicit_expr.get(f).is_none_or(|&e| {
                let x = self.ast.exprs.get(e);
                !x.is_value_dependent() && !x.is_type_dependent()
            })
    }

    /// The value of `explicit(expr)` ([dcl.fct.spec]/4), as
    /// [`Sema::resolve_explicit`]: `None` when it is value-dependent.
    /// After an error the declaration is not explicit (clang leaves the
    /// declaration without follow-on diagnostics).
    pub(crate) fn explicit_value(&mut self, e: kiln_ast::ExprId) -> Option<bool> {
        match self.resolve_explicit(e) {
            Ok(v) => v.map(|(_, v)| v),
            Err(()) => Some(false),
        }
    }

    /// Resolves `explicit(expr)` ([dcl.fct.spec]/4: a contextually
    /// converted constant expression of type `bool`, [`Sema::bool_cce`];
    /// clang `tryResolveExplicitSpecifier`): `Ok(None)` when it is
    /// value-dependent, else the converted `ConstantExpr` and its value.
    ///
    /// # Errors
    /// `Err` after an error was reported at the operand.
    pub(crate) fn resolve_explicit(
        &mut self,
        e: kiln_ast::ExprId,
    ) -> Result<Option<(kiln_ast::ExprId, bool)>, ()> {
        let x = self.ast.exprs.get(e);
        if x.is_value_dependent() || x.is_type_dependent() {
            return Ok(None);
        }
        if self.is_err_expr(e) {
            return Err(());
        }
        let at = self.begin_loc(e);
        self.bool_cce(e, at, "explicit specifier argument").map(Some).ok_or(())
    }

    /// A constructor, destructor, conversion-function or member
    /// `operator=` declarator ([class.ctor], [class.dtor],
    /// [class.conv.fct], [class.copy.assign]; spec/sema-class.md §4.7).
    /// `ty` is the declarator's type (return type `void` unless one was
    /// written); `invalid` whether an error was already reported. Returns
    /// the function, or `None` when the declaration is dropped.
    #[allow(clippy::too_many_lines, reason = "clang's declarator checks in order")]
    pub(crate) fn act_on_special_declarator(
        &mut self,
        ds: &DeclSpec,
        ty: QualType,
        d: &Declarator,
        invalid: &mut bool,
    ) -> Option<DeclId> {
        let (sp, sloc) = d.special?;
        let Some(class) = self.declarator_class(d) else {
            match sp {
                SpecialName::Conversion(_) => self.error(DiagId::sema_conv_not_member, sloc),
                SpecialName::Destructor(_) => self.error(DiagId::sema_dtor_not_member, sloc),
                SpecialName::Constructor(_) => self.unsupported("constructors", sloc),
                SpecialName::Operator(_) => self.unsupported("overloaded operators", sloc),
                SpecialName::LiteralOperator(_) => self.unsupported("literal operators", sloc),
                SpecialName::DeductionGuide(_) => self.unsupported("deduction guides here", sloc),
            }
            return None;
        };
        if !self.ast.types.is_function(ty) {
            if !*invalid {
                self.error(DiagId::parse_expected_unqualified_id, sloc);
            }
            return None;
        }
        let shape = match sp {
            SpecialName::Constructor(_) => Shape::Ctor,
            SpecialName::Destructor(n) => {
                if !self.dtor_name_matches(class, n, d.qualifier, sloc) {
                    return None;
                }
                Shape::Dtor
            }
            SpecialName::Conversion(t) => Shape::Conv(t),
            SpecialName::Operator(_) => Shape::Assign,
            SpecialName::DeductionGuide(_) | SpecialName::LiteralOperator(_) => return None,
        };
        let in_class = d.qualifier.is_none();
        let is_static = matches!(ds.storage, Some((StorageSpec::Static, _)));
        let static_loc = match ds.storage {
            Some((StorageSpec::Static, l)) => l,
            _ => sloc,
        };
        let _ = static_loc;
        let Some(Chunk::Function { params, variadic, rparen, quals, ref_qual, .. }) =
            d.chunks.first()
        else {
            return None;
        };
        let (params, variadic, rparen, quals, ref_qual) =
            (params.clone(), *variadic, *rparen, *quals, *ref_qual);
        // [dcl.fct]/6 (P0847): the first parameter may be an explicit
        // object parameter of a conversion function or `operator=`; a
        // constructor or destructor has none ([class.ctor.general]/1,
        // [class.dtor]/1; clang `err_explicit_object_parameter_constructor`).
        let xobj = params.first().is_some_and(|p| p.explicit_object.is_some());
        if let (true, Some(this_loc), Shape::Ctor | Shape::Dtor) =
            (xobj, params.first().and_then(|p| p.explicit_object), shape)
        {
            let m = self
                .diag(DiagId::sema_explicit_object_param_ctor_dtor, this_loc)
                .arg(u8::from(shape == Shape::Dtor));
            self.emit(m);
            *invalid = true;
        }
        // Any other misplaced explicit object parameter is reported before
        // the declarator's own checks, and one in a static or virtual
        // function makes the declaration invalid (clang
        // `CheckExplicitObjectMemberFunction` runs first).
        let eo = if !xobj || matches!(shape, Shape::Conv(_) | Shape::Assign) {
            self.explicit_object_member(ds, &params, true, false)
        } else {
            None
        };
        let xobj_member = eo == Some(true);
        if eo == Some(false) {
            *invalid = true;
        }
        let mut ds_virt = None;
        match shape {
            Shape::Ctor => {
                if ds.has_type_specifier() {
                    self.error(DiagId::sema_ctor_return_type, sloc);
                    return None;
                }
                if ds.virtual_.is_some() {
                    if !*invalid {
                        let m = self
                            .diag(DiagId::sema_ctor_dtor_cannot_be, sloc)
                            .arg("0")
                            .arg("virtual");
                        self.emit(m);
                    }
                    *invalid = true;
                    let mut c = ds.clone();
                    c.virtual_ = None;
                    ds_virt = Some(c);
                }
                if is_static {
                    if !*invalid {
                        let m = self
                            .diag(DiagId::sema_ctor_dtor_cannot_be, sloc)
                            .arg("0")
                            .arg("static");
                        self.emit(m);
                    }
                    *invalid = true;
                }
                self.check_ctor_dtor_quals(rparen, quals, ref_qual, "0", invalid);
            }
            Shape::Dtor => {
                // clang `CheckDestructorDeclarator` drops `static`
                // without invalidating the declarator.
                if is_static && !*invalid {
                    let m =
                        self.diag(DiagId::sema_ctor_dtor_cannot_be, sloc).arg("1").arg("static");
                    self.emit(m);
                }
                if !*invalid && ds.has_type_specifier() {
                    self.error(DiagId::sema_dtor_return_type, sloc);
                }
                self.check_ctor_dtor_quals(rparen, quals, ref_qual, "1", invalid);
                if !params.is_empty() {
                    self.error(DiagId::sema_dtor_params, sloc);
                    *invalid = true;
                }
                if variadic {
                    self.error(DiagId::sema_dtor_variadic, sloc);
                    *invalid = true;
                }
            }
            Shape::Conv(_) => {
                let id = if is_static {
                    Some(DiagId::sema_conv_not_member)
                } else if ds.has_type_specifier() {
                    Some(DiagId::sema_conv_return_type)
                } else if params.get(usize::from(xobj)).is_some() {
                    // [class.conv.fct]/1 (C++23): no non-object parameters;
                    // an explicit object parameter is allowed (P0847).
                    Some(DiagId::sema_conv_params)
                } else if variadic {
                    Some(DiagId::sema_conv_variadic)
                } else {
                    None
                };
                if let Some(id) = id {
                    if !*invalid {
                        self.error(id, sloc);
                    }
                    return None;
                }
            }
            Shape::Assign => {
                if is_static {
                    // [over.ass]/1: a non-static member function.
                    let m = self.diag(DiagId::sema_operator_overload_static, sloc).arg("operator=");
                    self.emit(m);
                    return None;
                }
            }
        }
        // The function type: constructors and destructors return `void`
        // and keep no qualifiers once diagnosed; a destructor has no
        // parameters; a conversion function returns its target.
        let ty = match shape {
            Shape::Ctor => self.rebuild_fn_type(ty, QualType::VOID, None, true),
            Shape::Dtor => self.rebuild_fn_type(ty, QualType::VOID, Some(&[]), true),
            Shape::Conv(t) => self.rebuild_fn_type(ty, t, None, false),
            Shape::Assign => ty,
        };
        // An explicit object member function takes no cv-/ref-qualifiers
        // ([dcl.fct]/6): reported and dropped as for any other.
        let ty = if xobj_member { self.drop_invalid_fn_quals(ty, d, true, ds.loc) } else { ty };
        let class_ty = self.class_type(class);
        let canon = self.ast.types.canonical(class_ty).ty();
        let name = match shape {
            Shape::Ctor => self.ast.names.constructor(canon),
            Shape::Dtor => self.ast.names.destructor(canon),
            Shape::Conv(t) => {
                let c = self.ast.types.canonical(t);
                self.ast.names.conversion(c)
            }
            Shape::Assign => DeclName::operator(OverloadedOperator::Equal),
        };
        // Not for a member of an instantiated class (clang suppresses it
        // for any instantiation, `ActOnConversionDeclarator`).
        if let Shape::Conv(t) = shape
            && in_class
            && !self.is_dependent_ctx(class)
            && self.inst.stack.depth() == 0
        {
            self.warn_class_conversion(class, t, sloc);
        }
        let (ctx, lexical) = self.decl_contexts(d);
        let ctx = self.ast.definition(ctx).unwrap_or(ctx);
        let fkind = shape.kind();
        // Redeclarations ([class.mem.general]/5, [class.mfct]/3).
        if !in_class && !self.is_dependent_ctx(class) {
            match shape {
                Shape::Ctor => {
                    for k in
                        [SpecialKind::DefaultCtor, SpecialKind::CopyCtor, SpecialKind::MoveCtor]
                    {
                        self.ensure_implicit(class, k);
                    }
                }
                Shape::Dtor => self.ensure_implicit(class, SpecialKind::Dtor),
                Shape::Assign => {
                    self.ensure_implicit(class, SpecialKind::CopyAssign);
                    self.ensure_implicit(class, SpecialKind::MoveAssign);
                }
                Shape::Conv(_) => {}
            }
        }
        let templ = self.templ.cur.is_some();
        // Its `enable_if` conditions: differing ones declare an overload
        // (clang `Sema::IsOverload`; spec/sema-overload.md §4.3).
        let enable_if = self.new_fn_enable_if(&d.attrs);
        let cands: Vec<DeclId> = self.ast.lookup_local(class, name).iter().collect();
        let mut matched = None;
        for c in cands {
            let c = if templ {
                // [temp.over.link]/7: member templates are the same entity
                // only when their template heads are equivalent too; heads
                // differing in a non-type parameter's (dependent) type,
                // e.g. `enable_if_t<C1, int> = 0` vs `enable_if_t<C2, int>
                // = 0`, declare overloads.
                let head = self.ast.template_info(c).map(|i| i.params);
                if let (Some(a), Some(b)) = (head, self.templ.cur)
                    && !self.heads_equivalent(a, b)
                {
                    continue;
                }
                match self.pattern_of(c) {
                    Some(p) => self.ast.latest(p),
                    None => continue,
                }
            } else {
                c
            };
            if self.kind_of(c) != fkind || (!templ && self.ast.described_template(c).is_some()) {
                continue;
            }
            let cty = self.ast.decls.get(c).ty().unwrap_or(QualType::ERROR);
            // [temp.over.link]/5: a different trailing requires-clause
            // declares a different (constrained) member.
            let creq = self.ast.templates.trailing_requires.get(c).copied();
            if self.same_ignoring_exc_spec(cty, ty)
                && self.opt_exprs_equivalent(creq, d.trailing_requires)
                && self.same_enable_if(c, enable_if)
            {
                matched = Some(c);
                break;
            }
        }
        // [over.load]/2.3 (C++20), [basic.scope.scope]/3 (C++23): a
        // conversion function or `operator=` differing from an earlier one
        // only in a ref-qualifier present on just one of them (WP-1041).
        // Like clang, not for member templates, and a conversion to a
        // dependent type only at instantiation (which declares it here).
        let ref_mix_shape = match shape {
            Shape::Conv(t) => !self.ast.types.is_dependent(t),
            Shape::Assign => true,
            _ => false,
        };
        if matched.is_none() && in_class && !*invalid && !xobj_member && !templ && ref_mix_shape {
            let new = crate::overload::redecl::NewFn {
                ty,
                is_static: false,
                head: None,
                requires: d.trailing_requires,
                enable_if,
                member_using: false,
                pass_object_size: false,
            };
            matched = self.ref_qualifier_mix(name, new, sloc);
        }
        if matched.is_none()
            && !in_class
            && self.at_explicit_head()
            && self.templ.explicit_inst.is_some()
        {
            // [temp.explicit]/4: no non-template member matches, so the
            // declarator names a member function template specialization.
            let templates = if let Shape::Conv(_) = shape {
                self.conversion_templates_of(ctx)
            } else {
                let found: Vec<DeclId> = self.ast.lookup_local(class, name).iter().collect();
                self.function_templates_in(&found)
            };
            self.act_on_explicit_special_template_inst(ds, d, &templates, name, ty, sloc);
            return None;
        }
        if matched.is_none()
            && !in_class
            && shape != Shape::Dtor
            && self.at_explicit_head()
            && self.templ.explicit_spec.is_some()
        {
            // [temp.expl.spec]/1, [temp.deduct.decl]: likewise after
            // `template<>`, the out-of-line declarator names an explicit
            // specialization of a constructor (conversion function,
            // `operator=`) template, deduced from its type (WP-668).
            let templates = if let Shape::Conv(_) = shape {
                self.conversion_templates_of(ctx)
            } else {
                let found: Vec<DeclId> = self.ast.lookup_local(class, name).iter().collect();
                self.function_templates_in(&found)
            };
            if !templates.is_empty() {
                return self
                    .act_on_explicit_special_template_spec(ds, d, &templates, name, ty, sloc);
            }
        }
        let mut prev_ok = None;
        let mut ty = ty;
        let mut redecl_invalid = false;
        if in_class {
            if let Some(p) = matched
                && !*invalid
            {
                if self.ast.decls.get(p).flags().contains(DeclFlags::INVALID) {
                    redecl_invalid = true;
                } else {
                    let m = match shape {
                        Shape::Assign => self.diag(DiagId::sema_member_redeclared, sloc),
                        _ => self.diag(DiagId::sema_special_redeclared, sloc).arg(shape.sel()),
                    };
                    let m = self.note_prev(m, p, false);
                    self.emit(m);
                    redecl_invalid = true;
                }
            }
        } else {
            match matched {
                Some(p) if !self.is_user_declared(p) => {
                    let kind = self.special_kind_of(p).unwrap_or(SpecialKind::DefaultCtor);
                    let m = self.diag(DiagId::sema_define_implicit_member, sloc).arg(kind.sel());
                    self.emit(m);
                    return None;
                }
                Some(p) if let Some((id, sel)) = self.defaulted_member_definition(p) => {
                    self.emit(self.diag(id, sloc).arg(sel));
                    return None;
                }
                Some(p) => {
                    prev_ok = Some(p);
                    let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    ty = self.merge_cc(ty, pty, p, sloc);
                    // [except.spec]/5, the implicit specification of a
                    // destructor, [except.spec]/8; an unwritten one is
                    // inherited.
                    let fname = self.decl_name_str(name);
                    let fix_at = crate::except::missing_exc_spec_fixit_loc(d);
                    ty = self.check_special_redecl_exc(
                        (p, pty, ty),
                        shape == Shape::Dtor,
                        (sloc, &fname, fix_at),
                    );
                }
                // A declarator with an invalid parameter (a type defined in
                // the parameter clause, [dcl.fct]/17) was diagnosed already
                // and matches nothing (clang's `D.isInvalidType()`).
                None if d.function_params().is_some_and(|ps| ps.iter().any(|p| p.invalid)) => {
                    redecl_invalid = true;
                }
                None => {
                    let what = if templ { None } else { Some(self.decl_name_str(name)) };
                    let what = what.unwrap_or_else(|| self.decl_name_str(name));
                    let m = self
                        .diag(DiagId::sema_member_def_no_match, sloc)
                        .arg(what)
                        .arg(self.class_context_str(ctx));
                    let m = self.with_class_defined_note(m, ctx);
                    self.emit(m);
                    redecl_invalid = true;
                }
            }
        }
        // `[extern] template C<args>::C(params);`: an explicit
        // instantiation of a constructor or destructor of a class
        // template specialization ([temp.explicit]/3) declares nothing.
        if let Some(p) = prev_ok
            && !in_class
            && self.at_explicit_head()
            && let Some((ext, _)) = self.templ.explicit_inst
            && self.ast.member_spec_info(p).is_some()
        {
            self.templ.explicit_inst = None;
            self.templ.cur = None;
            self.act_on_explicit_member_inst(self.ast.first(p), ext.is_some(), sloc);
            return None;
        }
        if let Some(p) = prev_ok
            && !in_class
        {
            self.mark_member_explicit_spec(p);
        }
        let mut spec = FnSpec::new();
        if ds.inline.is_some() {
            spec = spec.with_inline(true);
        }
        if ds.constexpr.is_some() {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr).with_implicit_inline(true);
        } else if ds.consteval.is_some() && shape != Shape::Dtor {
            spec = spec.with_constexpr_kind(ConstexprKind::Consteval).with_implicit_inline(true);
        }
        spec = spec.with_variadic(variadic && shape != Shape::Dtor);
        if let Some(p) = prev_ok
            && self.ast.decls.function(p).is_some_and(|x| x.spec.is_inline())
        {
            spec = spec.with_implicit_inline(true);
        }
        if is_static && !in_class {
            self.error(DiagId::sema_static_out_of_class, static_loc_of(ds));
        }
        if xobj_member {
            spec = spec.with_explicit_object(true);
        }
        let explicit =
            self.check_explicit_spec(ds, d, true, matches!(shape, Shape::Ctor | Shape::Conv(_)));
        let explicit_value = match (explicit, ds.explicit_expr) {
            (true, Some(e)) => self.explicit_value(e),
            (e, _) => Some(e),
        };
        if explicit_value != Some(false) && explicit {
            spec = spec.with_explicit(true);
        }
        // The explicit-specifier is a property of the first declaration
        // ([dcl.fct.spec]/4 allows it only in the class, clang's
        // `getExplicitSpecifier` reads the canonical declaration): an
        // out-of-line definition stays explicit, so copy-initialization
        // still does not consider it ([over.match.copy]/1.1).
        let inherited_explicit = prev_ok.filter(|_| !explicit).and_then(|p| {
            let e = self.ast.decls.cold.explicit_expr.get(p).copied();
            self.fn_spec(p).is_explicit().then_some(e)
        });
        if inherited_explicit.is_some() {
            spec = spec.with_explicit(true);
        }
        // [dcl.constexpr]/1: every declaration of a constexpr (consteval)
        // function is constexpr (consteval) (clang
        // `err_constexpr_redecl_mismatch`, `MergeFunctionDecl`).
        if let Some(p) = prev_ok
            && !in_class
        {
            let (ok, nk) = (self.fn_spec(p).constexpr_kind(), spec.constexpr_kind());
            if ok != nk {
                let m = self
                    .diag(DiagId::sema_constexpr_redecl_mismatch, sloc)
                    .arg(nk as u8)
                    .arg(self.decl_name_str(name))
                    .arg(ok as u8);
                let m = self.note_prev(m, p, false);
                self.emit(m);
                prev_ok = None;
                redecl_invalid = true;
            }
        }
        let fparams: Vec<ParamInfo> =
            if shape == Shape::Dtor { Vec::new() } else { params.clone() };
        // A parameter that names no type makes the function invalid
        // (clang's `ActOnFunctionDeclarator`): an invalid constructor is
        // not a user-declared one and no candidate (WP-318).
        *invalid |= fparams.iter().any(|p| p.invalid);
        let (defaults, redefined) = self.merge_default_args(prev_ok, false, &fparams, true);
        if redefined {
            prev_ok = None;
            redecl_invalid = true;
        }
        let mut h = DeclHead::new(ctx, name, sloc);
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let site = crate::class::VirtualSite {
            class: ctx,
            method: true,
            in_class,
            friend: false,
            prev: prev_ok,
            name,
        };
        let vds = ds_virt.as_ref().unwrap_or(ds);
        let virt = self.virtual_decl(vds, d, ty, site, &mut spec);
        let f = self.builder().function(h, fkind, ty, spec, prev_ok);
        self.finish_virtual_decl(f, &virt);
        if redecl_invalid || *invalid {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        if explicit && let Some(e) = ds.explicit_expr {
            let _ = self.ast.decls.cold.explicit_expr.insert(f, e);
        }
        if let Some(Some(e)) = inherited_explicit {
            let _ = self.ast.decls.cold.explicit_expr.insert(f, e);
        }
        // Where a constexpr or consteval defaulted one that cannot be is
        // diagnosed before C++26 (`check_defaulted_impl`).
        if ds.constexpr.or(ds.consteval).is_some()
            && ds.loc != d.name_loc()
            && self.std < kiln_lex::Std::Cxx26
        {
            self.constexpr_begin_locs.insert(f, ds.loc);
        }
        self.create_params(f, &fparams, &defaults);
        self.defer_late_noexcept(f, d);
        // A destructor without an exception specification gets the
        // implicit one ([except.spec]/8), computed when needed. In a
        // dependent context it is left alone; the instantiation gets its own
        // (clang `AdjustDestructorExceptionSpec` returns early there).
        if shape == Shape::Dtor
            && prev_ok.is_none()
            && !self.ast.decls.get(f).has(DeclFlags::DEPENDENT_CTX)
            && d.late_noexcept.is_none()
            && self.written_exc_spec(ty) == ExcSpec::None
        {
            let t = self.with_exc_spec(ty, ExcSpec::Deferred(f));
            self.ast.decls.set_ty(f, t);
            self.implicit_exc_dtors.insert(f);
        }
        if shape == Shape::Ctor && !*invalid && !redecl_invalid {
            self.check_copy_ctor_by_value(f, class, &fparams);
        }
        if shape == Shape::Assign && !*invalid && !redecl_invalid && !self.check_operator_decl(f) {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        Some(f)
    }

    /// The cv- and ref-qualifier checks of a constructor (`sel` "0") or
    /// destructor ("1") ([class.ctor.general]/3, [class.dtor]/2).
    fn check_ctor_dtor_quals(
        &mut self,
        rparen: Loc,
        quals: kiln_ast::types::Quals,
        ref_qual: RefQual,
        sel: &str,
        invalid: &mut bool,
    ) {
        if quals.is_empty() && ref_qual == RefQual::None {
            return;
        }
        let written = self.written_fn_quals(rparen);
        for (q, sp) in [
            (kiln_ast::types::Quals::CONST, "const"),
            (kiln_ast::types::Quals::VOLATILE, "volatile"),
        ] {
            if quals.contains(q) {
                let loc = written.iter().find(|w| w.0 == sp).map_or(rparen, |w| w.1);
                let m = self.diag(DiagId::sema_ctor_dtor_qualifier, loc).arg(sp).arg(sel);
                self.emit(m);
                *invalid = true;
            }
        }
        if ref_qual != RefQual::None {
            let sp = if ref_qual == RefQual::LValue { "&" } else { "&&" };
            let loc = written.iter().find(|w| w.0 == sp).map_or(rparen, |w| w.1);
            let m = self.diag(DiagId::sema_ctor_dtor_ref_qualifier, loc).arg(sp).arg(sel);
            self.emit(m);
            *invalid = true;
        }
    }

    /// The qualifier tokens (`const`, `volatile`, `&`, `&&`) written
    /// after the `)` at `rparen`, with their locations (the declarator
    /// keeps the qualifiers, not where they were written).
    fn written_fn_quals(&self, rparen: Loc) -> Vec<(&'static str, Loc)> {
        let mut out = Vec::new();
        let sm = self.diags.sm();
        let Some((file, off)) = sm.offset_of(rparen) else { return out };
        let text = sm.text(file).as_bytes();
        let mut i = off as usize + 1;
        while i < text.len() {
            while i < text.len() && text[i].is_ascii_whitespace() {
                i += 1;
            }
            let start = i;
            let word: &'static str = if text[i..].starts_with(b"&&") {
                i += 2;
                "&&"
            } else if text[i..].starts_with(b"&") {
                i += 1;
                "&"
            } else {
                while i < text.len() && (text[i].is_ascii_alphanumeric() || text[i] == b'_') {
                    i += 1;
                }
                match &text[start..i] {
                    b"const" => "const",
                    b"volatile" => "volatile",
                    _ => break,
                }
            };
            let Ok(o) = u32::try_from(start) else { break };
            out.push((word, sm.loc(file, o)));
        }
        out
    }

    /// The location of the first token after `loc`'s (one byte long)
    /// token: the class name after a destructor's `~`.
    pub(crate) fn next_token_loc(&self, loc: Loc) -> Loc {
        let sm = self.diags.sm();
        let Some((file, off)) = sm.offset_of(loc) else { return loc };
        let text = sm.text(file).as_bytes();
        let mut i = off as usize + 1;
        while i < text.len() && text[i].is_ascii_whitespace() {
            i += 1;
        }
        u32::try_from(i).map_or(loc, |o| sm.loc(file, o))
    }

    /// [class.dtor]/1: the class-name after `~` names the class
    /// (clang's `ActOnDestructorName`). Reports and returns false
    /// otherwise.
    fn dtor_name_matches(
        &mut self,
        class: DeclId,
        n: Symbol,
        qualifier: Option<DeclId>,
        tilde: Loc,
    ) -> bool {
        let mut found = if qualifier.is_some() {
            self.lookup_in_class(class, n, LookupKind::Ordinary).decls
        } else {
            Vec::new()
        };
        if found.is_empty() {
            found = self.lookup_name(n, LookupKind::Ordinary).decls;
        }
        let names_class = |s: &Self, x: DeclId| {
            let k = s.kind_of(x);
            (crate::lookup::is_tag_kind(k) || k == DeclKind::Typedef || k == DeclKind::TypeAlias)
                && s.ast.decls.get(x).ty().is_some_and(|t| s.is_class_type_of(t, class))
        };
        if found.iter().any(|&x| names_class(self, x)) || self.is_dependent_ctx(class) {
            return true;
        }
        let loc = self.next_token_loc(tilde);
        if found.is_empty() {
            let m = self
                .diag(DiagId::sema_dtor_undeclared_name, loc)
                .arg(self.ast.symbols().as_str(n).to_owned());
            self.emit(m);
        } else {
            self.error(DiagId::sema_dtor_class_name, loc);
        }
        false
    }

    /// `-Wclass-conversion` ([class.conv.fct]/1 note: a conversion to the
    /// class itself, a base or `void` is never used).
    /// The conversion function templates declared in class definition
    /// `def` (first declarations): a specialization of one is not found
    /// by name lookup, every one is considered instead ([temp.mem]/6).
    pub(crate) fn conversion_templates_of(&self, def: DeclId) -> Vec<DeclId> {
        let mut out = Vec::new();
        for m in self.ast.decls.members(def) {
            if self.kind_of(m) == DeclKind::FunctionTemplate
                && self
                    .templated_function(m)
                    .is_some_and(|f| self.kind_of(f) == DeclKind::CXXConversion)
            {
                let m = self.ast.first(m);
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
        out
    }

    fn warn_class_conversion(&mut self, class: DeclId, target: QualType, loc: Loc) {
        let t = self.non_reference(target);
        let t = self.ast.types.canonical(t).without_quals();
        let class_ty = self.class_type(class);
        let cls = self.ty_arg(class_ty);
        if let Some(r) = self.ast.types.as_record(t) {
            if self.ast.first(r) == self.ast.first(class) {
                let m = self.diag(DiagId::sema_conv_to_self, loc).arg(cls);
                self.emit(m);
            } else if crate::conv::bases::is_derived_from(self, class, r) {
                let m = self.diag(DiagId::sema_conv_to_base, loc).arg(cls).arg(self.ty_arg(t));
                self.emit(m);
            }
        } else if self.ast.types.is_void(t) {
            let m = self.diag(DiagId::sema_conv_to_void, loc).arg(cls).arg(self.ty_arg(t));
            self.emit(m);
        }
    }

    /// [class.copy.ctor]/5: a constructor whose first parameter is the
    /// class itself (by value) and whose others have defaults is
    /// ill-formed (clang `CheckConstructor`).
    fn check_copy_ctor_by_value(&mut self, f: DeclId, class: DeclId, params: &[ParamInfo]) {
        let Some(first) = params.first() else { return };
        if params[1..].iter().any(|p| p.default_arg == crate::declspec::DefaultArg::None) {
            return;
        }
        let t = self.ast.types.canonical(first.ty).without_quals();
        if self.ast.types.as_record(t).is_some_and(|r| self.ast.first(r) == self.ast.first(class))
            && !self.ast.types.is_reference(first.ty)
        {
            let loc = match first.name {
                Some((_, l)) => l,
                None if first.default_arg == crate::declspec::DefaultArg::None
                    && first.range.1.is_valid() =>
                {
                    first.range.1
                }
                None => first.loc,
            };
            self.error(DiagId::sema_copy_ctor_by_value, loc);
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
    }

    /// Function type `ty` with return type `ret`, parameters `params`
    /// (kept when `None`) and, with `strip`, no cv- or ref-qualifiers.
    pub(crate) fn rebuild_fn_type(
        &mut self,
        ty: QualType,
        ret: QualType,
        params: Option<&[QualType]>,
        strip: bool,
    ) -> QualType {
        let t = &self.ast.types;
        let Some(p) = t.proto_of(ty.ty()).or_else(|| t.proto(ty)).copied() else { return ty };
        let ps = match params {
            Some(ps) => ps.to_vec(),
            None => self.ast.types.params(&p).to_vec(),
        };
        let mut info = p.info();
        if strip {
            info = info.with_this_quals(kiln_ast::types::Quals::NONE).with_ref_qual(RefQual::None);
        }
        if params.is_some() {
            info = info.with_variadic(false);
        }
        let exc = self.written_exc_spec(ty);
        let _: FuncInfo = info;
        self.ast.types.function(ret, &ps, info, exc)
    }

    /// Function type `ty` with exception specification `exc`.
    pub(crate) fn with_exc_spec(&mut self, ty: QualType, exc: ExcSpec) -> QualType {
        let t = &self.ast.types;
        let Some(p) = t.proto_of(ty.ty()).or_else(|| t.proto(ty)).copied() else { return ty };
        let ps = self.ast.types.params(&p).to_vec();
        self.ast.types.function(p.ret(), &ps, p.info(), exc)
    }
}

/// Where `static` was written.
fn static_loc_of(ds: &DeclSpec) -> Loc {
    match ds.storage {
        Some((_, l)) => l,
        None => ds.loc,
    }
}
