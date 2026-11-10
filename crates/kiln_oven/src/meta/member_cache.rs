//! The member-list cache of the member queries (WP-478;
//! spec/reflection-members.md §2): each class, enumeration and namespace
//! gets one flat list of its members, built once, and every query copies
//! the entries that pass its filter into the Oven's reusable result
//! buffer ([`crate::Oven::meta_infos`]) — no allocation in steady state.
//!
//! A class's (and an enumeration's) list is final when it is built (the
//! entity is complete). A namespace grows: its list is a prefix of the
//! namespace's members, extended from per-block cursors when the
//! declaration count moved since it was last brought up to date (a
//! repeated query of an unchanged namespace walks nothing).

use kiln_ast::{DeclId, ReflectionId};
use kiln_source::fx::{FxHashMap, FxHashSet};

/// The kind bits of a [`MemberEntry`] (what the filtered queries test).
pub mod member_kind {
    /// A non-static data member ([`super::MemberList::Members`]; an
    /// anonymous union's object counts).
    pub const NSDM: u8 = 1;
    /// A static data member.
    pub const SDM: u8 = 2;
    /// An unnamed bit-field (a member, not a data member).
    pub const UNNAMED_BIT_FIELD: u8 = 4;
    /// Needs no access check: a public member of the class itself (the
    /// common case, tested without asking sema).
    pub const PUBLIC: u8 = 8;
    /// A variable or function declared with a placeholder type that was
    /// still undeduced when the list was built: each query checks it
    /// again, since a member whose type contains an undeduced placeholder
    /// type is not members-of-representable ([meta.reflection.member.queries]/3)
    /// and a later definition deduces it.
    pub const PLACEHOLDER: u8 = 16;
}

/// Which list of an entity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemberList {
    /// `members_of`: the members of a class or namespace.
    Members,
    /// `bases_of`: the direct base-specifiers of a class.
    Bases,
    /// `enumerators_of`: the enumerators of an enumeration.
    Enumerators,
}

/// One member: its reflection, the declaration that is checked for
/// access (a base's derived class for a base-specifier) and its kind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MemberEntry {
    /// The member's reflection (what the query returns).
    pub r: ReflectionId,
    /// The declaration access is checked on (for a base-specifier, the
    /// base class).
    pub decl: DeclId,
    /// [`member_kind`] bits.
    pub kind: u8,
}

/// One entity's list.
#[derive(Clone, Debug, Default)]
pub struct CachedList {
    /// The members in order.
    pub items: Vec<MemberEntry>,
    /// A namespace: the declaration count the list is current for.
    pub stamp: u32,
    /// A namespace: per declaration block (a `namespace` definition, or
    /// a linkage specification or export block in one), how many of its
    /// members were visited.
    pub cursors: Vec<(DeclId, u32)>,
    /// A namespace: the first declarations already listed (a
    /// redeclaration in a later block adds nothing).
    pub seen: FxHashSet<DeclId>,
}

/// The cache: one list per (entity, list kind).
#[derive(Debug, Default)]
pub struct MemberCache {
    lists: FxHashMap<(DeclId, MemberList), CachedList>,
    /// Lists built (a namespace counts each extension): the budget tests'
    /// evidence that a repeated query reuses its list.
    pub builds: u32,
}

impl MemberCache {
    /// The list of `(d, k)`, if built.
    pub fn get(&self, d: DeclId, k: MemberList) -> Option<&CachedList> {
        self.lists.get(&(d, k))
    }

    /// Takes the list of `(d, k)` out to extend it (a fresh one if none);
    /// [`MemberCache::put`] puts it back.
    pub fn take(&mut self, d: DeclId, k: MemberList) -> CachedList {
        self.lists.remove(&(d, k)).unwrap_or_default()
    }

    /// Stores the list of `(d, k)`.
    pub fn put(&mut self, d: DeclId, k: MemberList, l: CachedList) {
        self.builds += 1;
        self.lists.insert((d, k), l);
    }
}

/// Copies the reflections of the `items` that `keep` accepts into `out`
/// (cleared first); the count.
pub fn filter_members(
    items: &[MemberEntry],
    out: &mut Vec<ReflectionId>,
    mut keep: impl FnMut(&MemberEntry) -> bool,
) -> u32 {
    out.clear();
    out.extend(items.iter().filter(|m| keep(m)).map(|m| m.r));
    u32::try_from(out.len()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_reuses_the_buffer() {
        let e = |i: u32, kind| MemberEntry {
            r: ReflectionId::from_bits(u128::from(i)).unwrap(),
            decl: DeclId::new(i + 1),
            kind,
        };
        let mut c = MemberCache::default();
        let d = DeclId::new(7);
        let mut l = c.take(d, MemberList::Members);
        l.items.extend([e(1, member_kind::NSDM), e(2, member_kind::SDM), e(3, member_kind::NSDM)]);
        c.put(d, MemberList::Members, l);
        let mut out = Vec::with_capacity(8);
        let cap = out.capacity();
        for _ in 0..100 {
            let items = &c.get(d, MemberList::Members).unwrap().items;
            let n = filter_members(items, &mut out, |m| m.kind & member_kind::NSDM != 0);
            assert_eq!(n, 2);
        }
        assert_eq!(out.capacity(), cap);
        assert_eq!(c.builds, 1);
        assert!(c.get(d, MemberList::Bases).is_none());
    }
}
