//! Name lookup ([basic.lookup], spec/sema.md §3).
//!
//! Unqualified lookup walks the scope stack innermost-first: function-local
//! scopes search their own `locals`, context scopes (translation unit,
//! namespace, class, enumeration) the AST lookup table of their entity
//! plus the namespaces nominated by `using namespace` in that scope.
//! Qualified lookup searches one context's table.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_ast::names::DeclName;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::scope::{Ns, ScopeKind};

/// What a lookup is for: which declarations it may find.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LookupKind {
    /// Ordinary lookup: anything; non-tags hide tags of the same scope.
    Ordinary,
    /// An elaborated-type-specifier (`struct S`): tags only
    /// ([basic.lookup.elab]).
    Tag,
    /// The name before `::` ([basic.lookup.qual]/1): namespaces, types
    /// and enumerations.
    Nested,
    /// A using-declarator's target ([namespace.udecl]/1): everything,
    /// tags and non-tags alike (no hiding).
    All,
}

/// The result of a lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The declarations found (latest redeclarations), at most one per
    /// entity; more than one is an ambiguity (or an overload set, which
    /// M2 does not support). Using-shadows are looked through: these are
    /// their targets.
    pub decls: Vec<DeclId>,
    /// Parallel to `decls`: the declaration lookup actually found (the
    /// `UsingShadow` when the entity was found through a
    /// using-declaration, else the same declaration), clang's
    /// `FoundDecl`.
    pub found: Vec<DeclId>,
}

impl Found {
    /// The single result.
    pub fn single(&self) -> Option<DeclId> {
        if self.decls.len() == 1 { Some(self.decls[0]) } else { None }
    }

    /// True if nothing was found.
    pub fn is_empty(&self) -> bool {
        self.decls.is_empty()
    }

    /// The using-shadow through which `decls[i]` was found, if any.
    pub fn shadow(&self, i: usize) -> Option<DeclId> {
        let f = *self.found.get(i)?;
        (f != self.decls[i]).then_some(f)
    }
}

/// True if `k` declares a type.
pub fn is_type_kind(k: DeclKind) -> bool {
    matches!(k, DeclKind::CXXRecord | DeclKind::Enum | DeclKind::Typedef | DeclKind::TypeAlias)
}

/// True if `k` is a tag (class or enumeration).
pub fn is_tag_kind(k: DeclKind) -> bool {
    matches!(k, DeclKind::CXXRecord | DeclKind::Enum)
}

impl Sema<'_, '_> {
    /// Interns `s`.
    pub fn intern(&self, s: &str) -> Symbol {
        self.ast.symbols().intern(s)
    }

    pub(crate) fn kind_of(&self, d: DeclId) -> DeclKind {
        self.ast.decls.get(d).kind()
    }

    fn accept(&self, d: DeclId, kind: LookupKind) -> bool {
        let k = self.kind_of(d);
        match kind {
            LookupKind::Ordinary | LookupKind::All => true,
            LookupKind::Tag => is_tag_kind(k),
            LookupKind::Nested => {
                is_type_kind(k) || matches!(k, DeclKind::Namespace | DeclKind::NamespaceAlias)
            }
        }
    }

    /// Filters the results of one scope: using-shadows are looked through
    /// ([namespace.udecl]/1); non-tags hide tags ([basic.scope.hiding]/2)
    /// for ordinary lookup; duplicates (the same entity via two paths,
    /// e.g. a declaration and a using-declaration naming it) collapse to
    /// the later path; an unresolved `using_if_exists` target is dropped
    /// when anything else is found.
    fn filter(&self, v: Vec<DeclId>, kind: LookupKind) -> Found {
        let mut out = Found::default();
        for raw in v {
            let d = self.ast.strip_shadow(raw);
            if !self.accept(d, kind) {
                continue;
            }
            let f = self.ast.first(d);
            match out.decls.iter().position(|&o| self.ast.first(o) == f) {
                Some(i) => {
                    out.decls[i] = d;
                    out.found[i] = raw;
                }
                None => {
                    out.decls.push(d);
                    out.found.push(raw);
                }
            }
        }
        let keep = |out: &mut Found, pred: &dyn Fn(DeclId) -> bool| {
            let mut i = 0;
            while i < out.decls.len() {
                if pred(out.decls[i]) {
                    i += 1;
                } else {
                    out.decls.remove(i);
                    out.found.remove(i);
                }
            }
        };
        if kind == LookupKind::Ordinary && out.decls.iter().any(|&d| !is_tag_kind(self.kind_of(d)))
        {
            keep(&mut out, &|d| !is_tag_kind(self.kind_of(d)));
        }
        let unresolved = |d: DeclId| self.kind_of(d) == DeclKind::UnresolvedUsingIfExists;
        if out.decls.iter().any(|&d| !unresolved(d)) {
            keep(&mut out, &|d| !unresolved(d));
        }
        out
    }

    /// The members of context `ctx` named `name` (its own table, inline
    /// namespaces included).
    pub fn lookup_in(&self, ctx: DeclId, name: Symbol, kind: LookupKind) -> Found {
        let r = self.ast.lookup_local(ctx, DeclName::ident(name));
        self.filter(r.iter().collect(), kind)
    }

    /// Qualified lookup of `name` in `ctx` ([namespace.qual],
    /// [class.qual]). Namespaces nominated by using-directives in `ctx`
    /// are searched when `ctx` itself has no result.
    pub fn lookup_qualified(&self, ctx: DeclId, name: Symbol, kind: LookupKind) -> Found {
        let found = self.lookup_in(ctx, name, kind);
        if !found.is_empty() {
            return found;
        }
        // Using-directives at namespace scope are recorded on the scope
        // that was active; search scopes whose entity is `ctx`.
        let mut v = Vec::new();
        for s in &self.scopes {
            if s.entity.map(|e| self.ast.first(e)) == Some(self.ast.first(ctx)) {
                for &n in &s.using_dirs {
                    v.extend(self.ast.lookup_local(n, DeclName::ident(name)).iter());
                }
            }
        }
        self.filter(v, kind)
    }

    /// Unqualified lookup of `name` from the current scope
    /// ([basic.lookup.unqual]).
    pub fn lookup_name(&self, name: Symbol, kind: LookupKind) -> Found {
        // Namespaces nominated by block-scope using-directives: their
        // members appear as if declared in the nearest enclosing
        // namespace that contains both the directive and the nominated
        // namespace ([namespace.udir]/2).
        let mut local_dirs: Vec<DeclId> = Vec::new();
        for s in self.scopes.iter().rev() {
            if s.is_local() {
                local_dirs.extend(s.using_dirs.iter().copied());
                let hit = match kind {
                    LookupKind::Tag => s.find(name, Ns::Tag),
                    LookupKind::Ordinary => {
                        s.find(name, Ns::Ordinary).or_else(|| s.find(name, Ns::Tag))
                    }
                    LookupKind::All => {
                        let v: Vec<DeclId> =
                            s.locals.iter().filter(|e| e.0 == name).map(|e| e.1).collect();
                        let found = self.filter(v, kind);
                        if !found.is_empty() {
                            return found;
                        }
                        continue;
                    }
                    LookupKind::Nested => s
                        .locals
                        .iter()
                        .rev()
                        .find(|(n, d, _)| {
                            *n == name && self.accept(self.ast.strip_shadow(*d), kind)
                        })
                        .map(|e| e.1),
                };
                if let Some(raw) = hit {
                    let found = self.filter(vec![raw], kind);
                    if !found.is_empty() {
                        return found;
                    }
                }
                continue;
            }
            if s.kind == ScopeKind::LinkageSpec {
                continue;
            }
            let Some(entity) = s.entity else { continue };
            let mut v: Vec<DeclId> =
                self.ast.lookup_local(entity, DeclName::ident(name)).iter().collect();
            let own = self.filter(v.clone(), kind);
            if !own.is_empty() {
                return own;
            }
            v.clear();
            for &n in &s.using_dirs {
                v.extend(self.ast.lookup_local(n, DeclName::ident(name)).iter());
            }
            if matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit) {
                let mut i = 0;
                while i < local_dirs.len() {
                    if self.encloses(entity, local_dirs[i]) {
                        let n = local_dirs.remove(i);
                        v.extend(self.ast.lookup_local(n, DeclName::ident(name)).iter());
                    } else {
                        i += 1;
                    }
                }
            }
            let via = self.filter(v, kind);
            if !via.is_empty() {
                return via;
            }
        }
        Found::default()
    }

    /// True if namespace `outer` is `inner` or encloses it.
    fn encloses(&self, outer: DeclId, inner: DeclId) -> bool {
        let outer = self.ast.first(outer);
        let mut cur = Some(inner);
        for _ in 0..10_000 {
            let Some(c) = cur else { return false };
            if self.ast.first(c) == outer {
                return true;
            }
            cur = self.ast.decls.get(c).ctx();
        }
        false
    }

    /// Looks `name` up in the innermost scope only (for redeclaration
    /// checks, [basic.scope.declarative]): its locals, or the table of
    /// its entity. Using-shadows are not included (see
    /// [`Sema::current_scope_shadows`]).
    pub fn lookup_current_scope(&self, name: Symbol) -> Vec<DeclId> {
        let mut v = self.current_scope_raw(name);
        v.retain(|&d| self.kind_of(d) != DeclKind::UsingShadow);
        v
    }

    /// The using-shadows named `name` in the innermost scope (for the
    /// [namespace.udecl]/11 conflict checks).
    pub fn current_scope_shadows(&self, name: Symbol) -> Vec<DeclId> {
        let mut v = self.current_scope_raw(name);
        v.retain(|&d| self.kind_of(d) == DeclKind::UsingShadow);
        v
    }

    pub(crate) fn current_scope_raw(&self, name: Symbol) -> Vec<DeclId> {
        let Some(i) = self.scopes.iter().rposition(|s| s.kind != ScopeKind::LinkageSpec) else {
            return Vec::new();
        };
        let s = &self.scopes[i];
        if s.is_local() {
            let mut out: Vec<DeclId> = Vec::new();
            // The outermost block of a control statement's sub-statement
            // also conflicts with the condition's names ([basic.scope.block]/2).
            if s.kind == ScopeKind::Block
                && let Some(c) = i.checked_sub(1).map(|j| &self.scopes[j])
                && c.kind == ScopeKind::Control
            {
                out.extend(c.locals.iter().filter(|e| e.0 == name).map(|e| e.1));
            }
            out.extend(s.locals.iter().filter(|e| e.0 == name).map(|e| e.1));
            return out;
        }
        match s.entity {
            Some(e) => self.ast.lookup_local(e, DeclName::ident(name)).iter().collect(),
            None => Vec::new(),
        }
    }

    /// Adds a function-local declaration to the innermost local scope.
    pub fn push_local(&mut self, name: Symbol, d: DeclId) {
        let ns = if is_tag_kind(self.kind_of(self.ast.strip_shadow(d))) {
            Ns::Tag
        } else {
            Ns::Ordinary
        };
        if let Some(s) = self.scopes.iter_mut().rev().find(|s| s.is_local()) {
            s.locals.push((name, d, ns));
        }
    }

    /// True while the innermost non-linkage scope is function-local.
    pub fn in_local_scope(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .find(|s| !matches!(s.kind, ScopeKind::LinkageSpec | ScopeKind::Enum))
            .is_some_and(super::scope::Scope::is_local)
    }
}
