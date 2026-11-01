//! Substitution into declarations (spec/sema-templates.md §5.6): the
//! members of a class template's pattern and the local declarations of
//! a function body are re-declared through the actions the parser uses,
//! from a decl-specifier and a declarator rebuilt out of the pattern
//! declaration with substituted types.

use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, EnumData, FieldData, FnStorage, FriendTarget, InitStyle,
    ParamData, TlsKind, VarStorage,
};
use kiln_ast::names::DeclNameKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, QualType};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use super::{DeclOrPack, SubstFail, SubstResult, Substituter};
use crate::declspec::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, DefaultArg, ParamInfo, SpecialName,
    StorageSpec, TypeSpecKind,
};

impl Substituter<'_, '_, '_> {
    /// The decl-specifiers of pattern declaration `d` naming type `ty`.
    pub(crate) fn pattern_declspec(&self, d: DeclId, ty: QualType) -> DeclSpec {
        let ast = &self.sema.ast;
        let n = *ast.decls.get(d);
        let loc = n.loc();
        let mut ds = DeclSpec::new(loc);
        ds.base = TypeSpecKind::Named;
        ds.named = Some(ty);
        ds.type_loc = Some(loc);
        if let Some(v) = ast.decls.var(d) {
            let s = v.spec;
            match s.storage() {
                VarStorage::Static => ds.storage = Some((StorageSpec::Static, loc)),
                VarStorage::Extern => ds.storage = Some((StorageSpec::Extern, loc)),
                VarStorage::Register | VarStorage::None => {}
            }
            if s.tls() != TlsKind::None {
                ds.thread_local = Some(loc);
            }
            if s.is_constexpr() {
                ds.constexpr = Some(loc);
            }
            if s.is_constinit() {
                ds.constinit = Some(loc);
            }
            if s.is_inline() {
                ds.inline = Some(loc);
            }
        } else if let Some(f) = ast.decls.function(d) {
            let s = f.spec;
            if s.storage() == FnStorage::Static {
                ds.storage = Some((StorageSpec::Static, loc));
            }
            if s.is_inline() {
                ds.inline = Some(loc);
            }
            match s.constexpr_kind() {
                ConstexprKind::Constexpr => ds.constexpr = Some(loc),
                ConstexprKind::Consteval => ds.consteval = Some(loc),
                ConstexprKind::None => {}
            }
            if s.virtual_written() {
                ds.virtual_ = Some(loc);
            }
            if s.is_explicit() {
                ds.explicit = Some(loc);
            }
        } else if let Some(f) = ast.decls.field(d)
            && f.is_mutable()
        {
            ds.storage = Some((StorageSpec::Mutable, loc));
        }
        ds
    }

    /// The declarator-id of pattern declaration `d` in context `ctx`.
    pub(crate) fn pattern_declarator(
        &mut self,
        d: DeclId,
        ctx: DeclaratorContext,
    ) -> SubstResult<Declarator> {
        let n = *self.sema.ast.decls.get(d);
        let loc = n.loc();
        let mut dr = Declarator::new(ctx, loc);
        let name = n.name();
        match name.kind() {
            DeclNameKind::Identifier => {
                if let Some(s) = name.as_ident()
                    && !name.is_empty()
                {
                    dr.name = Some((s, loc));
                }
            }
            DeclNameKind::Operator => {
                if let Some(op) = name.as_operator() {
                    dr.special = Some((SpecialName::Operator(op), loc));
                }
            }
            DeclNameKind::Constructor | DeclNameKind::Destructor => {
                let class = n.ctx().unwrap_or(self.sema.ast.tu);
                let s = self.class_symbol(class);
                dr.special = Some((
                    if name.kind() == DeclNameKind::Constructor {
                        SpecialName::Constructor(s)
                    } else {
                        SpecialName::Destructor(s)
                    },
                    loc,
                ));
            }
            DeclNameKind::Conversion => {
                let t = self.decl_ty(d);
                let ret = match self.sema.ast.types.view(t.ty()) {
                    TypeView::Function(p) => p.ret(),
                    _ => return Err(SubstFail),
                };
                let ret = self.ty(ret, loc)?;
                dr.special = Some((SpecialName::Conversion(ret), loc));
            }
            DeclNameKind::LiteralOperator => {
                if let Some(s) = name.as_literal_operator() {
                    dr.special = Some((SpecialName::LiteralOperator(s), loc));
                }
            }
            // A member class template's guide ([temp.deduct.guide]/3):
            // for the member template of the instantiated class (WP-134).
            DeclNameKind::DeductionGuide => {
                let kiln_ast::names::DeclNameView::DeductionGuide(t) =
                    self.sema.ast.names.view(name)
                else {
                    return Err(SubstFail);
                };
                let t = self.find_instantiated_decl(t, loc)?;
                dr.special = Some((SpecialName::DeductionGuide(t), loc));
            }
        }
        Ok(dr)
    }

    fn class_symbol(&self, class: DeclId) -> Symbol {
        self.sema.ast.decls.get(class).name().as_ident().unwrap_or(Symbol::from_index(0))
    }

    /// The type of declaration `d` as declared.
    pub(crate) fn decl_ty(&self, d: DeclId) -> QualType {
        self.sema.ast.decls.get(d).ty().unwrap_or(QualType::ERROR)
    }

    /// `t`, the type of pattern declaration `d`, substituted with `d`'s
    /// name available to diagnostics.
    pub(crate) fn named_decl_ty(
        &mut self,
        d: DeclId,
        t: QualType,
        loc: Loc,
    ) -> SubstResult<QualType> {
        let name = self.sema.ast.decls.get(d).name().as_ident().map(|n| (n, loc));
        let saved = std::mem::replace(&mut self.declarator_name, name);
        let r = self.ty(t, loc);
        self.declarator_name = saved;
        r
    }

    /// A variable of the pattern (local or static data member): declared
    /// with the substituted type, then initialized (§5.6).
    pub(crate) fn var_decl(&mut self, d: DeclId, ctx: DeclaratorContext) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        let ty = self.named_decl_ty(d, t, loc)?;
        let ds = self.pattern_declspec(d, ty);
        let dr = self.pattern_declarator(d, ctx)?;
        let Some(v) = self.sema.act_on_declarator(&ds, ty, &dr) else { return Err(SubstFail) };
        self.locals.insert(d, DeclOrPack::Decl(v));
        self.var_init(d, v)?;
        Ok(v)
    }

    /// Pattern decomposition `d` ([dcl.struct.bind], clang
    /// `TemplateDeclInstantiator::VisitDecompositionDecl`): its bindings
    /// anew, then the variable from its declared type, initialized and
    /// decomposed as written.
    pub(crate) fn decomp_decl(&mut self, d: DeclId, ctx: DeclaratorContext) -> SubstResult<DeclId> {
        let (_, v) = self.decomp_declare(d, ctx)?;
        self.var_init(d, v)?;
        Ok(v)
    }

    /// Declares pattern decomposition `d` uninitialized: the declaration
    /// specifiers used and the variable.
    pub(crate) fn decomp_declare(
        &mut self,
        d: DeclId,
        ctx: DeclaratorContext,
    ) -> SubstResult<(DeclSpec, DeclId)> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.sema.decomp_written.get(&d).copied().unwrap_or_else(|| self.decl_ty(d));
        let ty = self.named_decl_ty(d, t, loc)?;
        let ds = self.pattern_declspec(d, ty);
        let mut dr = Declarator::new(ctx, loc);
        let old = self.sema.decomp_binding_list(d);
        let ids = old
            .iter()
            .map(|&b| {
                let n = self.sema.ast.decls.get(b);
                crate::declspec::BindingId {
                    name: n.name().as_ident().unwrap_or(Symbol::from_index(0)),
                    loc: n.loc(),
                    ellipsis: None,
                    attrs: Vec::new(),
                }
            })
            .collect();
        let dc = crate::declspec::Decomposition { lbracket: loc, rbracket: loc, ids };
        dr.decomp = Some(dc.clone());
        let invalid = self.sema.is_error_type(ty);
        let v = self.sema.declare_decomposition(&ds, ty, &dr, &dc, invalid);
        for (&b, nb) in old.iter().zip(self.sema.decomp_binding_list(v)) {
            self.locals.insert(b, DeclOrPack::Decl(nb));
        }
        self.locals.insert(d, DeclOrPack::Decl(v));
        Ok((ds, v))
    }

    /// Whether pattern static data member `d` is declared without its
    /// initializer ([temp.inst]/4): an inline one whose type does not
    /// come from the initializer (`auto`, an array of unknown bound).
    pub(crate) fn defers_init(&self, d: DeclId) -> bool {
        let ast = &self.sema.ast;
        let Some(v) = ast.decls.var(d) else { return false };
        let t = self.decl_ty(d);
        // A constexpr one without an initializer is diagnosed only when
        // its definition is instantiated (as clang).
        let missing = v.init.is_none() && v.spec.is_constexpr();
        (v.init.is_some() || missing)
            && (v.spec.is_inline() || v.spec.is_constexpr())
            && v.spec.storage() == VarStorage::Static
            && !matches!(ast.types.view(t.ty()), TypeView::Auto { .. })
            && !matches!(ast.types.view(ast.types.canonical(t).ty()), TypeView::IncompleteArray(_))
    }

    /// Declares pattern static data member `d` with its initializer
    /// deferred (see [`Self::defers_init`]); not `inline` until then.
    pub(crate) fn deferred_static_member(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        let ty = self.named_decl_ty(d, t, loc)?;
        let mut ds = self.pattern_declspec(d, ty);
        ds.inline = None;
        let dr = self.pattern_declarator(d, DeclaratorContext::Member)?;
        let saved = std::mem::replace(&mut self.sema.inst.deferring_init, true);
        let v = self.sema.act_on_declarator(&ds, ty, &dr);
        self.sema.inst.deferring_init = saved;
        let Some(v) = v else { return Err(SubstFail) };
        self.locals.insert(d, DeclOrPack::Decl(v));
        if self.sema.kind_of(v) == DeclKind::Var {
            self.sema.inst.deferred_inits.insert(v);
        }
        Ok(v)
    }

    /// Initializes `v` as pattern variable `d` is.
    pub(crate) fn var_init(&mut self, d: DeclId, v: DeclId) -> SubstResult<()> {
        let Some(data) = self.sema.ast.decls.var(d).copied() else { return Ok(()) };
        if self.sema.kind_of(v) != DeclKind::Var {
            return Ok(());
        }
        let Some(init) = data.init else {
            self.sema.act_on_uninit(v);
            return Ok(());
        };
        let style = data.spec.init_style();
        if let Ok(i) = self.initializer(init, style) {
            match i {
                Some(i) => self.sema.act_on_init(v, i),
                None => self.sema.act_on_uninit(v),
            }
            Ok(())
        } else {
            // clang marks the declaration invalid: no follow-up
            // "must be initialized" diagnostic.
            self.sema.ast.decls.set_flags(v, DeclFlags::INVALID);
            self.init_failures += 1;
            Err(SubstFail)
        }
    }

    /// Maps the parameters of pattern function `f` to parameters with
    /// substituted types, for a constraint of `f` that names them (its
    /// trailing requires-clause, [basic.scope.param]; clang's
    /// `addInstantiatedParametersToScope`).
    pub(crate) fn seed_function_params(&mut self, f: DeclId) -> SubstResult<()> {
        for p in self.param_infos(f)? {
            if let Some(d) = p.decl {
                self.sema.unowned_params.remove(&d);
            }
        }
        Ok(())
    }

    /// The parameters of pattern function `f` with substituted types.
    pub(crate) fn param_infos(&mut self, f: DeclId) -> SubstResult<Vec<ParamInfo>> {
        let params: Vec<DeclId> = self
            .sema
            .ast
            .decls
            .function(f)
            .map(|x| self.sema.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        let mut out = Vec::with_capacity(params.len());
        for p in params {
            let n = *self.sema.ast.decls.get(p);
            let pd = self.sema.ast.decls.param(p).copied();
            let t = pd.and_then(|x| x.original_ty).unwrap_or_else(|| self.decl_ty(p));
            let name = n.name().as_ident().filter(|_| !n.name().is_empty()).map(|s| (s, n.loc()));
            if pd.is_some_and(|x| x.flags & ParamData::PACK != 0)
                && let Some(pat) = crate::templ::pack::type_pattern(self.sema, t)
            {
                // A function parameter pack ([temp.variadic]/2): one
                // parameter per element (§7.1).
                let loc = n.loc();
                let tys = match self.expand_pattern(|s| s.element_ty(pat, loc))? {
                    crate::templ::pack::Expansion::Elements(v) => v,
                    crate::templ::pack::Expansion::Unexpanded(x) => {
                        // Still a pack: a parameter pack of a template
                        // being redeclared (a generic lambda's) whose
                        // own levels stay.
                        let ty = self.sema.ast.types.pack_expansion(x, None);
                        let ctx = self.sema.ctx;
                        let idx = u16::try_from(out.len()).unwrap_or(u16::MAX);
                        let np = self.sema.builder().prototype_param(
                            ctx,
                            n.name(),
                            loc,
                            ty,
                            idx,
                            ParamData::PACK,
                        );
                        self.sema.unowned_params.insert(np);
                        self.locals.insert(p, DeclOrPack::Decl(np));
                        out.push(ParamInfo {
                            name,
                            ty,
                            loc,
                            attrs: Vec::new(),
                            invalid: false,
                            default_arg: DefaultArg::None,
                            range: (loc, loc),
                            decl: Some(np),
                            explicit_object: None,
                        });
                        continue;
                    }
                };
                let mut decls = Vec::with_capacity(tys.len());
                for ty in tys {
                    let ctx = self.sema.ctx;
                    let idx = u16::try_from(out.len()).unwrap_or(u16::MAX);
                    let np = self.sema.builder().prototype_param(ctx, n.name(), loc, ty, idx, 0);
                    self.sema.unowned_params.insert(np);
                    decls.push(np);
                    // The elements share the pack's name (carried by
                    // `np`); it is not a redefinition ([temp.variadic]/5).
                    out.push(ParamInfo {
                        name: None,
                        ty,
                        loc,
                        attrs: Vec::new(),
                        invalid: false,
                        default_arg: DefaultArg::None,
                        range: (loc, loc),
                        decl: Some(np),
                        explicit_object: None,
                    });
                }
                let l = self.sema.ast.lists.push(&decls);
                self.locals.insert(p, DeclOrPack::Pack(l));
                continue;
            }
            let ty = self.ty(t, n.loc())?;
            // Created now, so that the rest of the signature (a trailing
            // return type, the exception specification) refers to it
            // ([basic.scope.param]); the function adopts it.
            let ctx = self.sema.ctx;
            let idx = u16::try_from(out.len()).unwrap_or(u16::MAX);
            let eo = pd.map_or(0, |x| x.flags & ParamData::EXPLICIT_OBJECT);
            let np = self.sema.builder().prototype_param(ctx, n.name(), n.loc(), ty, idx, eo);
            self.sema.unowned_params.insert(np);
            self.locals.insert(p, DeclOrPack::Decl(np));
            out.push(ParamInfo {
                name,
                ty,
                loc: n.loc(),
                attrs: Vec::new(),
                invalid: false,
                default_arg: DefaultArg::None,
                range: (n.loc(), n.loc()),
                decl: Some(np),
                explicit_object: (eo != 0).then_some(n.loc()),
            });
        }
        Ok(out)
    }

    /// A function of the pattern (member or friend): declared with the
    /// substituted signature; its definition is instantiated when used
    /// (§4.4). Default arguments are substituted now (a gap: clang
    /// instantiates them when a call uses them).
    #[allow(
        clippy::too_many_lines,
        reason = "the 112 lazy exception specs and the 109 constrained/friend members meet here"
    )]
    pub(crate) fn function_decl(
        &mut self,
        d: DeclId,
        ctx: DeclaratorContext,
        friend: Option<Loc>,
    ) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        let TypeView::Function(p) = self.sema.ast.types.view(t.ty()) else {
            return self.unimplemented("function declared with a typedef of function type", loc);
        };
        let info = p.info();
        // The parameters first: the return type and exception
        // specification may name them ([basic.scope.param]), and `this`
        // ([expr.prim.this]/2).
        let params = self.param_infos(d);
        let method = friend.is_none() && self.sema.kind_of(d) == DeclKind::CXXMethod;
        let lazy_exc = friend.is_none() && self.sema.lazy_exc_spec(d, &p);
        let this = self.sema.act_on_start_this_scope(method, info.this_quals());
        let sig = params.and_then(|params| {
            let ret = self.ty(p.ret(), loc)?;
            // A dependent `noexcept(expr)` waits until needed (WP-112).
            let exc = if lazy_exc {
                kiln_ast::types::ExcSpec::Deferred(d)
            } else {
                self.exc_spec(&p, loc)?
            };
            Ok((params, ret, exc))
        });
        self.sema.act_on_end_this_scope(this);
        let (params, ret, exc) = match sig {
            Ok(x) => x,
            Err(e) => {
                self.forget_pattern_params(d);
                return Err(e);
            }
        };
        let mut ds = self.pattern_declspec(d, ret);
        // `explicit(expr)`: substituted, so the declaration resolves it
        // when it no longer depends on these levels ([dcl.fct.spec]/3,
        // [temp.inst]/3; clang `InstantiateExplicitSpecifier`).
        if ds.explicit.is_some()
            && let Some(&e) = self.sema.ast.decls.cold.explicit_expr.get(d)
        {
            match self.expr(e) {
                Ok(e) => ds.explicit_expr = Some(e),
                Err(e) => {
                    self.forget_pattern_params(d);
                    return Err(e);
                }
            }
        }
        ds.friend = friend;
        let mut dr = self.pattern_declarator(d, ctx)?;
        if matches!(
            self.sema.ast.decls.get(d).name().kind(),
            DeclNameKind::Constructor | DeclNameKind::Destructor | DeclNameKind::Conversion
        ) {
            ds.base = TypeSpecKind::None;
            ds.named = None;
        }
        dr.chunks.push(Chunk::Function {
            params,
            variadic: info.variadic(),
            loc,
            rparen: loc,
            exc,
            quals: info.this_quals(),
            ref_qual: info.ref_qual(),
            trailing: None,
            ambiguous: false,
        });
        let fs = self.sema.ast.decls.function(d).map(|x| x.spec);
        if let Some(s) = fs {
            if s.is_override() {
                dr.virt_override = Some(loc);
            }
            if s.is_final() {
                dr.virt_final = Some(loc);
            }
            // The pure-specifier: declared again so the instantiation is
            // created pure (`is_pure` is not a late bit; [class.abstract]/2).
            if s.is_pure() {
                dr.pure = Some(loc);
            }
        }
        // The pattern's trailing requires-clause, unsubstituted: checked
        // with this instantiation's levels ([temp.inst]/17, clang
        // `InstantiateFunctionDeclaration` keeps it).
        let requires = self.sema.ast.templates.trailing_requires.get(d).copied();
        dr.trailing_requires = requires;
        let base = if ds.base == TypeSpecKind::None { QualType::VOID } else { ret };
        let r = match friend {
            Some(fl) => self.sema.act_on_friend(&ds, base, Some(&dr), fl),
            None => self.sema.act_on_declarator(&ds, base, &dr),
        };
        self.forget_pattern_params(d);
        let Some(r) = r else { return Err(SubstFail) };
        // The befriended function (act_on_friend returns the FriendDecl).
        let f = if self.sema.kind_of(r) == DeclKind::Friend {
            match self.sema.ast.decls.friend_target(r) {
                FriendTarget::Decl(f) => f,
                FriendTarget::Type { .. } => return Ok(r),
            }
        } else {
            r
        };
        // A befriended function template (`template<class U> friend R
        // g(U) { ... }`, via `friend_function_template`): the rest applies
        // to its pattern, the function.
        let f = if self.sema.kind_of(f) == DeclKind::FunctionTemplate {
            match self.sema.pattern_of(f) {
                Some(p) => p,
                None => return Ok(r),
            }
        } else {
            f
        };
        if self.sema.ast.decls.function(f).is_none() {
            // Recovery declared something else (an invalid redeclaration).
            return Ok(r);
        }
        if lazy_exc {
            let first = self.sema.ast.first(f);
            self.sema.uninstantiated_exc.insert(first, d);
        }
        if requires.is_some() {
            self.sema.inst.concepts.member_constraints.insert(f, (d, self.args.clone()));
        }
        // A friend defined in the class: its definition is instantiated
        // from the pattern when used ([temp.inst]/4, clang's
        // `isThisDeclarationInstantiatedFromAFriendDefinition`).
        if friend.is_some()
            && self.sema.ast.decls.function(d).is_some_and(|x| x.body.is_some())
            && self.sema.ast.definition(f).is_none()
            && self.sema.ast.instantiated_from(f).is_none()
        {
            self.member_link(f, d);
        }
        // Defined in its class: inline before its body is instantiated.
        if fs.is_some_and(kiln_ast::decl::FnSpec::implicit_inline) {
            self.sema
                .ast
                .decls
                .mark_function(f, kiln_ast::decl::FnSpec::new().with_implicit_inline(true));
        }
        self.defer_default_args(d, f);
        // `= delete` / `= default` on the pattern's declaration: declared
        // again on the instantiation, where its checks run with the class
        // (clang `InstantiateFunctionDeclaration` → `SetDeclDeleted` /
        // `SetDeclDefaulted`; spec/sema-class.md §4.4).
        if let Some(s) = fs {
            if s.is_deleted() {
                let msg = self.sema.ast.decls.cold.delete_message.get(d).copied();
                self.sema.act_on_deleted(f, loc, msg);
            } else if s.defaulted() == kiln_ast::decl::Defaulted::Explicit
                && (friend.is_none() || self.sema.cmp_kind(f).is_some())
            {
                self.sema.act_on_defaulted(f, loc);
            }
        }
        Ok(r)
    }

    /// The signature of function template specialization `spec` of
    /// pattern `pat` (§6.1 step 4, clang's `SubstFunctionType` under
    /// `FinishTemplateArgumentDeduction`): parameters, return type and
    /// exception specification substituted, parameter types adjusted
    /// ([dcl.fct]/5), default arguments substituted.
    pub(crate) fn function_spec_signature(&mut self, pat: DeclId, spec: DeclId) -> SubstResult<()> {
        let loc = self.sema.ast.decls.get(pat).loc();
        let t = self.decl_ty(pat);
        let TypeView::Function(p) = self.sema.ast.types.view(t.ty()) else {
            return self.unimplemented("function declared with a typedef of function type", loc);
        };
        let info = p.info();
        let params = self.param_infos(pat);
        let method = self.sema.kind_of(pat) == DeclKind::CXXMethod;
        let this = self.sema.act_on_start_this_scope(method, info.this_quals());
        let sig = params.and_then(|params| {
            let ret = self.ty(p.ret(), loc)?;
            let exc = self.exc_spec(&p, loc)?;
            Ok((params, ret, exc))
        });
        self.sema.act_on_end_this_scope(this);
        let (params, ret, exc) = match sig {
            Ok(x) => x,
            Err(e) => {
                self.forget_pattern_params(pat);
                return Err(e);
            }
        };
        let mut ptys = Vec::with_capacity(params.len());
        let mut ids = Vec::with_capacity(params.len());
        for pi in &params {
            let types = &mut self.sema.ast.types;
            let mut ty = pi.ty;
            if types.is_array(ty) {
                let e = types.element(ty).unwrap_or(QualType::ERROR);
                ty = types.pointer_to(e);
            } else if types.is_function(ty) {
                ty = types.pointer_to(ty);
            }
            ptys.push(ty);
            if let Some(d) = pi.decl {
                self.sema.unowned_params.remove(&d);
                self.sema.ast.decls.set_param_owner(d, spec);
                self.sema.ast.decls.set_ty(d, ty);
                ids.push(d);
            }
        }
        let fty = self.sema.ast.types.function(ret, &ptys, info, exc);
        self.sema.ast.decls.set_ty(spec, fty);
        let _ = self.sema.builder().set_params(spec, &ids);
        self.forget_pattern_params(pat);
        self.defer_default_args(pat, spec);
        Ok(())
    }

    /// The explicit-specifier of function template specialization
    /// `spec`: the pattern's `explicit(expr)` substituted, as part of
    /// deduction ([temp.deduct.general]/7, so a failure is a deduction
    /// failure), and resolved ([dcl.fct.spec]/3): `explicit(false)` makes
    /// the specialization a converting constructor (conversion function).
    pub(crate) fn function_spec_explicit(&mut self, pat: DeclId, spec: DeclId) -> SubstResult<()> {
        if !self.sema.fn_spec(pat).is_explicit() {
            return Ok(());
        }
        let Some(&e) = self.sema.ast.decls.cold.explicit_expr.get(pat) else { return Ok(()) };
        let x = self.sema.ast.exprs.get(e);
        if !x.is_value_dependent() && !x.is_type_dependent() {
            return Ok(());
        }
        let e = self.expr(e)?;
        match self.sema.explicit_value(e) {
            Some(true) => {}
            Some(false) => {
                let s = self.sema.fn_spec(spec).with_explicit(false);
                self.sema.ast.decls.respecify_function(spec, s);
            }
            None => {
                let x = self.sema.ast.exprs.get(e);
                if !x.is_value_dependent() && !x.is_type_dependent() {
                    return Err(SubstFail);
                }
                let _ = self.sema.ast.decls.cold.explicit_expr.insert(spec, e);
            }
        }
        Ok(())
    }

    /// Drops the mapping of pattern function `pat`'s parameters
    /// ([`Self::param_infos`]).
    pub(crate) fn forget_pattern_params(&mut self, pat: DeclId) {
        let pp: Vec<DeclId> = self
            .sema
            .ast
            .decls
            .function(pat)
            .map(|x| self.sema.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        for a in pp {
            self.locals.remove(&a);
        }
    }

    /// The exception specification of function type `p`.
    pub(crate) fn exc_spec(
        &mut self,
        p: &kiln_ast::types::FuncProto,
        loc: Loc,
    ) -> SubstResult<kiln_ast::types::ExcSpec> {
        use kiln_ast::types::{ExcSpec, NoexceptValue};
        Ok(match self.sema.ast.types.exc_spec(p) {
            ExcSpec::NoexceptExpr { expr, value: NoexceptValue::Dependent(_) } => {
                let e = self.expr(expr)?;
                self.sema.act_on_noexcept_spec(e, loc, loc)
            }
            ExcSpec::Dynamic(_) | ExcSpec::DependentNoexcept(_) => {
                return self.unimplemented("dependent exception specification", loc);
            }
            other => other,
        })
    }

    /// The default arguments of pattern function `pat` on `f`, recorded
    /// to be instantiated at their first use ([temp.inst]/12, clang
    /// `setUninstantiatedDefaultArg`): `f`'s parameter is marked
    /// `DEFAULT_ARG_PENDING` and mapped to the pattern's parameter
    /// (the original pattern's, when `pat`'s default is itself pending).
    /// A function in a local scope (a local class's member) gets them now,
    /// as clang (`isInLocalScopeForInstantiation`).
    pub(crate) fn defer_default_args(&mut self, pat: DeclId, f: DeclId) {
        if self.sema.decl_in_local_scope(f) {
            self.default_args(pat, f);
            return;
        }
        let params = |s: &Self, x: DeclId| -> Vec<DeclId> {
            s.sema
                .ast
                .decls
                .function(x)
                .map(|fd| s.sema.ast.lists.iter::<DeclId>(fd.params).collect())
                .unwrap_or_default()
        };
        let pp = params(self, pat);
        let fp = params(self, f);
        if pp.len() != fp.len() {
            // A pack expanded: substitute now.
            self.default_args(pat, f);
            return;
        }
        for (&a, &b) in pp.iter().zip(fp.iter()) {
            let src = if let Some(&src) = self.sema.inst.lazy_defaults.get(&a) {
                src
            } else {
                match self.sema.ast.decls.param(a).and_then(|x| x.default_arg) {
                    Some(def)
                        if self.sema.ast.exprs.get(def).kind()
                            != kiln_ast::expr::ExprKind::CXXDefaultArgExpr =>
                    {
                        (pat, a)
                    }
                    _ => continue,
                }
            };
            self.sema.ast.decls.mark_default_arg_pending(b);
            self.sema.inst.lazy_defaults.insert(b, src);
        }
    }

    /// The default argument of parameter `b` of `f`, from parameter `a`
    /// of pattern function `pat` (`f`'s parameters are mapped to
    /// `pat`'s, so the default can name an earlier parameter).
    pub(crate) fn lazy_default_arg(&mut self, pat: DeclId, a: DeclId, f: DeclId, b: DeclId) {
        let params = |s: &Self, x: DeclId| -> Vec<DeclId> {
            s.sema
                .ast
                .decls
                .function(x)
                .map(|fd| s.sema.ast.lists.iter::<DeclId>(fd.params).collect())
                .unwrap_or_default()
        };
        let pp = params(self, pat);
        let fp = params(self, f);
        self.map_params(&pp, &fp);
        self.default_arg_one(a, b);
        for a in pp {
            self.locals.remove(&a);
        }
    }

    /// Substitutes the default argument of pattern parameter `a` onto `b`.
    fn default_arg_one(&mut self, a: DeclId, b: DeclId) {
        let Some(def) = self.sema.ast.decls.param(a).and_then(|x| x.default_arg) else {
            return;
        };
        let e = match self.sema.ast.exprs.get(def).kind() {
            kiln_ast::expr::ExprKind::CXXDefaultArgExpr => return,
            // `= {...}` with a dependent type or element: the list
            // as written, list-initializing the substituted type.
            kiln_ast::expr::ExprKind::InitListExpr if self.needs_rebuild(def) => {
                if let Ok(l) = self.init_list(def) {
                    self.sema.act_on_param_default_list(b, &l);
                }
                return;
            }
            _ => self.expr(def),
        };
        if let Ok(e) = e {
            self.sema.act_on_param_default_arg(b, e);
        }
    }

    /// The default arguments of pattern function `pat` on `f`.
    pub(crate) fn default_args(&mut self, pat: DeclId, f: DeclId) {
        let pp: Vec<DeclId> = self
            .sema
            .ast
            .decls
            .function(pat)
            .map(|x| self.sema.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        let fp: Vec<DeclId> = self
            .sema
            .ast
            .decls
            .function(f)
            .map(|x| self.sema.ast.lists.iter::<DeclId>(x.params).collect())
            .unwrap_or_default();
        self.map_params(&pp, &fp);
        for (&a, &b) in pp.iter().zip(fp.iter()) {
            self.default_arg_one(a, b);
        }
        for a in pp {
            self.locals.remove(&a);
        }
    }

    /// A field of the pattern; its default member initializer is
    /// substituted after the class is complete (§4.6).
    pub(crate) fn field_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        let ty = self.named_decl_ty(d, t, loc)?;
        let ds = self.pattern_declspec(d, ty);
        let mut dr = self.pattern_declarator(d, DeclaratorContext::Member)?;
        let fd: Option<FieldData> = self.sema.ast.decls.field(d).copied();
        if let Some(w) = fd.and_then(|x| x.bit_width) {
            dr.bit_width = Some(self.expr(w)?);
        }
        if fd.and_then(|x| x.init).is_some() {
            dr.init_style = Some(InitStyle::C);
        }
        let f = self.sema.act_on_declarator(&ds, ty, &dr).ok_or(SubstFail)?;
        // Where the declaration begins ("declared here" for its default
        // member initializer).
        if let Some(&b) = self.sema.ast.decls.cold.begin_loc.get(d) {
            let _ = self.sema.ast.decls.cold.begin_loc.insert(f, b);
        }
        Ok(f)
    }

    /// A typedef or alias-declaration of the pattern.
    pub(crate) fn typedef_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        self.typedef_decl_in(d, DeclaratorContext::Member)
    }

    /// A typedef or alias-declaration of the pattern in context `ctx`.
    pub(crate) fn typedef_decl_in(
        &mut self,
        d: DeclId,
        ctx: DeclaratorContext,
    ) -> SubstResult<DeclId> {
        let n = *self.sema.ast.decls.get(d);
        let loc = n.loc();
        let u = self.sema.ast.decls.typedef(d).map(|x| x.underlying).ok_or(SubstFail)?;
        let ty = self.ty(u, loc)?;
        if n.kind() == DeclKind::TypeAlias {
            let name = n.name().as_ident().ok_or(SubstFail)?;
            return Ok(self.sema.act_on_alias_decl(name, loc, ty, &[]));
        }
        let mut ds = self.pattern_declspec(d, ty);
        ds.storage = Some((StorageSpec::Typedef, loc));
        let dr = self.pattern_declarator(d, ctx)?;
        self.sema.act_on_declarator(&ds, ty, &dr).ok_or(SubstFail)
    }

    /// A `static_assert` of the pattern: evaluated now ([temp.inst]/3).
    pub(crate) fn static_assert_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let Some(sa) = self.sema.ast.decls.static_assert(d).copied() else { return Err(SubstFail) };
        let c = self.expr(sa.cond)?;
        let m = self.opt_expr(sa.message)?;
        Ok(self.sema.act_on_static_assert(c, m, loc))
    }

    /// An enumeration of the pattern: defined now (§4.6; clang defines
    /// member enumerations with the class).
    pub(crate) fn enum_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let n = *self.sema.ast.decls.get(d);
        let loc = n.loc();
        let Some(data) = self.sema.ast.decls.enumeration(d).copied() else { return Err(SubstFail) };
        let fixed = if data.is_fixed() {
            match data.int_type {
                Some(t) => Some(self.ty(t, loc)?),
                None => None,
            }
        } else {
            None
        };
        let scoped = data.flags & (EnumData::SCOPED | EnumData::SCOPED_STRUCT);
        let name = n.name().as_ident().filter(|_| !n.name().is_empty()).map(|s| (s, loc));
        // A member enumeration defined out of line before the class is
        // instantiated (`template<class T> enum class A<T>::E { ... }`,
        // [temp.mem.enum]) is defined from that definition.
        let src = self.sema.ast.definition(d).filter(|&x| {
            x == d || self.sema.ast.decls.get(x).ctx() == self.sema.ast.decls.get(d).ctx()
        });
        let defined = src.is_some();
        let use_ = if defined {
            crate::decl::TagUse::Definition
        } else {
            crate::decl::TagUse::Declaration
        };
        let kw = kiln_ast::types::TagKeyword::Enum;
        let e = self.sema.act_on_tag(kw, name, None, use_, scoped, fixed, loc);
        if !defined {
            return Ok(e);
        }
        self.sema.act_on_start_enum_definition(e);
        let ens: Vec<DeclId> = self.sema.ast.decls.members(src.unwrap_or(d)).collect();
        for en in ens {
            let m = *self.sema.ast.decls.get(en);
            if m.kind() != DeclKind::EnumConstant {
                continue;
            }
            let init = self.sema.ast.decls.enumerator(en).and_then(|x| x.init);
            let init = match init {
                Some(i) => Some(self.expr(i)?),
                None => None,
            };
            let Some(s) = m.name().as_ident() else { continue };
            let x = self.sema.act_on_enumerator(s, m.loc(), init, &[]);
            self.locals.insert(en, DeclOrPack::Decl(x));
            self.member_link(x, en);
            // Found from the enumeration too (`FindInstantiatedDecl`
            // through the instantiated enum, as from a member class).
            self.sema.ast.templates.record_member(e, en, x);
        }
        self.sema.act_on_finish_enum_definition(e);
        Ok(e)
    }

    /// Records that `inst` was instantiated from pattern member `pattern`
    /// of the specialization being instantiated.
    pub(crate) fn member_link(&mut self, inst: DeclId, pattern: DeclId) {
        if let Some(spec) = self.entity {
            self.sema.builder().member_of_spec(spec, inst, pattern);
        }
    }

    /// A friend declaration of the pattern ([temp.friend]).
    pub(crate) fn friend_decl(&mut self, d: DeclId) -> SubstResult<Option<DeclId>> {
        let loc = self.sema.ast.decls.get(d).loc();
        if let Some(fs) = self.sema.friend_specs.get(&d).cloned() {
            // `friend bool operator==<>(const X&, const X&);`
            // ([temp.friend]/1.3).
            let ty = self.ty(fs.ty, loc)?;
            let args = fs
                .args
                .iter()
                .map(|&a| self.template_arg(a, loc))
                .collect::<SubstResult<Vec<_>>>()?;
            let record = self.sema.current_class().map(|c| c.record).ok_or(SubstFail)?;
            return Ok(self.sema.instantiate_friend_spec(record, &fs, ty, &args));
        }
        match self.sema.ast.decls.friend_target(d) {
            FriendTarget::Type { ty, owned_tag } => {
                if owned_tag.is_some() {
                    return self.unimplemented("friend class declared in a class template", loc);
                }
                let ty = self.ty(ty, loc)?;
                let mut ds = DeclSpec::new(loc);
                ds.base = TypeSpecKind::Named;
                ds.named = Some(ty);
                ds.type_loc = Some(loc);
                ds.friend = Some(loc);
                Ok(self.sema.act_on_friend(&ds, ty, None, loc))
            }
            FriendTarget::Decl(t) if self.sema.kind_of(t) == DeclKind::ClassTemplate => {
                // `template<class U> friend class X;` names a namespace
                // scope template, which substitution leaves unchanged.
                let record = self.sema.current_class().map(|c| c.record).ok_or(SubstFail)?;
                let fd = self.sema.builder().friend(record, loc, FriendTarget::Decl(t));
                let first = self.sema.ast.first(t);
                self.sema.push_friend(first);
                Ok(Some(fd))
            }
            FriendTarget::Decl(t) if self.sema.kind_of(t) == DeclKind::FunctionTemplate => {
                self.friend_function_template(t, loc).map(Some)
            }
            FriendTarget::Decl(f) => {
                if !matches!(self.sema.kind_of(f), DeclKind::Function | DeclKind::CXXMethod) {
                    return self.unimplemented("this kind of friend declaration", loc);
                }
                let ctx = DeclaratorContext::Member;
                let r = self.function_decl(f, ctx, Some(loc))?;
                if let FriendTarget::Decl(x) = self.sema.ast.decls.friend_target(r)
                    && self.sema.ast.instantiated_from(x).is_none()
                {
                    self.member_link(x, f);
                }
                Ok(Some(r))
            }
        }
    }
}
