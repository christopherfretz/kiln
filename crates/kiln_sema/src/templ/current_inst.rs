//! Calls naming members of the current instantiation through `this`
//! ([temp.dep.expr]/5, WP-1570, WP-1609; spec/sema-templates.md
//! "Calls through `this` of the current instantiation"): when the call
//! cannot resolve differently at instantiation, it gets its result type
//! at definition. The call keeps its callee as written (a dependent
//! member access) and is rebuilt, and its overload resolution redone, at
//! instantiation.

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, UnaryOp, ValueCategory};
use kiln_ast::ids::RawId;
use kiln_ast::names::DeclNameKind;
use kiln_ast::types::{TemplateArg, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};

use crate::Sema;
use crate::lookup::LookupKind;
use crate::overload::{ArgInfo, CandidateKind, IcsOptions, OvlOutcome, SetKind};

impl Sema<'_, '_> {
    /// The type and value category of the call `x(args)`, `x` the
    /// dependent member access `this->f`, `(*this).f` or the implicit
    /// `this->f` of an unqualified `f`, naming an overload set or a
    /// member template of the current instantiation; `None` keeps the
    /// call type-dependent. The cheap checks run first: the access has no
    /// qualifier and no template arguments, its object is `this` of a
    /// class that is complete here and has no dependent base (a dependent
    /// base may hide or add a member at instantiation, [temp.dep.type]/7),
    /// no argument is a braced list. Then lookup must find only
    /// candidates instantiation cannot change
    /// ([`Self::fixed_member_candidate`]), and overload resolution among
    /// them against `*this` must succeed (a call that is not viable,
    /// ambiguous or names a deleted function stays dependent: it is
    /// diagnosed at instantiation, as clang does for every call through
    /// `this`), with a winner whose type and template arguments are not
    /// dependent (a member template's deduced specialization decides:
    /// `template <class U> T f(U)` called as `f(1)` stays dependent).
    /// Nothing in the candidates or the arguments can change at
    /// instantiation, so the winner is the one instantiation chooses.
    pub(crate) fn current_instantiation_set_call_type(
        &mut self,
        x: ExprId,
        args: &[ExprId],
    ) -> Option<(QualType, ValueCategory)> {
        let n = *self.ast.exprs.get(x);
        let name = self.ast.dependent_member_name(x);
        if name.kind() == DeclNameKind::Conversion {
            return None;
        }
        let base = self.ignore_paren_imp_casts(ExprId::from_raw(n.a())?);
        let this = if n.aux() & 1 != 0 {
            base
        } else {
            let b = self.ast.exprs.get(base);
            if b.kind() != ExprKind::UnaryOperator
                || self.ast.exprs.unary_op(base) != UnaryOp::Deref
            {
                return None;
            }
            self.ignore_paren_imp_casts(self.ast.exprs.operand(base))
        };
        if self.ast.exprs.get(this).kind() != ExprKind::CXXThisExpr {
            return None;
        }
        let pointee = self.pointee_sugared(self.ety(this))?;
        let class = self.ast.types.as_record(pointee)?;
        let class = self.ast.definition(class)?;
        if !self.current_instantiation_complete(class) {
            return None;
        }
        let scope = match self.ast.templates.expr_nns.get(x).copied() {
            Some(q) => self.qualifier_in_current_instantiation(q, class, true)?,
            None => class,
        };
        let quals = self.ast.types.effective_quals(pointee);
        let obj = self.ast.types.qualify(self.tag_type(class), quals);
        let targs = self.ast.templates.expr_targs.get(x).copied();
        self.resolve_fixed_members(Some(obj), scope, name, targs, args, n.loc())
    }

    /// [`Self::current_instantiation_set_call_type`] for the qualified
    /// overload set `C::f` (an `UnresolvedLookupExpr` without
    /// argument-dependent lookup) of static member functions of the
    /// current instantiation `C`, called without an object.
    pub(crate) fn current_instantiation_static_call_type(
        &mut self,
        x: ExprId,
        args: &[ExprId],
    ) -> Option<(QualType, ValueCategory)> {
        let n = *self.ast.exprs.get(x);
        let q = self.ast.templates.expr_nns.get(x).copied()?;
        if n.aux() & 1 != 0 {
            return None;
        }
        let class = self.qualifier_in_current_instantiation(q, DeclId::new(1), false)?;
        if !self.current_instantiation_complete(class)
            || !self.ast.unresolved_decls(x).iter().all(|&d| {
                self.ast.decls.get(d).ctx().map(|c| self.ast.first(c))
                    == Some(self.ast.first(class))
                    && !self.is_instance_member(self.templated_or_self(d))
            })
        {
            return None;
        }
        let name = self.ast.unresolved_name(x);
        let targs = self.ast.templates.expr_targs.get(x).copied();
        self.resolve_fixed_members(None, class, name, targs, args, n.loc())
    }

    /// The class the qualifier `q` of a member of the current
    /// instantiation names: the current instantiation itself (`C::f`,
    /// for `base` any class whose first declaration is `class`'s) or,
    /// with `base`, a class that is not dependent and a base of `class`
    /// (`Q::f`). `None` for anything else, which stays dependent.
    fn qualifier_in_current_instantiation(
        &self,
        q: kiln_ast::NnsId,
        class: DeclId,
        base: bool,
    ) -> Option<DeclId> {
        let kiln_ast::types::Nns::Type { ty, .. } = self.ast.types.nns_view(q) else { return None };
        let r = self.ast.types.as_record(ty)?;
        let r = self.ast.definition(r)?;
        if !base {
            return self.ast.types.is_dependent(ty).then_some(r);
        }
        (self.ast.first(r) == self.ast.first(class)
            || !self.ast.types.is_dependent(ty)
                && crate::conv::bases::is_derived_from(self, class, r))
        .then_some(r)
    }

    /// The type and value category of the call of the members `name` of
    /// the class `scope` (looked up there), with the explicit template
    /// arguments `targs`, on the object type `object` (`None`: without
    /// an object, static members only), with the arguments `args`, when
    /// instantiation cannot resolve it differently (see
    /// [`Self::current_instantiation_set_call_type`]).
    fn resolve_fixed_members(
        &mut self,
        object: Option<QualType>,
        scope: DeclId,
        name: kiln_ast::names::DeclName,
        targs: Option<kiln_ast::ids::ArgListId>,
        args: &[ExprId],
        loc: kiln_source::Loc,
    ) -> Option<(QualType, ValueCategory)> {
        if args.iter().any(|&a| self.ast.exprs.get(a).kind() == ExprKind::InitListExpr)
            || targs.is_some_and(|t| {
                self.ast.types.arg_list_flags(t).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
            })
        {
            return None;
        }
        let found = self.lookup_in_class_dn(scope, name, LookupKind::Ordinary);
        if found.decls.is_empty()
            || found.ambiguity.is_some()
            || !found.decls.iter().all(|&d| self.fixed_member_candidate(d))
        {
            return None;
        }
        let kind = if object.is_some() { CandidateKind::Method } else { CandidateKind::Function };
        let mut set = self.begin_set(SetKind::Normal, loc);
        if targs.is_none() {
            self.defer_templates(&mut set);
        }
        let mark = self.push_call_args(&mut set, args, &[]);
        set.object = object.map(|o| ArgInfo::of_type(o, ValueCategory::LValue));
        for (&d, &f) in found.decls.iter().zip(&found.found) {
            if self.kind_of(d) != DeclKind::FunctionTemplate {
                // A template-id names only the templates
                // ([temp.arg.explicit]/4).
                if targs.is_none() {
                    self.add_candidate(&mut set, d, f, kind, IcsOptions::NONE);
                }
            } else if set.two_phase {
                set.deferred.push(crate::overload::defer::Deferred::Template {
                    ft: d,
                    found: f,
                    kind,
                    first_opts: IcsOptions::NONE,
                    loc,
                    poi: self.templ.deduce_poi,
                    adl_only: false,
                });
            } else {
                self.add_template_candidate(&mut set, d, f, targs, kind, IcsOptions::NONE, loc);
            }
        }
        let out = self.resolve_set(&mut set);
        self.ovl.lists.truncate(mark);
        let won = match out {
            OvlOutcome::Success(b) if !set.uncertain => Some(set.cand(b).func()),
            _ => None,
        };
        self.end_set(set);
        let won = won?;
        if let Some((_, info)) = self.ast.spec_info(won)
            && self
                .ast
                .types
                .arg_list_flags(info.args)
                .intersects(TypeFlags::DEPENDENT_INSTANTIATION)
        {
            return None;
        }
        let fty = self.ast.decls.get(won).ty()?;
        if self.ast.types.is_dependent(fty) || self.contained_placeholder(fty).is_some() {
            return None;
        }
        self.current_instantiation_result(fty)
    }

    /// Whether the class `class` of `this` is complete here, so lookup in
    /// it finds now every member it finds at instantiation: defined, not
    /// still being defined (a member declared later in a complete-class
    /// context), and without a dependent base.
    pub(crate) fn current_instantiation_complete(&self, class: DeclId) -> bool {
        !self.defining.contains(&class)
            && !self.suspended_defining.contains(&class)
            && !self.has_dependent_bases(class)
    }

    /// Whether the member `d` found by lookup in the current
    /// instantiation is a candidate that instantiation cannot change:
    /// a valid member function without an explicit object parameter (its
    /// object conversion is checked as a parameter) or a requires-clause
    /// (satisfaction is checked per specialization), whose type holds no
    /// placeholder; a non-template's type is not dependent; a member
    /// template's function type, parameter types and default template
    /// arguments name no template parameter of an enclosing template
    /// (deduction then succeeds or fails alike at instantiation), and it
    /// has no constraint and no template template parameter.
    fn fixed_member_candidate(&self, d: DeclId) -> bool {
        let tmpl = (self.kind_of(d) == DeclKind::FunctionTemplate).then_some(d);
        let f = match tmpl {
            Some(t) => match self.pattern_of(t) {
                Some(p) => p,
                None => return false,
            },
            None => d,
        };
        if self.kind_of(f) != DeclKind::CXXMethod
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
            || self.has_explicit_object(f)
            || self.ast.decls.cold.requires_clause.contains(f)
            || self.ast.templates.trailing_requires.get(f).is_some()
        {
            return false;
        }
        let Some(ty) = self.ast.decls.get(f).ty() else { return false };
        if self.contained_placeholder(ty).is_some() {
            return false;
        }
        let Some(t) = tmpl else { return !self.ast.types.is_dependent(ty) };
        let Some(info) = self.ast.template_info(t) else { return false };
        let list = *self.ast.templates.param_list(info.params);
        if list.requires.is_some() {
            return false;
        }
        let mut used = [false; 256];
        let mut outer = |s: &Self, ty: Option<QualType>, arg: Option<TemplateArg>| {
            (0..u32::from(list.depth)).any(|depth| {
                used.fill(false);
                if let Some(ty) = ty {
                    s.mark_used_type(ty, depth, &mut used, 0);
                }
                if let Some(a) = arg {
                    s.mark_used_arg(a, depth, &mut used, 0);
                }
                used.contains(&true)
            })
        };
        if outer(self, Some(ty), None) {
            return false;
        }
        self.ast.lists.iter::<DeclId>(list.params).all(|p| {
            let parm = self.ast.template_parm(p);
            self.kind_of(p) != DeclKind::TemplateTemplateParm
                && parm.is_none_or(|i| i.constraint.is_none())
                && !outer(self, self.ast.decls.get(p).ty(), None)
                && self.ast.templates.defaults.get(p).is_none_or(|&a| !outer(self, None, Some(a)))
        })
    }

    /// The type and value category of a call of a function of the
    /// non-dependent type `fty` ([expr.call]/14), or `None` when the
    /// result must stay dependent: an invalid return type, or one naming
    /// an incomplete class (`Heap *`), which may be complete at
    /// instantiation: a member access into it is checked then, not
    /// rejected at definition (GCC only warns there;
    /// `g++.dg/template/incomplete8.C`).
    pub(crate) fn current_instantiation_result(
        &self,
        fty: QualType,
    ) -> Option<(QualType, ValueCategory)> {
        let ret = self.sugared_proto(fty)?.ret();
        if ret == QualType::ERROR {
            return None;
        }
        let r = self.non_reference(ret);
        let obj = self.pointee_sugared(r).unwrap_or(r);
        if self.ast.types.as_record(obj).is_some() && !self.is_complete(obj) {
            return None;
        }
        Some(if self.ast.types.is_reference(ret) {
            let lvalue = matches!(self.ast.types.canonical_view(ret), TypeView::LValueRef(_))
                || self.ast.types.is_function(r);
            (r, if lvalue { ValueCategory::LValue } else { ValueCategory::XValue })
        } else if self.ast.types.as_record(ret).is_some() {
            (ret, ValueCategory::PRValue)
        } else {
            (crate::conv::strip_cv(self, ret), ValueCategory::PRValue)
        })
    }
}
