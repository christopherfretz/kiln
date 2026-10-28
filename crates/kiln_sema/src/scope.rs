//! The scope stack (spec/sema.md §2).
//!
//! Namespace-scope and class-scope names live in the AST's per-context
//! lookup tables (`AstBuilder::register_name`); function-local names have no
//! table and live in the [`Scope`] that declared them, clang's `Scope`
//! `DeclsInScope` plus `IdResolver` collapsed into one vector.

use kiln_ast::DeclId;
use kiln_source::symbol::Symbol;

/// What kind of region a [`Scope`] is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    /// The translation unit.
    TranslationUnit,
    /// A namespace body.
    Namespace,
    /// A `extern "C" { ... }` block (names go to the enclosing namespace).
    LinkageSpec,
    /// A struct/union/class body.
    Record,
    /// An enum body.
    Enum,
    /// A function prototype's parameter list.
    Prototype,
    /// A function body's outermost scope (holds the parameters).
    Function,
    /// A compound statement or the scope of a control statement.
    Block,
}

/// One entry of the scope stack.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The kind of region.
    pub kind: ScopeKind,
    /// The declaration context this scope corresponds to, when it is one.
    pub entity: Option<DeclId>,
    /// Names declared in this scope that have no AST lookup table
    /// (function-local declarations), in declaration order.
    pub locals: Vec<(Symbol, DeclId, Ns)>,
    /// Namespaces nominated by `using namespace` in this scope.
    pub using_dirs: Vec<DeclId>,
}

/// The name space (C's "tag" vs "ordinary" split) of a local entry.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Ns {
    /// Variables, functions, typedefs, enumerators.
    Ordinary,
    /// struct/union/enum tags.
    Tag,
}

impl Scope {
    /// A fresh scope of `kind` for `entity`.
    pub fn new(kind: ScopeKind, entity: Option<DeclId>) -> Self {
        Scope { kind, entity, locals: Vec::new(), using_dirs: Vec::new() }
    }

    /// True for scopes whose names live in the scope itself.
    pub fn is_local(&self) -> bool {
        matches!(self.kind, ScopeKind::Function | ScopeKind::Block | ScopeKind::Prototype)
    }

    /// The most recent local `name` in name space `ns`.
    pub fn find(&self, name: Symbol, ns: Ns) -> Option<DeclId> {
        self.locals.iter().rev().find(|(s, _, n)| *s == name && *n == ns).map(|e| e.1)
    }
}
