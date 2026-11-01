//! Class template specializations (spec/sema-templates.md §4): naming
//! one (`get_class_spec`), instantiating its definition when a complete
//! type is required (`instantiate_class`, [temp.inst]/2), alias template
//! specializations (§4.10), and the definitions of members, which are
//! instantiated when used (`instantiate_function_definition`,
//! [temp.inst]/4).

use kiln_ast::decl::{DeclFlags, DeclKind, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::{ArgListId, TemplateNameId};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TagKeyword, TemplateArg, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_ast::names::{DeclName, DeclNameView};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::class::BaseSpecInfo;
use crate::decl::TagUse;
use crate::declspec::DeclaratorContext;
use crate::init::Initializer;
use crate::scope::{Scope, ScopeKind};
use crate::templ::args::{LevelMode, MultiLevelArgs};
use crate::templ::stack::FrameKind;
use crate::templ::subst::{LocalMethod, SubstFail, Substituter};

/// The parse state set aside while an instantiation runs in the context
/// of its template (§4.5): an instantiation can be triggered from deep
/// inside any declaration, statement or expression.
#[derive(Default)]
struct SavedState {
    scopes: Vec<Scope>,
    ctx: Option<DeclId>,
    func: Option<crate::stmt::FnState>,
    unevaluated: u32,
    immediate: u32,
    pending_odr: Vec<kiln_ast::ExprId>,
    defining: Vec<DeclId>,
    enums: Vec<crate::decl::EnumBuild>,
    braced_args: Vec<(kiln_ast::ExprId, crate::init::InitList)>,
    prototype_params: Vec<crate::decl::PrototypeFrame>,
    deferring_call_ref: bool,
    deferred_call_refs: Vec<(kiln_ast::ExprId, DeclId)>,
    classes: Vec<crate::class::ClassBuild>,
    cleanup: crate::class::CleanupState,
    extend_collector: Option<Vec<kiln_ast::ExprId>>,
    nsdmi_this: Vec<(DeclId, QualType)>,
    initializing: Option<DeclId>,
    heads: Vec<super::HeadBuild>,
    cur: Option<kiln_ast::ids::TemplateParamListId>,
    entering_context: bool,
    invented: u16,
    req_ctx: Vec<DeclId>,
    dependent_depth: u32,
    friend_tag: Option<DeclId>,
}

impl Sema<'_, '_> {
    /// Runs `f` in the context of declaration context `ctx` (a namespace
    /// or a class), with the parse state of the point of instantiation
    /// set aside ([temp.point]: names are looked up from the template's
    /// definition, not from where the instantiation was required).
    pub(crate) fn in_inst_context<R>(&mut self, ctx: DeclId, f: impl FnOnce(&mut Self) -> R) -> R {
        let tu_dirs = self.scopes.first().map(|s| s.using_dirs.clone()).unwrap_or_default();
        let outer_depth = self.range_for_outer_depth;
        if outer_depth.is_none() {
            self.range_for_outer_depth = Some(self.clang_scope_depth());
        }
        let imm = self.take_immediate_state(true);
        let saved = SavedState {
            scopes: std::mem::take(&mut self.scopes),
            ctx: Some(self.ctx),
            func: self.func.take(),
            unevaluated: std::mem::take(&mut self.unevaluated),
            immediate: std::mem::take(&mut self.immediate),
            pending_odr: std::mem::take(&mut self.pending_odr),
            defining: std::mem::take(&mut self.defining),
            enums: std::mem::take(&mut self.enums),
            braced_args: std::mem::take(&mut self.braced_args),
            prototype_params: std::mem::take(&mut self.prototype_params),
            deferring_call_ref: std::mem::take(&mut self.deferring_call_ref),
            deferred_call_refs: std::mem::take(&mut self.deferred_call_refs),
            classes: std::mem::take(&mut self.classes),
            cleanup: std::mem::take(&mut self.cleanup),
            extend_collector: self.extend_collector.take(),
            nsdmi_this: std::mem::take(&mut self.nsdmi_this),
            initializing: self.initializing.take(),
            heads: std::mem::take(&mut self.templ.heads),
            cur: self.templ.cur.take(),
            entering_context: std::mem::take(&mut self.templ.entering_context),
            invented: std::mem::take(&mut self.templ.invented),
            req_ctx: std::mem::take(&mut self.templ.req_ctx),
            dependent_depth: std::mem::take(&mut self.ast.templates.dependent_depth),
            friend_tag: self.friend_tag.take(),
        };
        let mut tu = Scope::new(ScopeKind::TranslationUnit, Some(self.ast.tu));
        tu.using_dirs = tu_dirs;
        self.scopes.push(tu);
        let mut chain = Vec::new();
        let mut c = Some(ctx);
        while let Some(x) = c {
            if x == self.ast.tu || chain.len() > 1024 {
                break;
            }
            chain.push(x);
            c = self.ast.decls.get(x).ctx();
        }
        for x in chain.into_iter().rev() {
            let s = match self.kind_of(x) {
                DeclKind::CXXRecord => {
                    Scope::new(ScopeKind::Record, Some(self.ast.definition(x).unwrap_or(x)))
                }
                DeclKind::Namespace => Scope::new(ScopeKind::Namespace, Some(x)),
                DeclKind::LinkageSpec | DeclKind::Export => {
                    Scope::new(ScopeKind::LinkageSpec, Some(x))
                }
                // A local class's context (WP-070b): its function's
                // scope is gone; lookup sees only the enclosing namespaces.
                _ => continue,
            };
            self.scopes.push(s);
        }
        self.ctx = ctx;
        let r = f(self);
        self.scopes = saved.scopes;
        self.ctx = saved.ctx.unwrap_or(self.ast.tu);
        self.func = saved.func;
        self.unevaluated = saved.unevaluated;
        self.immediate = saved.immediate;
        self.pending_odr = saved.pending_odr;
        self.restore_immediate_state(imm);
        self.defining = saved.defining;
        self.enums = saved.enums;
        self.braced_args = saved.braced_args;
        self.prototype_params = saved.prototype_params;
        self.deferring_call_ref = saved.deferring_call_ref;
        self.deferred_call_refs = saved.deferred_call_refs;
        self.classes = saved.classes;
        self.cleanup = saved.cleanup;
        self.extend_collector = saved.extend_collector;
        self.range_for_outer_depth = outer_depth;
        self.nsdmi_this = saved.nsdmi_this;
        self.initializing = saved.initializing;
        self.templ.heads = saved.heads;
        self.templ.cur = saved.cur;
        self.templ.entering_context = saved.entering_context;
        self.templ.invented = saved.invented;
        self.templ.req_ctx = saved.req_ctx;
        self.ast.templates.dependent_depth = saved.dependent_depth;
        self.friend_tag = saved.friend_tag;
        r
    }

    /// The specialization of class template `tmpl` for `written`
    /// ([temp.spec.general], §4.2): the arguments are checked and
    /// converted, and the specialization found or created (undefined
    /// until a complete type is required).
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn get_class_spec(
        &mut self,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<DeclId, ()> {
        let tmpl = self.ast.first(tmpl);
        let conv = self.check_template_args(tmpl, written, arg_locs, loc)?;
        self.ensure_template_constraints(tmpl, conv.sugared, 0, loc)?;
        if let Some(s) = self.ast.templates.find_spec(tmpl, conv.canonical) {
            return Ok(s);
        }
        let tag = self
            .ast
            .template_info(tmpl)
            .and_then(|i| i.pattern)
            .and_then(|p| self.ast.decls.record(p))
            .map_or(TagKind::Struct, |r| r.tag);
        let spec = self.builder().class_spec(tmpl, conv.sugared, conv.canonical, tag);
        let print = self.suppress_default_args(tmpl, conv.sugared, written.len(), loc);
        if let Some((i, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(i, |s| s.print_args = print);
        }
        Ok(spec)
    }

    /// `args` without the trailing arguments equal to their parameter's
    /// default (clang's `SuppressDefaultTemplateArgs`): the arguments a
    /// specialization's name prints with. The first `written` came from
    /// the source; a default is recomputed for those silently (SFINAE).
    pub(crate) fn suppress_default_args(
        &mut self,
        tmpl: DeclId,
        args: ArgListId,
        written: usize,
        loc: Loc,
    ) -> ArgListId {
        let head = self.head_template(tmpl);
        let params = self.template_params(head);
        let elems: Vec<TemplateArg> = self.ast.types.args(args).to_vec();
        if elems.len() != params.len() {
            return args;
        }
        let depth = self.template_param_depth(head);
        let mut n = elems.len();
        while n > 0 {
            let p = params[n - 1];
            if self.ast.template_parm(p).is_none_or(|x| x.pack) {
                break;
            }
            let Some(def) = self.template_default(p) else { break };
            if n > written {
                n -= 1;
                continue;
            }
            if !self.push_inst_frame(FrameKind::DefaultArgChecking, p, tmpl, loc) {
                break;
            }
            if let Some(f) = self.inst.stack.frames.last_mut() {
                f.sfinae = Some(true);
            }
            self.inst.sfinae.borrow_mut().push(crate::templ::stack::SfinaeState::default());
            let r = self.default_template_arg(tmpl, depth, p, def, loc, &elems[..n - 1]);
            let failed = self.inst.sfinae.borrow_mut().pop().is_some_and(|s| s.errors > 0);
            self.pop_inst_frame();
            let types = &self.ast.types;
            match r {
                Ok(a) if !failed && types.canonical_arg(a) == types.canonical_arg(elems[n - 1]) => {
                    n -= 1;
                }
                _ => break,
            }
        }
        if n == elems.len() { args } else { self.ast.types.arg_list(&elems[..n]) }
    }

    /// The type `name<args>` naming a specialization of class template
    /// `tmpl` (§4.2): a `TemplateSpecializationType` sugaring the
    /// specialization's record type.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn class_template_type(
        &mut self,
        name: TemplateNameId,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<QualType, ()> {
        let spec = self.get_class_spec(tmpl, written, arg_locs, loc)?;
        let list = self.ast.types.arg_list(written);
        let rec = self.tag_type(spec);
        Ok(self.ast.types.template_specialization(name, list, Some(rec), false))
    }

    /// The type `name<args>` naming a specialization of alias template
    /// `tmpl` ([temp.alias]/2, §4.10): the aliased type with the
    /// arguments substituted, cached per canonical argument list.
    #[allow(clippy::result_unit_err, reason = "`Err` means already diagnosed")]
    pub fn alias_template_type(
        &mut self,
        name: TemplateNameId,
        tmpl: DeclId,
        written: &[TemplateArg],
        arg_locs: &[Loc],
        loc: Loc,
    ) -> Result<QualType, ()> {
        let tmpl = self.ast.first(tmpl);
        let conv = self.check_template_args(tmpl, written, arg_locs, loc)?;
        let list = self.ast.types.arg_list(written);
        if let Some(&u) = self.inst.alias_cache.get(&(tmpl, conv.canonical)) {
            return Ok(self.ast.types.template_specialization(name, list, Some(u), true));
        }
        let Some(pattern) = self.ast.template_info(tmpl).and_then(|i| i.pattern) else {
            return Err(());
        };
        let Some(u) = self.ast.decls.typedef(pattern).map(|t| t.underlying) else { return Err(()) };
        let depth = self.template_param_depth(tmpl);
        let mut args = Self::outer_levels(tmpl, depth);
        // clang 19 substitutes an alias with the canonical arguments: the
        // aliased type of `Ptr<I>` is `int *`, not `I *`.
        let subst_args = self.alias_subst_args(conv.sugared);
        args.push(subst_args, tmpl, LevelMode::Substitute);
        if !self.push_inst_frame(FrameKind::AliasTemplate, pattern, tmpl, loc) {
            return Err(());
        }
        // Names in the aliased type are looked up from the alias's own
        // context, not from the use ([temp.res.general]/1): `f<int>(t)`
        // in a member alias finds the class's `f` when the alias is used
        // outside the class.
        let ctx = self.ast.decls.get(tmpl).ctx().unwrap_or(self.ast.tu);
        let pairs = self.inst_context_pairs(ctx);
        let r = self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.map_contexts(&pairs);
            sub.ty(u, loc)
        });
        self.pop_inst_frame();
        let Ok(u) = r else { return Err(()) };
        let _ = self.inst.alias_cache.insert((tmpl, conv.canonical), u);
        Ok(self.ast.types.template_specialization(name, list, Some(u), true))
    }

    /// The arguments an alias template is substituted with: canonical,
    /// except that an injected-class-name argument stays one (clang's
    /// canonical `InjectedClassNameType` prints as `X<T>`, where kiln's
    /// canonical form is the dependent specialization).
    fn alias_subst_args(&mut self, sugared: ArgListId) -> ArgListId {
        let out: Vec<TemplateArg> = self
            .ast
            .types
            .args(sugared)
            .to_vec()
            .into_iter()
            .map(|a| match a.view() {
                ArgView::Type(q) if q.quals().is_empty() => {
                    let mut t = q;
                    loop {
                        if matches!(self.ast.types.view(t.ty()), TypeView::InjectedClassName { .. })
                        {
                            break TemplateArg::ty(t);
                        }
                        let n = self.ast.types.desugar_once(t);
                        if n == t {
                            break self.ast.types.canonical_arg(a);
                        }
                        t = n;
                    }
                }
                // A dependent expression stays one: its canonical form
                // (a profile) cannot be substituted.
                ArgView::Expr { profile: Some(_), .. } => a,
                _ => self.ast.types.canonical_arg(a),
            })
            .collect();
        self.ast.types.arg_list(&out)
    }

    /// The class template specialization, or member class of one, that
    /// `q` names (through sugar and arrays), if any and not yet defined.
    pub(crate) fn undefined_spec(&self, q: QualType) -> Option<DeclId> {
        let mut t = q;
        while let Some(e) = self.ast.types.element(t) {
            if !self.ast.types.is_array(t) {
                break;
            }
            t = e;
        }
        let r = self.ast.types.as_record(t)?;
        if self.ast.spec_info(r).is_none() && self.ast.instantiated_from(r).is_none() {
            return None;
        }
        self.ast.definition(r).is_none().then_some(r)
    }

    /// Whether `q` is complete at `loc`, first instantiating the class
    /// template specialization it names ([temp.inst]/2: a complete type
    /// is required). Errors in the instantiation are diagnosed.
    pub fn require_complete(&mut self, q: QualType, loc: Loc) -> bool {
        if let Some(r) = self.undefined_spec(q) {
            let _ = self.instantiate_class(r, loc, true);
        }
        self.is_complete(q)
    }

    /// As [`Sema::require_complete`], but an undefined template is not
    /// diagnosed (the caller reports the incomplete type).
    pub fn require_complete_silent(&mut self, q: QualType, loc: Loc) -> bool {
        if let Some(r) = self.undefined_spec(q) {
            let _ = self.instantiate_class(r, loc, false);
        }
        self.is_complete(q)
    }

    /// Instantiates class `d` at `loc` if it is an undefined class
    /// template specialization or member class of one ([temp.inst]/2;
    /// clang's `RequireCompleteType` in `TryConstructorInitialization`,
    /// `isCompleteType` in `TryUserDefinedConversion`), before an
    /// initialization looks at its constructors. An undefined template is
    /// not diagnosed: the caller reports the incomplete class.
    pub(crate) fn instantiate_for_init(&mut self, d: DeclId, loc: Loc) {
        if (self.ast.spec_info(d).is_some() || self.ast.instantiated_from(d).is_some())
            && self.ast.definition(d).is_none()
        {
            let _ = self.instantiate_class(d, loc, false);
        }
    }

    /// A class named in a nested-name-specifier must be complete
    /// ([expr.prim.id.qual]/1); false after a diagnostic.
    pub fn require_complete_nns(&mut self, q: QualType, loc: Loc) -> bool {
        if let Some(r) = self.undefined_spec(q) {
            return self.instantiate_class(r, loc, true);
        }
        if let Some(r) = self.ast.types.as_record(q)
            && !self.is_complete(q)
            // The class being defined may be named: lookup finds the
            // members declared so far ([class.mem.general]/8; clang's
            // `RequireCompleteDeclContext`), e.g. `C::duration` in a
            // default template argument substituted inside `C`.
            && !self.defining.iter().any(|&x| self.ast.first(x) == self.ast.first(r))
        {
            let d = self.diag(DiagId::sema_incomplete_nested_name_spec, loc).arg(self.ty_arg(q));
            self.emit(d);
            return false;
        }
        true
    }

    /// The arguments of specialization `spec` by depth (§5.1).
    pub(crate) fn spec_args(&self, spec: DeclId) -> Option<(DeclId, MultiLevelArgs)> {
        let (_, info) = self.ast.spec_info(spec)?;
        let tmpl = info.template;
        let mut args = self.base_levels(tmpl);
        // Instantiated from a partial specialization: its deduced
        // arguments ([temp.spec.partial.match]/2, [temp.inst]/2).
        match (info.from_args, self.ast.partial_spec_info(info.from)) {
            (Some(a), Some((_, p))) => args.push(a, p.partial, LevelMode::Substitute),
            _ => args.push(info.args, tmpl, LevelMode::Substitute),
        }
        Some((tmpl, args))
    }

    /// The template whose pattern instantiates the specializations of
    /// `tmpl`: the member template of a class template `tmpl` was
    /// instantiated from ([temp.mem]; its pattern is never substituted
    /// on its own, as clang's `getTemplateInstantiationPattern`), else
    /// `tmpl`.
    pub(crate) fn inst_template(&self, tmpl: DeclId) -> DeclId {
        self.ast
            .templates
            .instantiated_from_map(tmpl)
            .filter(|&t| kiln_ast::templates::is_template_kind(self.kind_of(t)))
            .unwrap_or(tmpl)
    }

    /// The argument levels enclosing `tmpl`'s own parameters: those of
    /// the specialization `tmpl` is a member of when it is an
    /// instantiated member template ([temp.mem]), else retained.
    pub(crate) fn base_levels(&self, tmpl: DeclId) -> MultiLevelArgs {
        if self.inst_template(tmpl) != tmpl {
            let mut c = Some(self.inst_lexical_class(tmpl));
            while let Some(x) = c.filter(|&x| self.kind_of(x) == DeclKind::CXXRecord) {
                if let Some((_, a)) = self.spec_args(x) {
                    return a;
                }
                c = self.ast.decls.get(x).ctx();
            }
        }
        Self::outer_levels(tmpl, self.template_param_depth(self.head_template(tmpl)))
    }

    /// The context of instantiated member template `tmpl` whose
    /// specialization arguments enclose its own: its class, or for a
    /// friend template defined in a class template, the class that
    /// declared it (its lexical context; [temp.friend]/1).
    pub(crate) fn inst_lexical_class(&self, tmpl: DeclId) -> DeclId {
        let ctx = self.ast.decls.get(tmpl).ctx().unwrap_or(self.ast.tu);
        if self.kind_of(ctx) != DeclKind::CXXRecord
            && let Some(l) = self.ast.decls.lexical_ctx(tmpl)
            && self.kind_of(l) == DeclKind::CXXRecord
        {
            return l;
        }
        ctx
    }

    /// The context whose instantiated classes map the pattern's classes
    /// for function template specialization `spec` of `tmpl`
    /// ([`Self::inst_context_pairs`]): its own, or the class that
    /// declared it for a friend template defined in a class template.
    pub(crate) fn spec_pairs_ctx(&self, spec: DeclId, tmpl: DeclId) -> DeclId {
        if self.inst_template(tmpl) != tmpl {
            return self.inst_lexical_class(tmpl);
        }
        self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu)
    }

    /// The instantiated classes from `x` outwards (stopping at the first
    /// context that is not a class), each paired with the pattern class
    /// it was instantiated from: member lookups into the patterns go to
    /// the instantiations (§5.6).
    pub(crate) fn inst_context_pairs(&self, x: DeclId) -> Vec<(DeclId, DeclId)> {
        let mut out = Vec::new();
        let mut c = Some(x);
        while let Some(r) = c.filter(|&r| self.kind_of(r) == DeclKind::CXXRecord) {
            let p = match self.ast.spec_info(r) {
                Some((_, s)) => self.spec_pattern_class(r, s.template),
                None => self.ast.instantiated_from(r),
            };
            if let Some(p) = p {
                out.push((p, r));
            }
            if out.len() > 1024 {
                break;
            }
            c = self.ast.decls.get(r).ctx();
        }
        out
    }

    /// Instantiates the definition of class template specialization
    /// `spec` ([temp.inst]/2-3, §4.3): the member declarations are
    /// substituted, the member definitions are not (§4.4). `poi` is the
    /// point of instantiation. False when the specialization cannot be
    /// defined (diagnosed if `diagnose`).
    pub fn instantiate_class(&mut self, spec: DeclId, poi: Loc, diagnose: bool) -> bool {
        if self.ast.definition(spec).is_some() {
            return true;
        }
        if self.defining.contains(&spec) || self.inst.fatal {
            return false;
        }
        if self.ast.spec_info(spec).is_none() {
            return self.instantiate_member_class(spec, poi);
        }
        let Some((tmpl, mut args)) = self.spec_args(spec) else { return false };
        let kind = self.ast.spec_info(spec).map(|(_, s)| s.kind);
        if self.ast.decls.get(spec).has(DeclFlags::INVALID) {
            self.inst.undefined_diagnosed.set(Some(spec));
            return false;
        }
        if kind == Some(SpecKind::ExplicitSpecialization) {
            // Declared, never defined ([temp.expl.spec]/1): the caller
            // reports the incomplete type.
            self.inst.undefined_diagnosed.set(None);
            return false;
        }
        // [temp.spec.partial.match]: the most specialized matching
        // partial specialization, else the primary template.
        let Ok(matched) = self.match_partial_specs(tmpl, spec, poi) else {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            self.inst.undefined_diagnosed.set(Some(spec));
            return false;
        };
        let pattern = match matched {
            Some(m) => {
                args = self.base_levels(tmpl);
                args.push(m.args, m.partial, LevelMode::Substitute);
                // A partial specialization redeclared with its member
                // template ([`Substituter::member_partial_spec`]) is
                // defined by the original.
                Some(self.ast.templates.instantiated_from_map(m.partial).unwrap_or(m.partial))
            }
            None => self.pattern_of(self.inst_template(tmpl)),
        };
        let Some(pattern) = pattern else {
            return false;
        };
        let Some(def) = self.ast.definition(pattern) else {
            if let Some(m) = matched
                && let Some((i, _)) = self.ast.spec_info(spec)
            {
                // Remembered for the explicit-instantiation diagnostic.
                self.ast.templates.update_spec(i, |s| {
                    s.from = m.partial;
                    s.from_args = Some(m.args);
                });
            }
            if diagnose {
                let t = self.tag_type(spec);
                let at = if matched.is_some() { pattern } else { tmpl };
                let note = self.diag(DiagId::note_template_decl_here, self.ast.decls.get(at).loc());
                let d = self
                    .diag(DiagId::sema_template_implicit_instantiate_undefined, poi)
                    .arg(self.decl_ty_str(t))
                    .with_note(note);
                self.emit(d);
                self.inst.undefined_diagnosed.set(Some(spec));
            } else {
                self.inst.undefined_diagnosed.set(None);
            }
            return false;
        };
        if !self.push_inst_frame(FrameKind::ClassInstantiation, spec, tmpl, poi) {
            return false;
        }
        if let Some((i, _)) = self.ast.spec_info(spec) {
            let from_args = matched.map(|m| m.args);
            self.ast.templates.update_spec(i, |s| {
                if s.kind == SpecKind::Undeclared {
                    s.kind = SpecKind::ImplicitInstantiation;
                }
                s.from = def;
                s.from_args = from_args;
                if s.point_of_instantiation == Loc::INVALID {
                    s.point_of_instantiation = poi;
                }
            });
        }
        let parent = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let outer = self.inst_context_pairs(parent);
        self.in_inst_context(parent, |s| s.instantiate_class_body(spec, def, &args, &outer));
        self.pop_inst_frame();
        self.ast.definition(spec).is_some()
    }

    /// Instantiates the definition of `r`, a member class of a class
    /// template specialization, from its pattern ([temp.inst]/3: a member
    /// class is declared with its enclosing class and defined when a
    /// complete type is required). False when the pattern has no
    /// definition (the caller reports the incomplete type).
    fn instantiate_member_class(&mut self, r: DeclId, poi: Loc) -> bool {
        let Some(pattern) = self.ast.instantiated_from(r) else { return false };
        let Some(def) = self.ast.definition(pattern) else { return false };
        // The enclosing instantiated classes, innermost first, up to the
        // specialization whose arguments the member is substituted with.
        let mut outer = Vec::new();
        let mut args = None;
        let mut c = self.ast.decls.get(r).ctx();
        while let Some(x) = c.filter(|&x| self.kind_of(x) == DeclKind::CXXRecord) {
            if let Some(p) = self.ast.instantiated_from(x) {
                outer.push((p, x));
            }
            if let Some((_, a)) = self.spec_args(x) {
                args = Some(a);
                break;
            }
            c = self.ast.decls.get(x).ctx();
        }
        let Some(args) = args else { return false };
        if !self.push_inst_frame(FrameKind::MemberClass, r, pattern, poi) {
            return false;
        }
        let parent = self.ast.decls.get(r).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(parent, |s| s.instantiate_class_body(r, def, &args, &outer));
        self.pop_inst_frame();
        self.ast.definition(r).is_some()
    }

    /// Defines `spec` from pattern definition `def`. `outer` maps the
    /// patterns of the enclosing classes of a member class to their
    /// instantiations.
    fn instantiate_class_body(
        &mut self,
        spec: DeclId,
        def: DeclId,
        args: &MultiLevelArgs,
        outer: &[(DeclId, DeclId)],
    ) {
        let mut sub = Substituter::new(self, args);
        sub.entity = Some(spec);
        sub.map_contexts(outer);
        sub.class_body(spec, def, None);
    }

    /// Instantiates the definition of `f`, a member function of a class
    /// template specialization (or a friend defined in one), from its
    /// pattern ([temp.inst]/4-5, §4.4). Nothing happens if `f` is already
    /// defined or its pattern has no definition (a link-time reference).
    pub fn instantiate_function_definition(&mut self, f: DeclId, poi: Loc) {
        if self.ast.definition(f).is_some() || self.inst.fatal {
            return;
        }
        // A use inside a ctor-initializer (`B<int>(3)` calling a
        // constructor instantiated here) must not clobber the enclosing
        // constructor's mem-initializer state.
        let outer = std::mem::take(&mut self.mem_inits);
        self.instantiate_function_definition_inner(f, poi);
        self.mem_inits = outer;
    }

    fn instantiate_function_definition_inner(&mut self, f: DeclId, poi: Loc) {
        let Some(pattern) = self.ast.instantiated_from(f) else { return };
        let Some(pdef) = self.ast.definition(pattern) else { return };
        let Some(body) = self.ast.decls.function(pdef).and_then(|x| x.body) else { return };
        if self.is_function_spec(f) {
            // A function template specialization (WP-071, §4.4).
            self.instantiate_function_spec_definition(f, pdef, body, poi);
            return;
        }
        // The specialization whose member `f` is: its semantic context
        // (a friend: the class that declared it).
        let Some(spec) = self.inst_owner(f) else { return };
        let Some((tmpl, args)) = self.spec_args(spec) else { return };
        let Some(pclass) = self.spec_pattern_class(spec, tmpl) else { return };
        let mut pairs = self.inst_context_pairs(spec);
        if pairs.first().is_none_or(|&(_, x)| x != spec) {
            pairs.insert(0, (pclass, spec));
        }
        if !self.push_inst_frame(FrameKind::MemberFunction, f, pattern, poi) {
            return;
        }
        if let Some((i, _)) = self.ast.member_spec_info(f) {
            self.ast.templates.update_member_spec(i, |m| {
                if m.point_of_instantiation == Loc::INVALID {
                    m.point_of_instantiation = poi;
                }
            });
        }
        let ctx = self.ast.decls.get(f).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            sub.function_def(f, pdef, body);
        });
        self.pop_inst_frame();
    }

    /// A reference to variable `v` at `loc`: a static data member of a
    /// specialization whose initializer was deferred gets it now
    /// ([temp.inst]/4; clang instantiates at the reference, before the
    /// value is needed). An unevaluated reference instantiates only a
    /// variable usable in constant expressions. The out-of-line
    /// definition of a non-inline one is instantiated here when the
    /// variable may be usable in constant expressions, else at the end
    /// of the translation unit ([temp.point]/7, as clang).
    pub(crate) fn note_var_referenced(&mut self, v: DeclId, loc: Loc) {
        let deferred = self.inst.deferred_inits.contains(&v);
        if !deferred && !self.inst.member_defs.contains(&v) {
            return;
        }
        let constant = self.might_be_constant_var(v);
        if self.unevaluated > 0 && !constant {
            return;
        }
        if deferred {
            self.instantiate_static_member_init(v, loc);
        } else if constant {
            self.instantiate_static_member_def(v, loc);
        } else {
            self.queue_instantiation(v, loc);
        }
    }

    /// clang's `mightBeUsableInConstantExpressions`: `constexpr`, or a
    /// `const` non-volatile integral, enumeration or reference variable
    /// ([expr.const]/4, before its initializer is known).
    fn might_be_constant_var(&self, v: DeclId) -> bool {
        if self.ast.decls.var(v).is_some_and(|x| x.spec.is_constexpr()) {
            return true;
        }
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_reference(ty) {
            return true;
        }
        let q = self.ast.types.effective_quals(ty);
        q.is_const() && !q.is_volatile() && self.is_integral_or_enum(ty)
    }

    /// Instantiates the definition of non-inline static data member `v`
    /// of a specialization from its pattern's out-of-line definition
    /// ([temp.inst]/4, [class.static.data]/3): a redeclaration of `v`
    /// in the namespace of that definition, initialized as it is. Nothing
    /// happens while the pattern has no out-of-line definition (a later
    /// one is found at the next reference or at the end of the TU).
    pub fn instantiate_static_member_def(&mut self, v: DeclId, poi: Loc) {
        if self.inst.fatal || !self.inst.member_defs.contains(&v) {
            return;
        }
        if self.is_var_spec(v) {
            self.instantiate_var_spec_init(v, poi);
            return;
        }
        let Some(pattern) = self.ast.instantiated_from(v) else { return };
        let Some(pdef) = self.ast.redecls_of(pattern).find(|&x| {
            self.kind_of(x) == DeclKind::Var
                && !self.is_in_class_static(x)
                && !self.ast.decls.get(x).has(DeclFlags::INVALID)
        }) else {
            return;
        };
        self.inst.member_defs.remove(&v);
        let Some(spec) = self.inst_owner(v) else { return };
        let Some((tmpl, args)) = self.spec_args(spec) else { return };
        let Some(pclass) = self.spec_pattern_class(spec, tmpl) else { return };
        let mut pairs = self.inst_context_pairs(spec);
        if pairs.first().is_none_or(|&(_, x)| x != spec) {
            pairs.insert(0, (pclass, spec));
        }
        if !self.push_inst_frame(FrameKind::StaticDataMember, v, pdef, poi) {
            return;
        }
        let pn = *self.ast.decls.get(pdef);
        let lexical = self.ast.decls.lexical_ctx(pdef).unwrap_or(self.ast.tu);
        let ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let vspec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let pspec = self.ast.decls.var(pdef).map(|x| x.spec).unwrap_or_default();
        let spec_bits = pspec
            .with_storage(kiln_ast::decl::VarStorage::None)
            .with_inline(vspec.is_inline())
            .with_constexpr(vspec.is_constexpr());
        let mut h = kiln_ast::DeclHead::new(spec, pn.name(), pn.loc());
        h.lexical = Some(lexical);
        let d = self.builder().var(h, ty, spec_bits, Some(v));
        self.ast.templates.set_instantiated_from(d, pdef);
        self.in_inst_context(spec, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            // [class.static.data]/3: the initializer is in the scope of
            // the class.
            sub.sema.initializing = Some(d);
            // A failure was diagnosed; the variable is left uninitialized.
            let _ = sub.var_init(pdef, d);
        });
        self.pop_inst_frame();
    }

    /// Instantiates the deferred initializer of static data member `v`.
    pub fn instantiate_static_member_init(&mut self, v: DeclId, poi: Loc) {
        if self.inst.fatal || !self.inst.deferred_inits.remove(&v) {
            return;
        }
        let Some(pattern) = self.ast.instantiated_from(v) else { return };
        let Some(spec) = self.inst_owner(v) else { return };
        let Some((tmpl, args)) = self.spec_args(spec) else { return };
        let Some(pclass) = self.spec_pattern_class(spec, tmpl) else { return };
        let mut pairs = self.inst_context_pairs(spec);
        if pairs.first().is_none_or(|&(_, x)| x != spec) {
            pairs.insert(0, (pclass, spec));
        }
        if !self.push_inst_frame(FrameKind::StaticDataMember, v, pattern, poi) {
            return;
        }
        self.ast.decls.mark_var(v, kiln_ast::decl::VarSpec::new().with_inline(true));
        let ctx = self.ast.decls.get(v).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(ctx, |s| {
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(spec);
            sub.map_contexts(&pairs);
            // A failure was diagnosed; the variable is left uninitialized.
            let _ = sub.var_init(pattern, v);
        });
        self.pop_inst_frame();
    }

    /// The class definition specialization `spec` of `tmpl` was
    /// instantiated from: the matched partial specialization's
    /// ([temp.spec.partial.match]) or the primary template's.
    pub(crate) fn spec_pattern_class(&self, spec: DeclId, tmpl: DeclId) -> Option<DeclId> {
        let from = self.ast.spec_info(spec).map(|(_, s)| s.from);
        if let Some(from) = from.filter(|&f| self.ast.partial_spec_info(f).is_some()) {
            return self.ast.definition(from);
        }
        self.ast.definition(self.pattern_of(self.inst_template(tmpl)).unwrap_or(spec))
    }

    /// The class template specialization member `f` was instantiated
    /// with.
    pub(crate) fn inst_owner(&self, f: DeclId) -> Option<DeclId> {
        let mut c = self.ast.decls.lexical_ctx(f).or_else(|| self.ast.decls.get(f).ctx());
        while let Some(x) = c {
            if self.ast.spec_info(x).is_some() {
                return Some(x);
            }
            c = self.ast.decls.get(x).ctx();
        }
        None
    }

    /// `f`, a function, was odr-used at `loc` ([temp.inst]/5): a member
    /// of a class template specialization is instantiated now when it
    /// is constexpr (constant evaluation may need it), else queued for
    /// the end of the translation unit ([temp.point]/7).
    pub(crate) fn note_used_for_instantiation(&mut self, f: DeclId, loc: Loc) {
        if self.ast.definition(f).is_some() || self.ast.instantiated_from(f).is_none() {
            return;
        }
        if !matches!(
            self.kind_of(f),
            DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        ) {
            return;
        }
        let constexpr = self.ast.decls.function(f).is_some_and(|x| {
            x.spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None
                || self.is_undeduced_auto(
                    self.ast
                        .types
                        .proto(self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR))
                        .map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret),
                )
        });
        if constexpr {
            // Instantiated now, so the evaluator never has to (clang's
            // `MarkFunctionReferenced`); if its pattern is not defined
            // yet, it is retried at end of TU (and stays undefined for
            // constant evaluation, spec/sema-templates.md §4.11).
            self.instantiate_function_definition(f, loc);
            if self.ast.definition(f).is_none() {
                self.queue_instantiation(f, loc);
            }
        } else {
            self.queue_instantiation(f, loc);
        }
    }
}

impl Substituter<'_, '_, '_> {
    /// Defines member function `f` from its pattern definition `pdef`
    /// with body `body` ([temp.inst]/4-5): the parameters are mapped, a
    /// constructor's ctor-initializer is substituted, then the body.
    pub(crate) fn function_def(&mut self, f: DeclId, pdef: DeclId, body: kiln_ast::StmtId) {
        let sub = self;
        sub.sema.act_on_start_function_def(f);
        let pp: Vec<DeclId> = sub
            .sema
            .ast
            .decls
            .function(pdef)
            .map(|x| sub.sema.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        let fp: Vec<DeclId> = sub
            .sema
            .ast
            .decls
            .function(f)
            .map(|x| sub.sema.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        let lbrace = sub.sema.ast.stmts.get(body).loc();
        let rbrace =
            if sub.sema.ast.stmts.get(body).kind() == kiln_ast::stmt::StmtKind::CompoundStmt {
                sub.sema.ast.stmts.rbrace(body)
            } else {
                lbrace
            };
        sub.map_params(&pp, &fp);
        if sub.sema.kind_of(f) == DeclKind::CXXConstructor {
            // The pattern's ctor-initializer, substituted and checked
            // as the parser's would be (spec/sema-class.md §4.9).
            let (inits, colon) =
                sub.sema.inst.pattern_mem_inits.get(&pdef).cloned().unwrap_or((Vec::new(), lbrace));
            for info in &inits {
                match sub.mem_initializer(info) {
                    Ok(v) => {
                        for i in &v {
                            sub.sema.act_on_mem_initializer(f, i);
                        }
                    }
                    Err(_) => sub.sema.note_failed_mem_initializer(f),
                }
            }
            sub.sema.act_on_mem_initializers(f, &[], colon);
        }
        let b = sub.body(body);
        sub.sema.act_on_finish_function_def(f, b, rbrace);
    }

    /// The base-specifiers of pattern definition `def`, substituted
    /// ([temp.inst]/3: the base-clause is instantiated with the class;
    /// [temp.variadic]/5: a pack expansion gives one base per element).
    fn class_bases(&mut self, def: DeclId) -> Vec<BaseSpecInfo> {
        // Base classes ([temp.inst]/3: the base-clause is instantiated
        // with the class).
        let bases: Vec<kiln_ast::decl::BaseSpec> =
            self.sema.ast.decls.bases(def, &self.sema.ast.lists).copied().collect();
        let mut specs = Vec::with_capacity(bases.len());
        for b in bases {
            if b.pack_expansion {
                // [temp.variadic]/5: one base per element.
                let tys = match self.expand_pattern(|s| s.element_ty(b.ty, b.loc)) {
                    Ok(crate::templ::pack::Expansion::Elements(v)) => v,
                    Ok(crate::templ::pack::Expansion::Unexpanded(_)) => {
                        let _ = self.unimplemented::<()>("base pack expansion", b.loc);
                        continue;
                    }
                    Err(_) => continue,
                };
                for ty in tys {
                    specs.push(BaseSpecInfo {
                        ty,
                        loc: b.loc,
                        is_virtual: b.is_virtual,
                        access: b.access,
                        pack_expansion: false,
                        attrs: Vec::new(),
                    });
                }
                continue;
            }
            if let Ok(ty) = self.ty(b.ty, b.loc) {
                specs.push(BaseSpecInfo {
                    ty,
                    loc: b.loc,
                    is_virtual: b.is_virtual,
                    access: b.access,
                    pack_expansion: false,
                    attrs: Vec::new(),
                });
            }
        }
        specs
    }

    /// Defines `spec` from pattern definition `def` (bases, members,
    /// default member initializers). `local` is set for a local class
    /// ([class.local], [temp.inst]/3: instantiated with its enclosing
    /// function): its member classes are defined with it, and the member
    /// functions it defines are collected there, to be defined after the
    /// outermost local class is complete (as the parser replays them);
    /// otherwise the class contexts are finished here.
    pub(crate) fn class_body(
        &mut self,
        spec: DeclId,
        def: DeclId,
        local: Option<&mut Vec<LocalMethod>>,
    ) {
        self.class_body_with(spec, def, local, false);
    }

    /// [`Substituter::class_body`]; `anonymous` for an anonymous struct
    /// or union (its `}` is followed by `;`).
    pub(crate) fn class_body_with(
        &mut self,
        spec: DeclId,
        def: DeclId,
        mut local: Option<&mut Vec<LocalMethod>>,
        anonymous: bool,
    ) {
        let saved_entity = self.entity.replace(spec);
        self.sema.act_on_start_record_definition(spec);
        self.class_attrs(spec, def);
        self.map_contexts(&[(def, spec)]);
        let sub = self;
        let specs = sub.class_bases(def);
        sub.sema.act_on_base_specifiers(spec, &specs);
        let members: Vec<DeclId> = sub.sema.ast.decls.members(def).collect();
        let mut nsdmis = Vec::new();
        for m in members {
            let n = *sub.sema.ast.decls.get(m);
            if n.flags().contains(DeclFlags::IMPLICIT) {
                continue;
            }
            // A friend function defined in the class is a member of the
            // list as well as its `FriendDecl`'s target; the
            // `FriendDecl` redeclares it.
            if n.kind() == DeclKind::Function
                && n.misc().friend() != kiln_ast::decl::FriendKind::None
            {
                continue;
            }
            if n.kind() == DeclKind::IndirectField {
                // Injected by its anonymous struct or union.
                continue;
            }
            if local.is_none() && sub.is_unnamed_member_class(m) {
                let _ = sub.unnamed_member_class(m);
                continue;
            }
            let r = if local.is_some()
                && n.kind() == DeclKind::CXXRecord
                && n.misc().friend() == kiln_ast::decl::FriendKind::None
            {
                Ok(Some(sub.local_class(m, local.as_deref_mut())))
            } else {
                sub.member(m)
            };
            if let Ok(Some(x)) = r {
                sub.member_link(x, m);
                if let Some(l) = local.as_deref_mut() {
                    sub.collect_local_method(spec, x, m, l);
                }
                if n.kind() == DeclKind::Field
                    && let Some(init) = sub.sema.ast.decls.field(m).and_then(|f| f.init)
                {
                    nsdmis.push((x, init));
                }
            }
        }
        sub.sema.act_on_finish_record_definition(spec, anonymous);
        // Default member initializers: in the complete-class context
        // (clang instantiates them when a constructor uses them; a gap).
        if !nsdmis.is_empty() {
            sub.sema.act_on_start_delayed_class(spec);
            for (field, init) in nsdmis {
                if sub.sema.kind_of(field) != DeclKind::Field {
                    continue;
                }
                sub.sema.act_on_start_field_init(field);
                let i = if sub.sema.ast.exprs.get(init).kind() == ExprKind::InitListExpr {
                    sub.init_list(init).map(Initializer::List)
                } else {
                    sub.expr(init).map(Initializer::Expr)
                };
                match i {
                    Ok(i) => sub.sema.act_on_field_init(field, i),
                    Err(SubstFail) => {
                        if sub.sema.nsdmi_this.last().is_some_and(|&(f, _)| f == field) {
                            sub.sema.nsdmi_this.pop();
                        }
                    }
                }
            }
            sub.sema.act_on_finish_delayed_class(spec);
        }
        if local.is_none() {
            sub.sema.act_on_finish_class_contexts(spec);
        }
        sub.entity = saved_entity;
    }

    /// A member class is declared with its class; its definition is
    /// instantiated when it is required to be complete ([temp.inst]/3).
    pub(crate) fn member_class_decl(&mut self, m: DeclId) -> Result<DeclId, SubstFail> {
        let n = *self.sema.ast.decls.get(m);
        let loc = n.loc();
        let Some(name) = n.name().as_ident().filter(|_| !n.name().is_empty()) else {
            return self.unimplemented("anonymous member classes of class templates", loc);
        };
        let kw = match self.sema.ast.decls.record(m).map(|r| r.tag) {
            Some(TagKind::Class) => TagKeyword::Class,
            Some(TagKind::Union) => TagKeyword::Union,
            _ => TagKeyword::Struct,
        };
        Ok(self.sema.act_on_tag(kw, Some((name, loc)), None, TagUse::Declaration, 0, None, loc))
    }

    /// A class-scope using-declaration of the pattern (`using B<T>::f;`,
    /// `using __base::__base;`), redone in the instantiation with its
    /// qualifier substituted (WP-114).
    fn member_using(&mut self, m: DeclId, loc: Loc) -> Result<Option<DeclId>, SubstFail> {
        let pnns = self.sema.ast.using_qualifier(m).ok_or(SubstFail)?;
        let dn = self.sema.ast.decls.get(m).name();
        // An operator- or conversion-function-id (`using B<T>::operator=;`)
        // is kept, a conversion's target type substituted; a constructor
        // name (or the base's own name) inherits constructors.
        let (name, inheriting) = match self.sema.ast.names.view(dn) {
            DeclNameView::Identifier(n) => (dn, self.sema.nns_last_name(pnns) == Some(n)),
            DeclNameView::Operator(_) | DeclNameView::LiteralOperator(_) => (dn, false),
            DeclNameView::Conversion(t) => {
                let t = self.ty(t, loc)?;
                let t = self.sema.ast.types.canonical(t);
                (self.sema.ast.names.conversion(t), false)
            }
            _ => (DeclName::ident(self.sema.nns_last_name(pnns).ok_or(SubstFail)?), true),
        };
        let packed = self
            .sema
            .ast
            .types
            .nns_flags(pnns)
            .contains(kiln_ast::types::TypeFlags::UNEXPANDED_PACK);
        let nnss = if packed {
            // `using Bases::operator()...;` ([temp.variadic]/5): one
            // using-declarator per element.
            match self.expand_pattern(|s| s.nns(pnns, loc))? {
                crate::templ::pack::Expansion::Elements(v) => v,
                crate::templ::pack::Expansion::Unexpanded(_) => {
                    return self.unimplemented("this using-declaration pack expansion", loc);
                }
            }
        } else {
            vec![self.nns(pnns, loc)?]
        };
        let mut last = None;
        for nns in nnss {
            if self.nns_still_dependent(nns) {
                return self.unimplemented("dependent using-declarations in templates", loc);
            }
            let ctx = self.nns_context(nns, loc)?;
            let q = crate::using::Qualifier { ctx, nns, loc };
            last = self.sema.instantiate_class_using(q, name, loc, inheriting).or(last);
        }
        Ok(last)
    }

    /// Replays member `m` of the pattern class (§4.6).
    fn member(&mut self, m: DeclId) -> Result<Option<DeclId>, SubstFail> {
        let n = *self.sema.ast.decls.get(m);
        let loc = n.loc();
        // A typedef member diagnosed as a redefinition in the pattern is
        // not instantiated (clang skips invalid members), so it is not
        // diagnosed again.
        if matches!(n.kind(), DeclKind::Typedef | DeclKind::TypeAlias) && n.has(DeclFlags::INVALID)
        {
            return Ok(None);
        }
        Ok(Some(match n.kind() {
            DeclKind::Field => self.field_decl(m)?,
            DeclKind::Var if self.defers_init(m) => self.deferred_static_member(m)?,
            DeclKind::Var => {
                let v = self.var_decl(m, DeclaratorContext::Member)?;
                let spec = self.sema.ast.decls.var(v).map(|x| x.spec);
                if self.sema.kind_of(v) == DeclKind::Var
                    && spec.is_some_and(|x| {
                        x.storage() == kiln_ast::decl::VarStorage::Static && !x.is_inline()
                    })
                {
                    self.sema.inst.member_defs.insert(v);
                }
                v
            }
            DeclKind::CXXMethod
            | DeclKind::CXXConstructor
            | DeclKind::CXXDestructor
            | DeclKind::CXXConversion => {
                // An out-of-line redeclaration is not a member of the
                // class's member list; only the first declaration is.
                self.function_decl(m, DeclaratorContext::Member, None)?
            }
            DeclKind::Typedef | DeclKind::TypeAlias => self.typedef_decl(m)?,
            DeclKind::StaticAssert => {
                self.static_assert_decl(m)?;
                return Ok(None);
            }
            DeclKind::Enum => self.enum_decl(m)?,
            DeclKind::AccessSpec => {
                let a = n.misc().access();
                let _ = self.sema.act_on_access_spec(a, loc);
                return Ok(None);
            }
            DeclKind::Friend => {
                let r = self.friend_decl(m)?;
                return Ok(r.filter(|&x| self.sema.kind_of(x) != DeclKind::Friend));
            }
            // A class-scope using-declaration is redone with its
            // substituted qualifier; its shadows come with it.
            DeclKind::Using | DeclKind::UnresolvedUsingValue => return self.member_using(m, loc),
            DeclKind::Empty | DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow => {
                return Ok(None);
            }
            // A friend function is in its class's member list as well as
            // in its FriendDecl, which instantiates it.
            // So is a friend template ([temp.friend]/1) or a class a
            // friend declaration introduced (`template<class> friend
            // class X;`).
            DeclKind::Function
            | DeclKind::FunctionTemplate
            | DeclKind::ClassTemplate
            | DeclKind::CXXRecord
                if n.misc().friend() != kiln_ast::decl::FriendKind::None =>
            {
                return Ok(None);
            }
            DeclKind::CXXRecord if self.sema.ast.partial_spec_info(m).is_some() => {
                self.member_partial_spec(m)?;
                return Ok(None);
            }
            DeclKind::CXXRecord => self.member_class_decl(m)?,
            DeclKind::ClassTemplate
            | DeclKind::FunctionTemplate
            | DeclKind::VarTemplate
            | DeclKind::TypeAliasTemplate => self.member_template(m)?,
            k => return self.unimplemented(&format!("{k:?} members of class templates"), loc),
        }))
    }
}
