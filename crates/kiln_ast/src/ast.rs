//! The [`Ast`]: every arena of a translation unit (spec/ast.md §2.2).

use std::sync::Arc;

use kiln_source::symbol::Interner;

use crate::attr::AttrArena;
use crate::consts::ConstStore;
use crate::decl::{
    DeclArena, DeclFlags, DeclKind, DeclMisc, EnumData, NewDecl, Payload, ScopeData,
};
use crate::expr::ExprArena;
use crate::ids::DeclId;
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
    /// The `TranslationUnitDecl`.
    pub tu: DeclId,
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
            lists: IdLists::new(),
            strings: StringPool::default(),
            names: NameTable::new(),
            lookup: LookupTables::new(),
            redecls: RedeclTable::new(),
            templates: TemplateTables::default(),
            consts: ConstStore::default(),
            reflections: ReflectionTable::default(),
            layouts: LayoutCache::default(),
            tu,
            symbols,
            inst: InstantiationMeter::default(),
        }
    }

    /// A builder appending to this AST.
    pub fn builder(&mut self) -> crate::AstBuilder<'_> {
        crate::AstBuilder::new(self)
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
            | DeclKind::UsingEnum
            | DeclKind::NamespaceAlias
            | DeclKind::UsingDirective => DeclId::try_new(n.data()),
            _ => None,
        }
    }

    /// The nested-name-specifier written in a `Using` declaration.
    pub fn using_qualifier(&self, d: DeclId) -> Option<crate::ids::NnsId> {
        let n = self.decls.get(d);
        if n.kind() == DeclKind::Using { crate::ids::NnsId::try_new(n.data()) } else { None }
    }

    /// `d` with using-shadows looked through: the declaration a
    /// `UsingShadow` introduces (an `UnresolvedUsingIfExists` for an
    /// unresolved `using_if_exists` target), else `d`.
    pub fn strip_shadow(&self, d: DeclId) -> DeclId {
        if self.decls.get(d).kind() == DeclKind::UsingShadow {
            self.using_target(d).unwrap_or(d)
        } else {
            d
        }
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

    // -- redeclarations (§4.4) ------------------------------------------

    /// The first declaration of `d`'s entity (`d` itself if not
    /// redeclarable).
    pub fn first(&self, d: DeclId) -> DeclId {
        self.decls.redecl(d).map_or(d, |r| self.redecls.get(r.group).first)
    }

    /// The latest declaration of `d`'s entity.
    pub fn latest(&self, d: DeclId) -> DeclId {
        self.decls.redecl(d).map_or(d, |r| self.redecls.get(r.group).latest)
    }

    /// The definition of `d`'s entity, once seen.
    pub fn definition(&self, d: DeclId) -> Option<DeclId> {
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
        self.redecls.set_definition(r.group, d)
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
    fn node_live_bytes(&self) -> u64 {
        self.decls.live_bytes()
            + self.exprs.live_bytes()
            + self.stmts.live_bytes()
            + self.attrs.live_bytes()
            + self.lists.live_bytes()
            + self.lookup.live_bytes()
            + self.redecls.live_bytes()
    }

    /// The identifier interner shared with the lexer and preprocessor.
    pub fn symbols(&self) -> &Interner {
        &self.symbols
    }

    /// The memory report of every arena (spec/ast.md §2.3), plus the
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
        r.extend_prefixed("lists", self.lists.heap_bytes());
        r.extend_prefixed("strings", self.strings.heap_bytes());
        r.extend_prefixed("names", self.names.heap_bytes());
        r.extend_prefixed("lookup", self.lookup.heap_bytes());
        r.extend_prefixed("redecls", self.redecls.heap_bytes());
        r.extend_prefixed("templates", self.templates.heap_bytes());
        r.extend_prefixed("consts", self.consts.heap_bytes());
        r.extend_prefixed("reflections", self.reflections.heap_bytes());
        r.extend_prefixed("layouts", self.layouts.heap_bytes());
        r.push(MemRow::new(
            "instantiation.retained",
            self.inst.count,
            self.inst.retained,
            self.inst.retained,
        ));
        r
    }
}
