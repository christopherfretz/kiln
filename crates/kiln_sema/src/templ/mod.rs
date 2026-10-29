//! Templates (spec/sema-templates.md). WP-069 lands the parse-time half:
//! template heads and parameters, attaching a head to the declaration it
//! introduces, the template-aware name classification and the dependent
//! expression nodes ([`actions`]). Instantiation, substitution, deduction
//! and satisfaction are WP-070..074. WP-070a lands the engine core:
//! argument checking ([`args`]), the substituter ([`subst`]), class
//! template instantiation ([`class`]), lazy member instantiation
//! ([`pending`]), the instantiation stack ([`stack`]) and the report
//! ([`report`]); deduction ([`deduce`]), partial ordering ([`order`]),
//! pack expansion and satisfaction still report
//! `sema_unimplemented_template_feature`.

pub mod actions;
pub mod args;
pub mod auto;
pub mod builtins;
pub mod class;
pub mod deduce;
pub mod order;
pub mod pending;
pub mod report;
pub mod stack;
pub mod subst;

use kiln_ast::decl::DeclKind;
use kiln_ast::ids::{ArgListId, TemplateParamListId};
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
    /// A declarator-id's nested-name-specifier is being parsed (clang's
    /// `EnteringContext`): `X<P...>::` names the current instantiation
    /// even outside `X`'s definition ([temp.mem]/1, out-of-line members).
    pub(crate) entering_context: bool,
    /// The head consumed by the declaration being built: its pattern is
    /// wrapped in a template decl by [`Sema::finish_template_pattern`].
    pub(crate) cur: Option<TemplateParamListId>,
    /// The number of invented parameters of the innermost abbreviated
    /// function template (`auto:N` names, [dcl.fct]/22).
    pub(crate) invented: u16,
    /// The declaration contexts saved by open requires-expression bodies.
    pub(crate) req_ctx: Vec<DeclId>,
    /// Where each template parameter's default argument starts (the
    /// redefinition and missing-default diagnostics point there).
    pub(crate) default_locs: kiln_source::fx::FxHashMap<DeclId, Loc>,
    /// Where each argument of the template-id being parsed starts (set by
    /// the parser right before `act_template_id`; consumed there).
    pub(crate) arg_locs: Vec<Loc>,
}

/// The instantiation engine's state (spec/sema-templates.md §11.2).
#[derive(Debug, Default)]
pub struct InstState {
    /// The active instantiations, innermost last (§10.1).
    pub(crate) stack: stack::InstantiationStack,
    /// `-ftemplate-depth=` / `-ftemplate-backtrace-limit=`.
    pub limits: stack::InstLimits,
    /// Member definitions whose instantiation was required at a point
    /// where it could not run (§4.4), drained at end of TU.
    pub(crate) pending: std::collections::VecDeque<pending::PendingInst>,
    /// One entry per active SFINAE context: the errors it captured (§5.7).
    pub(crate) sfinae: std::cell::RefCell<Vec<stack::SfinaeState>>,
    /// Alias template specializations already substituted (§4.6).
    pub(crate) alias_cache: kiln_source::fx::FxHashMap<(DeclId, ArgListId), QualType>,
    /// The instantiation report (§11.3).
    pub report: report::InstReport,
    /// The depth limit was hit: no further instantiation (the error is
    /// fatal).
    pub(crate) fatal: bool,
    /// The members already queued in `pending` (no duplicates).
    pub(crate) queued: kiln_source::fx::FxHashSet<DeclId>,
    /// Inline static data members of specializations whose initializer
    /// is not instantiated yet ([temp.inst]/4: on first reference).
    pub(crate) deferred_inits: kiln_source::fx::FxHashSet<DeclId>,
    /// Declaring such a member: its `constexpr` does not imply `inline`
    /// until the initializer is instantiated (as clang).
    pub(crate) deferring_init: bool,
    /// The specialization just diagnosed as an implicit instantiation of
    /// an undefined template: the caller's own incomplete-type error for
    /// it is suppressed, as clang's `RequireCompleteType` reports only
    /// the instantiation failure.
    pub(crate) undefined_diagnosed: std::cell::Cell<Option<DeclId>>,
    /// Drop the next error passed to `emit` (set by `incomplete_note`).
    pub(crate) suppress_next_error: std::cell::Cell<bool>,
}

impl Sema<'_, '_> {
    /// Reports a template feature that is designed but not implemented
    /// yet ("kiln: not yet implemented: {feature}"); never panics.
    pub fn templ_unimplemented(&self, feature: &str, loc: Loc) {
        let d = self.diag(DiagId::sema_unimplemented_template_feature, loc).arg(feature);
        self.emit(d);
    }

    /// Sets whether a declarator-id's nested-name-specifier is being
    /// parsed (clang's `EnteringContext`); returns the previous value.
    pub fn set_entering_context(&mut self, on: bool) -> bool {
        std::mem::replace(&mut self.templ.entering_context, on)
    }

    /// Records where each argument of the next template-id starts (the
    /// kind and conversion diagnostics point there, as clang's do).
    pub fn set_template_arg_locs(&mut self, locs: Vec<Loc>) {
        self.templ.arg_locs = locs;
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

    /// [temp.local]/6: a declaration of `name` (not a template
    /// parameter) whose ordinary lookup finds a template parameter
    /// redeclares it (clang's `DiagnoseTemplateParameterShadow`).
    pub(crate) fn check_template_param_shadow(&self, name: Symbol, loc: Loc) {
        if !self.in_template() {
            return;
        }
        let found = self.lookup_name(name, crate::lookup::LookupKind::Ordinary);
        let [p] = found.decls[..] else { return };
        if !kiln_ast::templates::is_template_parm_kind(self.kind_of(p)) {
            return;
        }
        let msg = self
            .diag(DiagId::sema_template_param_shadow, loc)
            .arg(format!("'{}'", self.ast.symbols().as_str(name)));
        let msg = self.note_prev_loc(msg, p);
        self.emit(msg);
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
        let mut c = self.immediately_declared_constraint(concept, ty, cargs, loc);
        if pack {
            c = self.pack_constraint(c, loc);
        }
        self.builder().set_parm_constraint(p, c);
        p
    }

    /// The immediately-declared constraint of a constrained pack is the
    /// fold `(C<Ts> && ...)` ([temp.param]/4; clang's
    /// `formImmediatelyDeclaredConstraint`).
    pub(crate) fn pack_constraint(&mut self, c: ExprId, loc: Loc) -> ExprId {
        self.builder().fold_expr(Some(c), kiln_ast::expr::BinaryOp::LAnd, None, loc, loc)
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
        // `int... N` keeps the type `int` (as clang); only a type with
        // unexpanded packs (`Ts... Vs`) becomes a pack expansion.
        let mut ty = p.ty;
        if pack
            && self.ast.types.flags(ty.ty()).contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK)
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
            return;
        }
        let _ = self.templ.default_locs.insert(p, loc);
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
    pub fn act_template_head(
        &mut self,
        requires: Option<ExprId>,
        r_angle: Loc,
    ) -> TemplateParamListId {
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

    /// clang's `CheckTemplateParameterList` for the template `kind` whose
    /// head is `list`, redeclaring the template `prev` if any:
    /// - [temp.param]/12: a default argument is not redefined by a
    ///   redeclaration; one given earlier is inherited;
    /// - [temp.param]/14: in a class, variable or alias template, every
    ///   parameter after one with a (possibly inherited) default has one or
    ///   is a pack, and a pack is the last parameter.
    pub(crate) fn check_template_param_list(
        &mut self,
        kind: DeclKind,
        list: TemplateParamListId,
        prev: Option<DeclId>,
    ) {
        let params: Vec<DeclId> = {
            let l = *self.ast.templates.param_list(list);
            self.ast.lists.iter::<DeclId>(l.params).collect()
        };
        let old: Vec<DeclId> = prev
            .and_then(|t| self.ast.template_info(t).copied())
            .map(|i| {
                let l = *self.ast.templates.param_list(i.params);
                self.ast.lists.iter::<DeclId>(l.params).collect()
            })
            .unwrap_or_default();
        let ordered = matches!(
            kind,
            DeclKind::ClassTemplate | DeclKind::VarTemplate | DeclKind::TypeAliasTemplate
        );
        let mut prev_default: Option<Loc> = None;
        for (i, &p) in params.iter().enumerate() {
            let new_loc = self.templ.default_locs.get(&p).copied();
            let old_loc = old.get(i).and_then(|&o| self.templ.default_locs.get(&o).copied());
            let pack = self.ast.template_parm(p).is_some_and(|x| x.pack);
            match (new_loc, old_loc) {
                (Some(n), Some(o)) => {
                    let d = self.diag(DiagId::sema_template_param_default_redefined, n).note(
                        DiagId::note_template_param_prev_default,
                        kiln_source::Span::empty(o),
                    );
                    self.emit(d);
                    prev_default = Some(n);
                }
                (Some(l), None) | (None, Some(l)) => prev_default = Some(l),
                (None, None) => {
                    if ordered
                        && !pack
                        && let Some(l) = prev_default
                    {
                        let loc = self.ast.decls.get(p).loc();
                        let d = self.diag(DiagId::sema_template_param_missing_default, loc).note(
                            DiagId::note_template_param_has_default,
                            kiln_source::Span::empty(l),
                        );
                        self.emit(d);
                    }
                }
            }
            if ordered && pack && i + 1 != params.len() {
                let loc = self.ast.decls.get(p).loc();
                self.error(DiagId::sema_template_param_pack_not_last, loc);
            }
        }
    }

    /// The innermost untaken head the declaration being declared consumes
    /// ([temp.pre]/1), after matching the outer heads to the class
    /// templates named by its qualifier (`template<class T> void
    /// A<T>::f()`, [temp.mem]/1; `match_template_lists_to_scope`). Sets
    /// [`TemplState::cur`].
    pub(crate) fn take_template_head(
        &mut self,
        qualifier: Option<DeclId>,
    ) -> Option<TemplateParamListId> {
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
        let &innermost = rest.last()?;
        if rest.len() > 1 {
            let loc = self.templ.heads[rest[0]].template_loc;
            self.error(DiagId::sema_extraneous_template_params, loc);
        }
        for &i in &rest {
            self.templ.heads[i].taken = true;
        }
        let list = self.templ.heads[innermost].list;
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
            k if k.payload() == kiln_ast::decl::PayloadClass::Function => {
                DeclKind::FunctionTemplate
            }
            _ => {
                let loc = self.ast.decls.get(pattern).loc();
                self.templ_unimplemented("this kind of template declaration", loc);
                return None;
            }
        };
        let prev = self.ast.prev(pattern).and_then(|p| self.ast.described_template(p));
        self.check_template_param_list(kind, list, prev);
        let t = self.builder().template_decl(kind, pattern, list, prev);
        Some(t)
    }

    /// The template decl `d` (a template, or the pattern of one) is part
    /// of, if any.
    pub fn template_of(&self, d: DeclId) -> Option<DeclId> {
        if is_template_kind(self.kind_of(d)) { Some(d) } else { self.ast.described_template(d) }
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
            self.ast.templates.dependent_depth =
                self.ast.templates.dependent_depth.saturating_sub(1);
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
        param_name: Option<Symbol>,
    ) -> QualType {
        // An explicit head is open once its list is built; an invented one
        // collects every `auto` parameter of the declarator.
        let needs_new =
            self.templ.heads.last().is_none_or(|h| h.taken || (!h.invented && h.list.is_none()));
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
        self.templ.invented += 1;
        let (depth, index) = (self.cur_depth(), self.next_param_index());
        // clang's names: `x:auto` for parameter `x`, else `auto:N` with
        // `N` the 1-based position in the list.
        let name = match param_name {
            Some(s) => {
                let text = format!("{}:auto", self.ast.symbols().as_str(s));
                self.intern(&text)
            }
            None => self.intern(&format!("auto:{}", u32::from(index) + 1)),
        };
        let h = DeclHead::new(self.ctx, DeclName::ident(name), loc)
            .flags(kiln_ast::decl::DeclFlags::IMPLICIT);
        let p = self.builder().template_type_parm(h, depth, index, pack, false);
        if let Some(head) = self.templ.heads.last_mut() {
            head.params.push(p);
        }
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if let Some((c, extra)) = constraint {
            let mut e = self.immediately_declared_constraint(c, ty, &extra, loc);
            if pack {
                e = self.pack_constraint(e, loc);
            }
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
        let requires = h.list.and_then(|l| self.ast.templates.param_list(l).requires);
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
        // Also the end of a declaration: uses of deleted functions that
        // no full-expression reported (spec/sema-class.md §4.3).
        if !self.special.pending_deleted.is_empty() {
            self.flush_pending_deleted();
        }
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

    /// A declarator-id that is a template-id naming the function or
    /// variable template `tmpl` (`f<T*>`, `v<int>`) after a template head
    /// ([temp.expl.spec], [temp.spec.partial]): a function template
    /// partial specialization is an error ([temp.spec.partial.general]/1
    /// covers only class and variable templates; clang's
    /// `err_function_template_partial_spec`); the other forms are
    /// WP-070a's. Returns false when the declarator should be dropped.
    pub fn act_on_specialization_declarator_id(&mut self, tmpl: DeclId, loc: Loc) -> bool {
        let explicit = self.templ.heads.last().is_some_and(|h| !h.taken && h.params.is_empty());
        match (self.kind_of(tmpl), explicit) {
            (DeclKind::FunctionTemplate, false) => {
                self.error(DiagId::sema_function_partial_spec, loc);
            }
            (DeclKind::FunctionTemplate, true) => {
                self.templ_unimplemented("explicit specialization of a function template", loc);
            }
            (_, false) => self.templ_unimplemented("variable template partial specialization", loc),
            (_, true) => {
                self.templ_unimplemented("explicit specialization of a variable template", loc);
            }
        }
        false
    }

    /// True while parsing between `template<...>` and the declaration it
    /// heads (the next declaration consumes it).
    pub fn has_pending_head(&self) -> bool {
        self.templ.heads.last().is_some_and(|h| !h.taken)
    }
}
