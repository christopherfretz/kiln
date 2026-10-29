//! Class-level side tables computed after a class is complete
//! (spec/sema-class.md §3.5, §8.1): the subobject graph and final
//! overriders (`Ast::virtuals`, computed by sema) and Itanium vtable
//! layouts (`Ast::vtables`, computed by `kiln_abi::vtable`).
//!
//! The data types live here rather than in `kiln_abi` because the [`Ast`]
//! stores them and `kiln_ast` does not depend on `kiln_abi`;
//! `kiln_abi::vtable` re-exports them. Both caches are insert-only, like
//! `Ast::layouts`.
//!
//! [`Ast`]: crate::Ast
#![warn(missing_docs)]

use kiln_source::fx::FxHashMap;

use crate::ids::DeclId;
use crate::mem::{HeapSize, MemReport, MemRow};

/// A base-class subobject of a most-derived class, numbered in preorder:
/// 0 is the most-derived class, then each direct non-virtual base's subtree
/// in declaration order, then each virtual base (once) in Itanium
/// inheritance-graph order with its non-virtual subtree (spec/sema-class.md
/// §3.5). The shared currency of sema, the Oven and codegen.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct SubobjectId(pub u16);

impl SubobjectId {
    /// The most-derived object itself.
    pub const COMPLETE: SubobjectId = SubobjectId(0);

    /// The index into [`ClassVirtuals::subobjects`].
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One node of the subobject graph.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Subobject {
    /// The class of this subobject.
    pub class: DeclId,
    /// The subobject this one is a direct base of (itself for 0; for a
    /// virtual base, the first subobject in preorder naming it).
    pub parent: SubobjectId,
    /// The index of the base-specifier in `parent`'s class (0 for the
    /// most-derived class).
    pub base_index: u16,
    /// A virtual base subobject.
    pub is_virtual: bool,
}

/// The subobjects and final overriders of a most-derived polymorphic class
/// ([class.virtual]/2–3).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ClassVirtuals {
    /// The subobject graph, indexed by [`SubobjectId`].
    pub subobjects: Vec<Subobject>,
    /// `(subobject, virtual method) → (final overrider, its subobject)`,
    /// sorted by the first two components.
    pub overriders: Vec<(SubobjectId, DeclId, DeclId, SubobjectId)>,
}

impl ClassVirtuals {
    /// The final overrider of `method` (a virtual member of the class of
    /// subobject `at`), if recorded.
    pub fn final_overrider(
        &self,
        at: SubobjectId,
        method: DeclId,
    ) -> Option<(DeclId, SubobjectId)> {
        self.overriders
            .binary_search_by(|e| (e.0, e.1).cmp(&(at, method)))
            .ok()
            .map(|i| (self.overriders[i].2, self.overriders[i].3))
    }

    fn heap(&self) -> u64 {
        (self.subobjects.capacity() * size_of::<Subobject>()
            + self.overriders.capacity() * size_of::<(SubobjectId, DeclId, DeclId, SubobjectId)>())
            as u64
    }
}

/// One vtable slot (Itanium C++ ABI §2.5.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VTableComponent {
    /// A vcall offset.
    VCallOffset(i64),
    /// A virtual-base offset.
    VBaseOffset(i64),
    /// The offset to the top of the complete object.
    OffsetToTop(i64),
    /// The RTTI pointer of the class.
    Rtti(DeclId),
    /// A virtual function pointer.
    Func(DeclId),
    /// The complete-object destructor (D1).
    CompleteDtor(DeclId),
    /// The deleting destructor (D0).
    DeletingDtor(DeclId),
    /// A slot whose function is never called (e.g. overridden in a
    /// construction vtable); emitted as null / `__cxa_pure_virtual`.
    UnusedFunc(DeclId),
}

/// A `this`/return adjustment thunk (Itanium §2.7; clang `ThunkInfo`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Thunk {
    /// `this` adjustment: (non-virtual offset, vcall-offset offset; 0 =
    /// none).
    pub this: (i64, i64),
    /// Return adjustment for covariant returns: (non-virtual offset,
    /// vbase-offset offset; 0 = none).
    pub ret: (i64, i64),
    /// The method whose signature the thunk has, when it is not the
    /// overrider's own (clang `ThunkInfo::Method`): the overridden
    /// method of a return-adjusting slot, or the original method of an
    /// inherited `this`-adjusting slot.
    pub method: Option<DeclId>,
    /// The class of the thunk's `this` parameter (clang
    /// `ThunkInfo::ThisType`, whose cv-qualifiers are the overrider's).
    pub this_class: Option<DeclId>,
}

impl Thunk {
    /// No adjustment at all (clang `ThunkInfo::isEmpty`).
    pub const fn is_empty(&self) -> bool {
        self.this.0 == 0 && self.this.1 == 0 && self.ret.0 == 0 && self.ret.1 == 0
    }
}

/// An Itanium vtable group of a dynamic class (spec/sema-class.md §3.5).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct VTableLayout {
    /// The slots of the primary and all secondary vtables.
    pub components: Vec<VTableComponent>,
    /// The first component of each vtable of the group, ascending (0
    /// first; clang `VTableIndices`).
    pub vtable_starts: Vec<u32>,
    /// The address point (component index) of each subobject's vtable,
    /// sorted by subobject. Subobjects are those of the class the group
    /// is laid out in (the most-derived class of a construction vtable).
    pub address_points: Vec<(SubobjectId, u32)>,
    /// Component index → its thunk, ascending.
    pub thunks: Vec<(u32, Thunk)>,
    /// Overrider declared in the class → the thunks to emit for it
    /// (clang `Thunks`), sorted by method; empty for construction
    /// vtables.
    pub method_thunks: Vec<(DeclId, Thunk)>,
    /// Virtual base → the offset (from the address point) of its
    /// vbase-offset slot.
    pub vbase_offset_offsets: Vec<(DeclId, i64)>,
    /// Virtual method → its slot index from the primary address point,
    /// sorted by method. A destructor's entry is its complete-object
    /// slot; the deleting destructor is the next one.
    pub method_indices: Vec<(DeclId, u32)>,
}

impl VTableLayout {
    fn heap(&self) -> u64 {
        (self.components.capacity() * size_of::<VTableComponent>()
            + self.vtable_starts.capacity() * size_of::<u32>()
            + self.address_points.capacity() * size_of::<(SubobjectId, u32)>()
            + self.thunks.capacity() * size_of::<(u32, Thunk)>()
            + self.method_thunks.capacity() * size_of::<(DeclId, Thunk)>()
            + self.vbase_offset_offsets.capacity() * size_of::<(DeclId, i64)>()
            + self.method_indices.capacity() * size_of::<(DeclId, u32)>()) as u64
    }
}

/// One VTT entry (Itanium §2.6.2): the address point of `base`'s vtable in
/// vtable `vtable_index` (0 = the class's own vtable group, `i > 0` = the
/// `i`-th construction vtable group).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VttComponent {
    /// Which vtable group.
    pub vtable_index: u32,
    /// The subobject whose address point the entry holds.
    pub base: SubobjectId,
}

/// An insert-only per-class cache (`Ast::virtuals`, `Ast::vtables`).
#[derive(Clone, Debug)]
pub struct ClassCache<T> {
    map: FxHashMap<DeclId, Box<T>>,
}

impl<T> Default for ClassCache<T> {
    fn default() -> Self {
        Self { map: FxHashMap::default() }
    }
}

impl<T> ClassCache<T> {
    /// Stores the entry of `record`. Inserting twice is a logic error
    /// (`debug_assert!`); in release builds the first entry is kept.
    pub fn insert(&mut self, record: DeclId, v: T) {
        let prev = self.map.insert(record, Box::new(v));
        debug_assert!(prev.is_none(), "class cache entry of {record:?} computed twice");
        if let Some(p) = prev {
            self.map.insert(record, p);
        }
    }

    /// The cached entry of `record`.
    pub fn get(&self, record: DeclId) -> Option<&T> {
        self.map.get(&record).map(|b| &**b)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn report(&self, heap: impl Fn(&T) -> u64) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_map("map", &self.map));
        let boxed: u64 = self.map.values().map(|v| size_of::<T>() as u64 + heap(v)).sum();
        r.push(MemRow::new("entries", self.map.len() as u64, boxed, boxed));
        r
    }
}

impl HeapSize for ClassCache<ClassVirtuals> {
    fn heap_bytes(&self) -> MemReport {
        self.report(ClassVirtuals::heap)
    }
}

impl HeapSize for ClassCache<VTableLayout> {
    fn heap_bytes(&self) -> MemReport {
        self.report(VTableLayout::heap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_overrider_binary_search() {
        let d = |i| DeclId::new(i);
        let cv = ClassVirtuals {
            subobjects: vec![Subobject {
                class: d(1),
                parent: SubobjectId(0),
                base_index: 0,
                is_virtual: false,
            }],
            overriders: vec![
                (SubobjectId(0), d(2), d(2), SubobjectId(0)),
                (SubobjectId(1), d(5), d(7), SubobjectId(0)),
            ],
        };
        assert_eq!(cv.final_overrider(SubobjectId(1), d(5)), Some((d(7), SubobjectId(0))));
        assert_eq!(cv.final_overrider(SubobjectId(1), d(2)), None);
        let mut c = ClassCache::default();
        c.insert(d(1), cv);
        assert!(c.get(d(1)).is_some() && c.get(d(2)).is_none());
        assert_eq!(c.heap_bytes().rows.len(), 2);
    }
}
