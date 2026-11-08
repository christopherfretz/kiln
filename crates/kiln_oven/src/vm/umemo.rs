//! The union-path memo (WP-760, spec/oven.md "Union-path memo"):
//! `Oven::union_path` answers remembered per (allocation, accessed type,
//! offset or offset region).
//!
//! An access to an allocation holding a union checks the active members
//! on the path from the allocation's root to the accessed object
//! ([class.union.general]/2, [expr.const]/5.10). The walk's `Ok` answer is
//! a pure function of the allocation's type, the offset, the accessed
//! type, the type table and the active-member maps; only a fault depends
//! on the access kind, and faults are never remembered. So an entry stays
//! valid while [`Memory::union_epoch`](crate::mem::Memory) (bumped by
//! every change to those maps) and [`Types::rebuilt`](crate::ty::Types)
//! are what they were when it was computed.
//!
//! An entry covers one offset, or a whole array without unions that the
//! walk reached after resolving every union by its active member: every
//! element of that array at the same position holds an object of the
//! accessed scalar type, and the walk toward any of them makes the same
//! choices, so one entry answers the whole array (an `inplace_vector`'s
//! `union { T data[N]; }` costs one walk, not one per element).
//!
//! The table is direct-mapped and fixed-size: a collision overwrites, so
//! the memo's memory does not grow with the evaluation.

use std::cell::Cell;

use crate::ty::OTy;

/// log2 of the number of entries.
const BITS: u32 = 9;

#[derive(Clone, Copy, Debug)]
struct Entry {
    /// [`Ptr::alloc_key`](crate::mem::Ptr::alloc_key) of the allocation.
    akey: u64,
    /// The union epoch the answer was computed at (`u64::MAX`: empty).
    epoch: u64,
    /// [`Types::rebuilt`](crate::ty::Types::rebuilt) at that time.
    rebuilt: u32,
    /// The allocation's type and the accessed type.
    aty: OTy,
    ty: OTy,
    /// The offsets covered: `lo + i * stride + phase` below `lo + len`.
    lo: u32,
    len: u32,
    stride: u32,
    phase: u32,
}

const EMPTY: Entry = Entry {
    akey: 0,
    epoch: u64::MAX,
    rebuilt: 0,
    aty: OTy(0),
    ty: OTy(0),
    lo: 0,
    len: 0,
    stride: 1,
    phase: 0,
};

/// The state an entry is valid in: the union epoch and the type table's
/// rebuild count.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Stamp {
    pub(crate) epoch: u64,
    pub(crate) rebuilt: u32,
}

/// The offsets a remembered `Ok` answer covers.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Cover {
    /// Exactly this offset.
    At(u32),
    /// Offsets `lo + i * stride + phase` below `lo + len` (an array's
    /// elements at one position).
    Region { lo: u32, len: u32, stride: u32, phase: u32 },
}

/// The memo's table (owned by the Oven; interior mutability, since
/// `union_path` reads memory through a shared reference).
#[derive(Debug)]
pub(crate) struct UnionMemo {
    e: Box<[Cell<Entry>]>,
}

impl Default for UnionMemo {
    fn default() -> Self {
        UnionMemo { e: (0..1usize << BITS).map(|_| Cell::new(EMPTY)).collect() }
    }
}

fn slot(akey: u64, ty: OTy, off: u64) -> usize {
    let h = (akey ^ u64::from(ty.0) << 24 ^ off << 40).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    (h >> (64 - BITS)) as usize
}

impl UnionMemo {
    /// Whether an `Ok` answer for the access of type `ty` at offset `off`
    /// of allocation `akey` (of type `aty`) is remembered at `stamp`.
    #[inline]
    pub(crate) fn hit(&self, akey: u64, aty: OTy, ty: OTy, off: u32, stamp: Stamp) -> bool {
        let m = |e: Entry| {
            let d = off.wrapping_sub(e.lo);
            e.epoch == stamp.epoch
                && e.akey == akey
                && e.aty == aty
                && e.ty == ty
                && e.rebuilt == stamp.rebuilt
                && d < e.len
                && d % e.stride == e.phase
        };
        m(self.e[slot(akey, ty, u64::MAX)].get()) || m(self.e[slot(akey, ty, off.into())].get())
    }

    /// Remembers an `Ok` answer.
    pub(crate) fn put(&self, akey: u64, aty: OTy, ty: OTy, cover: Cover, stamp: Stamp) {
        let (i, lo, len, stride, phase) = match cover {
            Cover::At(off) => (slot(akey, ty, off.into()), off, 1, 1, 0),
            Cover::Region { lo, len, stride, phase } => {
                (slot(akey, ty, u64::MAX), lo, len, stride, phase)
            }
        };
        let (epoch, rebuilt) = (stamp.epoch, stamp.rebuilt);
        self.e[i].set(Entry { akey, epoch, rebuilt, aty, ty, lo, len, stride, phase });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S0: Stamp = Stamp { epoch: 0, rebuilt: 0 };

    #[test]
    fn an_exact_entry_answers_only_its_offset_and_stamp() {
        let m = UnionMemo::default();
        let (a, t) = (OTy(3), OTy(4));
        assert!(!m.hit(7, a, t, 16, S0));
        m.put(7, a, t, Cover::At(16), S0);
        assert!(m.hit(7, a, t, 16, S0));
        assert!(!m.hit(7, a, t, 24, S0));
        assert!(!m.hit(8, a, t, 16, S0));
        assert!(!m.hit(7, a, OTy(5), 16, S0));
        assert!(!m.hit(7, a, t, 16, Stamp { epoch: 1, rebuilt: 0 }));
        assert!(!m.hit(7, a, t, 16, Stamp { epoch: 0, rebuilt: 1 }));
    }

    #[test]
    fn a_region_entry_answers_its_elements_at_one_position() {
        let m = UnionMemo::default();
        let (a, t) = (OTy(3), OTy(4));
        m.put(7, a, t, Cover::Region { lo: 8, len: 80, stride: 16, phase: 8 }, S0);
        assert!(m.hit(7, a, t, 16, S0));
        assert!(m.hit(7, a, t, 80, S0));
        assert!(!m.hit(7, a, t, 88, S0));
        assert!(!m.hit(7, a, t, 8, S0));
        assert!(!m.hit(7, a, t, 20, S0));
        assert!(!m.hit(7, a, t, 0, S0));
    }
}
