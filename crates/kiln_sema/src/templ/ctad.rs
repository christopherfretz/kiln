//! Class template argument deduction ([over.match.class.deduct]) and
//! deduction-guide declarations ([temp.deduct.guide]); spec/sema.md
//! "Class template argument deduction".

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, FnSpec, ParamData, TagKind};
use kiln_ast::expr::BinaryOp;
use kiln_ast::ids::{ArgListId, TemplateNameId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::types::{
    AutoKeyword, ExcSpec, FuncInfo, FuncProto, Quals, TemplateArg, TemplateName, TypeView,
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
use crate::templ::pack::Expansion;
use crate::templ::subst::{DeclOrPack, SubstFail, SubstResult, Substituter};

/// How the object whose type is a deduced class template specialization
/// is initialized ([over.match.class.deduct]/1, /3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// The signature of an implicit guide being formed.
struct GuideShape<'p> {
    ret: QualType,
    cparams: &'p [DeclId],
    ptys: &'p [QualType],
    variadic: bool,
    explicit: bool,
    /// The dependent depth of the guide's declaration.
    depth: u16,
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
            // re-depthed template parameter pack).
            match s.expand_pattern(|s| s.ty(pattern, loc))? {
                Expansion::Unexpanded(x) => s.sema.ast.types.pack_expansion(x, None),
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
            s.sema.builder().template_template_parm(h, inner, depth, index, pi.pack, pi.typename_kw)
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
        let _ = s.sema.builder().set_template_default(np, a);
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
    /// ([dcl.type.class.deduct]/1): a class template or (P1814) an
    /// alias template.
    pub fn is_ctad_template(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::ClassTemplate => !self.names_injected_class(d),
            DeclKind::TypeAliasTemplate => true,
            _ => false,
        }
    }

    /// True if a class or alias template name `d` alone is a type: its
    /// injected-class-name or a CTAD placeholder.
    pub fn names_class_type(&self, d: DeclId) -> bool {
        matches!(self.kind_of(d), DeclKind::ClassTemplate | DeclKind::TypeAliasTemplate)
    }

    /// The undeduced placeholder `X` / `nns::X` for class template `d`
    /// ([dcl.type.class.deduct]).
    pub fn ctad_placeholder(&mut self, d: DeclId, nns: Option<NnsId>) -> QualType {
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
        let (defaults, _) = self.merge_default_args(None, false, &params);
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
        }
        Some(f)
    }

    /// The class template `ty` is an undeduced placeholder for
    /// ([dcl.type.class.deduct]), with its template name as written.
    pub(crate) fn undeduced_ctad(&self, ty: QualType) -> Option<(TemplateNameId, DeclId)> {
        let TypeView::DeducedTemplateSpecialization { name, deduced: None } =
            self.ast.types.view(ty.ty())
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
    /// template decls that join no member list or lookup table.
    fn implicit_guides(&mut self, tmpl: DeclId) -> Vec<DeclId> {
        let first = self.ast.first(tmpl);
        if let Some(v) = self.templ.ctad_guides.get(&first) {
            return v.clone();
        }
        let Some(pat) = self.pattern_of(tmpl) else { return Vec::new() };
        let def = self.ast.definition(pat);
        // The template decl whose parameters the definition uses.
        let owner = def.and_then(|d| self.ast.described_template(d)).unwrap_or(tmpl);
        let Some(info) = self.ast.template_info(owner).copied() else { return Vec::new() };
        let Some(TypeView::InjectedClassName { injected, .. }) =
            self.injected_type_of(owner).map(|q| self.ast.types.view(q.ty()))
        else {
            return Vec::new();
        };
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
                        let g = self.ctor_template_guide(ctx, name, injected, info.params, m);
                        out.extend(g);
                    }
                    _ => {}
                }
            }
        }
        if !any_ctor {
            let loc = self.ast.decls.get(first).loc();
            out.extend(self.implicit_guide(ctx, name, injected, info.params, None, loc));
        }
        // [over.match.class.deduct]/1.3: `C(C<T...>) -> C<T...>`.
        let loc = self.ast.decls.get(owner).loc();
        let g = self.copy_guide(ctx, name, injected, info.params, loc);
        let _ = self.templ.ctad_origin.insert(g, GuideOrigin::Copy);
        out.push(g);
        self.ast.templates.dependent_depth = saved;
        if def.is_some() {
            let _ = self.templ.ctad_guides.insert(first, out.clone());
        }
        out
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
        let shape = GuideShape { ret, cparams: &cparams, ptys: &ptys, variadic, explicit, depth };
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
    fn ctor_template_guide(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        ret: QualType,
        class_params: TemplateParamListId,
        ft: DeclId,
    ) -> Option<DeclId> {
        let ctor = self.pattern_of(ft)?;
        let finfo = self.ast.template_info(ft).copied()?;
        let cl = *self.ast.templates.param_list(class_params);
        let fl = *self.ast.templates.param_list(finfo.params);
        if fl.depth != cl.depth + 1 {
            return None;
        }
        let class_ps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(cl.params).collect();
        let n = u16::try_from(class_ps.len()).ok()?;
        // The outer levels and the constructor template's stay; the class
        // template's level is "substituted" by undeduced (null) arguments,
        // which keep its parameters, so the constructor template's level
        // becomes the class template's ([`MultiLevelArgs::retained_depth`]).
        let mut levels = MultiLevelArgs::new();
        for _ in 0..cl.depth {
            levels.push(ArgListId::EMPTY, ft, LevelMode::Retain);
        }
        let nulls = vec![TemplateArg::NULL; class_ps.len()];
        let nulls = self.ast.types.arg_list(&nulls);
        levels.push(nulls, ft, LevelMode::Substitute);
        levels.push(ArgListId::EMPTY, ft, LevelMode::Retain);
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
        h.flags = DeclFlags::IMPLICIT;
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
            match data {
                Some(d) if d.default_arg.is_some() => {
                    if let Some(e) = d.default_arg {
                        let _ = self.ast.decls.set_default_arg(np, e);
                    }
                }
                Some(d) if d.flags & ParamData::DEFAULT_ARG_PENDING != 0 => {
                    self.ast.decls.mark_default_arg_pending(np);
                }
                _ => {}
            }
            ids.push(np);
        }
        let _ = self.builder().set_params(f, &ids);
        self.ast.templates.dependent_depth = saved;
        f
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
            self.injected_type_of(owner).map(|q| self.ast.types.view(q.ty()))
        else {
            return None;
        };
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
    ) -> Option<Vec<DeclId>> {
        if self.kind_of(tmpl) == DeclKind::TypeAliasTemplate {
            let (target, rhs) = self.alias_ctad_target(tmpl)?;
            if level > 64 {
                return Some(Vec::new());
            }
            let inner = self.ctad_guides(target, init, level + 1)?;
            return Some(
                inner.into_iter().filter_map(|g| self.alias_guide(tmpl, rhs, g)).collect(),
            );
        }
        let mut guides = self.implicit_guides(tmpl);
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
        if self.deduce(rhs, ty, TdfFlags::NONE, &mut d) != Tdk::Success {
            return false;
        }
        d.args.iter().zip(&params).all(|(a, &p)| {
            !matches!(a.view(), kiln_ast::types::ArgView::Null)
                || self.ast.template_parm(p).is_some_and(|i| i.pack)
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
            self.build_alias_guide(alias, rhs, g)
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
    fn build_alias_guide(&mut self, alias: DeclId, rhs: QualType, g: DeclId) -> Option<DeclId> {
        use crate::templ::deduce::{DeducedArgs, TdfFlags, Tdk};
        let f = self.pattern_of(g)?;
        let gl = *self.ast.templates.param_list(self.ast.template_info(g)?.params);
        let al = *self.ast.templates.param_list(self.ast.template_info(alias)?.params);
        if gl.depth != al.depth {
            return None;
        }
        let fty = self.ast.decls.get(f).ty()?;
        let proto = self.ast.types.proto(fty)?;
        let ret = proto.ret();
        let ptys: Vec<QualType> = self.ast.types.params(&proto).to_vec();
        let variadic = proto.variadic();
        let gps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(gl.params).collect();
        let mut d = DeducedArgs::new(gl.depth, gps.len());
        d.expansions = true;
        if self.deduce(ret, rhs, TdfFlags::NONE, &mut d) != Tdk::Success {
            return None;
        }
        let first = self.ast.first(alias);
        let ctx = self.ast.decls.get(first).ctx().unwrap_or(self.ast.tu);
        // The alias template's parameters the deductions name (clang
        // `MarkUsedTemplateParameters`); found as those deducible from
        // the deductions themselves.
        let aps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(al.params).collect();
        let deduced: Vec<TemplateArg> = d
            .args
            .iter()
            .copied()
            .filter(|a| !matches!(a.view(), kiln_ast::types::ArgView::Null))
            .collect();
        let mut du = DeducedArgs::new(al.depth, aps.len());
        du.expansions = true;
        let used: Vec<bool> = if self.deduce_args(&deduced, &deduced, true, &mut du, Loc::INVALID)
            == Tdk::Success
        {
            du.args.iter().map(|a| !matches!(a.view(), kiln_ast::types::ArgView::Null)).collect()
        } else {
            vec![true; aps.len()]
        };
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
                kiln_ast::types::ArgView::Null => continue,
                // A deduced pack (its elements may be expansions).
                kiln_ast::types::ArgView::Pack(l) => {
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
                kiln_ast::types::ArgView::Profile(p) => {
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
                if !matches!(sargs[i].view(), kiln_ast::types::ArgView::Null) {
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
                    match s.expand_pattern(|s| s.ty(pattern, loc))? {
                        Expansion::Unexpanded(x) => {
                            nptys.push(s.sema.ast.types.pack_expansion(x, None));
                        }
                        Expansion::Elements(es) => {
                            aligned &= es.len() == 1;
                            // An element that is an expansion (an alias
                            // template's pack) stays a function parameter
                            // pack: drop the substitution sugar over it.
                            nptys.extend(es.into_iter().map(|e| s.sema.strip_expansion_ty(e)));
                        }
                    }
                } else {
                    nptys.push(s.ty(t, loc)?);
                }
            }
            let nret = s.ty(ret, loc)?;
            let cparams: Vec<DeclId> = match s.sema.ast.decls.function(f) {
                Some(fd) if aligned => s.sema.ast.lists.iter::<DeclId>(fd.params).collect(),
                _ => Vec::new(),
            };
            let shape = GuideShape {
                ret: nret,
                cparams: &cparams,
                ptys: &nptys,
                variadic,
                explicit: s.sema.fn_spec(f).is_explicit(),
                depth: gl.depth + 1,
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
            Ok::<_, SubstFail>((nf, req))
        })();
        self.ast.templates.dependent_depth = saved;
        let (nf, requires) = built.ok()?;
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
        let _ = self.templ.ctad_alias.insert(ng, (alias, rhs));
        Some(ng)
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
            kiln_ast::types::ArgView::Type(t) => TemplateArg::ty(self.strip_expansion_ty(t)),
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
        let Some(guides) = self.ctad_guides(tmpl, init, 0) else {
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
        let mut set = self.begin_set(SetKind::Normal, loc);
        let base = self.push_call_args(&mut set, args, braced);
        for g in guides {
            let (pattern, template) = match self.kind_of(g) {
                DeclKind::FunctionTemplate => match self.pattern_of(g) {
                    Some(p) => (p, Some(g)),
                    None => continue,
                },
                DeclKind::CXXDeductionGuide => (g, None),
                _ => continue,
            };
            if self.ast.decls.get(pattern).has(DeclFlags::INVALID) {
                continue;
            }
            if how == CtadInit::Copy && self.fn_spec(pattern).is_explicit() {
                self.add_explicit_guide(&mut set, pattern, g, template);
                continue;
            }
            match template {
                Some(ft) => self.add_template_candidate(
                    &mut set,
                    ft,
                    g,
                    None,
                    CandidateKind::Function,
                    IcsOptions::NONE,
                    loc,
                ),
                None => self.add_candidate(
                    &mut set,
                    pattern,
                    g,
                    CandidateKind::Function,
                    IcsOptions::NONE,
                ),
            }
        }
        self.order_template_candidates(&mut set);
        let out = self.best_viable(&mut set);
        let shown = self.ty_str(declared.without_quals());
        let result = match out {
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
                    Some(with_quals(self, ret))
                }
            }
            OvlOutcome::NoViable => {
                let msg = self.diag(DiagId::sema_ctad_no_viable, loc).arg(shown);
                let msg = self.with_ovl_notes(msg, &set, false);
                self.emit(msg);
                None
            }
            OvlOutcome::Ambiguous => {
                let msg = self.diag(DiagId::sema_ctad_ambiguous, loc).arg(shown);
                let msg = self.with_ovl_notes(msg, &set, true);
                self.emit(msg);
                None
            }
        };
        self.ovl.lists.truncate(base);
        self.end_set(set);
        result
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
                    // clang prints a re-depthed type-constraint `C<>`.
                    let concept = pi.and_then(|x| x.constraint).and_then(|c| {
                        let n = self.ast.exprs.get(c);
                        (n.kind() == kiln_ast::expr::ExprKind::ConceptSpecializationExpr)
                            .then(|| <DeclId as kiln_ast::ids::RawId>::from_raw(n.a()))
                            .flatten()
                    });
                    if let Some(c) = concept {
                        out.push_str(&self.decl_name_str(self.ast.decls.get(c).name()));
                        out.push_str("<>");
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
            if let Some(a) = self.template_default(p) {
                out.push_str(" = ");
                out.push_str(&self.arg_str(a));
            }
        }
        out.push_str("> ");
        if let Some(&(alias, rhs)) = self.templ.ctad_alias.get(&g) {
            let n = self.decl_name_str(self.ast.decls.get(alias).name());
            out.push_str(&format!("requires __is_deducible({n}, {}) ", self.ty_str(rhs)));
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
            let names = crate::names::SemaNames::new(self);
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
        let t = self.deduce_ctad(ty, &[], &[], CtadInit::Direct, loc).unwrap_or(QualType::ERROR);
        self.ast.decls.set_ty(v, t);
        if self.is_error_type(t) {
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return None;
        }
        Some(t)
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
