//! Substitution into declarations (spec/sema-templates.md §5.6): the
//! members of a class template's pattern and the local declarations of
//! a function body are re-declared through the actions the parser uses,
//! from a decl-specifier and a declarator rebuilt out of the pattern
//! declaration with substituted types.

use kiln_ast::decl::{
    ConstexprKind, DeclKind, EnumData, FieldData, FnStorage, FriendTarget, InitStyle, ParamData,
    TlsKind, VarStorage,
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
            DeclNameKind::LiteralOperator | DeclNameKind::DeductionGuide => {
                return self.unimplemented(
                    "literal operators and deduction guides in class templates",
                    loc,
                );
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
    fn named_decl_ty(&mut self, d: DeclId, t: QualType, loc: Loc) -> SubstResult<QualType> {
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
            self.sema.act_on_init(v, i);
            Ok(())
        } else {
            self.sema.act_on_uninit(v);
            Err(SubstFail)
        }
    }

    /// The parameters of pattern function `f` with substituted types.
    fn param_infos(&mut self, f: DeclId) -> SubstResult<Vec<ParamInfo>> {
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
            if pd.is_some_and(|x| x.flags & ParamData::PACK != 0) {
                return self.unimplemented("function parameter packs", n.loc());
            }
            let t = pd.and_then(|x| x.original_ty).unwrap_or_else(|| self.decl_ty(p));
            let ty = self.ty(t, n.loc())?;
            let name = n.name().as_ident().filter(|_| !n.name().is_empty()).map(|s| (s, n.loc()));
            out.push(ParamInfo {
                name,
                ty,
                loc: n.loc(),
                attrs: Vec::new(),
                invalid: false,
                default_arg: DefaultArg::None,
                range: (n.loc(), n.loc()),
            });
        }
        Ok(out)
    }

    /// A function of the pattern (member or friend): declared with the
    /// substituted signature; its definition is instantiated when used
    /// (§4.4). Default arguments are substituted now (a gap: clang
    /// instantiates them when a call uses them).
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
        let ret = self.ty(p.ret(), loc)?;
        let exc = self.exc_spec(&p, loc)?;
        let params = self.param_infos(d)?;
        let mut ds = self.pattern_declspec(d, ret);
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
        }
        let base = if ds.base == TypeSpecKind::None { QualType::VOID } else { ret };
        let r = match friend {
            Some(fl) => self.sema.act_on_friend(&ds, base, Some(&dr), fl),
            None => self.sema.act_on_declarator(&ds, base, &dr),
        };
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
        if fs.is_some_and(kiln_ast::decl::FnSpec::is_pure) {
            self.sema.ast.decls.mark_function(f, kiln_ast::decl::FnSpec::new().with_pure(true));
        }
        // Defined in its class: inline before its body is instantiated.
        if fs.is_some_and(kiln_ast::decl::FnSpec::implicit_inline) {
            self.sema
                .ast
                .decls
                .mark_function(f, kiln_ast::decl::FnSpec::new().with_implicit_inline(true));
        }
        self.default_args(d, f);
        Ok(r)
    }

    /// The exception specification of function type `p`.
    fn exc_spec(
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

    /// The default arguments of pattern function `pat` on `f`.
    fn default_args(&mut self, pat: DeclId, f: DeclId) {
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
        for (&a, &b) in pp.iter().zip(fp.iter()) {
            self.locals.insert(a, DeclOrPack::Decl(b));
        }
        for (&a, &b) in pp.iter().zip(fp.iter()) {
            let Some(def) = self.sema.ast.decls.param(a).and_then(|x| x.default_arg) else {
                continue;
            };
            let e = match self.sema.ast.exprs.get(def).kind() {
                kiln_ast::expr::ExprKind::CXXDefaultArgExpr => continue,
                _ => self.expr(def),
            };
            if let Ok(e) = e {
                self.sema.act_on_param_default_arg(b, e);
            }
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
        self.sema.act_on_declarator(&ds, ty, &dr).ok_or(SubstFail)
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
        let defined = self.sema.ast.definition(d) == Some(d);
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
        let ens: Vec<DeclId> = self.sema.ast.decls.members(d).collect();
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
            FriendTarget::Decl(f) => {
                if !matches!(self.sema.kind_of(f), DeclKind::Function | DeclKind::CXXMethod) {
                    return self.unimplemented("friend templates", loc);
                }
                let ctx = DeclaratorContext::Member;
                self.function_decl(f, ctx, Some(loc)).map(Some)
            }
        }
    }
}
