//! The definition hash of the link-time one-definition-rule checks
//! (record kind 2 of `.mason.meta`; mortar `spec/odr.md` O-10..O-13;
//! spec/abi.md "Definition hash (`odr_def`)", which is the contract this
//! module implements: a later kiln must reproduce these bytes under the
//! same [`ODR_DEF_SCHEME`]).
//!
//! [`DefHasher::function`] serializes a function definition after
//! semantic analysis (its declarator, then its body) into a byte stream
//! and returns the FNV-1a 128 of the stream ([`crate::odr::fnv1a_128`])
//! with a `partial` flag. The stream is self-delimiting: every node opens
//! with a one-byte tag, every node kind has a fixed layout after it,
//! lists carry a `u32` count, strings a `u32` length, an absent optional
//! is [`T_NONE`]; so no two different trees give the same stream.
//!
//! The walk is an explicit work stack (no recursion on expression or
//! statement depth). The dispatch matches every [`ExprKind`] and
//! [`StmtKind`] with no wildcard arm, so a new node kind does not compile
//! until someone decides how it hashes. A kind whose area is not built
//! yet, or which only exists in dependent code, calls `partial()`: the
//! flag is set and a fixed token per node kind replaces the subtree, so
//! equal source still gives equal digests (mortar's ruling 2).
//!
//! Nothing here runs unless a caller asks: nothing is recorded during
//! parsing or sema, and the hasher has its own mangling state (it never
//! touches the module's [`AnonIds`]).

mod decl;
pub use decl::is_templated_def;
mod expr;
mod refs;
mod stmt;

use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{Ast, DeclId, ExprId, QualType, StmtId};
use kiln_source::SourceManager;
use kiln_source::fx::FxHashMap;

use crate::mangle::AnonIds;

/// The scheme of the definition serialization (mortar O-11; kind 1's
/// `odr.scheme`, separate from the layout serialization's
/// `odr.layout-scheme`). **Bump rule**: every change to what is
/// serialized or how renames it (the golden test of
/// `crates/kiln_codegen/tests/mason_def_hash.rs` guards it).
pub const ODR_DEF_SCHEME: &str = "kiln-odr-1.d1";

// Tags. Statement kinds are `1 + StmtKind`, expression kinds
// `0x21 + ExprKind` (both enums' numbering is stable); the rest follow.
const S_BASE: u8 = 0x01;
const E_BASE: u8 = 0x21;
/// An absent optional operand.
pub(crate) const T_NONE: u8 = 0xA0;
/// A local entity by declaration index (`u32`).
const T_LOCAL: u8 = 0xA1;
/// An external entity: its mangled name's FNV-1a 128 (16 bytes).
const T_ENTITY: u8 = 0xA2;
/// A type: its `<type>` encoding's FNV-1a 128 (16 bytes).
const T_TYPE: u8 = 0xA3;
/// The placeholder of an entity or type with a per-TU component.
const T_TU_PLACEHOLDER: u8 = 0xA4;
/// A builtin-macro or source-location value (then one byte: the kind).
const T_VALUE_PLACEHOLDER: u8 = 0xA5;
/// A skipped construct (then a class byte and a code byte).
const T_PARTIAL: u8 = 0xA6;
/// An enumerator: its enumeration (a type item), name, value.
const T_ENUMERATOR: u8 = 0xA7;
/// A non-static data member: its class (a type item) and name.
const T_FIELD: u8 = 0xA8;
/// A local declaration: index, type, specifier bits, initializer.
const T_LOCAL_DECL: u8 = 0xA9;
/// The declarator of a function definition.
const T_FUNCTION: u8 = 0xAA;

/// The digest of one definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefDigest {
    /// FNV-1a 128 of the serialization.
    pub hash: u128,
    /// The hasher skipped a construct its scheme covers (O-15 `PARTIAL`).
    pub partial: bool,
}

/// The package that will hash a skipped construct (spec/abi.md
/// "Definition hash", the table of node kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Area {
    /// Initialization and object expressions, special members (K-3c).
    Init,
    /// Lambdas, local classes, coroutines (K-3d).
    Lambda,
    /// The remaining kinds: GNU and builtin forms, reflection (K-3d).
    Misc,
    /// Attributes and default arguments (K-3e).
    Attrs,
    /// Dependent-only kinds: they wait for modules (K-6).
    Dependent,
}

/// What a `partial()` skipped: a node kind, a declaration kind met where
/// the hasher has no rule for it, or a named declarator item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    /// An expression kind.
    Expr(ExprKind),
    /// A statement kind.
    Stmt(StmtKind),
    /// A declaration kind (in a statement or as a reference).
    Decl(DeclKind),
    /// A declarator item.
    Item(Item),
}

/// A declarator or declaration item a `partial()` skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Item {
    /// A function template's own definition (only instantiations and
    /// non-templates are hashed; K-6).
    Templated = 1,
    /// An exception specification sema has not computed yet.
    UnresolvedNoexcept,
    /// A value-dependent exception specification.
    DependentNoexcept,
    /// A dependent trailing requires-clause.
    DependentRequires,
    /// An attribute of O-10's closed list (K-3e).
    Attributes,
    /// A default argument on some declaration (K-3e).
    DefaultArgs,
    /// A constructor (its initializers, K-3c).
    Constructor,
    /// A virtual destructor (the deallocation function it selects, K-3c).
    VirtualDestructor,
    /// An implicitly declared or defaulted member (as synthesized, K-3c).
    Synthesized,
    /// A `#pragma` floating-point override on a block.
    FpPragma,
    /// An integer literal wider than its value words say.
    WideLiteral,
}

/// Counters of the hasher (costs and the `partial` share by reason).
#[derive(Clone, Debug, Default)]
pub struct DefStats {
    /// Definitions hashed.
    pub definitions: u64,
    /// Of which `partial`.
    pub partial_definitions: u64,
    /// Expression and statement nodes visited.
    pub nodes: u64,
    /// Bytes fed to the hash.
    pub bytes: u64,
    /// Entity names: memo hits and manglings.
    pub entity_hits: u64,
    /// Entity names mangled (memo misses).
    pub entity_misses: u64,
    /// Types: memo hits.
    pub type_hits: u64,
    /// Types mangled (memo misses).
    pub type_misses: u64,
    /// `partial()` calls per site (at most one per definition and site).
    pub partial_sites: FxHashMap<(Area, Site), u64>,
}

/// One pending item of the walk.
#[derive(Clone, Copy, Debug)]
enum Work {
    Expr(ExprId),
    Stmt(StmtId),
    OptExpr(Option<ExprId>),
    OptStmt(Option<StmtId>),
    /// A declaration inside a statement (a local variable and its
    /// initializer, a structured binding, a local type).
    Decl(DeclId),
}

/// The definition hasher: one per unit, reused for every definition
/// (its memo maps and buffers are what make the cost per unit, not per
/// definition).
#[derive(Debug)]
pub struct DefHasher<'a> {
    ast: &'a Ast,
    sm: &'a SourceManager,
    refs: refs::Refs<'a>,
    out: Vec<u8>,
    stack: Vec<Work>,
    /// Local entities of the current definition, by declaration index.
    locals: FxHashMap<DeclId, u32>,
    /// The current definition's first declaration.
    def: DeclId,
    partial: bool,
    /// The sites already counted for the current definition.
    seen: Vec<(Area, Site)>,
    stats: DefStats,
}

impl<'a> DefHasher<'a> {
    /// A hasher over `ast` (its source manager `sm` tells builtin-macro
    /// tokens). `anon` is the hasher's OWN mangling state: a fresh
    /// table configured as the module's (reflection mangling, clang
    /// compatibility), never the module's, so codegen's numbering cannot
    /// depend on whether the hasher ran. Every name it mangles passes the
    /// per-TU guard first, so it numbers nothing that reaches the stream.
    pub fn new(ast: &'a Ast, sm: &'a SourceManager, anon: AnonIds) -> Self {
        DefHasher {
            ast,
            sm,
            refs: refs::Refs::new(ast, anon),
            out: Vec::with_capacity(4096),
            stack: Vec::with_capacity(256),
            locals: FxHashMap::default(),
            def: DeclId::new(1),
            partial: false,
            seen: Vec::new(),
            stats: DefStats::default(),
        }
    }

    /// Keeps every mangled name the hasher feeds (for tests that scan them).
    pub fn collect_names(&mut self) {
        self.refs.names = Some(Vec::new());
    }

    /// The names collected since [`Self::collect_names`].
    pub fn names(&self) -> &[String] {
        self.refs.names.as_deref().unwrap_or_default()
    }

    /// The counters so far.
    pub fn stats(&self) -> &DefStats {
        &self.stats
    }

    /// The heap the hasher holds (its memo maps and buffers), in bytes.
    pub fn memory(&self) -> usize {
        self.refs.memory()
            + self.out.capacity()
            + self.stack.capacity() * size_of::<Work>()
            + self.locals.capacity() * (size_of::<(DeclId, u32)>() + 1)
            + self.seen.capacity() * size_of::<(Area, Site)>()
    }

    /// The serialization of the last definition hashed (for tests).
    pub fn last_stream(&self) -> &[u8] {
        &self.out
    }

    /// The digest of function definition `def` (a declaration with a
    /// body): its declarator and specifiers, then its body (mortar O-10).
    pub fn function(&mut self, def: DeclId) -> DefDigest {
        self.out.clear();
        self.locals.clear();
        self.seen.clear();
        self.partial = false;
        self.stack.clear();
        self.def = self.ast.first(def);
        if decl::declarator(self, def) {
            let body = self.ast.decls.function(def).and_then(|f| f.body);
            self.stack.push(Work::OptStmt(body));
            self.run();
        }
        self.stats.definitions += 1;
        self.stats.partial_definitions += u64::from(self.partial);
        self.stats.bytes += self.out.len() as u64;
        DefDigest { hash: fnv1a_128_bytes(&self.out), partial: self.partial }
    }

    fn run(&mut self) {
        while let Some(w) = self.stack.pop() {
            match w {
                Work::Expr(e) | Work::OptExpr(Some(e)) => {
                    self.stats.nodes += 1;
                    let mark = self.stack.len();
                    self.expr(e);
                    self.stack[mark..].reverse();
                }
                Work::Stmt(s) | Work::OptStmt(Some(s)) => {
                    self.stats.nodes += 1;
                    let mark = self.stack.len();
                    self.stmt(s);
                    self.stack[mark..].reverse();
                }
                Work::OptExpr(None) | Work::OptStmt(None) => self.byte(T_NONE),
                Work::Decl(d) => {
                    let mark = self.stack.len();
                    self.local_decl(d);
                    self.stack[mark..].reverse();
                }
            }
        }
    }

    fn byte(&mut self, b: u8) {
        self.out.push(b);
    }

    fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    fn str(&mut self, s: &str) {
        self.u32(u32::try_from(s.len()).unwrap_or(u32::MAX));
        self.out.extend_from_slice(s.as_bytes());
    }

    /// A count prefix.
    fn count(&mut self, n: usize) {
        self.u32(u32::try_from(n).unwrap_or(u32::MAX));
    }

    fn stmt_tag(&mut self, k: StmtKind) {
        self.byte(S_BASE + k as u8);
    }

    fn expr_tag(&mut self, k: ExprKind) {
        self.byte(E_BASE + k as u8);
    }

    /// Skips a construct of `area`: sets the flag and writes the fixed
    /// token of `site` in place of the subtree.
    fn partial(&mut self, area: Area, site: Site) {
        self.partial = true;
        let (class, code) = match site {
            Site::Expr(k) => (b'e', k as u8),
            Site::Stmt(k) => (b's', k as u8),
            Site::Decl(k) => (b'd', k as u8),
            Site::Item(i) => (b'i', i as u8),
        };
        self.out.extend_from_slice(&[T_PARTIAL, class, code]);
        if !self.seen.contains(&(area, site)) {
            self.seen.push((area, site));
            *self.stats.partial_sites.entry((area, site)).or_default() += 1;
        }
    }

    /// A type item: the canonical type's `<type>` encoding, or the per-TU
    /// placeholder.
    fn ty(&mut self, q: QualType) {
        match self.refs.ty(q, &mut self.stats) {
            Some(h) => {
                self.byte(T_TYPE);
                self.out.extend_from_slice(&h.to_le_bytes());
            }
            None => self.byte(T_TU_PLACEHOLDER),
        }
    }

    /// The index of local entity `d` of the current definition, assigned
    /// at its first appearance in walk (source) order; `None` when `d` is
    /// not local to it.
    fn local(&mut self, d: DeclId) -> Option<u32> {
        if let Some(&i) = self.locals.get(&d) {
            return Some(i);
        }
        let ast = self.ast;
        let ctx = ast.decls.get(d).ctx()?;
        if ast.first(ctx) != self.def {
            return None;
        }
        let i = u32::try_from(self.locals.len()).unwrap_or(u32::MAX);
        self.locals.insert(d, i);
        Some(i)
    }

    /// Declares local `d` (a parameter in the declarator, a variable in a
    /// statement) and writes its index.
    fn declare(&mut self, d: DeclId) {
        let i = self.local(d).unwrap_or_else(|| {
            // A declaration whose context is not the definition (the
            // body of a local class is K-3d): numbered all the same.
            let i = u32::try_from(self.locals.len()).unwrap_or(u32::MAX);
            self.locals.insert(d, i);
            i
        });
        self.u32(i);
    }
}

/// FNV-1a 128 over bytes: [`crate::odr::fnv1a_128`]'s function (the
/// stream is not text).
fn fnv1a_128_bytes(b: &[u8]) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    b.iter().fold(OFFSET, |h, &x| (h ^ u128::from(x)).wrapping_mul(PRIME))
}

#[cfg(test)]
mod tests {
    #[test]
    fn stream_hash_is_fnv1a_128() {
        for s in ["", "a", "kiln-odr-1.d1", "_ZN2ns1fEv"] {
            assert_eq!(super::fnv1a_128_bytes(s.as_bytes()), crate::odr::fnv1a_128(s));
        }
    }
}
