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
    /// The `using namespace` directives of this scope; each names its
    /// namespace through `Ast::using_target`.
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
    /// The try-block or a handler of a function-try-block: its outermost
    /// block, and a handler's exception-declaration, may not redeclare a
    /// parameter ([basic.scope.block]/2).
    pub fn_try_block: bool,
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
            fn_try_block: false,
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

    /// Whether the unqualified-lookup walk must visit this scope: any
    /// scope but a block or control scope holding no locals,
    /// using-declarators or using-directives ([`ScopeStack::walk_below`];
    /// a function, prototype or requires-expression scope bounds the
    /// NRVO walk of a `return`, so it is always visited).
    pub fn walk_visits(&self) -> bool {
        !matches!(self.kind, ScopeKind::Block | ScopeKind::Control)
            || !self.locals.is_empty()
            || !self.local_names.is_empty()
            || !self.using_dirs.is_empty()
    }

    /// The most recent local `name` in name space `ns`.
    pub fn find(&self, name: Symbol, ns: Ns) -> Option<DeclId> {
        self.locals.iter().rev().find(|(s, _, n)| *s == name && *n == ns).map(|e| e.1)
    }
}

/// The next [`ScopeStack`] revision: one counter for every stack, so two
/// stacks (one moved aside and later reinstated, a fresh one in its
/// place) never share a revision unless one is an unchanged clone of the
/// other.
static NEXT_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn fresh_revision() -> u64 {
    NEXT_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// The scope stack, innermost last (spec/sema.md §2.1). It reads as the
/// `Vec<Scope>` it holds; every mutable access but one of NRVO state
/// ([`ScopeStack::nrvo_mut`]) gives it a new revision
/// ([`ScopeStack::revision`], the scope part of [`crate::Sema::lookup_stamp`]),
/// so the fingerprint costs O(1), not a hash of every open scope per
/// lookahead query, which made an `else if` chain of depth N O(N^2)
/// (WP-2478).
///
/// It also keeps the walk index of the unqualified-lookup walk: for each
/// scope, the nearest scope below it that the walk must visit, skipping
/// the block and control scopes that hold nothing (no locals,
/// using-declarators or using-directives; [`ScopeStack::walk_below`]),
/// and the template parameter scopes. `push`, `pop`, `truncate` and
/// `last_mut` keep the index; any other mutable access (through the
/// `Vec`) drops it, rebuilt on the next walk.
#[derive(Clone, Debug)]
pub struct ScopeStack {
    scopes: Vec<Scope>,
    revision: u64,
    /// Per scope, the revision before its push and the `epoch` then:
    /// popping it gives that revision back when no access through the
    /// `Vec` came in between, so a lookahead memo keyed on the stamp
    /// survives a scope entered and left.
    pushed: Vec<(u64, u64)>,
    /// Bumped by every access through the `Vec`.
    epoch: u64,
    index: std::cell::RefCell<WalkIndex>,
}

/// [`ScopeStack`]'s walk index.
#[derive(Clone, Debug, Default)]
struct WalkIndex {
    /// Whether `below` and `tparams` describe the stack.
    valid: bool,
    /// Per scope, the nearest scope below it the walk visits
    /// (`u32::MAX`: none).
    below: Vec<u32>,
    /// The template parameter scopes, innermost last.
    tparams: Vec<usize>,
}

impl WalkIndex {
    /// The `below` entry of a scope pushed on `scopes`.
    fn below_of_next(&self, scopes: &[Scope]) -> u32 {
        match scopes.len().checked_sub(1) {
            None => u32::MAX,
            Some(t) if scopes[t].walk_visits() => u32::try_from(t).unwrap_or(u32::MAX),
            Some(t) => self.below[t],
        }
    }

    fn rebuild(&mut self, scopes: &[Scope]) {
        kiln_ast::expr::note_search_steps(scopes.len() as u64);
        self.below.clear();
        self.tparams.clear();
        for i in 0..scopes.len() {
            let b = self.below_of_next(&scopes[..i]);
            self.below.push(b);
            if scopes[i].kind == ScopeKind::TemplateParams {
                self.tparams.push(i);
            }
        }
        self.valid = true;
    }

    fn truncate(&mut self, n: usize) {
        self.below.truncate(n);
        while self.tparams.last().is_some_and(|&j| j >= n) {
            self.tparams.pop();
        }
    }
}

impl Default for ScopeStack {
    fn default() -> Self {
        ScopeStack::from(Vec::new())
    }
}

impl From<Vec<Scope>> for ScopeStack {
    fn from(scopes: Vec<Scope>) -> Self {
        let pushed = vec![(0, u64::MAX); scopes.len()];
        ScopeStack {
            scopes,
            revision: fresh_revision(),
            pushed,
            epoch: 0,
            index: std::cell::RefCell::default(),
        }
    }
}

impl std::ops::Deref for ScopeStack {
    type Target = Vec<Scope>;
    fn deref(&self) -> &Vec<Scope> {
        &self.scopes
    }
}

impl std::ops::DerefMut for ScopeStack {
    /// Any mutable access: a new revision, and the walk index dropped.
    fn deref_mut(&mut self) -> &mut Vec<Scope> {
        self.revision = fresh_revision();
        self.epoch += 1;
        self.index.get_mut().valid = false;
        &mut self.scopes
    }
}

impl<'a> IntoIterator for &'a ScopeStack {
    type Item = &'a Scope;
    type IntoIter = std::slice::Iter<'a, Scope>;
    fn into_iter(self) -> Self::IntoIter {
        self.scopes.iter()
    }
}

impl ScopeStack {
    /// The revision: equal revisions mean equal stacks.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Enters `s` (the walk index kept).
    pub fn push(&mut self, s: Scope) {
        self.pushed.resize(self.scopes.len(), (0, u64::MAX));
        self.pushed.push((self.revision, self.epoch));
        self.revision = fresh_revision();
        let index = self.index.get_mut();
        if index.valid {
            let b = index.below_of_next(&self.scopes);
            index.below.push(b);
            if s.kind == ScopeKind::TemplateParams {
                index.tparams.push(self.scopes.len());
            }
        }
        self.scopes.push(s);
    }

    /// Leaves the innermost scope (the walk index kept).
    pub fn pop(&mut self) -> Option<Scope> {
        let n = self.scopes.len().checked_sub(1)?;
        let s = self.scopes.pop();
        self.left(n);
        s
    }

    /// Leaves every scope above the first `n` (the walk index kept).
    pub fn truncate(&mut self, n: usize) {
        if n < self.scopes.len() {
            self.scopes.truncate(n);
            self.left(n);
        }
    }

    /// The scopes from the `n`th on were left: the revision before the
    /// `n`th was pushed comes back when nothing changed the scopes below
    /// it since.
    fn left(&mut self, n: usize) {
        self.revision = match self.pushed.get(n) {
            Some(&(r, e)) if e == self.epoch => r,
            _ => fresh_revision(),
        };
        self.pushed.truncate(n);
        self.index.get_mut().truncate(n);
    }

    /// The innermost scope, to change (the walk index kept: an entry
    /// depends on the scopes below its own only).
    pub fn last_mut(&mut self) -> Option<&mut Scope> {
        self.revision = fresh_revision();
        self.scopes.last_mut()
    }

    /// Scope `i`, to change: as [`ScopeStack::last_mut`] for the
    /// innermost scope, else an access through the `Vec`.
    pub fn scope_mut(&mut self, i: usize) -> &mut Scope {
        if i + 1 == self.scopes.len() {
            self.revision = fresh_revision();
            &mut self.scopes[i]
        } else {
            &mut (**self)[i]
        }
    }

    /// The NRVO state of scope `i`, to change. The revision and the walk
    /// index stay: no lookup reads NRVO state, and it does not decide
    /// which scopes the walk visits.
    pub fn nrvo_mut(&mut self, i: usize) -> &mut NrvoState {
        &mut self.scopes[i].nrvo
    }

    fn with_index<R>(&self, f: impl FnOnce(&WalkIndex) -> R) -> R {
        if let Ok(mut index) = self.index.try_borrow_mut()
            && !index.valid
        {
            index.rebuild(&self.scopes);
        }
        match self.index.try_borrow() {
            Ok(index) if index.valid => f(&index),
            _ => {
                let mut fresh = WalkIndex::default();
                fresh.rebuild(&self.scopes);
                f(&fresh)
            }
        }
    }

    /// The nearest scope below scope `i` that the unqualified-lookup
    /// walk visits: every scope but a block or control scope that holds
    /// nothing ([`Scope::walk_visits`]).
    pub fn walk_below(&self, i: usize) -> Option<usize> {
        kiln_ast::expr::note_search_steps(1);
        self.with_index(|x| x.below.get(i).copied()).filter(|&b| b != u32::MAX).map(|b| b as usize)
    }

    /// The scopes the walk visits, innermost first, with their indices:
    /// the innermost scope, then [`ScopeStack::walk_below`] in turn.
    pub fn walk(&self) -> impl Iterator<Item = (usize, &Scope)> + '_ {
        let mut next = self.scopes.len().checked_sub(1);
        std::iter::from_fn(move || {
            let i = next?;
            next = self.walk_below(i);
            Some((i, &self.scopes[i]))
        })
    }

    /// The indices of the template parameter scopes, innermost last.
    pub fn template_param_scopes(&self) -> Vec<usize> {
        self.with_index(|x| x.tparams.clone())
    }
}

/// [`ScopeStack`] is written as its scopes (the revision and the walk
/// index are rebuilt).
impl kiln_snap::Snap for ScopeStack {
    const SCHEMA_HASH: u64 = <Vec<Scope> as kiln_snap::Snap>::SCHEMA_HASH;
    const SHAPE_HASH: u64 = <Vec<Scope> as kiln_snap::Snap>::SHAPE_HASH;

    fn put(&self, w: &mut kiln_snap::Writer) {
        self.scopes.put(w);
    }

    fn get(r: &mut kiln_snap::Reader<'_>) -> Result<Self, kiln_snap::SnapError> {
        Ok(ScopeStack::from(<Vec<Scope> as kiln_snap::Snap>::get(r)?))
    }

    fn schema(s: &mut kiln_snap::Schema) {
        <Vec<Scope> as kiln_snap::Snap>::schema(s);
    }
}

impl crate::Sema<'_, '_> {
    /// A fingerprint of the state name classification reads: the number
    /// of declarations, the AST lookup tables' generation, the current
    /// context, the scope stack's revision ([`ScopeStack::revision`]:
    /// a new one at every change of a scope) and the open prototype
    /// parameter lists and lambda init-captures. A revision tells apart
    /// two scope stacks of the same shape, as when the declaration scopes
    /// of two function templates with alike parameter lists are
    /// reinstated in turn (WP-919). While it is unchanged,
    /// [`crate::Sema::classify_name`] answers every query the same way, so
    /// the parser may reuse a lookahead scan (spec/parse.md §2.5, the
    /// template-argument memo). O(1) in the scope depth (WP-2478).
    pub fn lookup_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = kiln_source::fx::FxHasher::default();
        kiln_ast::expr::note_search_steps(1);
        (self.ast.decls.len(), self.ast.lookup.generation(), self.ctx, self.scopes.revision())
            .hash(&mut h);
        for f in &self.prototype_params {
            (f.depth, f.params.len(), f.index).hash(&mut h);
        }
        for l in &self.lambdas {
            l.init_cap_vars.len().hash(&mut h);
        }
        h.finish()
    }
}
