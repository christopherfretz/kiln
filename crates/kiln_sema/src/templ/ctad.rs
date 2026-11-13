//! Class template argument deduction ([over.match.class.deduct]) and
//! deduction-guide declarations ([temp.deduct.guide]); spec/sema.md
//! "Class template argument deduction".

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, FnSpec, ParamData, TagKind};
use kiln_ast::expr::BinaryOp;
use kiln_ast::ids::{ArgListId, TemplateNameId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::types::{
    ArgView, AutoKeyword, ExcSpec, FuncInfo, FuncProto, Nns, Quals, TagKeyword, TemplateArg,
    TemplateName, TypeKind, TypeView,
};
use kiln_ast::{DeclHead, DeclId, ExprId, NnsId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::declspec::{Chunk, DeclSpec, Declarator};
use crate::init::{Designator, InitItem, InitList, InitValue, Initializer};
use crate::overload::{
    Candidate, CandidateFailure, CandidateKind, IcsOptions, OverloadCandidateSet, OvlOutcome,
    SetKind,
};
use crate::templ::args::{LevelMode, MultiLevelArgs};
use crate::templ::deduce::{DeducedArgs, Tdk};
use crate::templ::pack::Expansion;
use crate::templ::stack::FrameKind;
use crate::templ::subst::{DeclOrPack, SubstFail, SubstResult, Substituter};

mod inherit;

/// How the object whose type is a deduced class template specialization
/// is initialized ([over.match.class.deduct]/1, /3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub(crate) enum CtadInit {
    /// `T x(a...)`, `T(a...)`, `new T(a...)`, `T x;`.
    Direct,
    /// `T x{a...}`, `T{a...}`.
    DirectList,
    /// `T x = a;`: explicit guides are not candidates.
    Copy,
    /// `T x = {a...};`: selecting an explicit guide is ill-formed.
    CopyList,
}

/// What an implicit deduction guide was formed from
/// ([over.match.class.deduct]/1, [over.match.best.general]/2.10-2.13).
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub(crate) enum GuideOrigin {
    /// A non-template constructor, or the hypothetical `C()`.
    Ctor,
    /// A constructor template.
    CtorTemplate,
    /// The copy deduction candidate `C(C<P...>) -> C<P...>`.
    Copy,
    /// The aggregate deduction candidate (P1816).
    Aggregate,
}

/// A guide formed from a guide of a base class template whose
/// constructors a class template inherits ([over.match.class.deduct]/1,
/// P2582; spec/sema-templates.md §6.6.1).
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct InheritedGuide {
    /// The inheriting class template.
    pub(crate) class: DeclId,
    /// The base as written in its base-specifier (`B`).
    pub(crate) base: QualType,
    /// `Some(R)`: the guide's return type is already `CC<R>::type`, and
    /// `R` (the source guide's return type, transformed) is what its
    /// deducibility constraint names. `None`: the return type is `R`,
    /// mapped to `CC<R>::type` when the guide is selected.
    pub(crate) source_ret: Option<QualType>,
}

/// The signature of an implicit guide being formed.
struct GuideShape<'p> {
    ret: QualType,
    cparams: &'p [DeclId],
    ptys: &'p [QualType],
    variadic: bool,
    explicit: bool,
    /// The dependent depth of the guide's declaration.
    depth: u16,
    /// Not written by the user: false only for an alias template's guide
    /// formed from a user-declared one, which keeps that guide's
    /// precedence over implicit ones ([over.match.best.general]/2.11;
    /// clang `buildDeductionGuide(..., F->isImplicit())`).
    implicit: bool,
}

/// The signature of the guide of a constructor template whose template
/// parameter list is `fl` ([`Sema::ctor_template_guide`]): the new
/// template parameters (indices from `offset`) and the constructor's
/// parameter types, substituted.
fn ctad_guide_head(
    s: &mut Substituter<'_, '_, '_>,
    ctx: DeclId,
    fl: kiln_ast::templates::TemplateParamList,
    offset: u16,
    cparams: &[DeclId],
    loc: Loc,
) -> SubstResult<(Vec<DeclId>, Vec<QualType>)> {
    let old: Vec<DeclId> = s.sema.ast.lists.iter::<DeclId>(fl.params).collect();
    let mut new = Vec::with_capacity(old.len());
    for p in old {
        new.push(ctad_redepth_param(s, ctx, p, offset)?);
    }
    let mut ptys = Vec::with_capacity(cparams.len());
    for &p in cparams {
        let t = s.sema.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let t = if let TypeView::PackExpansion { pattern, .. } = s.sema.ast.types.view(t.ty()) {
            // A function parameter pack stays one (its pack is the
            // re-depthed template parameter pack, or for a member
            // template's constructor, the member's own pack mapped to
            // a one-element expansion of its new pack).
            match s.expand_pattern(|s| s.ty(pattern, loc))? {
                Expansion::Unexpanded(x) => s.sema.ast.types.pack_expansion(x, None),
                Expansion::Elements(es) if es.len() == 1 => s.sema.strip_expansion_ty(es[0]),
                Expansion::Elements(_) => return Err(SubstFail),
            }
        } else {
            s.ty(t, loc)?
        };
        ptys.push(t);
    }
    Ok((new, ptys))
}

/// The constraints `requires` (the constructor template's head and
/// trailing requires-clauses) substituted, as a conjunction.
fn ctad_guide_requires(
    s: &mut Substituter<'_, '_, '_>,
    requires: [Option<ExprId>; 2],
    loc: Loc,
) -> SubstResult<Option<ExprId>> {
    let mut conj: Option<ExprId> = None;
    for r in requires.into_iter().flatten() {
        let e = s.expr(r)?;
        conj = Some(match conj {
            Some(l) => s.sema.act_on_binary(BinaryOp::LAnd, l, e, loc),
            None => e,
        });
    }
    Ok(conj)
}

/// Template parameter `p` of a constructor template, redeclared one
/// level shallower at index `offset + its index` (a template template
/// parameter's own list re-depthed with it), its default argument and
/// type-constraint substituted.
fn ctad_redepth_param(
    s: &mut Substituter<'_, '_, '_>,
    ctx: DeclId,
    p: DeclId,
    offset: u16,
) -> SubstResult<DeclId> {
    let n = *s.sema.ast.decls.get(p);
    let Some(pi) = s.sema.ast.template_parm(p).copied() else { return Err(SubstFail) };
    let depth = s.args.retained_depth(pi.depth);
    let index = pi.index.saturating_add(offset);
    let h = DeclHead::new(ctx, n.name(), n.loc());
    let np = match n.kind() {
        DeclKind::TemplateTypeParm => {
            s.sema.builder().template_type_parm(h, depth, index, pi.pack, pi.typename_kw)
        }
        DeclKind::NonTypeTemplateParm => {
            let t = s.decl_ty(p);
            let ty = s.ty(t, n.loc())?;
            s.sema.builder().non_type_template_parm(h, ty, depth, index, pi.pack)
        }
        DeclKind::TemplateTemplateParm => {
            let Some(inner) = pi.params else { return Err(SubstFail) };
            let il = *s.sema.ast.templates.param_list(inner);
            let olds: Vec<DeclId> = s.sema.ast.lists.iter::<DeclId>(il.params).collect();
            let mut news = Vec::with_capacity(olds.len());
            for q in olds {
                news.push(ctad_redepth_param(s, ctx, q, 0)?);
            }
            let idepth = s.args.retained_depth(il.depth);
            let inner = s.sema.builder().template_param_list(
                il.template_loc,
                il.l_angle,
                il.r_angle,
                idepth,
                &news,
                il.requires,
            );
            let np = s.sema.builder().template_template_parm(
                h,
                inner,
                depth,
                index,
                pi.pack,
                pi.typename_kw,
            );
            s.sema.builder().set_ttp_kind(np, pi.ttp_kind);
            np
        }
        _ => return Err(SubstFail),
    };
    s.locals.insert(p, DeclOrPack::Decl(np));
    if let Some(c) = pi.constraint {
        let c = s.expr(c)?;
        s.sema.builder().set_parm_constraint(np, c);
    }
    if let Some(a) = s.sema.template_default(p) {
        let a = s.template_arg(a, n.loc())?;
        // clang 22 clones an inherited default as inherited, so the
        // guide's printed head omits it (`class _Allocator`).
        if s.sema.clang_compat >= 22 && s.sema.ast.templates.defaults.get(p).is_none() {
            s.sema.inherited_defaults.insert(np, a);
        } else {
            let _ = s.sema.builder().set_template_default(np, a);
        }
    }
    Ok(np)
}

impl Sema<'_, '_> {
    /// The undeduced placeholder `auto`: the declared return type of a
    /// deduction guide before its trailing return type replaces it.
    pub fn plain_auto_type(&mut self) -> QualType {
        self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false)
    }

    /// True if `d`, named without template arguments outside its own
    /// definition, is a placeholder for a deduced class type
    /// ([dcl.type.class.deduct]/1): a class template, (P1814) an alias
    /// template, or a template template parameter (deduced once its
    /// argument is known; clang's `ClassifyName` makes the same
    /// dependent `DeducedTemplateSpecializationType`).
    pub fn is_ctad_template(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::ClassTemplate => !self.names_injected_class(d),
            DeclKind::TypeAliasTemplate | DeclKind::TemplateTemplateParm => true,
            _ => false,
        }
    }

    /// True if a class or alias template name (or template template
    /// parameter) `d` alone is a type: its injected-class-name or a CTAD
    /// placeholder.
    pub fn names_class_type(&self, d: DeclId) -> bool {
        matches!(
            self.kind_of(d),
            DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate | DeclKind::TemplateTemplateParm
        )
    }

    /// The undeduced placeholder `X` / `nns::X` for class template `d`
    /// ([dcl.type.class.deduct]), its type-specifier at `locs.0` and the
    /// template name at `locs.1`.
    pub fn ctad_placeholder(
        &mut self,
        d: DeclId,
        nns: Option<NnsId>,
        locs: (Loc, Loc),
    ) -> QualType {
        if locs.0.is_valid() && locs.1.is_valid() {
            let _ = self.templ.ctad_name_locs.insert(locs.0, locs.1);
        }
        let mut name = self.template_name_for(d);
        if let Some(nns) = nns {
            name = self.ast.types.template_name(TemplateName::Qualified {
                nns,
                template_kw: false,
                underlying: name,
            });
        }
        self.ast.types.deduced_template_specialization(name, None)
    }

    /// A deduction-guide declaration for class template `template`
    /// ([temp.deduct.guide]): a `CXXDeductionGuide` named
    /// `<deduction guide for X>` in the current context, wrapped in a
    /// function template by its template head.
    pub(crate) fn act_on_deduction_guide(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
        template: DeclId,
    ) -> Option<DeclId> {
        // clang forms the placeholder `X` for the guide's declarator name
        // while parsing it, which fixes the spelling every later deduced
        // `X` type prints with (`TypeContext::deduced_template_specialization`).
        // [dcl.attr.annotation]/1: a deduction guide declares no entity.
        self.reject_annotations_in(&ds.attrs, Some("a deduction guide"));
        self.reject_annotations_in(&d.attrs, Some("a deduction guide"));
        let placeholder_name = self.template_name_for(template);
        let _ = self.ast.types.deduced_template_specialization(placeholder_name, None);
        let sloc = d.special.map_or(d.loc, |(_, l)| l);
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        let invalid = ds.invalid || d.invalid || self.is_error_type(ty);
        let Some(Chunk::Function { params, variadic, .. }) = d.chunks.first() else {
            self.templ.cur = None;
            return None;
        };
        if !self.ast.types.is_function(ty) {
            self.templ.cur = None;
            return None;
        }
        let (params, variadic) = (params.clone(), *variadic);
        let mut spec = FnSpec::new().with_variadic(variadic);
        let explicit = ds.explicit.is_some();
        let explicit_value = match (explicit, ds.explicit_expr) {
            (true, Some(e)) => self.explicit_value(e),
            (e, _) => Some(e),
        };
        if explicit && explicit_value != Some(false) {
            spec = spec.with_explicit(true);
        }
        let (ctx, lexical) = self.decl_contexts(d);
        let ctx = self.ast.definition(ctx).unwrap_or(ctx);
        let first = self.ast.first(template);
        let name = self.ast.names.deduction_guide(first);
        let (defaults, _) = self.merge_default_args(None, false, &params, true);
        let mut h = DeclHead::new(ctx, name, sloc);
        h.lexical = Some(lexical);
        self.mark_pattern_next();
        let f = self.builder().deduction_guide(h, ty, spec);
        if invalid {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        if explicit && let Some(e) = ds.explicit_expr {
            let _ = self.ast.decls.cold.explicit_expr.insert(f, e);
        }
        self.create_params(f, &params, &defaults);
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(f, r);
        }
        self.set_member_access(f);
        if let Some(t) = self.finish_template_pattern(f) {
            self.set_member_access(t);
            if !invalid {
                self.check_guide_deducible(t, f, sloc);
            }
        }
        Some(f)
    }

    /// [temp.deduct.guide]/3 (clang `CheckDeductionGuideTemplate`): every
    /// template parameter of deduction-guide template `t` is deducible from
    /// the parameter types of its guide `f`, is a pack, or has a default
    /// argument; otherwise `S(X<T>::m*) -> S<T>` is ill-formed.
    fn check_guide_deducible(&mut self, t: DeclId, f: DeclId, loc: Loc) {
        let params = self.template_params(t);
        let depth = self.template_param_depth(t);
        let ptys: Vec<TemplateArg> = self
            .ast
            .decls
            .function(f)
            .map(|fd| self.ast.lists.iter::<DeclId>(fd.params).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|p| TemplateArg::ty(self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR)))
            .collect();
        let mut d = DeducedArgs::new(depth, params.len());
        // Each parameter type against itself: a parameter in a deduced
        // context binds (to itself), one only in non-deduced contexts
        // ([temp.deduct.type]/5) stays null.
        for &p in &ptys {
            if self.deduce_args(&[p], &[p], true, &mut d, loc) != Tdk::Success {
                return;
            }
        }
        let missing: Vec<DeclId> = params
            .iter()
            .zip(&d.args)
            .filter(|&(&p, a)| {
                matches!(a.view(), ArgView::Null)
                    && !self.is_pack_decl(p)
                    && self.template_default(p).is_none()
            })
            .map(|(&p, _)| p)
            .collect();
        if missing.is_empty() {
            return;
        }
        let mut msg = self
            .diag(DiagId::sema_deduction_guide_not_deducible, loc)
            .arg(u32::from(missing.len() > 1));
        for &m in &missing {
            let note = self
                .diag(DiagId::note_non_deducible_parameter, self.ast.decls.get(m).loc())
                .arg(self.decl_name(m));
            msg = msg.with_note(note);
        }
        self.emit(msg);
        self.ast.decls.set_flags(t, DeclFlags::INVALID);
        self.ast.decls.set_flags(f, DeclFlags::INVALID);
    }

    /// The class template `ty` is an undeduced placeholder for
    /// ([dcl.type.class.deduct]), with its template name as written.
    pub(crate) fn undeduced_ctad(&self, ty: QualType) -> Option<(TemplateNameId, DeclId)> {
        let Some(TypeView::DeducedTemplateSpecialization { name, deduced: None }) =
            self.ast.types.view_if(ty.ty(), TypeKind::DeducedTemplateSpecialization)
        else {
            return None;
        };
        let t = self.ast.types.template_decl(name)?;
        matches!(self.kind_of(t), DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate)
            .then_some((name, t))
    }

    /// The implicit deduction guides of class template `tmpl`
    /// ([over.match.class.deduct]/1.1-1.3): one per constructor of its
    /// definition (or one from a hypothetical `C()`), and the copy
    /// deduction candidate. Formed once per defined template; they are
    /// template decls that join no member list or lookup table. A
    /// diagnostic while forming them notes `name_loc`, the template name
    /// that first needed them (clang `BuildingDeductionGuides`).
    fn implicit_guides(&mut self, tmpl: DeclId, name_loc: Loc) -> Vec<DeclId> {
        let first = self.ast.first(tmpl);
        if let Some(v) = self.templ.ctad_guides.get(&first) {
            return v.clone();
        }
        if !self.push_inst_frame(FrameKind::BuildingDeductionGuides, tmpl, tmpl, name_loc) {
            return Vec::new();
        }
        let out = self.form_implicit_guides(tmpl, first);
        self.pop_inst_frame();
        out
    }

    /// The injected-class-name type `C<P...>` an implicit guide of class
    /// template `owner` returns (and its copy guide takes): clang builds
    /// it once from the first declaration's template parameters, so their
    /// names print (`vector<_Tp, _Alloc>` from `<__fwd/vector.h>`, and
    /// `map<type-parameter-0-0, ...>` from an unnamed `template <class,
    /// class, class, class> class map;`); see
    /// [`Sema::first_injected_params_template`] (clang 22 too).
    fn guide_injected_type(&mut self, owner: DeclId) -> Option<QualType> {
        let own = self
            .pattern_of(owner)
            .and_then(|p| self.first_injected_params_template(p))
            .unwrap_or(owner);
        self.injected_type_of(own)
    }

    /// The type an implicit guide of `owner` names its class with: clang 22
    /// names the injected specialization by its scope (`N::S<T>`,
    /// `Outer<int>::Q<U>`; inline namespaces elided), clang 19 bare.
    fn guide_class_type(&mut self, owner: DeclId, injected: QualType) -> QualType {
        if self.clang_compat < 22 {
            return injected;
        }
        let ctx = self.ast.decls.get(owner).ctx();
        match ctx.and_then(|c| self.scope_nns(c)) {
            Some(nns) => self.ast.types.elaborated(TagKeyword::None, Some(nns), injected),
            None => injected,
        }
    }

    /// The nested-name-specifier that names scope `ctx` from the global
    /// scope as clang's fully-qualified printing does: inline and
    /// anonymous namespaces are elided; a function scope ends it.
    fn scope_nns(&mut self, ctx: DeclId) -> Option<NnsId> {
        let parent = self.ast.decls.get(ctx).ctx();
        match self.kind_of(ctx) {
            DeclKind::Namespace => {
                let prefix = parent.and_then(|p| self.scope_nns(p));
                let first = self.ast.first(ctx);
                let elided = self
                    .ast
                    .decls
                    .namespace(first)
                    .is_some_and(kiln_ast::decl::NamespaceData::is_inline)
                    || self.decl_name_str(self.ast.decls.get(first).name()).is_empty();
                if elided {
                    return prefix;
                }
                Some(self.ast.types.nns(Nns::Namespace { prefix, ns: first }))
            }
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization => {
                let prefix = parent.and_then(|p| self.scope_nns(p));
                let ty = match self.ast.described_template(ctx) {
                    Some(t) => self.injected_type_of(t)?,
                    None => self.class_type(ctx),
                };
                Some(self.ast.types.nns(Nns::Type { prefix, ty, template_kw: false }))
            }
            _ => None,
        }
    }

    /// [`Sema::implicit_guides`] once the frame is pushed.
    fn form_implicit_guides(&mut self, tmpl: DeclId, first: DeclId) -> Vec<DeclId> {
        let Some(pat) = self.pattern_of(tmpl) else { return Vec::new() };
        let def = self.ast.definition(pat);
        // The template decl whose parameters the definition uses.
        let owner = def.and_then(|d| self.ast.described_template(d)).unwrap_or(tmpl);
        let Some(info) = self.ast.template_info(owner).copied() else { return Vec::new() };
        let Some(TypeView::InjectedClassName { injected, .. }) =
            self.guide_injected_type(owner).map(|q| self.ast.types.view(q.ty()))
        else {
            return Vec::new();
        };
        let injected = self.guide_class_type(owner, injected);
        let ctx = self.ast.decls.get(first).ctx().unwrap_or(self.ast.tu);
        let name = self.ast.names.deduction_guide(first);
        let saved = self.ast.templates.dependent_depth;
        let mut out = Vec::new();
        let mut any_ctor = false;
        if let Some(def) = def {
            let members: Vec<DeclId> = self.ast.members(def).collect();
            for m in members {
                let n = self.ast.decls.get(m);
                if n.has(DeclFlags::IMPLICIT) || n.has(DeclFlags::INVALID) {
                    continue;
                }
                match n.kind() {
                    DeclKind::CXXConstructor => {
                        any_ctor = true;
                        let g =
                            self.implicit_guide(ctx, name, injected, info.params, Some(m), n.loc());
                        out.extend(g);
                    }
                    DeclKind::FunctionTemplate
                        if self
                            .pattern_of(m)
                            .is_some_and(|p| self.kind_of(p) == DeclKind::CXXConstructor) =>
                    {
                        any_ctor = true;
                        let g = self.ctor_template_guide(ctx, name, injected, info.params, m, None);
                        out.extend(g);
                    }
                    _ => {}
                }
            }
        }
        // A member template of an instantiated class template ([temp.mem])
        // has no definition of its own until a specialization of it is
        // instantiated; its constructors are those of the original
        // member template's definition, the enclosing arguments
        // substituted.
        let mut member_def = None;
        if def.is_none() && self.inst_template(tmpl) != tmpl {
            let orig = self.inst_template(tmpl);
            member_def = self.pattern_of(orig).and_then(|p| self.ast.definition(p));
            if let Some(od) = member_def {
                let (gs, any) = self.member_ctor_guides(ctx, name, injected, info.params, tmpl, od);
                any_ctor = any;
                out.extend(gs);
            }
        }
        if !any_ctor {
            // clang's `buildSimpleDeductionGuide` sits at the template
            // whose definition this is, like the copy guide.
            let loc = self.ast.decls.get(owner).loc();
            out.extend(self.implicit_guide(ctx, name, injected, info.params, None, loc));
        }
        // [over.match.class.deduct]/1.3: `C(C<T...>) -> C<T...>`.
        let loc = self.ast.decls.get(owner).loc();
        let g = self.copy_guide(ctx, name, injected, info.params, loc);
        let _ = self.templ.ctad_origin.insert(g, GuideOrigin::Copy);
        out.push(g);
        self.ast.templates.dependent_depth = saved;
        if def.is_some() || member_def.is_some() {
            let _ = self.templ.ctad_guides.insert(first, out.clone());
        }
        out
    }

    /// The guides of the non-template constructors of `od`, the
    /// definition of the member template that `tmpl` (a member template
    /// of a class template specialization) was instantiated from: their
    /// parameter types with the enclosing specialization's arguments and
    /// `tmpl`'s own parameters substituted; and the guides of its
    /// constructor templates ([`Sema::ctor_template_guide`]). Also
    /// whether `od` declares a constructor.
    fn member_ctor_guides(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        ret: QualType,
        params: TemplateParamListId,
        tmpl: DeclId,
        od: DeclId,
    ) -> (Vec<DeclId>, bool) {
        let pl = *self.ast.templates.param_list(params);
        let own: Vec<DeclId> = self.ast.lists.iter::<DeclId>(pl.params).collect();
        let mut args = Vec::with_capacity(own.len());
        for &p in &own {
            let a = self.template_param_arg(p);
            let pack = self.ast.template_parm(p).is_some_and(|i| i.pack);
            args.push(if pack { TemplateArg::pack(self.ast.types.arg_list(&[a])) } else { a });
        }
        let args = self.ast.types.arg_list(&args);
        let mut levels = self.base_levels(tmpl);
        levels.push(args, tmpl, LevelMode::Substitute);
        let members: Vec<DeclId> = self.ast.members(od).collect();
        let mut out = Vec::new();
        let mut any_ctor = false;
        let saved = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = u32::from(pl.depth) + 1;
        for m in members {
            let n = *self.ast.decls.get(m);
            if n.has(DeclFlags::IMPLICIT) || n.has(DeclFlags::INVALID) {
                continue;
            }
            match n.kind() {
                DeclKind::CXXConstructor => {}
                DeclKind::FunctionTemplate
                    if self
                        .pattern_of(m)
                        .is_some_and(|p| self.kind_of(p) == DeclKind::CXXConstructor) =>
                {
                    any_ctor = true;
                    out.extend(self.ctor_template_guide(ctx, name, ret, params, m, Some(&levels)));
                    continue;
                }
                _ => continue,
            }
            any_ctor = true;
            let Some(fd) = self.ast.decls.function(m) else { continue };
            let cparams: Vec<DeclId> = self.ast.lists.iter::<DeclId>(fd.params).collect();
            let explicit = fd.spec.is_explicit();
            let cty = n.ty().unwrap_or(QualType::ERROR);
            let variadic = self.ast.types.proto(cty).is_some_and(FuncProto::variadic);
            let loc = n.loc();
            let ptys = (|| {
                let mut s = Substituter::new(self, &levels);
                let mut ptys = Vec::with_capacity(cparams.len());
                for &p in &cparams {
                    let t = s.sema.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    let t = if let TypeView::PackExpansion { pattern, .. } =
                        s.sema.ast.types.view(t.ty())
                    {
                        match s.expand_pattern(|s| s.ty(pattern, loc))? {
                            Expansion::Unexpanded(x) => s.sema.ast.types.pack_expansion(x, None),
                            Expansion::Elements(es) if es.len() == 1 => {
                                s.sema.strip_expansion_ty(es[0])
                            }
                            Expansion::Elements(_) => return Err(SubstFail),
                        }
                    } else {
                        s.ty(t, loc)?
                    };
                    ptys.push(t);
                }
                Ok::<_, SubstFail>(ptys)
            })();
            let Ok(ptys) = ptys else { continue };
            let shape = GuideShape {
                ret,
                cparams: &cparams,
                ptys: &ptys,
                variadic,
                explicit,
                depth: pl.depth + 1,
                implicit: true,
            };
            let f = self.guide_function(ctx, name, loc, &shape);
            let g = self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, params);
            let _ = self.templ.ctad_origin.insert(g, GuideOrigin::Ctor);
            out.push(g);
        }
        self.ast.templates.dependent_depth = saved;
        (out, any_ctor)
    }

    /// The guide `template<P...> [explicit] C(params of ctor) -> C<P...>`
    /// (`ctor` `None`: the hypothetical `C()`), with the constructor's
    /// trailing requires-clause ([over.match.class.deduct]/1.1).
    fn implicit_guide(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        ret: QualType,
        params: TemplateParamListId,
        ctor: Option<DeclId>,
        loc: Loc,
    ) -> Option<DeclId> {
        let (cparams, variadic, explicit) = match ctor {
            Some(c) => {
                let fd = self.ast.decls.function(c)?;
                let ps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(fd.params).collect();
                let ty = self.ast.decls.get(c).ty().unwrap_or(QualType::ERROR);
                let variadic = self.ast.types.proto(ty).is_some_and(FuncProto::variadic);
                (ps, variadic, fd.spec.is_explicit())
            }
            None => (Vec::new(), false, false),
        };
        let ptys: Vec<QualType> = cparams
            .iter()
            .map(|&p| self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR))
            .collect();
        let depth = self.ast.templates.param_list(params).depth + 1;
        let shape = GuideShape {
            ret,
            cparams: &cparams,
            ptys: &ptys,
            variadic,
            explicit,
            depth,
            implicit: true,
        };
        let f = self.guide_function(ctx, name, loc, &shape);
        if let Some(r) = ctor.and_then(|c| self.ast.templates.trailing_requires.get(c).copied()) {
            let _ = self.ast.templates.trailing_requires.insert(f, r);
        }
        let g = self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, params);
        let _ = self.templ.ctad_origin.insert(g, GuideOrigin::Ctor);
        Some(g)
    }

    /// The implicit guide of constructor template `ft`
    /// ([over.match.class.deduct]/1.1): its template parameters are the
    /// class template's (`class_params`) followed by the constructor
    /// template's, re-depthed to the class template's level and
    /// renumbered after it; its associated constraints are the class
    /// template's and the constructor template's.
    ///
    /// `member`: for a member template of a class template
    /// specialization ([`Sema::member_ctor_guides`]), the levels that
    /// substitute the enclosing arguments and map the original member's
    /// parameters to `class_params`; `ft` is then the original member's
    /// constructor template.
    fn ctor_template_guide(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        ret: QualType,
        class_params: TemplateParamListId,
        ft: DeclId,
        member: Option<&MultiLevelArgs>,
    ) -> Option<DeclId> {
        let ctor = self.pattern_of(ft)?;
        let finfo = self.ast.template_info(ft).copied()?;
        let cl = *self.ast.templates.param_list(class_params);
        let fl = *self.ast.templates.param_list(finfo.params);
        let class_ps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(cl.params).collect();
        let n = u16::try_from(class_ps.len()).ok()?;
        let levels = if let Some(member) = member {
            // The constructor template's level stays, renumbered to the
            // depth of `class_params` (every level outside it is
            // substituted or retained by `member`).
            if usize::from(fl.depth) != member.depth() {
                return None;
            }
            let mut levels = member.clone();
            levels.push(ArgListId::EMPTY, ft, LevelMode::Retain);
            levels
        } else {
            if fl.depth != cl.depth + 1 {
                return None;
            }
            // The outer levels and the constructor template's stay; the
            // class template's level is "substituted" by undeduced (null)
            // arguments, which keep its parameters, so the constructor
            // template's level becomes the class template's
            // ([`MultiLevelArgs::retained_depth`]).
            let mut levels = MultiLevelArgs::new();
            for _ in 0..cl.depth {
                levels.push(ArgListId::EMPTY, ft, LevelMode::Retain);
            }
            let nulls = vec![TemplateArg::NULL; class_ps.len()];
            let nulls = self.ast.types.arg_list(&nulls);
            levels.push(nulls, ft, LevelMode::Substitute);
            levels.push(ArgListId::EMPTY, ft, LevelMode::Retain);
            levels
        };
        let fd = self.ast.decls.function(ctor)?;
        let cparams: Vec<DeclId> = self.ast.lists.iter::<DeclId>(fd.params).collect();
        let cty = self.ast.decls.get(ctor).ty().unwrap_or(QualType::ERROR);
        let variadic = self.ast.types.proto(cty).is_some_and(FuncProto::variadic);
        let explicit = fd.spec.is_explicit();
        let loc = self.ast.decls.get(ft).loc();
        let trailing = self.ast.templates.trailing_requires.get(ctor).copied();
        let saved = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = u32::from(cl.depth) + 1;
        let built = (|| {
            let mut s = Substituter::new(self, &levels);
            let (new_ps, ptys) = ctad_guide_head(&mut s, ctx, fl, n, &cparams, loc)?;
            let mut all = class_ps;
            all.extend(new_ps);
            let list = s.sema.builder().template_param_list(
                fl.template_loc,
                fl.l_angle,
                fl.r_angle,
                cl.depth,
                &all,
                cl.requires,
            );
            let shape = GuideShape {
                ret,
                cparams: &cparams,
                ptys: &ptys,
                variadic,
                explicit,
                depth: cl.depth + 1,
                implicit: true,
            };
            let f = s.sema.guide_function(ctx, name, loc, &shape);
            // The trailing requires-clause may name the parameters.
            let gps: Vec<DeclId> = s
                .sema
                .ast
                .decls
                .function(f)
                .map(|x| s.sema.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            for (&o, nw) in cparams.iter().zip(gps) {
                s.locals.insert(o, DeclOrPack::Decl(nw));
            }
            let req = ctad_guide_requires(&mut s, [fl.requires, trailing], loc)?;
            Ok::<_, SubstFail>((list, f, req))
        })();
        self.ast.templates.dependent_depth = saved;
        let (list, f, requires) = built.ok()?;
        if let Some(r) = requires {
            let _ = self.ast.templates.trailing_requires.insert(f, r);
        }
        let g = self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, list);
        let _ = self.templ.ctad_origin.insert(g, GuideOrigin::CtorTemplate);
        Some(g)
    }

    /// The implicit `CXXDeductionGuide` `[explicit] C(ptys...) -> ret`,
    /// its parameters copied from the constructor's `cparams` (names and
    /// default arguments; [over.match.class.deduct]/1.1).
    fn guide_function(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        loc: Loc,
        g: &GuideShape<'_>,
    ) -> DeclId {
        let info = FuncInfo::new().with_trailing_return(true).with_variadic(g.variadic);
        let fty = self.ast.types.function(g.ret, g.ptys, info, ExcSpec::None);
        let mut h = DeclHead::new(ctx, name, loc);
        if g.implicit {
            h.flags = DeclFlags::IMPLICIT;
        }
        let spec = FnSpec::new().with_variadic(g.variadic).with_explicit(g.explicit);
        let saved = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = u32::from(g.depth);
        self.ast.templates.pattern_next = true;
        let f = self.builder().deduction_guide(h, fty, spec);
        let mut ids = Vec::with_capacity(g.ptys.len());
        for (i, &ty) in g.ptys.iter().enumerate() {
            // The aggregate deduction candidate has no constructor
            // parameters to copy: its parameters are unnamed.
            let (pname, ploc, data) = match g.cparams.get(i) {
                Some(&p) => {
                    let pn = *self.ast.decls.get(p);
                    (pn.name(), pn.loc(), self.ast.decls.param(p).copied())
                }
                None => (DeclName::EMPTY, loc, None),
            };
            let flags = match data {
                Some(d) => d.flags & !ParamData::DEFAULT_ARG_PENDING,
                None if matches!(self.ast.types.view(ty.ty()), TypeView::PackExpansion { .. }) => {
                    ParamData::PACK
                }
                None => 0,
            };
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            let np = self.builder().param(f, pname, ploc, ty, idx, flags);
            // A constructor parameter's default argument makes the guide's
            // parameter have one too, but its value is never used by
            // deduction: the guide gets a placeholder of the parameter's
            // type, not the constructor's expression (which names the
            // constructor's, not the guide's, template parameters; clang's
            // `transformFunctionTypeParam` builds the same `OpaqueValueExpr`).
            if let Some(d) = data
                && (d.default_arg.is_some() || d.flags & ParamData::DEFAULT_ARG_PENDING != 0)
            {
                let ph = self.guide_default_placeholder(ty, ploc);
                let _ = self.ast.decls.set_default_arg(np, ph);
            }
            ids.push(np);
        }
        let _ = self.builder().set_params(f, &ids);
        self.ast.templates.dependent_depth = saved;
        f
    }

    /// The default argument of a guide parameter of type `ty`: a
    /// source-less `OpaqueValueExpr` (printed `<null expr>`).
    pub(crate) fn guide_default_placeholder(&mut self, ty: QualType, loc: Loc) -> ExprId {
        let vc = if self.ast.types.is_reference(ty) {
            kiln_ast::expr::ValueCategory::LValue
        } else {
            kiln_ast::expr::ValueCategory::PRValue
        };
        self.builder().opaque_value_of(ty, vc, loc)
    }

    /// The copy deduction candidate `template<P...> C(C<P...>) -> C<P...>`.
    fn copy_guide(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        ret: QualType,
        params: TemplateParamListId,
        loc: Loc,
    ) -> DeclId {
        let info = FuncInfo::new().with_trailing_return(true);
        let fty = self.ast.types.function(ret, &[ret], info, ExcSpec::None);
        let mut h = DeclHead::new(ctx, name, loc);
        h.flags = DeclFlags::IMPLICIT;
        self.ast.templates.dependent_depth = 1;
        self.ast.templates.pattern_next = true;
        let f = self.builder().deduction_guide(h, fty, FnSpec::new());
        let p = self.builder().param(f, DeclName::EMPTY, loc, ret, 0, 0);
        let _ = self.builder().set_params(f, &[p]);
        self.ast.templates.dependent_depth = 0;
        self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, params)
    }

    /// The aggregate deduction candidate of class template `tmpl` for
    /// initializer `init` ([over.match.class.deduct]/1.5-1.8, P1816):
    /// `template<P...> C(T1, ..., Tn) -> C<P...>`, where `Ti` is the
    /// declared type of the element `xi` initializes. `None` when C is
    /// not a defined aggregate, the initializer is not a non-empty
    /// braced-init-list or parenthesized expression-list, or some `xi`
    /// initializes no element.
    fn aggregate_guide(&mut self, tmpl: DeclId, init: &Initializer) -> Option<DeclId> {
        let pat = self.pattern_of(tmpl)?;
        let def = self.ast.definition(pat)?;
        let bits = self.ast.decls.record(def).map(|x| x.defn).unwrap_or_default();
        if !bits.contains(DefinitionBits::AGGREGATE) {
            return None;
        }
        let mut ptys = Vec::new();
        match init {
            Initializer::List(l) if !l.items.is_empty() => {
                let mut pos = 0;
                self.agg_elements(def, &l.items, &mut pos, true, &mut ptys)?;
                if pos != l.items.len() {
                    return None;
                }
            }
            Initializer::Paren(es, ..) if !es.is_empty() => {
                // No brace elision and no designators in a parenthesized
                // list ([over.match.class.deduct]/1.6).
                let items: Vec<InitItem> = es
                    .iter()
                    .map(|&e| InitItem {
                        designator: Vec::new(),
                        value: match self.braced_args.iter().find(|(x, _)| *x == e) {
                            Some((_, l)) => InitValue::List(l.clone()),
                            None => InitValue::Expr(e),
                        },
                    })
                    .collect();
                let mut pos = 0;
                self.agg_elements(def, &items, &mut pos, true, &mut ptys)?;
                if pos != items.len() {
                    return None;
                }
            }
            _ => return None,
        }
        let first = self.ast.first(tmpl);
        let owner = self.ast.described_template(def).unwrap_or(tmpl);
        let info = self.ast.template_info(owner).copied()?;
        let Some(TypeView::InjectedClassName { injected, .. }) =
            self.guide_injected_type(owner).map(|q| self.ast.types.view(q.ty()))
        else {
            return None;
        };
        let injected = self.guide_class_type(owner, injected);
        let ctx = self.ast.decls.get(first).ctx().unwrap_or(self.ast.tu);
        let name = self.ast.names.deduction_guide(first);
        let loc = self.ast.decls.get(owner).loc();
        let depth = self.ast.templates.param_list(info.params).depth + 1;
        let shape = GuideShape {
            ret: injected,
            cparams: &[],
            ptys: &ptys,
            variadic: false,
            explicit: false,
            depth,
            implicit: true,
        };
        let saved = self.ast.templates.dependent_depth;
        let f = self.guide_function(ctx, name, loc, &shape);
        self.ast.templates.dependent_depth = saved;
        let g = self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, info.params);
        let _ = self.templ.ctad_origin.insert(g, GuideOrigin::Aggregate);
        Some(g)
    }

    /// The parameter types of the aggregate deduction candidate for the
    /// elements of class `def` that `items[*pos..]` initialize
    /// ([over.match.class.deduct]/1.5): bases then fields, brace elision
    /// assumed only for a non-dependent sub-aggregate or an array whose
    /// bound is not value-dependent. `braced` is false inside an elided
    /// sub-aggregate, where a designator or an excess clause ends it.
    /// `None` when some clause initializes no element.
    fn agg_elements(
        &mut self,
        def: DeclId,
        items: &[InitItem],
        pos: &mut usize,
        braced: bool,
        out: &mut Vec<QualType>,
    ) -> Option<()> {
        let bases: Vec<(QualType, bool)> =
            self.ast.decls.bases(def, &self.ast.lists).map(|b| (b.ty, b.pack_expansion)).collect();
        let fields = self.init_fields(def);
        let union = self.ast.decls.record(def).is_some_and(|x| x.tag == TagKind::Union);
        let nb = bases.len();
        let total = nb + fields.len();
        let mut cur = 0usize;
        let mut done_union = false;
        while *pos < items.len() {
            let item = &items[*pos];
            if let Some(d) = item.designator.first() {
                if !braced {
                    break;
                }
                let Designator::Field(name, _) = *d else { return None };
                if item.designator.len() > 1 {
                    return None;
                }
                let idx = fields
                    .iter()
                    .position(|&f| self.ast.decls.get(f).name().as_ident() == Some(name))?;
                cur = nb + idx;
            } else if union && done_union {
                cur = total;
            }
            if cur >= total {
                return if braced { None } else { Some(()) };
            }
            if cur < nb && bases[cur].1 {
                // A pack expansion element: when trailing, its parameter
                // pack takes every remaining clause ([over.match.class.
                // deduct]/1.7). clang forms no candidate otherwise.
                if cur + 1 != total
                    || !braced
                    || items[*pos..].iter().any(|i| !i.designator.is_empty())
                {
                    return None;
                }
                let pack = self.ast.types.pack_expansion(bases[cur].0, None);
                out.push(pack);
                *pos = items.len();
                return Some(());
            }
            let ety = if cur < nb {
                bases[cur].0
            } else {
                self.ast.decls.get(fields[cur - nb]).ty().unwrap_or(QualType::ERROR)
            };
            self.agg_element(ety, items, pos, out)?;
            cur += 1;
            done_union = union;
        }
        Some(())
    }

    /// The parameter type(s) for element type `ety` initialized by
    /// `items[*pos]` (and, under brace elision, the clauses after it).
    fn agg_element(
        &mut self,
        ety: QualType,
        items: &[InitItem],
        pos: &mut usize,
        out: &mut Vec<QualType>,
    ) -> Option<()> {
        let is_array = self.ast.types.is_array(ety);
        match items[*pos].value {
            // [over.match.class.deduct]/1.6.1: an array element initialized
            // by a braced-init-list is an rvalue reference to its type.
            InitValue::List(_) => {
                *pos += 1;
                out.push(if is_array { self.ast.types.rvalue_ref_to(ety) } else { ety });
            }
            InitValue::Expr(e) => {
                let elem = self.ast.types.element(ety);
                let string = is_array
                    && self.string_lit_of(e).is_some()
                    && elem.is_some_and(|t| self.ast.types.is_dependent(t) || self.is_char_type(t));
                if string {
                    // /1.6.2: by a string literal, an lvalue reference to
                    // its const-qualified type.
                    *pos += 1;
                    let c = self.ast.types.qualify(ety, Quals::CONST);
                    out.push(self.ast.types.lvalue_ref_to(c));
                } else if self.agg_elides(ety, e) {
                    if is_array {
                        let elem = elem.unwrap_or(QualType::ERROR);
                        let bound = self.ast.types.array_size(ety).unwrap_or(0);
                        let mut k = 0u64;
                        while k < bound && *pos < items.len() && items[*pos].designator.is_empty() {
                            self.agg_element(elem, items, pos, out)?;
                            k += 1;
                        }
                    } else {
                        let r = self.ast.types.as_record(ety)?;
                        let d = self.ast.definition(r)?;
                        self.agg_elements(d, items, pos, false, out)?;
                    }
                } else {
                    *pos += 1;
                    out.push(ety);
                }
            }
        }
        Some(())
    }

    /// Whether clause `e` initializing an element of type `ety` takes
    /// brace elision for deduction: never for a dependent non-array type
    /// or an array with a value-dependent bound; otherwise as
    /// [dcl.init.aggr]/16 (`Sema::init_element`).
    fn agg_elides(&mut self, ety: QualType, e: ExprId) -> bool {
        if self.ast.types.is_array(ety) {
            return self.ast.types.array_size(ety).is_some();
        }
        if self.ast.types.is_dependent(ety) {
            return false;
        }
        let Some(r) = self.ast.types.as_record(ety) else { return false };
        let ety_e = self.ast.exprs.get(e).ty();
        self.is_aggregate(ety)
            && !self.is_error_type(ety_e)
            && self.ast.types.as_record(ety_e).is_none_or(|x| {
                self.ast.first(x) != self.ast.first(r) && !conv::is_derived_from(self, x, r)
            })
    }

    /// The user-declared deduction guides of class template `tmpl`
    /// ([temp.deduct.guide]/3: declared in its scope).
    /// The guides of class or alias template `tmpl` for initializer
    /// `init`: a class template's implicit guides, its aggregate
    /// deduction candidate when it has no deduction-guides
    /// ([over.match.class.deduct]/1.5), and its deduction-guides; an
    /// alias template's are those of the template its defining-type-id
    /// names, transformed ([over.match.class.deduct]/3). `None` for an
    /// alias template whose defining-type-id names no class or alias
    /// template specialization.
    fn ctad_guides(
        &mut self,
        tmpl: DeclId,
        init: Option<&Initializer>,
        level: u32,
        name_loc: Loc,
    ) -> Option<Vec<DeclId>> {
        if self.kind_of(tmpl) == DeclKind::TypeAliasTemplate {
            let (target, rhs) = self.alias_ctad_target(tmpl)?;
            if level > 64 {
                return Some(Vec::new());
            }
            let mut inner = self.ctad_guides(target, init, level + 1, name_loc)?;
            // An inherited constructor's guide whose return type is still
            // the base's specialization (§6.6.1) cannot be transformed.
            inner.retain(|&g| !self.is_late_inherited_guide(g));
            let mut out = Vec::with_capacity(inner.len());
            for g in inner {
                let Some(ng) = self.alias_guide(tmpl, rhs, g) else { continue };
                // P2582: a guide formed from an inherited constructor's
                // stays one ([over.match.best.general]/2.10).
                if ng != g
                    && let Some(&ig) = self.templ.ctad_inherited.get(&g)
                {
                    let _ = self.templ.ctad_inherited.insert(ng, ig);
                }
                out.push(ng);
            }
            return Some(out);
        }
        let mut guides = self.implicit_guides(tmpl, name_loc);
        let user = self.user_guides(tmpl);
        // [over.match.class.deduct]/1.5: only when C has no
        // deduction-guides.
        if user.is_empty()
            && let Some(init) = init
            && let Some(g) = self.aggregate_guide(tmpl, init)
        {
            guides.push(g);
        }
        guides.extend(user);
        // P2582: the guides of the constructors `tmpl` inherits.
        let inherited = self.inherited_guides(tmpl, level, name_loc);
        guides.extend(inherited);
        Some(guides)
    }

    /// The class or alias template named by the defining-type-id of
    /// alias template `alias` and that type, when it is a
    /// simple-template-id ([over.match.class.deduct]/3).
    fn alias_ctad_target(&self, alias: DeclId) -> Option<(DeclId, QualType)> {
        let pat = self.ast.template_info(alias)?.pattern?;
        let mut rhs = self.ast.decls.typedef(pat)?.underlying;
        while let TypeView::Elaborated { named, .. } = self.ast.types.view(rhs.ty()) {
            rhs = named;
        }
        if !rhs.quals().is_empty() {
            return None;
        }
        let TypeView::TemplateSpecialization { name, .. } = self.ast.types.view(rhs.ty()) else {
            return None;
        };
        let t = self.ast.types.template_decl(name)?;
        matches!(self.kind_of(t), DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate)
            .then_some((t, rhs))
    }

    /// True if the template arguments of alias template `alias`
    /// (defining-type-id `rhs`) are deducible from `ty`
    /// ([over.match.class.deduct]/3, clang `__is_deducible`).
    pub(crate) fn alias_deducible(&mut self, alias: DeclId, rhs: QualType, ty: QualType) -> bool {
        use crate::templ::deduce::{DeducedArgs, TdfFlags, Tdk};
        let Some(info) = self.ast.template_info(alias).copied() else { return false };
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let mut d = DeducedArgs::new(list.depth, params.len());
        d.loc = self.ast.decls.get(alias).loc();
        if self.deduce(rhs, ty, TdfFlags::NONE, &mut d) != Tdk::Success {
            return false;
        }
        d.args.iter().zip(&params).all(|(a, &p)| {
            !matches!(a.view(), ArgView::Null) || self.ast.template_parm(p).is_some_and(|i| i.pack)
        })
    }

    /// The guide of alias template `alias` (defining-type-id `rhs`)
    /// formed from guide `g` of the template `rhs` names
    /// ([over.match.class.deduct]/3, P1814), cached; `None` when `g`'s
    /// return type does not match `rhs`.
    fn alias_guide(&mut self, alias: DeclId, rhs: QualType, g: DeclId) -> Option<DeclId> {
        if let Some(&r) = self.templ.ctad_alias_cache.get(&(alias, g)) {
            return r;
        }
        let r = if self.kind_of(g) == DeclKind::FunctionTemplate {
            self.build_alias_guide(alias, rhs, g, false).map(|r| r.0)
        } else {
            // A non-template guide: its return type is not dependent, so
            // its deducibility constraint is decided here.
            let ty = self.ast.decls.get(g).ty().unwrap_or(QualType::ERROR);
            let ret = self.ast.types.proto(ty).map(FuncProto::ret);
            ret.is_some_and(|r| self.alias_deducible(alias, rhs, r)).then_some(g)
        };
        let _ = self.templ.ctad_alias_cache.insert((alias, g), r);
        r
    }

    /// [over.match.class.deduct]/3 for guide template `g`: its return
    /// type's template arguments are deduced from `rhs`; the new guide's
    /// template parameters are the alias template's followed by `g`'s
    /// undeduced ones (renumbered after them), and its signature and
    /// constraints are `g`'s with the deductions substituted. Its
    /// deducibility constraint is checked by
    /// [`Sema::deduce_call`](crate::Sema) through `templ.ctad_alias`.
    ///
    /// `inherited`: `alias` is a class template inheriting constructors
    /// from base `rhs` (P2582, §6.6.1). The new guide's return type `R`
    /// is then replaced by `CC<R>::type` when the class template's
    /// arguments deduce from `R` here: that implies the deducibility
    /// constraint, which is not recorded, and `R` is returned beside the
    /// guide for its notes. Otherwise the return type stays `R`.
    #[allow(clippy::too_many_lines, reason = "the [over.match.class.deduct]/3 steps in order")]
    pub(super) fn build_alias_guide(
        &mut self,
        alias: DeclId,
        rhs: QualType,
        g: DeclId,
        inherited: bool,
    ) -> Option<(DeclId, Option<QualType>)> {
        use crate::templ::deduce::{DeducedArgs, TdfFlags};
        let f = self.pattern_of(g)?;
        let gl = *self.ast.templates.param_list(self.ast.template_info(g)?.params);
        let al = *self.ast.templates.param_list(self.ast.template_info(alias)?.params);
        if gl.depth != al.depth {
            return None;
        }
        let fty = self.ast.decls.get(f).ty()?;
        // As written: the new guide's parameters print with `g`'s sugar.
        let proto = self.ast.types.proto_of(fty.ty())?;
        let ret = proto.ret();
        let ptys: Vec<QualType> = self.ast.types.params(proto).to_vec();
        let variadic = proto.variadic();
        let gps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(gl.params).collect();
        let mut d = DeducedArgs::new(gl.depth, gps.len());
        d.expansions = true;
        d.loc = self.alias_guide_loc(alias);
        // clang 19 ignores the result: deduction stops at the first
        // argument it cannot deduce (`pair<A, B>`'s `B` against the `T...`
        // of `tuple<int, T...>`), and the guide is formed with what was
        // deduced, its deducibility constraint deciding later.
        let _ = self.deduce(ret, rhs, TdfFlags::NONE, &mut d);
        let first = self.ast.first(alias);
        let ctx = self.ast.decls.get(first).ctx().unwrap_or(self.ast.tu);
        // The alias template's parameters the deductions name (clang
        // `MarkUsedTemplateParameters`).
        let aps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(al.params).collect();
        let used = self.params_named(&d.args, al.depth, aps.len(), d.loc);
        let all_used = used.iter().all(|&u| u);
        // The deductions are canonical: name the (used, renumbered) alias
        // template parameters in them again.
        let mut resugar = MultiLevelArgs::new();
        for _ in 0..al.depth {
            resugar.push(ArgListId::EMPTY, alias, LevelMode::Retain);
        }
        let mut new_ps: Vec<DeclId> = Vec::with_capacity(aps.len());
        let mut named: Vec<TemplateArg> = vec![TemplateArg::NULL; aps.len()];
        for (j, &p) in aps.iter().enumerate() {
            if !used[j] {
                continue;
            }
            let pi = self.ast.template_parm(p).copied()?;
            let np = if all_used {
                p
            } else {
                let n = *self.ast.decls.get(p);
                let index = u16::try_from(new_ps.len()).ok()?;
                let h = DeclHead::new(ctx, n.name(), n.loc());
                match n.kind() {
                    DeclKind::TemplateTypeParm => self.builder().template_type_parm(
                        h,
                        al.depth,
                        index,
                        pi.pack,
                        pi.typename_kw,
                    ),
                    DeclKind::NonTypeTemplateParm => {
                        let mut lv = resugar.clone();
                        lv.push(self.ast.types.arg_list(&named), alias, LevelMode::Substitute);
                        let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                        let ty = Substituter::new(self, &lv).ty(t, n.loc()).ok()?;
                        self.builder().non_type_template_parm(h, ty, al.depth, index, pi.pack)
                    }
                    _ => return None,
                }
            };
            new_ps.push(np);
            let a = self.template_param_arg(np);
            named[j] = if pi.pack { TemplateArg::pack(self.ast.types.arg_list(&[a])) } else { a };
        }
        let named = self.ast.types.arg_list(&named);
        resugar.push(named, alias, LevelMode::Substitute);
        for a in &mut d.args {
            let mut s = Substituter::new(self, &resugar);
            *a = match a.view() {
                ArgView::Null => continue,
                // A deduced pack (its elements may be expansions).
                ArgView::Pack(l) => {
                    let l = s.args(l, Loc::INVALID).ok()?;
                    let v: Vec<TemplateArg> = self
                        .ast
                        .types
                        .args(l)
                        .iter()
                        .map(|&e| self.strip_expansion_sugar(e))
                        .collect();
                    TemplateArg::pack(self.ast.types.arg_list(&v))
                }
                // A canonical (profiled) value-dependent expression.
                ArgView::Profile(p) => {
                    let e = s.sema.ast.types.profile_expr(p)?;
                    let e = s.expr(e).ok()?;
                    s.sema.ast.builder().template_arg_expr(e)
                }
                _ => s.template_arg(*a, Loc::INVALID).ok()?,
            };
        }
        let name = self.ast.names.deduction_guide(first);
        let loc = self.alias_guide_loc(alias);
        let mut levels = MultiLevelArgs::new();
        for _ in 0..gl.depth {
            levels.push(ArgListId::EMPTY, g, LevelMode::Retain);
        }
        let saved = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = u32::from(gl.depth) + 1;
        let built = (|| {
            // The undeduced parameters, redeclared after the alias
            // template's.
            let mut sargs = d.args.clone();
            let mut moved: Vec<(DeclId, DeclId)> = Vec::new();
            for (i, &p) in gps.iter().enumerate() {
                if !matches!(sargs[i].view(), ArgView::Null) {
                    continue;
                }
                let n = *self.ast.decls.get(p);
                let pi = self.ast.template_parm(p).copied().ok_or(SubstFail)?;
                let index = u16::try_from(new_ps.len()).map_err(|_| SubstFail)?;
                let h = DeclHead::new(ctx, n.name(), n.loc());
                let np = match n.kind() {
                    DeclKind::TemplateTypeParm => self.builder().template_type_parm(
                        h,
                        gl.depth,
                        index,
                        pi.pack,
                        pi.typename_kw,
                    ),
                    DeclKind::NonTypeTemplateParm => {
                        let mut lv = levels.clone();
                        lv.push(self.ast.types.arg_list(&sargs), g, LevelMode::Substitute);
                        let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                        let ty = Substituter::new(self, &lv).ty(t, n.loc())?;
                        self.builder().non_type_template_parm(h, ty, gl.depth, index, pi.pack)
                    }
                    _ => return Err(SubstFail),
                };
                new_ps.push(np);
                moved.push((p, np));
                let a = self.template_param_arg(np);
                sargs[i] =
                    if pi.pack { TemplateArg::pack(self.ast.types.arg_list(&[a])) } else { a };
            }
            let sl = self.ast.types.arg_list(&sargs);
            levels.push(sl, g, LevelMode::Substitute);
            let mut s = Substituter::new(self, &levels);
            for &(p, np) in &moved {
                s.locals.insert(p, DeclOrPack::Decl(np));
                if let Some(c) = s.sema.ast.template_parm(p).and_then(|i| i.constraint) {
                    let c = s.expr(c)?;
                    s.sema.builder().set_parm_constraint(np, c);
                }
                if let Some(a) = s.sema.template_default(p) {
                    let a = s.template_arg(a, loc)?;
                    let _ = s.sema.builder().set_template_default(np, a);
                }
            }
            // The parameter types; a function parameter pack whose
            // template parameter pack was deduced as a list expands.
            let mut nptys = Vec::with_capacity(ptys.len());
            let mut aligned = true;
            for &t in &ptys {
                if let TypeView::PackExpansion { pattern, .. } = s.sema.ast.types.view(t.ty()) {
                    match s.expand_pattern_rewrap(None, |s| s.ty(pattern, loc))? {
                        Expansion::Unexpanded(x) => {
                            nptys.push(s.sema.ast.types.pack_expansion(x, None));
                        }
                        Expansion::Elements(es) => {
                            aligned &= es.len() == 1;
                            // An element that is an expansion (an alias
                            // template's pack `T...`) is substituted as its
                            // pattern and stays a function parameter pack:
                            // `const U&...` with `U = <int, T...>` is
                            // `const int&, const T&...` (clang
                            // `TransformFunctionTypeParams`).
                            let rw = s.take_rewrap();
                            for (k, e) in es.into_iter().enumerate() {
                                nptys.push(if rw.get(k) == Some(&true) {
                                    s.sema.ast.types.pack_expansion(e, None)
                                } else {
                                    s.sema.strip_expansion_ty(e)
                                });
                            }
                        }
                    }
                } else {
                    nptys.push(s.ty(t, loc)?);
                }
            }
            let nret = s.ty(ret, loc)?;
            let cc = if inherited {
                s.sema.inherited_guide_result(alias, rhs, nret, gl.depth, &new_ps)
            } else {
                None
            };
            let cparams: Vec<DeclId> = match s.sema.ast.decls.function(f) {
                Some(fd) if aligned => s.sema.ast.lists.iter::<DeclId>(fd.params).collect(),
                _ => Vec::new(),
            };
            let shape = GuideShape {
                ret: cc.unwrap_or(nret),
                cparams: &cparams,
                ptys: &nptys,
                variadic,
                explicit: s.sema.fn_spec(f).is_explicit(),
                depth: gl.depth + 1,
                implicit: s.sema.ast.decls.get(f).has(DeclFlags::IMPLICIT),
            };
            let nf = s.sema.guide_function(ctx, name, loc, &shape);
            let gps2: Vec<DeclId> = s
                .sema
                .ast
                .decls
                .function(nf)
                .map(|x| s.sema.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default();
            for (&o, nw) in cparams.iter().zip(gps2) {
                s.locals.insert(o, DeclOrPack::Decl(nw));
            }
            let trailing = s.sema.ast.templates.trailing_requires.get(f).copied();
            let req = ctad_guide_requires(&mut s, [gl.requires, trailing], loc)?;
            Ok::<_, SubstFail>((nf, req, cc.map(|_| nret)))
        })();
        self.ast.templates.dependent_depth = saved;
        let (nf, requires, source_ret) = built.ok()?;
        if let Some(r) = requires {
            let _ = self.ast.templates.trailing_requires.insert(nf, r);
        }
        let list = self.builder().template_param_list(
            al.template_loc,
            al.l_angle,
            al.r_angle,
            gl.depth,
            &new_ps,
            None,
        );
        let ng = self.builder().detached_template_decl(DeclKind::FunctionTemplate, nf, list);
        if let Some(&o) = self.templ.ctad_origin.get(&g) {
            let _ = self.templ.ctad_origin.insert(ng, o);
        }
        if source_ret.is_none() {
            let _ = self.templ.ctad_alias.insert(ng, (alias, rhs));
        }
        Some((ng, source_ret))
    }

    /// Which of the `n` template parameters of depth `depth` the deduced
    /// arguments `args` name (clang `MarkUsedTemplateParameters` without
    /// `OnlyDeduced`): each element of a deduced pack, or the pattern of
    /// an element that is an expansion (`T...` of `tuple<T..., U>`, whose
    /// list is otherwise a non-deduced context), probed by deducing it
    /// from itself.
    fn params_named(&mut self, args: &[TemplateArg], depth: u16, n: usize, loc: Loc) -> Vec<bool> {
        use crate::templ::deduce::{DeducedArgs, Tdk};
        let mut elems = Vec::new();
        for &a in args {
            match a.view() {
                ArgView::Null => {}
                ArgView::Pack(l) => elems.extend_from_slice(self.ast.types.args(l)),
                _ => elems.push(a),
            }
        }
        let mut used = vec![false; n];
        for e in elems {
            let mut probe = DeducedArgs::new(depth, n);
            probe.expansions = true;
            let e = match self.arg_expansion_pattern(e) {
                Some(p) => {
                    probe.pack_cur = Some(Vec::new());
                    p
                }
                None => e,
            };
            if self.deduce_args(&[e], &[e], true, &mut probe, loc) != Tdk::Success {
                return vec![true; n];
            }
            for (u, a) in used.iter_mut().zip(&probe.args) {
                *u |= !matches!(a.view(), ArgView::Null);
            }
            for (i, _) in probe.pack_cur.unwrap_or_default() {
                if let Some(u) = used.get_mut(usize::from(i)) {
                    *u = true;
                }
            }
        }
        used
    }

    /// `t` without the top-level sugar over a pack expansion (an alias
    /// template's pack substituted for a guide's): the expansion must be
    /// visible as one to expand it later.
    fn strip_expansion_ty(&self, t: QualType) -> QualType {
        let top = self.ast.types.desugar_top(t);
        match self.ast.types.view(top.ty()) {
            TypeView::PackExpansion { .. } => top,
            _ => t,
        }
    }

    /// [`Self::strip_expansion_ty`] for a type argument.
    fn strip_expansion_sugar(&self, a: TemplateArg) -> TemplateArg {
        match a.view() {
            ArgView::Type(t) => TemplateArg::ty(self.strip_expansion_ty(t)),
            _ => a,
        }
    }

    /// Where the guides of alias template `alias` are declared: clang
    /// puts them at the alias declaration (`using`).
    fn alias_guide_loc(&self, alias: DeclId) -> Loc {
        let pat = self.ast.template_info(alias).and_then(|i| i.pattern).unwrap_or(alias);
        self.templ.alias_using_loc.get(&pat).copied().unwrap_or(self.ast.decls.get(pat).loc())
    }

    /// Records the `using` keyword `loc` of alias declaration `d` when it
    /// is an alias template's pattern ([`Sema::alias_guide_loc`]).
    pub fn note_alias_decl_loc(&mut self, d: DeclId, loc: Loc) {
        let pat = match self.kind_of(d) {
            DeclKind::TypeAliasTemplate => self.ast.template_info(d).and_then(|i| i.pattern),
            DeclKind::TypeAlias if self.ast.described_template(d).is_some() => Some(d),
            _ => None,
        };
        if let Some(p) = pat {
            let _ = self.templ.alias_using_loc.insert(p, loc);
        }
    }

    fn user_guides(&mut self, tmpl: DeclId) -> Vec<DeclId> {
        let first = self.ast.first(tmpl);
        let ctx = self.ast.decls.get(first).ctx().unwrap_or(self.ast.tu);
        let name = self.ast.names.deduction_guide(first);
        self.ast.lookup_local(ctx, name).to_vec()
    }

    /// Class template argument deduction ([over.match.class.deduct]) for
    /// the placeholder `declared` initialized by `args` (`braced`: the
    /// braced-init-list ones) as `how`. The deduced type (keeping the
    /// placeholder as sugar), or `None` after a diagnostic at `loc`.
    pub(crate) fn deduce_ctad(
        &mut self,
        declared: QualType,
        args: &[ExprId],
        braced: &[Option<InitList>],
        how: CtadInit,
        loc: Loc,
    ) -> Option<QualType> {
        self.deduce_ctad_from(declared, args, braced, how, None, loc)
    }

    /// [`Sema::deduce_ctad`] with the initializer `init` the arguments
    /// come from, for the aggregate deduction candidate.
    fn deduce_ctad_from(
        &mut self,
        declared: QualType,
        args: &[ExprId],
        braced: &[Option<InitList>],
        how: CtadInit,
        init: Option<&Initializer>,
        loc: Loc,
    ) -> Option<QualType> {
        // The template name of the placeholder, for the note of an error
        // building the implicit guides: the type-specifier is the
        // variable's (`ctad_site`) or at `loc`.
        let site = self.templ.ctad_site.take().unwrap_or(loc);
        let name_loc = self.templ.ctad_name_locs.get(&site).copied().unwrap_or(loc);
        let (name, tmpl) = self.undeduced_ctad(declared)?;
        let with_quals = |s: &mut Self, t: QualType| {
            let d = s.ast.types.deduced_template_specialization(name, Some(t));
            QualType::new(d.ty(), d.quals_bits() | declared.quals_bits())
        };
        if self.ast.types.is_dependent(declared)
            || args.iter().zip(0..).any(|(&a, i)| {
                braced.get(i).and_then(Option::as_ref).is_none() && self.expr_is_type_dependent(a)
            })
            || braced.iter().flatten().any(|l| self.init_list_type_dependent(l))
        {
            return Some(with_quals(self, QualType::DEPENDENT));
        }
        // An argument already diagnosed (clang: an initializer that
        // contains errors deduces nothing, silently).
        if args
            .iter()
            .zip(0..)
            .any(|(&a, i)| braced.get(i).and_then(Option::as_ref).is_none() && self.is_err_expr(a))
        {
            return None;
        }
        let Some(guides) = self.ctad_guides(tmpl, init, 0, name_loc) else {
            // [dcl.type.class.deduct]/2: an alias template whose
            // defining-type-id is not a simple-template-id.
            let at = self.alias_guide_loc(tmpl);
            let note = self.diag(DiagId::note_template_decl_here, at);
            let n = self.decl_name_str(self.ast.decls.get(tmpl).name());
            let msg = self
                .diag(DiagId::sema_ctad_non_deducible_template, loc)
                .arg("alias template")
                .arg(format!("'{n}'"))
                .with_note(note);
            self.emit(msg);
            return None;
        };
        // [over.match.list]/1.1 (phase 1) with the guides as constructors:
        // the initializer-list guides with the list as their one argument,
        // except for a single element of a specialization of the template
        // (or a class derived from one).
        if let Some(Initializer::List(l)) = init
            && !self.ctad_list_of_spec(tmpl, l)
        {
            let list_guides: Vec<DeclId> = guides
                .iter()
                .copied()
                .filter(|&g| {
                    let p = match self.kind_of(g) {
                        DeclKind::FunctionTemplate => self.pattern_of(g),
                        _ => Some(g),
                    };
                    p.is_some_and(|p| self.is_init_list_guide(p))
                })
                .collect();
            if !list_guides.is_empty() {
                let mut set = self.begin_set(SetKind::Normal, loc);
                let base = self.ovl.lists.len();
                let idx = u32::try_from(base).unwrap_or(u32::MAX);
                self.ovl.lists.push(l.clone());
                set.args.push(crate::overload::ArgInfo {
                    list: Some(idx),
                    ..crate::overload::ArgInfo::of_type(
                        QualType::VOID,
                        kiln_ast::expr::ValueCategory::PRValue,
                    )
                });
                let out = self.ctad_resolve(&mut set, &list_guides, how, loc);
                let result = if matches!(out, OvlOutcome::NoViable) {
                    None
                } else {
                    Some(self.ctad_outcome(&set, out, declared, name, how, loc))
                };
                self.ovl.lists.truncate(base);
                self.end_set(set);
                if let Some(r) = result {
                    return r;
                }
            }
        }
        let mut set = self.begin_set(SetKind::Normal, loc);
        let base = self.push_call_args(&mut set, args, braced);
        let out = self.ctad_resolve(&mut set, &guides, how, loc);
        let result = self.ctad_outcome(&set, out, declared, name, how, loc);
        self.ovl.lists.truncate(base);
        self.end_set(set);
        result
    }

    /// Overload resolution over the guides `guides` for initialization
    /// `how` with the arguments already in `set`. At clang 21 and later
    /// the template guides are deferred candidates (`overload/defer.rs`):
    /// a non-template guide that is a perfect match is the result without
    /// deducing any template guide, which so names no specialization.
    fn ctad_resolve(
        &mut self,
        set: &mut OverloadCandidateSet,
        guides: &[DeclId],
        how: CtadInit,
        loc: Loc,
    ) -> OvlOutcome {
        self.defer_templates(set);
        self.add_ctad_candidates(set, guides, how, loc);
        self.resolve_set(set)
    }

    /// The guides `guides` as candidates of `set` for initialization
    /// `how` (an explicit guide is not one of copy-initialization); the
    /// template guides deferred in a two-phase set.
    fn add_ctad_candidates(
        &mut self,
        set: &mut OverloadCandidateSet,
        guides: &[DeclId],
        how: CtadInit,
        loc: Loc,
    ) {
        for &g in guides {
            if set.two_phase && self.kind_of(g) == DeclKind::FunctionTemplate {
                set.deferred.push(crate::overload::defer::Deferred::Guide { g, how, loc });
                continue;
            }
            self.add_ctad_candidate(set, g, how, loc);
        }
    }

    /// Guide `g` as a candidate of `set` for initialization `how`.
    pub(crate) fn add_ctad_candidate(
        &mut self,
        set: &mut OverloadCandidateSet,
        g: DeclId,
        how: CtadInit,
        loc: Loc,
    ) {
        let (pattern, template) = match self.kind_of(g) {
            DeclKind::FunctionTemplate => match self.pattern_of(g) {
                Some(p) => (p, Some(g)),
                None => return,
            },
            DeclKind::CXXDeductionGuide => (g, None),
            _ => return,
        };
        if self.ast.decls.get(pattern).has(DeclFlags::INVALID) {
            return;
        }
        if how == CtadInit::Copy && self.fn_spec(pattern).is_explicit() {
            self.add_explicit_guide(set, pattern, g, template);
            return;
        }
        match template {
            Some(ft) => self.add_template_candidate(
                set,
                ft,
                g,
                None,
                CandidateKind::Function,
                IcsOptions::NONE,
                loc,
            ),
            None => {
                self.add_candidate(set, pattern, g, CandidateKind::Function, IcsOptions::NONE);
            }
        }
    }

    /// The deduced type of overload resolution `out` over the guides in
    /// `set` for the placeholder `declared` (template name `name`), or
    /// `None` after a diagnostic at `loc`.
    fn ctad_outcome(
        &mut self,
        set: &OverloadCandidateSet,
        out: OvlOutcome,
        declared: QualType,
        name: TemplateNameId,
        how: CtadInit,
        loc: Loc,
    ) -> Option<QualType> {
        let with_quals = |s: &mut Self, t: QualType| {
            let d = s.ast.types.deduced_template_specialization(name, Some(t));
            QualType::new(d.ty(), d.quals_bits() | declared.quals_bits())
        };
        let shown = self.ty_str(declared.without_quals());
        match out {
            OvlOutcome::Success(b) | OvlOutcome::Deleted(b) => {
                let f = set.cand(b).func();
                let pattern = set.cand(b).template.and_then(|t| self.pattern_of(t)).unwrap_or(f);
                if how == CtadInit::CopyList && self.fn_spec(pattern).is_explicit() {
                    let implicit = self.ast.decls.get(pattern).has(DeclFlags::IMPLICIT);
                    let which = u8::from(!implicit);
                    let at = self.ast.decls.get(pattern).loc();
                    let note = self.diag(DiagId::note_ctad_explicit_here, at).arg(which);
                    let msg = self
                        .diag(DiagId::sema_ctad_explicit_copy_list, loc)
                        .arg(shown)
                        .arg(which)
                        .with_note(note);
                    self.emit(msg);
                    None
                } else {
                    // clang `MarkFunctionReferenced` on the selected guide.
                    self.mark_used(f);
                    let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                    let ret = self.ast.types.proto_of(ty.ty()).or_else(|| self.ast.types.proto(ty));
                    let ret = ret.map_or(QualType::ERROR, FuncProto::ret);
                    // A guide from an inherited constructor returns the
                    // base's specialization; map it to the derived class.
                    let g = set.cand(b).template.unwrap_or(f);
                    let mut ret = self.ctad_result_type(g, ret);
                    // An implicit guide returns the injected-class-name
                    // specialization, which substitutes to the class
                    // itself (clang's `RecordType`), not a template-id.
                    if self.ast.decls.get(pattern).has(DeclFlags::IMPLICIT)
                        && !self.ast.types.is_dependent(ret)
                    {
                        ret = self.ast.types.canonical(ret);
                    }
                    Some(with_quals(self, ret))
                }
            }
            OvlOutcome::NoViable => {
                let msg = self.diag(DiagId::sema_ctad_no_viable, loc).arg(shown);
                let msg = self.with_ovl_notes(msg, set, false);
                self.emit(msg);
                None
            }
            OvlOutcome::Ambiguous => {
                let msg = self.diag(DiagId::sema_ctad_ambiguous, loc).arg(shown);
                let msg = self.with_ovl_notes(msg, set, true);
                self.emit(msg);
                None
            }
        }
    }

    /// Whether guide `f` is an initializer-list "constructor"
    /// ([dcl.init.list]/2): its first parameter is (a reference to cv)
    /// `std::initializer_list<E>`, dependent or not, and any others have
    /// default arguments.
    fn is_init_list_guide(&mut self, f: DeclId) -> bool {
        let params = self.param_decls(f);
        let Some(&p) = params.first() else { return false };
        if params[1..].iter().any(|&q| !self.param_has_default(q)) {
            return false;
        }
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let t = self.non_reference(pty);
        self.initializer_list_param(t).is_some()
    }

    /// Whether braced list `l` is a single expression whose type is a
    /// specialization of class template `tmpl` or a class derived from
    /// one ([over.match.class.deduct]/4: initializer-list guides are then
    /// not considered first).
    fn ctad_list_of_spec(&mut self, tmpl: DeclId, l: &InitList) -> bool {
        let [item] = l.items.as_slice() else { return false };
        let InitValue::Expr(e) = item.value else { return false };
        let ty = self.ast.exprs.get(e).ty();
        let Some(rd) = self.ast.types.as_record(ty) else { return false };
        let first = self.ast.first(self.inst_template(tmpl));
        let mut work = vec![rd];
        let mut seen = Vec::new();
        while let Some(c) = work.pop() {
            let c = self.ast.first(c);
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            if let Some((_, info)) = self.ast.spec_info(c)
                && self.ast.first(self.inst_template(info.template)) == first
            {
                return true;
            }
            let Some(def) = self.ast.definition(c) else { continue };
            let bases: Vec<QualType> =
                self.ast.decls.bases(def, &self.ast.lists).map(|b| b.ty).collect();
            for b in bases {
                if let Some(r) = self.ast.types.as_record(b) {
                    work.push(r);
                }
            }
        }
        false
    }

    /// Explicit guide `g` (pattern `pattern`) as a non-viable candidate
    /// of copy-initialization ([over.match.class.deduct]/3.1).
    fn add_explicit_guide(
        &mut self,
        set: &mut OverloadCandidateSet,
        pattern: DeclId,
        g: DeclId,
        template: Option<DeclId>,
    ) {
        self.ovl.stats.candidates += 1;
        let mut c = Candidate::new(pattern, g, CandidateKind::Function);
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        c.ics = start..start;
        c.template = template;
        c.viable = false;
        c.failure = CandidateFailure::Explicit;
        set.cands.push(c);
    }

    /// For a candidate function (or template) `f` that is a deduction
    /// guide, what it was formed from: `Some(None)` for a user-declared
    /// guide, `Some(Some(origin))` for an implicit one
    /// ([over.match.best.general]/2.11-2.13).
    #[allow(clippy::option_option, reason = "not a guide / user-declared / implicit origin")]
    pub(crate) fn ctad_guide_kind(
        &self,
        f: DeclId,
        template: Option<DeclId>,
    ) -> Option<Option<GuideOrigin>> {
        let g = template.unwrap_or(f);
        let pattern = template.and_then(|t| self.pattern_of(t)).unwrap_or(f);
        if self.kind_of(pattern) != DeclKind::CXXDeductionGuide
            && self.kind_of(f) != DeclKind::CXXDeductionGuide
        {
            return None;
        }
        if !self.ast.decls.get(pattern).has(DeclFlags::IMPLICIT) {
            return Some(None);
        }
        Some(Some(self.templ.ctad_origin.get(&g).copied().unwrap_or(GuideOrigin::Ctor)))
    }

    /// clang's `implicit deduction guide declared as '...'` text for the
    /// implicit guide template `g`.
    #[allow(clippy::too_many_lines, reason = "clang DeclPrinter order")]
    pub(crate) fn implicit_guide_signature(&self, g: DeclId) -> Option<String> {
        let pattern = self.pattern_of(g)?;
        let info = self.ast.template_info(g)?;
        let list = *self.ast.templates.param_list(info.params);
        let mut out = String::from("template <");
        for (i, p) in self.ast.lists.iter::<DeclId>(list.params).enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let pi = self.ast.template_parm(p);
            let pack = pi.is_some_and(|x| x.pack);
            let pname = self.decl_name_str(self.ast.decls.get(p).name());
            match self.kind_of(p) {
                DeclKind::TemplateTypeParm => {
                    // clang prints a re-depthed type-constraint with its
                    // written arguments substituted, `<>` when there are
                    // none (`One<> R`, `Compat<T> R`). The concept's
                    // written qualifier is not kept on the expression.
                    let concept = pi.and_then(|x| x.constraint).and_then(|c| {
                        let mut n = self.ast.exprs.get(c);
                        // A constrained pack's constraint is the fold
                        // `(C<Q> && ...)`.
                        if n.kind() == kiln_ast::expr::ExprKind::CXXFoldExpr {
                            n = self
                                .ast
                                .exprs
                                .get(<ExprId as kiln_ast::ids::RawId>::from_raw(n.a())?);
                        }
                        (n.kind() == kiln_ast::expr::ExprKind::ConceptSpecializationExpr)
                            .then(|| {
                                let d = <DeclId as kiln_ast::ids::RawId>::from_raw(n.a())?;
                                Some((d, <ArgListId as kiln_ast::ids::RawId>::from_raw(n.b())))
                            })
                            .flatten()
                    });
                    if let Some((c, args)) = concept {
                        out.push_str(&self.decl_name_str(self.ast.decls.get(c).name()));
                        out.push('<');
                        let mut written = Vec::new();
                        for &a in args.map_or(&[][..], |l| self.ast.types.args(l)).iter().skip(1) {
                            match a.view() {
                                ArgView::Pack(l) => {
                                    written.extend(self.ast.types.args(l).iter().copied());
                                }
                                _ => written.push(a),
                            }
                        }
                        for (k, &a) in written.iter().enumerate() {
                            if k > 0 {
                                out.push_str(", ");
                            }
                            out.push_str(&self.arg_str(a));
                        }
                        out.push('>');
                    } else {
                        out.push_str(if pi.is_some_and(|x| x.typename_kw) {
                            "typename"
                        } else {
                            "class"
                        });
                    }
                }
                DeclKind::NonTypeTemplateParm => {
                    let mut t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(t.ty()) {
                        t = pattern;
                    }
                    out.push_str(&self.ty_str(t));
                }
                _ => out.push_str("template <...> class"),
            }
            // clang spells a pack `class ...Ts`.
            if pack {
                out.push_str(" ...");
                out.push_str(&pname);
            } else if !pname.is_empty() {
                out.push(' ');
                out.push_str(&pname);
            }
            let shown = if self.clang_compat >= 22 {
                self.ast.templates.defaults.get(p).copied()
            } else {
                self.template_default(p)
            };
            if let Some(a) = shown {
                out.push_str(" = ");
                out.push_str(&self.arg_str(a));
            }
        }
        out.push_str("> ");
        let deducible = match self.templ.ctad_alias.get(&g) {
            // clang names the guide's return type, which is the alias's
            // defining-type-id only when all of it was deduced.
            Some(&(alias, _)) => {
                let ty = self.ast.decls.get(pattern).ty().unwrap_or(QualType::ERROR);
                Some((
                    alias,
                    self.ast.types.proto_of(ty.ty()).map_or(QualType::ERROR, FuncProto::ret),
                ))
            }
            // A guide from an inherited constructor returns `CC<R>::type`
            // (§6.6.1); its constraint names `R`.
            None => self.templ.ctad_inherited.get(&g).and_then(|i| Some((i.class, i.source_ret?))),
        };
        if let Some((alias, ret)) = deducible {
            let n = self.decl_name_str(self.ast.decls.get(alias).name());
            let r = self.ty_str(ret);
            let _ = std::fmt::Write::write_fmt(
                &mut out,
                format_args!("requires __is_deducible({n}, {r}) "),
            );
        }
        if self.fn_spec(pattern).is_explicit() {
            out.push_str("explicit ");
        }
        let class = self.ast.decls.get(g).name();
        let class = match self.ast.names.view(class) {
            kiln_ast::names::DeclNameView::DeductionGuide(t) => self.ast.decls.get(t).name(),
            _ => class,
        };
        out.push_str(&self.decl_name_str(class));
        out.push('(');
        let ty = self.ast.decls.get(pattern).ty().unwrap_or(QualType::ERROR);
        let fd = self.ast.decls.function(pattern)?;
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(fd.params).collect();
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            // clang's `DeclPrinter`: the name (after `...` for a pack) is
            // the declarator's placeholder, `Ts ...xs`, `const Ts &...`.
            let mut t = self.ast.decls.get(*p).ty().unwrap_or(QualType::ERROR);
            let mut ph = String::new();
            if let TypeView::PackExpansion { pattern, .. } = self.ast.types.view(t.ty()) {
                t = pattern;
                ph.push_str("...");
            }
            ph.push_str(&self.decl_name_str(self.ast.decls.get(*p).name()));
            let names = crate::names::sema_names(self);
            kiln_ast::types::write_declarator(
                &self.ast.types,
                &names,
                t,
                kiln_ast::types::PrintOptions::SUGARED,
                &ph,
                &mut out,
            );
            // clang's placeholder for an inherited default argument.
            if self.ast.decls.param(*p).is_some_and(|d| {
                d.default_arg.is_some() || d.flags & ParamData::DEFAULT_ARG_PENDING != 0
            }) {
                out.push_str(" = <null expr>");
            }
        }
        if self.ast.types.proto(ty).is_some_and(FuncProto::variadic) {
            out.push_str(if params.is_empty() { "..." } else { ", ..." });
        }
        out.push_str(") -> ");
        let ret = self.ast.types.proto_of(ty.ty()).map_or(QualType::ERROR, FuncProto::ret);
        out.push_str(&self.ty_str(ret));
        Some(out)
    }

    /// The arguments of `init` for class template argument deduction:
    /// the expressions, and the braced-init-list ones (a nested list is
    /// a `void` placeholder).
    fn ctad_args(&mut self, init: &Initializer) -> (Vec<ExprId>, Vec<Option<InitList>>) {
        let peek = |s: &Self, e: ExprId| {
            s.braced_args.iter().find(|(x, _)| *x == e).map(|(_, l)| l.clone())
        };
        match init {
            Initializer::Expr(e) => (vec![*e], vec![None]),
            Initializer::Paren(es, ..) => {
                let braced = es.iter().map(|&e| peek(self, e)).collect();
                (es.clone(), braced)
            }
            Initializer::List(l) => {
                let mut args = Vec::with_capacity(l.items.len());
                let mut braced = Vec::with_capacity(l.items.len());
                for it in &l.items {
                    match &it.value {
                        InitValue::Expr(e) => {
                            braced.push(peek(self, *e));
                            args.push(*e);
                        }
                        InitValue::List(n) => {
                            let flat = Self::list_exprs(n);
                            let e = self.builder().init_list(
                                QualType::VOID,
                                &flat,
                                None,
                                None,
                                n.lbrace,
                                n.rbrace,
                            );
                            args.push(e);
                            braced.push(Some(n.clone()));
                        }
                    }
                }
                (args, braced)
            }
        }
    }

    /// [dcl.type.class.deduct]/1: the type of variable (or new-expression
    /// object) `declared` with initializer `init` (`direct`: a
    /// direct-initialization list), or `None` after a diagnostic at `loc`.
    pub(crate) fn ctad_for_initializer(
        &mut self,
        declared: QualType,
        init: &Initializer,
        direct: bool,
        loc: Loc,
    ) -> Option<QualType> {
        let how = match init {
            Initializer::Expr(_) => CtadInit::Copy,
            Initializer::Paren(..) => CtadInit::Direct,
            Initializer::List(_) if direct => CtadInit::DirectList,
            Initializer::List(_) => CtadInit::CopyList,
        };
        let (args, braced) = self.ctad_args(init);
        self.deduce_ctad_from(declared, &args, &braced, how, Some(init), loc)
    }

    /// `C x;` ([over.match.class.deduct], no arguments): the type of
    /// variable `v` declared as `ty`, deduced if a placeholder; `None`
    /// after a diagnostic, with `v` invalid.
    pub(crate) fn ctad_uninit(&mut self, v: DeclId, ty: QualType, loc: Loc) -> Option<QualType> {
        if self.undeduced_ctad(ty).is_none() {
            return Some(ty);
        }
        // Only an initializing declaration deduces without an initializer:
        // not an `extern` variable or a static data member (clang's
        // `err_auto_var_requires_init` from `deduceVarTypeFromInitializer`).
        let ext = self
            .ast
            .decls
            .var(v)
            .is_some_and(|x| x.spec.storage() == kiln_ast::decl::VarStorage::Extern);
        let member = matches!(
            self.ast.decls.get(v).ctx().map(|c| self.kind_of(c)),
            Some(DeclKind::CXXRecord)
        );
        if ext || member {
            let name = self.decl_name(v);
            let msg =
                self.diag(DiagId::sema_auto_var_requires_init, loc).arg(name).arg(self.ty_arg(ty));
            self.emit(msg);
            self.ast.decls.set_ty(v, QualType::ERROR);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return None;
        }
        self.templ.ctad_site = self.templ.ctad_var_type_locs.get(&v).copied();
        let t = self.deduce_ctad(ty, &[], &[], CtadInit::Direct, loc).unwrap_or(QualType::ERROR);
        self.ast.decls.set_ty(v, t);
        if self.is_error_type(t) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return None;
        }
        Some(t)
    }

    /// Deduces variable `v`'s placeholder type `ty` from `init`, the
    /// diagnostics citing the placeholder as `v`'s declaration wrote it
    /// (`ctad_var_type_locs`); the error type after a diagnostic.
    pub(crate) fn ctad_for_var(
        &mut self,
        v: DeclId,
        ty: QualType,
        init: &Initializer,
        direct: bool,
        loc: Loc,
    ) -> QualType {
        self.templ.ctad_site = self.templ.ctad_var_type_locs.get(&v).copied();
        self.ctad_for_initializer(ty, init, direct, loc).unwrap_or(QualType::ERROR)
    }

    /// Whether `ty` is a class template placeholder to deduce.
    pub fn is_ctad_placeholder(&self, ty: QualType) -> bool {
        self.undeduced_ctad(ty).is_some()
    }

    /// `C(args)` ([expr.type.conv]/1): the deduced type, or the error
    /// type after a diagnostic at `loc`; any other `ty` unchanged.
    pub(crate) fn ctad_cast_type(&mut self, ty: QualType, args: &[ExprId], loc: Loc) -> QualType {
        if !self.is_ctad_placeholder(ty) {
            return ty;
        }
        let init = Initializer::Paren(args.to_vec(), loc, loc);
        self.ctad_for_initializer(ty, &init, false, loc).unwrap_or(QualType::ERROR)
    }

    /// `C{args}` ([expr.type.conv]/1).
    pub(crate) fn act_on_ctad_list_cast_type(
        &mut self,
        ty: QualType,
        l: &InitList,
        loc: Loc,
    ) -> QualType {
        let init = Initializer::List(l.clone());
        self.ctad_for_initializer(ty, &init, true, loc).unwrap_or(QualType::ERROR)
    }
}
