//! The scope stack (spec/sema.md §2).
//!
//! Namespace-scope and class-scope names live in the AST's per-context
//! lookup tables (`AstBuilder::register_name`); function-local names have no
//! table and live in the [`Scope`] that declared them, clang's `Scope`
//! `DeclsInScope` plus `IdResolver` collapsed into one vector.

use kiln_ast::DeclId;
use kiln_source::symbol::Symbol;

use crate::class::NrvoState;

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
    /// A compound statement, or the implicit block of a sub-statement
    /// that is not one ([stmt.pre]/1).
    Block,
    /// A selection or iteration statement's own scope, holding its
    /// init-statement and condition declarations ([stmt.pre]/2). The
    /// outermost block of its sub-statement may not redeclare them
    /// ([basic.scope.block]/2).
    Control,
    /// A template parameter list and the declaration it heads
    /// (spec/sema-templates.md §2.1): holds the template parameters,
    /// searched by unqualified lookup but transparent for declarations
    /// (they go to the enclosing scope).
    TemplateParams,
    /// The parameters of a requires-expression ([expr.prim.req]/5,
    /// spec/sema-templates.md §2.6).
    RequiresExpr,
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
    /// The NRVO candidate of this scope (spec/sema-class.md §5.5).
    pub nrvo: NrvoState,
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
        Scope {
            kind,
            entity,
            locals: Vec::new(),
            using_dirs: Vec::new(),
            nrvo: NrvoState::default(),
        }
    }

    /// True for scopes whose names live in the scope itself.
    pub fn is_local(&self) -> bool {
        matches!(
            self.kind,
            ScopeKind::Function
                | ScopeKind::Block
                | ScopeKind::Control
                | ScopeKind::Prototype
                | ScopeKind::RequiresExpr
        )
    }

    /// True for scopes that declarations pass through to the enclosing
    /// one (`extern "C" { }` blocks and template parameter scopes).
    pub fn is_transparent(&self) -> bool {
        matches!(self.kind, ScopeKind::LinkageSpec | ScopeKind::TemplateParams)
    }

    /// The most recent local `name` in name space `ns`.
    pub fn find(&self, name: Symbol, ns: Ns) -> Option<DeclId> {
        self.locals.iter().rev().find(|(s, _, n)| *s == name && *n == ns).map(|e| e.1)
    }
}

impl crate::Sema<'_, '_> {
    /// A fingerprint of the state name classification reads: the number
    /// of declarations, the AST lookup tables' generation, the current
    /// context, the shape of the scope stack (kind, entity, number of
    /// locals and using-directives per scope) and the open prototype
    /// parameter lists. While it is unchanged, [`crate::Sema::classify_name`]
    /// answers every query the same way, so the parser may reuse a
    /// lookahead scan (spec/parse.md §2.5, the template-argument memo).
    pub fn lookup_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = kiln_source::fx::FxHasher::default();
        (self.ast.decls.len(), self.ast.lookup.generation(), self.ctx).hash(&mut h);
        for s in &self.scopes {
            (s.kind as u8, s.entity, s.locals.len(), s.using_dirs.len()).hash(&mut h);
        }
        for f in &self.prototype_params {
            (f.depth, f.params.len(), f.index).hash(&mut h);
        }
        h.finish()
    }
}
