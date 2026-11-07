//! The member and layout queries ([meta.reflection.member.queries],
//! [meta.reflection.layout]; spec/reflection-members.md; WP-478).
//!
//! Each class, enumeration and namespace has one cached list of its
//! members ([`kiln_oven::MemberCache`]); a query copies the entries its
//! filter and access context accept into the Oven's result buffer, so a
//! repeated query allocates nothing but the result vector.

use kiln_ast::decl::{Access, DeclFlags, DeclKind, TemplLinkKind};
use kiln_ast::reflect::Reflectee;
use kiln_ast::{DeclId, QualType, ReflectionId};
use kiln_oven::{
    CachedList, MemberCache, MemberEntry, MemberList, MemberOp, MetaCtx, MetaError, MetaOut, Oven,
    filter_members, info_arg, member_kind as mk,
};
use kiln_source::Loc;

use super::access::{Checked, access_ctx_arg};
use super::fail;
use crate::Sema;
use crate::class::{AccessScope, SpecialKind};
use crate::conv::bases::{is_derived_from, same_class};

/// The member queries' state: the cache and the access scope last
/// checked from.
#[derive(Default)]
pub(crate) struct MemberState {
    pub(crate) cache: MemberCache,
    pub(crate) scope: AccessScope,
}

/// The implicitly declared special members in the order `members_of`
/// lists them after the declared members (GCC 16.1's).
const SPECIAL_ORDER: [SpecialKind; 6] = [
    SpecialKind::DefaultCtor,
    SpecialKind::CopyCtor,
    SpecialKind::CopyAssign,
    SpecialKind::MoveCtor,
    SpecialKind::MoveAssign,
    SpecialKind::Dtor,
];

/// The declarations [meta.reflection.member.queries] never lists: no
/// entity (an access or using declaration, a `static_assert`, a friend),
/// or one a query reaches another way (an enumerator, a parameter).
/// A block of declarations inside a namespace definition whose members
/// are the namespace's: a linkage specification or an export block.
fn is_nested_block(k: DeclKind) -> bool {
    matches!(k, DeclKind::LinkageSpec | DeclKind::Export)
}

fn never_a_member(k: DeclKind) -> bool {
    use DeclKind as K;
    matches!(
        k,
        K::AccessSpec
            | K::StaticAssert
            | K::Friend
            | K::FriendTemplate
            | K::IndirectField
            | K::UsingDirective
            | K::Using
            | K::UsingEnum
            | K::UsingShadow
            | K::ConstructorUsingShadow
            | K::UsingPack
            | K::UnresolvedUsingValue
            | K::UnresolvedUsingTypename
            | K::UnresolvedUsingIfExists
            | K::Empty
            | K::FileScopeAsm
            | K::ClassTemplateSpecialization
            | K::ClassTemplatePartialSpecialization
            | K::VarTemplateSpecialization
            | K::VarTemplatePartialSpecialization
            | K::CXXDeductionGuide
            | K::EnumConstant
            | K::ParmVar
            | K::ImplicitParam
            | K::Binding
    )
}

impl std::fmt::Debug for MemberState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberState").field("cache", &self.cache).finish_non_exhaustive()
    }
}

impl Sema<'_, '_> {
    pub(super) fn meta_member(
        &mut self,
        m: MemberOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        let r = info_arg(args, 0)?;
        *out = match m {
            MemberOp::MembersOf => {
                let actx = access_ctx_arg(args, 1)?;
                MetaOut::Infos(self.members_of(r, actx, ctx, oven)?)
            }
            MemberOp::BasesOf
            | MemberOp::StaticDataMembersOf
            | MemberOp::NonstaticDataMembersOf
            | MemberOp::SubobjectsOf => {
                let actx = access_ctx_arg(args, 1)?;
                let Some(def) = self.complete_class_of(r, ctx, oven)? else {
                    return fail(loc, "not a complete class type");
                };
                let chk = self.checked(actx, def, ctx);
                let n = self.class_lists(def, m, &chk, oven, loc)?;
                MetaOut::Infos(n)
            }
            MemberOp::EnumeratorsOf => MetaOut::Infos(self.enumerators_of(r, ctx, oven)?),
            MemberOp::ParametersOf => MetaOut::Infos(self.parameters_of(r, loc, oven)?),
            MemberOp::OffsetOf => {
                let bits = self.offset_bits(r, loc)?;
                let (bytes, rem) = (bits / 8, bits % 8);
                MetaOut::Pair(u128::from(bytes), u128::from(rem))
            }
            MemberOp::SizeOf => MetaOut::Scalar(u128::from(self.layout_size(r, ctx, oven)?)),
            MemberOp::AlignmentOf => MetaOut::Scalar(u128::from(self.layout_align(r, ctx, oven)?)),
            MemberOp::BitSizeOf => MetaOut::Scalar(u128::from(self.layout_bit_size(r, ctx, oven)?)),
        };
        Ok(())
    }

    /// Whether a metafunction may instantiate a template here: the call is
    /// manifestly constant-evaluated and not a trial (a fold, or
    /// `__builtin_constant_p`'s operand; spec/consteval-blocks.md §3).
    /// Otherwise a query that would instantiate fails silently.
    pub(super) fn meta_may_instantiate(&self, ctx: &MetaCtx) -> bool {
        !ctx.trial && self.inst.eval_loc.is_some()
    }

    /// The class type `r` reflects (through aliases), completed
    /// (instantiated if it is an implicitly instantiable specialization);
    /// `None` if it is not a complete class type. A speculative
    /// evaluation does not instantiate (it fails silently).
    pub(super) fn complete_class_of(
        &mut self,
        r: ReflectionId,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<Option<DeclId>, MetaError> {
        let loc = ctx.loc;
        let Reflectee::Type(t) = self.ast.reflections.get(r) else { return Ok(None) };
        let Some(d) = self.ast.types.as_record(t) else { return Ok(None) };
        if self.undefined_spec(t).is_some() {
            if !self.meta_may_instantiate(ctx) {
                return Err(MetaError::silent());
            }
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        Ok(self.ast.definition(d).filter(|_| self.is_complete(t)))
    }

    /// `members_of(r, ctx)`: a complete class's members (its implicitly
    /// declarable special members declared first) or a namespace's
    /// members declared so far.
    fn members_of(
        &mut self,
        r: ReflectionId,
        actx: (ReflectionId, ReflectionId),
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<u32, MetaError> {
        let loc = ctx.loc;
        if let Reflectee::Namespace(ns) = self.ast.reflections.get(r) {
            let ns = self.ast.first(ns);
            self.refresh_namespace(ns, loc)?;
            let items = &self
                .meta_members
                .cache
                .get(ns, MemberList::Members)
                .map_or(&[][..], |l| &l.items[..]);
            // Namespace members need no access check.
            return Ok(filter_members(items, oven.meta_infos(), |_| true));
        }
        let Some(def) = self.complete_class_of(r, ctx, oven)? else {
            return fail(loc, "neither complete class type nor namespace");
        };
        let chk = self.checked(actx, def, ctx);
        self.class_lists(def, MemberOp::MembersOf, &chk, oven, loc)
    }

    /// Writes list query `m` of complete class `def`, filtered by `chk`,
    /// into the Oven's result buffer; the count.
    pub(super) fn class_lists(
        &mut self,
        def: DeclId,
        m: MemberOp,
        chk: &Checked,
        oven: &mut Oven,
        loc: Loc,
    ) -> Result<u32, MetaError> {
        self.build_class(def, oven, loc)?;
        let mut sc = std::mem::take(&mut self.meta_members.scope);
        if let Checked::From { from, .. } = *chk {
            self.set_access_scope(&mut sc, from);
        }
        let st = &self.meta_members.cache;
        let members = st.get(def, MemberList::Members).map_or(&[][..], |l| &l.items[..]);
        let bases = st.get(def, MemberList::Bases).map_or(&[][..], |l| &l.items[..]);
        let out = oven.meta_infos();
        let n = match m {
            MemberOp::MembersOf => {
                filter_members(members, out, |e| self.entry_accessible(e, def, false, chk, &sc))
            }
            MemberOp::BasesOf => {
                filter_members(bases, out, |e| self.entry_accessible(e, def, true, chk, &sc))
            }
            MemberOp::StaticDataMembersOf => filter_members(members, out, |e| {
                e.kind & mk::SDM != 0 && self.entry_accessible(e, def, false, chk, &sc)
            }),
            MemberOp::NonstaticDataMembersOf => filter_members(members, out, |e| {
                e.kind & mk::NSDM != 0 && self.entry_accessible(e, def, false, chk, &sc)
            }),
            _ => {
                filter_members(bases, out, |e| self.entry_accessible(e, def, true, chk, &sc));
                out.extend(
                    members
                        .iter()
                        .filter(|e| {
                            e.kind & mk::NSDM != 0 && self.entry_accessible(e, def, false, chk, &sc)
                        })
                        .map(|e| e.r),
                );
                u32::try_from(out.len()).unwrap_or(u32::MAX)
            }
        };
        self.meta_members.scope = sc;
        Ok(n)
    }

    /// Whether entry `e` of class `parent`'s members (or bases) passes
    /// access check `chk` ([meta.reflection.access.queries]).
    pub(super) fn entry_accessible(
        &self,
        e: &MemberEntry,
        parent: DeclId,
        base: bool,
        chk: &Checked,
        sc: &AccessScope,
    ) -> bool {
        match *chk {
            Checked::Unchecked => true,
            Checked::Nothing => false,
            Checked::From { naming, .. } => {
                if base {
                    let Reflectee::BaseSpec(_, i) = self.ast.reflections.get(e.r) else {
                        return true;
                    };
                    let Some(spec) = self.base_spec_index(parent, i) else { return true };
                    self.base_accessible_in(sc, naming, parent, spec, e.decl)
                } else if e.kind & mk::PUBLIC != 0 && self.same_entity(naming, parent) {
                    true
                } else {
                    self.member_accessible_in(sc, e.decl, naming)
                }
            }
        }
    }

    /// The `i`th base-specifier of class `def` (its index in the
    /// base-specifier table).
    pub(super) fn base_spec_index(&self, def: DeclId, i: u32) -> Option<u32> {
        let def = self.ast.definition(def).unwrap_or(def);
        let list = self.ast.decls.record(def)?.bases;
        self.ast.lists.get(list).get(i as usize).copied()
    }

    /// The access check access context `(scope, designating)` makes of
    /// the members of class `parent` (P2996R13
    /// [meta.reflection.access.queries]/2): none when the designating
    /// class is neither `parent` nor derived from it, unchecked for a
    /// null scope, else from the scope, the members named in the
    /// designating class.
    pub(super) fn checked(
        &self,
        (scope, designating): (ReflectionId, ReflectionId),
        parent: DeclId,
        _ctx: &MetaCtx,
    ) -> Checked {
        let naming = match self.reflected_class_def(designating) {
            None => parent,
            Some(d) if same_class(self, d, parent) => parent,
            Some(d) if is_derived_from(self, d, parent) => d,
            Some(_) => return Checked::Nothing,
        };
        match self.scope_decl(scope) {
            None => Checked::Unchecked,
            Some(from) => Checked::From { from, naming },
        }
    }

    /// The definition of the class type `r` reflects (null: `None`).
    pub(super) fn reflected_class_def(&self, r: ReflectionId) -> Option<DeclId> {
        let Reflectee::Type(t) = self.ast.reflections.get(r) else { return None };
        let d = self.ast.types.as_record(t)?;
        Some(self.ast.definition(d).unwrap_or(d))
    }

    /// The function, class or namespace an access context's scope `r`
    /// reflects (null: `None`).
    pub(super) fn scope_decl(&self, r: ReflectionId) -> Option<DeclId> {
        match self.ast.reflections.get(r) {
            Reflectee::Decl(d) | Reflectee::Namespace(d) => Some(d),
            Reflectee::Type(t) => {
                let d = self.ast.types.as_record(t)?;
                Some(self.ast.definition(d).unwrap_or(d))
            }
            _ => None,
        }
    }

    /// Builds the member and base lists of complete class `def` once,
    /// declaring its implicit special members first ([class.mem]/5).
    fn build_class(&mut self, def: DeclId, oven: &mut Oven, loc: Loc) -> Result<(), MetaError> {
        if self.meta_members.cache.get(def, MemberList::Members).is_some() {
            return Ok(());
        }
        self.lend_oven(oven, |s| {
            for k in SpecialKind::ALL {
                s.ensure_implicit(def, k);
            }
        });
        let decls: Vec<DeclId> = self.ast.members(def).collect();
        let mut l = CachedList::default();
        let mut implicit: Vec<(SpecialKind, DeclId)> = Vec::new();
        let dtor = self.ast.destructor(def).map(|d| self.ast.first(d));
        for m in decls {
            let n = self.ast.decls.get(m);
            let kind = n.kind();
            // A friend declaration names a non-member ([class.friend]/1).
            if never_a_member(kind) || n.misc().friend() != kiln_ast::decl::FriendKind::None {
                continue;
            }
            if kind == DeclKind::CXXDestructor && dtor.is_some_and(|d| !self.same_entity(d, m)) {
                continue;
            }
            if n.has(DeclFlags::IMPLICIT) && kind != DeclKind::Field {
                if let Some(k) = self.special_kind_of(m) {
                    implicit.push((k, m));
                }
                continue;
            }
            if matches!(self.ast.templ_link(m).get(), Some((TemplLinkKind::Specialization, _))) {
                continue;
            }
            // A function whose constraints are not satisfied is not
            // members-of-representable ([meta.reflection.member.queries]/2
            // P2996R13; GCC 16.1 skips it too): a member of a class template
            // specialization redeclared from a pattern member with a
            // trailing requires-clause that fails for these arguments.
            if kind.is_function()
                && self.ast.templates.trailing_requires.get(m).is_some()
                && self.lend_oven(oven, |s| {
                    s.check_function_constraints(m, loc).is_some_and(|sat| !s.sat_ok(sat))
                })
            {
                continue;
            }
            if !l.seen.insert(self.ast.first(m)) {
                continue;
            }
            let e = self.class_entry(m, loc)?;
            l.items.push(e);
        }
        for k in SPECIAL_ORDER {
            for &(_, m) in implicit.iter().filter(|x| x.0 == k) {
                if l.seen.insert(self.ast.first(m)) {
                    let e = self.class_entry(m, loc)?;
                    l.items.push(e);
                }
            }
        }
        l.seen = kiln_source::fx::FxHashSet::default();
        self.meta_members.cache.put(def, MemberList::Members, l);
        let mut b = CachedList::default();
        let first = self.ast.first(def);
        let specs: Vec<u32> =
            self.ast.decls.record(def).map_or(&[][..], |x| self.ast.lists.get(x.bases)).to_vec();
        for (i, spec) in specs.into_iter().enumerate() {
            let s = *self.ast.decls.base_spec(spec);
            let Some(base) = self.ast.types.as_record(s.ty) else { continue };
            let base = self.ast.definition(base).unwrap_or(base);
            let public = self.base_access(def, s.access) == Access::Public;
            let r = self.meta_intern(Reflectee::BaseSpec(first, i as u32), loc)?;
            b.items.push(MemberEntry { r, decl: base, kind: if public { mk::PUBLIC } else { 0 } });
        }
        self.meta_members.cache.put(def, MemberList::Bases, b);
        Ok(())
    }

    /// The entry of class member `m`.
    fn class_entry(&mut self, m: DeclId, loc: Loc) -> Result<MemberEntry, MetaError> {
        let n = self.ast.decls.get(m);
        let mut kind = match n.kind() {
            DeclKind::Field => {
                let unnamed_bf = n.name().is_empty()
                    && self.ast.decls.field(m).is_some_and(|f| f.bit_width.is_some());
                if unnamed_bf { mk::UNNAMED_BIT_FIELD } else { mk::NSDM }
            }
            DeclKind::Var => mk::SDM,
            _ => 0,
        };
        if n.misc().access() == Access::Public {
            kind |= mk::PUBLIC;
        }
        let r = self.member_reflection(m, loc)?;
        Ok(MemberEntry { r, decl: m, kind })
    }

    /// The reflection of member declaration `m`, keyed as `^^` keys it.
    fn member_reflection(&mut self, m: DeclId, loc: Loc) -> Result<ReflectionId, MetaError> {
        let r = self.reflectee_of_decl(m).unwrap_or(Reflectee::Decl(self.entity_decl(m)));
        self.meta_intern(r, loc)
    }

    /// Interns `r`; a full reflection table is the fatal
    /// `sema_too_many_reflections` (a partial cache list is dropped).
    pub(super) fn meta_intern(
        &mut self,
        r: Reflectee,
        loc: Loc,
    ) -> Result<ReflectionId, MetaError> {
        Ok(self.intern_reflection(r, loc)?)
    }

    /// Brings namespace `ns`'s (first declaration) list up to date: the
    /// members of each of its definitions so far, in order, each entity
    /// once (spec/reflection-members.md §2).
    fn refresh_namespace(&mut self, ns: DeclId, loc: Loc) -> Result<(), MetaError> {
        let stamp = u32::try_from(self.ast.decls.len()).unwrap_or(u32::MAX);
        if self.meta_members.cache.get(ns, MemberList::Members).is_some_and(|l| l.stamp == stamp) {
            return Ok(());
        }
        let mut l = self.meta_members.cache.take(ns, MemberList::Members);
        // New definitions of the namespace (redeclarations, latest first).
        // Cursors are per definition block, not per entity.
        // same_entity: raw ok
        let known = |l: &CachedList, d: DeclId| l.cursors.iter().any(|c| c.0 == d);
        let mut fresh: Vec<DeclId> = self.ast.redecls_of(ns).filter(|&d| !known(&l, d)).collect();
        fresh.reverse();
        l.cursors.extend(fresh.into_iter().map(|d| (d, 0)));
        // The namespace's own blocks, in order; a linkage specification or
        // export block inside one is walked in place, so the list is in
        // declaration order across them.
        for i in 0..l.cursors.len() {
            let block = l.cursors[i].0;
            if !is_nested_block(self.ast.decls.get(block).kind()) {
                self.walk_block(ns, i, &mut l, loc)?;
            }
        }
        // GCC 16.1 orders a namespace's members by `DECL_UID`, the creation
        // order of each entity's first declaration (gcc/cp/reflect.cc
        // `members_cmp`): a function first declared as a friend in a class
        // comes where the friend declaration is, not where the namespace
        // redeclares it. Declarations are allocated in creation order, so
        // the first declaration's id is that order.
        l.items.sort_by_key(|e| self.ast.first(e.decl).index());
        l.stamp = stamp;
        self.meta_members.cache.put(ns, MemberList::Members, l);
        Ok(())
    }

    /// Visits the members of the block of cursor `idx` not visited yet,
    /// walking each nested block (a linkage specification or export
    /// block) where it appears. Only the last member visited before can
    /// be a nested block still open, so it catches up first: its new
    /// members precede the block's own.
    fn walk_block(
        &mut self,
        ns: DeclId,
        idx: usize,
        l: &mut CachedList,
        loc: Loc,
    ) -> Result<(), MetaError> {
        let (block, done) = l.cursors[idx];
        let last = done.checked_sub(1).and_then(|k| self.ast.members(block).nth(k as usize));
        if let Some(last) = last.filter(|&d| is_nested_block(self.ast.decls.get(d).kind()))
            && let Some(c) = l.cursors.iter().position(|c| c.0 == last)
        {
            self.walk_block(ns, c, l, loc)?;
        }
        let news: Vec<DeclId> = self.ast.members(block).skip(done as usize).collect();
        for m in news {
            l.cursors[idx].1 += 1;
            if is_nested_block(self.ast.decls.get(m).kind()) {
                l.cursors.push((m, 0));
                let c = l.cursors.len() - 1;
                self.walk_block(ns, c, l, loc)?;
            } else {
                self.namespace_member(ns, m, l, loc)?;
            }
        }
        Ok(())
    }

    /// Adds namespace member declaration `m` (lexically in a definition
    /// of `ns`) to `l`.
    fn namespace_member(
        &mut self,
        ns: DeclId,
        m: DeclId,
        l: &mut CachedList,
        loc: Loc,
    ) -> Result<(), MetaError> {
        let n = self.ast.decls.get(m);
        let kind = n.kind();
        if never_a_member(kind)
            || n.has(DeclFlags::IMPLICIT)
            || kind == DeclKind::TranslationUnit
            || matches!(self.ast.templ_link(m).get(), Some((TemplLinkKind::Specialization, _)))
        {
            return Ok(());
        }
        // A declaration of a member of another scope (a qualified
        // redeclaration, a block-scope `extern`).
        let sem = n.ctx().map(|c| self.ast.first(self.ast.redecl_context(c)));
        if !sem.is_some_and(|c| self.same_entity(c, ns)) {
            return Ok(());
        }
        if !l.seen.insert(self.ast.first(m)) {
            return Ok(());
        }
        let r = self.member_reflection(m, loc)?;
        l.items.push(MemberEntry { r, decl: m, kind: mk::PUBLIC });
        Ok(())
    }

    /// `enumerators_of(r)`: the enumerators of the enumeration type `r`
    /// reflects, which must be enumerable (defined).
    fn enumerators_of(
        &mut self,
        r: ReflectionId,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<u32, MetaError> {
        let loc = ctx.loc;
        let def = match self.ast.reflections.get(r) {
            Reflectee::Type(t) => {
                if self.undefined_spec(t).is_some() && self.meta_may_instantiate(ctx) {
                    self.lend_oven(oven, |s| s.require_complete(t, loc));
                }
                self.ast.types.as_enum(t).and_then(|d| self.enum_definition(d))
            }
            _ => None,
        };
        let Some(def) = def else {
            return fail(loc, "reflection does not represent an enumerable enumeration type");
        };
        if self.meta_members.cache.get(def, MemberList::Enumerators).is_none() {
            let es: Vec<DeclId> = self
                .ast
                .members(def)
                .filter(|&e| self.ast.decls.get(e).kind() == DeclKind::EnumConstant)
                .collect();
            let mut l = CachedList::default();
            for e in es {
                let r = self.meta_intern(Reflectee::Decl(self.entity_decl(e)), loc)?;
                l.items.push(MemberEntry { r, decl: e, kind: mk::PUBLIC });
            }
            self.meta_members.cache.put(def, MemberList::Enumerators, l);
        }
        let items = self
            .meta_members
            .cache
            .get(def, MemberList::Enumerators)
            .map_or(&[][..], |l| &l.items[..]);
        Ok(filter_members(items, oven.meta_infos(), |_| true))
    }

    /// The definition of enumeration `d`, once its enumerator list was
    /// seen (`}` seen: an opaque declaration, or one being
    /// defined, is not enumerable).
    fn enum_definition(&self, d: DeclId) -> Option<DeclId> {
        let complete = |x: DeclId| {
            self.ast
                .decls
                .enumeration(x)
                .is_some_and(|e| e.flags & kiln_ast::decl::EnumData::COMPLETE != 0)
        };
        self.ast
            .definition(d)
            .filter(|&x| complete(x))
            .or_else(|| self.ast.redecls_of(d).find(|&x| complete(x)))
    }

    /// `parameters_of(r)`: a function's parameters (the explicit object
    /// parameter included) or a function type's parameter types.
    fn parameters_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<u32, MetaError> {
        let not_fn = || fail(loc, "reflection does not represent a function or function type");
        let mut v = std::mem::take(oven.meta_infos());
        v.clear();
        match self.ast.reflections.get(r) {
            Reflectee::Decl(f) if self.kind_of(f).is_function() => {
                let first = self.ast.first(f);
                let n = self
                    .ast
                    .decls
                    .function(first)
                    .map_or(0, |x| self.ast.lists.get(x.params).len());
                for i in 0..n {
                    v.push(self.meta_intern(Reflectee::FnParam(first, i as u32), loc)?);
                }
            }
            Reflectee::Type(t) if self.ast.types.proto(t).is_some() => {
                let ps: Vec<QualType> = self
                    .ast
                    .types
                    .proto(t)
                    .map(|p| self.ast.types.params(p).to_vec())
                    .unwrap_or_default();
                for p in ps {
                    let q = self.ast.reflection_type(p);
                    v.push(self.meta_intern(Reflectee::Type(q), loc)?);
                }
            }
            _ => {
                *oven.meta_infos() = v;
                return not_fn();
            }
        }
        let n = u32::try_from(v.len()).unwrap_or(u32::MAX);
        *oven.meta_infos() = v;
        Ok(n)
    }

    // -- layout ---------------------------------------------------------

    /// The bit-field width of field `f`, if it is a bit-field.
    fn bit_width_of(&self, f: DeclId) -> Option<u32> {
        let w = self.ast.decls.field(f)?.bit_width?;
        crate::conv::eval_width(self, w)
    }

    /// `offset_of(r)` in bits: a non-static data member's or unnamed
    /// bit-field's offset in its class, or a direct base's offset in
    /// (a complete object of) its derived class.
    fn offset_bits(&mut self, r: ReflectionId, loc: Loc) -> Result<u64, MetaError> {
        let bad = || fail(loc, "reflection unsuitable for offset_of");
        match self.ast.reflections.get(r) {
            Reflectee::Decl(f) if self.kind_of(f) == DeclKind::Field => {
                Ok(self.field_bit_offset(f))
            }
            Reflectee::BaseSpec(d, i) => {
                let def = self.ast.definition(d).unwrap_or(d);
                let Some(spec) = self.base_spec_index(def, i) else { return bad() };
                let s = *self.ast.decls.base_spec(spec);
                let Some(base) = self.ast.types.as_record(s.ty) else { return bad() };
                let base = self.ast.first(base);
                let Some(lay) = kiln_abi::ensure_layout(&mut self.ast, def, &self.abi) else {
                    return bad();
                };
                let list = if s.is_virtual { lay.vbases.clone() } else { lay.bases.clone() };
                let Some(&(_, off)) = list.iter().find(|(b, _)| self.same_entity(*b, base)) else {
                    return bad();
                };
                Ok(off * 8)
            }
            _ => bad(),
        }
    }

    /// The type whose size or alignment `size_of`/`alignment_of`/
    /// `bit_size_of` of `r` measure, and whether `r` is a bit-field
    /// (`Err`: not a suitable reflection; `what` names the query).
    fn layout_type(
        &mut self,
        r: ReflectionId,
        ctx: &MetaCtx,
        what: &str,
        oven: &mut Oven,
    ) -> Result<(QualType, Option<u32>), MetaError> {
        let loc = ctx.loc;
        let bad = || fail(loc, format!("reflection not suitable for {what}"));
        let (t, width, is_type) = match self.ast.reflections.get(r) {
            Reflectee::Type(t) => (t, None, true),
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Field => (
                    self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR),
                    self.bit_width_of(d),
                    false,
                ),
                DeclKind::Var | DeclKind::VarTemplateSpecialization => {
                    let t = self.reflected_var_type(d).unwrap_or(QualType::ERROR);
                    if self.ast.types.is_reference(t) {
                        return bad();
                    }
                    (t, None, false)
                }
                _ => return bad(),
            },
            Reflectee::Value(cv) | Reflectee::Object(cv) => (self.ast.consts.ty(cv), None, false),
            Reflectee::BaseSpec(d, i) => {
                let def = self.ast.definition(d).unwrap_or(d);
                let Some(spec) = self.base_spec_index(def, i) else { return bad() };
                (self.ast.decls.base_spec(spec).ty, None, false)
            }
            _ => return bad(),
        };
        if self.undefined_spec(t).is_some() && self.meta_may_instantiate(ctx) {
            self.lend_oven(oven, |s| s.require_complete(t, loc));
        }
        let _ = is_type;
        Ok((t, width))
    }

    /// `(sizeof, alignof)` of `t` as [meta.reflection.layout] measures a
    /// member of that type (a reference as a pointer).
    fn measured(&self, t: QualType) -> Option<(u64, u64)> {
        if self.ast.types.is_reference(t) {
            let p = self.abi.pointer_size;
            return Some((p, p));
        }
        crate::target::size_align(&self.ast, t, &self.abi)
    }

    fn layout_size(
        &mut self,
        r: ReflectionId,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<u64, MetaError> {
        let loc = ctx.loc;
        let (t, width) = self.layout_type(r, ctx, "size_of", oven)?;
        if width.is_some() {
            return fail(loc, "reflection not suitable for size_of");
        }
        match self.measured(t) {
            Some((s, _)) => Ok(s),
            None => fail(loc, "reflection with incomplete type in size_of"),
        }
    }

    fn layout_align(
        &mut self,
        r: ReflectionId,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<u64, MetaError> {
        let loc = ctx.loc;
        let (t, width) = self.layout_type(r, ctx, "alignment_of", oven)?;
        if width.is_some() {
            return fail(loc, "reflection not suitable for alignment_of");
        }
        // A variable's or data member's own alignment (`alignas` counts).
        let decl = match self.ast.reflections.get(r) {
            Reflectee::Decl(d) => Some(d),
            _ => None,
        };
        if self.measured(t).is_none() {
            return fail(loc, "reflection with incomplete type in alignment_of");
        }
        if let Some(d) = decl {
            if self.kind_of(d) == DeclKind::Field && self.ast.types.is_reference(t) {
                return Ok(self.abi.pointer_size);
            }
            return Ok(kiln_abi::layout::decl_align(&self.ast, d, &self.abi, true));
        }
        Ok(self.measured(t).map_or(1, |x| x.1))
    }

    fn layout_bit_size(
        &mut self,
        r: ReflectionId,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<u64, MetaError> {
        let loc = ctx.loc;
        let (t, width) = self.layout_type(r, ctx, "bit_size_of", oven)?;
        if let Some(w) = width {
            return Ok(u64::from(w));
        }
        match self.measured(t) {
            Some((s, _)) => Ok(s * 8),
            None => fail(loc, "reflection with incomplete type in bit_size_of"),
        }
    }
}
