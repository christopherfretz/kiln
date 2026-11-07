//! Access control ([class.access]; spec/sema-class.md §2.4; WP-066b): a
//! port of clang's `SemaAccess.cpp` without dependent contexts.
//!
//! A check names the entity (a member, or a base class for a
//! derived-to-base conversion), the naming class ([class.access.base]/5)
//! and, for a non-static member named through an object expression, the
//! object's class ([class.protected]).

use kiln_ast::decl::{Access, DeclKind, FriendKind};
use kiln_ast::{DeclId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use super::AccessCheck;
use crate::Sema;
use crate::conv::bases::{BasePath, base_paths, is_derived_from, same_class};
use crate::lookup::LookupKind;

/// Restrictiveness: public < protected < private < none (clang's
/// `AccessSpecifier` order).
const fn rank(a: Access) -> u8 {
    match a {
        Access::Public => 0,
        Access::Protected => 1,
        Access::Private => 2,
        Access::None => 3,
    }
}

fn max_access(a: Access, b: Access) -> Access {
    if rank(a) >= rank(b) { a } else { b }
}

/// clang `CXXRecordDecl::MergeAccess`: the access of a declaration with
/// access `decl` reached along a path with access `path`.
fn merge_access(path: Access, decl: Access) -> Access {
    if decl == Access::Private { Access::None } else { max_access(path, decl) }
}

/// The classes and functions whose members and friends the current
/// context is (clang `EffectiveContext`), first declarations.
struct EffectiveContext {
    records: Vec<DeclId>,
    functions: Vec<DeclId>,
}

/// What is being accessed (clang `AccessTarget`).
#[derive(Clone, Copy)]
struct Target {
    /// The member (`None` for a base class access).
    member: Option<DeclId>,
    /// The member's access in its declaring class.
    decl_access: Access,
    /// The class declaring the member, or the base class.
    declaring: DeclId,
    /// A non-static data member or member function.
    instance_member: bool,
    /// The class of the object expression for an instance member (the
    /// instance context), unless suppressed.
    object: Option<DeclId>,
}

impl Sema<'_, '_> {
    /// The class that declares member `d` (enumerators of an unscoped
    /// enumeration declared in a class belong to the class).
    pub(crate) fn declaring_class(&self, d: DeclId) -> Option<DeclId> {
        let mut cur = self.ast.decls.get(d).ctx();
        for _ in 0..64 {
            let c = cur?;
            match self.kind_of(c) {
                DeclKind::CXXRecord => return Some(c),
                DeclKind::Enum => cur = self.ast.decls.get(c).ctx(),
                _ => return None,
            }
        }
        None
    }

    fn effective_context(&self) -> EffectiveContext {
        self.effective_context_at(self.access_start())
    }

    /// The context access is checked from: the use's while a
    /// namespace-scope alias template is substituted
    /// ([`Sema::access_ctx`]), else the current one (the class of a
    /// static data member being initialized out of line).
    pub(crate) fn access_start(&self) -> DeclId {
        if let Some(c) = self.access_ctx {
            return c;
        }
        self.initializing.and_then(|v| self.ast.decls.get(v).ctx()).unwrap_or(self.ctx)
    }

    /// The effective context of a declaration in context `start`.
    fn effective_context_at(&self, start: DeclId) -> EffectiveContext {
        let mut ec = EffectiveContext { records: Vec::new(), functions: Vec::new() };
        let mut cur = Some(start);
        for _ in 0..10_000 {
            let Some(c) = cur else { break };
            let n = self.ast.decls.get(c);
            match n.kind() {
                DeclKind::CXXRecord
                | DeclKind::ClassTemplateSpecialization
                | DeclKind::ClassTemplatePartialSpecialization => {
                    ec.records.push(self.ast.first(c));
                    cur = n.ctx();
                }
                k if k.is_function() => {
                    ec.functions.push(self.ast.first(c));
                    cur = match self.friend_decl_of(c) {
                        Some(f) => self.ast.decls.lexical_ctx(f).or(n.ctx()),
                        None => n.ctx(),
                    };
                }
                DeclKind::TranslationUnit | DeclKind::Namespace => break,
                _ => cur = n.ctx(),
            }
        }
        ec
    }

    /// The friend declaration function `f` takes its lexical context
    /// from: `f` itself, or for a specialization of a friend function
    /// template, that template (or its pattern). A friend defined in a
    /// class is in the scope of that class ([class.friend]/7), so its
    /// effective context continues there, into the enclosing classes
    /// (clang's `EffectiveContext` walks `getLexicalDeclContext`).
    fn friend_decl_of(&self, f: DeclId) -> Option<DeclId> {
        let is_friend = |d: DeclId| self.ast.decls.get(d).misc().friend() != FriendKind::None;
        // A specialization is marked friend too, but declared at
        // namespace scope: the template's own declaration is the one in
        // the class.
        if let Some((_, s)) = self.ast.spec_info(f) {
            let t = s.template;
            let from = std::iter::once(t).chain(self.pattern_of(t)).find(|&d| is_friend(d));
            if from.is_some() {
                return from;
            }
        }
        is_friend(f).then_some(f)
    }

    /// True if `a` is `b` or derived from it.
    pub(crate) fn derived_inclusive(&self, a: DeclId, b: DeclId) -> bool {
        same_class(self, a, b) || is_derived_from(self, a, b)
    }

    /// The first declarations befriended by class `r`.
    pub(crate) fn friends_of(&self, r: DeclId) -> Vec<DeclId> {
        let def = self.ast.definition(r).unwrap_or(r);
        if let Some(b) = self.classes.iter().rev().find(|b| same_class(self, b.record, def)) {
            return b.friends.clone();
        }
        self.ast
            .decls
            .cold
            .friends
            .get(def)
            .map(|&l| self.ast.lists.iter::<DeclId>(l).collect())
            .unwrap_or_default()
    }

    /// clang `GetFriendKind`: whether the context is a friend of `class`.
    fn is_friend(&self, ec: &EffectiveContext, class: DeclId) -> bool {
        self.friends_of(class).into_iter().any(|f| {
            let f = self.ast.first(f);
            match self.kind_of(f) {
                // [temp.friend]/1: every specialization of a befriended
                // template is a friend (and the pattern, for uses inside
                // its definition).
                DeclKind::FunctionTemplate => {
                    ec.functions.iter().any(|&g| self.friend_template_of(g) == Some(f))
                }
                DeclKind::ClassTemplate => {
                    ec.records.iter().any(|&r| self.friend_template_of(r) == Some(f))
                }
                // A befriended specialization of the function template
                // whose definition is being parsed may be the context once
                // instantiated: decided then (clang's `MightInstantiateTo`
                // making the check dependent).
                k if k.is_function() => {
                    ec.functions.contains(&f)
                        || self.ast.spec_info(f).is_some_and(|(_, s)| {
                            let t = Some(self.ast.first(s.template));
                            ec.functions.iter().any(|&g| {
                                self.ast.described_template(self.ast.first(g)).is_some()
                                    && self.friend_template_of(g) == t
                            })
                        })
                }
                // Likewise a befriended class template specialization
                // (`friend struct X<int>;`, or `X<T>` once its class is
                // instantiated, [temp.friend]) when the context is the
                // pattern or a partial specialization of that template.
                _ => {
                    if ec.records.iter().any(|&r| same_class(self, r, f)) {
                        return true;
                    }
                    let spec = self.ast.spec_info(f).is_some_and(|(_, s)| {
                        let t = Some(self.ast.first(s.template));
                        ec.records.iter().any(|&r| {
                            (self.ast.described_template(self.ast.first(r)).is_some()
                                || self.ast.partial_spec_info(r).is_some())
                                && self.friend_template_of(r) == t
                        })
                    });
                    if spec {
                        self.speculative_friend.set(true);
                    }
                    spec
                }
            }
        })
    }

    /// The (first declaration of the) template `d` is a specialization,
    /// partial specialization or pattern of.
    fn friend_template_of(&self, d: DeclId) -> Option<DeclId> {
        let t = match self.ast.spec_info(d) {
            Some((_, s)) => Some(s.template),
            None => self
                .ast
                .partial_spec_info(d)
                .map(|(_, p)| p.template)
                .or_else(|| self.ast.described_template(self.ast.first(d))),
        };
        t.map(|t| self.ast.first(t))
    }

    /// clang `GetProtectedFriendKind`: friendship of any class `P` with
    /// `instance` derived from `P` derived from `naming`.
    fn is_protected_friend(&self, ec: &EffectiveContext, instance: DeclId, naming: DeclId) -> bool {
        let mut cands = vec![instance];
        for p in base_paths(self, instance, naming) {
            cands.extend(p.classes.iter().copied());
        }
        cands
            .into_iter()
            .filter(|&p| self.derived_inclusive(p, naming))
            .any(|p| self.is_friend(ec, p))
    }

    /// clang `HasAccess`: whether an entity with access `access` as a
    /// member of `naming` is accessible in `ec`.
    fn has_access(
        &self,
        ec: &EffectiveContext,
        naming: DeclId,
        access: Access,
        t: &Target,
    ) -> bool {
        match access {
            Access::Public => return true,
            Access::None => return false,
            Access::Private | Access::Protected => {}
        }
        // A context class not known to derive from `naming` that has a
        // dependent base may still turn out to (clang's `AR_dependent`
        // from `IsDerivedFromInclusive`): decided in each instantiation.
        let mut dependent = false;
        for &r in &ec.records {
            if access == Access::Private {
                if same_class(self, r, naming) {
                    return true;
                }
                continue;
            }
            if !self.derived_inclusive(r, naming) {
                dependent |= self.has_dependent_bases_in(r, 0);
                continue;
            }
            match t.object {
                None => {
                    if !t.instance_member || same_class(self, naming, r) {
                        return true;
                    }
                }
                Some(ic) => {
                    if self.derived_inclusive(ic, r) {
                        return true;
                    }
                }
            }
        }
        let friend = if access == Access::Protected && t.instance_member {
            match t.object {
                None => self.is_friend(ec, naming),
                Some(ic) => self.is_protected_friend(ec, ic, naming),
            }
        } else {
            self.is_friend(ec, naming)
        };
        if !friend && dependent {
            // [class.protected], [temp.dep.type]: re-checked against the
            // instantiated class ([`Sema::replay_dependent_access`]).
            self.speculative_friend.set(true);
            return true;
        }
        friend
    }

    /// Whether class `c` or a (non-dependent) base of it, transitively,
    /// has a dependent base ([temp.dep.type]; clang's
    /// `IsDerivedFromInclusive` answering `AR_dependent`).
    fn has_dependent_bases_in(&self, c: DeclId, depth: u32) -> bool {
        if depth > 64 {
            return false;
        }
        let Some(def) = self.ast.definition(c) else { return false };
        self.ast.decls.bases(def, &self.ast.lists).any(|b| {
            self.is_dependent_type(b.ty)
                || self
                    .ast
                    .types
                    .as_record(b.ty)
                    .is_some_and(|r| self.has_dependent_bases_in(r, depth + 1))
        })
    }

    /// The class containing the `j`th base-specifier of `p`, a path from
    /// `naming`.
    fn deriving_class(naming: DeclId, p: &BasePath, j: usize) -> DeclId {
        if j == 0 { naming } else { p.classes[j - 1] }
    }

    fn spec_access(&self, deriving: DeclId, spec: u32) -> Access {
        self.base_access(deriving, self.ast.decls.base_spec(spec).access)
    }

    /// clang `FindBestPath`: the most permissive path from `naming` to the
    /// target's declaring class, with its access.
    fn find_best_path(
        &self,
        ec: &EffectiveContext,
        t: &Target,
        naming: DeclId,
        final_access: Access,
    ) -> Option<(Access, BasePath)> {
        let mut best: Option<(Access, BasePath)> = None;
        for p in base_paths(self, naming, t.declaring) {
            let mut tt = *t;
            let mut acc = final_access;
            for j in (0..p.specs.len()).rev() {
                if acc == Access::Private {
                    acc = Access::None;
                    break;
                }
                let deriving = Self::deriving_class(naming, &p, j);
                acc = max_access(acc, self.spec_access(deriving, p.specs[j]));
                if self.has_access(ec, deriving, acc, &tt) {
                    acc = Access::Public;
                    tt.object = None;
                }
            }
            if best.as_ref().is_none_or(|b| rank(acc) < rank(b.0)) {
                let public = acc == Access::Public;
                best = Some((acc, p));
                if public {
                    break;
                }
            }
        }
        best
    }

    /// clang `IsAccessible`.
    fn is_accessible(
        &self,
        ec: &EffectiveContext,
        t: &Target,
        naming: DeclId,
        unprivileged: Access,
    ) -> bool {
        if unprivileged == Access::Public {
            return true;
        }
        if unprivileged != Access::None && self.has_access(ec, naming, unprivileged, t) {
            return true;
        }
        let mut tt = *t;
        let final_access = if t.member.is_some() {
            let mut fa = t.decl_access;
            if self.has_access(ec, t.declaring, fa, &tt) {
                fa = Access::Public;
                tt.object = None;
            }
            if same_class(self, t.declaring, naming) {
                return fa == Access::Public;
            }
            fa
        } else {
            Access::Public
        };
        self.find_best_path(ec, &tt, naming, final_access).is_some_and(|(a, _)| a == Access::Public)
    }

    /// The access of a member with access `decl` of class `declaring` as a
    /// member of `naming` without privileges ([class.access.base]/1; the
    /// most permissive path).
    fn unprivileged_access(&self, naming: DeclId, declaring: DeclId, decl: Access) -> Access {
        if same_class(self, naming, declaring) {
            return decl;
        }
        let mut best = Access::None;
        for p in base_paths(self, naming, declaring) {
            let mut acc = Access::Public;
            for (j, &s) in p.specs.iter().enumerate() {
                let b = self.spec_access(Self::deriving_class(naming, &p, j), s);
                acc = if j == 0 { b } else { merge_access(acc, b) };
            }
            let acc = merge_access(acc, decl);
            if rank(acc) < rank(best) {
                best = acc;
            }
        }
        best
    }

    /// Whether member `found` (the declaration lookup found: a member or
    /// a using-shadow) named in class `naming` is accessible here,
    /// diagnosing it at `loc` if not ([class.access]). `object` is the
    /// class of the object expression of a member access (`None` for a
    /// qualified name without an object, as in `&C::m`).
    pub(crate) fn check_member_access(
        &mut self,
        found: DeclId,
        naming: DeclId,
        object: Option<DeclId>,
        loc: Loc,
    ) -> bool {
        self.check_member_access_at(found, naming, object, loc, false)
    }

    /// [`Sema::check_member_access`] for a check that may share its
    /// location with another one of a different member (a new-expression's
    /// `operator new` and `operator delete`): its error is never folded
    /// into an earlier one at the same location (spec/diag.md §7.4).
    pub(crate) fn check_member_access_distinct(
        &mut self,
        found: DeclId,
        naming: DeclId,
        loc: Loc,
    ) -> bool {
        self.check_member_access_at(found, naming, None, loc, true)
    }

    fn check_member_access_at(
        &mut self,
        found: DeclId,
        naming: DeclId,
        object: Option<DeclId>,
        loc: Loc,
        distinct: bool,
    ) -> bool {
        // A public member named in its own class is accessible; through a
        // base its access is that along the path ([class.access.base]/1).
        let (decl_access, own_class) = match self.injected_class(found, naming) {
            Some(c) => (Access::Public, same_class(self, c, naming)),
            None => (
                self.ast.decls.get(found).misc().access(),
                self.declaring_class(found).is_some_and(|c| same_class(self, c, naming)),
            ),
        };
        if decl_access == Access::None || decl_access == Access::Public && own_class {
            return true;
        }
        self.access_checks = self.access_checks.wrapping_add(1);
        let repeat = distinct
            || self
                .inst
                .stack
                .frames
                .last()
                .is_some_and(|f| f.kind == crate::templ::stack::FrameKind::AliasTemplate);
        let check =
            AccessCheck { member: found, naming_class: naming, object_class: object, loc, repeat };
        // Delayed only while still in the declaration's own context: a
        // class or function body (or lambda) inside the delayed region is
        // its own context (clang's `pushUndelayed`). So is a SFINAE
        // substitution the declaration triggers (deduction for an explicit
        // instantiation, a default template argument): its checks are
        // made now, in the template's context, as substitution failures
        // ([temp.spec.general]/6 exempts only the names written,
        // [temp.deduct.general]/8).
        if !self.inst.stack.is_sfinae()
            && self.delayed_access_depth.last().is_some_and(|&(_, ctx, _)| ctx == self.ctx)
        {
            self.delayed_access.push(check);
            return true;
        }
        self.run_access_check(check)
    }

    /// The access check of a name lookup `found` found by class member
    /// lookup ([class.access]/4: every name, including type names,
    /// nested-name-specifier components and template names), at `loc`.
    /// Nothing for a name not found in a class.
    pub(crate) fn check_found_access(&mut self, found: &crate::lookup::Found, loc: Loc) -> bool {
        let Some(naming) = found.naming else { return true };
        let Some(&via) = found.found.first().or(found.decls.first()) else { return true };
        self.check_member_access(via, naming, None, loc)
    }

    /// The access check of a type or template name `name` that
    /// [`Sema::classify_name`] classified as declaration `d` (after the
    /// nested-name-specifier naming `qualifier`, if any), when the parser
    /// commits to it ([class.access]/4; spec/sema-class.md §2.4). The
    /// lookup is redone only for a class member: it gives the naming
    /// class and the using-shadow the name was found through. A type
    /// name is also a use of its declaration ([dcl.attr.deprecated]/4,
    /// clang's `getTypeName` → `DiagnoseUseOfDecl`).
    pub fn check_type_name_access(
        &mut self,
        name: Symbol,
        qualifier: Option<DeclId>,
        d: DeclId,
        loc: Loc,
    ) {
        self.check_name_access_as(name, qualifier, d, loc, LookupKind::Ordinary);
        self.diagnose_type_name_use(d, loc);
    }

    /// [`Sema::check_type_name_access`] for a name `d` found by a lookup
    /// of kind `kind` (`Tag` for an elaborated-type-specifier).
    pub(crate) fn check_name_access_as(
        &mut self,
        name: Symbol,
        qualifier: Option<DeclId>,
        d: DeclId,
        loc: Loc,
        kind: LookupKind,
    ) {
        // A non-member is checked only as an injected-class-name found
        // in a class scope (through a base).
        if self.declaring_class(d).is_none()
            && (self.kind_of(d) != DeclKind::CXXRecord || !self.in_class_scope(qualifier))
        {
            return;
        }
        let qualifier = qualifier.map(|q| self.type_name_qualifier(q));
        if qualifier.is_some_and(|q| self.is_dependent_ctx(q)) {
            return;
        }
        // Fast path: a member of an enclosing class, named in that class
        // (unqualified or `C::m`), is accessible to its members.
        let owner = self.declaring_class(d).unwrap_or(d);
        if qualifier.is_none_or(|q| same_class(self, q, owner)) && self.encloses_ctx(owner) {
            return;
        }
        let found = match qualifier {
            Some(q) => self.lookup_qualified(q, name, kind),
            None => self.lookup_name(name, kind),
        };
        if found.decls.first().is_some_and(|&x| self.ast.first(x) == self.ast.first(d)) {
            self.check_found_access(&found, loc);
        }
    }

    /// The class whose injected-class-name `found` is, when it was found
    /// by a lookup in (a class derived from) `naming`: a public member of
    /// that class ([class.pre]/2), not of the class enclosing it.
    fn injected_class(&self, found: DeclId, naming: DeclId) -> Option<DeclId> {
        if self.kind_of(found) != DeclKind::CXXRecord {
            return None;
        }
        let def = self.ast.definition(found)?;
        let naming = self.ast.definition(naming).unwrap_or(naming);
        self.derived_inclusive(naming, def).then_some(def)
    }

    /// Whether class `c` is (or encloses) the current context.
    fn encloses_ctx(&self, c: DeclId) -> bool {
        let mut cur = Some(self.ctx);
        for _ in 0..256 {
            let Some(x) = cur else { return false };
            if self.kind_of(x) == DeclKind::CXXRecord && same_class(self, x, c) {
                return true;
            }
            cur = self.ast.decls.get(x).ctx();
        }
        false
    }

    /// Whether a lookup qualified by `qualifier` (or unqualified, from
    /// the current context) searches a class scope.
    fn in_class_scope(&self, qualifier: Option<DeclId>) -> bool {
        if let Some(q) = qualifier {
            return self.kind_of(self.type_name_qualifier(q)) == DeclKind::CXXRecord;
        }
        let mut cur = Some(self.ctx);
        for _ in 0..256 {
            let Some(c) = cur else { return false };
            if self.kind_of(c) == DeclKind::CXXRecord {
                return true;
            }
            cur = self.ast.decls.get(c).ctx();
        }
        false
    }

    fn injected_target(&self, found: DeclId, naming: DeclId) -> Option<Target> {
        let declaring = self.injected_class(found, naming)?;
        Some(Target {
            member: Some(found),
            decl_access: Access::Public,
            declaring,
            instance_member: false,
            object: None,
        })
    }

    /// The access check of the members a member using-declarator names
    /// ([namespace.udecl]/19: each must be accessible), at `loc`. Nothing
    /// outside a class, for a dependent or non-class qualifier, or for
    /// inherited constructors.
    pub fn check_using_declarator_access(
        &mut self,
        qual: Option<crate::using::Qualifier>,
        name: kiln_ast::names::DeclName,
        loc: Loc,
    ) {
        let Some(q) = qual else { return };
        if self.kind_of(self.ctx) != DeclKind::CXXRecord
            || self.kind_of(q.ctx) != DeclKind::CXXRecord
            || self.is_dependent_ctx(q.ctx)
        {
            return;
        }
        let found = self.lookup_qualified_dn(q.ctx, name, LookupKind::All);
        let Some(naming) = found.naming else { return };
        // clang names the members through an object of the current class
        // (`setBaseObjectType` in `BuildUsingDeclaration`), so a derived
        // class may redeclare a protected base member ([class.protected]).
        let object = Some(self.ast.first(self.ctx));
        for (i, &d) in found.decls.iter().enumerate() {
            if self.kind_of(d) == DeclKind::CXXConstructor
                || self.injected_class(d, naming).is_some()
            {
                continue;
            }
            let via = found.found.get(i).copied().unwrap_or(d);
            self.check_member_access(via, naming, object, loc);
        }
    }

    fn member_target(&self, found: DeclId, object: Option<DeclId>) -> Option<Target> {
        let declaring = self.declaring_class(found)?;
        // A member function template is an instance member when its
        // pattern is ([class.protected]/1 applies to its specializations).
        let target = self.templated_or_self(self.ast.strip_shadow(found));
        let instance_member = self.is_instance_member(target);
        Some(Target {
            member: Some(found),
            decl_access: self.ast.decls.get(found).misc().access(),
            declaring,
            instance_member,
            object: object.filter(|_| instance_member),
        })
    }

    /// The entity whose instantiations replay a dependent check made in
    /// `ec` ([`Sema::replay_dependent_access`]): the innermost function or
    /// class, except a non-generic lambda's call operator and closure
    /// type, which are instantiated with their enclosing function
    /// (a generic lambda's call operator is instantiated on its own).
    fn dependent_access_key(&self, ec: &EffectiveContext) -> Option<DeclId> {
        let f = ec
            .functions
            .iter()
            .find(|&&g| !self.is_lambda_call_op(g) || self.ast.described_template(g).is_some());
        let r = || {
            ec.records
                .iter()
                .find(|&&r| self.ast.decls.record(r).is_none_or(|x| x.lambda.is_none()))
        };
        f.or_else(r).map(|&k| self.ast.first(k))
    }

    /// A non-static member other than an explicit object member function
    /// ([dcl.fct]/6): one whose address is a pointer to member.
    pub(crate) fn is_implicit_object_member(&self, d: DeclId) -> bool {
        self.is_instance_member(d) && !self.has_explicit_object(d)
    }

    /// A non-static data member or member function (clang
    /// `isCXXInstanceMember`).
    pub(crate) fn is_instance_member(&self, d: DeclId) -> bool {
        match self.ast.decls.get(d).kind() {
            DeclKind::Field | DeclKind::IndirectField => true,
            k if k.is_function() && k != DeclKind::Function => self
                .ast
                .decls
                .function(d)
                .is_some_and(|f| f.spec.storage() != kiln_ast::decl::FnStorage::Static),
            _ => false,
        }
    }

    fn run_access_check(&mut self, c: AccessCheck) -> bool {
        let t = self.injected_target(c.member, c.naming_class);
        let Some(t) = t.or_else(|| self.member_target(c.member, c.object_class)) else {
            return true;
        };
        let naming = self.ast.definition(c.naming_class).unwrap_or(c.naming_class);
        let ec = self.effective_context();
        let unpriv = self.unprivileged_access(naming, t.declaring, t.decl_access);
        self.speculative_friend.set(false);
        if self.is_accessible(&ec, &t, naming, unpriv) {
            // Accepted in a pattern only because its class may
            // instantiate to a befriended specialization: re-checked in
            // each instantiation of the innermost function or class
            // ([`Sema::replay_dependent_access`]).
            if self.speculative_friend.take()
                && let Some(k) = self.dependent_access_key(&ec)
            {
                self.dependent_access.entry(k).or_default().push(c);
            }
            return true;
        }
        // Not repeated by the replay of a check the instantiation redid.
        let key = self.dependent_access_key(&ec);
        if let Some(k) = key
            && !self.access_failed.insert((c.member, c.loc, k))
            && self.replaying_access
        {
            return false;
        }
        let name = self.decl_name(c.member);
        let msg = self
            .diag(DiagId::sema_access_member, c.loc)
            .arg(name)
            .arg(u8::from(unpriv == Access::Protected))
            .arg(self.ty_arg(self.tag_type(t.declaring)));
        let msg = self.access_path_notes(msg, &ec, &t, naming);
        self.emit(if c.repeat { msg.allow_duplicate() } else { msg });
        false
    }

    /// The access check of a use of special member `m` of class `naming`
    /// from the current context (spec/sema-class.md §4.8; clang
    /// `CheckDestructorAccess` / `CheckConstructorAccess`): `None` if it
    /// is accessible, else the error `mk(protected)` with clang's
    /// access-path notes.
    pub(crate) fn special_member_access(
        &self,
        m: DeclId,
        naming: DeclId,
        mk: impl FnOnce(&Self, bool) -> Diag,
    ) -> Option<Diag> {
        self.special_member_access_on(m, naming, naming, mk)
    }

    /// [`Sema::special_member_access`] with the [class.protected] object
    /// class `object`: the derived class an inherited constructor
    /// constructs ([namespace.udecl]/19; clang `CheckConstructorAccess`
    /// takes the `ConstructorUsingShadowDecl`'s class).
    pub(crate) fn special_member_access_on(
        &self,
        m: DeclId,
        naming: DeclId,
        object: DeclId,
        mk: impl FnOnce(&Self, bool) -> Diag,
    ) -> Option<Diag> {
        let naming = self.ast.definition(naming).unwrap_or(naming);
        let object = self.ast.definition(object).unwrap_or(object);
        let t = self.member_target(m, Some(object))?;
        let ec = self.effective_context();
        let unpriv = self.unprivileged_access(naming, t.declaring, t.decl_access);
        if self.is_accessible(&ec, &t, naming, unpriv) {
            return None;
        }
        let msg = mk(self, unpriv == Access::Protected);
        Some(self.access_path_notes(msg, &ec, &t, naming))
    }

    /// clang `isMemberAccessibleForDeletion` for a defaulted comparison's
    /// subobject comparison: the operator function `found`, a member
    /// named in class `naming` (the subobject's class) through an object
    /// of class `object`, as from the context `from`
    /// ([class.compare.default]/2, spec/sema-class.md §4.12). `None` if
    /// accessible, else `mk(protected)` with the access-path notes.
    pub(crate) fn cmp_member_access(
        &self,
        found: DeclId,
        naming: DeclId,
        object: DeclId,
        from: DeclId,
        mk: impl FnOnce(&Self, bool) -> Diag,
    ) -> Option<Diag> {
        let naming = self.ast.definition(naming).unwrap_or(naming);
        let t = self.member_target(found, Some(self.ast.first(object)))?;
        let ec = self.effective_context_at(from);
        let unpriv = self.unprivileged_access(naming, t.declaring, t.decl_access);
        if self.is_accessible(&ec, &t, naming, unpriv) {
            return None;
        }
        let msg = mk(self, unpriv == Access::Protected);
        Some(self.access_path_notes(msg, &ec, &t, naming))
    }

    /// clang `CheckConstructorAccess` for constructor `m` of a base class
    /// (`base`) or member subobject initialized by the constructor being
    /// defined (the current context): a base class constructor is named
    /// with the derived class as the object class (clang's `ObjectClass`
    /// for `EK_Base`), so a protected one is accessible; a member's with
    /// its own class. `None` if accessible, else `mk(protected)` with the
    /// access-path notes.
    pub(crate) fn subobject_member_access(
        &self,
        m: DeclId,
        base: bool,
        mk: impl FnOnce(&Self, bool) -> Diag,
    ) -> Option<Diag> {
        let declaring = self.declaring_class(m)?;
        let declaring = self.ast.definition(declaring).unwrap_or(declaring);
        let ec = self.effective_context();
        let object = if base { ec.records.first().copied() } else { Some(declaring) };
        let t = self.member_target(m, object)?;
        let unpriv = self.unprivileged_access(declaring, t.declaring, t.decl_access);
        if self.is_accessible(&ec, &t, declaring, unpriv) {
            return None;
        }
        let msg = mk(self, unpriv == Access::Protected);
        Some(self.access_path_notes(msg, &ec, &t, declaring))
    }

    /// Whether special member `m` of the class of a subobject is
    /// accessible from a member of class `from` (clang
    /// `SpecialMemberDeletionInfo::isAccessible`, [class.default.ctor]/2,
    /// [class.copy.ctor]/10): for a base class subobject (`base`) its
    /// members are named through an object of the derived class `from`;
    /// for a member subobject, through an object of its own class.
    pub(crate) fn special_accessible_from(&self, m: DeclId, from: DeclId, base: bool) -> bool {
        let Some(declaring) = self.declaring_class(m) else { return true };
        let declaring = self.ast.definition(declaring).unwrap_or(declaring);
        let object = if base { Some(self.ast.first(from)) } else { Some(declaring) };
        let Some(t) = self.member_target(m, object) else { return true };
        let ec = self.effective_context_at(from);
        let unpriv = self.unprivileged_access(declaring, t.declaring, t.decl_access);
        self.is_accessible(&ec, &t, declaring, unpriv)
    }

    /// Whether static member `found`, named in class `naming`, is
    /// accessible from context `from`, without diagnosing (clang
    /// `CheckAllocationAccess` with diagnostics suppressed, as
    /// `ShouldDeleteSpecialMember` calls `FindDeallocationFunction`).
    pub(crate) fn member_accessible_from(
        &self,
        found: DeclId,
        naming: DeclId,
        from: DeclId,
    ) -> bool {
        let naming = self.ast.definition(naming).unwrap_or(naming);
        let Some(t) = self.member_target(found, None) else { return true };
        let ec = self.effective_context_at(from);
        let unpriv = self.unprivileged_access(naming, t.declaring, t.decl_access);
        self.is_accessible(&ec, &t, naming, unpriv)
    }

    /// Whether a conversion from `derived` to its base class `base` along
    /// `path` is accessible ([conv.ptr]/3, [class.access.base]/4),
    /// diagnosing `cannot cast 'D' to its private base class 'B'` at
    /// `loc` if not. `derived_ty`/`base_ty` are the class types as the
    /// conversion names them.
    pub(crate) fn check_base_access(
        &mut self,
        derived_ty: QualType,
        base_ty: QualType,
        path: &BasePath,
        loc: Loc,
    ) -> bool {
        self.check_base_access_impl(derived_ty, base_ty, path, loc, BaseUse::Conversion)
    }

    /// [`Self::check_base_access`] for a covariant return type
    /// ([class.virtual]/8): `invalid covariant return for virtual
    /// function: 'B' is a private base class of 'D'` (WP-078a).
    pub(crate) fn check_covariant_base_access(
        &mut self,
        derived_ty: QualType,
        base_ty: QualType,
        path: &BasePath,
        loc: Loc,
    ) -> bool {
        self.check_base_access_impl(derived_ty, base_ty, path, loc, BaseUse::Covariant)
    }

    /// [`Self::check_base_access`] for the explicit object parameter of
    /// a lambda with captures ([expr.prim.lambda.closure]/5): `invalid
    /// explicit object parameter type 'D' in lambda with capture; the type
    /// must derive publicly from the lambda`.
    pub(crate) fn check_lambda_object_base_access(
        &mut self,
        derived_ty: QualType,
        base_ty: QualType,
        path: &BasePath,
        loc: Loc,
    ) -> bool {
        self.check_base_access_impl(derived_ty, base_ty, path, loc, BaseUse::LambdaObject)
    }

    fn check_base_access_impl(
        &mut self,
        derived_ty: QualType,
        base_ty: QualType,
        path: &BasePath,
        loc: Loc,
        usage: BaseUse,
    ) -> bool {
        let Some(derived) = self.ast.types.as_record(self.ast.types.canonical(derived_ty)) else {
            return true;
        };
        let Some(&base) = path.classes.last() else { return true };
        let derived = self.ast.definition(derived).unwrap_or(derived);
        let mut acc = Access::Public;
        for (j, &s) in path.specs.iter().enumerate() {
            let b = self.spec_access(Self::deriving_class(derived, path, j), s);
            acc = if j == 0 { b } else { merge_access(acc, b) };
        }
        if acc == Access::Public || (usage == BaseUse::Conversion && self.cast_ignores_base_access)
        {
            return true;
        }
        let t = Target {
            member: None,
            decl_access: Access::Public,
            declaring: base,
            instance_member: false,
            object: None,
        };
        let ec = self.effective_context();
        if self.is_accessible(&ec, &t, derived, acc) {
            return true;
        }
        let msg = match usage {
            BaseUse::Covariant => self
                .diag(DiagId::sema_covariant_return_inaccessible, loc)
                .arg(self.ty_arg(base_ty))
                .arg(u8::from(acc == Access::Protected))
                .arg(self.ty_arg(derived_ty)),
            BaseUse::Conversion => self
                .diag(DiagId::sema_access_base_conv, loc)
                .arg(self.ty_arg(derived_ty))
                .arg(u8::from(acc == Access::Protected))
                .arg(self.ty_arg(base_ty)),
            BaseUse::LambdaObject => self
                .diag(DiagId::sema_explicit_object_lambda_capture_type, loc)
                .arg(self.ty_arg(derived_ty))
                .arg(1u8),
        };
        let msg = self.access_path_notes(msg, &ec, &t, derived);
        self.emit(msg);
        false
    }

    /// clang `DiagnoseAccessPath`: the notes explaining why `t` is
    /// inaccessible as a member of `naming`.
    fn access_path_notes(
        &self,
        msg: Diag,
        ec: &EffectiveContext,
        t: &Target,
        naming: DeclId,
    ) -> Diag {
        let mut tt = *t;
        let mut so_far = Access::Public;
        if let Some(m) = t.member {
            so_far = t.decl_access;
            if self.has_access(ec, t.declaring, so_far, &tt) {
                so_far = Access::Public;
                tt.object = None;
            } else if so_far == Access::Private || same_class(self, t.declaring, naming) {
                return self.direct_access_notes(msg, ec, &tt, m, naming);
            }
        }
        let Some((_, path)) = self.find_best_path(ec, &tt, naming, so_far) else { return msg };
        let mut constraining: Option<usize> = None;
        for j in (0..path.specs.len()).rev() {
            if matches!(so_far, Access::None | Access::Private) {
                break;
            }
            let deriving = Self::deriving_class(naming, &path, j);
            let ba = self.spec_access(deriving, path.specs[j]);
            if rank(ba) > rank(so_far) {
                constraining = Some(j);
                so_far = ba;
            }
            if self.has_access(ec, deriving, so_far, &tt) {
                so_far = Access::Public;
                tt.object = None;
                constraining = None;
            }
            if so_far == Access::Private {
                break;
            }
        }
        let Some(j) = constraining else {
            return match t.member {
                Some(m) => self.direct_access_notes(msg, ec, &tt, m, naming),
                None => msg,
            };
        };
        let spec = *self.ast.decls.base_spec(path.specs[j]);
        let deriving = Self::deriving_class(naming, &path, j);
        let protected = u8::from(self.base_access(deriving, spec.access) == Access::Protected);
        let implicit = u8::from(spec.access == Access::None);
        let note = if t.member.is_some() || j + 1 != path.specs.len() {
            self.diag(DiagId::note_access_constrained_by_path, spec.loc)
                .arg(implicit)
                .arg(protected)
        } else if implicit == 1 {
            self.diag(DiagId::note_access_implicit, spec.loc).arg(protected)
        } else {
            self.diag(DiagId::note_access_declared, spec.loc).arg(protected)
        };
        let msg = msg.with_note(note);
        match t.member {
            Some(m) => {
                let ml = self.ast.decls.get(self.ast.first(m)).loc();
                msg.with_note(self.diag(DiagId::note_access_member_declared, ml))
            }
            None => msg,
        }
    }

    /// clang `diagnoseBadDirectAccess`: `declared private here` (or the
    /// [class.protected] explanation).
    fn direct_access_notes(
        &self,
        msg: Diag,
        ec: &EffectiveContext,
        t: &Target,
        m: DeclId,
        naming: DeclId,
    ) -> Diag {
        let first = self.ast.first(m);
        // clang locates an alias template at its `using` keyword.
        let loc = (self.kind_of(first) == DeclKind::TypeAliasTemplate)
            .then(|| self.ast.template_info(first).and_then(|i| i.pattern))
            .flatten()
            .and_then(|p| self.templ.alias_using_loc.get(&p).copied())
            .unwrap_or_else(|| self.ast.decls.get(first).loc());
        if t.decl_access == Access::Protected
            && let Some(note) = self.protected_note(ec, t, naming, loc)
        {
            return msg.with_note(note);
        }
        let mut implicit = true;
        let child = self.immediate_child(first, t.declaring);
        for c in self.ast.decls.members(t.declaring) {
            if Some(c) == child {
                break;
            }
            if self.ast.decls.get(c).kind() == DeclKind::AccessSpec {
                implicit = false;
                break;
            }
        }
        let protected = u8::from(t.decl_access == Access::Protected);
        let id = if implicit { DiagId::note_access_implicit } else { DiagId::note_access_declared };
        msg.with_note(self.diag(id, loc).arg(protected))
    }

    /// The member of `class` that declares (or is) `d`: an enumerator's
    /// enumeration.
    fn immediate_child(&self, d: DeclId, class: DeclId) -> Option<DeclId> {
        let mut cur = d;
        for _ in 0..64 {
            let ctx = self.ast.decls.lexical_ctx(cur)?;
            if same_class(self, ctx, class) {
                return Some(cur);
            }
            cur = ctx;
        }
        None
    }

    /// clang `TryDiagnoseProtectedAccess`.
    fn protected_note(
        &self,
        ec: &EffectiveContext,
        t: &Target,
        naming: DeclId,
        loc: Loc,
    ) -> Option<Diag> {
        if !t.instance_member {
            return None;
        }
        for &r in &ec.records {
            if !self.derived_inclusive(r, naming) {
                continue;
            }
            match t.object {
                None => {
                    if same_class(self, naming, r) {
                        continue;
                    }
                    return Some(
                        self.diag(DiagId::note_access_protected_noobject, loc)
                            .arg(self.ty_arg(self.tag_type(r))),
                    );
                }
                Some(ic) => {
                    if self.derived_inclusive(ic, r) {
                        continue;
                    }
                    // clang: a special diagnostic for constructors and
                    // destructors (and constructor templates).
                    let m = t.member.map(|m| self.ast.strip_shadow(m));
                    let m = m.map(|m| match self.kind_of(m) {
                        DeclKind::FunctionTemplate => self.pattern_of(m).unwrap_or(m),
                        _ => m,
                    });
                    let k = m.map(|m| self.kind_of(m));
                    if matches!(k, Some(DeclKind::CXXConstructor | DeclKind::CXXDestructor)) {
                        let dtor = k == Some(DeclKind::CXXDestructor);
                        return Some(
                            self.diag(DiagId::note_access_protected_restricted_ctordtor, loc)
                                .arg(u8::from(dtor)),
                        );
                    }
                    return Some(
                        self.diag(DiagId::note_access_protected_object, loc)
                            .arg(self.ty_arg(self.tag_type(r))),
                    );
                }
            }
        }
        None
    }

    /// Re-runs, in `inst` (an instantiated class or member function), the
    /// access checks of its pattern `pattern` that relied on a befriended
    /// specialization (spec/sema-templates.md §4.8; clang
    /// `PerformDependentDiagnostics`).
    pub(crate) fn replay_dependent_access(&mut self, pattern: DeclId, inst: DeclId) {
        let Some(checks) = self.dependent_access.get(&self.ast.first(pattern)).cloned() else {
            return;
        };
        let saved = (self.ctx, self.initializing.take());
        self.ctx = self.ast.definition(inst).unwrap_or(inst);
        self.replaying_access = true;
        let records = self.effective_context().records;
        for mut c in checks {
            // The object class was the pattern's own (`*this` in a member
            // of a class template, the class of a member using-declaration):
            // substituted like clang's `BaseObjectType`, it is the
            // instantiated class ([temp.dep.type], [class.protected]).
            if let Some(o) = c.object_class {
                c.object_class = Some(self.instantiated_record(o, &records).unwrap_or(o));
            }
            self.run_access_check(c);
        }
        self.replaying_access = false;
        (self.ctx, self.initializing) = saved;
    }

    /// The record of `records` (an instantiation's effective context)
    /// that is `o` or was instantiated from `o`, a class template's
    /// pattern or a member class of one.
    fn instantiated_record(&self, o: DeclId, records: &[DeclId]) -> Option<DeclId> {
        let o = self.ast.first(o);
        let tmpl = self.ast.redecls_of(o).find_map(|x| self.ast.described_template(x));
        records.iter().copied().find(|&r| {
            same_class(self, r, o)
                || self.ast.instantiated_from(r).is_some_and(|p| self.ast.first(p) == o)
                || tmpl.is_some_and(|t| {
                    self.ast
                        .spec_info(r)
                        .is_some_and(|(_, i)| self.ast.first(i.template) == self.ast.first(t))
                })
        })
    }

    /// Makes the instantiations of `new` (a generic lambda's call
    /// operator rebuilt from `pattern`) replay `pattern`'s dependent
    /// access checks.
    pub(crate) fn copy_dependent_access(&mut self, pattern: DeclId, new: DeclId) {
        let (p, n) = (self.ast.first(pattern), self.ast.first(new));
        if p == n {
            return;
        }
        if let Some(checks) = self.dependent_access.get(&p).cloned() {
            self.dependent_access.entry(n).or_default().extend(checks);
        }
    }

    /// Opens a delay point: access checks until the matching
    /// [`Sema::end_delayed_access`] wait for the declaration they belong
    /// to ([class.access]/7, spec/sema-class.md §2.4). For the parser,
    /// around a member declaration's decl-specifiers and declarator.
    pub fn begin_delayed_access(&mut self) {
        let exempt = std::mem::take(&mut self.exempt_next_delay);
        self.delayed_access_depth.push((self.delayed_access.len(), self.ctx, exempt));
        self.begin_delayed_deprecation();
    }

    /// Makes the next delay point drop its checks instead of running
    /// them: the names in the decl-specifiers and declarator of an
    /// explicit instantiation or explicit specialization are not access
    /// checked ([temp.spec.general]/6; clang's `SuppressAccessChecks`).
    /// Its function body, initializer, base-clause and
    /// member-specification are checked as usual. `false` withdraws an
    /// unused request.
    pub fn exempt_next_delayed_access(&mut self, exempt: bool) {
        self.exempt_next_delay = exempt;
    }

    /// The number of open delay points, for
    /// [`Sema::end_delayed_access_to`].
    pub fn delayed_access_level(&self) -> usize {
        self.delayed_access_depth.len()
    }

    /// Closes the delay points opened above `level` (none if they were
    /// already closed), running their checks as
    /// [`Sema::end_delayed_access`] does.
    pub fn end_delayed_access_to(&mut self, level: usize, decl: Option<DeclId>) {
        while self.delayed_access_depth.len() > level {
            self.end_delayed_access(decl);
        }
    }

    /// Closes the delay points opened above `level` without running
    /// their checks: `drop` discards them (the template arguments of a
    /// partial or explicit specialization's class-head; [temp.spec.general]/6,
    /// [temp.spec.partial.general]/10); otherwise they join the enclosing
    /// delay point (clang's `redelay`), or run now if there is none.
    pub fn end_delayed_access_to_enclosing(&mut self, level: usize, drop: bool) {
        let mut start = None;
        let popped = self.delayed_access_depth.len().saturating_sub(level);
        while self.delayed_access_depth.len() > level {
            start = self.delayed_access_depth.pop().map(|(s, _, _)| s);
        }
        self.pop_delayed_deprecations(popped, drop);
        let Some(start) = start else { return };
        if drop {
            self.delayed_access.truncate(start.min(self.delayed_access.len()));
        } else if self.delayed_access_depth.is_empty() {
            self.run_delayed_access(start, false, None);
        }
    }

    /// Closes the innermost delay point, running its checks in the
    /// context of `decl` (the declaration they belong to; `None` runs
    /// them in the current context).
    pub fn end_delayed_access(&mut self, decl: Option<DeclId>) {
        let Some((start, _, exempt)) = self.delayed_access_depth.pop() else { return };
        let level = self.depr_points.len().saturating_sub(1);
        self.end_delayed_deprecation_to(level, decl);
        self.run_delayed_access(start, exempt, decl);
    }

    /// Runs (or, `exempt`, drops) the delayed access checks from `start`
    /// on in the context of `decl`.
    fn run_delayed_access(&mut self, start: usize, exempt: bool, decl: Option<DeclId>) {
        let start = start.min(self.delayed_access.len());
        let checks: Vec<AccessCheck> = self.delayed_access.drain(start..).collect();
        if checks.is_empty() || exempt {
            return;
        }
        let saved = self.ctx;
        if let Some(d) = decl {
            self.ctx = self.access_context_of(d);
        }
        for c in checks {
            self.run_access_check(c);
        }
        self.ctx = saved;
    }

    /// The context whose access a declaration's own names are checked in:
    /// a function or class is its own context; a friend declaration is in
    /// the befriending class; anything else its semantic context.
    fn access_context_of(&self, d: DeclId) -> DeclId {
        let n = self.ast.decls.get(d);
        if n.misc().friend() != FriendKind::None {
            return self.ast.decls.lexical_ctx(d).unwrap_or(self.ctx);
        }
        if n.kind() == DeclKind::Friend {
            return n.ctx().unwrap_or(self.ctx);
        }
        if n.kind().is_function() || n.kind() == DeclKind::CXXRecord {
            return d;
        }
        if kiln_ast::templates::is_template_kind(n.kind())
            && let Some(p) = self.pattern_of(d)
        {
            return self.access_context_of(p);
        }
        n.ctx().unwrap_or(self.ctx)
    }

    /// Points `s` at context `from`, recomputing its effective context
    /// only when `from` changed (reflection access queries, WP-478: a
    /// loop over one class's members allocates nothing per call).
    pub(crate) fn set_access_scope(&self, s: &mut AccessScope, from: DeclId) {
        if s.from == Some(from) {
            return;
        }
        let ec = self.effective_context_at(from);
        s.ec.records.clone_from(&ec.records);
        s.ec.functions.clone_from(&ec.functions);
        s.from = Some(from);
    }

    /// Whether member `found` named in class `naming` is accessible from
    /// the context `s` was set to ([`Self::set_access_scope`]; P2996R13
    /// [meta.reflection.access.queries]). A non-static member is checked
    /// as if named through an object of class `naming` ([class.protected],
    /// as GCC 16.1 does).
    pub(crate) fn member_accessible_in(
        &self,
        s: &AccessScope,
        found: DeclId,
        naming: DeclId,
    ) -> bool {
        let naming = self.ast.definition(naming).unwrap_or(naming);
        let Some(t) = self.member_target(found, Some(naming)) else { return true };
        let unpriv = self.unprivileged_access(naming, t.declaring, t.decl_access);
        self.is_accessible(&s.ec, &t, naming, unpriv)
    }

    /// Whether the base class `base` named by base-specifier `spec` of
    /// `derived` is accessible, as a base of `naming` (`derived` or a
    /// class derived from it), from the context `s` was set to
    /// ([class.access.base]/4).
    pub(crate) fn base_accessible_in(
        &self,
        s: &AccessScope,
        naming: DeclId,
        derived: DeclId,
        spec: u32,
        base: DeclId,
    ) -> bool {
        let naming = self.ast.definition(naming).unwrap_or(naming);
        let derived = self.ast.definition(derived).unwrap_or(derived);
        let acc = if same_class(self, naming, derived) {
            self.spec_access(derived, spec)
        } else {
            self.unprivileged_access(naming, base, Access::Public)
        };
        if acc == Access::Public {
            return true;
        }
        let t = Target {
            member: None,
            decl_access: Access::Public,
            declaring: base,
            instance_member: false,
            object: None,
        };
        self.is_accessible(&s.ec, &t, naming, acc)
    }
}

/// A reusable effective context for repeated access checks from one
/// point ([`Sema::set_access_scope`]).
pub(crate) struct AccessScope {
    ec: EffectiveContext,
    from: Option<DeclId>,
}

impl Default for AccessScope {
    fn default() -> Self {
        Self { ec: EffectiveContext { records: Vec::new(), functions: Vec::new() }, from: None }
    }
}

/// What a derived-to-base access check is for (the diagnostic it gives).
#[derive(Copy, Clone, PartialEq, Eq)]
enum BaseUse {
    /// A derived-to-base conversion ([conv.ptr]/3).
    Conversion,
    /// A covariant return type ([class.virtual]/8).
    Covariant,
    /// A lambda's explicit object parameter ([expr.prim.lambda.closure]/5).
    LambdaObject,
}
