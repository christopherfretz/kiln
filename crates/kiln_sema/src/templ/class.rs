//! Class template specializations (spec/sema-templates.md §4): naming
//! one (`get_class_spec`), instantiating its definition when a complete
//! type is required (`instantiate_class`, [temp.inst]/2), alias template
//! specializations (§4.10), and the definitions of members, which are
//! instantiated when used (`instantiate_function_definition`,
//! [temp.inst]/4).

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, TagKind};
use kiln_ast::expr::ExprKind;
use kiln_ast::ids::{ArgListId, TemplateNameId};
use kiln_ast::names::{DeclName, DeclNameView};
use kiln_ast::templates::SpecKind;
use kiln_ast::types::{ArgView, TagKeyword, TemplateArg, TemplateName, TypeFlags, TypeView};
use kiln_ast::{AnnId, DeclId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::class::BaseSpecInfo;
use crate::decl::TagUse;
use crate::declspec::DeclaratorContext;
use crate::init::Initializer;
use crate::scope::{Scope, ScopeKind};
use crate::templ::args::{ConvertedArgs, LevelMode, MultiLevelArgs};
use crate::templ::stack::FrameKind;
use crate::templ::subst::{LocalLate, LocalMethod, SubstFail, SubstResult, Substituter};

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
    potential_caps: Vec<kiln_ast::ExprId>,
    defining: Vec<DeclId>,
    enums: Vec<crate::decl::EnumBuild>,
    braced_args: Vec<(kiln_ast::ExprId, crate::init::InitList)>,
    prototype_params: Vec<crate::decl::PrototypeFrame>,
    requires_param_scopes: usize,
    deferring_call_ref: bool,
    deferred_call_refs: Vec<(kiln_ast::ExprId, DeclId)>,
    cleanup: crate::class::CleanupState,
    extend_collector: Option<Vec<kiln_ast::ExprId>>,
    nsdmi_this: Vec<(DeclId, QualType)>,
    contract_ctx: Vec<crate::contracts::ContractCtx>,
    initializing: Option<DeclId>,
    heads: Vec<super::HeadBuild>,
    cur: Option<kiln_ast::ids::TemplateParamListId>,
    entering_context: bool,
    invented: u16,
    req_ctx: Vec<DeclId>,
    dependent_depth: u32,
    friend_tag: Option<DeclId>,
    member_refs: kiln_source::fx::FxHashMap<kiln_ast::ExprId, crate::class::MemberRef>,
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
        let classes = std::mem::take(&mut self.classes);
        self.suspended_classes.push(classes);
        let saved = SavedState {
            scopes: std::mem::take(&mut self.scopes),
            ctx: Some(self.ctx),
            func: self.func.take(),
            unevaluated: std::mem::take(&mut self.unevaluated),
            immediate: std::mem::take(&mut self.immediate),
            pending_odr: std::mem::take(&mut self.pending_odr),
            potential_caps: std::mem::take(&mut self.potential_caps),
            defining: std::mem::take(&mut self.defining),
            enums: std::mem::take(&mut self.enums),
            braced_args: std::mem::take(&mut self.braced_args),
            prototype_params: std::mem::take(&mut self.prototype_params),
            requires_param_scopes: std::mem::take(&mut self.requires_param_scopes),
            deferring_call_ref: std::mem::take(&mut self.deferring_call_ref),
            deferred_call_refs: std::mem::take(&mut self.deferred_call_refs),
            cleanup: std::mem::take(&mut self.cleanup),
            extend_collector: self.extend_collector.take(),
            nsdmi_this: std::mem::take(&mut self.nsdmi_this),
            contract_ctx: std::mem::take(&mut self.contract_ctx),
            initializing: self.initializing.take(),
            heads: std::mem::take(&mut self.templ.heads),
            cur: self.templ.cur.take(),
            entering_context: std::mem::take(&mut self.templ.entering_context),
            invented: std::mem::take(&mut self.templ.invented),
            req_ctx: std::mem::take(&mut self.templ.req_ctx),
            dependent_depth: std::mem::take(&mut self.ast.templates.dependent_depth),
            friend_tag: self.friend_tag.take(),
            member_refs: std::mem::take(&mut self.member_refs),
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
        let access_ctx = self.access_ctx.take();
        let lambda_base = std::mem::replace(&mut self.lambda_base, self.lambdas.len());
        // Likewise the outer expression's deleted functions and template-id
        // warnings waiting for their use: an instantiated body's
        // full-expressions do not report them (WP-1170).
        let r = self.setting_aside_uses_outside(&[], f);
        self.lambda_base = lambda_base;
        self.access_ctx = access_ctx;
        self.scopes = saved.scopes;
        self.ctx = saved.ctx.unwrap_or(self.ast.tu);
        self.func = saved.func;
        self.unevaluated = saved.unevaluated;
        self.immediate = saved.immediate;
        self.pending_odr = saved.pending_odr;
        self.potential_caps = saved.potential_caps;
        self.restore_immediate_state(imm);
        self.defining = saved.defining;
        self.enums = saved.enums;
        self.braced_args = saved.braced_args;
        self.prototype_params = saved.prototype_params;
        self.requires_param_scopes = saved.requires_param_scopes;
        self.deferring_call_ref = saved.deferring_call_ref;
        self.deferred_call_refs = saved.deferred_call_refs;
        self.classes = self.suspended_classes.pop().unwrap_or_default();
        self.cleanup = saved.cleanup;
        self.extend_collector = saved.extend_collector;
        self.range_for_outer_depth = outer_depth;
        self.nsdmi_this = saved.nsdmi_this;
        self.contract_ctx = saved.contract_ctx;
        self.initializing = saved.initializing;
        self.templ.heads = saved.heads;
        self.templ.cur = saved.cur;
        self.templ.entering_context = saved.entering_context;
        self.templ.invented = saved.invented;
        self.templ.req_ctx = saved.req_ctx;
        self.ast.templates.dependent_depth = saved.dependent_depth;
        self.friend_tag = saved.friend_tag;
        // The outer full-expression's pending names ([`Sema::flush_member_refs`])
        // are not the instantiation's to flush; any it left pending stay so.
        let inner = std::mem::replace(&mut self.member_refs, saved.member_refs);
        self.member_refs.extend(inner);
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
        // The constraints of the declaration lookup finds (clang's notes
        // and `[with ...]` name its head), never a friend first
        // declaration's deeper head.
        self.ensure_template_constraints(self.visible_template(tmpl), conv.sugared, 0, loc)?;
        if let Some(s) = self.ast.templates.find_spec(tmpl, conv.canonical) {
            return Ok(s);
        }
        let tag = self
            .ast
            .template_info(tmpl)
            .and_then(|i| i.pattern)
            .and_then(|p| self.ast.decls.record(p))
            .map_or(TagKind::Struct, |r| r.tag);
        let args = self.first_written_template_args(conv);
        let spec = self.builder().class_spec(tmpl, args, conv.canonical, tag);
        let print = self.suppress_default_args(tmpl, conv.canonical, written.len(), loc);
        if let Some((i, _)) = self.ast.spec_info(spec) {
            self.ast.templates.update_spec(i, |s| s.print_args = print);
        }
        // GCC counts every argument of a specialization first formed by
        // substitution (its coercion receives the defaults already filled
        // in), only the written ones otherwise.
        let n = if self.subst_template_id { u32::MAX } else { written.len() as u32 };
        self.ast.templates.set_written_args(spec, n);
        Ok(spec)
    }

    /// The arguments a new class template specialization keeps: the
    /// canonical ones, except that clang 22 keeps a template template
    /// argument as first written (`X<int>::apply`, a qualified template
    /// name), which its members' types then print with
    /// (`'X<int>::apply<int>':'int *'`; [temp.arg.template]/1).
    fn first_written_template_args(&mut self, conv: ConvertedArgs) -> ArgListId {
        if self.clang_compat < 22 {
            return conv.canonical;
        }
        let sugared = self.ast.types.args(conv.sugared).to_vec();
        let canonical = self.ast.types.args(conv.canonical).to_vec();
        if sugared.len() != canonical.len() {
            return conv.canonical;
        }
        let mut changed = false;
        let out: Vec<TemplateArg> = sugared
            .iter()
            .zip(&canonical)
            .map(|(&s, &c)| match s.view() {
                ArgView::Template(n)
                    if matches!(
                        self.ast.types.template_name_view(n),
                        TemplateName::Qualified { .. }
                    ) && !self
                        .ast
                        .types
                        .template_name_flags(n)
                        .intersects(TypeFlags::DEPENDENT_INSTANTIATION) =>
                {
                    changed = true;
                    s
                }
                _ => c,
            })
            .collect();
        if changed { self.ast.types.arg_list(&out) } else { conv.canonical }
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
            if self.ast.decls.get(p).kind() == DeclKind::NonTypeTemplateParm
                && !self.integral_constant_default(def)
            {
                // clang's `isSubstitutedTemplateArgument`: a value equals a
                // non-type default only when the default is a
                // non-dependent integral constant expression (`= 2`, not
                // `= N` or `= Max<L>::value`).
                break;
            }
            if let (ArgView::Type(pat), ArgView::Type(t)) = (def.view(), elems[n - 1].view())
                && !self.default_shape_matches(t, pat, &elems[..n - 1], depth)
            {
                break;
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

    /// Whether written type argument `t` can equal its default `pat` in
    /// clang's `isSubstitutedType` (TypePrinter.cpp), which compares
    /// structurally instead of substituting: a type equal to the
    /// pattern, a type parameter, or a specialization (left to the
    /// substitution [`Self::suppress_default_args`] then makes), reached
    /// through pointer-like declarators (`*`, `&`, `&&`, `C::*`) of the
    /// same type class under equal qualifiers. Anything else, notably an
    /// array or function declarator (clang's FIXME), never matches, so a
    /// written `X<long, long[3]>` keeps `long[3]` against `class A =
    /// T[3]`.
    fn default_shape_matches(
        &mut self,
        t: QualType,
        pat: QualType,
        prefix: &[TemplateArg],
        depth: u16,
    ) -> bool {
        let types = &self.ast.types;
        let (ct, cp) = (types.canonical(t), types.canonical(pat));
        if ct == cp {
            return true;
        }
        match types.canonical_view(pat) {
            // The parameter's argument with the pattern's qualifiers is
            // the written type (clang's `getQualifiedType`: a qualified
            // reference never equals one).
            TypeView::CanonTemplateTypeParm { depth: d, index, .. } => {
                let Some(ArgView::Type(a)) = prefix
                    .get(index as usize)
                    .filter(|_| d == u32::from(depth))
                    .map(TemplateArg::view)
                else {
                    return false;
                };
                let ca = types.canonical(a);
                if cp.quals().is_empty() {
                    return ca == ct;
                }
                if types.is_reference(ca) {
                    return false;
                }
                let q = self.ast.types.qualify(ca, cp.quals());
                return self.ast.types.canonical(q) == ct;
            }
            // A specialization recurses into its arguments (clang's
            // `isSubstitutedType` on a `TemplateSpecializationType`)
            // before the substitution is made: substituting
            // `Set<typename GT<G>::NodeRef>` for a written `Set<N*>`
            // would instantiate `GT<N*>` merely to print the name, ahead
            // of a later explicit specialization ([temp.expl.spec]/7).
            TypeView::TemplateSpecialization { args: pargs, .. } => {
                let targs =
                    types.as_record(ct).and_then(|r| self.ast.spec_info(r)).map(|(_, x)| x.args);
                let Some(targs) = targs else { return true };
                let pa: Vec<TemplateArg> = types.args(pargs).to_vec();
                let ta: Vec<TemplateArg> = types.args(targs).to_vec();
                if pa.len() != ta.len() {
                    return true;
                }
                return pa.iter().zip(&ta).all(|(p, a)| match (p.view(), a.view()) {
                    (ArgView::Type(p), ArgView::Type(a)) => {
                        self.default_shape_matches(a, p, prefix, depth)
                    }
                    _ => true,
                });
            }
            _ => {}
        }
        if ct.quals() != cp.quals() {
            return false;
        }
        let pointee = |q: QualType| match types.view(types.desugar_top(q).ty()) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            TypeView::MemberPointer { pointee, .. } => Some(pointee),
            _ => None,
        };
        match (pointee(t), pointee(pat)) {
            (Some(a), Some(b)) if types.kind(t.ty()) == types.kind(pat.ty()) => {
                self.default_shape_matches(a, b, prefix, depth)
            }
            _ => false,
        }
    }

    /// Whether the non-type default `def` is an expression that is not
    /// value-dependent, of integral or unscoped enumeration type.
    fn integral_constant_default(&self, def: TemplateArg) -> bool {
        let ArgView::Expr { expr, .. } = def.view() else { return false };
        let n = self.ast.exprs.get(expr);
        !n.is_value_dependent() && crate::conv::rank::is_integral_or_unscoped_enum(self, n.ty())
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
        self.diagnose_template_id_use(spec, loc);
        // A template-id naming a still-undeclared specialization after
        // the template's `preferred_name` attribute gives it the
        // attribute (clang `CheckTemplateIdType` `InstantiateAttrsForDecl`);
        // a typedef written before the attribute does not.
        let first = self.ast.first(spec);
        if self.ast.spec_info(spec).is_some_and(|(_, s)| s.kind == SpecKind::Undeclared)
            && !self.ast.templates.is_preferred_named(first)
            && self.ast.has_preferred_name_attr(tmpl)
        {
            self.ast.templates.mark_preferred_named(first);
        }
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
        // [temp.names]/9: a non-dependent alias template-id requires the
        // alias's associated constraints satisfied (clang's
        // `CheckTemplateIdType`; a SFINAE failure in an immediate context,
        // so `requires { typename std::projected<int, F>; }` is false).
        let dependent = (self.ast.types.arg_list_flags(conv.sugared)
            | self.ast.types.template_name_flags(name))
        .intersects(TypeFlags::DEPENDENT_INSTANTIATION);
        if !dependent {
            self.ensure_template_constraints(tmpl, conv.sugared, 3, loc)?;
        }
        let list = self.ast.types.arg_list(written);
        // clang 22 substitutes the arguments as written (`'Ptr<I>':'I *'`,
        // `'Ptr<Ptr<T>>':'Ptr<T> *'`), so the cache is per written list.
        let sugared = self.clang_compat >= 22;
        let key = (tmpl, if sugared { conv.sugared } else { conv.canonical });
        if let Some(&u) = self.inst.alias_cache.get(&key) {
            return Ok(self.alias_spec_type(name, list, u, conv, dependent));
        }
        let Some(pattern) = self.ast.template_info(tmpl).and_then(|i| i.pattern) else {
            return Err(());
        };
        let Some(u) = self.ast.decls.typedef(pattern).map(|t| t.underlying) else { return Err(()) };
        let depth = self.template_param_depth(tmpl);
        let mut args = Self::outer_levels(tmpl, depth);
        // clang 19 substitutes an alias with the canonical arguments: the
        // aliased type of `Ptr<I>` is `int *`, not `I *`.
        let subst_args = if sugared { conv.sugared } else { self.alias_subst_args(conv.sugared) };
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
        // Access in a namespace-scope alias is checked from the use, in a
        // member alias from its class (clang's `CheckTemplateIdType`
        // enters the alias's context only when that is not a file
        // context).
        let use_ctx = self.access_start();
        let access_ctx = (!matches!(
            self.kind_of(ctx),
            DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization
        ))
        .then_some(use_ctx);
        // clang substitutes the aliased type without its source
        // locations: diagnostics on types point at the alias
        // declaration's `using` (spec/sema-templates.md §4.10).
        let at = self.templ.alias_using_loc.get(&pattern).copied().unwrap_or(loc);
        let checks = self.access_checks;
        let trapped = self.sfinae_errors();
        let r = self.in_inst_context(ctx, |s| {
            s.access_ctx = access_ctx;
            let mut sub = Substituter::new(s, &args);
            sub.written_locs = false;
            sub.map_contexts(&pairs);
            sub.ty(u, at)
        });
        self.pop_inst_frame();
        let Ok(u) = r else {
            self.enable_if_alias_failure(tmpl, written.first().copied());
            return Err(());
        };
        // A substitution that checked access depends on the context it
        // was used from: redone at every use, as clang does. One whose
        // error a SFINAE context captured (a narrowing list element in a
        // `decltype`) is redone too, so every use fails
        // ([temp.deduct.general]/8).
        if self.access_checks == checks && self.sfinae_errors() == trapped {
            let _ = self.inst.alias_cache.insert(key, u);
        }
        Ok(self.alias_spec_type(name, list, u, conv, dependent))
    }

    /// The sugar type `name<written>` over aliased type `u`. It keeps the
    /// arguments as written for printing; a non-dependent one records its
    /// canonical converted arguments as its identity ([temp.alias]/2,
    /// [temp.type]/1: `A<1 + 2>` and `A<3>` are the same specialization;
    /// clang's `CheckTemplateArgumentList`), which `^^A<1 + 2>` reflects
    /// (spec/sema-templates.md §4.10).
    fn alias_spec_type(
        &mut self,
        name: TemplateNameId,
        written: ArgListId,
        u: QualType,
        conv: ConvertedArgs,
        dependent: bool,
    ) -> QualType {
        let t = self.ast.types.template_specialization(name, written, Some(u), true);
        if !dependent {
            self.ast.types.note_alias_converted(t.ty(), written, conv.canonical);
        }
        t
    }

    /// clang 19's `enable_if_t` rewrite (`CheckTemplateIdType`,
    /// spec/sema-templates.md §5.7.1): when substituting an alias named
    /// `enable_if_t` or `__enable_if_t` failed in a SFINAE context with the
    /// `'enable_if' cannot be used` error, the captured error is replaced
    /// by the failed requirement of the alias's first argument, at the
    /// same location. Only the deduction-failure note's text changes.
    fn enable_if_alias_failure(&mut self, tmpl: DeclId, first: Option<TemplateArg>) {
        if !self.inst.stack.is_sfinae()
            || !matches!(self.decl_name(tmpl).as_str(), "enable_if_t" | "__enable_if_t")
        {
            return;
        }
        let Some(ArgView::Expr { expr, .. }) = first.map(|a| a.view()) else {
            return;
        };
        let mut sf = self.inst.sfinae.borrow_mut();
        let Some(st) = sf.last_mut() else { return };
        let Some(old) = st.first.as_ref() else { return };
        if old.id() != Some(DiagId::sema_typename_nested_not_found_enable_if) {
            return;
        }
        let at = old.loc();
        drop(sf);
        let failed = self.failed_condition(expr);
        let d = self
            .diag(DiagId::sema_typename_nested_not_found_requirement, at)
            .arg(self.requirement_text(failed))
            .range(self.expr_range(failed));
        if let Some(st) = self.inst.sfinae.borrow_mut().last_mut() {
            st.first = Some(d);
        }
    }

    /// The arguments an alias template is substituted with: canonical,
    /// except that an injected-class-name argument stays one (clang's
    /// canonical `InjectedClassNameType` prints as `X<T>`, where kiln's
    /// canonical form is the dependent specialization).
    fn alias_subst_args(&mut self, sugared: ArgListId) -> ArgListId {
        let out: Vec<TemplateArg> =
            self.ast
                .types
                .args(sugared)
                .to_vec()
                .into_iter()
                .map(|a| match a.view() {
                    ArgView::Type(q) if q.quals().is_empty() => {
                        let mut t = q;
                        loop {
                            if matches!(
                                self.ast.types.view(t.ty()),
                                TypeView::InjectedClassName { .. }
                            ) {
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
                    // A pack holding a pack expansion (`index_sequence<Is...>`)
                    // keeps its elements substitutable the same way.
                    ArgView::Pack(l)
                        if self.ast.types.args(l).iter().any(|e| {
                            matches!(e.view(), ArgView::Expr { profile: Some(_), .. })
                        }) =>
                    {
                        TemplateArg::pack(self.alias_subst_args(l))
                    }
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

    /// The member enumeration of a class template specialization that `q`
    /// names, if not yet defined: a scoped one is defined only when
    /// required ([temp.inst]/3, `Substituter::enum_decl`).
    pub(crate) fn undefined_member_enum(&self, q: QualType) -> Option<DeclId> {
        let e = self.ast.types.as_enum(q)?;
        (self.ast.definition(e).is_none() && self.ast.instantiated_from(e).is_some()).then_some(e)
    }

    /// Defines member enumeration `e` of a class template specialization
    /// if it is one and not yet defined (`using enum`, a qualified
    /// enumerator named from a template's pattern).
    pub(crate) fn require_member_enum(&mut self, e: DeclId, loc: Loc) {
        if self.kind_of(e) == DeclKind::Enum
            && self.ast.definition(e).is_none()
            && self.ast.instantiated_from(e).is_some()
        {
            let _ = self.instantiate_class(e, loc, false);
        }
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

    /// Whether this use of `q` instantiates a class template
    /// specialization whose instantiation fails ([temp.inst]/2, diagnosed
    /// then). clang's `RequireCompleteType` returns the instantiation's
    /// invalidity, so the use that instantiated it fails quietly (`A<int>
    /// a;` is an invalid variable, `p->g()` builds no call and so does not
    /// instantiate `g`); later uses find the class complete.
    pub(crate) fn instantiation_fails(&mut self, q: QualType, loc: Loc) -> bool {
        let Some(r) = self.undefined_spec(q) else { return false };
        if self.instantiate_class(r, loc, true)
            || !self.ast.decls.get(r).has(DeclFlags::INVALID)
            || self.ast.definition(r).is_none()
        {
            return false;
        }
        self.inst.undefined_diagnosed.set(None);
        true
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

    /// The point of instantiation of a class or enumeration completed as
    /// a nested-name-specifier component written at `loc`: clang 22 uses
    /// the begin of the whole specifier (`SS.getBeginLoc()`; spec/sema.md
    /// §1.1), clang 19 the component. Consumes [`Sema::nns_start`].
    pub(crate) fn nns_poi(&mut self, loc: Loc) -> Loc {
        let start = self.nns_start.take();
        if self.clang_compat >= 22 { start.unwrap_or(loc) } else { loc }
    }

    /// A class named in a nested-name-specifier must be complete
    /// ([expr.prim.id.qual]/1); false after a diagnostic.
    pub fn require_complete_nns(&mut self, q: QualType, loc: Loc) -> bool {
        if let Some(r) = self.undefined_spec(q) {
            let poi = self.nns_poi(loc);
            if self.instantiate_class(r, poi, true) {
                return true;
            }
            // A declared, never defined explicit specialization
            // ([temp.expl.spec]/1) is reported here as incomplete.
            if self
                .ast
                .spec_info(r)
                .is_some_and(|(_, s)| s.kind == SpecKind::ExplicitSpecialization)
            {
                let d =
                    self.diag(DiagId::sema_incomplete_nested_name_spec, poi).arg(self.ty_arg(q));
                let d = self.incomplete_note(d, q);
                self.emit(d);
            }
            return false;
        }
        if let Some(e) = self.undefined_member_enum(q) {
            // A scoped member enumeration is defined when named in a
            // nested-name-specifier (clang `RequireCompleteDeclContext`);
            // one without a definition is opaque, so its scope is empty.
            let poi = self.nns_poi(loc);
            let _ = self.instantiate_class(e, poi, false);
            return true;
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
            let d = self.incomplete_note(d, q);
            self.emit(d);
            return false;
        }
        true
    }

    /// The class or enumeration `ctx` that a nested-name-specifier
    /// component `name::` at `loc` named, before the next name is looked
    /// up in it: it must be complete ([expr.prim.id.qual]/1, clang's
    /// `RequireCompleteDeclContext` at the last qualifier's name). A
    /// dependent context is checked at instantiation, a class being
    /// defined is complete enough, a namespace is accepted. The type
    /// prints fully qualified (clang prints the context's own type, not
    /// the typedef that named it); the location is [`Sema::nns_poi`].
    /// False after a diagnostic.
    pub fn require_complete_nns_ctx(&mut self, ctx: DeclId, loc: Loc) -> bool {
        if !self.nns_ctx_incomplete(ctx) {
            self.nns_start = None;
            return true;
        }
        let kind = self.kind_of(ctx);
        let ty = self.tag_type(ctx);
        let at = self.nns_poi(loc);
        let d = self
            .diag(DiagId::sema_incomplete_nested_name_spec, at)
            .arg(format!("'{}'", self.decl_ty_str(ty)));
        // An opaque enumeration gets no note (clang).
        let d = if kind == DeclKind::CXXRecord { self.incomplete_note(d, ty) } else { d };
        self.emit(d);
        false
    }

    /// Whether [`Sema::require_complete_nns_ctx`] rejects `ctx`: a class
    /// or enumeration declared but never defined, neither dependent, nor
    /// declared in a template, nor a template specialization (those are instantiated on demand).
    pub fn nns_ctx_incomplete(&self, ctx: DeclId) -> bool {
        matches!(self.kind_of(ctx), DeclKind::CXXRecord | DeclKind::Enum)
            && self.ast.definition(ctx).is_none()
            && !self.is_dependent_ctx(ctx)
            // A member of a template (clang `isDependentContext`).
            && !self.ast.decls.get(ctx).has(DeclFlags::DEPENDENT_CTX)
            && self.ast.spec_info(ctx).is_none()
            && self.ast.instantiated_from(ctx).is_none()
    }

    /// The arguments of specialization `spec` by depth (§5.1).
    pub(crate) fn spec_args(&self, spec: DeclId) -> Option<(DeclId, MultiLevelArgs)> {
        let (_, info) = self.ast.spec_info(spec)?;
        let tmpl = info.template;
        // Instantiated from a partial specialization: its deduced
        // arguments ([temp.spec.partial.match]/2, [temp.inst]/2).
        let args = if let (Some(a), Some((_, p))) =
            (info.from_args, self.ast.partial_spec_info(info.from))
        {
            self.partial_levels(tmpl, p.partial, a)
        } else {
            let mut args = self.base_levels(tmpl);
            args.push(info.args, tmpl, LevelMode::Substitute);
            args
        };
        Some((tmpl, args))
    }

    /// The argument levels of a specialization of `tmpl` instantiated
    /// from its partial specialization `partial` with deduced arguments
    /// `deduced`: the levels enclosing `tmpl`, then `deduced`. A partial
    /// specialization declared for a member template of an implicit
    /// specialization (`template<> template<int n> struct
    /// O<int>::A<char, n>`, [temp.expl.spec]/16) has no enclosing
    /// template levels, so only those above its own depth are kept.
    pub(crate) fn partial_levels(
        &self,
        tmpl: DeclId,
        partial: DeclId,
        deduced: ArgListId,
    ) -> MultiLevelArgs {
        let mut args = self.base_levels(tmpl);
        // The depth of the definition substituted: a partial
        // specialization redeclared with its member template is defined
        // by the original ([`Substituter::member_partial_spec`]).
        let pattern = self.ast.templates.instantiated_from_map(partial).unwrap_or(partial);
        if let Some((_, info)) = self.ast.partial_spec_info(pattern) {
            let depth = usize::from(self.ast.templates.param_list(info.params).depth);
            args.levels.truncate(depth);
        }
        args.push(deduced, partial, LevelMode::Substitute);
        args
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

    /// Whether the definition of class `def` is being parsed, here or in
    /// a context an enclosing instantiation set aside (clang
    /// `TagDecl::isBeingDefined`).
    pub(crate) fn class_being_defined(&self, def: DeclId) -> bool {
        self.defining.contains(&def)
            || self.classes.iter().any(|b| b.record == def)
            || self.suspended_classes.iter().flatten().any(|b| b.record == def)
    }

    /// Whether specialization `spec` needs instantiating while its
    /// pattern `def` is being defined, diagnosed as in
    /// [`Sema::instantiate_class_as`].
    fn instantiation_within_definition(
        &mut self,
        spec: DeclId,
        def: DeclId,
        poi: Loc,
        diagnose: bool,
        implicit: bool,
    ) -> bool {
        if !self.class_being_defined(def) {
            return false;
        }
        let quiet =
            self.invalid_defining.contains(&def) || self.ast.decls.get(def).has(DeclFlags::INVALID);
        if diagnose && !quiet {
            let t = self.tag_type(spec);
            let d = self
                .diag(DiagId::sema_template_instantiate_within_definition, poi)
                .arg(u8::from(!implicit))
                .arg(self.decl_ty_str(t));
            self.emit(d);
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            self.inst.undefined_diagnosed.set(Some(spec));
        } else {
            self.inst.undefined_diagnosed.set(None);
        }
        true
    }

    /// Instantiates the definition of class template specialization
    /// `spec` ([temp.inst]/2-3, §4.3): the member declarations are
    /// substituted, the member definitions are not (§4.4). `poi` is the
    /// point of instantiation. False when the specialization cannot be
    /// defined (diagnosed if `diagnose`).
    pub fn instantiate_class(&mut self, spec: DeclId, poi: Loc, diagnose: bool) -> bool {
        self.instantiate_class_as(spec, poi, diagnose, true)
    }

    /// [`Sema::match_partial_specs`] for `spec`'s instantiation. A request
    /// for `spec` while its partial specializations are matched
    /// (`X<T, enable_if_t<is_convertible_v<T, S>>>` with a constructor
    /// template of `S` defaulting an argument to `X<T>::type`) finds it
    /// incomplete, as clang's `getPatternForClassTemplateSpecialization`
    /// does (WP-787, Boost.StaticString `enable_if_viewable`).
    fn select_partial_spec(
        &mut self,
        tmpl: DeclId,
        spec: DeclId,
        poi: Loc,
    ) -> Result<Option<super::order::PartialMatch>, ()> {
        self.inst.selecting.push(spec);
        let matched = self.match_partial_specs(tmpl, spec, poi);
        self.inst.selecting.pop();
        matched
    }

    /// [`Sema::instantiate_class`]; `implicit` relocates an implicit
    /// instantiation at its pattern's definition (false for an explicit
    /// instantiation, already located at its template-name).
    pub(crate) fn instantiate_class_as(
        &mut self,
        spec: DeclId,
        poi: Loc,
        diagnose: bool,
        implicit: bool,
    ) -> bool {
        if self.ast.definition(spec).is_some() {
            return true;
        }
        if self.defining.contains(&spec) || self.inst.selecting.contains(&spec) || self.inst.fatal {
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
        let Ok(matched) = self.select_partial_spec(tmpl, spec, poi) else {
            self.ast.decls.set_flags(spec, DeclFlags::INVALID);
            self.inst.undefined_diagnosed.set(Some(spec));
            return false;
        };
        let pattern = match matched {
            Some(m) => {
                args = self.partial_levels(tmpl, m.partial, m.args);
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
                // clang notes the latest declaration (one after
                // `template<> template<...>` is a new decl,
                // [temp.expl.spec]/16; kiln adds none for a plain
                // redeclaration).
                let at = if matched.is_some() { pattern } else { self.ast.latest(pattern) };
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
        // The pattern is still being defined (`basic<long>::m` inside
        // `template<class S> struct basic`, [temp.inst]/2 needs the class
        // complete, [class.mem.general]/8): clang
        // `DiagnoseUninstantiableTemplate`, no note (we are lexically
        // inside the template), the specialization made invalid; quiet
        // once a member of incomplete type made the pattern invalid.
        if self.instantiation_within_definition(spec, def, poi, diagnose, implicit)
            || !self.push_inst_frame(FrameKind::ClassInstantiation, spec, tmpl, poi)
        {
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
            // clang `InstantiateClass` locates an implicit instantiation at
            // its pattern's definition (the primary's or the partial
            // specialization's name), so the implicit special members
            // declared in it point there too; an explicit instantiation
            // stays at its own template-name.
            if implicit
                && self
                    .ast
                    .templates
                    .spec(i)
                    .is_some_and(|s| s.kind == SpecKind::ImplicitInstantiation)
            {
                let at = self.ast.decls.get(def).loc();
                self.builder().relocate_spec(spec, at);
            }
        }
        // The primary's attributes are instantiated onto the
        // specialization (clang `InstantiateClass` `InstantiateAttrs`):
        // from here it prints by its `preferred_name` typedef.
        if matched.is_none() && self.ast.has_preferred_name_attr(tmpl) {
            let first = self.ast.first(spec);
            self.ast.templates.mark_preferred_named(first);
        }
        let parent = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        let outer = self.inst_context_pairs(parent);
        self.in_inst_context(parent, |s| s.instantiate_class_body(spec, def, &args, &outer));
        self.replay_dependent_access(def, spec);
        self.pop_inst_frame();
        // clang `InstantiateClass` returns the specialization's
        // invalidity: the use that instantiated it fails quietly (`A<int>
        // a;` is an invalid variable); later uses find it complete.
        if self.ast.decls.get(spec).has(DeclFlags::INVALID) {
            self.inst.undefined_diagnosed.set(Some(spec));
            return false;
        }
        self.ast.definition(spec).is_some()
    }

    /// Instantiates the definition of `r`, a member class of a class
    /// template specialization, from its pattern ([temp.inst]/3: a member
    /// class is declared with its enclosing class and defined when a
    /// complete type is required). False when the pattern has no
    /// definition (the caller reports the incomplete type).
    fn instantiate_member_class(&mut self, r: DeclId, poi: Loc) -> bool {
        let Some(pattern) = self.ast.instantiated_from(r) else { return false };
        let first = self.ast.first(r);
        if let Some((i, info)) = self.ast.member_spec_info(first).map(|(i, x)| (i, *x)) {
            // [temp.expl.spec]/7: an explicitly specialized member is
            // never instantiated from the pattern; one only declared is
            // incomplete (the caller reports it).
            if info.kind == SpecKind::ExplicitSpecialization {
                return false;
            }
            if info.point_of_instantiation == Loc::INVALID && poi != Loc::INVALID {
                self.ast.templates.update_member_spec(i, |m| m.point_of_instantiation = poi);
            }
        }
        let Some(def) = self.member_pattern_definition(r, pattern) else { return false };
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
                // The specialization and the classes enclosing it map
                // too: a member class of a member template's
                // specialization names a sibling member template of the
                // outermost class (`view<V>::inner<C>` in
                // `view<int>::outer<true>::value_type`).
                outer.extend(self.inst_context_pairs(x));
                break;
            }
            c = self.ast.decls.get(x).ctx();
        }
        let Some(args) = args else { return false };
        if self.kind_of(r) == DeclKind::Enum {
            return self.instantiate_member_enum(r, def, (&args, &outer), poi);
        }
        if !self.push_inst_frame(FrameKind::MemberClass, r, pattern, poi) {
            return false;
        }
        let parent = self.ast.decls.get(r).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(parent, |s| s.instantiate_class_body(r, def, &args, &outer));
        self.replay_dependent_access(def, r);
        self.pop_inst_frame();
        // As for a specialization: an instantiation that failed leaves
        // the class complete but invalid ([`Sema::instantiation_fails`]).
        self.ast.definition(r).is_some() && !self.ast.decls.get(r).has(DeclFlags::INVALID)
    }

    /// The definition of member `r`'s pattern `pattern`: for an
    /// enumeration, one in the pattern's class or out of line before the
    /// class was instantiated ([temp.mem.enum]; `Substituter::enum_decl`).
    fn member_pattern_definition(&self, r: DeclId, pattern: DeclId) -> Option<DeclId> {
        let def = self.ast.definition(pattern)?;
        if self.kind_of(r) != DeclKind::Enum {
            return Some(def);
        }
        let ctx = |d: DeclId| self.ast.decls.get(d).ctx();
        (def == pattern || ctx(def) == ctx(pattern)).then_some(def)
    }

    /// Defines scoped member enumeration `e` of a class template
    /// specialization from pattern definition `def` when it is first
    /// required ([temp.inst]/3; clang `InstantiateEnum`).
    fn instantiate_member_enum(
        &mut self,
        e: DeclId,
        def: DeclId,
        (args, outer): (&MultiLevelArgs, &[(DeclId, DeclId)]),
        poi: Loc,
    ) -> bool {
        if !self.push_inst_frame(FrameKind::Enum, e, def, poi) {
            return false;
        }
        let parent = self.ast.decls.get(e).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(parent, |s| {
            let mut sub = Substituter::new(s, args);
            sub.entity = Some(parent);
            sub.map_contexts(outer);
            let _ = sub.enum_definition(e, def);
        });
        self.pop_inst_frame();
        self.ast.definition(e).is_some()
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
        let f = self.member_redecl_instance(f);
        if self.ast.definition(f).is_some() || self.instantiation_stopped() {
            return;
        }
        // [temp.expl.spec]/7: an explicit specialization is never
        // instantiated from the pattern (its definition, if any, is the
        // user's, possibly in another translation unit).
        let explicit = self
            .ast
            .spec_info(f)
            .map(|(_, i)| i.kind)
            .or_else(|| self.ast.member_spec_info(f).map(|(_, m)| m.kind));
        if explicit == Some(SpecKind::ExplicitSpecialization) {
            return;
        }
        // A use inside a ctor-initializer (`B<int>(3)` calling a
        // constructor instantiated here) must not clobber the enclosing
        // constructor's mem-initializer state.
        let outer = std::mem::take(&mut self.mem_inits);
        let (body_errors, errors) = (self.inst.body_errors, self.diags.error_count());
        let checked = self.inst.checked_decl_errors;
        self.instantiate_function_definition_inner(f, poi);
        self.inst.body_errors = body_errors + self.diags.error_count().saturating_sub(errors);
        // Counted once, in `body_errors`.
        self.inst.checked_decl_errors = checked;
        self.mem_inits = outer;
        // clang `InstantiateFunctionDefinition` ends with
        // `HandleTopLevelDecl` (coverage's zero record for an
        // instantiation never emitted, WP-1083).
        if let Some(def) = self.ast.definition(f)
            && self.ast.decls.function(def).is_some_and(|x| x.body.is_some())
        {
            self.ast.decls.note_fn_def_inst(def);
        }
    }

    fn instantiate_function_definition_inner(&mut self, f: DeclId, poi: Loc) {
        // Already being instantiated further up (constexpr members that
        // use each other, each instantiated at its use: a move
        // constructor calling a `swap` that moves `*this`): the
        // definition is in progress (clang's `isDefined` is true once
        // `InstantiateFunctionDefinition` has started).
        if self.inst.stack.contains(FrameKind::MemberFunction, f) {
            return;
        }
        let Some(pattern) = self.ast.instantiated_from(f) else { return };
        let Some(pdef) = self.ast.definition(pattern) else {
            self.instantiate_out_of_line_defaulted(f, pattern, poi);
            return;
        };
        let Some(body) = self.ast.decls.function(pdef).and_then(|x| x.body) else { return };
        if self.suppressed_by_explicit_inst_declaration(f, pattern) {
            return;
        }
        // clang `InstantiateFunctionDefinition`: the function is located
        // at the definition it is instantiated from from now on.
        self.builder().relocate_to_pattern_definition(f, pdef);
        // A `weak` attribute written on the pattern's out-of-line
        // definition only (`template<class T> __attribute__((weak))
        // C<T>::C(T) {}`) makes the instantiation weak too, as in clang,
        // which gives it `weak` linkage; the in-class declaration's
        // attributes came with the member's declaration.
        if pdef != pattern && !self.ast.is_weak_decl(f) {
            let ast = &self.ast;
            let weak = ast
                .lists
                .iter::<kiln_ast::ids::AttrId>(ast.attrs.of_decl(pdef))
                .map(|a| *ast.attrs.get(a))
                .find(|n| n.kind == kiln_ast::attr::AttrKind::WEAK);
            if let Some(n) = weak {
                let a = self.ast.attrs.push(n);
                self.builder().append_attrs(f, &[a]);
            }
        }
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
        let name_loc = self.ast.decls.get(f).loc();
        self.check_consteval_only_inst(f, pattern, name_loc);
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
        self.replay_dependent_access(pdef, f);
        self.pop_inst_frame();
    }

    /// [temp.explicit]/12: under an explicit instantiation declaration
    /// (of `f` itself or of an enclosing class), only an inline function
    /// or one with a deduced return type is implicitly instantiated
    /// (clang `InstantiateFunctionDefinition` returns early for
    /// `TSK_ExplicitInstantiationDeclaration` unless the pattern
    /// `isInlined()` or its return type contains `auto`). A constexpr or
    /// consteval function is inline ([dcl.constexpr]/1).
    fn suppressed_by_explicit_inst_declaration(&self, f: DeclId, pattern: DeclId) -> bool {
        if self.ast.governing_explicit_inst_kind(f)
            != Some(SpecKind::ExplicitInstantiationDeclaration)
        {
            return false;
        }
        let spec_any = |d: DeclId, p: &dyn Fn(kiln_ast::decl::FnSpec) -> bool| {
            self.ast.redecls_of(d).any(|r| self.ast.decls.function(r).is_some_and(|x| p(x.spec)))
        };
        let inline = self.instantiated_pattern_inline(f).unwrap_or(false)
            || spec_any(pattern, &|s| s.constexpr_kind() != kiln_ast::decl::ConstexprKind::None);
        let deduced = spec_any(pattern, &kiln_ast::decl::FnSpec::return_deduced)
            || spec_any(f, &kiln_ast::decl::FnSpec::return_deduced)
            || self.ast.redecls_of(pattern).any(|r| {
                let ty = self.ast.decls.get(r).ty().unwrap_or(QualType::ERROR);
                self.sugared_proto(ty).is_some_and(|p| self.contains_placeholder(p.ret()))
            });
        !inline && !deduced
    }

    /// A special member whose pattern is defaulted out of line
    /// (`template<class T> C<T>::C() = default;`): the instantiation is
    /// defaulted and defined there (clang `InstantiateFunctionDefinition`
    /// → `DefineDefaultedFunction`, without `SetDeclDefaulted`'s checks).
    fn instantiate_out_of_line_defaulted(&mut self, f: DeclId, pattern: DeclId, poi: Loc) {
        let Some(def) = self.ast.redecls_of(pattern).find(|&d| {
            self.ast.prev(d).is_some()
                && self
                    .ast
                    .decls
                    .function(d)
                    .is_some_and(|x| x.spec.defaulted() == kiln_ast::decl::Defaulted::Explicit)
        }) else {
            return;
        };
        let Some(kind) = self.special_kind_of(f) else { return };
        if !self.push_inst_frame(FrameKind::MemberFunction, f, pattern, poi) {
            return;
        }
        // The function, the diagnostics of its definition and the
        // definition's context note are at the pattern's out-of-line
        // `= default` definition (clang `InstantiateFunctionDefinition`
        // relocates it there first).
        self.builder().relocate_to_pattern_definition(f, def);
        let at = self.ast.decls.get(def).loc();
        self.define_implicit_special(f, kind, at);
        self.pop_inst_frame();
    }

    /// A reference to variable `v` at `loc`: a static data member of a
    /// specialization whose initializer was deferred gets it now
    /// ([temp.inst]/4; clang instantiates at the reference, before the
    /// value is needed). An unevaluated reference instantiates only a
    /// variable usable in constant expressions. The out-of-line
    /// definition of a non-inline one is instantiated here when the
    /// variable may be usable in constant expressions, else at the end
    /// of the translation unit ([temp.point]/7, as clang). A definition
    /// instantiated here is handed to codegen at the namespace-scope
    /// declaration containing the reference ([temp.point]/1,
    /// [`Self::record_ref_var_inst`]).
    ///
    /// A variable that may be usable in constant expressions and is
    /// odr-used before its template's definition is defined from the
    /// definition seen since: at its next reference, or at the end of the
    /// TU, which is also its point of instantiation ([temp.point]/7).
    /// clang instantiates it only at a later reference and otherwise
    /// leaves it undefined (spec/sema-templates.md §4.6 "Points of
    /// instantiation", WP-490).
    pub(crate) fn note_var_referenced(&mut self, v: DeclId, loc: Loc) {
        // Neither an unevaluated operand nor a discarded statement
        // ([stmt.if]/2) odr-uses `v`: only a variable usable in constant
        // expressions is instantiated there (clang `DoMarkVarDeclReferenced`).
        let odr = self.unevaluated == 0 && !self.in_discarded_statement();
        let deferred = self.inst.deferred_inits.contains(&v);
        if !deferred && !self.inst.member_defs.contains(&v) {
            if self.is_var_spec(v) && self.var_pattern_def(v).is_none() {
                let used = odr;
                let poi = self.inst.undefined_var_uses.entry(v).or_insert((loc, used));
                poi.1 |= used;
                let poi = poi.0;
                // clang `DoMarkVarDeclReferenced`: every odr-use of a
                // specialization that cannot be constant queues it at its
                // first reference (clang's FIXME: it cannot tell whether
                // the definition was requested already), so a definition
                // still missing at the end of the TU is reported once
                // per use (-Wundefined-var-template, §4.5).
                // One that may be constant is queued once, quietly (the
                // end of the TU is a point of instantiation; a deviation).
                if used
                    && self.is_implicit_var_spec(v)
                    && (!self.might_be_constant_var(v) || self.inst.quiet_pending.insert(v))
                {
                    self.inst.pending.push_back(super::pending::PendingInst { decl: v, poi });
                }
            } else if self.is_implicit_var_spec(v)
                && self.might_be_constant_var(v)
                && self.ast.decls.var(v).is_some_and(|x| x.init.is_none())
                && self
                    .var_pattern_def(v)
                    .is_some_and(|d| self.ast.spec_info(v).is_some_and(|(_, i)| i.from != d))
            {
                // Named before its template's definition, which is seen
                // now: instantiated here, as the reference may need its
                // value (clang `InstantiateVariableDefinition` finds the
                // definition).
                self.instantiate_late_var_spec(v, loc);
                if self.ast.decls.var(v).is_some_and(|x| x.init.is_some()) {
                    self.record_ref_var_inst(v);
                }
            }
            return;
        }
        let constant = self.might_be_constant_var(v);
        if !odr && !constant {
            return;
        }
        if deferred && constant {
            self.instantiate_static_member_init(v, loc);
            if !self.inst.deferred_inits.contains(&v) {
                self.record_ref_var_inst(v);
            }
        } else if deferred {
            // clang `DoMarkVarDeclReferenced`: an inline static data
            // member that cannot be constant is queued like any other
            // definition (its initializer is instantiated by the
            // end-of-TU drain, in queue order; WP-186).
            self.queue_instantiation(v, loc);
        } else if constant {
            self.instantiate_static_member_def(v, loc);
            if !self.inst.member_defs.contains(&v) {
                self.record_ref_var_inst(v);
            } else if odr && !self.is_var_spec(v) && self.inst.quiet_pending.insert(v) {
                // No out-of-line definition yet: the end of the TU is a
                // point of instantiation too ([temp.point]/7).
                self.queue_instantiation(v, loc);
            }
        } else {
            self.queue_instantiation(v, loc);
        }
    }

    /// clang `InstantiateVariableDefinition`'s `PassToConsumerRAII`: the
    /// definition of variable `v`, just instantiated at a reference, is
    /// handed to codegen at once. While the TU is parsed that is before
    /// the namespace-scope declaration containing the reference (its
    /// point of instantiation, [temp.point]/1), or after the last one when
    /// the reference is in a declaration that is not a member (a
    /// `static_assert`). A reference inside another instantiation, or
    /// during the end-of-TU drain, is left to codegen's first use (not
    /// recorded).
    fn record_ref_var_inst(&mut self, v: DeclId) {
        if self.inst.at_end_of_tu || !self.inst.stack.is_empty() {
            return;
        }
        // The innermost enclosing namespace, and its member containing
        // the current context (a function body's context may be the
        // namespace itself: then the function's).
        let (mut ns, mut below) = self.enclosing_ns_member(self.ctx);
        if below.is_none()
            && let Some(f) = self.func.as_ref().map(|f| f.decl)
        {
            (ns, below) = self.enclosing_ns_member(f);
        }
        let Some(ns) = ns else { return };
        // A namespace-scope variable whose initializer is parsed.
        let below = below.or_else(|| {
            self.depr_init
                .or(self.initializing)
                .filter(|&d| self.ast.decls.lexical_ctx(d) == Some(ns))
        });
        if let Some(d) = below {
            self.ast.templates.add_ref_var_inst(d, false, v);
        } else {
            let last = self.ast.decls.member_list(ns).and_then(|l| l.last);
            self.ast.templates.add_ref_var_inst(last.unwrap_or(ns), true, v);
        }
    }

    /// The innermost namespace (or linkage specification) enclosing `d`
    /// lexically, with its member containing `d` (`None` when `d` is
    /// the namespace).
    fn enclosing_ns_member(&self, d: DeclId) -> (Option<DeclId>, Option<DeclId>) {
        let mut ns = d;
        let mut below = None;
        for _ in 0..1024 {
            if matches!(
                self.kind_of(ns),
                DeclKind::Namespace
                    | DeclKind::TranslationUnit
                    | DeclKind::LinkageSpec
                    | DeclKind::Export
            ) {
                return (Some(ns), below);
            }
            below = Some(ns);
            let Some(up) = self.ast.decls.lexical_ctx(ns) else { break };
            ns = up;
        }
        (None, None)
    }

    /// Whether variable template specialization `v` is implicitly
    /// instantiated (not explicitly specialized or instantiated).
    pub(crate) fn is_implicit_var_spec(&self, v: DeclId) -> bool {
        self.ast.spec_info(v).is_some_and(|(_, s)| s.kind == SpecKind::ImplicitInstantiation)
    }

    /// clang's `mightBeUsableInConstantExpressions`: `constexpr`, or a
    /// `const` non-volatile integral, enumeration or reference variable
    /// ([expr.const]/4, before its initializer is known).
    pub(crate) fn might_be_constant_var(&self, v: DeclId) -> bool {
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
        // [temp.explicit]/12: an explicit instantiation declaration
        // suppresses the definition unless the member may be usable in
        // constant expressions (clang `InstantiateVariableDefinition`).
        // The member stays pending so a later explicit instantiation
        // definition still instantiates it.
        if self.ast.governing_explicit_inst_kind(v)
            == Some(SpecKind::ExplicitInstantiationDeclaration)
            && !self.might_be_constant_var(v)
        {
            return;
        }
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
        let mut ty = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        // A member declared as an array of unknown bound gets its bound
        // from the out-of-line definition ([dcl.array]/3), whose type is
        // the one instantiated (clang `InstantiateVariableDefinition`):
        // `static T s[];` ... `template<class U> T C<U>::s[N];`, as in
        // libstdc++'s copy-on-write `basic_string::_Rep::_S_empty_rep_storage`.
        let incomplete = |s: &Self, t: QualType| {
            matches!(s.ast.types.canonical_view(t), TypeView::IncompleteArray(_))
        };
        if incomplete(self, ty)
            && let Some(pty) = pn.ty()
            && !incomplete(self, pty)
        {
            let sty = self.in_inst_context(spec, |s| {
                let mut sub = Substituter::new(s, &args);
                sub.entity = Some(spec);
                sub.map_contexts(&pairs);
                sub.ty(pty, pn.loc()).ok()
            });
            if let Some(t) = sty.filter(|&t| !self.is_error_type(t)) {
                ty = t;
            }
        }
        let vspec = self.ast.decls.var(v).map(|x| x.spec).unwrap_or_default();
        let pspec = self.ast.decls.var(pdef).map(|x| x.spec).unwrap_or_default();
        let spec_bits = pspec
            .with_storage(kiln_ast::decl::VarStorage::None)
            .with_inline(vspec.is_inline())
            .with_constexpr(vspec.is_constexpr());
        let mut h = kiln_ast::DeclHead::new(spec, pn.name(), pn.loc());
        h.lexical = Some(lexical);
        // The definition belongs to the specialization, which is not a
        // dependent context ([temp.dep.type]/1; clang's
        // `InstantiateVariableDefinition` gives it the specialization as
        // its `DeclContext`), even when the reference instantiating it
        // sits in a template definition (`alignment_of<char>::value` in
        // boost's `type_with_alignment<Align>`). Built at that
        // reference's template depth it got `DEPENDENT_CTX`, and as the
        // member's latest redeclaration it made every later reference to
        // the member instantiation-dependent: a template-id taking one as
        // an argument (boost.mpl's `lcm<S::value, alignment_of<T>::value>`)
        // stayed dependent after substitution.
        let depth = std::mem::take(&mut self.ast.templates.dependent_depth);
        let d = self.builder().var(h, ty, spec_bits, Some(v));
        self.ast.templates.dependent_depth = depth;
        self.ast.templates.set_instantiated_from(d, pdef);
        // A `constinit` definition stays the initializing declaration
        // that has `constinit` ([dcl.constinit]/1).
        if let Some(&at) = self.constinit_locs.get(&pdef) {
            self.constinit_locs.insert(d, at);
        }
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
        self.subst_static_member_init(v, poi, true);
    }

    /// Instantiates the in-class initializer of static data member `v`
    /// of a specialization that was declared without it although its
    /// initializer is not deferred: the specialization was instantiated
    /// from a declaration image before the pattern's initializer was set
    /// (spec/modules.md §11.11 "Templates as patterns", WP-1300). The
    /// member stays as declared (not made `inline`).
    pub(crate) fn instantiate_missing_static_member_init(&mut self, v: DeclId, poi: Loc) {
        if self.inst.fatal || self.inst.deferred_inits.contains(&v) {
            return;
        }
        self.subst_static_member_init(v, poi, false);
    }

    /// [`Self::instantiate_static_member_init`]'s substitution; `inline`:
    /// the member becomes `inline` (a deferred one was declared without).
    fn subst_static_member_init(&mut self, v: DeclId, poi: Loc, inline: bool) {
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
        if inline {
            self.ast.decls.mark_var(v, kiln_ast::decl::VarSpec::new().with_inline(true));
        }
        // The member was declared without its initializer: it takes the
        // pattern's form now (`static constexpr E e{X::v};` is direct-list-
        // initialization, [dcl.init.list]/3.8).
        let style = self.ast.decls.var(pattern).map(|x| x.spec.init_style());
        let cur = self.ast.decls.var(v).map(|x| x.spec.init_style());
        if let Some(style) = style
            && cur == Some(kiln_ast::decl::InitStyle::C)
            && style != kiln_ast::decl::InitStyle::C
        {
            self.ast.decls.mark_var(v, kiln_ast::decl::VarSpec::new().with_init_style(style));
        }
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

    /// Instantiates the deferred default member initializer of field
    /// `f` of a class template specialization, at its first use from
    /// `poi` ([temp.inst]/3; clang `InstantiateInClassInitializer`).
    /// Nothing happens for a field without one.
    pub(crate) fn instantiate_nsdmi(&mut self, f: DeclId, poi: Loc) {
        if self.inst.deferred_nsdmis.is_empty() {
            return;
        }
        let Some((spec, init)) = self.inst.deferred_nsdmis.remove(&f) else { return };
        if self.inst.fatal {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            return;
        }
        let Some((args, pairs)) = self.inst.nsdmi_ctx.get(&spec).cloned() else { return };
        let pattern = self.ast.instantiated_from(f).unwrap_or(f);
        if !self.push_inst_frame(FrameKind::Nsdmi, f, pattern, poi) {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
            return;
        }
        let parent = self.ast.decls.get(spec).ctx().unwrap_or(self.ast.tu);
        self.in_inst_context(parent, |s| {
            s.act_on_start_delayed_class(spec);
            let mut sub = Substituter::new(s, &args);
            sub.entity = Some(spec);
            sub.pairs = pairs;
            sub.nsdmi(f, init);
            s.act_on_finish_delayed_class(spec);
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
    /// The declaration of `f` instantiated from a pattern: `f` itself,
    /// or the first declaration when `f` is a redeclaration that is not
    /// (a friend `friend R B<T>::f(W<T>&);` in an instantiated `W<A>`
    /// redeclaring the member of `B<A>`, [class.friend]/10; WP-1071).
    pub(crate) fn member_redecl_instance(&self, f: DeclId) -> DeclId {
        if self.ast.instantiated_from(f).is_some() {
            return f;
        }
        let first = self.ast.first(f);
        if first != f && self.ast.instantiated_from(first).is_some() { first } else { f }
    }

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
        let f = self.member_redecl_instance(f);
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
                self.queue_or_defer_instantiation(f, loc);
            }
        } else {
            self.queue_or_defer_instantiation(f, loc);
        }
    }

    /// [`Sema::queue_instantiation`], unless a default argument is being
    /// converted outside any other function body: then `f` is recorded
    /// for the default argument and queued where a call uses it.
    fn queue_or_defer_instantiation(&mut self, f: DeclId, loc: Loc) {
        let cur = self.func.as_ref().map(|s| s.decl);
        if let Some((at, uses)) = self.inst.default_arg_uses.as_mut()
            && *at == cur
        {
            uses.push((f, loc));
            return;
        }
        self.queue_instantiation(f, loc);
    }

    /// Runs `conv`, the conversion of a default argument to its
    /// parameter's type, giving the stored argument. Function
    /// definitions it names are not instantiated yet: a default argument
    /// is potentially evaluated only where a call uses it (clang's
    /// `PotentiallyEvaluatedIfUsed` context, re-marked by
    /// `CheckCXXDefaultArgExpr`), so `P<Incomplete> p = nullptr` does not
    /// instantiate `~P<Incomplete>` until then ([temp.inst]/5,
    /// [basic.def.odr]/8).
    pub(crate) fn defer_default_arg_uses(
        &mut self,
        conv: impl FnOnce(&mut Self) -> kiln_ast::ExprId,
    ) -> kiln_ast::ExprId {
        let cur = self.func.as_ref().map(|s| s.decl);
        let saved = self.inst.default_arg_uses.replace((cur, Vec::new()));
        let e = conv(self);
        let uses = std::mem::replace(&mut self.inst.default_arg_uses, saved);
        if let Some((_, uses)) = uses.filter(|(_, u)| !u.is_empty()) {
            self.inst.deferred_default_uses.insert(e, uses);
        }
        e
    }

    /// A call uses parameter `p`'s default argument at `loc`: the
    /// instantiations its conversion deferred are queued now.
    pub(crate) fn queue_default_arg_uses(&mut self, p: DeclId, loc: Loc) {
        let Some(d) = self.ast.decls.param(p).and_then(|x| x.default_arg) else { return };
        let Some(uses) = self.inst.deferred_default_uses.get(&d) else { return };
        for (f, at) in uses.clone() {
            self.note_used_for_instantiation(f, loc);
            self.note_undefined_use(f, at);
        }
    }
}

impl Substituter<'_, '_, '_> {
    /// Defines member function `f` from its pattern definition `pdef`
    /// with body `body` ([temp.inst]/4-5): the parameters are mapped, a
    /// constructor's ctor-initializer is substituted, then the body.
    pub(crate) fn function_def(&mut self, f: DeclId, pdef: DeclId, body: kiln_ast::StmtId) {
        let sub = self;
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
        // The member was declared from the in-class declaration, whose
        // parameters may be named differently from the out-of-class
        // definition's: the body sees the definition's names (clang
        // `addInstantiatedParametersToScope`), so a local `__p` beside
        // a definition parameter `__f` is no redefinition of the
        // declaration's `__p` ([basic.scope.block]/2, [temp.inst]/4).
        sub.rename_params_from_pattern(&pp);
        sub.sema.act_on_start_function_def(f);
        let mut inits_failed = false;
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
            inits_failed = sub.sema.instantiated_mem_inits_failed(f);
            if sub.sema.inst.failed_mem_inits.contains(&pdef) {
                sub.sema.note_failed_mem_initializer(f);
            }
            sub.sema.act_on_mem_initializers(f, &[], colon);
        }
        let b = sub.body(body);
        // After the body: a deduced return type is known
        // ([dcl.contract.res]/2).
        sub.function_contracts(f, pdef);
        if sub.stmt_failed || inits_failed {
            // [temp.inst]: a body or a mem-initializer that failed to
            // instantiate makes the member specialization invalid (clang
            // `setInvalidDecl`, also in `InstantiateMemInitializers`), as
            // for a function template specialization (funcs.rs).
            sub.sema.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        sub.sema.act_on_finish_function_def(f, b, rbrace);
    }

    /// The base-specifiers of pattern definition `def`, substituted
    /// ([temp.inst]/3: the base-clause is instantiated with the class;
    /// [temp.variadic]/5: a pack expansion gives one base per element).
    /// The flag is set when a base failed to substitute (clang
    /// `InstantiateClass`: the specialization is then invalid). With each
    /// specifier, its annotations substituted for `spec`
    /// ([dcl.attr.annotation]/3: new annotations; each element of a pack
    /// expansion gets its own copies).
    #[allow(clippy::type_complexity)]
    fn class_bases(
        &mut self,
        spec: DeclId,
        def: DeclId,
    ) -> (Vec<BaseSpecInfo>, Vec<Vec<AnnId>>, bool) {
        // Base classes ([temp.inst]/3: the base-clause is instantiated
        // with the class).
        let bases: Vec<kiln_ast::decl::BaseSpec> =
            self.sema.ast.decls.bases(def, &self.sema.ast.lists).copied().collect();
        let mut specs = Vec::with_capacity(bases.len());
        let mut anns: Vec<Vec<AnnId>> = Vec::new();
        let mut failed = false;
        let def_first = self.sema.ast.first(def);
        for (bi, b) in bases.into_iter().enumerate() {
            let ast = &self.sema.ast;
            let pat: Vec<AnnId> =
                ast.lists.iter::<AnnId>(ast.annots.of_base(def_first, bi as u32)).collect();
            // [temp.variadic]/5.4: the pattern of `B...` is the whole
            // base-specifier, so an annotation operand naming the pack is
            // substituted per element (WP-867); the others once, copied.
            let in_pattern = |a: AnnId| {
                let n = ast.annots.get(a);
                n.value.is_none()
                    && ast
                        .exprs
                        .get(n.expr)
                        .deps()
                        .contains(kiln_ast::expr::ExprDeps::UNEXPANDED_PACK)
            };
            let per_elem: Vec<AnnId> = if b.pack_expansion {
                pat.iter().copied().filter(|&a| in_pattern(a)).collect()
            } else {
                Vec::new()
            };
            let once: Vec<AnnId> = if per_elem.is_empty() {
                pat.clone()
            } else {
                pat.iter().copied().filter(|a| !per_elem.contains(a)).collect()
            };
            let mine =
                if once.is_empty() { Vec::new() } else { self.subst_annotations(spec, &once) };
            if b.pack_expansion {
                // [temp.variadic]/5: one base per element.
                let exprs: Vec<kiln_ast::ExprId> =
                    per_elem.iter().map(|&a| self.sema.ast.annots.get(a).expr).collect();
                let elems = self.expand_pattern(|s| {
                    let ty = s.element_ty(b.ty, b.loc)?;
                    let es = exprs.iter().map(|&e| s.expr(e)).collect::<SubstResult<Vec<_>>>()?;
                    Ok((ty, es))
                });
                let elems = match elems {
                    Ok(crate::templ::pack::Expansion::Elements(v)) => v,
                    Ok(crate::templ::pack::Expansion::Unexpanded(_)) => {
                        let _ = self.unimplemented::<()>("base pack expansion", b.loc);
                        continue;
                    }
                    Err(_) => {
                        failed = true;
                        continue;
                    }
                };
                for (k, (ty, es)) in elems.into_iter().enumerate() {
                    specs.push(BaseSpecInfo {
                        ty,
                        loc: b.loc,
                        is_virtual: b.is_virtual,
                        access: b.access,
                        pack_expansion: false,
                        attrs: Vec::new(),
                    });
                    let copies = if k == 0 { mine.clone() } else { self.sema.copy_annots(&mine) };
                    if es.is_empty() {
                        anns.push(copies);
                        continue;
                    }
                    // Source order: the pattern's list, each operand
                    // naming the pack taken from this element.
                    let (mut copies, mut es) = (copies.into_iter(), es.into_iter());
                    let mut list = Vec::with_capacity(pat.len());
                    for &a in &pat {
                        if per_elem.contains(&a) {
                            let loc = self.sema.ast.annots.get(a).loc;
                            if let Some(e) = es.next()
                                && let Some(id) = self.sema.annotate(spec, e, loc)
                            {
                                list.push(id);
                            }
                        } else if let Some(id) = copies.next() {
                            list.push(id);
                        }
                    }
                    anns.push(list);
                }
                continue;
            }
            match self.ty(b.ty, b.loc) {
                Ok(ty) => {
                    specs.push(BaseSpecInfo {
                        ty,
                        loc: b.loc,
                        is_virtual: b.is_virtual,
                        access: b.access,
                        pack_expansion: false,
                        attrs: Vec::new(),
                    });
                    anns.push(mine);
                }
                Err(_) => failed = true,
            }
        }
        (specs, anns, failed)
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

    /// The pattern's class-virt-specifier `final` ([class.pre]/1:
    /// `__is_final` of a specialization of a `final` class template is
    /// true; libc++'s compressed-pair padding of a final allocator), then
    /// its attributes ([`Substituter::class_attrs`]).
    fn class_final_and_attrs(&mut self, spec: DeclId, def: DeclId) {
        if self.sema.ast.decls.record(def).is_some_and(|r| r.defn.contains(DefinitionBits::FINAL)) {
            let loc = self.sema.ast.decls.get(def).loc();
            self.sema.act_on_class_final(spec, loc);
        }
        self.class_attrs(spec, def);
    }

    /// Instantiates member `m` of a class template (`templ`; not a local
    /// class) if it is a `static_assert` or a consteval block (`kind`):
    /// whether it failed; `None` for any other member.
    fn assert_like_failed(&mut self, m: DeclId, kind: DeclKind, templ: bool) -> Option<bool> {
        let r = match kind {
            DeclKind::StaticAssert if templ => self.static_assert_decl(m),
            DeclKind::ConstevalBlock if templ => self.consteval_block_decl(m),
            _ => return None,
        };
        Some(matches!(r, Ok((_, true))))
    }

    /// Starts `spec`'s definition with the class-key of the definition
    /// instantiated, not of the declaration the specialization was named
    /// through: it sets the default member and base access
    /// ([class.access]/2).
    fn start_instantiated_definition(&mut self, spec: DeclId, def: DeclId) {
        if let (Some(t), Some(s)) =
            (self.sema.ast.decls.record(def).map(|r| r.tag), self.sema.ast.decls.record(spec))
            && s.tag != t
            && (s.tag == TagKind::Union) == (t == TagKind::Union)
        {
            self.sema.ast.decls.set_definition_tag(spec, t);
        }
        // clang's `InstantiateClass` copies the pattern's
        // `MaxFieldAlignmentAttr`: the `#pragma pack` in effect where the
        // pattern was defined applies, not the one where it is
        // instantiated (WP-782).
        let pat = self.sema.ast.decls.cold.record_pack.get(def).copied().unwrap_or(0);
        let saved = std::mem::replace(&mut self.sema.pack.0, pat);
        // Nor does `#pragma ms_struct` (the pattern's `ms_struct`, implied
        // or written, is copied with its attributes).
        let saved_ms = std::mem::replace(&mut self.sema.ms_struct_pragma, false);
        self.sema.act_on_start_record_definition(spec);
        self.sema.pack.0 = saved;
        self.sema.ms_struct_pragma = saved_ms;
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
        self.start_instantiated_definition(spec, def);
        self.class_final_and_attrs(spec, def);
        self.map_contexts(&[(def, spec)]);
        let sub = self;
        let (specs, anns, mut invalid) = sub.class_bases(spec, def);
        invalid |= !sub.sema.attach_bases_annotated(spec, &specs, &anns);
        // clang `CheckBaseSpecifier`: "if the base class is invalid the
        // derived class is as well" (`underlying_type<E>` deriving from an
        // `__underlying_type_impl<E>` whose `type` failed: a later
        // `underlying_type<E>::type` is quiet).
        invalid |= specs.iter().any(|b| {
            sub.sema.ast.types.as_record(b.ty).is_some_and(|r| {
                let d = sub.sema.ast.definition(r).unwrap_or(r);
                sub.sema.ast.decls.get(d).has(DeclFlags::INVALID)
            })
        });
        let members: Vec<DeclId> = sub.sema.ast.decls.members(def).collect();
        let mut nsdmis = Vec::new();
        // clang `InstantiateClass`: a base that fails to substitute, an
        // invalid member of the pattern (not instantiated; a member
        // function with a parameter naming no type too, WP-318), or one
        // that instantiates invalid makes the specialization invalid (its
        // default member initializers are not instantiated).
        for m in members {
            let n = *sub.sema.ast.decls.get(m);
            if n.flags().contains(DeclFlags::IMPLICIT) || sub.sema.is_implicit_friend(m) {
                continue;
            }
            if n.has(DeclFlags::INVALID)
                || n.kind() == DeclKind::FunctionTemplate
                    && sub.sema.pattern_of(m).is_some_and(|p| sub.sema.is_invalid_decl(p))
            {
                invalid = true;
                continue;
            }
            // A friend function defined in the class is a member of the
            // list as well as its `FriendDecl`'s target; the
            // `FriendDecl` redeclares it. So is a local class's friend
            // redeclaring a member of an enclosing local class
            // ([class.friend]/11).
            if matches!(n.kind(), DeclKind::Function | DeclKind::CXXMethod)
                && n.misc().friend() != kiln_ast::decl::FriendKind::None
            {
                continue;
            }
            if n.kind() == DeclKind::IndirectField {
                // Injected by its anonymous struct or union.
                continue;
            }
            // clang `InstantiateClass`: a failed static_assert (or consteval
            // block) makes the specialization invalid and stops its
            // instantiation (the later members are not declared).
            if let Some(failed) = sub.assert_like_failed(m, n.kind(), local.is_none()) {
                if failed {
                    invalid = true;
                    break;
                }
                continue;
            }
            // An anonymous struct or union of a local class has no
            // member functions of its own: it is instantiated as in a
            // class template, declaring its member object.
            if sub.is_unnamed_member_class(m) && (local.is_none() || sub.is_anonymous_record(m)) {
                let _ = sub.guarded_member(m, spec, |s| s.unnamed_member_class(m));
                continue;
            }
            let r = if local.is_some()
                && n.kind() == DeclKind::CXXRecord
                && n.misc().friend() == kiln_ast::decl::FriendKind::None
            {
                Ok(Some(sub.local_class(m, local.as_deref_mut())))
            } else {
                let salt = std::mem::replace(&mut sub.sema.inst.stack.member_salt, m.index());
                let r = sub.guarded_member(m, spec, |s| s.member(m));
                sub.sema.inst.stack.member_salt = salt;
                r
            };
            let data = matches!(n.kind(), DeclKind::Field | DeclKind::Var);
            invalid |= data && r.is_err();
            if let Ok(Some(x)) = r {
                // clang `InstantiateClass`: any member instantiated
                // invalid (a field, typedef or enumeration whose type
                // fails) makes the specialization invalid.
                invalid |= sub.sema.ast.decls.get(x).has(DeclFlags::INVALID);
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
        if invalid {
            sub.sema.ast.decls.set_flags(spec, DeclFlags::INVALID);
        }
        if !nsdmis.is_empty() && !invalid {
            sub.class_nsdmis(spec, local.is_some(), nsdmis);
        }
        if local.is_none() {
            sub.sema.act_on_finish_class_contexts(spec);
        }
        sub.entity = saved_entity;
    }

    /// The default member initializers `nsdmis` (instantiated field,
    /// pattern initializer) of class `spec` ([temp.inst]/3): instantiated
    /// when a constructor or an aggregate initialization uses one
    /// ([`Sema::instantiate_nsdmi`], clang `InstantiateInClassInitializer`),
    /// an anonymous member's included. Those of a local class (`local`,
    /// and of its anonymous members) are instantiated with the outermost
    /// local class's complete-class contexts, after the declaration parts
    /// (the parser replays them so; `LocalLate::Nsdmi`, WP-905).
    fn class_nsdmis(&mut self, spec: DeclId, local: bool, nsdmis: Vec<(DeclId, kiln_ast::ExprId)>) {
        let fields: Vec<_> =
            nsdmis.into_iter().filter(|&(f, _)| self.sema.kind_of(f) == DeclKind::Field).collect();
        if self.local_late.is_some() && (local || self.sema.decl_in_local_scope(spec)) {
            for (field, init) in fields {
                self.push_local_late(LocalLate::Nsdmi { field, init });
            }
        } else if !local {
            let ctx = (self.args.clone(), self.pairs.clone());
            let _ = self.sema.inst.nsdmi_ctx.insert(spec, ctx);
            for (field, init) in fields {
                self.sema.special.nsdmi.insert(field);
                let _ = self.sema.inst.deferred_nsdmis.insert(field, (spec, init));
            }
        } else {
            self.sema.act_on_start_delayed_class(spec);
            for (field, init) in fields {
                self.nsdmi(field, init);
            }
            self.sema.act_on_finish_delayed_class(spec);
        }
    }

    /// Instantiates pattern initializer `init` as `field`'s default
    /// member initializer, in the class scope. On failure the field is
    /// invalid, so a constructor using it is quiet ([class.base.init]/9.1,
    /// clang `InstantiateInClassInitializer`).
    pub(crate) fn nsdmi(&mut self, field: DeclId, init: kiln_ast::ExprId) {
        // The pattern's full-expression is rebuilt without its
        // `ExprWithCleanups` (clang `TransformInitializer`), so the
        // initializer's conversion to the field's type is inside the
        // instantiated one, as for a non-template class: a non-dependent
        // operand reused whole kept its own, inside the conversion, and a
        // use in a lifetime-extending context could not rebuild its
        // temporaries (spec/sema.md §7.6a, WP-1177).
        let ex = &self.sema.ast.exprs;
        let init =
            if ex.get(init).kind() == ExprKind::ExprWithCleanups { ex.operand(init) } else { init };
        self.sema.act_on_start_field_init(field);
        let style = self
            .sema
            .ast
            .decls
            .field(field)
            .map_or(kiln_ast::decl::InitStyle::C, kiln_ast::decl::FieldData::init_style);
        let i = if self.sema.ast.exprs.get(init).kind() == ExprKind::InitListExpr {
            self.init_list(init).map(Initializer::List)
        } else if style == kiln_ast::decl::InitStyle::List {
            // `T m{x}` built in the pattern (a construction, `x` only
            // value-dependent) goes back to its braced list and stays
            // direct-list-initialization ([dcl.init.general]/15), as for
            // a variable (clang `TransformInitializer`).
            match self.initializer(init, style) {
                Ok(Some(i)) => Ok(i),
                Ok(None) => self.expr(init).map(Initializer::Expr),
                Err(e) => Err(e),
            }
        } else {
            self.expr(init).map(Initializer::Expr)
        };
        if let Ok(i) = i {
            self.sema.act_on_field_init(field, i);
        } else {
            if self.sema.nsdmi_this.last().is_some_and(|&(f, _)| f == field) {
                self.sema.nsdmi_this.pop();
            }
            self.sema.ast.decls.set_flags(field, DeclFlags::INVALID);
        }
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
        // Qualifier errors and the qualifier's notes are located where
        // the nested-name-specifier begins, not at the name.
        let qloc = self.qualifier_begin(pnns, loc);
        let packed = self.sema.ast.types.nns_flags(pnns).contains(TypeFlags::UNEXPANDED_PACK);
        let nnss = if packed {
            // `using Bases::operator()...;` ([temp.variadic]/5): one
            // using-declarator per element.
            match self.expand_pattern(|s| s.nns(pnns, qloc))? {
                crate::templ::pack::Expansion::Elements(v) => v,
                crate::templ::pack::Expansion::Unexpanded(_) => {
                    return self.unimplemented("this using-declaration pack expansion", loc);
                }
            }
        } else {
            vec![self.nns(pnns, qloc)?]
        };
        let mut last = None;
        for nns in nnss {
            if self.nns_still_dependent(nns) {
                last = self.sema.still_dependent_class_using(nns, name, loc, inheriting).or(last);
                continue;
            }
            let ctx = self.nns_context(nns, qloc)?;
            let q = crate::using::Qualifier { ctx, nns, loc: qloc };
            let resolved = self.sema.kind_of(m) == DeclKind::Using;
            // [namespace.udecl]/19: a dependent qualifier's members are
            // access-checked once it is known (a resolved pattern was
            // checked when it was parsed).
            if !resolved && !inheriting {
                self.sema.check_using_declarator_access(Some(q), name, loc);
            }
            let reverse = std::mem::replace(&mut self.sema.reverse_using_shadows, resolved);
            last = self.sema.instantiate_class_using(q, name, loc, inheriting).or(last);
            self.sema.reverse_using_shadows = reverse;
        }
        Ok(last)
    }

    /// Redoes the dependent `using typename B<T>::name;` that pattern
    /// alias `m` stands for, with its substituted qualifier (clang's
    /// `UnresolvedUsingTypenameDecl` instantiation): a using-declaration
    /// and its shadows, or "'typename' keyword used on a non-type".
    fn member_using_typename(&mut self, m: DeclId, loc: Loc) -> Result<Option<DeclId>, SubstFail> {
        let Some(pnns) = self.sema.ast.using_qualifier(m) else { return Ok(None) };
        let Some(name) = self.sema.ast.decls.get(m).name().as_ident() else { return Ok(None) };
        let qloc = self.qualifier_begin(pnns, loc);
        let nns = self.nns(pnns, qloc)?;
        if self.nns_still_dependent(nns) {
            return Ok(Some(self.sema.still_dependent_using_typename(
                nns,
                DeclName::ident(name),
                loc,
            )));
        }
        let ctx = self.nns_context(nns, qloc)?;
        let q = crate::using::Qualifier { ctx, nns, loc: qloc };
        self.sema.check_using_declarator_access(Some(q), DeclName::ident(name), loc);
        Ok(self.sema.act_on_using_typename_declarator(
            loc,
            Some(q),
            DeclName::ident(name),
            loc,
            false,
            None,
        ))
    }

    /// Instantiates pattern member `m` of `spec` with `f`. A member whose
    /// declaration fails is diagnosed where it fails; one dropped with no
    /// error is a kiln bug, made an error here rather than a class
    /// silently missing a member (§4.6 "No silent drop", WP-1054).
    fn guarded_member<R>(
        &mut self,
        m: DeclId,
        spec: DeclId,
        f: impl FnOnce(&mut Self) -> Result<R, SubstFail>,
    ) -> Result<R, SubstFail> {
        let mark = self.sema.failure_mark();
        let r = f(self);
        if r.is_err() && self.sema.failed_silently_since(mark) {
            self.member_dropped(m, spec);
        }
        r
    }

    /// The internal error for pattern member `m` of `spec` that failed to
    /// instantiate with nothing diagnosed (§4.6).
    fn member_dropped(&mut self, m: DeclId, spec: DeclId) {
        let n = *self.sema.ast.decls.get(m);
        let name = self.sema.decl_name_str(n.name());
        let d = self
            .sema
            .diag(DiagId::sema_inst_member_dropped, n.loc())
            .arg(name)
            .arg(self.ctx_arg(spec));
        self.sema.emit(d);
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
            // An in-class partial specialization of a member variable
            // template, redeclared for the member template's
            // redeclaration ([temp.spec.partial.member]).
            DeclKind::Var if self.sema.ast.partial_spec_info(m).is_some() => {
                self.member_partial_spec(m)?;
                return Ok(None);
            }
            DeclKind::Var if self.sema.is_var_spec(m) => self.member_explicit_var_spec(m)?,
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
            DeclKind::UnresolvedUsingTypename => return self.member_using_typename(m, loc),
            DeclKind::Typedef | DeclKind::TypeAlias => self.typedef_decl(m)?,
            DeclKind::StaticAssert => {
                self.static_assert_decl(m)?;
                return Ok(None);
            }
            DeclKind::ConstevalBlock => {
                self.consteval_block_decl(m)?;
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
