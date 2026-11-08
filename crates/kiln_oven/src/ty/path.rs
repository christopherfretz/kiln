//! The subobject path cache (WP-644, spec/oven.md "Path cache"): the
//! memory-independent part of the Oven's walks from an allocation's root
//! type to a designated subobject, memoized per (record type, byte
//! offset, target type).
//!
//! A walk (`Oven::union_path`, `Oven::mutable_path`,
//! `Oven::place_in_unions`) descends from the root through arrays and
//! records to the object a pointer designates. Which member it descends
//! into at a non-union record, where the next union or array lies, and
//! which members of that union may hold the object are pure functions of
//! the types, so [`Types::hop`] computes them once per key. Only the
//! active-member reads at each union depend on memory, and the walks keep
//! doing those on every access, so a cached hop never freezes an
//! active-member answer. Array levels stay in the walks (index
//! arithmetic), which keeps the keys bounded by the size of the records
//! rather than of the arrays holding them. [`Types::extent`] and
//! [`Types::has_subobject`] are cached the same way below their leading
//! arrays.
//!
//! Each map holds at most [`PATH_CACHE_MAX`] entries; the insert past
//! that clears it (the entries are recomputable), so the cache's memory is
//! bounded however many distinct paths an evaluation touches. Rebuilding
//! a provisional class slot in place ([`Types::refresh`]) clears it too:
//! hops computed while the class was `Opaque` would be stale.
//!
//! In front of the extent map, a small fixed table ([`ExtentFront`],
//! WP-760) answers [`Types::extent`] for a whole array at once: an
//! answer naming an array that no other member overlaps on the way down
//! from the root is the answer for every byte of that array, so stepping
//! a pointer through an array member (`union { T data[N]; }` in a class)
//! costs one probe instead of one map lookup per element.

use std::cell::{Cell, RefCell};
use std::hash::Hash;

use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;

use super::{FieldInfo, OTy, TyKind, Types};

/// The most entries each map of the path cache holds; the next insert
/// clears the map (tests/conformance/class.union/constexpr-path-cache-evict.cpp touches
/// more distinct paths than this).
pub(crate) const PATH_CACHE_MAX: usize = 1 << 14;

/// Which walk a hop is for: each picks members by its own rule.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Walk {
    /// `Oven::union_path`: members toward the object
    /// ([`Types::member_toward`], else the first holding the byte); a
    /// union member may hold it as a subobject, a bit-field or a
    /// bit-field's storage unit.
    Union,
    /// `Oven::place_in_unions`: as [`Walk::Union`] without bit-fields.
    Place,
    /// `Oven::mutable_path`: the first member holding the byte; a
    /// `mutable` one on the path ends the walk.
    Mutable,
}

/// The memory-independent part of a walk from a record toward an object,
/// up to the first point where it needs memory or index arithmetic.
/// Offsets are from the start of the record the hop was asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Hop {
    /// The walk reaches the object without meeting a union.
    Reached,
    /// The walk ends without reaching the object or meeting a union (no
    /// member holds the byte, a bit-field holds it, or a scalar does).
    Stop,
    /// The walk continues in array `t` at byte `at`.
    Array { at: u64, t: OTy },
    /// The union `u` at byte `at` holds the byte. `reach` has bit `i` set
    /// when member `i` (of the first 64) may hold the object by the walk's
    /// rule ([`Types::union_member_reaches`]); the walk picks among them by
    /// the union's active member.
    Union { at: u64, u: OTy, reach: u64 },
    /// [`Walk::Mutable`]: the walk meets `mutable` member `decl`.
    Mutable(DeclId),
}

type HopKey = (OTy, u64, OTy, Walk);
type Key = (OTy, u64, OTy);
/// An extent answer ([`Types::extent`]): start, length, is an array.
type Extent = Option<(u64, u64, bool)>;

/// The path cache's maps (owned by [`Types`]; interior mutability, since
/// the walks read the type table through shared references).
#[derive(Debug, Default)]
pub(crate) struct PathCache {
    hops: RefCell<FxHashMap<HopKey, Hop>>,
    extents: RefCell<FxHashMap<Key, Extent>>,
    subobjects: RefCell<FxHashMap<Key, bool>>,
    front: ExtentFront,
}

impl PathCache {
    /// Forgets every entry (a type was rebuilt in place).
    pub(crate) fn clear(&mut self) {
        self.hops.get_mut().clear();
        self.extents.get_mut().clear();
        self.subobjects.get_mut().clear();
        self.front.clear();
    }

    /// Bytes of heap the maps hold.
    pub(crate) fn footprint(&self) -> usize {
        fn map<K, V>(m: &RefCell<FxHashMap<K, V>>) -> usize {
            m.borrow().capacity() * (size_of::<(K, V)>() + 1)
        }
        map(&self.hops)
            + map(&self.extents)
            + map(&self.subobjects)
            + self.front.e.len() * size_of::<FrontEntry>()
    }
}

/// log2 of the number of [`ExtentFront`] entries.
const FRONT_BITS: u32 = 8;

/// An [`ExtentFront`] entry: for a pointer to `p` into an object of type
/// `root`, every byte in `[lo, hi)` lies in the array of `n` elements
/// starting at `lo` (`hi == 0`: empty).
#[derive(Clone, Copy, Debug)]
struct FrontEntry {
    root: OTy,
    p: OTy,
    lo: u64,
    hi: u64,
    n: u64,
}

const FRONT_EMPTY: FrontEntry = FrontEntry { root: OTy(0), p: OTy(0), lo: 0, hi: 0, n: 0 };

/// The whole-array answers of [`Types::extent`] (WP-760): direct-mapped
/// and two-way per (root, element type), fixed-size, so a collision
/// overwrites and the table never grows.
#[derive(Debug)]
struct ExtentFront {
    e: Box<[Cell<FrontEntry>]>,
}

impl Default for ExtentFront {
    fn default() -> Self {
        ExtentFront { e: (0..1usize << FRONT_BITS).map(|_| Cell::new(FRONT_EMPTY)).collect() }
    }
}

impl ExtentFront {
    fn slot(root: OTy, p: OTy) -> usize {
        let h = (u64::from(root.0) | u64::from(p.0) << 32).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        (h >> (64 - FRONT_BITS)) as usize & !1
    }

    fn get(&self, root: OTy, o: u64, p: OTy) -> Option<Extent> {
        let i = Self::slot(root, p);
        for e in &self.e[i..i + 2] {
            let e = e.get();
            if e.root == root && e.p == p && o >= e.lo && o < e.hi {
                return Some(Some((e.lo, e.n, true)));
            }
        }
        None
    }

    fn put(&self, root: OTy, p: OTy, lo: u64, hi: u64, n: u64) {
        let i = Self::slot(root, p);
        // The first way holds the latest answer; the one it held moves to
        // the second, so two arrays of one (root, element) pair coexist.
        let old = self.e[i].replace(FrontEntry { root, p, lo, hi, n });
        if old.root != root || old.p != p || old.lo != lo {
            self.e[i + 1].set(old);
        }
    }

    fn clear(&mut self) {
        for e in &mut self.e {
            *e.get_mut() = FRONT_EMPTY;
        }
    }
}

/// The cached value of `k` in `m`, computing and inserting it on a miss
/// (clearing the map first when it is full). No borrow is held while
/// `f` runs, so `f` may consult the cache itself.
fn cached<K: Eq + Hash + Copy, V: Copy>(
    m: &RefCell<FxHashMap<K, V>>,
    k: K,
    f: impl FnOnce() -> V,
) -> V {
    if let Some(&v) = m.borrow().get(&k) {
        return v;
    }
    let v = f();
    let mut m = m.borrow_mut();
    if m.len() >= PATH_CACHE_MAX {
        m.clear();
    }
    m.insert(k, v);
    v
}

/// The members (below `n`, ascending) that `alts` marks as possibly
/// active, other than `active` (`Memory::union_alts`, WP-233).
pub(crate) fn alt_members(alts: u64, n: u32, active: Option<u32>) -> impl Iterator<Item = u32> {
    let below = if n >= 64 { u64::MAX } else { (1u64 << n) - 1 };
    let not_active = active.filter(|&i| i < 64).map_or(u64::MAX, |i| !(1u64 << i));
    let mut rest = alts & below & not_active;
    std::iter::from_fn(move || {
        (rest != 0).then(|| {
            let i = rest.trailing_zeros();
            rest &= rest - 1;
            i
        })
    })
}

impl Types {
    /// The walk `w` from record `t` toward the object of type `p` at byte
    /// `off` of it, up to its first union or array (cached).
    pub(crate) fn hop(&self, w: Walk, t: OTy, off: u64, p: OTy) -> Hop {
        cached(&self.paths.hops, (t, off, p, w), || self.hop_in(w, t, off, p))
    }

    fn hop_in(&self, w: Walk, mut t: OTy, mut off: u64, p: OTy) -> Hop {
        let mut at = 0;
        loop {
            if t == p && off == 0 {
                return Hop::Reached;
            }
            match self.kind(t) {
                TyKind::Array { .. } => return Hop::Array { at, t },
                TyKind::Record { union: true, .. } => {
                    let reach = self
                        .fields(t)
                        .iter()
                        .take(64)
                        .enumerate()
                        .filter(|(_, f)| self.union_member_reaches(w, off, p, f))
                        .fold(0u64, |m, (i, _)| m | 1 << i);
                    return Hop::Union { at, u: t, reach };
                }
                TyKind::Record { .. } => {
                    let contains =
                        |f: &FieldInfo| off >= f.offset && off < f.offset + self.size(f.ty).max(1);
                    let f = match w {
                        Walk::Union => self
                            .member_toward(t, off, p)
                            .or_else(|| self.near(t, off).find(|f| contains(f)).copied()),
                        Walk::Place => self.member_toward(t, off, p).or_else(|| {
                            self.near(t, off).find(|f| f.width.is_none() && contains(f)).copied()
                        }),
                        Walk::Mutable => self.near(t, off).find(|f| contains(f)).copied(),
                    };
                    let Some(f) = f else { return Hop::Stop };
                    if w == Walk::Mutable && f.mutable {
                        return Hop::Mutable(f.decl);
                    }
                    if f.width.is_some() {
                        return Hop::Stop;
                    }
                    at += f.offset;
                    off -= f.offset;
                    t = f.ty;
                }
                _ => return Hop::Stop,
            }
        }
    }

    /// Whether member `f` of a union may hold the object of type `p` at
    /// byte `off` of the union, by walk `w`'s rule (the uncached
    /// predicate of [`Hop::Union`]'s `reach`).
    pub(crate) fn union_member_reaches(&self, w: Walk, off: u64, p: OTy, f: &FieldInfo) -> bool {
        if off < f.offset || off >= f.offset + self.size(f.ty).max(1) {
            return false;
        }
        let rel = off - f.offset;
        match w {
            Walk::Union => {
                f.width.is_some()
                    || self.subobject_in(f.ty, rel, p)
                    || self.has_bit_unit(f.ty, rel, p)
            }
            Walk::Place => f.width.is_none() && self.subobject_in(f.ty, rel, p),
            Walk::Mutable => true,
        }
    }

    /// Whether member `i` of union `u` may hold the object (see
    /// [`Hop::Union`]): `reach`'s bit for the first 64 members, the
    /// predicate itself past them.
    pub(crate) fn reaches(&self, w: Walk, u: OTy, off: u64, p: OTy, reach: u64, i: u32) -> bool {
        if i < 64 {
            return reach & 1 << i != 0;
        }
        self.fields(u).get(i as usize).is_some_and(|f| self.union_member_reaches(w, off, p, f))
    }

    /// [`Types::extent`] below the leading arrays of `t`, cached per
    /// record.
    pub(super) fn extent_cached(&self, t: OTy, rel: u64, p: OTy) -> Extent {
        cached(&self.paths.extents, (t, rel, p), || self.extent_in(t, 0, rel, p, false))
    }

    /// [`Types::extent`] of the byte at `off` of a `root` object (`past`:
    /// one past the end of the object there) when the whole-array table
    /// holds it: the array's first byte and its bound. `None` when the
    /// table does not (not a fault: [`Types::extent`] decides).
    #[inline]
    pub(crate) fn extent_hit(&self, root: OTy, off: u64, past: bool, p: OTy) -> Option<(u64, u64)> {
        let o = if past { off.checked_sub(1)? } else { off };
        match self.paths.front.get(root, o, p) {
            Some(Some((lo, n, _))) => Some((lo, n)),
            _ => None,
        }
    }

    /// [`Types::extent`] of byte `o` (already moved back from one past
    /// the end) through the whole-array table: a hit, else the walk, whose
    /// array answer is remembered for the whole array when it is one
    /// ([`Types::extent_unique`]).
    pub(super) fn extent_front(&self, root: OTy, o: u64, p: OTy) -> Extent {
        if let Some(r) = self.paths.front.get(root, o, p) {
            return r;
        }
        let r = self.extent_in(root, 0, o, p, true);
        if let Some((lo, n, true)) = r {
            let es = self.size(p);
            if es > 0
                && let Some(hi) = n.checked_mul(es).and_then(|b| b.checked_add(lo))
                && self.extent_unique(root, o, p, lo, hi)
            {
                self.paths.front.put(root, p, lo, hi, n);
            }
        }
        r
    }

    /// Whether the array of `p` at bytes `[lo, hi)` of a `root` object,
    /// the extent of byte `o`, is the extent of every byte in it: on the
    /// way down from `root`, no member other than the one holding it
    /// overlaps those bytes ([`Types::extent_in`] then takes the same
    /// steps for each of them). Records with many members are not
    /// examined (`false`).
    fn extent_unique(&self, mut t: OTy, o: u64, p: OTy, lo: u64, hi: u64) -> bool {
        const FIELDS_MAX: usize = 64;
        let mut base = 0;
        loop {
            match self.kind(t) {
                TyKind::Array { elem, n } => {
                    if self.same_unqualified(elem, p) {
                        return base == lo;
                    }
                    let es = self.size(elem);
                    if self.same_unqualified(t, p) || es == 0 || n == 0 {
                        return false;
                    }
                    base += (o.saturating_sub(base) / es).min(n - 1) * es;
                    t = elem;
                }
                TyKind::Record { .. } if !self.same_unqualified(t, p) => {
                    let fs = self.fields(t);
                    if fs.len() > FIELDS_MAX {
                        return false;
                    }
                    let mut one = None;
                    for f in fs.iter().filter(|f| f.width.is_none()) {
                        let start = base + f.offset;
                        let end = start + self.size(f.ty).max(1);
                        if start < hi && lo < end {
                            if one.is_some() {
                                return false;
                            }
                            one = Some((f.ty, start));
                        }
                    }
                    let Some((ft, start)) = one else { return false };
                    (t, base) = (ft, start);
                }
                _ => return false,
            }
        }
    }

    /// [`Types::has_subobject`] for record `t`, cached.
    pub(super) fn subobject_cached(&self, t: OTy, off: u64, p: OTy) -> bool {
        cached(&self.paths.subobjects, (t, off, p), || self.subobject_in(t, off, p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_map_is_cleared_by_the_next_insert() {
        let m: RefCell<FxHashMap<u64, u64>> = RefCell::default();
        for k in 0..PATH_CACHE_MAX as u64 {
            assert_eq!(cached(&m, k, || k + 1), k + 1);
        }
        assert_eq!(m.borrow().len(), PATH_CACHE_MAX);
        // A hit does not insert.
        assert_eq!(cached(&m, 3, || unreachable!()), 4);
        assert_eq!(m.borrow().len(), PATH_CACHE_MAX);
        // The next miss clears, then holds only itself.
        assert_eq!(cached(&m, u64::MAX, || 7), 7);
        assert_eq!(m.borrow().len(), 1);
        assert_eq!(cached(&m, 3, || 9), 9);
    }

    #[test]
    fn alt_members_ascend_and_skip_the_active_one() {
        let v: Vec<u32> = alt_members(0b1011_0110, 8, Some(2)).collect();
        assert_eq!(v, [1, 4, 5, 7]);
        let v: Vec<u32> = alt_members(0b1011_0110, 5, None).collect();
        assert_eq!(v, [1, 2, 4]);
        let v: Vec<u32> = alt_members(u64::MAX, 70, Some(63)).collect();
        assert_eq!(v.len(), 63);
        assert_eq!(alt_members(1 << 3, 70, Some(64)).collect::<Vec<_>>(), [3]);
    }
}
