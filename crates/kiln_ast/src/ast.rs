//! The [`Ast`]: every arena of a translation unit (spec/ast.md §2.2).

use std::sync::Arc;

use kiln_source::symbol::Interner;

use crate::attr::{AttrArena, AttrNode};
use crate::class::{ClassCache, ClassVirtuals, CmpCategories, VTableLayout};
use crate::consts::ConstStore;
use crate::decl::{
    DeclArena, DeclFlags, DeclKind, DeclMisc, EnumData, NewDecl, Payload, ScopeData,
};
use crate::expr::ExprArena;
use crate::expr::ExprKind;
use crate::ids::{DeclId, ExprId};
use crate::lists::IdLists;
use crate::lookup::{LookupResult, LookupTables};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::names::{DeclName, NameTable};
use crate::redecl::RedeclTable;
use crate::reflect::ReflectionTable;
use crate::stmt::StmtArena;
use crate::strings::StringPool;
use crate::templates::TemplateTables;
use crate::types::{LayoutCache, TypeContext};

/// The typed AST of one translation unit.
///
/// Created by sema at TU start, owned by `kiln_sema::Sema` while parsing,
/// then moved to codegen. `Send`, not shared during construction
/// (spec/ast.md §13 Q8). Nodes are appended through
/// the [`AstBuilder`](crate::AstBuilder) and never removed.
#[derive(Debug)]
pub struct Ast {
    /// Types (spec/types.md).
    pub types: TypeContext,
    /// Declarations (§4).
    pub decls: DeclArena,
    /// Expressions (§5).
    pub exprs: ExprArena,
    /// Statements (§6).
    pub stmts: StmtArena,
    /// Attributes (§7).
    pub attrs: AttrArena,
    /// Annotations (P3394 `[[=e]]`; spec/annotations.md).
    pub annots: crate::annot::AnnotArena,
    /// Immutable id lists.
    pub lists: IdLists,
    /// String literal bytes.
    pub strings: StringPool,
    /// Interned special declaration names (§3 `names.special`).
    pub names: NameTable,
    /// Lookup tables (§4.3).
    pub lookup: LookupTables,
    /// Redeclaration groups (§4.4).
    pub redecls: RedeclTable,
    /// Template tables (§8, M3).
    pub templates: TemplateTables,
    /// Interned constant values (§9).
    pub consts: ConstStore,
    /// Interned reflections (§10, M6).
    pub reflections: ReflectionTable,
    /// Record layouts (spec/types.md §11).
    pub layouts: LayoutCache,
    /// Subobject graphs and final overriders of polymorphic classes
    /// (spec/sema-class.md §3.5; computed by sema, insert-only).
    pub virtuals: ClassCache<ClassVirtuals>,
    /// Itanium vtable layouts (computed by `kiln_abi::vtable`,
    /// insert-only).
    pub vtables: ClassCache<VTableLayout>,
    /// The validated `<compare>` comparison category classes (spec/sema-class.md
    /// §4.4; recorded by sema, read by the Oven and codegen).
    pub cmp_categories: CmpCategories,
    /// The `TranslationUnitDecl`.
    pub tu: DeclId,
    /// Module units and declaration ownership (spec/modules.md §5).
    pub modules: crate::modules::ModuleTable,
    /// The translation unit's contract evaluation semantic
    /// (`-fcontract-evaluation-semantic=`; spec/contracts.md §6): what a
    /// contract assertion whose semantic slot is 0 does.
    pub contract_semantic: crate::stmt::ContractSemantic,
    /// The `std::contracts::contract_violation` data of each contract
    /// assertion sema built outside a template pattern (spec/contracts.md
    /// §6.3): recorded by sema, read by codegen.
    pub contract_sites: crate::slot::ColdMap<crate::ids::StmtId, crate::stmt::ContractSite>,
    symbols: Arc<Interner>,
    inst: InstantiationMeter,
}

/// Bytes appended to the AST node arenas while instantiating (spec/ast.md
/// §13 Q6, §15): reported as the `instantiation.retained` row.
#[derive(Clone, Copy, Debug, Default)]
struct InstantiationMeter {
    depth: u32,
    start: u64,
    retained: u64,
    count: u64,
}

impl Ast {
    /// The location of the first token of `e` (clang's `getBeginLoc`):
    /// a node whose own location is an operator or a name after its first
    /// operand begins where that operand does.
    pub fn begin_loc(&self, mut e: ExprId) -> kiln_source::Loc {
        loop {
            let n = self.exprs.get(e);
            let first = match n.kind() {
                ExprKind::ImplicitCastExpr
                | ExprKind::ConstantExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::CXXStdInitializerListExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::BinaryOperator
                | ExprKind::CompoundAssignOperator
                | ExprKind::ConditionalOperator
                | ExprKind::BinaryConditionalOperator
                | ExprKind::ArraySubscriptExpr
                | ExprKind::CallExpr
                | ExprKind::CXXMemberCallExpr
                | ExprKind::CXXPseudoDestructorExpr
                | ExprKind::DesignatedInitUpdateExpr
                | ExprKind::ExtVectorElementExpr
                | ExprKind::MemberExpr => true,
                ExprKind::UnaryOperator => self.exprs.unary_op(e).is_postfix(),
                ExprKind::CXXRewrittenBinaryOperator => {
                    e = self.exprs.rewritten_semantic(e);
                    continue;
                }
                ExprKind::PseudoObjectExpr => {
                    e = self.exprs.pseudo_syntactic(e, &self.lists);
                    continue;
                }
                // clang's `CXXOperatorCallExpr::getSourceRangeImpl`: a
                // prefix unary operator begins at its token, every other
                // form at its first operand.
                ExprKind::CXXOperatorCallExpr => {
                    use crate::names::OverloadedOperator as O;
                    let op = self.exprs.operator_call_op(e);
                    let args = self.exprs.call_args(e);
                    let first = self.lists.at::<ExprId>(args, 0);
                    let prefix = self.lists.get(args).len() == 1
                        && !matches!(op, O::Call | O::Subscript | O::Arrow);
                    match first {
                        Some(a) if !prefix => {
                            e = a;
                            continue;
                        }
                        _ => return n.loc(),
                    }
                }
                _ => false,
            };
            let kid = if first { self.exprs.first_child(e, &self.lists) } else { None };
            match kid {
                Some(k) => e = k,
                None => return self.templates.expr_begin.get(e).copied().unwrap_or(n.loc()),
            }
        }
    }

    /// A new AST holding only the `TranslationUnitDecl`.
    pub fn new(symbols: Arc<Interner>, types: TypeContext) -> Ast {
        let mut decls = DeclArena::new();
        let tu = decls.push(
            NewDecl {
                kind: DeclKind::TranslationUnit,
                flags: DeclFlags::empty(),
                misc: DeclMisc::new(),
                name: DeclName::EMPTY,
                ctx: None,
                loc: kiln_source::Loc::INVALID,
                ty: None,
            },
            Payload::Scope(ScopeData { members: crate::decl::Members::default(), aux: 0 }),
        );
        Ast {
            types,
            decls,
            exprs: ExprArena::new(),
            stmts: StmtArena::new(),
            attrs: AttrArena::new(),
            annots: crate::annot::AnnotArena::new(),
            lists: IdLists::new(),
            strings: StringPool::default(),
            names: NameTable::new(),
            lookup: LookupTables::new(),
            redecls: RedeclTable::new(),
            templates: TemplateTables::default(),
            consts: ConstStore::default(),
            reflections: ReflectionTable::default(),
            layouts: LayoutCache::default(),
            virtuals: ClassCache::default(),
            vtables: ClassCache::default(),
            cmp_categories: CmpCategories::default(),
            tu,
            modules: crate::modules::ModuleTable::default(),
            contract_semantic: crate::stmt::ContractSemantic::default(),
            contract_sites: crate::slot::ColdMap::new("contract_sites"),
            symbols,
            inst: InstantiationMeter::default(),
        }
    }

    /// A builder appending to this AST.
    pub fn builder(&mut self) -> crate::AstBuilder<'_> {
        crate::AstBuilder::new(self)
    }

    /// Re-records the minimum argument count of function `f` from its
    /// parameter list ([`crate::decl::DeclArena::min_args`]), after
    /// default arguments were added to parameters already in the list.
    pub fn refresh_min_args(&mut self, f: DeclId) {
        let Some(fd) = self.decls.function(f) else { return };
        let list = fd.params;
        self.decls.refresh_min_args(f, self.lists.iter::<DeclId>(list));
    }

    /// The access path of an `IndirectField`: the `Field`s from its
    /// context down to the named member (a `ListId` of `DeclId`s).
    ///
    /// # Panics
    /// If `d` is not an `IndirectField`.
    pub fn indirect_field_chain(&self, d: DeclId) -> crate::ids::ListId {
        let n = self.decls.get(d);
        assert!(n.kind() == DeclKind::IndirectField, "not an IndirectField: {:?}", n.kind());
        crate::ids::ListId::new(n.data())
    }

    /// The inline payload word of a using-family declaration
    /// (spec/ast.md §4.2): a `UsingShadow`'s target, a `UsingEnum`'s
    /// enumeration, a `NamespaceAlias`'s aliased namespace or alias as
    /// written, a `UsingDirective`'s nominated namespace. `None` for other
    /// kinds.
    pub fn using_target(&self, d: DeclId) -> Option<DeclId> {
        let n = self.decls.get(d);
        match n.kind() {
            DeclKind::UsingShadow
            | DeclKind::ConstructorUsingShadow
            | DeclKind::UsingEnum
            | DeclKind::NamespaceAlias
            | DeclKind::UsingDirective => DeclId::try_new(n.data()),
            _ => None,
        }
    }

    /// The template (a `StringLiteral`) of a `FileScopeAsm` declaration.
    pub fn file_scope_asm_string(&self, d: DeclId) -> Option<ExprId> {
        let n = self.decls.get(d);
        (n.kind() == DeclKind::FileScopeAsm).then(|| ExprId::new(n.data()))
    }

    /// The nested-name-specifier written in a `Using` declaration.
    pub fn using_qualifier(&self, d: DeclId) -> Option<crate::ids::NnsId> {
        let n = self.decls.get(d);
        if matches!(
            n.kind(),
            DeclKind::Using | DeclKind::UnresolvedUsingValue | DeclKind::UnresolvedUsingTypename
        ) {
            crate::ids::NnsId::try_new(n.data())
        } else {
            None
        }
    }

    /// `d` with using-shadows looked through: the declaration a
    /// `UsingShadow` introduces (an `UnresolvedUsingIfExists` for an
    /// unresolved `using_if_exists` target), else `d`.
    pub fn strip_shadow(&self, d: DeclId) -> DeclId {
        if matches!(
            self.decls.get(d).kind(),
            DeclKind::UsingShadow | DeclKind::ConstructorUsingShadow
        ) {
            self.using_target(d).unwrap_or(d)
        } else {
            d
        }
    }

    /// The class (first declaration) whose constructor
    /// constructor-using-shadow `shadow` nominates: that of its
    /// nominated base's shadow `s`, or for `None` that of its target.
    pub fn shadow_base_class(&self, s: Option<DeclId>, shadow: DeclId) -> Option<DeclId> {
        let d = match s {
            Some(s) => s,
            None => self.using_target(shadow)?,
        };
        self.decls.get(d).ctx().map(|c| self.first(c))
    }

    /// clang `ConstructorUsingShadowDecl`'s constructed base class shadow
    /// (`None`: the target's class) and `constructsVirtualBase`: a
    /// shadow whose nominated base is a virtual base, or whose nominated
    /// shadow constructs one, constructs that virtual base directly
    /// ([class.inhctor.init]/2).
    pub fn shadow_constructed(&self, s: DeclId) -> (Option<DeclId>, bool) {
        let mut chain = vec![s];
        let mut cur = s;
        while let Some(n) = self.decls.ctor_shadow_nominated(cur) {
            if chain.len() > 256 {
                break;
            }
            chain.push(n);
            cur = n;
        }
        // Innermost first: the first shadow nominating a virtual base
        // fixes the constructed shadow for every shadow above it.
        let mut out: (Option<DeclId>, bool) = (None, false);
        for &sh in chain.iter().rev() {
            let ns = self.decls.ctor_shadow_nominated(sh);
            if out.1 {
                continue;
            }
            let virt = match (self.decls.get(sh).ctx(), self.shadow_base_class(ns, sh)) {
                (Some(d), Some(n)) => self.definition(d).is_some_and(|def| {
                    self.decls.bases(def, &self.lists).any(|b| {
                        b.is_virtual
                            && self.types.as_record(b.ty).is_some_and(|c| self.first(c) == n)
                    })
                }),
                _ => false,
            };
            out = (ns, virt);
        }
        out
    }

    /// The namespace a `NamespaceAlias` denotes (aliases of aliases
    /// followed), as its first declaration; `d` itself otherwise.
    pub fn resolve_alias(&self, d: DeclId) -> DeclId {
        let mut cur = d;
        for _ in 0..10_000 {
            if self.decls.get(cur).kind() != DeclKind::NamespaceAlias {
                break;
            }
            match self.using_target(cur) {
                Some(t) => cur = t,
                None => return d,
            }
        }
        if self.decls.get(cur).kind() == DeclKind::Namespace { self.first(cur) } else { cur }
    }

    /// The shared identifier interner (an `Arc` clone).
    pub fn symbols_arc(&self) -> Arc<Interner> {
        Arc::clone(&self.symbols)
    }

    /// The expression the `CXXDefaultArgExpr` `e` stands for: its
    /// parameter's default argument without the `ExprWithCleanups` of its
    /// own full-expression (clang `ParmVarDecl::getDefaultArg`). Its
    /// temporaries are destroyed with the full-expression containing the
    /// call ([intro.execution]/5, [class.temporary]/6.9).
    pub fn default_arg_expr(&self, e: ExprId) -> Option<ExprId> {
        let p = self.exprs.default_arg_param(e);
        let d = self.decls.param(p)?.default_arg?;
        Some(match self.exprs.get(d).kind() {
            ExprKind::ExprWithCleanups => self.exprs.operand(d),
            _ => d,
        })
    }

    // -- redeclarations (§4.4) ------------------------------------------

    /// The first declaration of `d`'s entity (`d` itself if not
    /// redeclarable).
    ///
    /// A template decl has no redeclaration group of its own; its
    /// redeclarations are those of its pattern ([temp.decls]/1), so the
    /// first one is the template described by the pattern's first
    /// declaration (clang's `RedeclarableTemplateDecl::getFirstDecl`).
    #[inline]
    pub fn first(&self, d: DeclId) -> DeclId {
        if self.decls.get(d).misc().is_first() {
            debug_assert_eq!(self.first_slow(d), d, "stale first bit");
            return d;
        }
        self.first_slow(d)
    }

    /// [`Ast::first`] through the redeclaration group.
    fn first_slow(&self, d: DeclId) -> DeclId {
        match self.decls.redecl(d) {
            Some(r) => self.redecls.get(r.group).first,
            None => self.template_redecl(d, true),
        }
    }

    /// The function contract assertions of function `f`
    /// ([dcl.contract.func]/1, spec/contracts.md §4): the sequence on
    /// `f` itself, else the one on its first declaration (a later
    /// declaration repeats it or has none, [dcl.contract.func]/4).
    pub fn function_contracts(&self, f: DeclId) -> Option<crate::ids::ListId> {
        let c = &self.decls.cold.contracts;
        c.get(f).or_else(|| c.get(self.contract_first(f))).copied()
    }

    /// The first declaration of function `f`'s entity in the sense of
    /// [dcl.contract.func]/3-4: [`Ast::first`], except that an explicit
    /// specialization's own first declaration is its first written one,
    /// not the declaration that deduction or class instantiation made of
    /// the specialization before it was explicitly specialized (the
    /// template's function contract specifiers have no effect on it,
    /// [temp.expl.spec]/12; spec/contracts.md §4).
    pub fn contract_first(&self, f: DeclId) -> DeclId {
        use crate::templates::SpecKind;
        let first = self.first(f);
        if first == f {
            return f;
        }
        let explicit =
            self.spec_info(first).is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization)
                || self
                    .member_spec_info(first)
                    .is_some_and(|(_, i)| i.kind == SpecKind::ExplicitSpecialization);
        if !explicit {
            return first;
        }
        let mut d = f;
        while let Some(p) = self.prev(d) {
            if p == first {
                return d;
            }
            d = p;
        }
        first
    }

    /// When the function contract assertions of `f` were written on
    /// another declaration (its first; [dcl.contract.func]/4), that
    /// declaration's parameters paired with `f`'s: the predicates name
    /// the former, an evaluation binds them to the latter. Empty when
    /// they are `f`'s own.
    pub fn contract_param_aliases(&self, f: DeclId) -> Vec<(DeclId, DeclId)> {
        let c = &self.decls.cold.contracts;
        let src = self.contract_first(f);
        if c.get(f).is_some() || src == f || c.get(src).is_none() {
            return Vec::new();
        }
        let params = |d: DeclId| -> Vec<DeclId> {
            self.decls
                .function(d)
                .map(|x| self.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        params(src).into_iter().zip(params(f)).collect()
    }

    /// The latest declaration of `d`'s entity (a template's follows its
    /// pattern's, as for [`Ast::first`]).
    pub fn latest(&self, d: DeclId) -> DeclId {
        match self.decls.redecl(d) {
            Some(r) => self.redecls.get(r.group).latest,
            None => self.template_redecl(d, false),
        }
    }

    /// The template described by the first (or latest) declaration of
    /// template `d`'s pattern; `d` itself for other declarations.
    fn template_redecl(&self, d: DeclId, first: bool) -> DeclId {
        let Some(pattern) = self.template_info(d).and_then(|t| t.pattern) else { return d };
        let Some(link) = self.decls.redecl(pattern) else { return d };
        let group = self.redecls.get(link.group);
        let target = if first { group.first } else { group.latest };
        self.described_template(target).unwrap_or(d)
    }

    /// The definition of `d`'s entity, once seen.
    #[inline]
    pub fn definition(&self, d: DeclId) -> Option<DeclId> {
        if self.decls.get(d).misc().is_definition() {
            debug_assert_eq!(self.definition_slow(d), Some(d), "stale definition bit");
            return Some(d);
        }
        self.definition_slow(d)
    }

    /// [`Ast::definition`] through the redeclaration group.
    fn definition_slow(&self, d: DeclId) -> Option<DeclId> {
        match self.decls.redecl(d) {
            Some(r) => self.redecls.get(r.group).definition,
            None => Some(d),
        }
    }

    /// The previous declaration of `d`'s entity.
    pub fn prev(&self, d: DeclId) -> Option<DeclId> {
        self.decls.redecl(d).and_then(|r| r.prev)
    }

    /// All declarations of `d`'s entity, latest first.
    pub fn redecls_of(&self, d: DeclId) -> impl Iterator<Item = DeclId> + '_ {
        std::iter::successors(Some(self.latest(d)), move |&x| self.prev(x))
    }

    /// Records `d` as the definition of its entity (once).
    ///
    /// # Panics
    /// If `d` is not redeclarable.
    pub fn set_definition(&mut self, d: DeclId) -> Result<(), crate::slot::AlreadySet> {
        let r = self.decls.redecl(d).expect("set_definition on a non-redeclarable decl");
        self.redecls.set_definition(r.group, d)?;
        self.decls.mark_definition(d);
        Ok(())
    }

    // -- contexts and lookup (§4.3) ------------------------------------

    /// True for contexts whose names are visible in the enclosing context
    /// (clang's `isTransparentContext`): `LinkageSpec`, `Export` and
    /// unscoped enumerations.
    pub fn is_transparent(&self, ctx: DeclId) -> bool {
        match self.decls.get(ctx).kind() {
            DeclKind::LinkageSpec | DeclKind::Export => true,
            DeclKind::Enum => {
                self.decls.enumeration(ctx).is_some_and(|e| e.flags & EnumData::SCOPED == 0)
            }
            _ => false,
        }
    }

    /// The nearest enclosing context of `ctx` (itself included) that is
    /// not a `LinkageSpec` or `Export` (where its names are declared).
    pub fn redecl_context(&self, mut ctx: DeclId) -> DeclId {
        while matches!(self.decls.get(ctx).kind(), DeclKind::LinkageSpec | DeclKind::Export) {
            ctx = self.decls.get(ctx).ctx().expect("transparent context has a parent");
        }
        ctx
    }

    /// The context whose lookup table holds `ctx`'s names: the first
    /// declaration of a namespace, the definition of a class or
    /// enumeration, else `ctx` itself.
    pub fn primary_context(&self, ctx: DeclId) -> DeclId {
        match self.decls.get(ctx).kind() {
            DeclKind::Namespace => self.first(ctx),
            DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
            | DeclKind::Enum => self.definition(ctx).unwrap_or(ctx),
            _ => ctx,
        }
    }

    /// Records a using-directive in namespace or translation unit `ctx`
    /// nominating namespace `nominated` ([namespace.udir]). The directive
    /// is also recorded in each enclosing namespace of which `ctx` is an
    /// inline member ([namespace.def.general]/7: the members, and so the
    /// using-directives, of an inline namespace are seen from its
    /// enclosing namespace), so [`LookupTables::using_directives`] of a
    /// namespace covers its inline namespace set.
    ///
    /// [`LookupTables::using_directives`]: crate::lookup::LookupTables::using_directives
    pub fn add_using_directive(&mut self, ctx: DeclId, nominated: DeclId) {
        let nominated = self.first(nominated);
        let mut cur = self.primary_context(self.redecl_context(ctx));
        for _ in 0..4096 {
            self.lookup.add_using_directive(cur, nominated);
            let inline =
                self.decls.namespace(cur).is_some_and(crate::decl::NamespaceData::is_inline);
            match self.decls.get(cur).ctx() {
                Some(p) if inline => cur = self.primary_context(self.redecl_context(p)),
                _ => return,
            }
        }
    }

    /// The declarations named `name` in context `ctx` (its own table:
    /// no hiding, using-directives or ADL; those rules are sema's). Each
    /// result is the latest declaration of its entity, in order of first
    /// declaration.
    pub fn lookup_local(&self, ctx: DeclId, name: DeclName) -> LookupResult<'_> {
        self.lookup.lookup_local(self.primary_context(self.redecl_context(ctx)), name)
    }

    /// The lexical members of `ctx` in declaration order.
    pub fn members(&self, ctx: DeclId) -> crate::decl::MemberIter<'_> {
        self.decls.members(ctx)
    }

    // -- classes (spec/sema-class.md §8.1) ------------------------------

    /// Whether `d` is a destroying operator delete
    /// ([basic.stc.dynamic.deallocation]/5, clang
    /// `FunctionDecl::isDestroyingOperatorDelete`): a class member
    /// `operator delete` whose second parameter is
    /// `std::destroying_delete_t`.
    pub fn is_destroying_operator_delete(&self, d: DeclId) -> bool {
        let decl = self.decls.get(d);
        if decl.kind() != DeclKind::CXXMethod
            || decl.name().as_operator() != Some(crate::names::OverloadedOperator::Delete)
        {
            return false;
        }
        let Some(proto) = decl.ty().and_then(|t| self.types.proto(t)) else { return false };
        let Some(&p1) = self.types.params(proto).get(1) else { return false };
        self.types.as_record(p1).is_some_and(|r| self.is_std_named(r, "destroying_delete_t"))
    }

    /// Whether `d` is named `name` and declared in namespace `::std`,
    /// through inline namespaces (clang `isInStdNamespace`).
    pub fn is_std_named(&self, d: DeclId, name: &str) -> bool {
        let named = |x: DeclId, n: &str| {
            self.decls.get(x).name().as_ident().is_some_and(|s| self.symbols().as_str(s) == n)
        };
        if !named(d, name) {
            return false;
        }
        let mut ns = self.decls.get(d).ctx();
        for _ in 0..64 {
            let Some(n) = ns else { return false };
            let Some(data) = self.decls.namespace(n) else { return false };
            if !data.is_inline() {
                break;
            }
            ns = self.decls.get(n).ctx();
        }
        ns.is_some_and(|n| {
            self.decls.get(n).kind() == DeclKind::Namespace
                && named(n, "std")
                && self.decls.get(n).ctx().is_some_and(|c| {
                    let c = self.redecl_context(c);
                    self.decls.get(c).kind() == DeclKind::TranslationUnit
                })
        })
    }

    /// The destructor of class `record`, if one has been declared (a table
    /// read: never declares the implicit destructor, which is sema's job).
    /// Of several prospective destructors ([class.dtor]/4), the one
    /// selected at the end of the class definition: the first not marked
    /// [`crate::decl::FnSpec::not_selected`] (before selection, or when
    /// it failed, the first declared).
    pub fn destructor(&self, record: DeclId) -> Option<DeclId> {
        let ty = self.decls.get(record).ty()?;
        let name = self.names.find_destructor(self.types.canonical(ty).ty())?;
        let found = self.lookup_local(record, name);
        let first = found.iter().next()?;
        Some(
            found
                .iter()
                .find(|&d| !self.decls.function(d).is_some_and(|f| f.spec.not_selected()))
                .unwrap_or(first),
        )
    }

    /// Whether some declaration of function `d` carries a `NoThrowAttr`
    /// (clang `hasAttr<NoThrowAttr>()`, inherited by redeclarations): the
    /// implicit one of a recognized C library function
    /// (spec/sema.md §5.4) or of a lazily declared builtin.
    pub fn has_nothrow_attr(&self, d: DeclId) -> bool {
        self.redecls_of(d).any(|r| {
            self.lists
                .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                .any(|a| self.attrs.get(a).kind == crate::attr::AttrKind::NOTHROW)
        })
    }

    /// WP-803: the `pass_object_size` of parameter `p` (clang
    /// `PassObjectSizeAttr`): its object-size type and whether it is the
    /// dynamic spelling.
    pub fn pass_object_size(&self, p: DeclId) -> Option<(u32, bool)> {
        self.lists
            .iter::<crate::ids::AttrId>(self.attrs.of_decl(p))
            .map(|a| self.attrs.get(a))
            .find(|n| n.kind == crate::attr::AttrKind::PASS_OBJECT_SIZE)
            .map(|n| (n.a, n.b != 0))
    }

    /// WP-803: `(index, type, dynamic)` of each parameter of function `d`
    /// with a `pass_object_size` (each takes a hidden `size_t` argument
    /// after it).
    pub fn pass_object_size_params(&self, d: DeclId) -> Vec<(usize, u32, bool)> {
        let Some(f) = self.decls.function(d) else { return Vec::new() };
        let params = self.lists.iter::<DeclId>(f.params);
        params
            .enumerate()
            .filter_map(|(i, p)| self.pass_object_size(p).map(|(t, y)| (i, t, y)))
            .collect()
    }

    /// WP-803: some declaration of `d` is `overloadable` (clang
    /// `OverloadableAttr`).
    pub fn is_overloadable(&self, d: DeclId) -> bool {
        self.any_redecl_has(d, crate::attr::AttrKind::OVERLOADABLE)
    }

    /// Whether some declaration of `d` carries `attr`.
    fn any_redecl_has(&self, d: DeclId, attr: crate::attr::AttrKind) -> bool {
        self.redecls_of(d).any(|r| {
            self.lists
                .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                .any(|a| self.attrs.get(a).kind == attr)
        })
    }

    /// Whether some declaration of `d`, or of the pattern it was
    /// instantiated from, carries clang `internal_linkage` (an inheritable
    /// attribute: clang `computeLVForDecl` gives the entity internal
    /// linkage).
    pub fn has_internal_linkage_attr(&self, d: DeclId) -> bool {
        self.inherited_attr(d, crate::attr::AttrKind::INTERNAL_LINKAGE).is_some()
    }

    /// The first `kind` attribute on a declaration of `d` (in declaration
    /// order), else on one of the pattern it was instantiated from: an
    /// inheritable attribute (clang `mergeDeclAttributes`).
    pub fn inherited_attr(&self, d: DeclId, kind: crate::attr::AttrKind) -> Option<AttrNode> {
        let mut d = d;
        // Each step leaves one level of instantiation.
        for _ in 0..64 {
            let redecls: Vec<DeclId> = self.redecls_of(d).collect();
            let found = redecls.iter().rev().find_map(|&r| {
                self.lists
                    .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                    .map(|a| *self.attrs.get(a))
                    .find(|n| n.kind == kind)
            });
            if found.is_some() {
                return found;
            }
            match self.instantiated_from(self.first(d)) {
                Some(p) if p != d => d = p,
                _ => return None,
            }
        }
        None
    }

    /// Whether `d` is a weak declaration (clang `Decl::isWeak`): some
    /// declaration of it carries GNU `weak` or `weakref`. Its address may
    /// be null at run time, so a test of it is not a constant expression
    /// ([expr.const]) and is not folded.
    pub fn is_weak_decl(&self, d: DeclId) -> bool {
        self.any_redecl_has(d, crate::attr::AttrKind::WEAK)
            || self.any_redecl_has(d, crate::attr::AttrKind::WEAKREF)
    }

    /// The symbol a GNU `weakref` declaration `d` refers to: the target of
    /// the `alias` attribute that accompanies its `weakref` (clang
    /// `GetWeakRefReference`), or `None` when `d` is no weak reference.
    pub fn weakref_target(&self, d: DeclId) -> Option<crate::ids::StringLitId> {
        use crate::ids::RawId as _;
        if !self.any_redecl_has(d, crate::attr::AttrKind::WEAKREF) {
            return None;
        }
        self.redecls_of(d).find_map(|r| {
            self.lists
                .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                .map(|a| self.attrs.get(a))
                .find(|a| a.kind == crate::attr::AttrKind::ALIAS)
                .and_then(|a| crate::ids::StringLitId::from_raw(a.a))
        })
    }

    /// The GNU `alias("target")` or `ifunc("resolver")` that makes `d` a
    /// definition (clang `Decl::getDefiningAttr`, ignoring the `alias` of
    /// a `weakref`): `(is_ifunc, target)` from the first declaration of
    /// `d` that carries one, or `None`.
    pub fn alias_definition(&self, d: DeclId) -> Option<(bool, crate::ids::StringLitId)> {
        use crate::ids::RawId as _;
        if self.any_redecl_has(d, crate::attr::AttrKind::WEAKREF) {
            return None;
        }
        self.redecls_of(d).find_map(|r| {
            self.lists
                .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                .map(|a| self.attrs.get(a))
                .find(|a| {
                    matches!(a.kind, crate::attr::AttrKind::ALIAS | crate::attr::AttrKind::IFUNC)
                })
                .and_then(|a| {
                    let l = crate::ids::StringLitId::from_raw(a.a)?;
                    Some((a.kind == crate::attr::AttrKind::IFUNC, l))
                })
        })
    }

    /// The section a GNU `section("name")` attribute on any declaration
    /// of `d` places it in (clang `SectionAttr`, inherited by
    /// redeclarations).
    pub fn section_of(&self, d: DeclId) -> Option<crate::ids::StringLitId> {
        use crate::ids::RawId as _;
        self.redecls_of(d).find_map(|r| {
            self.lists
                .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                .map(|a| self.attrs.get(a))
                .find(|a| a.kind == crate::attr::AttrKind::SECTION)
                .and_then(|a| crate::ids::StringLitId::from_raw(a.a))
        })
    }

    /// The label of a GNU `goto *&&l;` (clang `IndirectGotoStmt::
    /// getConstantTarget`: the target with parentheses and implicit casts
    /// peeled is an `AddrLabelExpr`), which behaves as `goto l;`.
    pub fn indirect_goto_constant_target(&self, s: crate::ids::StmtId) -> Option<DeclId> {
        let mut e = self.stmts.indirect_goto_target(s);
        for _ in 0..64 {
            match self.exprs.get(e).kind() {
                ExprKind::AddrLabelExpr => return Some(self.exprs.addr_label(e)),
                ExprKind::ImplicitCastExpr
                | ExprKind::ParenExpr
                | ExprKind::ExprWithCleanups
                | ExprKind::ConstantExpr => {
                    e = self.exprs.first_child(e, &self.lists)?;
                }
                _ => return None,
            }
        }
        None
    }

    /// Whether the function `d` does not return (clang
    /// `FunctionDecl::isNoReturn` minus the Microsoft spelling): some
    /// declaration of it carries `[[noreturn]]` /
    /// `__attribute__((noreturn))` / C11 `_Noreturn` ([dcl.attr.noreturn]/1: the attribute
    /// appertains to the function), or its type is `noreturn`.
    pub fn is_noreturn_fn(&self, d: DeclId) -> bool {
        let attr = |r: DeclId| {
            self.lists
                .iter::<crate::ids::AttrId>(self.attrs.of_decl(r))
                .any(|a| self.attrs.get(a).kind == crate::attr::AttrKind::NORETURN)
        };
        self.redecls_of(d).any(attr)
            || self
                .decls
                .get(d)
                .ty()
                .and_then(|t| self.types.proto(t))
                .is_some_and(|p| p.info().noreturn())
    }

    /// clang `VarDecl::needsDestruction`: the (array element) type is a class
    /// with a non-trivial destructor ([basic.life]/1). Sema declares the
    /// destructor of every variable's class (spec/sema-class.md §4.8), so a
    /// class without a declared destructor is not destroyed.
    pub fn needs_destruction(&self, t: crate::ids::QualType) -> bool {
        let mut t = t;
        for _ in 0..256 {
            match self.types.element(t) {
                Some(e) if self.types.is_array(t) => t = e,
                _ => break,
            }
        }
        let Some(def) = self.types.as_record(t).and_then(|r| self.definition(r)) else {
            return false;
        };
        self.decls
            .record(def)
            .is_some_and(|x| !x.defn.contains(crate::decl::DefinitionBits::TRIVIAL_DTOR))
            && self
                .destructor(def)
                .and_then(|d| self.decls.function(d))
                .is_some_and(|f| !f.spec.is_trivial())
    }

    /// The GCC type class of `q` that `__builtin_classify_type` returns
    /// (clang `EvaluateBuiltinClassifyType` in C++, identical in clang 19
    /// and 22; WP-419, spec/sema.md §5.4): `void` 0, integer and character
    /// types 1, enumerations 3, `bool` 4, pointers, arrays and functions 5
    /// (the decayed type), pointers to data members 7, floating types 8,
    /// `_Complex` 9, classes and pointers to member functions 12, unions
    /// 13, `_BitInt` 18, vectors 19, and -1 (`no_type_class`) for
    /// `std::nullptr_t`, the sizeless SVE types and `std::meta::info`.
    /// `_Atomic(T)` classifies as `T` and a reference as its referee.
    pub fn gcc_type_class(&self, q: crate::ids::QualType) -> i32 {
        use crate::types::{BuiltinType as B, TypeView as V};
        let mut q = q;
        for _ in 0..8 {
            q = self.types.canonical(q);
            match self.types.canonical_view(q) {
                V::Atomic(t) | V::LValueRef(t) | V::RValueRef(t) => q = t,
                V::Builtin(b) => {
                    return match b {
                        B::Void => 0,
                        B::Bool => 4,
                        B::Char
                        | B::SChar
                        | B::UChar
                        | B::WChar
                        | B::Char8
                        | B::Char16
                        | B::Char32
                        | B::Short
                        | B::UShort
                        | B::Int
                        | B::UInt
                        | B::Long
                        | B::ULong
                        | B::LongLong
                        | B::ULongLong
                        | B::Int128
                        | B::UInt128 => 1,
                        B::Float16
                        | B::BFloat16
                        | B::Float
                        | B::Double
                        | B::LongDouble
                        | B::Float128
                        | B::Half
                        | B::Float32
                        | B::Float64
                        | B::StdFloat128
                        | B::Float32x
                        | B::Float64x => 8,
                        _ => -1,
                    };
                }
                V::BitInt { .. } => return 18,
                V::Pointer(_)
                | V::ConstantArray(_)
                | V::IncompleteArray(_)
                | V::VariableArray { .. }
                | V::Function(_) => return 5,
                V::MemberPointer { pointee, .. } => {
                    return if self.types.is_function(pointee) { 12 } else { 7 };
                }
                V::Record(d) => {
                    let union =
                        self.decls.record(d).is_some_and(|r| r.tag == crate::decl::TagKind::Union);
                    return if union { 13 } else { 12 };
                }
                V::Enum(_) => return 3,
                V::Complex(_) => return 9,
                V::Vector { .. } => return 19,
                _ => return -1,
            }
        }
        -1
    }

    /// True if `e` is a `CXXConstructExpr` calling a trivial constructor
    /// with no arguments: default-initialization that performs no
    /// initialization ([dcl.init.general]/7, [class.default.ctor]/3).
    /// Codegen and the Oven leave the object uninitialized.
    pub fn is_trivial_default_construction(&self, e: ExprId) -> bool {
        self.exprs.get(e).kind() == ExprKind::CXXConstructExpr
            && self.lists.is_empty(self.exprs.construct_args(e))
            && self.decls.function(self.exprs.constructor(e)).is_some_and(|f| f.spec.is_trivial())
    }

    /// The constructors of class `record` declared so far (a table read:
    /// never declares implicit constructors), in order of first
    /// declaration; templates included.
    pub fn constructors(&self, record: DeclId) -> LookupResult<'_> {
        let Some(ty) = self.decls.get(record).ty() else { return LookupResult::Empty };
        match self.names.find_constructor(self.types.canonical(ty).ty()) {
            Some(name) => self.lookup_local(record, name),
            None => LookupResult::Empty,
        }
    }

    /// The ctor-initializers of constructor `ctor` in initialization order
    /// (empty until set, or if it has none).
    pub fn ctor_inits(
        &self,
        ctor: DeclId,
    ) -> impl ExactSizeIterator<Item = &crate::decl::CtorInit> + '_ {
        self.decls.ctor_inits(ctor, &self.lists)
    }

    // -- instantiation accounting (§13 Q6) -----------------------------

    /// Starts an instantiation. Nested calls nest; the outermost
    /// `begin`/`end` pair adds the growth of the AST node arenas to the
    /// `instantiation.retained` memory row.
    pub fn begin_instantiation(&mut self) {
        if self.inst.depth == 0 {
            self.inst.start = self.node_live_bytes();
        }
        self.inst.depth += 1;
    }

    /// Ends the instantiation started by the matching
    /// [`Ast::begin_instantiation`].
    ///
    /// # Panics
    /// Without a matching `begin_instantiation`.
    pub fn end_instantiation(&mut self) {
        assert!(self.inst.depth > 0, "end_instantiation without begin_instantiation");
        self.inst.depth -= 1;
        if self.inst.depth == 0 {
            self.inst.retained += self.node_live_bytes().saturating_sub(self.inst.start);
        }
        self.inst.count += 1;
    }

    /// Live bytes of the arenas instantiation appends to (declarations,
    /// expressions, statements, attributes, lists, lookup, redeclaration
    /// groups). Types and constants interned during an instantiation are
    /// shared and not attributed to it.
    pub fn node_live_bytes(&self) -> u64 {
        self.decls.live_bytes()
            + self.exprs.live_bytes()
            + self.stmts.live_bytes()
            + self.attrs.live_bytes()
            + self.annots.live_bytes()
            + self.lists.live_bytes()
            + self.lookup.live_bytes()
            + self.redecls.live_bytes()
    }

    /// The number of instantiations ended so far (every
    /// [`Ast::end_instantiation`]).
    pub fn instantiation_count(&self) -> u64 {
        self.inst.count
    }

    /// The number of declaration, expression and statement nodes (the
    /// instantiation report's `nodes`, sema-templates.md §11.3).
    pub fn node_count(&self) -> u64 {
        (self.decls.len() + self.exprs.len() + self.stmts.len()) as u64
    }

    /// The identifier interner shared with the lexer and preprocessor.
    pub fn symbols(&self) -> &Interner {
        &self.symbols
    }

    /// The memory report of every arena (spec/ast.md §2.3), the
    /// redeclaration index's change journals (`journal.*`), plus the
    /// derived `instantiation.retained` row (count = instantiations, bytes
    /// = node-arena growth inside outermost instantiations; excluded from
    /// [`MemReport::live_bytes`] as it is a subset of the other rows).
    pub fn memory(&self) -> MemReport {
        let mut r = MemReport::new();
        r.extend_prefixed("types", self.types.heap_bytes());
        r.extend_prefixed("decls", self.decls.heap_bytes());
        r.extend_prefixed("exprs", self.exprs.heap_bytes());
        r.extend_prefixed("stmts", self.stmts.heap_bytes());
        r.extend_prefixed("attrs", self.attrs.heap_bytes());
        r.extend_prefixed("annots", self.annots.heap_bytes());
        r.extend_prefixed("lists", self.lists.heap_bytes());
        r.extend_prefixed("strings", self.strings.heap_bytes());
        r.extend_prefixed("names", self.names.heap_bytes());
        r.extend_prefixed("lookup", self.lookup.heap_bytes());
        r.extend_prefixed("redecls", self.redecls.heap_bytes());
        r.extend_prefixed("templates", self.templates.heap_bytes());
        r.extend_prefixed("consts", self.consts.heap_bytes());
        r.extend_prefixed("reflections", self.reflections.heap_bytes());
        r.extend_prefixed("layouts", self.layouts.heap_bytes());
        r.extend_prefixed("virtuals", self.virtuals.heap_bytes());
        r.extend_prefixed("vtables", self.vtables.heap_bytes());
        // The change journals of sema's redeclaration index (WP-658,
        // spec/sema.md §3): bookkeeping, not nodes, so outside the
        // per-instantiation node meter.
        r.push(self.lookup.journal.mem_row("journal.lookup"));
        r.push(self.decls.ty_journal.mem_row("journal.decl_types"));
        r.push(self.redecls.journal.mem_row("journal.redecls"));
        r.push(MemRow::new(
            "instantiation.retained",
            self.inst.count,
            self.inst.retained,
            self.inst.retained,
        ));
        r
    }
}
