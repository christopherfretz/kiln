//! Template tables (spec/ast.md §8, spec/sema-templates.md §4.1, §11.1).
//!
//! WP-069 lands the parse-time subset: template parameter lists, the
//! template decls' [`TemplateInfo`], template parameter records, the cold
//! default-argument and trailing-requires maps, requires-expression
//! requirements, and the [`AstBuilder`] constructors for template decls and
//! the dependent expression nodes of sema-templates.md §2.4. Specialization
//! tables (`specs`, `member_specs`, `specializations`, `partials`, ...) are
//! WP-070's.

use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::ast::Ast;
use crate::builder::{AstBuilder, DeclHead};
use crate::decl::{DeclFlags, DeclKind, Members, NewDecl, Payload, ScopeData, TemplLink, TemplLinkKind};
use crate::names::DeclName;
use crate::expr::{BinaryOp, ExprDeps, ExprFlags, ExprKind, ObjectKind, UnaryOp, ValueCategory};
use crate::ids::{ArgListId, DeclId, ExprId, ListId, NnsId, QualType, RawId, TemplateParamListId};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::slot::ColdMap;
use crate::types::{ArgView, TemplateArg, TypeFlags};

/// A template parameter list ([temp.pre]/1): `template < params >
/// requires-clause?`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateParamList {
    /// The `template` keyword.
    pub template_loc: Loc,
    /// `<`.
    pub l_angle: Loc,
    /// `>`.
    pub r_angle: Loc,
    /// The nesting depth of the parameters ([temp.param]); `template<>`
    /// does not count (spec/sema-templates.md §2.1).
    pub depth: u16,
    /// The parameter decls (`TemplateTypeParm`, `NonTypeTemplateParm`,
    /// `TemplateTemplateParm`), as a `ListId` of `DeclId`s.
    pub params: ListId,
    /// The requires-clause after the head ([temp.pre]/9).
    pub requires: Option<ExprId>,
}

/// What a template decl (`ClassTemplate`, `FunctionTemplate`,
/// `VarTemplate`, `TypeAliasTemplate`, `Concept`) templates. Indexed by
/// the template decl's inline payload and its pattern's
/// [`TemplLink::Pattern`] index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateInfo {
    /// The template decl itself.
    pub decl: DeclId,
    /// Its parameters.
    pub params: TemplateParamListId,
    /// The templated declaration (none for a concept).
    pub pattern: Option<DeclId>,
    /// A concept's constraint-expression ([temp.concept]/1).
    pub constraint: Option<ExprId>,
}

/// A template parameter's position and form ([temp.param]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TemplateParmInfo {
    /// The depth (number of enclosing non-empty template parameter lists).
    pub depth: u16,
    /// The position in its list.
    pub index: u16,
    /// A template parameter pack ([temp.variadic]/1).
    pub pack: bool,
    /// Declared with `typename` rather than `class` (type and template
    /// template parameters).
    pub typename_kw: bool,
    /// A type parameter's immediately-declared constraint ([temp.param]/4),
    /// a `ConceptSpecializationExpr`.
    pub constraint: Option<ExprId>,
    /// A template template parameter's own parameters.
    pub params: Option<TemplateParamListId>,
}

/// A requirement of a requires-expression ([expr.prim.req]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requirement {
    /// `expr ;` ([expr.prim.req.simple]).
    Simple(ExprId),
    /// `typename T::type ;` ([expr.prim.req.type]).
    Type(QualType),
    /// `{ expr } noexcept? -> type-constraint? ;`
    /// ([expr.prim.req.compound]); `constraint` is the immediately-declared
    /// constraint of the return-type-requirement.
    Compound {
        /// The expression.
        expr: ExprId,
        /// `noexcept` was written.
        noexcept: bool,
        /// The return-type-requirement's constraint.
        constraint: Option<ExprId>,
    },
    /// `requires constraint-expression ;` ([expr.prim.req.nested]).
    Nested(ExprId),
}

/// A requires-expression's body and requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequiresData {
    /// The `RequiresExprBody` decl holding the parameters.
    pub body: DeclId,
    /// The parameters (`ParmVar` decls).
    pub params: ListId,
    first: u32,
    len: u32,
}

/// Template tables (spec/ast.md §8, M3).
#[derive(Debug)]
pub struct TemplateTables {
    param_lists: Vec<TemplateParamList>,
    infos: Vec<TemplateInfo>,
    parms: Vec<TemplateParmInfo>,
    requirements: Vec<Requirement>,
    requires: Vec<RequiresData>,
    /// Default template arguments, by parameter decl (the cold "template
    /// default args" map, spec/ast.md §8.1).
    pub defaults: ColdMap<DeclId, TemplateArg>,
    /// Trailing requires-clauses of function declarations
    /// ([dcl.decl.general]/2).
    pub trailing_requires: ColdMap<DeclId, ExprId>,
    /// Explicit template arguments of dependent and unresolved names.
    pub expr_targs: ColdMap<ExprId, ArgListId>,
    /// Nested-name-specifiers of dependent and unresolved names.
    pub expr_nns: ColdMap<ExprId, NnsId>,
    /// Set by sema right before it creates a template's pattern: the next
    /// declaration the builder adds is neither a member nor a name (its
    /// template decl is both, [`AstBuilder::template_decl`]).
    pub pattern_next: bool,
    /// The number of enclosing template parameter scopes; while non-zero
    /// new declarations get `DEPENDENT_CTX` ([temp.dep]).
    pub dependent_depth: u32,
}

impl Default for TemplateTables {
    fn default() -> Self {
        let dummy_list = TemplateParamList {
            template_loc: Loc::INVALID,
            l_angle: Loc::INVALID,
            r_angle: Loc::INVALID,
            depth: 0,
            params: ListId::EMPTY,
            requires: None,
        };
        let dummy_parm = TemplateParmInfo {
            depth: 0,
            index: 0,
            pack: false,
            typename_kw: false,
            constraint: None,
            params: None,
        };
        Self {
            param_lists: vec![dummy_list],
            infos: Vec::new(),
            parms: vec![dummy_parm],
            requirements: Vec::new(),
            requires: Vec::new(),
            defaults: ColdMap::new("template_defaults"),
            trailing_requires: ColdMap::new("trailing_requires"),
            expr_targs: ColdMap::new("expr_targs"),
            expr_nns: ColdMap::new("expr_nns"),
            pattern_next: false,
            dependent_depth: 0,
        }
    }
}

impl TemplateTables {
    /// Template parameter list `l`.
    pub fn param_list(&self, l: TemplateParamListId) -> &TemplateParamList {
        &self.param_lists[l.index() as usize]
    }

    /// Template info `i` (a [`TemplLink::Pattern`] index or a template
    /// decl's payload).
    pub fn info(&self, i: u32) -> Option<&TemplateInfo> {
        self.infos.get(i as usize)
    }

    /// The requirements of requires-expression record `r`.
    pub fn requirements(&self, r: &RequiresData) -> &[Requirement] {
        &self.requirements[r.first as usize..(r.first + r.len) as usize]
    }

    /// Requires-expression record `i`.
    pub fn requires_data(&self, i: u32) -> Option<&RequiresData> {
        self.requires.get(i as usize)
    }

    /// `h` with `DEPENDENT_CTX` added while inside a template.
    pub(crate) fn dependent_head(&self, mut h: DeclHead) -> DeclHead {
        if self.dependent_depth > 0 {
            h.flags = h.flags.with(DeclFlags::DEPENDENT_CTX);
        }
        h
    }
}

impl HeapSize for TemplateTables {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("templ.param_lists", &self.param_lists));
        r.push(MemRow::of_vec("templ.infos", &self.infos));
        r.push(MemRow::of_vec("templ.parms", &self.parms));
        r.push(MemRow::of_vec("templ.requirements", &self.requirements));
        r.push(MemRow::of_vec("templ.requires", &self.requires));
        r.push(self.defaults.mem_row());
        r.push(self.trailing_requires.mem_row());
        r.push(self.expr_targs.mem_row());
        r.push(self.expr_nns.mem_row());
        r
    }
}

/// True for the template decl kinds that carry a [`TemplateInfo`].
pub const fn is_template_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::ClassTemplate
            | DeclKind::FunctionTemplate
            | DeclKind::VarTemplate
            | DeclKind::TypeAliasTemplate
            | DeclKind::Concept
    )
}

/// True for the template parameter decl kinds.
pub const fn is_template_parm_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::TemplateTypeParm | DeclKind::NonTypeTemplateParm | DeclKind::TemplateTemplateParm
    )
}

impl Ast {
    /// The template info of template decl `d`.
    pub fn template_info(&self, d: DeclId) -> Option<&TemplateInfo> {
        let n = self.decls.get(d);
        if is_template_kind(n.kind()) { self.templates.info(n.data()) } else { None }
    }

    /// The parameter record of template parameter `d`.
    pub fn template_parm(&self, d: DeclId) -> Option<&TemplateParmInfo> {
        let n = self.decls.get(d);
        if is_template_parm_kind(n.kind()) {
            self.templates.parms.get(n.data() as usize)
        } else {
            None
        }
    }

    /// The template decl whose pattern is `d`.
    pub fn described_template(&self, d: DeclId) -> Option<DeclId> {
        let link = match self.decls.get(d).kind().payload() {
            crate::decl::PayloadClass::Function => self.decls.function(d)?.templ,
            crate::decl::PayloadClass::Var => self.decls.var(d)?.templ,
            crate::decl::PayloadClass::Record => self.decls.record(d)?.templ,
            crate::decl::PayloadClass::Typedef => self.decls.typedef(d)?.templ,
            _ => return None,
        };
        match link.get() {
            Some((TemplLinkKind::Pattern, i)) => self.templates.info(i).map(|t| t.decl),
            _ => None,
        }
    }

    /// The name an `UnresolvedLookupExpr` looks up.
    ///
    /// # Panics
    /// If `e` is not an `UnresolvedLookupExpr`.
    pub fn unresolved_name(&self, e: ExprId) -> Symbol {
        let n = self.exprs.get(e);
        assert_eq!(n.kind(), ExprKind::UnresolvedLookupExpr, "not an UnresolvedLookupExpr");
        Symbol::from_index(n.a())
    }

    /// The declarations an `UnresolvedLookupExpr` found.
    ///
    /// # Panics
    /// If `e` is not an `UnresolvedLookupExpr`.
    pub fn unresolved_decls(&self, e: ExprId) -> Vec<DeclId> {
        let n = self.exprs.get(e);
        assert_eq!(n.kind(), ExprKind::UnresolvedLookupExpr, "not an UnresolvedLookupExpr");
        self.lists.iter::<DeclId>(ListId::new(n.b())).collect()
    }

    /// The requires-expression record of `RequiresExpr` `e`.
    pub fn requires_of(&self, e: ExprId) -> Option<&RequiresData> {
        let n = self.exprs.get(e);
        if n.kind() == ExprKind::RequiresExpr { self.templates.requires_data(n.b()) } else { None }
    }
}

/// The expression dependence of a template argument.
fn arg_deps(b: &AstBuilder<'_>, a: TemplateArg) -> ExprDeps {
    match a.view() {
        ArgView::Expr { expr, .. } => b.deps(expr),
        _ => {
            let f = b.ast.types.arg_flags(a);
            let mut d = ExprDeps::NONE;
            if f.contains(TypeFlags::DEPENDENT) {
                d |= ExprDeps::VALUE_INST;
            }
            if f.contains(TypeFlags::INSTANTIATION_DEPENDENT) {
                d |= ExprDeps::INSTANTIATION;
            }
            if f.contains(TypeFlags::UNEXPANDED_PACK) {
                d |= ExprDeps::UNEXPANDED_PACK;
            }
            d
        }
    }
}

const DEP_ALL: ExprDeps = ExprDeps::TYPE_VALUE_INST;

impl AstBuilder<'_> {
    // -------------------------------------------------------------------
    // template parameters and decls
    // -------------------------------------------------------------------

    /// A template parameter list.
    pub fn template_param_list(
        &mut self,
        template_loc: Loc,
        l_angle: Loc,
        r_angle: Loc,
        depth: u16,
        params: &[DeclId],
        requires: Option<ExprId>,
    ) -> TemplateParamListId {
        let params = self.ast.lists.push(params);
        let t = &mut self.ast.templates;
        t.param_lists.push(TemplateParamList {
            template_loc,
            l_angle,
            r_angle,
            depth,
            params,
            requires,
        });
        TemplateParamListId::new((t.param_lists.len() - 1) as u32)
    }

    fn parm(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: Option<QualType>,
        info: TemplateParmInfo,
    ) -> DeclId {
        let i = self.ast.templates.parms.len() as u32;
        self.ast.templates.parms.push(info);
        let h = self.ast.templates.dependent_head(h);
        self.ast.decls.push(
            NewDecl {
                kind,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty,
            },
            Payload::None(i),
        )
    }

    /// A type template parameter ([temp.param]/2); its declared type is the
    /// `TemplateTypeParm` type. Not a member of any list or lookup table:
    /// sema scopes it.
    pub fn template_type_parm(
        &mut self,
        h: DeclHead,
        depth: u16,
        index: u16,
        pack: bool,
        typename_kw: bool,
    ) -> DeclId {
        let d = self.ast.decls.next_id();
        let ty = self.ast.types.template_type_parm(d, depth.into(), index.into(), pack);
        let info =
            TemplateParmInfo { depth, index, pack, typename_kw, constraint: None, params: None };
        self.parm(h, DeclKind::TemplateTypeParm, Some(QualType::unqual(ty)), info)
    }

    /// A non-type template parameter of type `ty` ([temp.param]/1).
    pub fn non_type_template_parm(
        &mut self,
        h: DeclHead,
        ty: QualType,
        depth: u16,
        index: u16,
        pack: bool,
    ) -> DeclId {
        let info = TemplateParmInfo {
            depth,
            index,
            pack,
            typename_kw: false,
            constraint: None,
            params: None,
        };
        self.parm(h, DeclKind::NonTypeTemplateParm, Some(ty), info)
    }

    /// A template template parameter with parameters `params`.
    pub fn template_template_parm(
        &mut self,
        h: DeclHead,
        params: TemplateParamListId,
        depth: u16,
        index: u16,
        pack: bool,
        typename_kw: bool,
    ) -> DeclId {
        let info = TemplateParmInfo {
            depth,
            index,
            pack,
            typename_kw,
            constraint: None,
            params: Some(params),
        };
        self.parm(h, DeclKind::TemplateTemplateParm, None, info)
    }

    /// Sets type parameter `d`'s immediately-declared constraint.
    pub fn set_parm_constraint(&mut self, d: DeclId, c: ExprId) {
        let i = self.ast.decls.get(d).data() as usize;
        if let Some(p) = self.ast.templates.parms.get_mut(i) {
            p.constraint = Some(c);
        }
    }

    /// Sets template parameter `d`'s default argument.
    ///
    /// # Errors
    /// If `d` already has one (sema diagnoses "template parameter
    /// redefines default argument" first).
    pub fn set_template_default(
        &mut self,
        d: DeclId,
        a: TemplateArg,
    ) -> Result<(), crate::slot::AlreadySet> {
        self.ast.templates.defaults.insert(d, a)
    }

    /// Adds a template pattern: pushed like [`AstBuilder::add`] but neither
    /// linked into a member list nor entered into lookup.
    pub(crate) fn add_pattern(
        &mut self,
        h: DeclHead,
        kind: DeclKind,
        ty: Option<QualType>,
        payload: Payload,
    ) -> DeclId {
        let h = self.ast.templates.dependent_head(h);
        let d = self.ast.decls.push(
            NewDecl {
                kind,
                flags: h.flags,
                misc: h.misc,
                name: h.name,
                ctx: Some(h.ctx),
                loc: h.loc,
                ty,
            },
            payload,
        );
        if let Some(lex) = h.lexical.filter(|&l| l != h.ctx) {
            self.ast
                .decls
                .set_lexical_ctx(d, lex)
                .expect("fresh declaration has no lexical context");
        }
        d
    }

    /// The template decl of `kind` for `pattern` (created through
    /// [`TemplateTables::pattern_next`]): same name, contexts, location and
    /// access; it joins the member list and lookup in the pattern's place.
    /// `prev` is the template decl it redeclares.
    pub fn template_decl(
        &mut self,
        kind: DeclKind,
        pattern: DeclId,
        params: TemplateParamListId,
        prev: Option<DeclId>,
    ) -> DeclId {
        let n = *self.ast.decls.get(pattern);
        let ctx = n.ctx().unwrap_or(self.ast.tu);
        let mut h = DeclHead::new(ctx, n.name(), n.loc());
        h.lexical = self.ast.decls.lexical_ctx(pattern);
        h.misc = n.misc();
        h.flags = n.flags().without(DeclFlags::HAS_LEXICAL_CTX.with(DeclFlags::DEPENDENT_CTX));
        let i = self.ast.templates.infos.len() as u32;
        let d = self.ast.decls.next_id();
        self.ast.templates.infos.push(TemplateInfo {
            decl: d,
            params,
            pattern: Some(pattern),
            constraint: None,
        });
        // The template decl sits in the scope enclosing its own head: it
        // is dependent only inside an outer template.
        let depth = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = depth.saturating_sub(1);
        let d2 = self.add(h, kind, None, Payload::None(i), prev);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        let _ = self.ast.decls.set_templ(pattern, TemplLink::new(TemplLinkKind::Pattern, i));
        d
    }

    /// A concept ([temp.concept]): `template<params> concept C = e;`.
    pub fn concept(&mut self, h: DeclHead, params: TemplateParamListId, e: ExprId) -> DeclId {
        let i = self.ast.templates.infos.len() as u32;
        let d = self.ast.decls.next_id();
        self.ast.templates.infos.push(TemplateInfo {
            decl: d,
            params,
            pattern: None,
            constraint: Some(e),
        });
        let depth = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = depth.saturating_sub(1);
        let d2 = self.add(h, DeclKind::Concept, None, Payload::None(i), None);
        self.ast.templates.dependent_depth = depth;
        debug_assert_eq!(d, d2);
        d2
    }

    // -------------------------------------------------------------------
    // dependent expressions (spec/sema-templates.md §2.4)
    // -------------------------------------------------------------------

    fn dependent_ty(&self) -> QualType {
        QualType::DEPENDENT
    }

    fn side(&mut self, e: ExprId, nns: Option<NnsId>, targs: Option<ArgListId>) {
        if let Some(n) = nns {
            let _ = self.ast.templates.expr_nns.insert(e, n);
        }
        if let Some(t) = targs {
            let _ = self.ast.templates.expr_targs.insert(e, t);
        }
    }

    fn targs_deps(&self, targs: Option<ArgListId>) -> ExprDeps {
        let mut d = ExprDeps::NONE;
        if let Some(l) = targs {
            for &a in self.ast.types.args(l) {
                d |= arg_deps(self, a);
            }
        }
        d
    }

    /// The `RequiresExprBody` of a requires-expression in context `ctx`
    /// ([expr.prim.req]/5): the parameters' declaration context.
    pub fn requires_body(&mut self, ctx: DeclId, loc: Loc) -> DeclId {
        let payload = Payload::Scope(ScopeData { members: Members::default(), aux: 0 });
        // Not a member of `ctx` (clang never lists it among its decls).
        let h = DeclHead::new(ctx, DeclName::EMPTY, loc);
        self.add_unregistered(h, DeclKind::RequiresExprBody, payload, false)
    }

    /// A template argument for expression `e`: dependent expressions get
    /// a stub profile carrying their dependence (WP-070 replaces it with
    /// the canonical profile, spec/types.md §7.2).
    pub fn template_arg_expr(&mut self, e: ExprId) -> TemplateArg {
        let deps = self.deps(e);
        if deps.is_none() {
            return TemplateArg::expr(e, None);
        }
        let mut f = TypeFlags::NONE;
        if deps.intersects(ExprDeps::TYPE.with(ExprDeps::VALUE)) {
            f |= TypeFlags::DEPENDENT_INSTANTIATION;
        }
        if deps.contains(ExprDeps::INSTANTIATION) {
            f |= TypeFlags::INSTANTIATION_DEPENDENT;
        }
        if deps.contains(ExprDeps::UNEXPANDED_PACK) {
            f |= TypeFlags::UNEXPANDED_PACK;
        }
        let p = self.ast.types.profile(&[e.raw()], e, f);
        TemplateArg::expr(e, Some(p))
    }

    /// `T::name` / `X<T>::name<args>` in an expression
    /// ([temp.dep.expr]/3): `DependentScopeDeclRefExpr`, a type-dependent
    /// lvalue.
    pub fn dependent_scope_decl_ref(
        &mut self,
        nns: NnsId,
        name: Symbol,
        targs: Option<ArgListId>,
        loc: Loc,
    ) -> ExprId {
        let ty = self.dependent_ty();
        let deps = DEP_ALL | self.targs_deps(targs);
        let e = self.simple(
            ExprKind::DependentScopeDeclRefExpr,
            ValueCategory::LValue,
            deps,
            0,
            ty,
            loc,
            nns.raw(),
            name.index(),
        );
        self.side(e, None, targs);
        e
    }

    /// An unresolved function name `f` (for a dependent call or a
    /// function template-id): `UnresolvedLookupExpr` of overloaded
    /// function type naming `decls`; `adl` when argument-dependent lookup
    /// applies at instantiation ([temp.dep.candidate]).
    pub fn unresolved_lookup(
        &mut self,
        name: Symbol,
        decls: &[DeclId],
        adl: bool,
        nns: Option<NnsId>,
        targs: Option<ArgListId>,
        loc: Loc,
    ) -> ExprId {
        let list = self.ast.lists.push(decls);
        let deps = self.targs_deps(targs);
        let deps = if deps.intersects(ExprDeps::VALUE) { deps | DEP_ALL } else { deps };
        // clang's `OverloadExpr`: a type-dependent name (dependent explicit
        // arguments) has the dependent type.
        let ty = if deps.contains(ExprDeps::TYPE) { self.dependent_ty() } else { QualType::OVERLOAD };
        let e = self.simple(
            ExprKind::UnresolvedLookupExpr,
            ValueCategory::LValue,
            deps,
            u8::from(adl),
            ty,
            loc,
            name.index(),
            list.raw(),
        );
        self.side(e, nns, targs);
        e
    }

    /// `base.name` / `base->name` with a type-dependent object
    /// ([temp.dep.expr]/5): `CXXDependentScopeMemberExpr`.
    #[allow(clippy::too_many_arguments, reason = "the member-access fields")]
    pub fn dependent_member(
        &mut self,
        base: Option<ExprId>,
        arrow: bool,
        name: Symbol,
        template_kw: bool,
        nns: Option<NnsId>,
        targs: Option<ArgListId>,
        loc: Loc,
    ) -> ExprId {
        let ty = self.dependent_ty();
        let deps = DEP_ALL | base.map_or(ExprDeps::NONE, |b| self.deps(b)) | self.targs_deps(targs);
        let aux = u8::from(arrow) | (u8::from(template_kw) << 1);
        let e = self.simple(
            ExprKind::CXXDependentScopeMemberExpr,
            ValueCategory::LValue,
            deps,
            aux,
            ty,
            loc,
            ExprId::raw_opt(base),
            name.index(),
        );
        self.side(e, nns, targs);
        e
    }

    /// `T(args)` / `T{args}` with a dependent `T` ([expr.type.conv]):
    /// `CXXUnresolvedConstructExpr`; `list` for the braced form.
    pub fn unresolved_construct(
        &mut self,
        ty: QualType,
        args: &[ExprId],
        list: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = self.type_deps(ty);
        for &a in args {
            deps |= self.deps(a);
        }
        let vk = self.ref_category(ty);
        let l = self.ast.lists.push(args);
        let e = self.simple(
            ExprKind::CXXUnresolvedConstructExpr,
            vk,
            deps,
            u8::from(list),
            ty,
            loc,
            l.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rparen);
        e
    }

    fn ref_category(&self, ty: QualType) -> ValueCategory {
        use crate::types::TypeView;
        match self.ast.types.view(ty.ty()) {
            TypeView::LValueRef(_) => ValueCategory::LValue,
            TypeView::RValueRef(_) => ValueCategory::XValue,
            _ => ValueCategory::PRValue,
        }
    }

    /// A built-in unary operator on a type-dependent operand
    /// ([temp.dep.expr]/1, resolved at instantiation): `<dependent type>`,
    /// an lvalue for `*` and prefix `++`/`--` (clang's `CreateOverloadedUnaryOp`
    /// with no visible operator functions), else a prvalue.
    pub fn dependent_unary(&mut self, op: UnaryOp, operand: ExprId, loc: Loc) -> ExprId {
        let vk = if matches!(op, UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::Deref) {
            ValueCategory::LValue
        } else {
            ValueCategory::PRValue
        };
        let deps = self.deps(operand) | DEP_ALL;
        self.push_expr(
            ExprKind::UnaryOperator,
            vk,
            ObjectKind::Ordinary,
            ExprFlags::CANNOT_OVERFLOW,
            deps,
            op as u8,
            self.dependent_ty(),
            loc,
            operand.raw(),
            0,
        )
    }

    /// A built-in binary operator with a type-dependent operand
    /// ([temp.dep.expr]/1): a `<dependent type>` prvalue, or for a
    /// compound assignment an lvalue whose computation types are
    /// dependent too.
    pub fn dependent_binary(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId, loc: Loc) -> ExprId {
        let ty = self.dependent_ty();
        if op.is_compound_assignment() {
            return self.compound_assign(op, lhs, rhs, ty, ty, ty, loc);
        }
        let deps = self.deps(lhs) | self.deps(rhs) | DEP_ALL;
        self.simple(
            ExprKind::BinaryOperator,
            ValueCategory::PRValue,
            deps,
            op as u8,
            ty,
            loc,
            lhs.raw(),
            rhs.raw(),
        )
    }

    /// `c ? t : e` with a type-dependent arm ([temp.dep.expr]/1): a
    /// `<dependent type>` prvalue.
    pub fn dependent_conditional(&mut self, cond: ExprId, then: ExprId, els: ExprId, loc: Loc) -> ExprId {
        let deps = self.deps(cond) | self.deps(then) | self.deps(els) | DEP_ALL;
        let side = self.ast.exprs.push_cond_arms(then, els);
        let ty = self.dependent_ty();
        self.simple(ExprKind::ConditionalOperator, ValueCategory::PRValue, deps, 0, ty, loc, cond.raw(), side)
    }

    /// `( exprs )` kept unresolved as a dependent initializer
    /// ([dcl.init.general]/16 is applied at instantiation): `ParenListExpr`.
    pub fn paren_list(&mut self, exprs: &[ExprId], l: Loc, r: Loc) -> ExprId {
        let mut deps = ExprDeps::NONE;
        for &a in exprs {
            deps |= self.deps(a);
        }
        let ty = self.dependent_ty();
        let list = self.ast.lists.push(exprs);
        let e = self.simple(
            ExprKind::ParenListExpr,
            ValueCategory::PRValue,
            deps,
            0,
            ty,
            l,
            list.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, r);
        e
    }

    /// `sizeof...(pack)` ([expr.sizeof]/5): `SizeOfPackExpr`, a
    /// value-dependent prvalue of type `size_t`.
    pub fn size_of_pack(
        &mut self,
        pack: DeclId,
        size_t: QualType,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let e = self.simple(
            ExprKind::SizeOfPackExpr,
            ValueCategory::PRValue,
            ExprDeps::VALUE_INST,
            0,
            size_t,
            loc,
            pack.raw(),
            0,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rparen);
        e
    }

    /// `pattern...` ([temp.variadic]/5): `PackExpansionExpr` of dependent
    /// type with the pattern's category; the expansion has no unexpanded
    /// packs of its own.
    pub fn pack_expansion_expr(&mut self, pattern: ExprId, ellipsis: Loc) -> ExprId {
        let deps = (self.deps(pattern) | DEP_ALL).without(ExprDeps::UNEXPANDED_PACK);
        let vk = self.ast.exprs.get(pattern).value_category();
        let ty = self.dependent_ty();
        self.simple(ExprKind::PackExpansionExpr, vk, deps, 0, ty, ellipsis, pattern.raw(), 0)
    }

    /// A fold-expression ([expr.prim.fold]): `CXXFoldExpr`, a dependent
    /// prvalue. `lhs`/`rhs` are the operands as written (`( ... op e )` has
    /// no `lhs`, `( e op ... )` no `rhs`).
    pub fn fold_expr(
        &mut self,
        lhs: Option<ExprId>,
        op: BinaryOp,
        rhs: Option<ExprId>,
        lparen: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut deps = DEP_ALL;
        for e in [lhs, rhs].into_iter().flatten() {
            deps |= self.deps(e);
        }
        let deps = deps.without(ExprDeps::UNEXPANDED_PACK);
        let ty = self.dependent_ty();
        let e = self.simple(
            ExprKind::CXXFoldExpr,
            ValueCategory::PRValue,
            deps,
            op as u8,
            ty,
            lparen,
            ExprId::raw_opt(lhs),
            ExprId::raw_opt(rhs),
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rparen);
        e
    }

    /// A requires-expression ([expr.prim.req]): a `bool` prvalue,
    /// value-dependent when a requirement is.
    pub fn requires_expr(
        &mut self,
        body: DeclId,
        params: &[DeclId],
        reqs: &[Requirement],
        bool_ty: QualType,
        loc: Loc,
        rbrace: Loc,
    ) -> ExprId {
        let mut dep = false;
        for r in reqs {
            dep |= match *r {
                Requirement::Simple(e) | Requirement::Nested(e) => !self.deps(e).is_none(),
                Requirement::Compound { expr, .. } => !self.deps(expr).is_none(),
                Requirement::Type(t) => !self.type_deps(t).is_none(),
            };
        }
        let t = &mut self.ast.templates;
        let first = t.requirements.len() as u32;
        t.requirements.extend_from_slice(reqs);
        let params = self.ast.lists.push(params);
        let t = &mut self.ast.templates;
        let i = t.requires.len() as u32;
        t.requires.push(RequiresData { body, params, first, len: reqs.len() as u32 });
        let deps = if dep { ExprDeps::VALUE_INST } else { ExprDeps::NONE };
        let e = self.simple(
            ExprKind::RequiresExpr,
            ValueCategory::PRValue,
            deps,
            0,
            bool_ty,
            loc,
            body.raw(),
            i,
        );
        let _ = self.ast.exprs.cold.end_loc.insert(e, rbrace);
        e
    }

    /// `C<args>` naming concept `c` ([temp.names]/8): a
    /// `ConceptSpecializationExpr`, a `bool` prvalue, value-dependent when
    /// an argument is.
    pub fn concept_spec_expr(
        &mut self,
        c: DeclId,
        args: ArgListId,
        bool_ty: QualType,
        loc: Loc,
    ) -> ExprId {
        let d = self.targs_deps(Some(args));
        let deps = if d.is_none() { ExprDeps::NONE } else { ExprDeps::VALUE_INST.with(d) };
        let deps = deps.without(ExprDeps::TYPE);
        self.push_expr(
            ExprKind::ConceptSpecializationExpr,
            ValueCategory::PRValue,
            ObjectKind::Ordinary,
            ExprFlags::NONE,
            deps,
            0,
            bool_ty,
            loc,
            c.raw(),
            args.raw(),
        )
    }
}
