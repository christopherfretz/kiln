//! Parser actions for template-ids, dependent names and the dependent
//! expression forms (spec/sema-templates.md §2.2-§2.7, §11.2). Bodies
//! build nodes and check kinds only; everything that needs the engine
//! (WP-070..074) reports `sema_unimplemented_template_feature`.

use kiln_ast::decl::{DeclFlags, DeclKind, FriendKind};
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind};
use kiln_ast::ids::{ArgListId, NnsId, TemplateNameId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::templates::{Requirement, TemplateParmInfo};
use kiln_ast::types::{
    ArgView, Nns, TagKeyword, TemplateArg, TemplateName, TypeFlags, TypeKind, TypeView,
};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::decl::sym_str;
use crate::lookup::LookupKind;
use crate::scope::{Scope, ScopeKind};
use crate::templ::MemberScope;
use crate::templ::written::WrittenKey;

/// What a template-id names ([temp.names]/3-8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateIdRef {
    /// A class, alias or template template parameter specialization.
    Type(QualType),
    /// A concept-id (`ConceptSpecializationExpr`) or a variable template
    /// specialization (an `UnresolvedLookupExpr` while dependent).
    Value(ExprId),
    /// A function template-id: an `UnresolvedLookupExpr` naming the
    /// templates, resolved by the call (WP-071).
    Overload(ExprId),
}

impl Sema<'_, '_> {
    // -- dependent qualifiers --------------------------------------------------

    /// True if qualifier context `d` is a dependent marker
    /// (spec/sema-templates.md §11 "Landed"): a type template parameter,
    /// template template parameter, a class or alias template named with
    /// dependent arguments that are not the current instantiation, or a
    /// typedef of a dependent type. Qualified lookup into it finds nothing
    /// and names after it are dependent ([temp.dep.type]/6).
    pub fn is_dependent_ctx(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::TemplateTypeParm
            | DeclKind::TemplateTemplateParm
            | DeclKind::ClassTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::BuiltinTemplate
            | DeclKind::UnresolvedUsingTypename => true,
            DeclKind::Typedef | DeclKind::TypeAlias => {
                self.ast.decls.get(d).ty().is_some_and(|t| {
                    self.is_dependent_type(t) && self.ast.types.as_record(t).is_none()
                })
            }
            _ => false,
        }
    }

    /// Whether class `c`, named by a nested-name-specifier, is declared in
    /// a dependent context but is not the current instantiation here
    /// ([temp.dep.type]/1: a local class of a function template, or a
    /// member class of a class template, named outside its own
    /// definition; clang `computeDeclContext` returns no context for it).
    /// A type found in it is a dependent qualified name, so it needs
    /// `typename` where [temp.res.general]/4 implies none (spec/sema-
    /// templates.md §2.3). kiln still looks the name up in `c`.
    pub fn is_unentered_dependent_class(&self, c: DeclId) -> bool {
        self.kind_of(c) == DeclKind::CXXRecord
            && self.ast.decls.get(c).has(DeclFlags::DEPENDENT_CTX)
            && self.inst.stack.is_empty()
            && !self.encloses(c, self.ctx)
            && !self.in_declarator_scope_of(c)
    }

    /// Whether `class-key c::name` names a member of an unknown
    /// specialization ([temp.dep.type]/6): `c` is a class template's
    /// pattern named by a dependent specifier outside its definition
    /// (`struct A<T>::K` in another template) and has no tag `name`, which
    /// an explicit or partial specialization may declare (clang builds a
    /// `DependentNameType` without looking).
    pub fn is_unknown_specialization_tag(&self, c: DeclId, name: Symbol) -> bool {
        self.is_unentered_dependent_class(c)
            && self.lookup_qualified(c, name, LookupKind::Tag).decls.is_empty()
    }

    /// The type `q::name` names when qualified lookup did not find a type
    /// in `q` and `q` can name an unknown specialization ([temp.dep.type]/6,
    /// clang `CheckTypenameType` / `isDependentScopeSpecifier`): a
    /// dependent marker (`T::B`), or the current instantiation with a
    /// dependent base (`D::B`, clang `hasAnyDependentBases`), whose
    /// specifier is rebuilt on its injected-class-name type so that
    /// instantiation looks `name` up in the specialization. A
    /// `DependentNameType` with keyword `kw`; `None` otherwise. Used for a
    /// qualified mem-initializer-id ([class.base.init]/2) and a
    /// `typename`-specifier ([temp.res.general]/5).
    pub fn act_unknown_specialization_type(
        &mut self,
        q: crate::using::Qualifier,
        name: Symbol,
        kw: TagKeyword,
        loc: Loc,
    ) -> Option<QualType> {
        let nns = self.unknown_specialization_nns(q)?;
        Some(self.act_dependent_name_at(nns, name, None, kw, loc))
    }

    /// The specifier of `q::name` in an id-expression: `q`'s own when `q`
    /// is dependent; for `q::template name<...>` that lookup finds nothing
    /// for, the [`Sema::unknown_specialization_nns`] of `q` (the current
    /// instantiation with a dependent base, `D::template f<U>(x)`;
    /// [temp.dep.type]/6, WP-803); `None` otherwise.
    pub fn dep_member_nns(
        &mut self,
        q: crate::using::Qualifier,
        name: Symbol,
        template_id: bool,
    ) -> Option<NnsId> {
        if self.is_dependent_ctx(q.ctx) {
            Some(q.nns)
        } else if template_id
            && matches!(
                self.classify_name(name, Some(q.ctx)),
                crate::decl::NameClass::Undeclared | crate::decl::NameClass::Dependent
            )
        {
            self.unknown_specialization_nns(q)
        } else {
            None
        }
    }

    /// The specifier under which `q::name` names a member of an unknown
    /// specialization when lookup found nothing in `q` ([temp.dep.type]/6):
    /// `q`'s own when it is dependent, the injected-class-name type's when
    /// `q` is the current instantiation with a dependent base; `None`
    /// otherwise.
    pub(crate) fn unknown_specialization_nns(
        &mut self,
        q: crate::using::Qualifier,
    ) -> Option<NnsId> {
        if self.is_dependent_ctx(q.ctx) {
            Some(q.nns)
        } else if self.kind_of(q.ctx) == DeclKind::CXXRecord && self.has_dependent_bases(q.ctx) {
            let prefix = match self.ast.types.nns_view(q.nns) {
                Nns::Type { prefix, .. } => prefix,
                _ => None,
            };
            let ty = self.this_class_type(q.ctx);
            Some(self.ast.types.nns(Nns::Type { prefix, ty, template_kw: false }))
        } else if self.is_unentered_dependent_class(q.ctx) && self.ast.definition(q.ctx).is_none() {
            // A member class of the current instantiation declared but not
            // yet defined (`struct Block;` defined out of line after the
            // member functions that name `Block::get`): its specifier is
            // dependent (clang `computeDeclContext` has no context for
            // it), so the name is looked up at instantiation (WP-726).
            Some(q.nns)
        } else {
            None
        }
    }

    /// True if `t` is a dependent type ([temp.dep.type]/7): its meaning
    /// (canonical type) is unknown until instantiation, so semantic
    /// checks on it wait (spec/sema-templates.md §3.1).
    pub fn is_dependent_type(&self, t: QualType) -> bool {
        self.ast.types.is_dependent(t)
    }

    /// True if `t` is dependent or instantiation-dependent: it mentions a
    /// template parameter or a template's local somewhere, even where its
    /// canonical type is known (`void_t<T>`, `TC<N>*` for a constant local
    /// `N`, a `decltype` of such an expression). Substitution must still
    /// visit it (spec/sema-templates.md §3.1).
    pub fn is_instantiation_dependent_type(&self, t: QualType) -> bool {
        self.ast.types.flags(t.ty()).intersects(TypeFlags::DEPENDENT_INSTANTIATION)
    }

    /// One component `name::` of a nested-name-specifier after `prefix`
    /// (its context and specifier): [`Sema::act_on_nested_name_spelled`]
    /// plus the dependent forms (`T::`, `T::X::`, [temp.dep.type]/6).
    pub fn act_on_nested_name_component(
        &mut self,
        prefix: Option<(DeclId, NnsId)>,
        name: Symbol,
        loc: Loc,
    ) -> Option<(DeclId, NnsId)> {
        if let Some((p, pn)) = prefix
            && self.is_dependent_ctx(p)
        {
            let n = self.ast.types.nns(Nns::Identifier { prefix: Some(pn), name });
            self.note_written_loc(WrittenKey::Nns(n), loc);
            return Some((p, n));
        }
        let scope = self.templ.member_scope.take();
        if prefix.is_none()
            && let Some(scope) = scope
            && let Some(r) = self.member_qualifier_first(scope, name, loc)
        {
            return r.ok();
        }
        // `D::X::` with `D` the current instantiation, a dependent base
        // and nothing named `X` found in it: a member of an unknown
        // specialization ([temp.dep.type]/6), looked up again at
        // instantiation (`using G::F::F;`, [over.match.class.deduct]/1's
        // example).
        if let Some((p, pn)) = prefix
            && self.kind_of(p) == DeclKind::CXXRecord
            && self.has_dependent_bases(p)
            && self.lookup_qualified(p, name, LookupKind::Nested).decls.is_empty()
            && self.lookup_qualified(p, name, LookupKind::Ordinary).decls.is_empty()
            && let Some(un) =
                self.unknown_specialization_nns(crate::using::Qualifier { ctx: p, nns: pn, loc })
        {
            let n = self.ast.types.nns(Nns::Identifier { prefix: Some(un), name });
            self.note_written_loc(WrittenKey::Nns(n), loc);
            return Some((self.decltype_scope_marker(), n));
        }
        let (ctx, spelled) =
            self.act_on_nested_name_written(prefix.map(|(p, n)| (p, Some(n))), name, loc)?;
        let n = self.nns_component(prefix.map(|p| p.1), ctx, spelled);
        if self.ast.types.nns_flags(n).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
            self.note_written_loc(WrittenKey::Nns(n), loc);
        }
        Some((ctx, n))
    }

    /// Starts the nested-name-specifier of a member access `base.N::m` /
    /// `base->N::m`: its first component is looked up in the object's
    /// class, then in the context ([basic.lookup.qual.general]/2). Ended
    /// by [`Sema::end_member_qualifier`].
    pub fn begin_member_qualifier(&mut self, base: ExprId, arrow: bool) {
        let bt = self.ety(base);
        let obj = if self.is_err_expr(base) {
            None
        } else if arrow {
            if self.ast.types.is_pointer(bt) {
                self.ast.types.pointee(bt)
            } else if self.ast.types.is_dependent(bt) {
                Some(QualType::DEPENDENT)
            } else {
                None
            }
        } else {
            Some(bt)
        };
        self.templ.member_scope = obj.and_then(|o| {
            if let Some(r) = self.ast.types.as_record(o) {
                Some(MemberScope::Class(self.ast.definition(r).unwrap_or(r)))
            } else if self.ast.types.is_dependent(o) {
                Some(MemberScope::Dependent)
            } else {
                None
            }
        });
    }

    /// Ends [`Sema::begin_member_qualifier`].
    pub fn end_member_qualifier(&mut self) {
        self.templ.member_scope = None;
    }

    /// The first component `name::` of a member access's qualifier in
    /// object scope `scope`: a type or template found in the object's
    /// class; for a dependent object, a name found nowhere is a
    /// dependent `name::` (clang's `ActOnCXXNestedNameSpecifier` with a
    /// dependent object type). `None` to use ordinary lookup, `Err`
    /// after diagnosing.
    fn member_qualifier_first(
        &mut self,
        scope: MemberScope,
        name: Symbol,
        loc: Loc,
    ) -> Option<Result<(DeclId, NnsId), ()>> {
        let dependent_class = match scope {
            MemberScope::Class(c) => {
                let found = self.lookup_in_class(c, name, LookupKind::Nested);
                if !found.decls.is_empty() {
                    let r = self.nested_name_from_found(None, name, &found, loc).map(
                        |(ctx, spelled)| {
                            let n = self.nns_component(None, ctx, spelled);
                            (ctx, n)
                        },
                    );
                    return Some(r.ok_or(()));
                }
                self.is_dependent_type(self.tag_type(c))
            }
            MemberScope::Dependent => true,
        };
        if !dependent_class
            || !self.lookup_name(name, LookupKind::Nested).decls.is_empty()
            || !self.lookup_name(name, LookupKind::Ordinary).decls.is_empty()
        {
            return None;
        }
        let n = self.ast.types.nns(Nns::Identifier { prefix: None, name });
        Some(Ok((self.decltype_scope_marker(), n)))
    }

    /// `decltype(e)::`, the first component of a nested-name-specifier
    /// ([expr.prim.id.qual]/1, [dcl.type.decltype]): `ty` is the
    /// `decltype` type, kept as written in a `Type` specifier (clang's
    /// `TypeSpec` with a `DecltypeType`; printed `decltype(e)::`). A
    /// dependent type gives the dependent-scope marker (names after it
    /// are dependent, [temp.dep.type]/6; substitution re-evaluates the
    /// `decltype` and looks the name up again, §5.3); otherwise the type
    /// must be a complete class or an enumeration ("'decltype(e)' (aka
    /// 'int') is not a class, namespace, or enumeration").
    pub fn act_on_nested_name_decltype(
        &mut self,
        ty: QualType,
        loc: Loc,
    ) -> Option<(DeclId, NnsId)> {
        let n = self.ast.types.nns(Nns::Type { prefix: None, ty, template_kw: false });
        if self.ast.types.flags(ty.ty()).contains(TypeFlags::DEPENDENT) {
            return Some((self.decltype_scope_marker(), n));
        }
        let t = &self.ast.types;
        let Some(r) = t.as_record(ty).or_else(|| t.as_enum(ty)) else {
            let msg = self.diag(DiagId::sema_expected_class_or_namespace, loc).arg(self.ty_arg(ty));
            self.emit(msg);
            return None;
        };
        if !self.require_complete_nns(ty, loc) {
            return None;
        }
        Some((self.ast.definition(r).unwrap_or(r), n))
    }

    /// The qualifier context of a dependent `decltype(e)::`: one implicit
    /// alias of `<dependent type>` linked into no context, so that
    /// [`Sema::is_dependent_ctx`] holds for it (the specifier itself
    /// carries the `decltype`).
    pub(crate) fn decltype_scope_marker(&mut self) -> DeclId {
        if let Some(d) = self.templ.decltype_scope {
            return d;
        }
        let h =
            DeclHead::new(self.ast.tu, DeclName::EMPTY, Loc::INVALID).flags(DeclFlags::IMPLICIT);
        let d = self.ast.builder().unlinked_alias(h, QualType::DEPENDENT);
        self.templ.decltype_scope = Some(d);
        d
    }

    /// `prefix X<args>::` where the template-id named type `ty` of
    /// template `tmpl` ([temp.res.general]/2): the current instantiation
    /// (its pattern, [temp.dep.type]/1), a dependent marker, or the
    /// instantiated specialization ([temp.inst]/2).
    pub fn act_on_nested_name_template_id(
        &mut self,
        prefix: Option<NnsId>,
        tmpl: DeclId,
        ty: QualType,
        template_kw: bool,
        loc: Loc,
    ) -> Option<(DeclId, NnsId)> {
        let n = self.ast.types.nns(Nns::Type { prefix, ty, template_kw });
        // A dependent `X<args>::` is completed at instantiation; its point
        // of instantiation is where the template-id was written (§5.8,
        // clang `RequireCompleteDeclContext`'s last qualifier name; clang
        // 22: the whole specifier's begin, as `nns_poi`).
        if self.ast.types.nns_flags(n).intersects(TypeFlags::DEPENDENT_INSTANTIATION) {
            let at = if self.clang_compat >= 22 { self.nns_start.unwrap_or(loc) } else { loc };
            self.note_written_loc(WrittenKey::Nns(n), at);
        }
        if let Some(p) = self.current_instantiation(tmpl, ty) {
            return Some((p, n));
        }
        if let Some(p) = self.entered_partial_by_converted_args(tmpl, ty, loc) {
            return Some((p, n));
        }
        if !self.is_dependent_type(ty) {
            // [expr.prim.id.qual]/1: the class must be complete.
            if !self.require_complete_nns(ty, loc) {
                return None;
            }
            let r = self.ast.types.as_record(ty)?;
            return Some((self.ast.definition(r).unwrap_or(r), n));
        }
        // `template<class T> void P<T*>::f()`: a declarator qualifier
        // naming a partial specialization by its own arguments
        // ([temp.spec.partial.general]/7, [temp.class.general]/3).
        if self.templ.entering_context
            && let TypeView::TemplateSpecialization { args, .. } = self.ast.types.view(ty.ty())
        {
            let canonical = self.ast.types.canonical_args(args);
            let t = self.ast.first(tmpl);
            if let Some(i) = self.ast.templates.find_partial(t, canonical)
                && let Some(p) = self.ast.templates.partial(i)
            {
                let p = p.partial;
                return Some((self.ast.definition(p).unwrap_or(p), n));
            }
            self.templ.unmatched_qualifier = Some((tmpl, n));
        }
        Some((tmpl, n))
    }

    /// The pattern definition named by `X<args>` when `args` are `X`'s own
    /// parameters in order, or the arguments of one of `X`'s partial
    /// specializations ([temp.spec.partial.member]: its out-of-line
    /// members), and that definition (or one of its members) is being
    /// processed ([temp.dep.type]/1.2).
    pub(crate) fn current_instantiation(&self, tmpl: DeclId, ty: QualType) -> Option<DeclId> {
        if self.kind_of(tmpl) != DeclKind::ClassTemplate {
            return None;
        }
        let TypeView::TemplateSpecialization { args, .. } = self.ast.types.view(ty.ty()) else {
            return None;
        };
        // A friend redeclaration nested in a class template has deeper
        // parameters (libc++ `template <class _Up> friend class weak_ptr;`
        // inside `weak_ptr`).
        let own_decl = self.head_template(tmpl);
        let info = self.ast.template_info(own_decl).copied()?;
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let written = self.ast.types.args(args).to_vec();
        let own = params.len() == written.len()
            && params.iter().zip(&written).all(|(&p, &a)| self.arg_names_param(p, a));
        let pat = if own {
            self.own_args_partial(tmpl, args, &list).unwrap_or(self.pattern_of(tmpl)?)
        } else {
            let canonical = self.ast.types.canonical_args(args);
            let i = self.ast.templates.find_partial(self.ast.first(tmpl), canonical)?;
            self.ast.templates.partial(i)?.partial
        };
        if !self.templ.entering_context
            && !self.encloses(pat, self.ctx)
            && !self.in_declarator_scope_of(pat)
        {
            // `X<P...>` outside `X` (an alias template's or another
            // template's own parameters at the same positions) is an
            // ordinary dependent specialization.
            return None;
        }
        Some(self.ast.definition(pat).unwrap_or(pat))
    }

    /// The partial specialization `X<P...>` of `tmpl` with the primary's
    /// own arguments, more constrained than the primary
    /// ([temp.spec.partial.general]/9), that the out-of-line member
    /// declaration being entered names: its template head's
    /// requires-clause is the partial specialization's and not the
    /// primary's (`primary`, clang's `findPartialSpecialization` by
    /// arguments and template parameters). libstdc++ ranges:6380
    /// `template<view _Vp> requires forward_range<_Vp> template<bool>
    /// class chunk_view<_Vp>::_Iterator`.
    fn own_args_partial(
        &self,
        tmpl: DeclId,
        args: ArgListId,
        primary: &kiln_ast::templates::TemplateParamList,
    ) -> Option<DeclId> {
        if !self.templ.entering_context {
            return None;
        }
        let canonical = self.ast.types.canonical_args(args);
        let i = self.ast.templates.find_partial(self.ast.first(tmpl), canonical)?;
        let partial = self.ast.templates.partial(i)?.partial;
        let (_, ps) = self.ast.partial_spec_info(partial)?;
        let plist = *self.ast.templates.param_list(ps.params);
        let depth = self.template_param_depth(self.head_template(tmpl));
        let head = self.templ.heads.iter().rev().find(|h| !h.taken && h.depth == depth)?;
        let hlist = *self.ast.templates.param_list(head.list?);
        (self.opt_exprs_equivalent(hlist.requires, plist.requires)
            && !self.opt_exprs_equivalent(hlist.requires, primary.requires))
        .then_some(partial)
    }

    /// Whether the class scope of pattern `pat` was entered for the rest
    /// of a qualified declarator (`A<T>::f(typename A<T>::X)`).
    fn in_declarator_scope_of(&self, pat: DeclId) -> bool {
        let pat = self.ast.first(pat);
        self.scopes.iter().any(|s| {
            s.kind == ScopeKind::Record && s.entity.is_some_and(|e| self.ast.first(e) == pat)
        })
    }

    /// `template<int S> void B<1, S>::g()`: a dependent template-id keeps
    /// its written arguments, so a non-dependent non-type argument (`1`)
    /// does not match the partial specialization, which was registered
    /// with converted ones. When entering a declarator scope, convert
    /// them as the partial specialization's declaration did and look
    /// again (clang checks the arguments of every template-id).
    fn entered_partial_by_converted_args(
        &mut self,
        tmpl: DeclId,
        ty: QualType,
        loc: Loc,
    ) -> Option<DeclId> {
        if self.kind_of(tmpl) != DeclKind::ClassTemplate {
            return None;
        }
        // Outside a declarator qualifier, only within (or in the declarator
        // scope of) a class whose injected-class-name names `tmpl`:
        // `template<class T> void B<1, T>::f(typename B<1, T>::size_type)`
        // ([temp.dep.type]/1.2).
        let entering = self.templ.entering_context;
        if !entering && self.injected_class_for(tmpl).is_none() {
            return None;
        }
        let TypeView::TemplateSpecialization { args, .. } = self.ast.types.view(ty.ty()) else {
            return None;
        };
        let written = self.ast.types.args(args).to_vec();
        // A non-dependent expression argument (profiled by
        // `profile_value_args`).
        let unconverted = |a: &TemplateArg| match a.view() {
            ArgView::Expr { profile: None, .. } => true,
            ArgView::Expr { expr, profile: Some(_) } => self.ast.exprs.get(expr).deps().is_none(),
            _ => false,
        };
        // Or arguments left to the primary template's defaults: the
        // partial specialization's own arguments include them
        // ([temp.arg.general]/9; `template<class T, class S = typename
        // T::S> struct E;` then `template<class U> class E<W<U>>::I`).
        // Or arguments for a template parameter pack, which the partial
        // specialization's own arguments hold as one argument pack
        // (`template<class... A> int B<int, A...>::g()`).
        let params = self.template_params(tmpl);
        let first_pack = params
            .iter()
            .position(|&p| self.ast.template_parm(p).is_some_and(|x| x.pack))
            .unwrap_or(params.len());
        if !written.iter().any(unconverted)
            && written.len() >= params.len()
            && first_pack == params.len()
        {
            return None;
        }
        // A pack expansion for a parameter that is not a pack (`Ts<T...>`
        // in `template<template<class> class H, template<class> class...
        // T> struct Ts`, googletest's `Templates`) covers parameters only
        // once substituted: no partial specialization's own arguments.
        // Every argument from `first_pack` on is the pack's, if any.
        if let Some(i) = written.iter().position(|&a| self.is_pack_expansion_arg(a))
            && (i < first_pack || first_pack == params.len())
        {
            return None;
        }
        let conv = self.check_template_args(tmpl, &written, &[], loc).ok()?;
        let i = self.ast.templates.find_partial(self.ast.first(tmpl), conv.canonical)?;
        let pat = self.ast.templates.partial(i)?.partial;
        if !entering && !self.encloses(pat, self.ctx) && !self.in_declarator_scope_of(pat) {
            return None;
        }
        Some(self.ast.definition(pat).unwrap_or(pat))
    }

    /// [`Sema::current_instantiation`], also when `ty` writes a
    /// non-dependent non-type argument of a partial specialization
    /// unconverted (`vec<2, T>`, [temp.dep.type]/1.2) or leaves an
    /// argument to its default.
    pub(crate) fn current_instantiation_at(
        &mut self,
        tmpl: DeclId,
        ty: QualType,
        loc: Loc,
    ) -> Option<DeclId> {
        self.current_instantiation(tmpl, ty)
            .or_else(|| self.entered_partial_by_converted_args(tmpl, ty, loc))
    }

    /// `list` with each non-dependent expression argument given its
    /// canonical profile: a dependent specialization `X<T, 1>` is not
    /// converted ([temp.arg]), so its non-type arguments compare as
    /// expressions ([temp.over.link]/5) and two declarations writing the
    /// same `X<T, 1>` name the same type.
    fn profile_value_args(&mut self, list: ArgListId) -> ArgListId {
        let args = self.ast.types.args(list).to_vec();
        if !args.iter().any(|a| matches!(a.view(), ArgView::Expr { profile: None, .. })) {
            return list;
        }
        let out: Vec<TemplateArg> = args
            .iter()
            .map(|&a| match a.view() {
                ArgView::Expr { expr, profile: None } => {
                    TemplateArg::expr(expr, Some(self.ast.profile_expr(expr)))
                }
                _ => a,
            })
            .collect();
        self.ast.types.arg_list(&out)
    }

    /// Whether `d` is a template parameter of `p`'s kind at `p`'s depth
    /// and index (`info`).
    fn same_template_parm(&self, d: DeclId, p: DeclId, info: TemplateParmInfo) -> bool {
        self.kind_of(d) == self.kind_of(p)
            && self
                .ast
                .template_parm(d)
                .is_some_and(|i| i.depth == info.depth && i.index == info.index)
    }

    /// Whether template argument `a` names parameter `p` (at its depth
    /// and index; a pack expanded).
    pub(crate) fn arg_names_param(&self, p: DeclId, a: TemplateArg) -> bool {
        let Some(info) = self.ast.template_parm(p).copied() else { return false };
        match (self.kind_of(p), a.view()) {
            (DeclKind::TemplateTypeParm, ArgView::Type(t)) => {
                let t = match self.ast.types.view(t.ty()) {
                    TypeView::PackExpansion { pattern, .. } if info.pack => pattern,
                    _ if info.pack => return false,
                    _ => t,
                };
                let own = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                self.ast.types.canonical(t) == self.ast.types.canonical(own)
            }
            (DeclKind::NonTypeTemplateParm, ArgView::Expr { expr, .. }) => {
                let mut e = expr;
                if info.pack && self.ast.exprs.get(e).kind() == ExprKind::PackExpansionExpr {
                    e = self.ast.exprs.operand(e);
                }
                // The parameter at the same position: the out-of-line
                // head redeclares the class template's parameters.
                self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
                    && self.same_template_parm(self.ast.exprs.decl(e), p, info)
            }
            (
                DeclKind::TemplateTemplateParm,
                ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. },
            ) => {
                matches!(self.ast.types.template_name_view(n), TemplateName::TemplateTemplateParm { decl, .. } if self.same_template_parm(decl, p, info))
            }
            _ => false,
        }
    }

    // -- template-ids -------------------------------------------------------------

    /// The function template-id `e` (an `UnresolvedLookupExpr`), written
    /// unqualified and followed by `(`: the call also uses
    /// argument-dependent lookup ([basic.lookup.argdep]/1).
    pub fn act_on_template_id_call_name(&mut self, e: ExprId) {
        if self.ast.exprs.get(e).kind() == ExprKind::UnresolvedLookupExpr {
            let _ = self.ast.exprs.set_adl(e);
        }
    }

    /// The function template-id `e` (an `UnresolvedLookupExpr` built from
    /// the first template `name` found) widened to the whole overload set
    /// the lookup of `name` (in `qualifier`, if any) finds
    /// ([temp.arg.explicit]/4, [over.match.call.general]/1): every
    /// function template of the set is a candidate.
    pub fn widen_function_template_id(
        &mut self,
        e: ExprId,
        name: Symbol,
        qualifier: Option<DeclId>,
    ) -> ExprId {
        if self.ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            return e;
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary),
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        let decls: Vec<DeclId> = found
            .decls
            .iter()
            .copied()
            .filter(|&d| {
                // A member function is a candidate too: libstdc++
                // `bitset::to_string(char, char)` calls the overloaded
                // member template `to_string<char, ...>(z, o)`.
                matches!(
                    self.kind_of(self.ast.strip_shadow(d)),
                    DeclKind::FunctionTemplate | DeclKind::Function | DeclKind::CXXMethod
                )
            })
            .collect();
        if decls.len() < 2 || decls.len() != found.decls.len() {
            return e;
        }
        let n = *self.ast.exprs.get(e);
        let nns = self.ast.templates.expr_nns.get(e).copied();
        let targs = self.ast.templates.expr_targs.get(e).copied();
        let dep = self.overload_set_is_dependent(&decls);
        self.builder().unresolved_lookup_in(
            DeclName::ident(name),
            &decls,
            false,
            nns,
            targs,
            dep,
            n.loc(),
        )
    }

    /// True if the unqualified `name`, followed by `<`, is assumed to name
    /// a function template ([temp.names]/3, P0846): ordinary lookup finds
    /// nothing (and it is no builtin) or only non-member functions.
    pub fn is_assumed_template_name(&self, name: Symbol) -> bool {
        let found = self.lookup_name(name, LookupKind::Ordinary);
        if found.decls.is_empty() {
            return !self.is_builtin_name(name);
        }
        found.decls.iter().all(|&d| self.is_assumable_function(self.ast.strip_shadow(d)))
    }

    /// The declaration an unqualified `name` followed by `<` refers to
    /// when it might have been intended as a template-name (clang's
    /// `mightBeIntendedToBeTemplateName`): ordinary lookup found exactly
    /// one declaration (a `DeclRefExpr` or implicit member access, not
    /// an overload set) and it is no template.
    pub fn potential_template_name(&self, name: Symbol) -> Option<DeclId> {
        self.potential_template_name_in(None, DeclName::ident(name))
    }

    /// [`Sema::potential_template_name`] for `name` looked up in the
    /// context `qual` when qualified (`::f<int>`, `N::f<int>`).
    pub fn potential_template_name_in(
        &self,
        qual: Option<DeclId>,
        name: DeclName,
    ) -> Option<DeclId> {
        let found = match qual {
            Some(q) => self.lookup_qualified_dn(q, name, LookupKind::Ordinary),
            None => self.lookup_name_dn(name, LookupKind::Ordinary),
        };
        let [d] = found.decls[..] else { return None };
        let d = self.ast.strip_shadow(d);
        if self.kind_of(d) == DeclKind::CXXMethod
            && !self.is_static_method(d)
            && !self.implicit_this_available()
        {
            // `S::f` outside a member function is already an error ("call
            // to non-static member function without an object argument"),
            // so clang's `checkPotentialAngleBracket` does not run.
            return None;
        }
        (!matches!(
            self.kind_of(d),
            DeclKind::FunctionTemplate | DeclKind::VarTemplate | DeclKind::ClassTemplate
        ))
        .then_some(d)
    }

    /// clang's `diagnoseExprIntendedAsTemplateName` for the name `name`
    /// at `loc`, found as the non-template `found`, followed by a
    /// template argument list ([temp.names]/3): "'f' does not name a
    /// template but is followed by template arguments" with the range
    /// of the list from its `<` at `less` to its `>` at `greater`, and a
    /// note at the declaration. `name` may be an operator-function-id
    /// (`::operator+<int>(a, b)`).
    pub fn diagnose_expr_intended_as_template_name(
        &mut self,
        name: DeclName,
        found: DeclId,
        loc: Loc,
        (less, greater): (Loc, Loc),
    ) {
        let text = self.decl_name_str(name);
        let at = self.ast.decls.get(found).loc();
        let msg = self
            .diag(DiagId::sema_non_template_in_template_id, loc)
            .arg(text)
            .range(kiln_source::Span::new(less, greater.with_offset(1)))
            .with_note(self.diag(DiagId::note_non_template_in_template_id_found, at));
        self.emit(msg);
    }

    /// A function lookup may find for a name followed by `<` to be
    /// assumed a template ([temp.names]/3): a non-member function, or
    /// from C++20 a member function too (clang 22; the template-id is then
    /// diagnosed as not referring to a template).
    fn is_assumable_function(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::Function => true,
            DeclKind::CXXMethod => self.std >= kiln_lex::Std::Cxx20,
            _ => false,
        }
    }

    /// The template-id `name<args>` for a name assumed to be a template
    /// ([temp.names]/3): an `UnresolvedLookupExpr` with the explicit
    /// arguments over the functions ordinary lookup found, to be
    /// completed by argument-dependent lookup at the call
    /// ([basic.lookup.argdep]; at instantiation when dependent).
    pub fn act_on_assumed_template_id(
        &mut self,
        name: Symbol,
        args: &[TemplateArg],
        loc: Loc,
    ) -> ExprId {
        let found = self.lookup_name(name, LookupKind::Ordinary);
        let decls: Vec<DeclId> = found.decls.iter().map(|&d| self.ast.strip_shadow(d)).collect();
        if decls.iter().any(|&d| self.kind_of(d) == DeclKind::CXXMethod) {
            // A class member was found: no argument-dependent lookup
            // ([basic.lookup.argdep]/3), so no template can be found.
            self.diagnose_args_for_non_template(DeclName::ident(name), &decls, loc);
            return self.act_on_error_expr(&[], loc);
        }
        let list = self.ast.types.arg_list(args);
        self.builder().unresolved_lookup(DeclName::ident(name), &decls, true, None, Some(list), loc)
    }

    /// The template-id `name<args>` assumed to name a template
    /// ([temp.names]/3) whose lookup found no function template
    /// (`ordinary`, the ordinary-lookup result; argument-dependent lookup
    /// found none either), diagnosed at `loc` as clang 22 does: lexically
    /// inside a class (a member function body, also through a lambda, a
    /// default member initializer) with a non-empty `ordinary` "'f' does
    /// not refer to a template" with a note at its first declaration
    /// (clang 19: "member 'f' used before its declaration" or "...from
    /// dependent base class", [`Sema::diagnose_member_found_in_class`]);
    /// otherwise as an undeclared name.
    pub(crate) fn diagnose_args_for_non_template(
        &mut self,
        name: DeclName,
        ordinary: &[DeclId],
        loc: Loc,
    ) {
        let text = self.decl_name_str(name);
        if self.clang_compat < 22
            && !ordinary.is_empty()
            && self.in_class_context()
            && let Some(decls) = self.diagnose_member_found_in_class(name, loc, false)
        {
            // clang 19 diagnoses the member as found late in the class;
            // the call then needs an object.
            let member = decls.iter().any(|&d| {
                let d = self.ast.strip_shadow(d);
                self.kind_of(d) == DeclKind::CXXMethod && !self.is_static_method(d)
            });
            if member && !self.implicit_this_available() {
                let msg = self.diag(DiagId::sema_member_call_without_object, loc);
                self.emit(msg);
            }
            return;
        }
        match ordinary.first() {
            Some(&d) if self.in_class_context() => {
                let at = self.ast.decls.get(d).loc();
                let msg = self
                    .diag(DiagId::sema_does_not_refer_to_template, loc)
                    .arg(text)
                    .with_note(self.diag(DiagId::note_declared_non_template, at));
                self.emit(msg);
            }
            _ => self.diagnose_undeclared_template_name(name, loc),
        }
    }

    /// "use of undeclared 'operator+'" for the template-name `name` at
    /// `loc`, with (clang 22) the range of the token there.
    fn diagnose_undeclared_template_name(&mut self, name: DeclName, loc: Loc) {
        let mut msg = self.diag(Self::undeclared_use_id(name), loc).arg(self.decl_name_str(name));
        if self.clang_compat >= 22 {
            msg = msg.range(kiln_source::Span::new(loc, self.token_end(loc).unwrap_or(loc)));
        }
        self.emit(msg);
    }

    /// Whether the current context is lexically inside a class: the
    /// innermost enclosing context that is neither a function nor a
    /// lambda's closure type is a class (clang's `getCurLambda`-skipping
    /// `CurContext` walk for [temp.names]/3 recovery).
    fn in_class_context(&self) -> bool {
        let mut cur = Some(self.ctx);
        for _ in 0..1024 {
            let Some(d) = cur else { return false };
            let decl = self.ast.decls.get(d);
            if decl.kind().is_function() {
                cur = decl.ctx();
                continue;
            }
            match self.ast.decls.record(d) {
                Some(r) if r.lambda.is_some() => cur = decl.ctx(),
                Some(_) => return true,
                None => return false,
            }
        }
        false
    }

    /// Whether the operator-function-id `name`, optionally qualified by
    /// the non-dependent `qualifier`, followed by `<` starts a
    /// template-id ([temp.names]/3): lookup finds a function template;
    /// unqualified, also when it finds nothing or only functions (P0846).
    pub fn operator_names_template(&mut self, name: DeclName, qualifier: Option<DeclId>) -> bool {
        match qualifier {
            Some(q) => self.declare_implicit_named(q, name),
            None => self.declare_implicit_named_in_scope(name),
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified_dn(q, name, LookupKind::Ordinary),
            None => self.lookup_name_dn(name, LookupKind::Ordinary),
        };
        let kinds: Vec<DeclKind> =
            found.decls.iter().map(|&d| self.kind_of(self.ast.strip_shadow(d))).collect();
        kinds.contains(&DeclKind::FunctionTemplate)
            || (qualifier.is_none()
                && found
                    .decls
                    .iter()
                    .all(|&d| self.is_assumable_function(self.ast.strip_shadow(d))))
    }

    /// `operator@ < args >`, optionally qualified by the non-dependent
    /// `qualifier` ([temp.names]/1: an operator-function-id is a
    /// template-name when it names a function template; `operator()<T>()`
    /// in a member function, `Q::operator()<T>()`): an
    /// `UnresolvedLookupExpr` over the functions lookup finds, completed
    /// by the call as for `f<T>`. Unqualified and `call`ed, a name lookup
    /// finds nothing or only non-member functions for is assumed to be a
    /// template, completed by argument-dependent lookup ([temp.names]/3,
    /// P0846: `operator+<7>(a, b)`). Diagnoses a name that finds no
    /// function template otherwise.
    pub fn act_on_operator_template_id(
        &mut self,
        name: DeclName,
        args: &[TemplateArg],
        loc: Loc,
        qualifier: Option<DeclId>,
        call: bool,
    ) -> ExprId {
        match qualifier {
            Some(q) => self.declare_implicit_named(q, name),
            None => self.declare_implicit_named_in_scope(name),
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified_dn(q, name, LookupKind::Ordinary),
            None => self.lookup_name_dn(name, LookupKind::Ordinary),
        };
        let decls: Vec<DeclId> = found.decls.iter().map(|&d| self.ast.strip_shadow(d)).collect();
        let functions = decls.iter().all(|&d| {
            matches!(
                self.kind_of(d),
                DeclKind::FunctionTemplate | DeclKind::Function | DeclKind::CXXMethod
            )
        });
        let list = self.ast.types.arg_list(args);
        if qualifier.is_none() && call && decls.iter().all(|&d| self.is_assumable_function(d)) {
            if decls.iter().any(|&d| self.kind_of(d) == DeclKind::CXXMethod) {
                // `operator()<int>(1)` naming only non-template members:
                // no argument-dependent lookup ([basic.lookup.argdep]/3).
                self.diagnose_args_for_non_template(name, &decls, loc);
                return self.act_on_error_expr(&[], loc);
            }
            return self.builder().unresolved_lookup(name, &decls, true, None, Some(list), loc);
        }
        if decls.is_empty() && qualifier.is_none() {
            self.diagnose_undeclared_template_name(name, loc);
            return self.act_on_error_expr(&[], loc);
        }
        if !functions || !decls.iter().any(|&d| self.kind_of(d) == DeclKind::FunctionTemplate) {
            let msg = self.diag(DiagId::sema_no_template_named, loc).arg(self.decl_name_str(name));
            self.emit(msg);
            return self.act_on_error_expr(&[], loc);
        }
        let dep = self.overload_set_is_dependent(&decls);
        self.builder().unresolved_lookup_in(name, &decls, false, None, Some(list), dep, loc)
    }

    /// The qualified function or variable template-id `e` named at `loc`
    /// begins at its qualifier `begin` (clang's `OverloadExpr::getBeginLoc`,
    /// `DeclRefExpr::getBeginLoc`).
    pub fn set_template_id_begin(&mut self, e: ExprId, begin: Loc, loc: Loc) {
        if begin != loc
            && matches!(
                self.ast.exprs.get(e).kind(),
                ExprKind::UnresolvedLookupExpr | ExprKind::DeclRefExpr
            )
        {
            let _ = self.ast.templates.expr_begin.insert(e, begin);
        }
    }

    /// Records the written qualifier of the qualified function
    /// template-id or concept-id `e` (an `UnresolvedLookupExpr` or
    /// `ConceptSpecializationExpr`); dependent signatures and
    /// requires-clauses mangle it (`clsr3stdE7declvalIT_EE`,
    /// `Qsr3stdE7same_asIT_iE`, spec/abi.md §8.15–§8.16), or of the
    /// `DeclRefExpr` naming a variable template specialization, which
    /// prints it (`decltype(N::v<int>)`).
    pub fn set_template_id_qualifier(&mut self, e: ExprId, nns: NnsId) {
        if matches!(
            self.ast.exprs.get(e).kind(),
            ExprKind::UnresolvedLookupExpr | ExprKind::ConceptSpecializationExpr
        ) {
            let _ = self.ast.templates.expr_nns.insert(e, nns);
        } else if self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr
            && self.ast.exprs.decl_ref_extra(e).is_some()
        {
            // A variable template specialization (`N::v<int>`) keeps its
            // qualifier as written (clang's `DeclRefExpr` qualifier).
            let _ = self.ast.exprs.set_decl_ref_qualifier(e, nns);
        }
    }

    /// Records the `template` keyword at `kw` (if written) after the
    /// qualifier of the name `e` (`N::template v<T>`), which the AST
    /// printer writes back as clang's `hasTemplateKeyword` (WP-308).
    pub fn set_template_kw(&mut self, e: ExprId, kw: Option<Loc>) {
        if let Some(kw) = kw
            && matches!(
                self.ast.exprs.get(e).kind(),
                ExprKind::DeclRefExpr
                    | ExprKind::UnresolvedLookupExpr
                    | ExprKind::DependentScopeDeclRefExpr
            )
        {
            let _ = self.ast.templates.template_kw.insert(e, kw);
        }
    }

    /// The template name of template (or template template parameter)
    /// `d` as written.
    pub fn template_name_for(&mut self, d: DeclId) -> TemplateNameId {
        if let Some(i) = self.ast.template_parm(d).copied()
            && self.kind_of(d) == DeclKind::TemplateTemplateParm
        {
            return self.ast.types.template_name(TemplateName::TemplateTemplateParm {
                decl: d,
                depth: u32::from(i.depth),
                index: u32::from(i.index),
                pack: i.pack,
            });
        }
        self.ast.types.template_name_of(self.ast.first(d))
    }

    /// The arity of `TT<args>` for a template template parameter `TT`
    /// (clang's `CheckTemplateArgumentList` on the parameter's own list:
    /// "too few/many template arguments for template template parameter
    /// 'TT'"). Arguments that are pack expansions are matched later.
    fn check_ttp_arity(&mut self, ttp: DeclId, args: &[TemplateArg], l: Loc) -> Result<(), ()> {
        if args.iter().any(|&a| self.is_pack_expansion_arg(a)) {
            return Ok(());
        }
        let Some(list) = self.ast.template_parm(ttp).and_then(|x| x.params) else {
            return Ok(());
        };
        let l2 = *self.ast.templates.param_list(list);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(l2.params).collect();
        let is_pack = |s: &Self, p: DeclId| s.ast.template_parm(p).is_some_and(|x| x.pack);
        if params.iter().any(|&p| is_pack(self, p)) {
            return Ok(());
        }
        let too_many = args.len() > params.len();
        let too_few = params
            .get(args.len()..)
            .is_some_and(|rest| rest.iter().any(|&p| self.template_default(p).is_none()));
        if too_many || too_few {
            self.diag_template_arity(ttp, too_many, l);
            return Err(());
        }
        Ok(())
    }

    /// [temp.arg.general]/9 with [temp.over.link]/5: the canonical form
    /// of the dependent specialization `name<args>` of class template
    /// `tmpl` carries the default arguments of the parameters not
    /// written, so `enable_if<B>` and `enable_if<B, void>` (an alias
    /// template's expansion) are the same type; the written arguments
    /// stay as sugar (clang's converted arguments). `None` when nothing
    /// is defaulted.
    fn dependent_spec_canon(
        &mut self,
        name: TemplateNameId,
        tmpl: DeclId,
        args: &[TemplateArg],
        l: Loc,
    ) -> Option<QualType> {
        let full = self.dependent_spec_full_args(tmpl, args, l)?;
        let full = self.ast.types.arg_list(&full);
        let full = self.profile_value_args(full);
        Some(self.ast.types.template_specialization(name, full, None, false))
    }

    /// The arguments of the dependent specialization `tmpl<args>` of class
    /// template `tmpl` completed with the default arguments of the
    /// parameters not written ([temp.arg.general]/9), each substituted
    /// with the arguments before it; `None` when nothing is missing, when
    /// a pack is involved, or when a default cannot be substituted yet
    /// (the specialization then stands as written).
    fn dependent_spec_full_args(
        &mut self,
        tmpl: DeclId,
        args: &[TemplateArg],
        l: Loc,
    ) -> Option<Vec<TemplateArg>> {
        let head = self.head_template(tmpl);
        let params = self.template_params(head);
        if args.len() >= params.len()
            || params.iter().any(|&p| self.ast.template_parm(p).is_some_and(|x| x.pack))
            || args.iter().any(|&a| self.is_pack_expansion_arg(a))
        {
            return None;
        }
        let depth = self.template_param_depth(head);
        let first = self.ast.first(tmpl);
        let mut out = args.to_vec();
        for &p in &params[args.len()..] {
            let def = self.template_default(p)?;
            let dependent = self
                .ast
                .types
                .arg_flags(def)
                .intersects(TypeFlags::DEPENDENT_INSTANTIATION | TypeFlags::UNEXPANDED_PACK);
            if !dependent {
                out.push(def);
                continue;
            }
            let levels = self.prior_levels(first, depth, &out);
            let so_far = self.ast.types.arg_list(&out);
            let (r, failed) = self.in_sfinae(
                crate::templ::stack::FrameKind::DefaultTemplateArg,
                first,
                Some(so_far),
                l,
                |s| crate::templ::subst::Substituter::new(s, &levels).template_arg(def, l),
            )?;
            match (r, failed) {
                (Ok(a), Ok(())) => out.push(a),
                _ => return None,
            }
        }
        Some(out)
    }

    /// `name < args >` ([temp.names]): what the template-id names, by the
    /// kind of the template. Dependent specializations are built as
    /// written; non-dependent class and alias template specializations
    /// are formed (§4.2, §4.10); variable templates need WP-071.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn act_template_id(
        &mut self,
        name: TemplateNameId,
        args: &[TemplateArg],
        l: Loc,
        r: Loc,
    ) -> Result<TemplateIdRef, ()> {
        let _ = r;
        let res = self.template_id(name, args, l);
        // A dependent or instantiation-dependent specialization: where written (§5.8).
        if let Ok(TemplateIdRef::Type(t)) = res
            && self.is_instantiation_dependent_type(t)
        {
            self.note_written_loc(WrittenKey::Type(t.without_quals()), l);
        }
        res
    }

    /// Whether `name<list>` is a dependent template-id. A member template
    /// of a class template's pattern is dependent too ([temp.dep.type]/9).
    /// An argument that is only instantiation-dependent (an alias of a
    /// dependent alias template specialization that resolves to a concrete
    /// type) does not make the specialization dependent ([temp.dep.type]/8
    /// speaks of dependent arguments; clang's
    /// `anyDependentTemplateArguments` tests the canonical arguments).
    fn template_id_dependent(&self, name: TemplateNameId, list: ArgListId) -> bool {
        let canon_list = self.ast.types.canonical_args(list);
        (self.ast.types.arg_list_flags(canon_list) | self.ast.types.template_name_flags(name))
            .intersects(TypeFlags::DEPENDENT | TypeFlags::UNEXPANDED_PACK)
    }

    #[allow(clippy::too_many_lines, reason = "one arm per kind of template named")]
    fn template_id(
        &mut self,
        name: TemplateNameId,
        args: &[TemplateArg],
        l: Loc,
    ) -> Result<TemplateIdRef, ()> {
        let converted = self.injected_template_args(name, args);
        let args = converted.as_deref().unwrap_or(args);
        let list = self.ast.types.arg_list(args);
        let dep_args = self.template_id_dependent(name, list);
        let Some(d) = self.ast.types.template_decl(name) else {
            if let TemplateName::TemplateTemplateParm { decl, .. } =
                self.ast.types.template_name_view(name)
            {
                self.check_ttp_arity(decl, args, l)?;
                // `C<args>` / `V<args>` for a concept or variable template
                // template parameter (P2841R7): value-dependent until
                // substituted (spec/sema-templates.md §2.1.1).
                let k = self.ttp_kind(decl);
                if k != kiln_ast::templates::TtpKind::Type {
                    let concept = k == kiln_ast::templates::TtpKind::Concept;
                    let e =
                        self.builder().ttp_template_id_expr(decl, concept, list, QualType::BOOL, l);
                    return Ok(TemplateIdRef::Value(e));
                }
            }
            return Ok(TemplateIdRef::Type(match self.ast.types.template_name_view(name) {
                // `TT<args>` for a template template parameter `TT`, and
                // `[: r :]<args>` with a dependent `r` in a type position
                // (P2996 [temp.names]/1).
                TemplateName::TemplateTemplateParm { .. }
                | TemplateName::CanonTemplateTemplateParm { .. }
                | TemplateName::Splice { .. }
                | TemplateName::SpliceProfile(_) => {
                    self.ast.types.template_specialization(name, list, None, false)
                }
                // `T::template X<args>` ([temp.names]/5).
                TemplateName::DependentIdentifier { nns, name: id } => self
                    .ast
                    .types
                    .dependent_template_specialization(TagKeyword::None, nns, id, list),
                _ => QualType::DEPENDENT,
            }));
        };
        match self.kind_of(d) {
            // spec/sema-templates.md §9.3.
            DeclKind::BuiltinTemplate => {
                let locs = std::mem::take(&mut self.templ.arg_locs);
                self.builtin_template_id(name, d, args, &locs, l)
            }
            DeclKind::ClassTemplate | DeclKind::TemplateTemplateParm => {
                let ttp = self.kind_of(d) == DeclKind::TemplateTemplateParm;
                if ttp {
                    self.check_ttp_arity(d, args, l)?;
                } else if dep_args {
                    self.check_dependent_template_args(d, args, l)?;
                } else {
                    let locs = std::mem::take(&mut self.templ.arg_locs);
                    let t = self.class_template_type(name, d, args, &locs, l)?;
                    return Ok(TemplateIdRef::Type(t));
                }
                let list = if ttp { list } else { self.profile_value_args(list) };
                let mut canon =
                    if ttp { None } else { self.dependent_spec_canon(name, d, args, l) };
                if !ttp && canon.is_none() {
                    let t = self.ast.types.template_specialization(name, list, None, false);
                    canon = self.partial_current_instantiation_canon(d, t, l);
                }
                Ok(TemplateIdRef::Type(
                    self.ast.types.template_specialization(name, list, canon, false),
                ))
            }
            DeclKind::TypeAliasTemplate => {
                // [temp.alias]/2: the alias is substituted even with
                // dependent arguments; an argument that is a pack expansion
                // cannot be matched to parameters yet (WP-071), so that
                // specialization stands for itself.
                // The exception is an expansion that only reaches the
                // alias's trailing parameter pack (`index_sequence<Is...>`
                // for `template<size_t... I> using index_sequence =
                // integer_sequence<size_t, I...>`): the pack is bound to
                // the expansion and substitution re-forms it (clang's
                // `CheckTemplateArgumentList` + `SubstTemplateTypeParmPack`).
                if self.alias_args_substitutable(d, args) {
                    let locs = std::mem::take(&mut self.templ.arg_locs);
                    let t = self.alias_template_type(name, d, args, &locs, l)?;
                    return Ok(TemplateIdRef::Type(t));
                }
                // The arguments before the expansion are checked, and an
                // expansion for a parameter that is not a pack is an error
                // (§3.2).
                self.check_dependent_template_args(d, args, l)?;
                Ok(TemplateIdRef::Type(
                    self.ast.types.template_specialization(name, list, None, true),
                ))
            }
            DeclKind::VarTemplate => {
                if !dep_args && !args.iter().any(|&a| self.is_pack_expansion_arg(a)) {
                    // [temp.names]/1, [temp.inst]/7: names the
                    // specialization (§4.7).
                    let locs = std::mem::take(&mut self.templ.arg_locs);
                    let v = self.get_var_spec(d, args, &locs, l)?;
                    let extra = kiln_ast::expr::DeclRefExtra {
                        qualifier: None,
                        template_args: Some(list),
                        found_decl: Some(self.ast.first(d)),
                    };
                    return Ok(TemplateIdRef::Value(self.build_decl_ref_extra(v, l, Some(extra))));
                }
                // Checked against the parameters even so (clang
                // `CheckTemplateArgumentList`, §3.2).
                self.check_dependent_template_args(d, args, l)?;
                self.check_var_template_own_init(d, list, l)?;
                let n = self.ast.decls.get(d).name();
                let dep_name = self
                    .ast
                    .types
                    .template_name_flags(name)
                    .intersects(TypeFlags::DEPENDENT_INSTANTIATION);
                let e = self.builder().unresolved_lookup_in(
                    n,
                    &[d],
                    false,
                    None,
                    Some(list),
                    dep_name,
                    l,
                );
                Ok(TemplateIdRef::Value(e))
            }
            DeclKind::FunctionTemplate => {
                let n = self.ast.decls.get(d).name();
                let dep = self.overload_set_is_dependent(&[d]);
                let e =
                    self.builder().unresolved_lookup_in(n, &[d], false, None, Some(list), dep, l);
                Ok(TemplateIdRef::Overload(e))
            }
            DeclKind::Concept => {
                let locs = std::mem::take(&mut self.templ.arg_locs);
                let e = self.build_concept_id(d, list, &locs, l)?;
                Ok(TemplateIdRef::Value(e))
            }
            _ => {
                let msg = self
                    .diag(DiagId::sema_not_a_template, l)
                    .arg(format!("'{}'", self.decl_name(d)));
                self.emit(msg);
                Err(())
            }
        }
    }

    /// Whether alias template `d` can be substituted with `args` now:
    /// no argument is a pack expansion, or every pack expansion falls on
    /// the alias's trailing parameter pack (WP-156).
    fn alias_args_substitutable(&self, d: DeclId, args: &[TemplateArg]) -> bool {
        let Some(first) = args.iter().position(|&a| self.is_pack_expansion_arg(a)) else {
            return true;
        };
        let params = self.template_params(self.ast.first(d));
        params.last().is_some_and(|&p| self.is_pack_decl(p)) && first + 1 >= params.len()
    }

    /// Whether `a` is a pack expansion (`T...`, `TT...`, `e...`).
    pub(crate) fn is_pack_expansion_arg(&self, a: TemplateArg) -> bool {
        match a.view() {
            ArgView::Type(q) => self.ast.types.kind(q.ty()) == TypeKind::PackExpansion,
            ArgView::TemplateExpansion { .. } => true,
            ArgView::Expr { expr, .. } => {
                self.ast.exprs.get(expr).kind() == ExprKind::PackExpansionExpr
            }
            _ => false,
        }
    }

    /// `[typename] nns::name` / `nns::[template] name<args>` with a
    /// dependent `nns` ([temp.res.general]/4): `DependentNameType` or
    /// `DependentTemplateSpecializationType`.
    pub fn act_dependent_name(
        &mut self,
        nns: NnsId,
        name: Symbol,
        targs: Option<ArgListId>,
        kw: TagKeyword,
    ) -> QualType {
        match targs {
            Some(a) => self.ast.types.dependent_template_specialization(kw, nns, name, a),
            None => self.ast.types.dependent_name(kw, nns, name),
        }
    }

    /// [`Sema::act_dependent_name`] for a name written at `loc`, which
    /// diagnostics found after substitution point at (§5.8).
    pub fn act_dependent_name_at(
        &mut self,
        nns: NnsId,
        name: Symbol,
        targs: Option<ArgListId>,
        kw: TagKeyword,
        loc: Loc,
    ) -> QualType {
        let t = self.act_dependent_name(nns, name, targs, kw);
        self.note_written_loc(WrittenKey::Type(t.without_quals()), loc);
        t
    }

    /// `nns::name` (with optional explicit arguments) in an expression,
    /// with a dependent `nns` ([temp.dep.expr]/3); `begin` is the
    /// qualifier's first token.
    pub fn act_dependent_scope_ref(
        &mut self,
        nns: NnsId,
        name: DeclName,
        targs: Option<ArgListId>,
        begin: Loc,
        loc: Loc,
    ) -> ExprId {
        self.builder().dependent_scope_decl_ref(nns, name, targs, begin, loc)
    }

    // -- concepts and requires ------------------------------------------------------

    /// After `concept name =`: the concept consumes its head before its
    /// constraint-expression, so a class defined there
    /// (`requires (struct S {} x)`) takes none ([temp.pre]/1).
    pub fn act_on_concept_head(&mut self) {
        let _ = self.take_template_head(None);
        self.templ.cur = None;
    }

    /// `template<params> concept name = e;` ([temp.concept]/1); the head
    /// was consumed by [`Sema::take_template_head`].
    pub fn act_concept(
        &mut self,
        head: TemplateParamListId,
        name: Symbol,
        loc: Loc,
        e: ExprId,
    ) -> DeclId {
        let _ = self.take_template_head(None);
        self.templ.cur = None;
        if self.in_local_scope() || self.kind_of(self.ctx) == DeclKind::CXXRecord {
            self.error(DiagId::sema_template_outside_namespace_or_class, loc);
        }
        // [temp.param]/14: a template parameter pack is the last
        // parameter of a concept's head.
        self.check_template_param_list(DeclKind::Concept, head, None);
        // A non-`bool` atom makes the definition invalid (clang's
        // `ParseConstraintExpression`).
        // [temp.variadic]/12: the constraint-expression names no pack
        // outside an expansion (clang's `ActOnConceptDefinition`).
        let e = if self.diagnose_unexpanded_written(e, "expression", None) {
            let at = self.begin_loc(e);
            self.act_on_error_expr(&[e], at)
        } else if self.check_constraint_expression(e) {
            e
        } else {
            let at = self.ast.exprs.get(e).loc();
            self.act_on_error_expr(&[e], at)
        };
        // The same concept defined in another unit's global module
        // fragment (clang `CheckConceptRedefinition`): the earlier one is
        // made visible and the new one kept out of lookup
        // (spec/modules.md §6).
        let merged = self
            .prev_candidates(name, None)
            .into_iter()
            .rev()
            .find(|&p| self.kind_of(p) == DeclKind::Concept)
            .filter(|&p| self.module_merge_hidden_definition(p));
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
        let c = self.builder().concept(h, head, e);
        if merged.is_some() {
            self.builder().unregister_name(c);
        }
        c
    }

    /// `requires (params) {` ([expr.prim.req]/1): opens the body whose
    /// parameters the requirements see. Returns the `RequiresExprBody`.
    pub fn act_on_requires_expr_start(&mut self, loc: Loc) -> DeclId {
        let ctx = self.ctx;
        let body = self.builder().requires_body(ctx, loc);
        self.scopes.push(Scope::new(ScopeKind::RequiresExpr, Some(body)));
        self.templ.req_ctx.push(self.ctx);
        self.ctx = body;
        // The requirements are unevaluated operands ([expr.prim.req.general]/2).
        self.enter_unevaluated();
        body
    }

    /// The `(` of a requires-expression's parameter-declaration-clause:
    /// until [`Sema::act_on_end_requires_params`], a function type in a
    /// parameter is one prototype scope deeper (`ParamData::scope_depth`).
    pub fn act_on_start_requires_params(&mut self) {
        self.requires_param_scopes += 1;
    }

    /// The `)` closing what [`Sema::act_on_start_requires_params`] opened.
    pub fn act_on_end_requires_params(&mut self) {
        self.requires_param_scopes = self.requires_param_scopes.saturating_sub(1);
    }

    /// `}` of a requires-expression: closes the scope opened by
    /// [`Sema::act_on_requires_expr_start`].
    pub fn act_requires_expr(
        &mut self,
        body: DeclId,
        params: &[DeclId],
        reqs: &[Requirement],
        loc: Loc,
        rbrace: Loc,
    ) -> ExprId {
        if let Some(i) = self.scopes.iter().rposition(|s| s.kind == ScopeKind::RequiresExpr) {
            self.scopes.truncate(i);
        }
        if let Some(c) = self.templ.req_ctx.pop() {
            self.ctx = c;
            self.exit_unevaluated();
        }
        let e = self.builder().requires_expr(body, params, reqs, QualType::BOOL, loc, rbrace);
        if !self.ast.exprs.get(e).deps().is_none() {
            return e;
        }
        // [expr.prim.req.general]/5: a non-dependent requires-expression
        // is evaluated at once (§8.6).
        let levels = crate::templ::args::MultiLevelArgs::new();
        let r = crate::templ::subst::Substituter::new(self, &levels).requires_expr(e, loc);
        // The written expression keeps its requirements; it takes the
        // evaluated one's value and notes.
        if let Ok(x) = r {
            let ok = self.ast.exprs.satisfied(x).unwrap_or(true);
            let _ = self.ast.exprs.set_satisfied(e, ok);
            if let Some(&s) = self.inst.concepts.expr_sat.get(&x) {
                self.inst.concepts.expr_sat.insert(e, s);
            }
        }
        e
    }

    /// `{ e } -> C<args>` ([expr.prim.req.compound]/1): the immediately
    /// declared constraint `C<decltype((e)), args...>` written with an
    /// invented parameter, as clang builds it.
    pub fn act_on_return_type_requirement(
        &mut self,
        concept: DeclId,
        args: &[TemplateArg],
        loc: Loc,
    ) -> ExprId {
        let depth = self.template_depth();
        // clang names the invented parameter `expr-type` (not implicit).
        let name = self.intern("expr-type");
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
        let p = self.builder().template_type_parm(h, depth, 0, false, false);
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        self.immediately_declared_constraint(concept, ty, args, loc)
    }

    // -- packs and folds ----------------------------------------------------------------

    /// `pattern...` ([temp.variadic]/5): the pattern must name a pack.
    pub fn act_pack_expansion(&mut self, pattern: ExprId, ellipsis: Loc) -> ExprId {
        if self.is_err_expr(pattern) {
            return pattern;
        }
        if !self.ast.exprs.get(pattern).deps().contains(ExprDeps::UNEXPANDED_PACK) {
            self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
            return self.recover_err(&[pattern], ellipsis);
        }
        let keep = self.clang_compat >= 22;
        self.builder().pack_expansion_expr(pattern, ellipsis, keep)
    }

    /// `{a, b}...`: a braced-init-list as the pattern of an
    /// initializer-list pack expansion ([temp.variadic]/5.4, whose
    /// pattern is an *initializer-clause*). The list is kept unanalyzed
    /// (an `InitListExpr` of type `void`, as [`Self::dependent_init_list`])
    /// under a `PackExpansionExpr`; instantiation expands it into one
    /// braced-init-list per element (spec/sema-templates.md §5.3).
    pub fn act_pack_expansion_list(&mut self, l: &crate::init::InitList, ellipsis: Loc) -> ExprId {
        let e = self.dependent_init_list(l);
        self.act_pack_expansion(e, ellipsis)
    }

    /// `T...` as a type ([temp.variadic]/5).
    pub fn act_pack_expansion_type(&mut self, pattern: QualType, ellipsis: Loc) -> QualType {
        if !self.ast.types.flags(pattern.ty()).contains(TypeFlags::UNEXPANDED_PACK) {
            self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
            return pattern;
        }
        self.ast.types.pack_expansion(pattern, None)
    }

    /// `pack...[index]` as an expression (C++26 [expr.prim.pack.index]).
    pub fn act_pack_indexing(
        &mut self,
        pattern: ExprId,
        ellipsis: Loc,
        index: ExprId,
        rsquare: Loc,
    ) -> ExprId {
        if self.is_err_expr(pattern) || self.is_err_expr(index) {
            return self.recover_err(&[pattern, index], ellipsis);
        }
        if !self.ast.exprs.get(pattern).deps().contains(ExprDeps::UNEXPANDED_PACK) {
            self.error(DiagId::parse_expected_name_of_pack, self.begin_loc(pattern));
            return self.recover_err(&[pattern, index], ellipsis);
        }
        if !self.pack_index_may_be_constant(index) {
            // Without the pattern: an error naming no pack.
            return self.recover_err(&[index], ellipsis);
        }
        self.builder().pack_indexing_expr(pattern, index, rsquare)
    }

    /// [expr.prim.pack.index]/1: an index that is not value-dependent is
    /// checked as a constant expression when the pack index is formed
    /// (clang's array-size converted constant expression check, "array
    /// size is not a constant expression"), before the pack's length is
    /// known. False after that diagnostic. A type pack index
    /// ([dcl.type.pack.index]) is checked only at instantiation, as clang.
    fn pack_index_may_be_constant(&mut self, index: ExprId) -> bool {
        if !self.in_template() || self.expr_is_value_dependent(index) {
            return true;
        }
        let at = self.begin_loc(index);
        let c = crate::conv::decay(self, index);
        let c = self.act_on_full_expr(c);
        let ctx = kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, at);
        match self.eval_int(c, ctx) {
            Ok(_) => true,
            Err(f) => {
                let d = self.diag(DiagId::sema_expr_not_cce, at).arg("array size");
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                false
            }
        }
    }

    /// [`Sema::act_pack_indexing_type`] for the type name `name` written
    /// at `name_loc`: a `T` that is not a pack is reported there by name,
    /// as clang does ("'T' does not refer to the name of a parameter
    /// pack").
    pub fn act_pack_indexing_type_named(
        &mut self,
        pattern: QualType,
        name: Symbol,
        name_loc: Loc,
        ellipsis: Loc,
        index: ExprId,
    ) -> QualType {
        if !self.ast.types.flags(pattern.ty()).contains(TypeFlags::UNEXPANDED_PACK) {
            let text = format!("'{}'", self.ast.symbols().as_str(name));
            let msg = self.diag(DiagId::sema_sizeof_pack_no_pack, name_loc).arg(text);
            self.emit(msg);
            return QualType::ERROR;
        }
        self.act_pack_indexing_type(pattern, ellipsis, index)
    }

    /// `T...[index]` as a type (C++26 [dcl.type.pack.index]).
    pub fn act_pack_indexing_type(
        &mut self,
        pattern: QualType,
        ellipsis: Loc,
        index: ExprId,
    ) -> QualType {
        if !self.ast.types.flags(pattern.ty()).contains(TypeFlags::UNEXPANDED_PACK) {
            self.error(DiagId::parse_expected_name_of_pack, ellipsis);
            return QualType::ERROR;
        }
        let p = self.dependent_profile(index);
        self.ast.types.pack_indexing(pattern, index, Some(p), None)
    }

    /// A template argument followed by `...`.
    pub fn act_pack_expansion_arg(&mut self, a: TemplateArg, ellipsis: Loc) -> TemplateArg {
        match a.view() {
            ArgView::Type(t) => TemplateArg::ty(self.act_pack_expansion_type(t, ellipsis)),
            ArgView::Expr { expr, .. } => {
                let e = self.act_pack_expansion(expr, ellipsis);
                self.builder().template_arg_expr(e)
            }
            ArgView::Template(n) => {
                if !self.ast.types.template_name_flags(n).contains(TypeFlags::UNEXPANDED_PACK) {
                    self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
                    return a;
                }
                TemplateArg::template_expansion(n, None)
            }
            _ => a,
        }
    }

    /// A fold-expression ([expr.prim.fold]): `( ... op e )`, `( e op ... )`
    /// or `( e1 op ... op e2 )`; exactly one operand of a binary fold has
    /// unexpanded packs ([temp.variadic]/11).
    pub fn act_fold(
        &mut self,
        lhs: Option<ExprId>,
        op: BinaryOp,
        rhs: Option<ExprId>,
        lparen: Loc,
        ellipsis: Loc,
        rparen: Loc,
    ) -> ExprId {
        let packs = |s: &Self, e: Option<ExprId>| {
            e.is_some_and(|e| s.ast.exprs.get(e).deps().contains(ExprDeps::UNEXPANDED_PACK))
        };
        let (l, r) = (packs(self, lhs), packs(self, rhs));
        if !l && !r {
            self.error(DiagId::sema_pack_expansion_without_packs, ellipsis);
            let kids: Vec<ExprId> = [lhs, rhs].into_iter().flatten().collect();
            return self.recover_err(&kids, lparen);
        }
        if l && r {
            self.error(DiagId::sema_fold_packs_both_sides, ellipsis);
        }
        let e = self.builder().fold_expr(lhs, op, rhs, lparen, rparen);
        self.templ.fold_ellipsis.insert(e, ellipsis);
        e
    }

    /// `sizeof...(name)` ([expr.sizeof]/5): `name` must name a pack.
    pub fn act_on_sizeof_pack_name(
        &mut self,
        name: Symbol,
        name_loc: Loc,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let found = self.lookup_name(name, LookupKind::Ordinary);
        let pack = found.decls.first().copied().filter(|&d| self.is_pack_decl(d));
        match pack {
            Some(d) => {
                self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
                let st = self.size_type();
                self.builder().size_of_pack(d, st, loc, rparen)
            }
            None => self.act_on_sizeof_pack(name, name_loc, loc),
        }
    }

    /// True for a template parameter pack, a function parameter pack, an
    /// init-capture pack or a structured binding pack ([temp.variadic]/3).
    pub fn is_pack_decl(&self, d: DeclId) -> bool {
        if let Some(i) = self.ast.template_parm(d) {
            return i.pack;
        }
        matches!(self.kind_of(d), DeclKind::ParmVar | DeclKind::Var | DeclKind::Binding)
            && self.ast.decls.get(d).ty().is_some_and(|t| {
                matches!(self.ast.types.view(t.ty()), TypeView::PackExpansion { .. })
            })
    }

    // -- specializations, instantiations, guides -------------------------------------

    /// `[extern] template declaration` ([temp.explicit]): explicit
    /// instantiation is WP-070a.
    pub fn act_explicit_instantiation(
        &mut self,
        extern_loc: Option<Loc>,
        loc: Loc,
    ) -> Option<DeclId> {
        let what = if extern_loc.is_some() {
            "explicit instantiation declaration"
        } else {
            "explicit instantiation"
        };
        self.templ_unimplemented(what, loc);
        None
    }

    /// `template<> declaration` ([temp.expl.spec]) or a partial
    /// specialization `template<params> class X<args>`
    /// ([temp.spec.partial]): WP-070a.
    pub fn act_explicit_specialization(&mut self, partial: bool, loc: Loc) -> Option<DeclId> {
        let what = if partial {
            "class template partial specialization"
        } else {
            "explicit specialization"
        };
        self.templ_unimplemented(what, loc);
        None
    }

    /// A deduction guide ([temp.deduct.guide]): CTAD is WP-074.
    pub fn act_deduction_guide(&mut self, name: Symbol, loc: Loc) -> Option<DeclId> {
        let _ = sym_str(self, name);
        self.templ_unimplemented("deduction guide", loc);
        None
    }

    // -- dependent expression forms ([temp.dep.expr]) -----------------------------

    /// True if `e` is type-dependent ([temp.dep.expr]/1).
    pub fn expr_is_type_dependent(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).deps().contains(ExprDeps::TYPE)
    }

    /// True if `e` is value-dependent ([temp.dep.constexpr]).
    pub fn expr_is_value_dependent(&self, e: ExprId) -> bool {
        self.ast.exprs.get(e).deps().intersects(ExprDeps::TYPE.with(ExprDeps::VALUE))
    }

    /// True if an operand of type `t` defers operator lookup to
    /// instantiation: a dependent type that may become a class or
    /// enumeration (clang's `isOverloadableType`; dependent pointers,
    /// arrays, functions and member pointers keep built-in semantics).
    /// The type of a subscript with a type-dependent operand (clang's
    /// `getDependentArraySubscriptType`): with an integral or unscoped
    /// enumeration index, the other operand's pointee or element type if
    /// that is dependent; else `<dependent type>`.
    pub(crate) fn dependent_subscript_type(&self, base: ExprId, idx: ExprId) -> QualType {
        let (bt, it) = (self.ety(base), self.ety(idx));
        let index_like = |t: QualType| {
            !self.ast.types.is_dependent(t)
                && (self.ast.types.is_integral(t) || crate::conv::is_unscoped_enum(self, t))
        };
        // The pointee or element type, keeping sugar (clang's
        // `getAs<PointerType>` / `getAsArrayTypeUnsafe`).
        let elem = |t: QualType| {
            let mut cur = t;
            for _ in 0..256 {
                match self.ast.types.view(cur.ty()) {
                    TypeView::Pointer(p) | TypeView::IncompleteArray(p) => return Some(p),
                    TypeView::ConstantArray(a) => return Some(a.elem),
                    TypeView::VariableArray { elem, .. }
                    | TypeView::DependentSizedArray { elem, .. } => return Some(elem),
                    _ => {
                        let next = self.ast.types.desugar_once(cur);
                        if next == cur {
                            return None;
                        }
                        cur = next;
                    }
                }
            }
            None
        };
        let r = if index_like(it) {
            elem(bt)
        } else if index_like(bt) {
            elem(it)
        } else {
            None
        };
        r.filter(|&t| self.ast.types.is_dependent(t)).unwrap_or(QualType::DEPENDENT)
    }

    pub(crate) fn is_dependent_overloadable(&self, t: QualType) -> bool {
        use kiln_ast::types::TypeKind;
        self.ast.types.is_dependent(t)
            && !matches!(
                self.ast.types.canonical_kind(t),
                TypeKind::Pointer | TypeKind::Function | TypeKind::MemberPointer
            )
            && !self.ast.types.is_array(t)
    }

    /// A cast to a dependent type or of a type-dependent operand
    /// ([temp.dep.expr]/3): kept with cast kind `Dependent` and the
    /// operand unconverted. `T(x)` is a `CXXUnresolvedConstructExpr`.
    pub(crate) fn dependent_cast(
        &mut self,
        style: crate::expr::CastStyle,
        ty: QualType,
        e: ExprId,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{CastKind, ExprFlags, ValueCategory};
        if style == crate::expr::CastStyle::Functional {
            return self.builder().unresolved_construct(ty, &[e], false, loc, rparen);
        }
        // Through sugar: `static_cast<result_type>(b)` with a member
        // `typedef X<R>& result_type;` is an lvalue (libstdc++ future:751).
        let vk = match self.ast.types.canonical_kind(ty) {
            TypeKind::LValueRef => ValueCategory::LValue,
            TypeKind::RValueRef => ValueCategory::XValue,
            _ => ValueCategory::PRValue,
        };
        let to = self.non_reference(ty);
        let kind = style.kind();
        // A cast to `void` is never dependent ([expr.static.cast]/6;
        // clang's `ToVoid`).
        let ck = if self.ast.types.is_void(to) { CastKind::ToVoid } else { CastKind::Dependent };
        self.builder().cast(kind, ck, e, to, vk, None, loc, ExprFlags::NONE)
    }

    /// A braced-init-list kept unanalyzed in a dependent context: an
    /// `InitListExpr` of type `void` ([temp.dep.expr]/3).
    pub(crate) fn dependent_init_list(&mut self, l: &crate::init::InitList) -> ExprId {
        let mut inits = Vec::with_capacity(l.items.len());
        for it in &l.items {
            let e = match &it.value {
                crate::init::InitValue::Expr(e) => *e,
                crate::init::InitValue::List(n) => self.dependent_init_list(n),
            };
            let e = if it.designator.is_empty() {
                e
            } else {
                // A designated clause is a `DesignatedInitExpr` (clang
                // `ActOnDesignatedInitializer`), rebuilt at instantiation.
                let ds: Vec<kiln_ast::expr::Designator> =
                    it.designator.iter().map(|d| self.designator_ast(d)).collect();
                let loc = it.designator[0].loc();
                // A designated pack expansion `.x = u...` is the
                // expansion of the designated clause (clang parses the
                // `...` after the whole designation).
                match crate::templ::pack::expr_pattern(self, e) {
                    Some(pat) => {
                        let ellipsis = self.ast.exprs.get(e).loc();
                        let d = self.builder().designated_init(&ds, pat, loc);
                        let keep = self.clang_compat >= 22;
                        self.builder().pack_expansion_expr(d, ellipsis, keep)
                    }
                    None => self.builder().designated_init(&ds, e, loc),
                }
            };
            inits.push(e);
        }
        self.builder().init_list(QualType::VOID, &inits, None, None, l.lbrace, l.rbrace)
    }

    /// The AST form of designator `d`: a value-dependent array index
    /// stays as written; a constant one is wrapped in its
    /// `ConstantExpr` (clang `CheckArrayDesignatorExpr`).
    fn designator_ast(&mut self, d: &crate::init::Designator) -> kiln_ast::expr::Designator {
        match *d {
            crate::init::Designator::Index(e, loc) => {
                kiln_ast::expr::Designator::Index { index: self.designator_index_ast(e, loc), loc }
            }
            crate::init::Designator::Range(a, b, loc, ellipsis) => {
                kiln_ast::expr::Designator::Range {
                    first: self.designator_index_ast(a, loc),
                    last: self.designator_index_ast(b, loc),
                    loc,
                    ellipsis,
                }
            }
            crate::init::Designator::Field(..) => d.to_ast(),
        }
    }

    /// An array designator's index in the AST: as written when
    /// value-dependent, else wrapped in its `ConstantExpr`.
    fn designator_index_ast(&mut self, e: ExprId, loc: Loc) -> ExprId {
        if self.expr_is_value_dependent(e) || self.is_err_expr(e) {
            return e;
        }
        let c = crate::conv::lvalue_to_rvalue(self, e);
        let ctx = kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, loc);
        match self.eval_int(c, ctx) {
            Ok(v) => {
                let ty = self.ast.exprs.get(c).ty();
                let cv = self.int_const(ty, v);
                self.builder().constant_expr(c, Some(cv))
            }
            Err(_) => e,
        }
    }

    /// Records the written form of the semantic list `sem` built from
    /// the designated list `l` in a template pattern, for its
    /// instantiation to rebuild from (§5.6 of spec/ast.md; clang keeps
    /// the syntactic form of every list).
    pub(crate) fn note_syntactic_list(&mut self, sem: ExprId, l: &crate::init::InitList) {
        if self.ast.exprs.get(sem).kind() != ExprKind::InitListExpr
            || self.ast.exprs.syntactic_list(sem).is_some()
        {
            return;
        }
        let w = self.dependent_init_list(l);
        self.ast.exprs.set_syntactic_list(sem, w);
    }

    /// True if any initializer of `l` (recursively) is type-dependent,
    /// or an array designator's index is value-dependent (the element it
    /// names is known only at instantiation).
    pub(crate) fn init_list_type_dependent(&self, l: &crate::init::InitList) -> bool {
        l.items.iter().any(|it| {
            it.designator.iter().any(|d| match *d {
                crate::init::Designator::Index(e, _) => self.expr_is_value_dependent(e),
                crate::init::Designator::Range(a, b, _, _) => {
                    self.expr_is_value_dependent(a) || self.expr_is_value_dependent(b)
                }
                crate::init::Designator::Field(..) => false,
            }) || match &it.value {
                crate::init::InitValue::Expr(e) => self.expr_is_type_dependent(*e),
                crate::init::InitValue::List(n) => self.init_list_type_dependent(n),
            }
        })
    }

    /// A call with a type-dependent callee or argument ([temp.dep.expr]/3,
    /// [temp.dep.candidate]): a `<dependent type>` prvalue `CallExpr`. An
    /// unqualified callee naming non-member functions becomes an
    /// `UnresolvedLookupExpr` with ADL, unless the function is an implicit
    /// builtin ([basic.lookup.argdep]/1, clang `UseArgumentDependentLookup`): `__builtin_addressof(x)` with
    /// `x` a `Holder<Incomplete>*` must not complete `Holder<Incomplete>`
    /// at instantiation ([temp.inst]/2).
    pub(crate) fn dependent_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        use kiln_ast::expr::{ExprFlags, ValueCategory};
        let mut f = callee;
        let n = *self.ast.exprs.get(callee);
        if n.kind() == ExprKind::DeclRefExpr {
            let d = self.ast.exprs.decl(callee);
            let qualified =
                self.ast.exprs.decl_ref_extra(callee).is_some_and(|x| x.qualifier.is_some());
            if self.kind_of(d) == DeclKind::Function
                && !qualified
                && self.ast.decls.builtin_id(d).is_none()
            {
                let name = self.ast.decls.get(d).name().as_ident();
                if let Some(name) = name {
                    let name = DeclName::ident(name);
                    f = self.builder().unresolved_lookup(name, &[d], true, None, None, n.loc());
                }
            }
        }
        // The callee of a call is not a reference outside an immediate
        // invocation ([expr.const]/18): a dependent call of an immediate
        // function, qualified (kept as the `DeclRefExpr`) or not, is
        // checked as an invocation when instantiated.
        let r = self.ignore_parens(callee);
        self.forget_immediate_ref(r);
        let ty = QualType::DEPENDENT;
        self.builder().call(f, args, ty, ValueCategory::PRValue, loc, rparen, ExprFlags::NONE)
    }

    /// An unqualified name followed by `(` that ordinary lookup does not
    /// find: an implicit builtin, else an empty `UnresolvedLookupExpr`
    /// with ADL, which argument-dependent lookup at the call (or, inside a
    /// template, at instantiation, [temp.dep.candidate]) may resolve
    /// ([basic.lookup.argdep]). The call diagnoses an undeclared
    /// identifier when ADL finds nothing.
    pub fn act_on_undeclared_call_name(&mut self, name: Symbol, loc: Loc) -> ExprId {
        self.act_on_undeclared_call_name_dn(DeclName::ident(name), loc)
    }

    /// [`Sema::act_on_undeclared_call_name`] for any unqualified-id
    /// (`operator==(a, b)` found only by ADL; WP-121).
    pub fn act_on_undeclared_call_name_dn(&mut self, name: DeclName, loc: Loc) -> ExprId {
        if let Some(sym) = name.as_ident()
            && let Some(b) = self.lazily_create_builtin(sym, loc)
        {
            return self.build_decl_ref(b, loc);
        }
        self.builder().unresolved_lookup(name, &[], true, None, None, loc)
    }

    /// `base.name` / `base->name` on a type-dependent object
    /// ([temp.dep.expr]/5): a `CXXDependentScopeMemberExpr`; `->` on a
    /// pointer to a dependent type converts the pointer first. `None` if
    /// the object is not dependent.
    pub fn try_dependent_member(
        &mut self,
        base: ExprId,
        name: DeclName,
        targs: Option<ArgListId>,
        template_kw: bool,
        loc: Loc,
        arrow: bool,
    ) -> Option<ExprId> {
        self.try_dependent_member_q(base, None, name, targs, template_kw, loc, arrow)
    }

    /// [`Sema::try_dependent_member`] with an optional qualifier
    /// (`t.N::m`): a dependent qualifier ([temp.dep.type]/6) also makes
    /// the access dependent, and the specifier is kept for substitution
    /// (WP-121).
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub(crate) fn try_dependent_member_q(
        &mut self,
        base: ExprId,
        qual: Option<crate::using::Qualifier>,
        name: DeclName,
        targs: Option<ArgListId>,
        template_kw: bool,
        loc: Loc,
        arrow: bool,
    ) -> Option<ExprId> {
        let bt = self.ety(base);
        let dep_qual = qual.is_some_and(|q| self.is_dependent_ctx(q.ctx));
        if !dep_qual && !self.ast.types.is_dependent(bt) {
            return None;
        }
        // A member of the current instantiation found by lookup is an
        // ordinary member access ([temp.dep.expr]/5, clang's MemberExpr).
        if targs.is_none() && !template_kw && !dep_qual {
            let obj = if arrow {
                self.pointee_sugared(bt).filter(|_| self.ast.types.is_pointer(bt))
            } else {
                Some(bt)
            };
            if let Some(rec) = obj.and_then(|o| self.ast.types.as_record(o)) {
                let found = match qual {
                    Some(q) => {
                        let def = self.ast.definition(rec).unwrap_or(rec);
                        if self.qualifier_names_base(q.ctx, def) {
                            self.lookup_qualified_dn(q.ctx, name, LookupKind::Ordinary).decls
                        } else {
                            Vec::new()
                        }
                    }
                    None => self.lookup_in_class_dn(rec, name, LookupKind::Ordinary).decls,
                };
                // A member named by a dependent using-declaration
                // (`using __base::has_value;`) is found at instantiation.
                if let [d] = found[..]
                    && self.kind_of(d) != DeclKind::UnresolvedUsingValue
                {
                    return None;
                }
            }
        }
        let b = if arrow && !self.is_dependent_overloadable(bt) {
            crate::conv::decay_dependent(self, base)
        } else {
            base
        };
        let nns = qual.map(|q| q.nns);
        let e = self.builder().dependent_member(Some(b), arrow, name, template_kw, nns, targs, loc);
        if let Some(q) = qual {
            let _ = self.ast.templates.expr_begin.insert(e, q.loc);
        }
        Some(e)
    }
    /// Parameter `p` (the `idx`th) of the requires-expression whose body
    /// is `body` ([expr.prim.req.general]/4): a `ParmVarDecl` in the body,
    /// visible to the requirements.
    pub fn act_on_requires_param(
        &mut self,
        body: DeclId,
        p: &crate::declspec::ParamInfo,
        idx: u16,
    ) -> DeclId {
        let name = p.name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0));
        // A parameter pack ([temp.variadic]/5), expanded on substitution.
        let pack = matches!(self.ast.types.view(p.ty.ty()), TypeView::PackExpansion { .. });
        let mut flags = if pack { kiln_ast::decl::ParamData::PACK } else { 0 };
        // The prototype scopes enclosing the requires-expression's own
        // (`ParamData::scope_depth`): a trailing requires-clause's function
        // parameters count, an enclosing requires-expression body does not
        // (clang's `getFunctionScopeDepth`).
        let depth = self.prototype_params.len() + self.requires_param_scopes.saturating_sub(1);
        flags |= kiln_ast::decl::ParamData::scope_depth_flags(depth);
        let d = self.builder().param(body, name, p.loc, p.ty, idx, flags);
        if p.invalid {
            self.ast.decls.set_flags(d, DeclFlags::INVALID);
        }
        if let Some((n, _)) = p.name {
            self.push_local(n, d);
        }
        d
    }

    /// True if `t` names a parameter pack that is not expanded
    /// ([temp.variadic]/7; clang's `containsUnexpandedParameterPack`).
    pub fn has_unexpanded_pack(&self, t: QualType) -> bool {
        self.ast.types.flags(t.ty()).contains(TypeFlags::UNEXPANDED_PACK)
    }

    /// Whether declarator `d` with decl-specifiers `ds` names an
    /// unexpanded pack, judged syntactically before the declarator's type
    /// is built (clang's `containsUnexpandedParameterPacks(Declarator&)`):
    /// the named type of `ds`, an array bound, a function chunk's
    /// parameter types, `noexcept` operand or trailing return type. A
    /// pack the declarator itself expands (`Ts... a`) counts, as in clang.
    pub fn declarator_has_unexpanded_pack(
        &self,
        ds: &crate::declspec::DeclSpec,
        d: &crate::declspec::Declarator,
    ) -> bool {
        use crate::declspec::Chunk;
        let expr_pack =
            |e: ExprId| self.ast.exprs.get(e).deps().contains(ExprDeps::UNEXPANDED_PACK);
        if ds.named.is_some_and(|t| self.has_unexpanded_pack(t)) {
            return true;
        }
        d.chunks.iter().any(|c| match c {
            Chunk::Array { size, .. } => size.is_some_and(expr_pack),
            Chunk::Function { params, exc, trailing, .. } => {
                params.iter().any(|p| self.has_unexpanded_pack(p.ty))
                    || matches!(*exc, kiln_ast::types::ExcSpec::NoexceptExpr { expr, .. } if expr_pack(expr))
                    || trailing.is_some_and(|(t, _)| self.has_unexpanded_pack(t))
            }
            _ => false,
        })
    }

    /// The profile of a value-dependent expression `e` (for dependent
    /// array bounds and `noexcept` operands).
    pub(crate) fn dependent_profile(&mut self, e: ExprId) -> kiln_ast::ProfileId {
        let f = kiln_ast::types::deps_type_flags(self.ast.exprs.get(e).deps());
        let f = if f.is_empty() { TypeFlags::DEPENDENT_INSTANTIATION } else { f };
        self.ast.profile_expr_with(e, f)
    }

    /// True if class template `tmpl`'s name, used without arguments here,
    /// is its injected-class-name: inside its definition ([temp.local]/1).
    pub fn names_injected_class(&self, tmpl: DeclId) -> bool {
        self.kind_of(tmpl) == DeclKind::ClassTemplate && self.injected_class_for(tmpl).is_some()
    }

    /// The class whose injected-class-name class template `tmpl` names
    /// here ([temp.local]/1): the innermost enclosing pattern, partial
    /// specialization or specialization of `tmpl`. `None` outside them.
    pub fn injected_class_decl(&self, tmpl: DeclId) -> Option<DeclId> {
        if self.kind_of(tmpl) != DeclKind::ClassTemplate {
            return None;
        }
        self.injected_class_for(tmpl)
    }

    /// `args` with each injected-class-name written for a template
    /// template parameter of `name`'s template made the class template
    /// it names ([temp.local]/1; clang's
    /// `convertTypeTemplateArgumentToTemplate`), so that substitution
    /// keeps a template there, not the specialization; `None` when
    /// there is none.
    fn injected_template_args(
        &mut self,
        name: TemplateNameId,
        args: &[TemplateArg],
    ) -> Option<Vec<TemplateArg>> {
        let d = self.ast.types.template_decl(name)?;
        if !matches!(
            self.kind_of(d),
            DeclKind::ClassTemplate
                | DeclKind::TypeAliasTemplate
                | DeclKind::VarTemplate
                | DeclKind::Concept
        ) {
            return None;
        }
        let params = self.template_params(self.head_template(d));
        let mut out: Option<Vec<TemplateArg>> = None;
        for (i, &a) in args.iter().enumerate() {
            let Some(&p) = params.get(i).or_else(|| {
                params.last().filter(|&&p| self.ast.template_parm(p).is_some_and(|x| x.pack))
            }) else {
                break;
            };
            if self.kind_of(p) != DeclKind::TemplateTemplateParm {
                continue;
            }
            let ArgView::Type(t) = a.view() else { continue };
            let Some(tmpl) = self.injected_class_template(t) else { continue };
            let n = self.template_name_for(tmpl);
            out.get_or_insert_with(|| args.to_vec())[i] = TemplateArg::template(n);
        }
        out
    }

    /// The class template an unqualified-cv injected-class-name type
    /// `t` (through sugar) names.
    fn injected_class_template(&self, mut t: QualType) -> Option<DeclId> {
        let types = &self.ast.types;
        for _ in 0..64 {
            if !t.quals().is_empty() {
                return None;
            }
            match types.view(t.ty()) {
                TypeView::InjectedClassName { pattern, .. } => {
                    return self.injected_template_of(pattern);
                }
                TypeView::Elaborated { .. } | TypeView::Paren(_) => {}
                _ => return None,
            }
            let n = types.desugar_once(t);
            if n == t {
                return None;
            }
            t = n;
        }
        None
    }

    /// The class template whose injected-class-name class `d` declares
    /// ([temp.local]/1): the template of a pattern, a partial
    /// specialization or a specialization.
    pub(crate) fn injected_template_of(&self, d: DeclId) -> Option<DeclId> {
        let first = self.ast.first(d);
        if let Some(t) = self.ast.described_template(first) {
            return Some(t);
        }
        if let Some((_, p)) = self.ast.partial_spec_info(first) {
            return Some(p.template);
        }
        self.ast.spec_info(first).map(|(_, s)| s.template)
    }

    /// True inside a class whose injected-class-name names class template
    /// `tmpl` (its pattern, a partial or explicit specialization) or while
    /// a declarator scope is being entered: there `tmpl<args>::name` may
    /// name the current instantiation ([temp.dep.type]/1), so the
    /// parser's lookahead keeps classifying `name` by lookup in `tmpl`.
    pub fn may_name_current_instantiation(&self, tmpl: DeclId) -> bool {
        self.templ.entering_context || self.injected_class_for(tmpl).is_some()
    }

    /// The innermost class enclosing the current context whose
    /// injected-class-name names class template `tmpl` ([temp.local]/1):
    /// its pattern, one of its partial specializations or one of its
    /// specializations.
    pub(crate) fn injected_class_for(&self, tmpl: DeclId) -> Option<DeclId> {
        let tmpl = self.ast.first(tmpl);
        // Lexically enclosing classes (a friend defined in its class),
        // then semantically enclosing ones (an out-of-line member
        // definition `template<class T> void X<T>::f() { X x; }` is in
        // the scope of `X`, [basic.scope.class]).
        for semantic in [false, true] {
            let mut c = Some(self.ctx);
            while let Some(x) = c {
                if self.kind_of(x) == DeclKind::CXXRecord
                    && self.injected_template_of(x).is_some_and(|t| self.ast.first(t) == tmpl)
                {
                    return Some(x);
                }
                let (lex, sem) = (self.ast.decls.lexical_ctx(x), self.ast.decls.get(x).ctx());
                c = if semantic { sem.or(lex) } else { lex.or(sem) };
            }
        }
        // The class scopes entered for the rest of a qualified declarator
        // (`template<class T> void A<T>::f(const A&)`, [basic.lookup.unqual]/1).
        self.scopes.iter().rev().filter(|s| s.kind == ScopeKind::Record).find_map(|s| {
            let x = s.entity?;
            (self.kind_of(x) == DeclKind::CXXRecord
                && self.injected_template_of(x).is_some_and(|t| self.ast.first(t) == tmpl))
            .then_some(x)
        })
    }

    /// The class template specialization whose injected-class-name `name`
    /// finds by lookup in `qualifier` (unqualified when `None`) through a
    /// base class, not as an enclosing class: in `struct D : B<int> {}`,
    /// `B` inside `D`, `D::B` and `typename D::B` name the type `B<int>`
    /// when no `<` follows ([temp.local]/1-2, [class.qual]/2). `None` for
    /// any other result, an ambiguous one included.
    pub fn inherited_injected_class(
        &self,
        name: Symbol,
        qualifier: Option<DeclId>,
    ) -> Option<DeclId> {
        let found = match qualifier.map(|q| self.type_name_qualifier(q)) {
            Some(q) if self.is_dependent_ctx(q) => return None,
            Some(q) => self.lookup_qualified(q, name, LookupKind::Ordinary),
            None => self.lookup_name(name, LookupKind::Ordinary),
        };
        let &d = found.decls.first()?;
        let first = self.ast.first(d);
        if self.kind_of(d) != DeclKind::CXXRecord
            || found.decls.iter().any(|&x| self.ast.first(x) != first)
            || self.ast.described_template(first).is_some()
            || self.ast.partial_spec_info(first).is_some()
        {
            return None;
        }
        self.ast.spec_info(first).map(|_| d)
    }

    /// The class template whose parameters spell the injected-class-name
    /// of class template pattern `c`: clang builds that type once, when
    /// the first declaration is seen, and caches it in the redeclaration
    /// chain's common data, so the first declaration's parameter names
    /// print. A friend declared in a templated class does not count: clang
    /// links it into the chain only at instantiation.
    /// At `-fclang-compat=22` the type belongs to the definition, so the
    /// definition's parameter names print (spec/sema.md §1.1); an
    /// implicit deduction guide still uses the first declaration's
    /// ([`Sema::first_injected_params_template`]).
    pub(crate) fn injected_params_template(&self, c: DeclId) -> Option<DeclId> {
        if self.clang_compat >= 22
            && let Some(t) = self.ast.definition(c).and_then(|d| self.ast.described_template(d))
        {
            return Some(t);
        }
        self.first_injected_params_template(c)
    }

    /// [`Sema::injected_params_template`] as clang 19 picks it, whatever
    /// the compat level: the first declaration that is not a friend of a
    /// templated class. clang 22's implicit deduction guides
    /// (`vector<_Tp, _Alloc>` from `<__fwd/vector.h>`) still name it.
    pub(crate) fn first_injected_params_template(&self, c: DeclId) -> Option<DeclId> {
        let own =
            self.ast.redecls_of(c).filter_map(|d| self.ast.described_template(d)).filter(|&t| {
                self.ast.decls.get(t).misc().friend() == FriendKind::None
                    || !self.in_templated_class(self.ast.decls.lexical_ctx(t))
            });
        own.last().or_else(|| self.ast.described_template(c))
    }

    /// The injected-class-name of class template `tmpl` used without
    /// arguments inside its own definition ([temp.local]/1): `X<P...>`
    /// for its parameters `P`, as an `InjectedClassNameType`. `None`
    /// outside the definition.
    pub fn injected_class_type(&mut self, tmpl: DeclId) -> Option<QualType> {
        if self.kind_of(tmpl) != DeclKind::ClassTemplate {
            return None;
        }
        let c = self.injected_class_for(tmpl)?;
        let first = self.ast.first(c);
        if self.ast.described_template(first).is_some() {
            let own = self.injected_params_template(c).unwrap_or(tmpl);
            return self.injected_type_of(own);
        }
        if self.ast.partial_spec_info(first).is_some() {
            return self.partial_injected_type(first);
        }
        Some(self.tag_type(c))
    }

    /// The canonical type of dependent specialization `ty` of class
    /// template `tmpl` when it names a partial specialization's current
    /// instantiation with a non-type argument written unconverted
    /// (`DequeIterator<V, 1>` in a member definition of the partial
    /// specialization `DequeIterator<V, 1>`, [temp.dep.type]/1.2): its
    /// injected-class-name type, which holds the converted arguments.
    pub(crate) fn partial_current_instantiation_canon(
        &mut self,
        tmpl: DeclId,
        ty: QualType,
        loc: Loc,
    ) -> Option<QualType> {
        let p = self.entered_partial_by_converted_args(tmpl, ty, loc)?;
        let first = self.ast.first(p);
        let inj = self.partial_injected_type(first)?;
        Some(self.ast.types.canonical(inj))
    }

    /// `X<A...>` for partial specialization `p`'s own arguments `A`, as
    /// an `InjectedClassNameType` ([temp.local]/1). The arguments are as
    /// written (clang `getInjectedSpecializationType`): the converted
    /// list's argument pack for the primary's trailing parameter pack
    /// (`vd<int, T...>` for `template<class I, class... T> struct vd`)
    /// is spread into its elements, so substituting the type converts
    /// them against the primary's parameters again (WP-787, Boost.Fusion
    /// `vector_data`).
    pub(crate) fn partial_injected_type(&mut self, p: DeclId) -> Option<QualType> {
        let (_, info) = self.ast.partial_spec_info(p)?;
        let (tmpl, mut args) = (info.template, info.args);
        let written = self.ast.types.args(args);
        if written.iter().any(|a| matches!(a.view(), ArgView::Pack(_))) {
            let mut flat = Vec::with_capacity(written.len());
            for &a in written {
                match a.view() {
                    ArgView::Pack(l) => flat.extend_from_slice(self.ast.types.args(l)),
                    _ => flat.push(a),
                }
            }
            args = self.ast.types.arg_list(&flat);
        }
        let name = self.template_name_for(tmpl);
        let spec = self.ast.types.template_specialization(name, args, None, false);
        Some(self.ast.types.injected_class_name(p, spec.ty()).into())
    }

    /// The type of `this`'s pointee in a member of class `class`
    /// ([expr.prim.this]/2): for the pattern of a class template, its
    /// injected-class-name type `X<P...>` (as clang); else the class type.
    pub(crate) fn this_class_type(&mut self, class: DeclId) -> QualType {
        let first = self.ast.first(class);
        if self.kind_of(class) == DeclKind::CXXRecord
            && let Some(t) = self.ast.described_template(first)
            && let Some(q) = self.injected_type_of(t)
        {
            return q;
        }
        if self.ast.partial_spec_info(first).is_some()
            && let Some(q) = self.partial_injected_type(first)
        {
            return q;
        }
        self.tag_type(class)
    }

    /// `X<P...>` for class template `tmpl`'s own parameters `P`, as an
    /// `InjectedClassNameType`.
    pub(crate) fn injected_type_of(&mut self, tmpl: DeclId) -> Option<QualType> {
        let pat = self.pattern_of(tmpl)?;
        let info = self.ast.template_info(tmpl).copied()?;
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let mut args = Vec::with_capacity(params.len());
        for p in params {
            let a = self.template_param_arg(p);
            args.push(a);
        }
        let name = self.template_name_for(tmpl);
        let list = self.ast.types.arg_list(&args);
        let spec = self.ast.types.template_specialization(name, list, None, false);
        let pat = self.ast.definition(pat).unwrap_or(pat);
        Some(self.ast.types.injected_class_name(pat, spec.ty()).into())
    }

    /// Template parameter `p` named as a template argument, as in an
    /// injected-class-name: `T`, `N`, `TT`, or `T...` for a pack.
    pub(crate) fn template_param_arg(&mut self, p: DeclId) -> TemplateArg {
        let pack = self.ast.template_parm(p).is_some_and(|i| i.pack);
        let loc = self.ast.decls.get(p).loc();
        match self.kind_of(p) {
            DeclKind::TemplateTypeParm => {
                let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                TemplateArg::ty(if pack { self.ast.types.pack_expansion(t, None) } else { t })
            }
            DeclKind::NonTypeTemplateParm => {
                // Built directly: naming the parameter here is not a
                // reference to it (clang leaves it unreferenced).
                let mut t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(t.ty()) {
                    t = pattern;
                }
                let t = self.non_reference(t);
                let mut e = self.builder().decl_ref(
                    p,
                    t,
                    loc,
                    None,
                    kiln_ast::expr::NonOdrUse::None,
                    kiln_ast::expr::ExprFlags::NONE,
                );
                if pack {
                    e = self.act_pack_expansion(e, loc);
                }
                self.builder().template_arg_expr(e)
            }
            _ => {
                let n = self.template_name_for(p);
                if pack {
                    TemplateArg::template_expansion(n, None)
                } else {
                    TemplateArg::template(n)
                }
            }
        }
    }
}
