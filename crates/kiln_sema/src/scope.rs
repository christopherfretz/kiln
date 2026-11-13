//! The scope stack (spec/sema.md §2).
//!
//! Namespace-scope and class-scope names live in the AST's per-context
//! lookup tables (`AstBuilder::register_name`); function-local names have no
//! table and live in the [`Scope`] that declared them, clang's `Scope`
//! `DeclsInScope` plus `IdResolver` collapsed into one vector.

use kiln_ast::DeclId;
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_source::symbol::Symbol;

use crate::class::NrvoState;

/// What kind of region a [`Scope`] is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
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
#[derive(Clone, Debug, kiln_snap::Snap)]
pub struct Scope {
    /// The kind of region.
    pub kind: ScopeKind,
    /// The declaration context this scope corresponds to, when it is one.
    pub entity: Option<DeclId>,
    /// Names declared in this scope that have no AST lookup table
    /// (function-local declarations), in declaration order.
    pub locals: Vec<(Symbol, DeclId, Ns)>,
    /// Block-scope operator function declarations (`bool operator==(A,
    /// A);` in a function body), in declaration order: found only by a
    /// local class's friend redeclaration ([class.friend]/11).
    pub local_ops: Vec<(OverloadedOperator, DeclId)>,
    /// The `UsingShadow`s that block-scope using-declarators naming an
    /// operator function or a literal operator (`using N::operator+;`,
    /// `using udl::operator""_t;`, [namespace.udecl]/1) introduce, in
    /// declaration order: found by unqualified lookup of that name from
    /// the point of declaration to the end of the block
    /// ([basic.lookup.unqual]/4, [over.match.oper]/3.2, [lex.ext]/5).
    pub local_names: Vec<(DeclName, DeclId)>,
    /// GNU local labels (`__label__ name;`) declared at the start of this
    /// block: they shadow the function's labels of the same name inside
    /// it (spec/sema.md §7.4).
    pub labels: Vec<(Symbol, DeclId)>,
    /// Namespaces nominated by `using namespace` in this scope.
    pub using_dirs: Vec<DeclId>,
    /// The NRVO candidate of this scope (spec/sema-class.md §5.5).
    pub nrvo: NrvoState,
    /// A try-block's compound statement (clang `TryScope`): the search
    /// for a thrown variable's scope stops here ([class.copy.elision]/3.2).
    pub try_barrier: bool,
    /// A handler's scope (clang `CatchScope`): `co_await` and `co_yield`
    /// are ill-formed inside it up to the enclosing function scope
    /// ([expr.await]/2).
    pub catch_handler: bool,
    /// For a namespace or translation-unit scope: the index of the first
    /// declaration created while this scope instance is open. Members of
    /// the entity with a lower index were declared in an earlier
    /// definition of the namespace (clang's `Scope::isDeclScope`, which
    /// orders the names of an ambiguous lookup). `u32::MAX` for none.
    pub first_decl: u32,
    /// The unrecoverable error count when the scope was entered: an error
    /// (not a warning promoted by `-Werror`) inside it
    /// suppresses its unused-declaration warnings (clang
    /// `Scope::hasUnrecoverableErrorOccurred`).
    pub errors: u32,
}

/// The name space (C's "tag" vs "ordinary" split) of a local entry.
#[derive(Copy, Clone, Debug, PartialEq, Eq, kiln_snap::Snap)]
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
            local_ops: Vec::new(),
            local_names: Vec::new(),
            labels: Vec::new(),
            using_dirs: Vec::new(),
            nrvo: NrvoState::default(),
            try_barrier: false,
            catch_handler: false,
            first_decl: u32::MAX,
            errors: 0,
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
    /// context, the scope stack (per scope: kind, entity, number of
    /// locals and the last of them, number of local using-declarators and
    /// using-directives) and the open prototype parameter lists. The last
    /// local tells apart two scope stacks of the same shape, as when the
    /// declaration scopes of two function templates with alike parameter
    /// lists are reinstated in turn (WP-919): locals are only appended and
    /// a declaration is a local of one scope. While it is unchanged, [`crate::Sema::classify_name`]
    /// answers every query the same way, so the parser may reuse a
    /// lookahead scan (spec/parse.md §2.5, the template-argument memo).
    pub fn lookup_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = kiln_source::fx::FxHasher::default();
        (self.ast.decls.len(), self.ast.lookup.generation(), self.ctx).hash(&mut h);
        for s in &self.scopes {
            let last = s.locals.last().map(|l| l.1);
            (s.kind as u8, s.entity, s.locals.len(), last, s.local_names.len(), s.using_dirs.len())
                .hash(&mut h);
        }
        for f in &self.prototype_params {
            (f.depth, f.params.len(), f.index).hash(&mut h);
        }
        for l in &self.lambdas {
            l.init_cap_vars.len().hash(&mut h);
        }
        h.finish()
    }
}
