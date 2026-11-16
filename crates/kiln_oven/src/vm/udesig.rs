//! Union member designators (WP-1494, WP-1508; spec/oven.md "Union member
//! designators"). An Oven pointer is an allocation and an offset, so two
//! members of one union that can hold an object of the same type at the
//! same offset (`union { int a; int b; }`, `union { S s; S arr[2]; }`,
//! `union { S s; int arr[2]; }` for `&u.s.k`) cannot be told apart by the
//! pointer alone. The glvalue `u.b` names `b` all the same ([basic.life]/7:
//! a read through it while `a` is active is a read of an object outside
//! its lifetime, [expr.const]/10.9), so the compiler tags the address of
//! such a subobject (`VoidCast` aux `kbc::UNION_DESIG`) with the union
//! members its member access chain goes through: an interned path of
//! (union type, member index) pairs, outermost first, in bits 96..128 of
//! the pointer register (the high half of the register's upper word,
//! which no other pointer state uses; bits 64..96 are the `void *` tag).
//!
//! The tag travels with the register (copies, calls, returns, a reference
//! bound to the member, a conversion to `void *` and back, a member or
//! base projection, pointer arithmetic: none leaves the designated
//! member's subobject in a constant expression) and through memory: a
//! stored pointer keeps it in its granule's pointer mark, interned with
//! the `void *` tag ([`crate::mem::Memory::set_void_tag`]; not in static
//! memory, which a PCH snapshot keeps while this table is rebuilt). It is
//! read by the operations that act on the designated object: a load or
//! store through the pointer (checked only on the side path of an
//! allocation holding unions, `Oven::slow_access`), a placement new (which
//! constructs that member, not another of the same type), an explicit
//! destruction (which ends that member) and a pointer subtraction (two
//! members are not one array, [expr.add]/5). Untagged pointers pay
//! nothing.

use kiln_source::fx::FxHashMap;

use super::Fault;
use crate::Oven;
use crate::mem::{MemFault, Ptr, UKey};
use crate::ty::{Hop, OTy, TyKind, Walk, alt_members};

/// One level of an interned path: its parent (the enclosing level's tag,
/// 0: none), the union type and the member index.
type Level = (u32, OTy, u32);

/// The interned designator paths: tag `i + 1` is `list[i]` below the
/// path of its parent tag. Append-only for the translation unit (a tag
/// lives in registers and in frame and heap memory, within one
/// evaluation's pointer marks or a later one's, never in static memory).
#[derive(Default, Debug)]
pub(crate) struct UnionDesigs {
    list: Vec<Level>,
    ix: FxHashMap<Level, u32>,
    /// The last extension and its result: a loop tags the same member
    /// on every iteration.
    last: Option<(Level, u32)>,
}

impl UnionDesigs {
    /// The tag of path `parent` extended with member `idx` of union `u`.
    /// A path holds one level per union type (a union cannot contain a
    /// union of its own type): `parent` is returned unchanged when it
    /// names `u` already.
    pub fn extend(&mut self, parent: u32, u: OTy, idx: u32) -> u32 {
        let key = (parent, u, idx);
        if let Some((k, t)) = self.last
            && k == key
        {
            return t;
        }
        let t = if self.member(parent, u).is_some() {
            parent
        } else if let Some(&t) = self.ix.get(&key) {
            t
        } else {
            self.list.push(key);
            let t = self.list.len() as u32;
            self.ix.insert(key, t);
            t
        };
        self.last = Some((key, t));
        t
    }

    /// The member of union type `u` that path `t` designates, if the path
    /// goes through a union of that type.
    pub fn member(&self, mut t: u32, u: OTy) -> Option<u32> {
        while let Some(&(parent, lu, idx)) = self.list.get((t as usize).checked_sub(1)?) {
            if lu == u {
                return Some(idx);
            }
            t = parent;
        }
        None
    }

    pub fn footprint(&self) -> usize {
        self.list.capacity() * size_of::<Level>()
            + self.ix.capacity() * (size_of::<(Level, u32)>() + 1)
    }
}

/// The designator tag of a pointer register value.
#[inline]
pub(crate) fn desig_of(v: u128) -> u32 {
    (v >> 96) as u32
}

/// `v` with designator tag `t` (bits 96..128 replaced).
#[inline]
pub(crate) fn with_desig(v: u128, t: u32) -> u128 {
    v & ((1u128 << 96) - 1) | u128::from(t) << 96
}

/// The designator bits of `v` alone (bits 96..128), to be or'ed into a
/// pointer derived from it: one shift pair, no branch.
#[inline]
pub(crate) fn desig_bits(v: u128) -> u128 {
    v >> 96 << 96
}

impl Oven {
    /// An access `k` of the `ty` object at `raw` through a pointer whose
    /// tag `desig` names union members: walking from the allocation root
    /// to the object, each union on the path whose type the tag names must
    /// have that member (possibly) active. A union the tag does not name
    /// is passed by its active member as the access's own walk does
    /// (`Oven::union_walk`), and a union with no member holding the object
    /// is left to that walk. Run before it, so the note names the
    /// designated member.
    #[cold]
    #[inline(never)]
    pub(super) fn desig_check(&self, raw: u64, desig: u32, ty: OTy, k: u32) -> Result<(), Fault> {
        let Some(a) = self.alloc_any(Ptr::decode(raw)) else { return Ok(()) };
        let target = u64::from(Ptr::decode(raw).off);
        let (mut t, mut base) = (a.ty, 0u64);
        loop {
            if base == target && self.types.same_object(t, ty) {
                return Ok(());
            }
            let info = self.types.info(t);
            if !info.has_union {
                return Ok(());
            }
            match info.kind {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let i = (target - base) / es;
                    if i >= n {
                        return Ok(());
                    }
                    base += i * es;
                    t = elem;
                }
                TyKind::Record { .. } => match self.types.hop(Walk::Union, t, target - base, ty) {
                    Hop::Reached | Hop::Stop | Hop::Mutable(_) => return Ok(()),
                    Hop::Array { at, t: arr } => {
                        base += at;
                        t = arr;
                    }
                    Hop::Union { at, u, reach } => {
                        base += at;
                        let rel = target - base;
                        let key = Ptr::union_key(raw, base, u);
                        let ok = |i: u32| self.types.reaches(Walk::Union, u, rel, ty, reach, i);
                        let i = match self.udesig.member(desig, u) {
                            Some(di) if ok(di) => {
                                if !self.mem.may_be_active(key, di) {
                                    return Err(self.desig_fault(raw, key, di, ty, k));
                                }
                                di
                            }
                            _ => {
                                let n = self.types.fields(u).len() as u32;
                                let active = self.mem.active(key);
                                let alts = self.mem.union_alts.get(&key).copied().unwrap_or(0);
                                let alt = alt_members(alts, n, active);
                                let Some(i) = active.into_iter().chain(alt).find(|&i| ok(i)) else {
                                    return Ok(());
                                };
                                i
                            }
                        };
                        let Some(f) = self.types.fields(u).get(i as usize) else { return Ok(()) };
                        if f.width.is_some() {
                            return Ok(());
                        }
                        base += f.offset;
                        t = f.ty;
                    }
                },
                _ => return Ok(()),
            }
        }
    }

    /// The fault of an access `k` of the `ty` object through member `idx`
    /// of union `key`, which is not (possibly) active: "outside its
    /// lifetime" when it is the union's ended member, else the union
    /// member note naming the active (or ended) member, if any.
    #[cold]
    fn desig_fault(&self, raw: u64, key: UKey, idx: u32, ty: OTy, k: u32) -> Fault {
        let fields = self.types.fields(key.ty);
        let active = match (self.mem.active(key), self.mem.ended_member(key)) {
            (Some(a), _) => Some(a),
            (None, Some(e)) if e == idx => {
                return Fault::Mem { f: MemFault::Lifetime, ak: k, ty, p: raw };
            }
            (None, e) => e,
        };
        let Some(f) = fields.get(idx as usize) else {
            return Fault::Mem { f: MemFault::Lifetime, ak: k, ty, p: raw };
        };
        Fault::Union {
            ak: k,
            field: f.decl,
            active: active.and_then(|a| fields.get(a as usize)).map(|f| f.decl),
        }
    }

    /// The fault of an access through a tagged pointer whose bytes are
    /// outside their lifetime (a member an explicit destruction ended
    /// before C++26): the designated member's fault comes first, so the
    /// note names the union's member as clang does ("read of member 'i'
    /// of union with active member 'm'"); any other fault stays.
    #[cold]
    #[inline(never)]
    pub(super) fn desig_first(&self, raw: u64, desig: u32, ty: OTy, k: u32, f: Fault) -> Fault {
        if desig == 0 || !matches!(f, Fault::Mem { f: MemFault::Lifetime, .. }) {
            return f;
        }
        match self.alloc_any(Ptr::decode(raw)) {
            Some(a) if a.flags & crate::mem::flags::HAS_UNION != 0 => {
                self.desig_check(raw, desig, ty, k).err().unwrap_or(f)
            }
            _ => f,
        }
    }

    /// The member of union type `u` that a pointer's tag designates, if
    /// any.
    pub(super) fn desig_member(&self, desig: u32, u: OTy) -> Option<u32> {
        if desig == 0 { None } else { self.udesig.member(desig, u) }
    }

    /// Whether two pointers into the same allocation designate different
    /// members of one union object: their tags name different members of
    /// a union type on both paths, and the union of that type that holds
    /// the first pointer's object holds the second's too
    /// ([expr.add]/5: then they are not elements of one array object).
    #[cold]
    #[inline(never)]
    pub(super) fn desig_apart(&self, p: u64, dp: u32, q: u64, dq: u32) -> bool {
        if dp == 0 || dq == 0 || dp == dq || Ptr::alloc_key(p) != Ptr::alloc_key(q) {
            return false;
        }
        let Some(a) = self.alloc_any(Ptr::decode(p)) else { return false };
        let (po, qo) = (u64::from(Ptr::decode(p).off), u64::from(Ptr::decode(q).off));
        // Walk the shared prefix of both positions: a union on it whose
        // members the tags name differently separates them.
        let (mut t, mut base) = (a.ty, 0u64);
        for _ in 0..64 {
            match self.types.kind(t) {
                TyKind::Array { elem, n } => {
                    let es = self.types.size(elem).max(1);
                    let (i, j) = (po.saturating_sub(base) / es, qo.saturating_sub(base) / es);
                    if i != j || i >= n {
                        return false;
                    }
                    base += i * es;
                    t = elem;
                }
                TyKind::Record { union, .. } => {
                    if union
                        && let (Some(i), Some(j)) =
                            (self.udesig.member(dp, t), self.udesig.member(dq, t))
                    {
                        if i != j {
                            return true;
                        }
                        let Some(f) = self.types.fields(t).get(i as usize) else { return false };
                        base += f.offset;
                        t = f.ty;
                        continue;
                    }
                    let contains = |o: u64, f: &crate::ty::FieldInfo| {
                        o >= base + f.offset && o < base + f.offset + self.types.size(f.ty).max(1)
                    };
                    let Some(f) = self
                        .types
                        .fields(t)
                        .iter()
                        .find(|f| f.width.is_none() && contains(po, f) && contains(qo, f))
                    else {
                        return false;
                    };
                    base += f.offset;
                    t = f.ty;
                }
                _ => return false,
            }
        }
        false
    }
}
