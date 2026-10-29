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

use super::AccessCheck;
use crate::Sema;
use crate::conv::bases::{BasePath, base_paths, is_derived_from, same_class};

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
        let start = self.initializing.and_then(|v| self.ast.decls.get(v).ctx()).unwrap_or(self.ctx);
        self.effective_context_at(start)
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
                    cur = if n.misc().friend() == FriendKind::None {
                        n.ctx()
                    } else {
                        self.ast.decls.lexical_ctx(c).or(n.ctx())
                    };
                }
                DeclKind::TranslationUnit | DeclKind::Namespace => break,
                _ => cur = n.ctx(),
            }
        }
        ec
    }

    /// True if `a` is `b` or derived from it.
    pub(crate) fn derived_inclusive(&self, a: DeclId, b: DeclId) -> bool {
        same_class(self, a, b) || is_derived_from(self, a, b)
    }

    /// The first declarations befriended by class `r`.
    fn friends_of(&self, r: DeclId) -> Vec<DeclId> {
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
            if self.ast.decls.get(f).kind().is_function() {
                ec.functions.contains(&f)
            } else {
                ec.records.iter().any(|&r| same_class(self, r, f))
            }
        })
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
        for &r in &ec.records {
            if access == Access::Private {
                if same_class(self, r, naming) {
                    return true;
                }
                continue;
            }
            if !self.derived_inclusive(r, naming) {
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
        if access == Access::Protected && t.instance_member {
            return match t.object {
                None => self.is_friend(ec, naming),
                Some(ic) => self.is_protected_friend(ec, ic, naming),
            };
        }
        self.is_friend(ec, naming)
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
        // A public member named in its own class is accessible; through a
        // base its access is that along the path ([class.access.base]/1).
        let decl_access = self.ast.decls.get(found).misc().access();
        let own_class = self.declaring_class(found).is_some_and(|c| same_class(self, c, naming));
        if decl_access == Access::None || decl_access == Access::Public && own_class {
            return true;
        }
        let check = AccessCheck { member: found, naming_class: naming, object_class: object, loc };
        if !self.delayed_access_depth.is_empty() {
            self.delayed_access.push(check);
            return true;
        }
        self.run_access_check(check)
    }

    fn member_target(&self, found: DeclId, object: Option<DeclId>) -> Option<Target> {
        let declaring = self.declaring_class(found)?;
        let target = self.ast.strip_shadow(found);
        let instance_member = self.is_instance_member(target);
        Some(Target {
            member: Some(found),
            decl_access: self.ast.decls.get(found).misc().access(),
            declaring,
            instance_member,
            object: object.filter(|_| instance_member),
        })
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
        let Some(t) = self.member_target(c.member, c.object_class) else { return true };
        let naming = self.ast.definition(c.naming_class).unwrap_or(c.naming_class);
        let ec = self.effective_context();
        let unpriv = self.unprivileged_access(naming, t.declaring, t.decl_access);
        if self.is_accessible(&ec, &t, naming, unpriv) {
            return true;
        }
        let name = self.decl_name(c.member);
        let msg = self
            .diag(DiagId::sema_access_member, c.loc)
            .arg(name)
            .arg(u8::from(unpriv == Access::Protected))
            .arg(self.ty_arg(self.tag_type(t.declaring)));
        let msg = self.access_path_notes(msg, &ec, &t, naming);
        self.emit(msg);
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
        let naming = self.ast.definition(naming).unwrap_or(naming);
        let t = self.member_target(m, Some(naming))?;
        let ec = self.effective_context();
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
        self.check_base_access_impl(derived_ty, base_ty, path, loc, false)
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
        self.check_base_access_impl(derived_ty, base_ty, path, loc, true)
    }

    fn check_base_access_impl(
        &mut self,
        derived_ty: QualType,
        base_ty: QualType,
        path: &BasePath,
        loc: Loc,
        covariant: bool,
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
        let ec = self.effective_context();
        if self.is_accessible(&ec, &t, derived, acc) {
            return true;
        }
        let msg = if covariant {
            self.diag(DiagId::sema_covariant_return_inaccessible, loc)
                .arg(self.ty_arg(base_ty))
                .arg(u8::from(acc == Access::Protected))
                .arg(self.ty_arg(derived_ty))
        } else {
            self.diag(DiagId::sema_access_base_conv, loc)
                .arg(self.ty_arg(derived_ty))
                .arg(u8::from(acc == Access::Protected))
                .arg(self.ty_arg(base_ty))
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
        let loc = self.ast.decls.get(first).loc();
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
                    return Some(
                        self.diag(DiagId::note_access_protected_object, loc)
                            .arg(self.ty_arg(self.tag_type(r))),
                    );
                }
            }
        }
        None
    }

    /// Opens a delay point: access checks until the matching
    /// [`Sema::end_delayed_access`] wait for the declaration they belong
    /// to ([class.access]/7, spec/sema-class.md §2.4). For the parser,
    /// around a member declaration's decl-specifiers and declarator.
    pub fn begin_delayed_access(&mut self) {
        self.delayed_access_depth.push(self.delayed_access.len());
    }

    /// Closes the innermost delay point, running its checks in the
    /// context of `decl` (the declaration they belong to; `None` runs
    /// them in the current context).
    pub fn end_delayed_access(&mut self, decl: Option<DeclId>) {
        let Some(start) = self.delayed_access_depth.pop() else { return };
        let start = start.min(self.delayed_access.len());
        let checks: Vec<AccessCheck> = self.delayed_access.drain(start..).collect();
        if checks.is_empty() {
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
        if n.kind().is_function() {
            return d;
        }
        n.ctx().unwrap_or(self.ctx)
    }
}
