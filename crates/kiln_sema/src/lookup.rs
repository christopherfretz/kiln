//! Name lookup ([basic.lookup], spec/sema.md §3).
//!
//! Unqualified lookup walks the scope stack innermost-first: function-local
//! scopes search their own `locals`, context scopes (translation unit,
//! namespace, class, enumeration) the AST lookup table of their entity
//! plus the namespaces nominated by `using namespace` in that scope.
//! Qualified lookup searches one context's table.

use kiln_ast::decl::{DeclFlags, DeclKind};
/// Lookup-table entry flags (owned by spec/sema-overload.md §2.3): a hidden
/// friend's entry is `ADL_ONLY`, invisible to ordinary and qualified
/// lookup ([namespace.memdef]/3) but seen by redeclaration lookup and ADL.
pub use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::DeclName;
use kiln_ast::{DeclId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};
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
    /// A type-only lookup ([basic.lookup.general]/4): types and type
    /// templates; namespaces and non-types are skipped (the class-name of
    /// a base-specifier, [class.derived.general]/2).
    Type,
    /// A namespace-name ([basic.lookup.udir]): namespaces and namespace
    /// aliases only, so a type or function of the same name in an inner
    /// scope does not stop the lookup.
    Namespace,
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

/// One declaration set found by the class member search, the path from
/// the searched class to the class it was found in, and the virtual
/// bases on that path.
type Leaf = (Found, BasePath, Vec<DeclId>);

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
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::Enum
            | DeclKind::Typedef
            | DeclKind::TypeAlias
            | DeclKind::TemplateTypeParm
            | DeclKind::UnresolvedUsingTypename
    )
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
        // C17 6.2.3p1: tags are a name space of their own.
        if self.is_c() && is_tag_kind(k) && kind != LookupKind::Tag {
            return false;
        }
        match kind {
            LookupKind::Ordinary | LookupKind::All => true,
            LookupKind::Tag => is_tag_kind(k),
            LookupKind::Nested => {
                is_type_kind(k)
                    || matches!(
                        k,
                        DeclKind::Namespace
                            | DeclKind::NamespaceAlias
                            | DeclKind::ClassTemplate
                            | DeclKind::TypeAliasTemplate
                            | DeclKind::TemplateTemplateParm
                    )
            }
            LookupKind::Type => {
                is_type_kind(k)
                    || matches!(
                        k,
                        DeclKind::ClassTemplate
                            | DeclKind::TypeAliasTemplate
                            | DeclKind::TemplateTemplateParm
                    )
            }
            LookupKind::Namespace => matches!(k, DeclKind::Namespace | DeclKind::NamespaceAlias),
        }
    }

    /// Filters the results of one scope: using-shadows are looked through
    /// ([namespace.udecl]/1); non-tags hide tags ([basic.scope.hiding]/2)
    /// for ordinary lookup; duplicates (the same entity via two paths,
    /// e.g. a declaration and a using-declaration naming it) collapse to
    /// the later path; an unresolved `using_if_exists` target is dropped
    /// when anything else is found.
    fn filter(&self, v: impl IntoIterator<Item = DeclId>, kind: LookupKind) -> Found {
        let mut out = Found::default();
        // The `first` of each kept result, by index (WP-655): one `first`
        // per result instead of one per pair. Inline for the usual few.
        let mut firsts = [self.ast.tu; 8];
        let mut firsts_spill: Vec<DeclId> = Vec::new();
        for raw in v {
            if self.ast.decls.hidden_shadow(raw).is_some() {
                continue;
            }
            let Some(raw) = self.module_visible_redecl(raw) else { continue };
            let d = self.ast.strip_shadow(raw);
            if !self.accept(d, kind) {
                continue;
            }
            let f = self.ast.first(d);
            let n = out.decls.len();
            let first_of = |i: usize| {
                if i < firsts.len() { firsts[i] } else { firsts_spill[i - firsts.len()] }
            };
            if let Some(i) = (0..n).find(|&i| first_of(i) == f) {
                out.decls[i] = d;
                out.found[i] = raw;
            } else {
                if n < firsts.len() {
                    firsts[n] = f;
                } else {
                    firsts_spill.push(f);
                }
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
            // [basic.scope.hiding]/2 as clang applies it
            // (`LookupResult::resolveKind`): a unique tag is hidden by a
            // name declared in the same scope; a tag of another scope (a
            // namespace nominated by a different using-directive) makes
            // the lookup ambiguous instead ([basic.lookup.general]/1).
            let tags: Vec<usize> =
                (0..out.decls.len()).filter(|&i| is_tag_kind(self.kind_of(out.decls[i]))).collect();
            let hide = match tags[..] {
                [t] => {
                    let other = if t == 0 { out.decls.len() - 1 } else { 0 };
                    // clang's `getContextForScopeMatching`: the context
                    // the declaration (or using-declaration) is in, past
                    // transparent contexts.
                    let scope_of = |raw: DeclId| {
                        let mut c = self.ast.decls.get(raw).ctx()?;
                        for _ in 0..64 {
                            if !self.ast.is_transparent(c) {
                                break;
                            }
                            c = self.ast.decls.get(c).ctx()?;
                        }
                        Some(self.ast.first(c))
                    };
                    scope_of(out.found[t]) == scope_of(out.found[other])
                }
                // Distinct tags are of distinct scopes: ambiguous.
                _ => false,
            };
            if hide {
                keep(&mut out, &|d| !is_tag_kind(self.kind_of(d)));
            }
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
        self.lookup_in_dn(ctx, DeclName::ident(name), kind)
    }

    /// [`Sema::lookup_in`] for any declaration name (operator-function-ids,
    /// WP-068b).
    pub(crate) fn lookup_in_dn(&self, ctx: DeclId, name: DeclName, kind: LookupKind) -> Found {
        let r = self.ast.lookup_local(ctx, name);
        self.filter(r.iter_ordinary(), kind)
    }

    /// Qualified lookup of `name` in `ctx` ([namespace.qual],
    /// [class.qual]). Namespaces nominated by using-directives in `ctx`
    /// are searched when `ctx` itself has no result.
    pub fn lookup_qualified(&self, ctx: DeclId, name: Symbol, kind: LookupKind) -> Found {
        self.lookup_qualified_dn(ctx, DeclName::ident(name), kind)
    }

    /// [`Sema::lookup_qualified`] for any declaration name.
    pub(crate) fn lookup_qualified_dn(
        &self,
        ctx: DeclId,
        name: DeclName,
        kind: LookupKind,
    ) -> Found {
        if self.is_dependent_ctx(ctx) {
            // [temp.dep.type]/6: nothing is known about a dependent scope.
            return Found::default();
        }
        if self.kind_of(ctx) == DeclKind::CXXRecord {
            return self.lookup_in_class_dn(ctx, name, kind);
        }
        let found = self.lookup_in_dn(ctx, name, kind);
        if !found.is_empty() || self.ast.lookup.has_no_using_directives() {
            return found;
        }
        // [namespace.qual]/2: when `ctx` (with its inline namespace set)
        // declares nothing acceptable, the namespaces nominated by its
        // using-directives are searched, each in the same way, and the
        // results are united. Every namespace is searched once.
        let mut v = Vec::new();
        let mut seen = vec![self.ast.first(ctx)];
        let mut work: Vec<DeclId> =
            self.ast.lookup.using_directives(self.ast.primary_context(ctx)).to_vec();
        work.reverse();
        while let Some(n) = work.pop() {
            if seen.contains(&n) {
                continue;
            }
            seen.push(n);
            let here: Vec<DeclId> = self.ast.lookup_local(n, name).iter_ordinary().collect();
            if self.filter(here.clone(), kind).is_empty() {
                let dirs = self.ast.lookup.using_directives(n);
                work.extend(dirs.iter().rev().copied());
            } else {
                v.extend(here);
            }
        }
        self.filter(v, kind)
    }

    /// Unqualified lookup of `name` from the current scope
    /// ([basic.lookup.unqual]).
    pub fn lookup_name(&self, name: Symbol, kind: LookupKind) -> Found {
        self.lookup_name_dn(DeclName::ident(name), kind)
    }

    /// [`Sema::lookup_name`] for any declaration name: a name that is not
    /// an identifier (an operator-function-id) is never a parameter,
    /// template parameter or block-scope name here.
    pub(crate) fn lookup_name_dn(&self, dname: DeclName, kind: LookupKind) -> Found {
        self.lookup_name_scoped(dname, kind, false)
    }

    /// The unqualified lookup of an operator function name for an operator
    /// expression ([over.match.oper]/3.2): class scopes are skipped.
    pub(crate) fn lookup_operator_name(&self, dname: DeclName) -> Found {
        self.lookup_name_scoped(dname, LookupKind::Ordinary, true)
    }

    /// The declarations block scope `s` gives for `name` when `raw` is
    /// the last one found: a function is one of an overload set of every
    /// block-scope function and using-declarator target of the name
    /// ([basic.lookup.general]/1, [namespace.udecl]/1).
    fn block_scope_set(
        &self,
        s: &crate::scope::Scope,
        name: Symbol,
        raw: DeclId,
        kind: LookupKind,
    ) -> Vec<DeclId> {
        let fnlike = |d: DeclId| {
            matches!(
                self.kind_of(self.ast.strip_shadow(d)),
                DeclKind::Function | DeclKind::FunctionTemplate
            )
        };
        // [basic.scope.scope]/5 (P2169): every declaration of the scope
        // is found with a name-independent one (an ambiguity unless it is
        // the only one).
        if self.ast.decls.get(raw).misc().name_independent() {
            return s
                .locals
                .iter()
                .filter(|e| e.0 == name && e.2 == Ns::Ordinary)
                .map(|e| e.1)
                .collect();
        }
        // An unscoped enumerator declared after block-scope functions of
        // its name (clang accepts it, WP-929) is found with them: an
        // ambiguity, latest first.
        if kind == LookupKind::Ordinary && self.kind_of(raw) == DeclKind::EnumConstant {
            let mut v: Vec<DeclId> = s
                .locals
                .iter()
                .rev()
                .filter(|e| e.0 == name && e.2 == Ns::Ordinary && (e.1 == raw || fnlike(e.1)))
                .map(|e| e.1)
                .collect();
            v.dedup();
            return v;
        }
        if kind != LookupKind::Ordinary || !fnlike(raw) {
            return vec![raw];
        }
        s.locals
            .iter()
            .filter(|e| e.0 == name && e.2 == Ns::Ordinary && fnlike(e.1))
            .map(|e| e.1)
            .collect()
    }

    /// The declarations of `name` that block scope `s` makes visible to a
    /// lookup of `kind`, or `None` when there are none.
    fn lookup_in_block_scope(
        &self,
        s: &crate::scope::Scope,
        name: Symbol,
        kind: LookupKind,
    ) -> Option<Found> {
        let hit = match kind {
            LookupKind::Tag => s.find(name, Ns::Tag),
            LookupKind::Ordinary => s.find(name, Ns::Ordinary).or_else(|| s.find(name, Ns::Tag)),
            LookupKind::All => {
                let v: Vec<DeclId> = s.locals.iter().filter(|e| e.0 == name).map(|e| e.1).collect();
                let found = self.filter(v, kind);
                return (!found.is_empty()).then_some(found);
            }
            LookupKind::Nested | LookupKind::Type | LookupKind::Namespace => s
                .locals
                .iter()
                .rev()
                .find(|(n, d, _)| *n == name && self.accept(self.ast.strip_shadow(*d), kind))
                .map(|e| e.1),
        };
        let hit = self.prefer_valid_local(s, hit?);
        let found = self.filter(self.block_scope_set(s, name, hit, kind), kind);
        (!found.is_empty()).then_some(found)
    }

    /// clang `LookupResult::resolveKind` ("ignore an invalid declaration
    /// unless it's the only one left"): an invalid block-scope `hit`
    /// gives way to the latest valid declaration of its name in the same
    /// name space of scope `s`. So `R::X x; R::f(x);`, where the unknown
    /// type `R::f` makes `x` an invalid redeclaration, leaves later uses
    /// naming the first `x` (spec/sema.md §3).
    fn prefer_valid_local(&self, s: &crate::scope::Scope, hit: DeclId) -> DeclId {
        if !self.ast.decls.get(hit).has(DeclFlags::INVALID) {
            return hit;
        }
        let Some(&(name, _, ns)) = s.locals.iter().find(|e| e.1 == hit) else { return hit };
        s.locals
            .iter()
            .rev()
            .find(|e| e.0 == name && e.2 == ns && !self.ast.decls.get(e.1).has(DeclFlags::INVALID))
            .map_or(hit, |e| e.1)
    }

    /// A template parameter named `name` in a template parameter scope
    /// outside class scope `i` (of class `entity`) that belongs to a
    /// member of the class rather than to the class's own templates: its
    /// depth is at least the class's template levels. Such scopes are
    /// outside the class scope only for an out-of-line member definition.
    fn own_template_param_outside(
        &self,
        i: usize,
        entity: DeclId,
        name: Symbol,
        kind: LookupKind,
    ) -> Option<DeclId> {
        let mut outer = self.scopes[..i].iter().rev();
        if !outer.clone().any(|s| s.kind == ScopeKind::TemplateParams) {
            return None;
        }
        let levels = self.ctx_template_levels(entity);
        outer.find_map(|s| {
            let d = (s.kind == ScopeKind::TemplateParams)
                .then(|| s.find(name, Ns::Ordinary))
                .flatten()?;
            let depth = self.ast.template_parm(d)?.depth;
            (depth >= levels && self.accept(d, kind)).then_some(d)
        })
    }

    fn lookup_name_scoped(&self, dname: DeclName, kind: LookupKind, skip_records: bool) -> Found {
        let ident = dname.as_ident();
        let name = ident.unwrap_or(Symbol::from_index(0));
        // The namespaces nominated by the using-directives in scope here
        // and the namespace each one's members appear in (computed when
        // the walk first reaches a namespace scope; [namespace.udir]/2).
        let mut udirs: Option<Vec<(DeclId, DeclId)>> = None;
        for (i, s) in self.scopes.iter().enumerate().rev() {
            // [basic.scope.param]: the parameters of a declarator being
            // parsed, before the scope the declarator appears in.
            if kind != LookupKind::Tag
                && ident.is_some()
                && !self.prototype_params.is_empty()
                && let Some(d) = self.prototype_param_named(i + 1, name)
            {
                let found = self.filter(vec![d], kind);
                if !found.is_empty() {
                    return found;
                }
            }
            // [expr.prim.lambda.capture]/6 (P2036): a lambda's
            // init-captures, from its introducer on.
            if kind != LookupKind::Tag
                && ident.is_some()
                && !self.lambdas.is_empty()
                && let v = self.lambda_init_captures_named(i + 1, name)
                && !v.is_empty()
            {
                let found = self.filter(v, kind);
                if !found.is_empty() {
                    return found;
                }
            }
            if s.kind == ScopeKind::TemplateParams {
                // [temp.local]/6: template parameter names.
                if kind != LookupKind::Tag
                    && ident.is_some()
                    && let Some(d) = s.find(name, Ns::Ordinary)
                    && self.accept(d, kind)
                {
                    return self.filter(vec![d], kind);
                }
                continue;
            }
            if s.is_local() {
                if ident.is_some()
                    && let Some(found) = self.lookup_in_block_scope(s, name, kind)
                {
                    return found;
                }
                // An operator or literal-operator name a block-scope
                // using-declarator introduced hides the outer ones
                // ([basic.lookup.unqual]/4, [lex.ext]/5).
                if ident.is_none() && !s.local_names.is_empty() {
                    let v: Vec<DeclId> =
                        s.local_names.iter().filter(|e| e.0 == dname).map(|e| e.1).collect();
                    let found = self.filter(v, kind);
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
                if skip_records {
                    continue;
                }
                // [temp.local]/7: in a member template defined outside its
                // class, the class's members (and its bases') do not hide
                // the member's own template parameters, whose scopes lie
                // outside the re-entered class scope (`template <unsigned
                // size> void P::f() { size; }` with a base `size()`).
                if kind != LookupKind::Tag
                    && ident.is_some()
                    && let Some(d) = self.own_template_param_outside(i, entity, name, kind)
                {
                    return self.filter(vec![d], kind);
                }
                let found = self.lookup_in_class_unqual(entity, dname, kind);
                if !found.is_empty() || found.ambiguity.is_some() {
                    return found;
                }
                continue;
            }
            // [dcl.enum]/11: an unscoped enumeration's enumerators are
            // declared in the enclosing scope, where the lookup finds them
            // together with whatever else that scope declares (an overload
            // set of the same name makes `enum { f, g = f }` ambiguous;
            // WP-929).
            if s.kind == ScopeKind::Enum && !self.is_scoped_enum(entity) {
                continue;
            }
            let local = self.ast.lookup_local(entity, dname);
            if !matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit) {
                let own = self.filter(local.iter_ordinary(), kind);
                if !own.is_empty() {
                    return own;
                }
                continue;
            }
            // The nominated namespaces' results, after the namespace's own.
            let mut nominated: Vec<DeclId> = Vec::new();
            // [namespace.udir]/2: the members of a nominated namespace
            // appear as if declared in the nearest enclosing namespace of
            // both the directive's scope and the nominated namespace; the
            // first namespace scope out from there that encloses it when
            // that namespace has no open scope.
            let dirs = udirs.get_or_insert_with(|| self.visible_using_directives());
            let mut j = 0;
            while j < dirs.len() {
                if self.encloses(entity, dirs[j].1) {
                    let (n, _) = dirs.remove(j);
                    nominated.extend(self.ast.lookup_local(n, dname).iter_ordinary());
                } else {
                    j += 1;
                }
            }
            // United with the namespace's own declarations (clang's
            // `CppNamespaceLookup`); nothing found filters to nothing.
            let found = self.filter(local.iter_ordinary().chain(nominated), kind);
            if !found.is_empty() {
                return found;
            }
        }
        Found::default()
    }

    /// The namespaces nominated by the using-directives visible from the
    /// current scope, transitively ([namespace.udir]/4), each with the
    /// namespace its members appear in for unqualified lookup: the
    /// nearest enclosing namespace of the nominated namespace and of the
    /// innermost namespace containing the directive's scope (clang's
    /// `UnqualUsingDirectiveSet`). A block-scope directive counts as in
    /// the innermost enclosing namespace of the lookup. Each namespace
    /// appears once.
    fn visible_using_directives(&self) -> Vec<(DeclId, DeclId)> {
        let mut out: Vec<(DeclId, DeclId)> = Vec::new();
        let tables = &self.ast.lookup;
        let has_local = self.scopes.iter().any(|s| s.is_local() && !s.using_dirs.is_empty());
        if tables.has_no_using_directives() && !has_local {
            return out;
        }
        let innermost_ns = self
            .scopes
            .iter()
            .rev()
            .filter(|s| matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit))
            .find_map(|s| s.entity)
            .unwrap_or(self.ast.tu);
        let mut work: Vec<(DeclId, DeclId)> = Vec::new();
        for s in self.scopes.iter().rev() {
            if s.is_local() {
                work.extend(s.using_dirs.iter().rev().map(|&n| (n, innermost_ns)));
            } else if matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit)
                && let Some(e) = s.entity
            {
                let dirs = tables.using_directives(self.ast.primary_context(e));
                work.extend(dirs.iter().rev().map(|&n| (n, e)));
            }
            while let Some((n, eff)) = work.pop() {
                let n = self.ast.first(n);
                if out.iter().any(|&(m, _)| m == n) {
                    continue;
                }
                out.push((n, self.common_namespace(n, eff)));
                let next = tables.using_directives(n);
                work.extend(next.iter().rev().map(|&m| (m, eff)));
            }
        }
        out
    }

    /// The nearest namespace (or the translation unit) that encloses
    /// both namespace `a` and namespace `b` (each encloses itself).
    fn common_namespace(&self, a: DeclId, b: DeclId) -> DeclId {
        let mut cur = Some(b);
        for _ in 0..10_000 {
            let Some(c) = cur else { break };
            if matches!(self.kind_of(c), DeclKind::Namespace | DeclKind::TranslationUnit)
                && self.encloses(c, a)
            {
                return self.ast.first(c);
            }
            cur = self.ast.decls.get(c).ctx();
        }
        self.ast.tu
    }

    /// The order in which clang lists the declarations of an ambiguous
    /// lookup result `decls` (in lookup-table order). An unqualified
    /// lookup that ends at a namespace scope first finds the names
    /// declared in that open scope instance, most recent first (clang's
    /// `IdentifierResolver` chain), then the namespace's lookup table;
    /// duplicates are then removed by moving the last entry into the
    /// duplicate's place (`LookupResult::resolveKind`).
    /// Whether the current context is written inside the definition of
    /// class `c` (its lexical parents reach `c`): an out-of-line member
    /// definition is not.
    fn lexically_within(&self, c: DeclId) -> bool {
        let mut cur = Some(self.ctx);
        let mut n = 0;
        while let Some(x) = cur
            && n < 1024
        {
            if self.ast.first(x) == c {
                return true;
            }
            cur = self.ast.decls.lexical_ctx(x).or_else(|| self.ast.decls.get(x).ctx());
            n += 1;
        }
        false
    }

    pub(crate) fn ambiguity_order(&self, decls: &[DeclId], unqualified: bool) -> Vec<DeclId> {
        if !unqualified {
            return decls.to_vec();
        }
        // An unscoped enumerator belongs to the scope enclosing its
        // enumeration ([dcl.enum]/11), where clang lists it among the
        // scope's own declarations, latest first (WP-929).
        let ctx_of = |d: DeclId| {
            let mut c = self.ast.decls.get(d).ctx()?;
            if self.kind_of(c) == DeclKind::Enum && !self.is_scoped_enum(c) {
                c = self.ast.decls.get(c).ctx()?;
            }
            Some(self.ast.first(self.ast.redecl_context(c)))
        };
        // Members of one class looked up from inside its definition
        // (clang's scope chain, not the class's lookup table): latest
        // first, as the namespace-scope ones below. Not in an out-of-line
        // member (`int S::g() { ... }`), which looks into the class.
        if let Some(c) = decls.first().and_then(|&d| ctx_of(d))
            && self.kind_of(c) == DeclKind::CXXRecord
            && decls.iter().all(|&d| ctx_of(d) == Some(c))
            && self.lexically_within(c)
        {
            let mut v = decls.to_vec();
            v.sort_by_key(|d| std::cmp::Reverse(d.index()));
            return v;
        }
        let Some(scope) = self.scopes.iter().rev().find(|s| {
            matches!(s.kind, ScopeKind::Namespace | ScopeKind::TranslationUnit)
                && s.entity.is_some_and(|e| {
                    let e = self.ast.first(e);
                    decls.iter().any(|&d| ctx_of(d) == Some(e))
                })
        }) else {
            return decls.to_vec();
        };
        let entity = scope.entity.map(|e| self.ast.first(e));
        let mut in_scope: Vec<DeclId> = decls
            .iter()
            .copied()
            .filter(|&d| ctx_of(d) == entity && d.index() >= scope.first_decl)
            .collect();
        if in_scope.is_empty() {
            return decls.to_vec();
        }
        in_scope.sort_by_key(|d| std::cmp::Reverse(d.index()));
        let mut v = in_scope;
        v.extend_from_slice(decls);
        let mut i = 0;
        while i < v.len() {
            if v[..i].contains(&v[i]) {
                let last = v.pop().expect("non-empty");
                if i < v.len() {
                    v[i] = last;
                }
                continue;
            }
            i += 1;
        }
        v
    }

    /// Class member lookup of `name` in class `c` ([class.member.lookup]):
    /// the class's own members (its injected-class-name included,
    /// [class.pre]/2), else the members found in its direct bases,
    /// searched recursively. A name found in a class hides the same name
    /// in that class's bases. Different declaration sets found through
    /// different bases, or one non-static member found in distinct base
    /// subobjects, make the lookup ambiguous ([`Found::ambiguity`]); a
    /// static member, type or enumerator found in several subobjects of
    /// the same type is not ([class.member.lookup]/6).
    /// A declaration found in a class dominates the declarations found in
    /// that class's virtual bases (WP-078a).
    pub fn lookup_in_class(&self, c: DeclId, name: Symbol, kind: LookupKind) -> Found {
        self.lookup_in_class_dn(c, DeclName::ident(name), kind)
    }

    /// [`Sema::lookup_in_class`] for any declaration name.
    pub(crate) fn lookup_in_class_dn(&self, c: DeclId, name: DeclName, kind: LookupKind) -> Found {
        self.lookup_in_class_with(c, name, kind, false)
    }

    /// The class-scope step of unqualified lookup ([basic.lookup.unqual]/2,
    /// [class.member.lookup]): as [`Sema::lookup_in_class_dn`], except that
    /// in a template definition a dependent base class is not searched
    /// ([temp.dep.general]/3): a base declared in a dependent context (a
    /// local class of a function template, a member class of a class
    /// template) that is not the current instantiation here (clang
    /// `lookupInBases` skips such a base). The names it declares are
    /// found by a qualified name (`this->m`, `A::m`) only
    /// (spec/sema-templates.md §3.1.1, WP-1084).
    fn lookup_in_class_unqual(&self, c: DeclId, name: DeclName, kind: LookupKind) -> Found {
        self.lookup_in_class_with(c, name, kind, self.inst.stack.is_empty())
    }

    /// Whether base class `b` is a dependent base here: a class declared
    /// in a dependent context ([temp.dep.type]/9) that is not the current
    /// instantiation, i.e. the context of the lookup is not nested in it
    /// ([temp.dep.type]/1, clang `isCurrentInstantiation`).
    fn is_dependent_base(&self, b: DeclId) -> bool {
        self.ast.decls.get(b).has(DeclFlags::DEPENDENT_CTX)
            && self.kind_of(b) == DeclKind::CXXRecord
            && !self.encloses(b, self.ctx)
    }

    fn lookup_in_class_with(
        &self,
        c: DeclId,
        name: DeclName,
        kind: LookupKind,
        skip_dependent_bases: bool,
    ) -> Found {
        let def = self.ast.definition(c).unwrap_or(c);
        let Some(hit) = self.class_hit(def, name, kind, skip_dependent_bases) else {
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

    /// The search of `def` and its bases: the class's own members, else
    /// the merged results of its direct bases, with virtual base members
    /// a found class dominates removed ([class.member.lookup]/6).
    fn class_hit(
        &self,
        def: DeclId,
        name: DeclName,
        kind: LookupKind,
        skip_dependent_bases: bool,
    ) -> Option<ClassHit> {
        let mut st = (FxHashMap::default(), false);
        let mut leaves = self.class_leaves(def, name, kind, 0, skip_dependent_bases, &mut st);
        self.remove_dominated(&mut leaves);
        let mut it = leaves.into_iter();
        let (found, path, _) = it.next()?;
        let mut acc = ClassHit { found, paths: vec![path], mixed: false };
        // [class.member.lookup]/3: a type declaration stands for the type
        // it designates (clang `HasSameDeclarations`), so typedefs of one
        // type found in different bases name the same member.
        let keys = |f: &Found| -> Vec<Result<QualType, DeclId>> {
            let mut v = Vec::new();
            for &d in &f.decls {
                let k = self.designated_type(self.ast.strip_shadow(d)).ok_or(self.ast.first(d));
                if !v.contains(&k) {
                    v.push(k);
                }
            }
            v
        };
        let same = |a: &Found, b: &Found| {
            let (x, y) = (keys(a), keys(b));
            x.len() == y.len() && x.iter().all(|k| y.contains(k))
        };
        for (sub, path, _) in it {
            if acc.mixed || !same(&acc.found, &sub) {
                acc.mixed = true;
                for (j, &d) in sub.decls.iter().enumerate() {
                    let f = self.ast.first(d);
                    if !acc.found.decls.iter().any(|&o| self.ast.first(o) == f) {
                        acc.found.decls.push(d);
                        acc.found.found.push(sub.found[j]);
                    }
                }
            }
            acc.paths.push(path);
        }
        Some(acc)
    }

    /// Every declaration set the search of `def` finds, with the path
    /// from `def` to the class it is found in (in base-specifier order).
    /// `barren` holds the classes whose search found nothing: a class
    /// reached again through another path (a virtual lattice) is not
    /// searched again, so a name declared nowhere costs one walk per class,
    /// not one per path (WP-400). A class whose search the depth limit
    /// cut is recorded with the depth it was searched at, and searched
    /// again only from a shallower one (WP-414: without that, a lattice
    /// deeper than the limit was searched once per path again).
    fn class_leaves(
        &self,
        def: DeclId,
        name: DeclName,
        kind: LookupKind,
        depth: usize,
        skip_dependent_bases: bool,
        barren: &mut (FxHashMap<DeclId, usize>, bool),
    ) -> Vec<Leaf> {
        if depth > 256 {
            barren.1 = true;
            return Vec::new();
        }
        if barren.0.get(&def).is_some_and(|&d| d <= depth) {
            return Vec::new();
        }
        let mut own = self.filter(self.ast.lookup_local(def, name).iter_ordinary(), kind);
        if own.is_empty()
            && self.ast.decls.get(def).name() == name
            && !matches!(kind, LookupKind::All | LookupKind::Namespace)
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
            return vec![(own, empty, Vec::new())];
        }
        let Some(rec) = self.ast.decls.record(def) else { return Vec::new() };
        let idx: Vec<u32> = self.ast.lists.get(rec.bases).to_vec();
        let specs: Vec<kiln_ast::decl::BaseSpec> =
            self.ast.decls.bases(def, &self.ast.lists).copied().collect();
        let cut_above = std::mem::replace(&mut barren.1, false);
        let mut out = Vec::new();
        for (&i, spec) in idx.iter().zip(specs) {
            let Some(b) = self.ast.types.as_record(spec.ty) else { continue };
            if skip_dependent_bases && self.is_dependent_base(b) {
                continue;
            }
            let Some(bdef) = self.ast.definition(b) else { continue };
            for (f, mut p, mut vs) in
                self.class_leaves(bdef, name, kind, depth + 1, skip_dependent_bases, barren)
            {
                p.specs.insert(0, i);
                p.classes.insert(0, b);
                p.written.insert(0, spec.ty);
                if spec.is_virtual {
                    p.first_virtual = Some(b);
                    p.last_virtual = p.last_virtual.or(Some(b));
                    vs.push(b);
                }
                out.push((f, p, vs));
                if out.len() > 4096 {
                    barren.1 |= cut_above;
                    return out;
                }
            }
        }
        if out.is_empty() {
            // Barren at any depth unless the limit cut it.
            barren.0.insert(def, if barren.1 { depth } else { 0 });
        }
        barren.1 |= cut_above;
        out
    }

    /// [class.member.lookup]/6 dominance (clang `lookupInBases`): a
    /// result found through a virtual base `V` is hidden when another
    /// result is found in a class that has `V` as a virtual base.
    fn remove_dominated(&self, leaves: &mut Vec<Leaf>) {
        if leaves.len() < 2 || leaves.iter().all(|l| l.2.is_empty()) {
            return;
        }
        let ends: Vec<DeclId> = leaves.iter().filter_map(|l| l.1.classes.last().copied()).collect();
        let keep: Vec<bool> = leaves
            .iter()
            .map(|l| !l.2.iter().any(|&v| ends.iter().any(|&h| self.has_virtual_base(h, v))))
            .collect();
        let mut k = keep.into_iter();
        leaves.retain(|_| k.next().unwrap_or(true));
    }

    /// True if `class` has `v` as a (direct or indirect) virtual base
    /// (clang `isVirtuallyDerivedFrom`).
    /// Each class is walked once (WP-400).
    fn has_virtual_base(&self, class: DeclId, v: DeclId) -> bool {
        let v = self.ast.first(v);
        let mut work = vec![class];
        let mut walked: FxHashSet<DeclId> = FxHashSet::default();
        while let Some(c) = work.pop() {
            let Some(def) = self.ast.definition(c) else { continue };
            if !walked.insert(def) || walked.len() > 4096 {
                continue;
            }
            for s in self.ast.decls.bases(def, &self.ast.lists) {
                let Some(b) = self.ast.types.as_record(s.ty) else { continue };
                if s.is_virtual && self.ast.first(b) == v {
                    return true;
                }
                work.push(b);
            }
        }
        false
    }

    /// Reports an ambiguous class member lookup ([class.member.lookup]/6)
    /// with clang's diagnostics; true if `found` was ambiguous.
    pub(crate) fn diagnose_member_ambiguity(
        &self,
        found: &Found,
        name: DeclName,
        loc: kiln_source::Loc,
    ) -> bool {
        let Some(amb) = &found.ambiguity else { return false };
        let text = self.decl_name_str(name);
        match amb {
            MemberAmbiguity::Types(decls) => {
                let mut msg =
                    self.diag(kiln_diag::DiagId::sema_ambiguous_member_types, loc).arg(text);
                for &d in decls {
                    // A type is printed (clang: a typedef-name's
                    // underlying type, else the declared type).
                    let loc = self.ast.decls.get(d).loc();
                    let k = self.kind_of(d);
                    let note = if matches!(k, DeclKind::Typedef | DeclKind::TypeAlias) {
                        let ty = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
                        let ty = match self.ast.types.view(ty.ty()) {
                            kiln_ast::types::TypeView::Typedef { underlying, .. } => underlying,
                            _ => ty,
                        };
                        self.diag(kiln_diag::DiagId::note_ambiguous_member_type_found, loc)
                            .arg(self.ty_arg(ty))
                    } else if is_type_kind(k) {
                        // clang prints the class's type with its scope
                        // (`getTypeDeclType`), without an `aka`.
                        let names = crate::names::sema_names(self);
                        let ty = kiln_ast::types::print_type(
                            &self.ast.types,
                            &names,
                            self.tag_type(d),
                            kiln_ast::types::PrintOptions::CANONICAL,
                        );
                        self.diag(kiln_diag::DiagId::note_ambiguous_member_type_found, loc)
                            .arg(format!("'{ty}'"))
                    } else {
                        self.diag(kiln_diag::DiagId::note_ambiguous_member_found, loc)
                    };
                    msg = msg.with_note(note);
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
        self.current_scope_raw_dn(DeclName::ident(name))
    }

    /// [`Sema::current_scope_raw`] for any declaration name (block scopes
    /// hold identifiers only). The members of a namespace's inline
    /// namespaces are in other scopes and are not included
    /// ([basic.scope.scope]/4, [namespace.def.general]/6; clang's
    /// `FilterLookupForScope` without `AllowInlineNamespace`).
    pub(crate) fn current_scope_raw_dn(&self, dname: DeclName) -> Vec<DeclId> {
        self.current_scope_raw_impl(dname, false)
    }

    /// [`Sema::current_scope_raw_dn`] including the members of inline
    /// namespaces: what an explicit specialization may name
    /// ([temp.expl.spec]/3, [namespace.memdef]/2).
    pub(crate) fn current_scope_with_inline_dn(&self, dname: DeclName) -> Vec<DeclId> {
        self.current_scope_raw_impl(dname, true)
    }

    fn current_scope_raw_impl(&self, dname: DeclName, with_inline: bool) -> Vec<DeclId> {
        let Some(i) = self.scopes.iter().rposition(|s| !s.is_transparent()) else {
            return Vec::new();
        };
        let s = &self.scopes[i];
        if s.is_local() {
            let Some(name) = dname.as_ident() else {
                return s.local_names.iter().filter(|e| e.0 == dname).map(|e| e.1).collect();
            };
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
            Some(e) => self
                .ast
                .lookup_local(e, dname)
                .iter_with_inline()
                .filter(|&(d, via_inline)| {
                    (with_inline || !via_inline) && self.module_redecl_candidate(d)
                })
                .map(|(d, _)| d)
                .collect(),
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
        // Every local variable may take the return slot (clang
        // `Scope::AddDecl` fills `ReturnSlots`; parameters do not).
        let var = self.kind_of(d) == DeclKind::Var;
        if let Some(s) = self.scopes.iter_mut().rev().find(|s| s.is_local()) {
            s.locals.push((name, d, ns));
            if var {
                s.nrvo.slots.push(d);
            }
        }
    }

    /// Adds a block-scope declaration of operator function `d` to the
    /// innermost local scope ([`crate::scope::Scope::local_ops`]).
    pub(crate) fn push_local_operator(
        &mut self,
        op: kiln_ast::names::OverloadedOperator,
        d: DeclId,
    ) {
        if self.in_local_scope()
            && let Some(s) = self.scopes.iter_mut().rev().find(|s| s.is_local())
        {
            s.local_ops.push((op, d));
        }
    }

    /// Adds the `UsingShadow` `d` of an operator or literal-operator name
    /// to the innermost local scope ([`crate::scope::Scope::local_names`]).
    pub(crate) fn push_local_name(&mut self, name: DeclName, d: DeclId) {
        if let Some(s) = self.scopes.iter_mut().rev().find(|s| s.is_local()) {
            s.local_names.push((name, d));
        }
    }

    /// True while the innermost non-linkage scope is function-local.
    pub fn in_local_scope(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .find(|s| !s.is_transparent() && s.kind != ScopeKind::Enum)
            .is_some_and(super::scope::Scope::is_local)
    }
}
