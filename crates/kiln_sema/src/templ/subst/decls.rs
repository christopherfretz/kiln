//! Substitution into declarations (spec/sema-templates.md §5.6): the
//! members of a class template's pattern and the local declarations of
//! a function body are re-declared through the actions the parser uses,
//! from a decl-specifier and a declarator rebuilt out of the pattern
//! declaration with substituted types.

use kiln_ast::decl::{
    ConstexprKind, DeclFlags, DeclKind, EnumData, FieldData, FnStorage, FriendTarget, ParamData,
    TlsKind, VarStorage,
};
use kiln_ast::ids::TemplateParamListId;
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
use crate::templ::stack::{FrameKind, SfinaeState};
use crate::templ::written::{WrittenScope, WrittenSide};

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
                ds.thread_kind = s.tls();
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

    /// Makes pattern declaration `d`'s written outermost `&`/`&&`
    /// ([`crate::Sema::ref_locs`]) the next reference rebuilt, and its
    /// innermost `*`/`[` ([`crate::Sema::inner_chunk_locs`]) the next
    /// pointer or array; returns the ones they replace.
    fn swap_ref_loc(&mut self, d: DeclId) -> (Option<Loc>, Option<Loc>) {
        let l = self.sema.ref_locs.get(&d).copied();
        let c = self.sema.inner_chunk_locs.get(&d).copied();
        (std::mem::replace(&mut self.ref_loc, l), std::mem::replace(&mut self.chunk_loc, c))
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
        let begin = self.decl_begin(d);
        let outer = self.type_begin.replace(begin);
        let outer_ref = self.swap_ref_loc(d);
        let scope = self.decl_written_scope(d, loc);
        let r = self.with_written_scope(scope, |s| s.ty(t, loc));
        self.declarator_name = saved;
        self.type_begin = outer;
        (self.ref_loc, self.chunk_loc) = outer_ref;
        r
    }

    /// Where pattern declaration `d`'s type begins: its first
    /// type-specifier when recorded, else its name.
    fn decl_begin(&self, d: DeclId) -> Loc {
        let s = &self.sema;
        s.type_spec_locs.get(&d).copied().unwrap_or_else(|| s.ast.decls.get(d).loc())
    }

    /// A variable of the pattern (local or static data member): declared
    /// with the substituted type, then initialized (§5.6).
    pub(crate) fn var_decl(&mut self, d: DeclId, ctx: DeclaratorContext) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        let ty = self.named_decl_ty(d, t, loc)?;
        self.var_function_type(ty, loc, ctx)?;
        let ds = self.pattern_declspec(d, ty);
        let mut dr = self.pattern_declarator(d, ctx)?;
        // The initialization's form as written (`T v{...}` is
        // direct-list-initialization, which may choose an explicit
        // constructor; [dcl.init.general]/16).
        if let Some(x) = self.sema.ast.decls.var(d)
            && x.init.is_some()
        {
            dr.init_style = Some(x.spec.init_style());
        }
        let Some(v) = self.sema.act_on_declarator(&ds, ty, &dr) else { return Err(SubstFail) };
        self.copy_begin_loc(d, v, ty);
        self.locals.insert(d, DeclOrPack::Decl(v));
        if ctx == DeclaratorContext::Member {
            // A static data member's attributes, `[[deprecated]]` among
            // them ([dcl.attr.deprecated]/4 warns a use of
            // `C<int>::x`; clang `InstantiateAttrs`).
            self.class_attrs(v, d);
        } else {
            self.var_align_attrs(v, d);
        }
        // A constrained placeholder's diagnostics point at its
        // type-constraint, as in the pattern.
        if let Some(&l) = self.sema.inst.concepts.placeholder_locs.get(&d) {
            self.sema.inst.concepts.placeholder_locs.insert(v, l);
        }
        self.var_init(d, v)?;
        self.sema.diagnose_unused_inst_var(d, v);
        Ok(v)
    }

    /// Pattern decomposition `d` ([dcl.struct.bind], clang
    /// `TemplateDeclInstantiator::VisitDecompositionDecl`): its bindings
    /// anew, then the variable from its declared type, initialized and
    /// decomposed as written.
    pub(crate) fn decomp_decl(&mut self, d: DeclId, ctx: DeclaratorContext) -> SubstResult<DeclId> {
        let (_, v) = self.decomp_declare(d, ctx)?;
        self.var_init(d, v)?;
        self.remap_binding_pack(d);
        self.sema.diagnose_unused_inst_var(d, v);
        Ok(v)
    }

    /// Once instantiated decomposition of pattern `d` is complete, its
    /// structured binding pack (P1061) names the bindings it expanded
    /// into (spec/sema-templates.md §7.1); a pack whose decomposition
    /// failed makes every use fail (already diagnosed). A pack that is
    /// still dependent (a generic lambda's, its levels retained) stays.
    pub(crate) fn remap_binding_pack(&mut self, d: DeclId) {
        let old = self.sema.decomp_binding_list(d);
        let Some(&p) = old.iter().find(|b| self.sema.binding_packs.contains(b)) else { return };
        let Some(&DeclOrPack::Decl(np)) = self.locals.get(&p) else { return };
        if let Some(&l) = self.sema.binding_pack_elems.get(&np) {
            self.locals.insert(p, DeclOrPack::Pack(l));
        } else if !self.sema.is_pack_decl(np) {
            self.locals.remove(&p);
            self.failed_locals.push(p);
        }
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
        // `auto [a, b]{x}` is direct-list-initialized: `e` deduces the
        // type of `x`, not `std::initializer_list` ([dcl.struct.bind]/1,
        // [dcl.type.auto.deduct]/2.1).
        if let Some(x) = self.sema.ast.decls.var(d)
            && x.init.is_some()
        {
            dr.init_style = Some(x.spec.init_style());
        }
        let old = self.sema.decomp_binding_list(d);
        let ids = old
            .iter()
            .map(|&b| {
                let n = self.sema.ast.decls.get(b);
                crate::declspec::BindingId {
                    name: n.name().as_ident().unwrap_or(Symbol::from_index(0)),
                    loc: n.loc(),
                    ellipsis: self.sema.binding_packs.contains(&b).then_some(n.loc()),
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
            && !Self::has_placeholder(ast, t)
            && !matches!(ast.types.view(ast.types.canonical(t).ty()), TypeView::IncompleteArray(_))
    }

    /// Whether `t` is a placeholder under pointer and reference
    /// declarators (`auto`, `auto*`, `const auto&`).
    fn has_placeholder(ast: &kiln_ast::Ast, t: QualType) -> bool {
        let mut cur = t;
        for _ in 0..256 {
            match ast.types.view(cur.ty()) {
                TypeView::Auto { .. } => return true,
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => cur = p,
                _ => return false,
            }
        }
        false
    }

    /// Declares pattern static data member `d` with its initializer
    /// deferred (see [`Self::defers_init`]); not `inline` until then.
    pub(crate) fn deferred_static_member(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        let ty = self.named_decl_ty(d, t, loc)?;
        self.var_function_type(ty, loc, DeclaratorContext::Member)?;
        let mut ds = self.pattern_declspec(d, ty);
        ds.inline = None;
        let dr = self.pattern_declarator(d, DeclaratorContext::Member)?;
        let saved = std::mem::replace(&mut self.sema.inst.deferring_init, true);
        let v = self.sema.act_on_declarator(&ds, ty, &dr);
        self.sema.inst.deferring_init = saved;
        let Some(v) = v else { return Err(SubstFail) };
        self.copy_begin_loc(d, v, ty);
        self.locals.insert(d, DeclOrPack::Decl(v));
        self.class_attrs(v, d);
        if self.sema.kind_of(v) == DeclKind::Var {
            self.sema.inst.deferred_inits.insert(v);
        }
        Ok(v)
    }

    /// [dcl.meaning.general]/5 ([temp.inst]/17): a variable or static data
    /// member whose type substitutes to a function type is not declared
    /// (clang `VisitVarDecl`'s `err_variable_instantiates_to_function`, at
    /// its name).
    fn var_function_type(
        &mut self,
        ty: QualType,
        loc: Loc,
        ctx: DeclaratorContext,
    ) -> SubstResult<()> {
        if !self.sema.ast.types.is_function(ty) {
            return Ok(());
        }
        let m = self
            .sema
            .diag(kiln_diag::DiagId::sema_var_instantiates_to_function, loc)
            .arg(u8::from(ctx == DeclaratorContext::Member))
            .arg(self.sema.ty_arg(ty));
        self.sema.emit(m);
        Err(SubstFail)
    }

    /// Initializes `v` as pattern variable `d` is.
    pub(crate) fn var_init(&mut self, d: DeclId, v: DeclId) -> SubstResult<()> {
        let Some(data) = self.sema.ast.decls.var(d).copied() else { return Ok(()) };
        if self.sema.kind_of(v) != DeclKind::Var {
            return Ok(());
        }
        let Some(init) = data.init else {
            if self.sema.undeduced_var_spec_without_init(v) {
                return Ok(());
            }
            self.sema.act_on_uninit(v);
            return Ok(());
        };
        let style = data.spec.init_style();
        // A lambda in the initializer is numbered in `v`'s context
        // (spec/sema-lambda.md; clang `TemplatedVariable`).
        let saved = self.sema.lambda_mangle.var.replace(v);
        let errors = self.sema.diags.error_count();
        let outer = self.sema.note_var_init_subst(Some(v));
        let i = self.initializer(init, style);
        self.sema.note_var_init_subst(outer);
        self.sema.lambda_mangle.var = saved;
        if let Ok(i) = i {
            match i {
                Some(i) => self.sema.act_on_init(v, i),
                None => self.sema.act_on_uninit(v),
            }
            Ok(())
        } else {
            // clang marks the declaration invalid: no follow-up
            // "must be initialized" diagnostic.
            self.sema.ast.decls.set_flags(v, DeclFlags::INVALID);
            self.sema.report_deferred_immediates(v, false);
            self.init_failures += 1;
            // Only an error of this instantiation: an error of the
            // pattern's initializer leaves its uses `RecoveryExpr`s.
            if self.sema.diags.error_count() > errors {
                self.failed_inits.push(v);
            }
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

    /// A substituted function parameter type: `void` fails
    /// ([temp.deduct.general]/11.10, clang `SubstParmVarDecl`'s
    /// `err_param_with_void_type`), any other type is adjusted
    /// ([dcl.fct]/5).
    pub(crate) fn param_type(&mut self, ty: QualType, loc: Loc) -> SubstResult<QualType> {
        if self.sema.ast.types.is_void(ty) {
            self.sema.error(kiln_diag::DiagId::sema_param_with_void_type, loc);
            return Err(SubstFail);
        }
        Ok(self.sema.adjust_param_type(ty))
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
            // The function-scope depth stays (`ParamData::scope_depth`).
            let depth = pd.map_or(0, |x| x.flags & ParamData::SCOPE_DEPTH_MASK);
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
                            ParamData::PACK | depth,
                        );
                        self.sema.unowned_params.insert(np);
                        self.locals.insert(p, DeclOrPack::Decl(np));
                        out.push(subst_param_info(name, ty, loc, np, None));
                        continue;
                    }
                };
                let mut decls = Vec::with_capacity(tys.len());
                for ty in tys {
                    let ty = self.param_type(ty, loc)?;
                    let ctx = self.sema.ctx;
                    let idx = u16::try_from(out.len()).unwrap_or(u16::MAX);
                    let np =
                        self.sema.builder().prototype_param(ctx, n.name(), loc, ty, idx, depth);
                    self.sema.unowned_params.insert(np);
                    decls.push(np);
                    // The elements share the pack's name (carried by
                    // `np`); it is not a redefinition ([temp.variadic]/5).
                    out.push(subst_param_info(None, ty, loc, np, None));
                }
                let l = self.sema.ast.lists.push(&decls);
                self.locals.insert(p, DeclOrPack::Pack(l));
                continue;
            }
            let scope = self.decl_written_scope(p, n.loc());
            let begin = self.decl_begin(p);
            let outer = self.type_begin.replace(begin);
            let outer_ref = self.swap_ref_loc(p);
            let named = n.name().as_ident().map(|x| (x, n.loc()));
            let outer_name = std::mem::replace(&mut self.declarator_name, named);
            let ty = self.with_written_scope(scope, |s| s.ty(t, n.loc()));
            self.type_begin = outer;
            (self.ref_loc, self.chunk_loc) = outer_ref;
            self.declarator_name = outer_name;
            let declared = ty?;
            let ty = self.param_type(declared, n.loc())?;
            // Created now, so that the rest of the signature (a trailing
            // return type, the exception specification) refers to it
            // ([basic.scope.param]); the function adopts it.
            let ctx = self.sema.ctx;
            let idx = u16::try_from(out.len()).unwrap_or(u16::MAX);
            let eo = pd.map_or(0, |x| x.flags & ParamData::EXPLICIT_OBJECT);
            let np =
                self.sema.builder().prototype_param(ctx, n.name(), n.loc(), ty, idx, eo | depth);
            self.sema.unowned_params.insert(np);
            self.locals.insert(p, DeclOrPack::Decl(np));
            // The type as declared is kept only while still dependent (a
            // member template of a partially instantiated class), for the
            // next substitution's invalid-type check (§5.7.2); clang's
            // parse-time-only uses of it (a range-`for` over an array
            // parameter) do not apply to an instantiation.
            let original_ty = (declared != ty
                && self.sema.is_instantiation_dependent_type(declared))
            .then_some(declared);
            if let Some(o) = original_ty {
                self.sema.ast.decls.set_param_original_ty(np, o);
            }
            let eo_loc = (eo != 0).then_some(n.loc());
            out.push(ParamInfo { original_ty, ..subst_param_info(name, ty, n.loc(), np, eo_loc) });
        }
        Ok(out)
    }

    /// The occurrence rule of pattern declaration `d`'s type at `loc`
    /// (§5.8): the first occurrence from its decl-specifiers when their
    /// start is known (a parameter, a field), so a declarator such as
    /// `int (*p)(T::type)` finds its own; else the last one before the
    /// declarator-id, or where it would be.
    fn decl_written_scope(&self, d: DeclId, loc: Loc) -> WrittenScope {
        match self.sema.ast.decls.cold.begin_loc.get(d) {
            Some(&begin) if begin < loc => {
                WrittenScope { anchor: loc, side: WrittenSide::After, bound: begin }
            }
            _ => WrittenScope { anchor: loc, side: WrittenSide::Before, bound: loc },
        }
    }

    /// The occurrence rule of pattern function `d`'s return type (§5.8),
    /// and whether it is substituted before the parameters: a leading
    /// return type is written before the declarator-id; a conversion
    /// function's type after `operator`; a trailing return type after the
    /// last parameter.
    fn ret_written_scope(&self, d: DeclId, trailing: bool, loc: Loc) -> (WrittenScope, bool) {
        let ast = &self.sema.ast;
        let conversion = ast.decls.get(d).name().kind() == DeclNameKind::Conversion;
        if !trailing && !conversion {
            return (WrittenScope { anchor: loc, side: WrittenSide::Before, bound: loc }, true);
        }
        // After the `->` when it was recorded: the last parameter's own
        // type may name the same entity after its name.
        let last = ast.decls.function(d).and_then(|f| ast.lists.iter::<DeclId>(f.params).last());
        let bound = match self.sema.templ.trailing_arrow.get(&d) {
            Some(&arrow) if trailing => arrow,
            _ => last.map_or(loc, |p| ast.decls.get(p).loc().max(loc)),
        };
        (WrittenScope { anchor: loc, side: WrittenSide::After, bound }, false)
    }

    /// Return type `ret` of pattern function `d` substituted, with `d`'s
    /// name and innermost written `*`/`[` ([`crate::Sema::inner_chunk_locs`])
    /// available to declarator diagnostics, as clang names them.
    fn ret_ty(&mut self, d: DeclId, ret: QualType, loc: Loc) -> SubstResult<QualType> {
        let name = self.sema.ast.decls.get(d).name().as_ident().map(|n| (n, loc));
        let chunk = self.sema.inner_chunk_locs.get(&d).copied();
        let outer_name = std::mem::replace(&mut self.declarator_name, name);
        let outer_chunk = std::mem::replace(&mut self.chunk_loc, chunk);
        let r = self.ty(ret, loc);
        (self.declarator_name, self.chunk_loc) = (outer_name, outer_chunk);
        r
    }

    /// The substituted parameters, return type and exception
    /// specification of pattern function `d` of type `p`. On failure the
    /// pattern's parameters are forgotten.
    fn function_sig(
        &mut self,
        d: DeclId,
        p: &kiln_ast::types::FuncProto,
        friend: Option<Loc>,
        loc: Loc,
    ) -> SubstResult<(Vec<ParamInfo>, QualType, kiln_ast::types::ExcSpec)> {
        let info = p.info();
        let fn_ty = self.sema.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        // A leading return type first, as written (clang
        // `TransformFunctionProtoType`: the first error is the first
        // written); otherwise the parameters first: a trailing return type
        // and the exception specification may name them
        // ([basic.scope.param]), and `this` ([expr.prim.this]/2).
        let (ret_scope, leading) = self.ret_written_scope(d, info.trailing_return(), loc);
        let early_ret =
            leading.then(|| self.with_written_scope(ret_scope, |s| s.ret_ty(d, p.ret(), loc)));
        let params = match early_ret {
            Some(Err(e)) => Err(e),
            _ => self.param_infos(d),
        };
        let method = friend.is_none() && self.sema.kind_of(d) == DeclKind::CXXMethod;
        let lazy_exc = self.sema.lazy_exc_spec(d, p);
        let this = self.sema.act_on_start_this_scope(method, info.this_quals());
        let sig = params.and_then(|params| {
            let ret = match early_ret {
                Some(r) => r?,
                None => self.with_written_scope(ret_scope, |s| s.ret_ty(d, p.ret(), loc))?,
            };
            // [dcl.fct]/11, at the return type as written (clang's
            // `BuildFunctionType` after the parameters).
            let ret_loc = self.sema.type_spec_locs.get(&d).copied().unwrap_or(loc);
            self.check_return_type(ret, ret_loc)?;
            self.check_volatile_return(p.ret(), ret, ret_loc);
            let ptys: Vec<QualType> = params.iter().map(|p| p.ty).collect();
            self.check_volatile_params(fn_ty, &ptys, ret_loc);
            self.check_param_types(&ptys, ret_loc)?;
            // A dependent `noexcept(expr)` waits until needed (WP-112).
            let exc = if lazy_exc {
                kiln_ast::types::ExcSpec::Deferred(d)
            } else {
                self.exc_spec(p, loc)?
            };
            Ok((params, ret, exc))
        });
        self.sema.act_on_end_this_scope(this);
        if sig.is_err() {
            self.forget_pattern_params(d);
        }
        sig
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
        // `F m;` with `typedef R F(P...)` (no function declarator,
        // [dcl.fct]/13): the function type is substituted with its sugar
        // (clang `TransformTypedefType`) and declared again the same way,
        // so the declaration builds the parameters from it.
        let (p, sig) = match self.sema.ast.types.view(t.ty()) {
            TypeView::Function(p) => (p, Some(self.function_sig(d, &p, friend, loc)?)),
            _ => match self.sema.sugared_proto(t) {
                Some(p) => (p, None),
                None => return self.unimplemented("function declared with this type", loc),
            },
        };
        let info = p.info();
        let via_typedef = sig.is_none();
        let (params, ret, exc) = match sig {
            Some(sig) => sig,
            None => (Vec::new(), self.ty(t, loc)?, kiln_ast::types::ExcSpec::None),
        };
        let lazy_exc = matches!(exc, kiln_ast::types::ExcSpec::Deferred(x) if x == d);
        let mut ds = self.pattern_declspec(d, ret);
        // `explicit(expr)`: substituted, so the declaration resolves it
        // when it no longer depends on these levels ([dcl.fct.spec]/3,
        // [temp.inst]/3; clang `InstantiateExplicitSpecifier`).
        if ds.explicit.is_some()
            && let Some(&e) = self.sema.ast.decls.cold.explicit_expr.get(d)
        {
            // Resolved here once it no longer depends on anything: an
            // invalid one drops the member ([temp.inst]/3, clang
            // `InstantiateExplicitSpecifier`), the declaration then sees
            // the converted constant.
            match self.expr(e).map(|e| (e, self.sema.resolve_explicit(e))) {
                Ok((e, Ok(None))) => ds.explicit_expr = Some(e),
                Ok((_, Ok(Some((c, _))))) => ds.explicit_expr = Some(c),
                Ok((_, Err(()))) | Err(_) => {
                    self.forget_pattern_params(d);
                    return Err(SubstFail);
                }
            }
        }
        ds.friend = friend;
        let mut dr = self.pattern_declarator(d, ctx)?;
        if friend.is_some() {
            dr.qualifier = self.sema.friend_qualifiers.get(&d).copied();
        }
        if matches!(
            self.sema.ast.decls.get(d).name().kind(),
            DeclNameKind::Constructor | DeclNameKind::Destructor | DeclNameKind::Conversion
        ) {
            ds.base = TypeSpecKind::None;
            ds.named = None;
        }
        // A trailing return type is declared again as written, `auto`
        // then `-> R` ([dcl.fct]/2), so the instantiation's type keeps the
        // trailing form (`auto (int) -> int`, clang's
        // `TransformFunctionProtoType`; WP-318).
        let trailing = (info.trailing_return() && ds.base != TypeSpecKind::None && !via_typedef)
            .then(|| {
                let arrow = self.sema.templ.trailing_arrow.get(&d).copied().unwrap_or(loc);
                (ret, arrow)
            });
        if trailing.is_some() {
            ds.named = Some(self.sema.plain_auto_type());
        }
        if !via_typedef {
            dr.chunks.push(Chunk::Function {
                params,
                variadic: info.variadic(),
                loc,
                rparen: loc,
                exc,
                quals: info.this_quals(),
                ref_qual: info.ref_qual(),
                trailing,
                ambiguous: false,
            });
        }
        let fs = self.sema.ast.decls.function(d).map(|x| x.spec);
        if let Some(s) = fs {
            // Where the pattern's specifiers were written, when its
            // override control was deferred (`virt_spec_locs`).
            let (ol, fl) = self.sema.virt_spec_locs.get(&d).copied().unwrap_or_default();
            if s.is_override() {
                dr.virt_override = Some(ol.unwrap_or(loc));
            }
            if s.is_final() {
                dr.virt_final = Some(fl.unwrap_or(loc));
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
        let base = match ds.named {
            _ if ds.base == TypeSpecKind::None => QualType::VOID,
            Some(auto) if trailing.is_some() => auto,
            _ => ret,
        };
        // Its `enable_if` conditions, for the redeclaration check: the
        // instantiation's own are copied after it is declared.
        let ei = self.sema.enable_if_list_of(d);
        self.sema.inst_enable_if = (ei != kiln_ast::ids::ListId::EMPTY).then_some(ei);
        let r = match friend {
            Some(fl) => self.sema.act_on_friend(&ds, base, Some(&dr), fl),
            None => self.sema.act_on_declarator(&ds, base, &dr),
        };
        self.sema.inst_enable_if = None;
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
        // The pattern's attributes (`const`, `pure`, `[[noreturn]]`,
        // `[[nodiscard]]`, ...), as clang `InitFunctionInstantiation` →
        // `InstantiateAttrs` (WP-182). A befriended function gets them
        // too (`[[nodiscard]] friend It operator+(It, int)`, a hidden
        // friend found by ADL, [dcl.attr.nodiscard]/2; WP-491); one
        // redeclaring a function keeps that function's own as well, on
        // its earlier declarations.
        self.class_attrs(f, d);
        // The pattern's GNU `noreturn` function type (spec/types.md §5.4;
        // clang's `TransformFunctionProtoType` keeps the `ExtInfo`): the
        // rebuilt declarator carries no attributes, so the bit is set on
        // the instantiation's type here (WP-749).
        if info.noreturn()
            && let Some(ft) = self.sema.ast.decls.get(f).ty()
            && let Some(nt) = self.sema.with_fn_noreturn(ft, true, false)
        {
            self.sema.ast.decls.set_ty(f, nt);
        }
        // [expr.prim.this]/3: `this` in a static member function's
        // *noexcept-specifier* is diagnosed again in each instantiation
        // of the class, at the pattern's `this` (clang
        // `checkThisInStaticMemberFunctionExceptionSpec` from
        // `InitMethodInstantiation`).
        if friend.is_none()
            && self.sema.is_static_method(d)
            && let kiln_ast::types::ExcSpec::NoexceptExpr { expr, .. } =
                self.sema.ast.types.exc_spec(&p)
        {
            self.sema.check_this_in_static_exception_spec(expr);
        }
        if lazy_exc {
            let first = self.sema.ast.first(f);
            if first == f {
                self.sema.uninstantiated_exc.insert(first, d);
            } else {
                // A friend redeclaring a function: its specification is
                // instantiated now and must match the earlier
                // declaration's ([except.spec]/5; clang
                // `CheckEquivalentExceptionSpec` resolves an
                // `EST_Uninstantiated` one first). The pair was queued
                // by `act_on_friend` while the spec was `Deferred`; it
                // is re-aimed at the first declaration, which clang's
                // note names, so an earlier mismatched friend (itself
                // diagnosed) is not the reference.
                self.sema.delayed_exc_equiv.retain(|&(n, _)| n != f);
                self.sema.delayed_exc_equiv.push((f, first));
                self.sema.uninstantiated_exc.insert(f, d);
                self.sema.instantiate_exception_spec(f, loc);
                self.sema.check_delayed_exc_equiv(f);
            }
        }
        if requires.is_some() {
            self.sema.inst.concepts.member_constraints.insert(f, (d, self.args.clone()));
        }
        if friend.is_some() {
            self.friend_definition(f, d);
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
                // A comparison's synthesized body is located at the
                // pattern's `default` keyword.
                let first = self.sema.ast.first(d);
                let at = self.sema.special.cmp.default_loc.get(&first).map_or(loc, |&(_, l)| l);
                self.sema.act_on_defaulted(f, at);
            }
        }
        Ok(r)
    }

    /// A friend defined in the class: its definition is instantiated
    /// from the pattern `d` when used ([temp.inst]/4, clang's
    /// `isThisDeclarationInstantiatedFromAFriendDefinition`).
    fn friend_definition(&mut self, f: DeclId, d: DeclId) {
        if self.sema.ast.decls.function(d).is_none_or(|x| x.body.is_none()) {
            return;
        }
        if let Some(prev) = self.friend_prior_definition(f) {
            // [temp.inst]/4, [basic.def.odr]/1: the function was
            // already defined, or will be by another friend
            // definition already instantiated (clang
            // `TemplateDeclInstantiator::VisitFunctionDecl`).
            let msg = self
                .sema
                .diag(kiln_diag::DiagId::sema_redefinition, self.sema.ast.decls.get(f).loc())
                .arg(self.sema.decl_name(f));
            let msg = self.sema.note_prev(msg, prev, true);
            self.sema.emit(msg);
        } else if self.sema.ast.instantiated_from(f).is_none() {
            self.member_link(f, d);
        }
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
            let ret = self.ret_ty(pat, p.ret(), loc)?;
            self.check_return_type(ret, loc)?;
            let ret_loc = self.sema.type_spec_locs.get(&pat).copied().unwrap_or(loc);
            self.check_volatile_return(p.ret(), ret, ret_loc);
            let ptys: Vec<QualType> = params.iter().map(|p| p.ty).collect();
            self.check_volatile_params(t, &ptys, ret_loc);
            self.check_param_types(&ptys, loc)?;
            // DR1330 ([temp.inst]/14): a dependent `noexcept(expr)` is
            // not part of deduction; it is substituted when needed (clang
            // `InitFunctionInstantiation` -> `EST_Uninstantiated`), from
            // the specialization's own context. A local-scope pattern's is
            // substituted now, as clang.
            let exc = if matches!(
                self.sema.ast.types.exc_spec(&p),
                kiln_ast::types::ExcSpec::NoexceptExpr {
                    value: kiln_ast::types::NoexceptValue::Dependent(_),
                    ..
                }
            ) && !self.sema.decl_in_local_scope(pat)
            {
                self.sema.uninstantiated_exc.insert(spec, pat);
                kiln_ast::types::ExcSpec::Deferred(spec)
            } else {
                self.exc_spec(&p, loc)?
            };
            Ok((params, ret, exc))
        });
        self.sema.act_on_end_this_scope(this);
        // A pattern whose specification waits (a member template's
        // redeclared pattern, or one awaiting its complete-class context):
        // so does the specialization's ([`Sema::defer_spec_exception_spec`]).
        let sig = sig.map(|(params, ret, exc)| match exc {
            kiln_ast::types::ExcSpec::Deferred(_)
                if self.sema.defer_spec_exception_spec(pat, spec) =>
            {
                (params, ret, kiln_ast::types::ExcSpec::Deferred(spec))
            }
            _ => (params, ret, exc),
        });
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
        if self.sema.kind_of(spec) == DeclKind::CXXConversion {
            // Its printed name (`operator int<int>`); the declaration
            // keeps the pattern's name for the mangling.
            let target = self.sema.ast.types.canonical(ret);
            let _ = self.sema.ast.decls.cold.conversion_spec_target.insert(spec, target);
        }
        let _ = self.sema.builder().set_params(spec, &ids);
        self.forget_pattern_params(pat);
        self.defer_default_args(pat, spec);
        // The pattern's attributes (clang `InitFunctionInstantiation` →
        // `InstantiateAttrs`; WP-182), merged with its other
        // declarations' unused-result attributes (WP-491).
        self.function_spec_attrs(spec, pat);
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
        match self.sema.resolve_explicit(e) {
            Ok(Some((_, true))) => {}
            Ok(Some((_, false))) => {
                let s = self.sema.fn_spec(spec).with_explicit(false);
                self.sema.ast.decls.respecify_function(spec, s);
            }
            Ok(None) => {
                let _ = self.sema.ast.decls.cold.explicit_expr.insert(spec, e);
            }
            Err(()) => return Err(SubstFail),
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
            // The type-id-list of a dynamic exception specification is
            // substituted.
            ExcSpec::Dynamic(l) => {
                let tys = self.sema.ast.types.type_list_types(l).to_vec();
                let tys = self.expand_types(&tys, loc)?;
                // `throw(Ts...)` with an empty pack is `throw()`, non-throwing
                // ([except.spec]/12 as of C++14).
                if tys.is_empty() {
                    ExcSpec::DynamicNone
                } else {
                    ExcSpec::Dynamic(self.sema.ast.types.type_list(&tys))
                }
            }
            // The canonical form of a value-dependent `noexcept(expr)`:
            // its operand is the profile's first expression.
            ExcSpec::DependentNoexcept(pr) => {
                let e = self.sema.ast.types.profile_expr(pr).ok_or(SubstFail)?;
                let e = self.expr(e)?;
                self.sema.act_on_noexcept_spec(e, loc, loc)
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
    /// `f`'s parameter list is set first, so its recorded minimum argument
    /// count is refreshed after (`Ast::refresh_min_args`, WP-663).
    pub(crate) fn defer_default_args(&mut self, pat: DeclId, f: DeclId) {
        self.defer_default_args_to(pat, f);
        self.sema.ast.refresh_min_args(f);
    }

    /// [`Self::defer_default_args`] without the refresh.
    fn defer_default_args_to(&mut self, pat: DeclId, f: DeclId) {
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
        // A pack expanded (to any length, none included): the parameters
        // around it pair up as `map_params` maps them, so an explicit
        // argument before an empty pack does not instantiate its
        // parameter's default ([temp.inst]/12; EnTT's `sort(Compare =
        // Compare{}, Args&&...)` with a lambda `Compare`).
        let pairs = if pp.len() == fp.len() {
            pp.iter().copied().zip(fp.iter().copied()).collect()
        } else {
            let saved: Vec<_> = pp.iter().map(|&a| (a, self.locals.get(&a).copied())).collect();
            let pairs = self.param_pairs(&pp, &fp);
            for (a, old) in saved {
                match old {
                    Some(v) => self.locals.insert(a, v),
                    None => self.locals.remove(&a),
                };
            }
            pairs
        };
        for (a, b) in pairs {
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
            // A deduction guide's placeholder ([`Sema::guide_default_placeholder`]):
            // another placeholder, of the substituted type.
            kiln_ast::expr::ExprKind::OpaqueValueExpr
                if self.sema.ast.exprs.opaque_source(def).is_none() =>
            {
                let ty = self.sema.ast.decls.get(b).ty().unwrap_or(QualType::ERROR);
                let loc = self.sema.ast.exprs.get(def).loc();
                let ph = self.sema.guide_default_placeholder(ty, loc);
                let _ = self.sema.ast.decls.set_default_arg(b, ph);
                return;
            }
            // `= {...}` with a dependent type or element: the list
            // as written, list-initializing the substituted type.
            // (A `void` list was kept as written for a dependent
            // parameter type, even with non-dependent elements.)
            kiln_ast::expr::ExprKind::InitListExpr
                if self.needs_rebuild(def)
                    || self.sema.ast.exprs.get(def).ty() == QualType::VOID =>
            {
                if let Ok(l) = self.init_list(def) {
                    self.sema.act_on_param_default_list(b, &l);
                }
                return;
            }
            _ => self.expr(def),
        };
        if let Ok(e) = e {
            // (A pattern's pack has no default argument to substitute.)
            self.sema.act_on_param_default_arg(b, e, Loc::INVALID);
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
        // Each default argument is substituted under a frame of its own
        // (clang `SubstDefaultArgument`): "in instantiation of default
        // function argument expression for 'f<X>'", and never in the
        // immediate context of a deduction that reached the closure or
        // local class ([temp.deduct.general]/8, P4149R1).
        let frame_args = self.args.levels.last().map(|l| l.args);
        for (a, b) in self.param_pairs(&pp, &fp) {
            let Some(def) = self.sema.ast.decls.param(a).and_then(|x| x.default_arg) else {
                continue;
            };
            let poi = self.sema.ast.exprs.get(def).loc();
            if !self.sema.push_inst_frame(FrameKind::DefaultFunctionArg, f, pat, poi) {
                break;
            }
            if let Some(l) = frame_args.filter(|&l| l != kiln_ast::ids::ArgListId::EMPTY) {
                self.sema.set_frame_args(l);
            }
            self.default_arg_one(a, b);
            self.sema.pop_inst_frame();
        }
        for a in pp {
            self.locals.remove(&a);
        }
    }

    /// Maps pattern parameters `pp` onto `fp` ([`Self::map_params`]) and
    /// returns the non-pack pairs: a parameter after an expanded pack
    /// pairs with its shifted counterpart, not the one at its index.
    fn param_pairs(&mut self, pp: &[DeclId], fp: &[DeclId]) -> Vec<(DeclId, DeclId)> {
        self.map_params(pp, fp);
        pp.iter()
            .filter_map(|&a| match self.locals.get(&a) {
                Some(&DeclOrPack::Decl(b)) => Some((a, b)),
                _ => None,
            })
            .collect()
    }

    /// A field of the pattern; its default member initializer is
    /// substituted after the class is complete (§4.6).
    pub(crate) fn field_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let loc = self.sema.ast.decls.get(d).loc();
        let t = self.decl_ty(d);
        // clang `VisitFieldDecl`: a field whose type fails to substitute
        // is still declared, invalid, so lookup finds it and its uses are
        // silent; the specialization is invalid ([temp.inst]/3, §4.3).
        let ty = self.named_decl_ty(d, t, loc);
        // clang `VisitFieldDecl`: a field whose type substitutes to a
        // function type is declared invalid ([dcl.meaning.general]/5).
        let to_function = ty.is_ok_and(|t| self.sema.ast.types.is_function(t));
        if let (true, Ok(t)) = (to_function, ty) {
            let m = self
                .sema
                .diag(kiln_diag::DiagId::sema_field_instantiates_to_function, loc)
                .arg(self.sema.ty_arg(t));
            self.sema.emit(m);
        }
        let failed = ty.is_err() || to_function;
        let ty = if to_function { QualType::ERROR } else { ty.unwrap_or(QualType::ERROR) };
        let ds = self.pattern_declspec(d, ty);
        let mut dr = self.pattern_declarator(d, DeclaratorContext::Member)?;
        if failed {
            let f = self.sema.act_on_declarator(&ds, ty, &dr).ok_or(SubstFail)?;
            self.sema.ast.decls.set_flags(f, DeclFlags::INVALID);
            return Ok(f);
        }
        let fd: Option<FieldData> = self.sema.ast.decls.field(d).copied();
        if let Some(mut w) = fd.and_then(|x| x.bit_width) {
            // A non-dependent width is already a `ConstantExpr`, which
            // the declarator adds again (clang `TransformConstantExpr`
            // transforms the operand).
            if self.sema.ast.exprs.get(w).kind() == kiln_ast::expr::ExprKind::ConstantExpr {
                w = self.sema.ast.exprs.operand(w);
            }
            dr.bit_width = Some(self.expr(w)?);
        }
        if let Some(x) = fd.filter(|x| x.init.is_some()) {
            dr.init_style = Some(x.init_style());
        }
        let f = self.sema.act_on_declarator(&ds, ty, &dr).ok_or(SubstFail)?;
        // Its attributes (`[[no_unique_address]]`, `alignas`, ...) as for
        // the class (clang `InstantiateAttrs`).
        self.class_attrs(f, d);
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
        // `typedef T x;` names its type from its decl-specifiers (or
        // before `x`), `using x = T;` after `x`.
        let scope = if n.kind() == DeclKind::TypeAlias {
            WrittenScope { anchor: loc, side: WrittenSide::After, bound: loc }
        } else {
            self.decl_written_scope(d, loc)
        };
        let begin = self.decl_begin(d);
        let outer = self.type_begin.replace(begin);
        let outer_ref = self.swap_ref_loc(d);
        let named = n.name().as_ident().map(|x| (x, loc));
        let outer_name = std::mem::replace(&mut self.declarator_name, named);
        let ty = self.with_written_scope(scope, |s| s.ty(u, loc));
        (self.ref_loc, self.chunk_loc) = outer_ref;
        self.type_begin = outer;
        self.declarator_name = outer_name;
        // clang `VisitTypedefNameDecl`: a member typedef whose type fails
        // to substitute is still declared, invalid (lookup finds it, its
        // uses are silent, the specialization is invalid).
        let failed = ty.is_err() && ctx == DeclaratorContext::Member;
        let ty = if failed { QualType::ERROR } else { ty? };
        if n.kind() == DeclKind::TypeAlias {
            let name = n.name().as_ident().ok_or(SubstFail)?;
            let a = self.sema.act_on_alias_decl(name, loc, ty, &[], Loc::INVALID);
            if failed {
                self.sema.ast.decls.set_flags(a, DeclFlags::INVALID);
            }
            self.var_align_attrs(a, d);
            return Ok(a);
        }
        let mut ds = self.pattern_declspec(d, ty);
        ds.storage = Some((StorageSpec::Typedef, loc));
        let dr = self.pattern_declarator(d, ctx)?;
        let td = self.sema.act_on_declarator(&ds, ty, &dr).ok_or(SubstFail)?;
        if failed {
            self.sema.ast.decls.set_flags(td, DeclFlags::INVALID);
        }
        // `typedef enum { } D;` in a template: the instantiated unnamed
        // tag takes the instantiated typedef's name for linkage purposes
        // as the pattern's did ([dcl.typedef]/9; clang
        // `VisitTypedefNameDecl`), so `S<false>::D` mangles as
        // `N1SILb0EE1DE`, not as the unnamed type `Ut_`.
        if !failed {
            let types = &self.sema.ast.types;
            let tag_of = |q: QualType| types.as_record(q).or_else(|| types.as_enum(q));
            if let Some(old) = tag_of(u)
                && self.sema.ast.decls.typedef_for_anon(old) == Some(d)
                && let Some(new) = tag_of(ty)
                && new != old
                && self.sema.ast.decls.get(new).name().is_empty()
                && self.sema.ast.decls.typedef_for_anon(new).is_none()
            {
                let _ = self.sema.ast.decls.set_typedef_for_anon(new, td);
            }
        }
        // A typedef's `__attribute__((aligned(N)))` is instantiated with
        // it (clang `InstantiateAttrs`; libc++'s `atomic_ref` names its
        // object as `typedef _Tp _Aligned_Tp
        // __attribute__((aligned(required_alignment)))`).
        self.var_align_attrs(td, d);
        self.copy_begin_loc(d, td, ty);
        Ok(td)
    }

    /// Keeps pattern `d`'s decl-specifier start on its instantiation `new`
    /// while the type `ty` is still dependent (a member of a member
    /// template is substituted again, §5.8).
    fn copy_begin_loc(&mut self, d: DeclId, new: DeclId, ty: QualType) {
        if let Some(&b) = self.sema.ast.decls.cold.begin_loc.get(d)
            && self.sema.is_instantiation_dependent_type(ty)
        {
            let _ = self.sema.ast.decls.cold.begin_loc.insert(new, b);
        }
    }

    /// A `static_assert` of the pattern: evaluated now ([temp.inst]/3).
    /// With whether it failed ([`Sema::static_assert_checked`]).
    pub(crate) fn static_assert_decl(&mut self, d: DeclId) -> SubstResult<(DeclId, bool)> {
        let loc = self.sema.ast.decls.get(d).loc();
        let Some(sa) = self.sema.ast.decls.static_assert(d).copied() else { return Err(SubstFail) };
        let c = self.expr(sa.cond)?;
        let m = self.opt_expr(sa.message)?;
        Ok(self.sema.static_assert_checked(c, m, loc))
    }

    /// A consteval block of the pattern: substituted and evaluated once
    /// per instantiation (spec/consteval-blocks.md §2). With whether it
    /// failed ([`Sema::consteval_block_checked`]).
    pub(crate) fn consteval_block_decl(&mut self, d: DeclId) -> SubstResult<(DeclId, bool)> {
        let loc = self.sema.ast.decls.get(d).loc();
        let Some(sa) = self.sema.ast.decls.static_assert(d).copied() else { return Err(SubstFail) };
        self.sema.enter_constant_evaluated();
        let c = self.expr(sa.cond);
        self.sema.exit_constant_evaluated(c.as_ref().ok().copied());
        Ok(self.sema.consteval_block_checked(c?, loc))
    }

    /// An enumeration of the pattern: an unscoped one is defined now
    /// (§4.6; clang defines member enumerations with the class), a scoped
    /// member enumeration only when it is required
    /// ([temp.inst]/3: "the declarations, but not the definitions, of
    /// scoped member enumerations"; `Sema::instantiate_member_enum`).
    pub(crate) fn enum_decl(&mut self, d: DeclId) -> SubstResult<DeclId> {
        let n = *self.sema.ast.decls.get(d);
        let loc = n.loc();
        let Some(data) = self.sema.ast.decls.enumeration(d).copied() else { return Err(SubstFail) };
        // clang `VisitEnumDecl`: an underlying type that fails to
        // substitute leaves an invalid enumeration of underlying type
        // `int`, whose enumerators are still declared.
        let mut failed = false;
        let fixed = if data.is_fixed() {
            data.int_type.map(|t| {
                self.ty(t, loc).unwrap_or_else(|_| {
                    failed = true;
                    QualType::INT
                })
            })
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
        let member = n.ctx().is_some_and(|c| self.sema.kind_of(c) == DeclKind::CXXRecord);
        let src = src.filter(|_| scoped == 0 || !member);
        let use_ = if src.is_some() {
            crate::decl::TagUse::Definition
        } else {
            crate::decl::TagUse::Declaration
        };
        let kw = kiln_ast::types::TagKeyword::Enum;
        let e = self.sema.act_on_tag(kw, name, None, use_, scoped, fixed, loc);
        if failed {
            self.sema.ast.decls.set_flags(e, DeclFlags::INVALID);
        }
        // clang `VisitEnumDecl` -> `InstantiateAttrs`, ahead of the
        // definition (a `packed` one's underlying type).
        self.class_attrs(e, d);
        if let Some(src) = src {
            self.enum_definition(e, src)?;
        }
        Ok(e)
    }

    /// Defines enumeration `e` from the pattern's definition `src`: its
    /// enumerators are substituted in order ([temp.inst]/3).
    pub(crate) fn enum_definition(&mut self, e: DeclId, src: DeclId) -> SubstResult<()> {
        self.sema.act_on_start_enum_definition(e);
        let ens: Vec<DeclId> = self.sema.ast.decls.members(src).collect();
        for en in ens {
            let m = *self.sema.ast.decls.get(en);
            if m.kind() != DeclKind::EnumConstant {
                continue;
            }
            let init = self.sema.ast.decls.enumerator(en).and_then(|x| x.init);
            let init = match init {
                Some(i) => match self.expr(i) {
                    Ok(x) => Some(x),
                    Err(f) => {
                        self.sema.act_on_finish_enum_definition(e);
                        return Err(f);
                    }
                },
                None => None,
            };
            let Some(s) = m.name().as_ident() else { continue };
            let x = self.sema.act_on_enumerator(s, m.loc(), init, &[]);
            self.copy_annotations(x, en);
            self.locals.insert(en, DeclOrPack::Decl(x));
            self.member_link(x, en);
            // Found from the enumeration too (`FindInstantiatedDecl`
            // through the instantiated enum, as from a member class).
            self.sema.ast.templates.record_member(e, en, x);
        }
        self.sema.act_on_finish_enum_definition(e);
        Ok(())
    }

    /// Records that `inst` was instantiated from pattern member `pattern`
    /// of the specialization being instantiated.
    pub(crate) fn member_link(&mut self, inst: DeclId, pattern: DeclId) {
        if let Some(spec) = self.entity {
            self.sema.builder().member_of_spec(spec, inst, pattern);
        }
    }

    /// The definition of the function a friend definition instantiated
    /// as `f` redeclares, if there already is one: a declaration with a
    /// body, or another instantiated friend whose pattern has one.
    fn friend_prior_definition(&self, f: DeclId) -> Option<DeclId> {
        if let Some(def) = self.sema.ast.definition(f)
            && def != f
        {
            return Some(def);
        }
        self.sema.ast.redecls_of(f).filter(|&y| y != f).find(|&y| {
            self.sema
                .ast
                .instantiated_from(y)
                .and_then(|p| self.sema.ast.decls.function(p))
                .is_some_and(|x| x.body.is_some())
        })
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
                // `friend struct Z;` that first declared `Z` in the
                // enclosing namespace: the instantiation befriends the
                // same class (not dependent). A tag declared in a local
                // scope would need its instantiated declaration.
                if let Some(t) = owned_tag
                    && !self.sema.ast.decls.get(t).ctx().is_some_and(|c| {
                        matches!(
                            self.sema.kind_of(c),
                            DeclKind::Namespace | DeclKind::TranslationUnit
                        )
                    })
                {
                    return self.unimplemented("friend class declared in a class template", loc);
                }
                // `friend Ts...;` ([class.friend]/3, P2893): one friend
                // per element of the expansion.
                let tys = if crate::templ::pack::type_pattern(self.sema, ty).is_some() {
                    self.expand_types(&[ty], loc)?
                } else {
                    vec![self.ty(ty, loc)?]
                };
                let mut last = None;
                for ty in tys {
                    let mut ds = DeclSpec::new(loc);
                    ds.base = TypeSpecKind::Named;
                    ds.named = Some(ty);
                    ds.type_loc = Some(loc);
                    ds.friend = Some(loc);
                    last = self.sema.act_on_friend(&ds, ty, None, loc).or(last);
                }
                Ok(last)
            }
            FriendTarget::Decl(t) if self.sema.kind_of(t) == DeclKind::ClassTemplate => {
                // `template<class U> friend class X;` names a namespace
                // scope template, which substitution leaves unchanged; a
                // qualified `A<T>::B` names a member template of a class
                // template, found again in the instantiated class.
                let record = self.sema.current_class().map(|c| c.record).ok_or(SubstFail)?;
                let in_class = self
                    .sema
                    .ast
                    .decls
                    .get(t)
                    .ctx()
                    .is_some_and(|c| self.sema.kind_of(c) == DeclKind::CXXRecord);
                let t = if in_class { self.find_instantiated_decl(t, loc)? } else { t };
                self.check_friend_class_head(t);
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

    /// [temp.over.link]/6 for `template<...> friend class X;` in a class
    /// template, deferred from its definition (where its head is one
    /// level deeper than `X`'s and may name the enclosing parameters):
    /// the head substituted with this specialization's arguments must be
    /// equivalent to that of `X`'s latest declaration outside a dependent
    /// context (clang's
    /// `TemplateDeclInstantiator::VisitClassTemplateDecl`). A head whose
    /// substitution fails is not compared.
    fn check_friend_class_head(&mut self, t: DeclId) {
        let latest = self.sema.ast.latest(t);
        let Some(prev) = self.sema.head_comparable_decl(latest) else { return };
        let head = |s: &Self, d: DeclId| s.sema.ast.template_info(d).map(|i| i.params);
        let (Some(new), Some(old)) = (head(self, t), head(self, prev)) else { return };
        if prev == t || new == old {
            return;
        }
        let l = *self.sema.ast.templates.param_list(new);
        if self.args.retained_depth(l.depth) != self.sema.ast.templates.param_list(old).depth {
            return;
        }
        if !self.sema.push_inst_frame(FrameKind::ConstraintSubstitution, t, t, l.template_loc) {
            return;
        }
        if let Some(fr) = self.sema.inst.stack.frames.last_mut() {
            fr.sfinae = Some(true);
        }
        self.sema.inst.sfinae.borrow_mut().push(SfinaeState::default());
        let list = self.friend_head(new).ok();
        let st = self.sema.inst.sfinae.borrow_mut().pop().unwrap_or_default();
        self.sema.pop_inst_frame();
        if let Some(list) = list.filter(|_| st.errors == 0) {
            let _ = self.sema.check_template_heads_match(list, old);
        }
    }

    /// Template parameter list `list` of a friend class template
    /// substituted with its type-constraints and requires-clause
    /// ([`Self::param_list`] keeps those unsubstituted).
    fn friend_head(&mut self, list: TemplateParamListId) -> SubstResult<TemplateParamListId> {
        let sub = self.param_list(list)?;
        let l = *self.sema.ast.templates.param_list(sub);
        let old = self.sema.ast.templates.param_list(list).params;
        let old: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(old).collect();
        let params: Vec<DeclId> = self.sema.ast.lists.iter::<DeclId>(l.params).collect();
        for (&p, &o) in params.iter().zip(&old) {
            // An unconstrained parameter is diagnosed at its first token.
            if let Some(&b) = self.sema.ast.decls.cold.begin_loc.get(o) {
                let _ = self.sema.ast.decls.cold.begin_loc.insert(p, b);
            }
            if let Some(c) = self.sema.ast.template_parm(p).and_then(|i| i.constraint) {
                let c = self.expr(c)?;
                self.sema.builder().set_parm_constraint(p, c);
            }
        }
        let requires = l.requires.map(|r| self.expr(r)).transpose()?;
        Ok(self.sema.builder().template_param_list(
            l.template_loc,
            l.l_angle,
            l.r_angle,
            l.depth,
            &params,
            requires,
        ))
    }
}

/// A parameter of a substituted function type: the declaration `decl`
/// already made, with nothing written but its name and location.
fn subst_param_info(
    name: Option<(Symbol, Loc)>,
    ty: QualType,
    loc: Loc,
    decl: DeclId,
    explicit_object: Option<Loc>,
) -> ParamInfo {
    ParamInfo {
        name,
        ty,
        original_ty: None,
        loc,
        attrs: Vec::new(),
        invalid: false,
        default_arg: DefaultArg::None,
        default_loc: Loc::INVALID,
        range: (loc, loc),
        type_loc: loc,
        ref_loc: Loc::INVALID,
        chunk_loc: Loc::INVALID,
        decl: Some(decl),
        explicit_object,
    }
}
