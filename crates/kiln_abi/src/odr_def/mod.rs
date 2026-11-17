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
mod init;
mod lambda;
pub use lambda::lambda_in_declarator;
mod misc;
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
/// `crates/kiln_codegen/tests/mason_def_hash.rs` guards it). History:
/// `d0` the declared type only (no body; K-3a's rows), `d1` the
/// serializer core (declarators, statements, basic expressions; K-3b),
/// `d2` initialization and object expressions, ctor-initializers,
/// synthesized members, a destructor's deallocation function, variables'
/// initializers, `visibility` off the attribute list (K-3c), `d3`
/// closures, local classes and enumerations, the rows of closure members,
/// statement expressions, coroutines, `asm`, label addresses, the GNU,
/// vector and atomic builtin forms, `_Generic`, constraints (K-3d).
pub const ODR_DEF_SCHEME: &str = "kiln-odr-1.d3";

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
/// A variable definition: kind, type, bits, initializer.
const T_VARIABLE: u8 = 0xAB;
/// A synthesized (implicit or defaulted) member.
const T_SYNTHESIZED: u8 = 0xAD;
/// A closure's serialization (its digest is what the enclosing stream
/// and the call operator's row hold).
const T_CLOSURE: u8 = 0xB0;
/// A captured entity inside a closure's body: `u32` capture position.
const T_CAPTURE: u8 = 0xB1;
/// A local class, enumeration or closure type of the current scope: a
/// shape byte, then its `u32` local index.
const T_LOCAL_TYPE: u8 = 0xB2;
/// A member of a local class or closure: the class (`0xB2`), the member's
/// name, declaration kind and declared type.
const T_LOCAL_MEMBER: u8 = 0xB3;
/// A local class or enumeration definition.
const T_LOCAL_CLASS: u8 = 0xB4;
/// The row of a closure member (`operator()` or a specialization of it).
const T_CALL_OP_ROW: u8 = 0xB5;
/// A local entity of an enclosing scope (inside a closure): the depth
/// byte, then its `u32` index there.
const T_OUTER_LOCAL: u8 = 0xB6;

/// The names codegen already gave entities (deliverable 2 of K-3c): the
/// hasher asks before mangling. An answer must be exactly the name
/// [`crate::mangle::mangle_decl`] gives the first declaration (the
/// caller leaves out asm labels, `weakref`s and uniqued names), so a
/// digest never depends on whether the module had declared the entity.
pub trait KnownNames {
    /// The symbol name of the entity whose first declaration is `first`,
    /// if the module holds it.
    fn name(&self, first: DeclId) -> Option<&str>;
}

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
    /// Unused since `d2` (a constructor's initializers are hashed); the
    /// codes stay stable.
    Constructor,
    /// Unused since `d2` (a destructor's deallocation function is hashed).
    VirtualDestructor,
    /// Unused since `d2` (synthesized members are hashed).
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
    /// Entity names taken from [`KnownNames`] (of the misses).
    pub entity_known: u64,
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
    /// A string literal's bytes (an `asm` template, constraint, clobber).
    Str(kiln_ast::ids::StringLitId),
}

/// The lexical state a closure's call operator replaces while its body is
/// hashed (lambda.rs): the scope's definition, locals and captures.
#[derive(Debug, Default)]
struct Scope {
    def: Option<DeclId>,
    locals: FxHashMap<DeclId, u32>,
    captures: Vec<(DeclId, u32)>,
    inline_defs: Vec<DeclId>,
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
    /// The current definition's first declaration (inside a closure, its
    /// call operator's).
    def: DeclId,
    /// Captured entities of the closure whose body is walked, by capture
    /// position.
    captures: Vec<(DeclId, u32)>,
    /// Member functions of local classes hashed inside the current scope:
    /// their parameters and locals share its numbering.
    inline_defs: Vec<DeclId>,
    /// The saved enclosing scopes (and spare maps for new ones).
    scopes: Vec<Scope>,
    spare: Vec<Scope>,
    /// Closure digests, per closure type (first declaration).
    closures: FxHashMap<DeclId, DefDigest>,
    /// The work stack below this length belongs to an enclosing walk.
    floor: usize,
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
            captures: Vec::new(),
            inline_defs: Vec::new(),
            scopes: Vec::new(),
            spare: Vec::new(),
            closures: FxHashMap::default(),
            floor: 0,
            partial: false,
            seen: Vec::new(),
            stats: DefStats::default(),
        }
    }

    /// Takes entity names from `known` before mangling them.
    #[must_use]
    pub fn with_names(mut self, known: &'a dyn KnownNames) -> Self {
        self.refs.known = Some(known);
        self
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
            + self.closures.capacity() * (size_of::<(DeclId, DefDigest)>() + 1)
    }

    /// The serialization of the last definition hashed (for tests).
    pub fn last_stream(&self) -> &[u8] {
        &self.out
    }

    /// The digest of function definition `def` (a declaration with a
    /// body): its declarator and specifiers, then its body (mortar O-10).
    pub fn function(&mut self, def: DeclId) -> DefDigest {
        // A closure's call operator or a specialization of it: the
        // closure's digest (O-14, lambda.rs).
        if let Some(l) = self.ast.call_op_lambda(def) {
            return self.call_op_row(def, l);
        }
        self.walk_function(def);
        self.finish()
    }

    /// The walk of [`Self::function`] without the digest (also what fills
    /// the closure memo for a call operator's row).
    fn walk_function(&mut self, def: DeclId) {
        self.start(def);
        if decl::declarator(self, def) {
            // A constructor: its ctor-initializers; a destructor: the
            // deallocation function it selects; then the body.
            match self.ast.decls.get(def).kind() {
                DeclKind::CXXConstructor => self.ctor_inits(def),
                DeclKind::CXXDestructor => self.dealloc(self.def),
                _ => {}
            }
            let body = self.ast.decls.function(def).and_then(|f| f.body);
            self.stack.push(Work::OptStmt(body));
            self.run();
        }
    }

    /// The digest of variable definition `def` (an inline variable, a
    /// static data member of a class template, a variable template
    /// specialization; O-10, K9, N22): its declaration kind, declared
    /// type, thread-local kind, and the INITIALIZER as an expression
    /// (never its evaluated value: constant and dynamic initialization of
    /// one expression hash alike). Not hashed: `inline`, `constexpr`,
    /// `constinit`, `static` (the row's key and kind 3 say where it
    /// lives; an out-of-line definition need not repeat them).
    pub fn variable(&mut self, def: DeclId) -> DefDigest {
        self.start(def);
        let ast = self.ast;
        let n = ast.decls.get(def);
        self.byte(T_VARIABLE);
        self.byte(n.kind() as u8);
        if is_templated_def(ast, def) {
            self.partial(Area::Dependent, Site::Item(Item::Templated));
            return self.finish();
        }
        match n.ty() {
            Some(t) => self.ty(t),
            None => self.byte(T_NONE),
        }
        // The declaration with the initializer (an in-class one for a
        // static data member defined out of line).
        let with_init = ast
            .redecls_of(def)
            .find_map(|r| ast.decls.var(r).and_then(|v| v.init.map(|i| (v.spec, i))));
        let tls = ast.decls.var(def).map_or(0, |v| v.spec.tls() as u8);
        self.byte(tls);
        if ast.redecls_of(def).any(|r| decl::has_listed_attr(self, r)) {
            self.partial(Area::Attrs, Site::Item(Item::Attributes));
        }
        match with_init {
            Some((spec, init)) => {
                self.byte(spec.init_style() as u8);
                self.stack.push(Work::Expr(init));
                self.run();
            }
            None => self.byte(T_NONE),
        }
        self.finish()
    }

    fn start(&mut self, def: DeclId) {
        while !self.scopes.is_empty() {
            self.leave_scope();
        }
        self.out.clear();
        self.locals.clear();
        self.captures.clear();
        self.inline_defs.clear();
        self.seen.clear();
        self.partial = false;
        self.stack.clear();
        self.floor = 0;
        self.def = self.ast.first(def);
    }

    fn finish(&mut self) -> DefDigest {
        self.stats.definitions += 1;
        self.stats.partial_definitions += u64::from(self.partial);
        self.stats.bytes += self.out.len() as u64;
        DefDigest { hash: fnv1a_128_bytes(&self.out), partial: self.partial }
    }

    /// Runs the work stack down to [`Self::floor`].
    fn run(&mut self) {
        while self.stack.len() > self.floor {
            let Some(w) = self.stack.pop() else { break };
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
                Work::Str(id) => {
                    let (_, bytes) = self.ast.strings.get(id);
                    self.count(bytes.len());
                    self.out.extend_from_slice(bytes);
                }
            }
        }
    }

    /// Runs the work pushed since the stack had `base` items, leaving the
    /// enclosing walk's items alone.
    fn run_nested(&mut self, base: usize) {
        let saved = std::mem::replace(&mut self.floor, base);
        self.run();
        self.floor = saved;
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
            None => self.local_type(q),
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
        let ctx = ast.first(ast.decls.get(d).ctx()?);
        if ctx != self.def && !self.inline_defs.contains(&ctx) {
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
