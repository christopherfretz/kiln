//! Class template argument deduction ([over.match.class.deduct]) and
//! deduction-guide declarations ([temp.deduct.guide]); spec/sema.md
//! "Class template argument deduction".

use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec, ParamData};
use kiln_ast::ids::{TemplateNameId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::types::{AutoKeyword, ExcSpec, FuncInfo, TemplateName, TypeView};
use kiln_ast::{DeclHead, DeclId, ExprId, NnsId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{Chunk, DeclSpec, Declarator};
use crate::init::{InitList, InitValue, Initializer};
use crate::overload::{
    Candidate, CandidateFailure, CandidateKind, IcsOptions, OvlOutcome, SetKind,
};

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

impl Sema<'_, '_> {
    /// The undeduced placeholder `auto`: the declared return type of a
    /// deduction guide before its trailing return type replaces it.
    pub fn plain_auto_type(&mut self) -> QualType {
        self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false)
    }

    /// True if `d`, named without template arguments outside its own
    /// definition, is a placeholder for a deduced class type
    /// ([dcl.type.class.deduct]/1): a class template. Alias templates
    /// (P1814) are not deduced yet.
    pub fn is_ctad_template(&self, d: DeclId) -> bool {
        self.kind_of(d) == DeclKind::ClassTemplate && !self.names_injected_class(d)
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

    /// Deduces the template arguments of a class template from an
    /// initializer (WP-071b): reports and fails.

    /// The class template `ty` is an undeduced placeholder for
    /// ([dcl.type.class.deduct]), with its template name as written.
    pub(crate) fn undeduced_ctad(&self, ty: QualType) -> Option<(TemplateNameId, DeclId)> {
        let TypeView::DeducedTemplateSpecialization { name, deduced: None } =
            self.ast.types.view(ty.ty())
        else {
            return None;
        };
        let t = self.ast.types.template_decl(name)?;
        (self.kind_of(t) == DeclKind::ClassTemplate).then_some((name, t))
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
                        // Constructor templates (guides with the
                        // constructor's own parameters appended) are
                        // deferred (spec/sema.md, WP-071b).
                        any_ctor = true;
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
        if let Some(g) = self.copy_guide(ctx, name, injected, info.params, loc) {
            let _ = self.templ.ctad_copy.insert(g);
            out.push(g);
        }
        self.ast.templates.dependent_depth = saved;
        if def.is_some() {
            let _ = self.templ.ctad_guides.insert(first, out.clone());
        }
        out
    }

    /// The guide `template<P...> [explicit] C(params of ctor) -> C<P...>`
    /// (`ctor` `None`: the hypothetical `C()`).
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
                let variadic = self.ast.types.proto(ty).is_some_and(|p| p.variadic());
                (ps, variadic, fd.spec.is_explicit())
            }
            None => (Vec::new(), false, false),
        };
        let ptys: Vec<QualType> = cparams
            .iter()
            .map(|&p| self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR))
            .collect();
        let info = FuncInfo::new().with_trailing_return(true).with_variadic(variadic);
        let fty = self.ast.types.function(ret, &ptys, info, ExcSpec::None);
        let mut h = DeclHead::new(ctx, name, loc);
        h.flags = DeclFlags::IMPLICIT;
        let spec = FnSpec::new().with_variadic(variadic).with_explicit(explicit);
        self.ast.templates.dependent_depth = 1;
        self.ast.templates.pattern_next = true;
        let f = self.builder().deduction_guide(h, fty, spec);
        let mut ids = Vec::with_capacity(cparams.len());
        for (i, &p) in cparams.iter().enumerate() {
            let pn = *self.ast.decls.get(p);
            let data = self.ast.decls.param(p).copied();
            let flags = data.map_or(0, |d| d.flags & !ParamData::DEFAULT_ARG_PENDING);
            let idx = u16::try_from(i).unwrap_or(u16::MAX);
            let ty = pn.ty().unwrap_or(QualType::ERROR);
            let np = self.builder().param(f, pn.name(), pn.loc(), ty, idx, flags);
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
        self.ast.templates.dependent_depth = 0;
        Some(self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, params))
    }

    /// The copy deduction candidate `template<P...> C(C<P...>) -> C<P...>`.
    fn copy_guide(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        ret: QualType,
        params: TemplateParamListId,
        loc: Loc,
    ) -> Option<DeclId> {
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
        Some(self.builder().detached_template_decl(DeclKind::FunctionTemplate, f, params))
    }

    /// The user-declared deduction guides of class template `tmpl`
    /// ([temp.deduct.guide]/3: declared in its scope).
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
        let mut guides = self.implicit_guides(tmpl);
        guides.extend(self.user_guides(tmpl));
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
                self.ovl.stats.candidates += 1;
                let mut c = Candidate::new(pattern, g, CandidateKind::Function);
                let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
                c.ics = start..start;
                c.template = template;
                c.viable = false;
                c.failure = CandidateFailure::Explicit;
                set.cands.push(c);
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
                    let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                    let ret = self.ast.types.proto_of(ty.ty()).or_else(|| self.ast.types.proto(ty));
                    let ret = ret.map_or(QualType::ERROR, |p| p.ret());
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

    /// Whether candidate function (or template) `f` is an implicit
    /// deduction guide, and the copy deduction candidate
    /// ([over.match.best.general]/2.10, /2.12).
    pub(crate) fn ctad_guide_kind(
        &self,
        f: DeclId,
        template: Option<DeclId>,
    ) -> Option<(bool, bool)> {
        let g = template.unwrap_or(f);
        let pattern = template.and_then(|t| self.pattern_of(t)).unwrap_or(f);
        if self.kind_of(pattern) != DeclKind::CXXDeductionGuide
            && self.kind_of(f) != DeclKind::CXXDeductionGuide
        {
            return None;
        }
        let implicit = self.ast.decls.get(pattern).has(DeclFlags::IMPLICIT);
        Some((implicit, self.templ.ctad_copy.contains(&g)))
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
                    out.push_str(if pi.is_some_and(|x| x.typename_kw) {
                        "typename"
                    } else {
                        "class"
                    });
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
            if pack {
                out.push_str("...");
            }
            if !pname.is_empty() {
                out.push(' ');
                out.push_str(&pname);
            }
        }
        out.push_str("> ");
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
            out.push_str(&self.ty_str(self.ast.decls.get(*p).ty().unwrap_or(QualType::ERROR)));
        }
        if self.ast.types.proto(ty).is_some_and(|p| p.variadic()) {
            out.push_str(if params.is_empty() { "..." } else { ", ..." });
        }
        out.push_str(") -> ");
        let ret = self.ast.types.proto_of(ty.ty()).map_or(QualType::ERROR, |p| p.ret());
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
        self.deduce_ctad(declared, &args, &braced, how, loc)
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
