//! Union member designators (WP-1494; spec/oven.md "Union member
//! designators"). An Oven pointer is an allocation and an offset, so two
//! members of one union that can hold an object of the same type at the
//! same offset (`union { int a; int b; }`, `union { S s; S arr[2]; }`)
//! cannot be told apart by the pointer alone. The glvalue `u.b` names `b`
//! all the same ([basic.life]/7: a read through it while `a` is active is
//! a read of an object outside its lifetime, [expr.const]/10.9), so the
//! compiler tags the address of such a member (`VoidCast` aux
//! `kbc::UNION_DESIG`) with the member it designates: an interned
//! (union type, member index) pair in bits 96..128 of the pointer register.
//!
//! The tag travels with the register (copies, calls, returns, a reference
//! bound to the member, a conversion to `void *` and back) and is read by
//! the operations that act on the designated object as a whole: a load or
//! store through the pointer (checked only on the side path of an
//! allocation holding unions, `Oven::slow_access`), a placement new (which
//! constructs that member, not another of the same type) and an explicit
//! destruction (which ends that member). Every other pointer operation
//! reads the low 64 bits, and a store of the pointer to memory keeps no
//! tag: such a pointer is checked by offset and type, as before.
//! Untagged pointers pay nothing.

use kiln_source::fx::FxHashMap;

use super::Fault;
use crate::Oven;
use crate::mem::{MemFault, Ptr};
use crate::ty::OTy;

/// The interned designators: tag `i + 1` is `list[i]`. Append-only for the
/// translation unit (a tag lives in registers only, within one evaluation).
#[derive(Default, Debug)]
pub(crate) struct UnionDesigs {
    list: Vec<(OTy, u32)>,
    ix: FxHashMap<(OTy, u32), u32>,
}

impl UnionDesigs {
    /// The tag of member `idx` of union `u`.
    pub fn tag(&mut self, u: OTy, idx: u32) -> u32 {
        if let Some(&t) = self.ix.get(&(u, idx)) {
            return t;
        }
        self.list.push((u, idx));
        let t = self.list.len() as u32;
        self.ix.insert((u, idx), t);
        t
    }

    /// The (union type, member index) of tag `t` (nonzero).
    pub fn get(&self, t: u32) -> Option<(OTy, u32)> {
        self.list.get((t as usize).checked_sub(1)?).copied()
    }

    pub fn footprint(&self) -> usize {
        self.list.capacity() * size_of::<(OTy, u32)>()
            + self.ix.capacity() * (size_of::<((OTy, u32), u32)>() + 1)
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

impl Oven {
    /// An access `k` of the `ty` object at `raw` through a pointer whose
    /// tag `desig` names a union member: the member must be (possibly)
    /// active. Run after the access's own union walk succeeded, so a union
    /// with no entry (none on the path, or never active, which the walk
    /// reported) is left to it.
    #[cold]
    #[inline(never)]
    pub(super) fn desig_check(&self, raw: u64, desig: u32, ty: OTy, k: u32) -> Result<(), Fault> {
        let Some((u, idx)) = self.udesig.get(desig) else { return Ok(()) };
        let fields = self.types.fields(u);
        let Some(f) = fields.get(idx as usize) else { return Ok(()) };
        let Some(base) = u64::from(Ptr::decode(raw).off).checked_sub(f.offset) else {
            return Ok(());
        };
        let key = Ptr::union_key(raw, base, u);
        if self.mem.may_be_active(key, idx) {
            return Ok(());
        }
        let active = match (self.mem.active(key), self.mem.ended_member(key)) {
            (Some(a), _) => a,
            (None, Some(e)) if e == idx => {
                return Err(Fault::Mem { f: MemFault::Lifetime, ak: k, ty, p: raw });
            }
            (None, Some(e)) => e,
            (None, None) => return Ok(()),
        };
        Err(Fault::Union {
            ak: k,
            field: f.decl,
            active: fields.get(active as usize).map(|f| f.decl),
        })
    }

    /// The (union type, member index) a pointer's tag designates, if any.
    pub(super) fn desig(&self, desig: u32) -> Option<(OTy, u32)> {
        if desig == 0 { None } else { self.udesig.get(desig) }
    }
}
