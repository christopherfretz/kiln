//! Name lookup ([basic.lookup], spec/sema.md §3).
//!
//! Unqualified lookup walks the scope stack innermost-first: function-local
//! scopes search their own `locals`, context scopes (translation unit,
//! namespace, class, enumeration) the AST lookup table of their entity
//! plus the namespaces nominated by `using namespace` in that scope.
//! Qualified lookup searches one context's table.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
/// Lookup-table entry flags (owned by spec/sema-overload.md §2.3): a hidden
/// friend's entry is `ADL_ONLY`, invisible to ordinary and qualified
/// lookup ([namespace.memdef]/3) but seen by redeclaration lookup and ADL.
pub use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::DeclName;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::bases::{BasePath, subobject_key};
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
    /// The naming class ([class.access.base]/5) when the name was found
    /// by class member lookup: the class searched (the qualifier's class,
    /// the object expression's class, or the class whose scope contained
    /// the unqualified name).
    pub naming: Option<DeclId>,
    /// Set when class member lookup is ambiguous
    /// ([class.member.lookup]/6); the diagnostic is
    /// [`Sema::diagnose_member_ambiguity`]'s.
    pub ambiguity: Option<MemberAmbiguity>,
}

/// Why a class member lookup is ambiguous ([class.member.lookup]/6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberAmbiguity {
    /// Different declaration sets were found in different base classes
    /// (clang `AmbiguousBaseSubobjectTypes`): every declaration found.
    Types(Vec<DeclId>),
    /// The same non-static member was found in distinct base class
    /// subobjects (clang `AmbiguousBaseSubobjects`): the member and the
    /// paths from the naming class to each subobject.
    Subobjects {
        /// The member found.
        decl: DeclId,
        /// The paths to the subobjects it was found in.
        paths: Vec<BasePath>,
    },
}

/// One result of the recursive class member search: the declarations
/// found in some class, and the paths from the searched class to it.
struct ClassHit {
    found: Found,
    paths: Vec<BasePath>,
    /// Different declaration sets met: the lookup is ambiguous.
    mixed: bool,
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
            if let Some(i) = out.decls.iter().position(|&o| self.ast.first(o) == f) {
                out.decls[i] = d;
                out.found[i] = raw;
            } else {
                out.decls.push(d);
                out.found.push(raw);
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
        self.filter(r.iter_ordinary().collect(), kind)
    }

    /// Qualified lookup of `name` in `ctx` ([namespace.qual],
    /// [class.qual]). Namespaces nominated by using-directives in `ctx`
    /// are searched when `ctx` itself has no result.
    pub fn lookup_qualified(&self, ctx: DeclId, name: Symbol, kind: LookupKind) -> Found {
        if self.kind_of(ctx) == DeclKind::CXXRecord {
            return self.lookup_in_class(ctx, name, kind);
        }
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
                    v.extend(self.ast.lookup_local(n, DeclName::ident(name)).iter_ordinary());
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
            if s.kind == ScopeKind::Record {
                let found = self.lookup_in_class(entity, name, kind);
                if !found.is_empty() || found.ambiguity.is_some() {
                    return found;
                }
                continue;
            }
            let mut v: Vec<DeclId> =
                self.ast.lookup_local(entity, DeclName::ident(name)).iter_ordinary().collect();
            let own = self.filter(v.clone(), kind);
            if !own.is_empty() {
                return own;
            }
            v.clear();
            for &n in &s.using_dirs {
                v.extend(self.ast.lookup_local(n, DeclName::ident(name)).iter_ordinary());
            }
            if matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit) {
                let mut i = 0;
                while i < local_dirs.len() {
                    if self.encloses(entity, local_dirs[i]) {
                        let n = local_dirs.remove(i);
                        v.extend(self.ast.lookup_local(n, DeclName::ident(name)).iter_ordinary());
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

    /// Class member lookup of `name` in class `c` ([class.member.lookup]):
    /// the class's own members (its injected-class-name included,
    /// [class.pre]/2), else the members found in its direct bases,
    /// searched recursively. A name found in a class hides the same name
    /// in that class's bases. Different declaration sets found through
    /// different bases, or one non-static member found in distinct base
    /// subobjects, make the lookup ambiguous ([`Found::ambiguity`]); a
    /// static member, type or enumerator found in several subobjects of
    /// the same type is not ([class.member.lookup]/6). Virtual base
    /// classes are not supported (WP-078a), so no declaration dominates
    /// another.
    pub fn lookup_in_class(&self, c: DeclId, name: Symbol, kind: LookupKind) -> Found {
        let def = self.ast.definition(c).unwrap_or(c);
        let Some(hit) = self.class_hit(def, name, kind, 0) else {
            return Found { naming: Some(def), ..Found::default() };
        };
        let mut found = hit.found;
        found.naming = Some(def);
        if hit.mixed {
            found.ambiguity = Some(MemberAmbiguity::Types(found.decls.clone()));
            return found;
        }
        let mut keys: Vec<(Option<DeclId>, Vec<u32>)> = Vec::new();
        for p in &hit.paths {
            let k = subobject_key(self, p);
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        if keys.len() > 1
            && !found.decls.iter().all(|&d| self.is_static_member_kind(d))
            && let Some(&decl) = found.decls.first()
        {
            found.ambiguity = Some(MemberAmbiguity::Subobjects { decl, paths: hit.paths });
        }
        found
    }

    /// clang's `HasOnlyStaticMembers` test for one declaration: a static
    /// data member, a static member function, a type or an enumerator.
    fn is_static_member_kind(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::Var | DeclKind::EnumConstant => true,
            k if is_type_kind(k) => true,
            DeclKind::CXXMethod => self
                .ast
                .decls
                .function(d)
                .is_some_and(|f| f.spec.storage() == kiln_ast::decl::FnStorage::Static),
            _ => false,
        }
    }

    fn class_hit(
        &self,
        def: DeclId,
        name: Symbol,
        kind: LookupKind,
        depth: usize,
    ) -> Option<ClassHit> {
        if depth > 256 {
            return None;
        }
        let mut own = self.filter(
            self.ast.lookup_local(def, DeclName::ident(name)).iter_ordinary().collect(),
            kind,
        );
        if own.is_empty()
            && self.ast.decls.get(def).name() == DeclName::ident(name)
            && !matches!(kind, LookupKind::All)
        {
            // The injected-class-name ([class.pre]/2).
            own.decls.push(def);
            own.found.push(def);
        }
        if !own.is_empty() {
            let empty = BasePath {
                specs: Vec::new(),
                classes: Vec::new(),
                written: Vec::new(),
                last_virtual: None,
                first_virtual: None,
            };
            return Some(ClassHit { found: own, paths: vec![empty], mixed: false });
        }
        let rec = self.ast.decls.record(def)?;
        let idx: Vec<u32> = self.ast.lists.get(rec.bases).to_vec();
        let specs: Vec<kiln_ast::decl::BaseSpec> =
            self.ast.decls.bases(def, &self.ast.lists).copied().collect();
        let mut acc: Option<ClassHit> = None;
        for (&i, spec) in idx.iter().zip(specs) {
            let Some(b) = self.ast.types.as_record(spec.ty) else { continue };
            let Some(bdef) = self.ast.definition(b) else { continue };
            let Some(mut sub) = self.class_hit(bdef, name, kind, depth + 1) else { continue };
            for p in &mut sub.paths {
                p.specs.insert(0, i);
                p.classes.insert(0, b);
                p.written.insert(0, spec.ty);
                if spec.is_virtual {
                    p.first_virtual = Some(b);
                    p.last_virtual = p.last_virtual.or(Some(b));
                }
            }
            acc = Some(match acc {
                None => sub,
                Some(mut a) => {
                    let firsts = |f: &Found| -> Vec<DeclId> {
                        let mut v: Vec<DeclId> =
                            f.decls.iter().map(|&d| self.ast.first(d)).collect();
                        v.sort_unstable();
                        v
                    };
                    if !(a.mixed || sub.mixed) && firsts(&a.found) == firsts(&sub.found) {
                        a.paths.extend(sub.paths);
                    } else {
                        a.mixed = true;
                        for (j, &d) in sub.found.decls.iter().enumerate() {
                            let f = self.ast.first(d);
                            if !a.found.decls.iter().any(|&o| self.ast.first(o) == f) {
                                a.found.decls.push(d);
                                a.found.found.push(sub.found.found[j]);
                            }
                        }
                        a.paths.extend(sub.paths);
                    }
                    a
                }
            });
        }
        acc
    }

    /// Reports an ambiguous class member lookup ([class.member.lookup]/6)
    /// with clang's diagnostics; true if `found` was ambiguous.
    pub(crate) fn diagnose_member_ambiguity(
        &self,
        found: &Found,
        name: Symbol,
        loc: kiln_source::Loc,
    ) -> bool {
        let Some(amb) = &found.ambiguity else { return false };
        let text = self.ast.symbols().as_str(name).to_owned();
        match amb {
            MemberAmbiguity::Types(decls) => {
                let mut msg =
                    self.diag(kiln_diag::DiagId::sema_ambiguous_member_types, loc).arg(text);
                for &d in decls {
                    let loc = self.ast.decls.get(d).loc();
                    msg = msg
                        .with_note(self.diag(kiln_diag::DiagId::note_ambiguous_member_found, loc));
                }
                self.emit(msg);
            }
            MemberAmbiguity::Subobjects { decl, paths } => {
                let naming = found.naming.unwrap_or(*decl);
                let nty = self.tag_type(naming);
                let base_ty = paths.first().and_then(|p| p.written.last().copied()).unwrap_or(nty);
                let display = crate::conv::bases::ambiguous_paths_string(self, nty, paths);
                let note = self.diag(
                    kiln_diag::DiagId::note_ambiguous_member_found,
                    self.ast.decls.get(*decl).loc(),
                );
                let msg = self
                    .diag(kiln_diag::DiagId::sema_ambiguous_member_subobjects, loc)
                    .arg(text)
                    .arg(self.ty_arg(base_ty))
                    .arg(display)
                    .with_note(note);
                self.emit(msg);
            }
        }
        true
    }

    /// True if namespace `outer` is `inner` or encloses it.
    pub(crate) fn encloses(&self, outer: DeclId, inner: DeclId) -> bool {
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
