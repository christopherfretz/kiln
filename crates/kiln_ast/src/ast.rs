//! The [`Ast`]: every arena of a translation unit (spec/ast.md §2.2).

use std::sync::Arc;

use kiln_source::symbol::Interner;

use crate::attr::AttrArena;
use crate::consts::ConstStore;
use crate::decl::DeclArena;
use crate::expr::ExprArena;
use crate::ids::DeclId;
use crate::lists::IdLists;
use crate::lookup::LookupTables;
use crate::mem::{HeapSize, MemReport};
use crate::names::NameTable;
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
/// the `AstBuilder` and never removed.
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
}

impl Ast {
    /// The identifier interner shared with the lexer and preprocessor.
    pub fn symbols(&self) -> &Interner {
        &self.symbols
    }

    /// The memory report of every arena (spec/ast.md §2.3).
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
        r
    }
}
