//! Templates (spec/sema-templates.md). WP-069 lands the parse-time half:
//! template heads and parameters, attaching a head to the declaration it
//! introduces, the template-aware name classification and the dependent
//! expression nodes ([`actions`]). Instantiation, substitution, deduction
//! and satisfaction are WP-070..074; until then they report
//! `sema_unimplemented_template_feature`.

pub mod actions;

use kiln_ast::decl::DeclKind;
use kiln_ast::ids::TemplateParamListId;
use kiln_ast::names::DeclName;
use kiln_ast::templates::is_template_kind;
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclHead, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::ParamInfo;
use crate::scope::{Ns, Scope, ScopeKind};

/// A template parameter list being parsed or waiting for its
/// declaration (one per `ScopeKind::TemplateParams` scope, plus invented
/// heads of abbreviated function templates).
#[derive(Clone, Debug)]
pub(crate) struct HeadBuild {
    template_loc: Loc,
    l_angle: Loc,
    depth: u16,
    params: Vec<DeclId>,
    /// The finished list ([`Sema::act_template_head`]).
    list: Option<TemplateParamListId>,
    /// Consumed by a declaration (or matched to a qualifier).
    taken: bool,
    /// Holds only invented parameters (`auto:N`, [dcl.fct]/22) and has no
    /// scope.
    invented: bool,
}

/// The template state of [`Sema`] (spec/sema-templates.md §11.2; the
/// engine's `InstState` is WP-070's).
#[derive(Clone, Debug, Default)]
pub struct TemplState {
    pub(crate) heads: Vec<HeadBuild>,
    /// The head consumed by the declaration being built: its pattern is
    /// wrapped in a template decl by [`Sema::finish_template_pattern`].
    pub(crate) cur: Option<TemplateParamListId>,
    /// The number of invented parameters of the innermost abbreviated
    /// function template (`auto:N` names, [dcl.fct]/22).
    pub(crate) invented: u16,
    /// The declaration contexts saved by open requires-expression bodies.
    pub(crate) req_ctx: Vec<DeclId>,
}

impl Sema<'_, '_> {
    /// Reports a template feature that is designed but not implemented
    /// yet ("kiln: not yet implemented: {feature}"); never panics.
    pub fn templ_unimplemented(&self, feature: &str, loc: Loc) {
        let d = self.diag(DiagId::sema_unimplemented_template_feature, loc).arg(feature);
        self.emit(d);
    }

    /// The depth the next template parameter list gets: the number of
    /// enclosing non-empty lists (spec/sema-templates.md §2.1).
    pub fn template_depth(&self) -> u16 {
        self.templ.heads.len() as u16
    }

    /// True inside a template parameter scope (a dependent context,
    /// [temp.dep]).
    pub fn in_template(&self) -> bool {
        self.ast.templates.dependent_depth > 0
    }

    /// `template <` ([temp.pre]/1): opens the parameter scope; returns the
    /// depth of the parameters. Paired with
    /// [`Sema::act_on_finish_template_decl`].
    pub fn act_on_template_params_start(&mut self, template_loc: Loc, l_angle: Loc) -> u16 {
        let depth = self.template_depth();
        if self.in_local_scope() {
            self.error(DiagId::sema_template_outside_namespace_or_class, template_loc);
        }
        self.templ.heads.push(HeadBuild {
            template_loc,
            l_angle,
            depth,
            params: Vec::new(),
            list: None,
            taken: false,
            invented: false,
        });
        self.scopes.push(Scope::new(ScopeKind::TemplateParams, None));
        self.ast.templates.dependent_depth += 1;
        depth
    }

    /// Leaves the template parameter scope of the innermost head.
    pub fn act_on_finish_template_decl(&mut self) {
        if let Some(i) = self.scopes.iter().rposition(|s| s.kind == ScopeKind::TemplateParams) {
            self.scopes.truncate(i);
        }
        self.templ.heads.pop();
        self.ast.templates.dependent_depth = self.ast.templates.dependent_depth.saturating_sub(1);
    }

    fn param_head(&self, name: Option<(Symbol, Loc)>, loc: Loc) -> DeclHead {
        let dname = name.map_or(DeclName::EMPTY, |n| DeclName::ident(n.0));
        DeclHead::new(self.ctx, dname, name.map_or(loc, |n| n.1))
    }

    /// Records parameter `p` of the innermost head and makes its name
    /// visible; diagnoses a name declared twice in one list.
    fn push_template_param(&mut self, p: DeclId, name: Option<(Symbol, Loc)>) {
        let Some(h) = self.templ.heads.last_mut() else { return };
        h.params.push(p);
        let Some((n, loc)) = name else { return };
        let Some(s) = self.scopes.iter_mut().rev().find(|s| s.kind == ScopeKind::TemplateParams)
        else {
            return;
        };
        if let Some(prev) = s.find(n, Ns::Ordinary) {
            s.locals.push((n, p, Ns::Ordinary));
            let msg = self
                .diag(DiagId::sema_template_param_shadow, loc)
                .arg(format!("'{}'", self.ast.symbols().as_str(n)));
            let msg = self.note_prev_loc(msg, prev);
            self.emit(msg);
            return;
        }
        s.locals.push((n, p, Ns::Ordinary));
    }

    fn note_prev_loc(&self, msg: kiln_diag::Diag, prev: DeclId) -> kiln_diag::Diag {
        let l = self.ast.decls.get(prev).loc();
        msg.note(DiagId::note_template_param_here, kiln_source::Span::empty(l))
    }

    fn next_param_index(&self) -> u16 {
        self.templ.heads.last().map_or(0, |h| h.params.len() as u16)
    }

    fn cur_depth(&self) -> u16 {
        self.templ.heads.last().map_or(0, |h| h.depth)
    }

    /// `class T`, `typename... Ts`, `typename = void`
    /// ([temp.param]/2): a type parameter.
    pub fn act_on_type_template_param(
        &mut self,
        typename_kw: bool,
        kw_loc: Loc,
        pack: bool,
        name: Option<(Symbol, Loc)>,
    ) -> DeclId {
        let h = self.param_head(name, kw_loc);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        let p = self.builder().template_type_parm(h, depth, index, pack, typename_kw);
        self.push_template_param(p, name);
        p
    }

    /// `C T`, `C<int>... Ts` ([temp.param]/4): a type parameter whose
    /// type-constraint `concept<args>` becomes its immediately-declared
    /// constraint `concept<T, args...>`. clang prints it with `class`.
    pub fn act_on_constrained_template_param(
        &mut self,
        concept: DeclId,
        cargs: &[TemplateArg],
        loc: Loc,
        pack: bool,
        name: Option<(Symbol, Loc)>,
    ) -> DeclId {
        let p = self.act_on_type_template_param(false, loc, pack, name);
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        let c = self.immediately_declared_constraint(concept, ty, cargs, loc);
        self.builder().set_parm_constraint(p, c);
        p
    }

    /// `concept<ty, extra...>` as a `ConceptSpecializationExpr`
    /// ([temp.param]/4, [dcl.type.auto.deduct]).
    pub(crate) fn immediately_declared_constraint(
        &mut self,
        concept: DeclId,
        ty: QualType,
        extra: &[TemplateArg],
        loc: Loc,
    ) -> ExprId {
        let mut args = Vec::with_capacity(extra.len() + 1);
        args.push(TemplateArg::ty(ty));
        args.extend_from_slice(extra);
        let l = self.ast.types.arg_list(&args);
        self.builder().concept_spec_expr(concept, l, QualType::BOOL, loc)
    }

    /// A non-type template parameter ([temp.param]/2) declared like a
    /// function parameter `p`; `pack` for `T... N`.
    pub fn act_on_non_type_template_param(&mut self, p: &ParamInfo, pack: bool) -> DeclId {
        let h = self.param_head(p.name, p.loc);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        let mut ty = p.ty;
        if pack
            && !matches!(self.ast.types.view(ty.ty()), kiln_ast::types::TypeView::PackExpansion { .. })
        {
            ty = self.ast.types.pack_expansion(ty, None);
        }
        let d = self.builder().non_type_template_parm(h, ty, depth, index, pack);
        self.push_template_param(d, p.name);
        d
    }

    /// Starts the parameter list of a template template parameter
    /// (`template<class> class TT`): its parameters are one level deeper.
    pub fn act_on_template_template_params_start(&mut self, template_loc: Loc, l_angle: Loc) {
        let depth = self.template_depth();
        self.templ.heads.push(HeadBuild {
            template_loc,
            l_angle,
            depth,
            params: Vec::new(),
            list: None,
            taken: true,
            invented: false,
        });
        self.scopes.push(Scope::new(ScopeKind::TemplateParams, None));
        self.ast.templates.dependent_depth += 1;
    }

    /// `template<params> class TT` ([temp.param]/2), after
    /// [`Sema::act_on_template_template_params_start`] and the inner
    /// [`Sema::act_template_head`]: closes the inner list and declares
    /// the parameter in the outer one.
    pub fn act_on_template_template_param(
        &mut self,
        params: TemplateParamListId,
        typename_kw: bool,
        kw_loc: Loc,
        pack: bool,
        name: Option<(Symbol, Loc)>,
    ) -> DeclId {
        self.act_on_finish_template_decl();
        let h = self.param_head(name, kw_loc);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        let p = self.builder().template_template_parm(h, params, depth, index, pack, typename_kw);
        self.push_template_param(p, name);
        p
    }

    /// The default argument `= arg` of parameter `p`
    /// ([temp.param]/10-12).
    pub fn act_on_template_param_default(&mut self, p: DeclId, arg: TemplateArg, loc: Loc) {
        if self.ast.template_parm(p).is_some_and(|x| x.pack) {
            // [temp.param]/11 ("template parameter pack cannot have a
            // default argument") is diagnosed by clang; kiln drops it.
            self.templ_unimplemented("default argument of a template parameter pack", loc);
            return;
        }
        if self.builder().set_template_default(p, arg).is_err() {
            self.error(DiagId::sema_template_param_default_redefined, loc);
        }
    }

    /// `>` and the optional requires-clause of the innermost head
    /// ([temp.pre]/1): builds its `TemplateParameterList`, checking the
    /// pack and default-argument ordering rules ([temp.param]/14).
    ///
    /// Deviation from the spec's `act_template_head(params, requires,
    /// depth)`: the parameters were declared one by one through the
    /// `act_on_*_template_param` actions (they must be visible to the
    /// later parameters' defaults), so the head takes them from the
    /// innermost open list.
    pub fn act_template_head(&mut self, requires: Option<ExprId>, r_angle: Loc) -> TemplateParamListId {
        let Some(h) = self.templ.heads.last().cloned() else {
            return self.builder().template_param_list(
                Loc::INVALID,
                Loc::INVALID,
                r_angle,
                0,
                &[],
                requires,
            );
        };
        self.check_template_param_order(&h.params);
        let l = self.builder().template_param_list(
            h.template_loc,
            h.l_angle,
            r_angle,
            h.depth,
            &h.params,
            requires,
        );
        if let Some(h) = self.templ.heads.last_mut() {
            h.list = Some(l);
        }
        l
    }

    /// [temp.param]/14: a pack is last in a primary class template's
    /// list, and every parameter after one with a default has one.
    fn check_template_param_order(&mut self, params: &[DeclId]) {
        let mut seen_default: Option<DeclId> = None;
        for &p in params {
            let has = self.ast.templates.defaults.get(p).is_some();
            let pack = self.ast.template_parm(p).is_some_and(|x| x.pack);
            if has {
                seen_default = Some(p);
            } else if let Some(prev) = seen_default
                && !pack
            {
                let loc = self.ast.decls.get(p).loc();
                let l = self.ast.decls.get(prev).loc();
                let msg = self.diag(DiagId::sema_template_param_missing_default, loc).note(
                    DiagId::note_template_param_has_default,
                    kiln_source::Span::empty(l),
                );
                self.emit(msg);
            }
        }
    }

    /// The innermost untaken head the declaration being declared consumes
    /// ([temp.pre]/1), after matching the outer heads to the class
    /// templates named by its qualifier (`template<class T> void
    /// A<T>::f()`, [temp.mem]/1; `match_template_lists_to_scope`). Sets
    /// [`TemplState::cur`].
    pub(crate) fn take_template_head(&mut self, qualifier: Option<DeclId>) -> Option<TemplateParamListId> {
        self.templ.cur = None;
        // The untaken heads directly around this declaration.
        let mut open = Vec::new();
        for (i, h) in self.templ.heads.iter().enumerate().rev() {
            if h.taken {
                break;
            }
            open.push(i);
        }
        open.reverse();
        if open.is_empty() {
            return None;
        }
        let consumed = self.match_template_lists_to_scope(qualifier);
        let mut it = open.iter().copied();
        for _ in 0..consumed {
            if let Some(i) = it.next() {
                self.templ.heads[i].taken = true;
            }
        }
        let rest: Vec<usize> = it.collect();
        let Some(&last) = rest.last() else { return None };
        if rest.len() > 1 {
            let loc = self.templ.heads[rest[0]].template_loc;
            self.error(DiagId::sema_extraneous_template_params, loc);
        }
        for &i in &rest {
            self.templ.heads[i].taken = true;
        }
        let list = self.templ.heads[last].list;
        self.templ.cur = list;
        list
    }

    /// How many template parameter lists the qualifier of an out-of-line
    /// declaration accounts for: one per class template (pattern) on its
    /// context chain that is not already open (clang's
    /// `MatchTemplateParametersToScopeSpecifier`, [temp.mem]/1). Explicit
    /// specializations (`template<>`) are not matched yet (WP-070a).
    pub fn match_template_lists_to_scope(&self, qualifier: Option<DeclId>) -> usize {
        let Some(mut c) = qualifier else { return 0 };
        let mut n = 0;
        for _ in 0..1024 {
            if self.kind_of(c) == DeclKind::CXXRecord
                && self.ast.described_template(self.ast.first(c)).is_some()
                && !self.defining.iter().any(|&d| self.ast.first(d) == self.ast.first(c))
            {
                n += 1;
            }
            match self.ast.decls.get(c).ctx() {
                Some(p) => c = p,
                None => break,
            }
        }
        n
    }

    /// Called right before the builder creates the declaration a head
    /// consumed: the next declaration becomes the template's pattern.
    pub(crate) fn mark_pattern_next(&mut self) {
        if self.templ.cur.is_some() {
            self.ast.templates.pattern_next = true;
        }
    }

    /// Wraps `pattern` in its template decl when the declaration consumed
    /// a head ([`Sema::take_template_head`]); returns the template decl.
    pub(crate) fn finish_template_pattern(&mut self, pattern: DeclId) -> Option<DeclId> {
        let list = self.templ.cur.take()?;
        let kind = match self.kind_of(pattern) {
            DeclKind::CXXRecord => DeclKind::ClassTemplate,
            DeclKind::Var => DeclKind::VarTemplate,
            DeclKind::TypeAlias => DeclKind::TypeAliasTemplate,
            k if k.payload() == kiln_ast::decl::PayloadClass::Function => DeclKind::FunctionTemplate,
            _ => {
                let loc = self.ast.decls.get(pattern).loc();
                self.templ_unimplemented("this kind of template declaration", loc);
                return None;
            }
        };
        let prev = self.ast.prev(pattern).and_then(|p| self.ast.described_template(p));
        let t = self.builder().template_decl(kind, pattern, list, prev);
        Some(t)
    }

    /// The template decl `d` (a template, or the pattern of one) is part
    /// of, if any.
    pub fn template_of(&self, d: DeclId) -> Option<DeclId> {
        if is_template_kind(self.kind_of(d)) {
            Some(d)
        } else {
            self.ast.described_template(d)
        }
    }

    /// The most recent pattern of template `t`.
    pub(crate) fn pattern_of(&self, t: DeclId) -> Option<DeclId> {
        self.ast.template_info(t).and_then(|i| i.pattern)
    }

    /// Re-enters the template parameter scopes of `f`'s template (and of
    /// the class templates around it that are not open) for a body parsed
    /// after its declaration (a member function template of a class,
    /// [class.mem]/7). Returns how many heads were pushed.
    pub fn act_on_reenter_template_scopes(&mut self, f: DeclId) -> usize {
        let Some(t) = self.ast.described_template(f) else { return 0 };
        let Some(info) = self.ast.template_info(t).copied() else { return 0 };
        let list = *self.ast.templates.param_list(info.params);
        if self.templ.heads.iter().any(|h| h.list == Some(info.params)) {
            return 0;
        }
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        self.templ.heads.push(HeadBuild {
            template_loc: list.template_loc,
            l_angle: list.l_angle,
            depth: list.depth,
            params: params.clone(),
            list: Some(info.params),
            taken: true,
            invented: false,
        });
        let mut s = Scope::new(ScopeKind::TemplateParams, None);
        for p in params {
            if let Some(n) = self.ast.decls.get(p).name().as_ident() {
                s.locals.push((n, p, Ns::Ordinary));
            }
        }
        // Below the scopes of the function's own definition.
        self.scopes.push(s);
        self.ast.templates.dependent_depth += 1;
        1
    }

    /// Leaves the `n` template parameter scopes entered by
    /// [`Sema::act_on_reenter_template_scopes`].
    pub fn act_on_exit_reentered_template_scopes(&mut self, n: usize) {
        for _ in 0..n {
            if self.scopes.last().is_some_and(|s| s.kind == ScopeKind::TemplateParams) {
                self.scopes.pop();
            }
            self.templ.heads.pop();
            self.ast.templates.dependent_depth = self.ast.templates.dependent_depth.saturating_sub(1);
        }
    }

    /// The invented type parameter `auto:N` of a parameter whose type
    /// contains `auto` ([dcl.fct]/22): added to the innermost open head,
    /// or to a new invented head one level deeper when the function has
    /// none. Returns its type; `constraint` is the type-constraint `C` of
    /// `C auto` with its extra arguments.
    pub fn act_on_invented_template_param(
        &mut self,
        loc: Loc,
        pack: bool,
        constraint: Option<(DeclId, Vec<TemplateArg>)>,
    ) -> QualType {
        let needs_new = self.templ.heads.last().is_none_or(|h| h.taken || h.list.is_none());
        if needs_new {
            let depth = self.template_depth();
            self.templ.heads.push(HeadBuild {
                template_loc: loc,
                l_angle: loc,
                depth,
                params: Vec::new(),
                list: None,
                taken: false,
                invented: true,
            });
            self.ast.templates.dependent_depth += 1;
            self.templ.invented = 0;
        }
        let n = self.templ.invented;
        self.templ.invented += 1;
        let name = self.intern(&format!("auto:{}", n + 1));
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc);
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        let p = self.builder().template_type_parm(h, depth, index, pack, false);
        self.ast.decls.set_flags(p, kiln_ast::decl::DeclFlags::IMPLICIT);
        if let Some(head) = self.templ.heads.last_mut() {
            head.params.push(p);
        }
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if let Some((c, extra)) = constraint {
            let e = self.immediately_declared_constraint(c, ty, &extra, loc);
            self.builder().set_parm_constraint(p, e);
        }
        ty
    }

    /// Before a function declarator's declaration is created: finishes
    /// the head an abbreviated function template's invented parameters
    /// were added to ([dcl.fct]/22). An explicit head gets a new list
    /// with the invented parameters appended.
    pub(crate) fn finish_invented_head(&mut self) {
        let Some(h) = self.templ.heads.last() else { return };
        if h.taken || self.templ.invented == 0 {
            return;
        }
        let h = h.clone();
        let requires = h.list.map(|l| self.ast.templates.param_list(l).requires).unwrap_or(None);
        let r = h.list.map_or(h.l_angle, |l| self.ast.templates.param_list(l).r_angle);
        let l = self.builder().template_param_list(
            h.template_loc,
            h.l_angle,
            r,
            h.depth,
            &h.params,
            requires,
        );
        if let Some(x) = self.templ.heads.last_mut() {
            x.list = Some(l);
        }
        self.templ.invented = 0;
    }

    /// After a declaration with invented parameters only (no written
    /// head) is complete: drops its invented head.
    pub fn act_on_end_abbreviated(&mut self) {
        if self.templ.heads.last().is_some_and(|h| h.invented) {
            self.templ.heads.pop();
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
        }
        self.templ.invented = 0;
    }

    /// Attaches the trailing requires-clause `r` to function `f`
    /// ([dcl.decl.general]/4); only a templated function may have one
    /// ([dcl.decl.general]/5).
    pub fn act_on_trailing_requires(&mut self, f: DeclId, r: ExprId) {
        if self.kind_of(f).payload() != kiln_ast::decl::PayloadClass::Function {
            let loc = self.ast.exprs.get(r).loc();
            self.error(DiagId::sema_trailing_requires_non_function, loc);
            return;
        }
        if !self.in_template() {
            let loc = self.ast.exprs.get(r).loc();
            self.error(DiagId::sema_trailing_requires_non_template, loc);
        }
        let _ = self.ast.templates.trailing_requires.insert(f, r);
    }

    /// The default argument recorded for template parameter `p`.
    pub fn template_default(&self, p: DeclId) -> Option<TemplateArg> {
        self.ast.templates.defaults.get(p).copied()
    }

    /// True while parsing between `template<...>` and the declaration it
    /// heads (the next declaration consumes it).
    pub fn has_pending_head(&self) -> bool {
        self.templ.heads.last().is_some_and(|h| !h.taken)
    }
}
