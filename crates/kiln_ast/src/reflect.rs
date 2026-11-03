//! Reflections (spec/reflection.md §2).
//!
//! WP-267 spike (`-freflection`): the table interns the entity a
//! `std::meta::info` value designates. A [`ReflectionId`] is a dense index
//! into it, so a value the Oven hands back (a scalar of shadow kind
//! `Reflection`, frozen as `Scalar::Reflection`) is looked up again with
//! [`ReflectionTable::get`]. Ids are 1-based like every arena id; the
//! Oven's 8-byte value is `index - 1` ([`ReflectionId::bits`]), so id 1 is
//! the null reflection and an all-zero (value-initialized)
//! `std::meta::info` is null.
//!
//! Mapping (spec/reflection.md §2.2; the spike subset):
//! - a type-id reflects its `QualType` with alias sugar dropped (the
//!   spike canonicalizes every type; P2996 keeps `^^Alias` distinct);
//! - a variable, function, data member or namespace reflects its
//!   canonical `DeclId`.

use crate::ids::{DeclId, QualType, ReflectionId};
use crate::mem::{HeapSize, MemReport, MemRow};
use kiln_source::fx::FxHashMap;

/// The entity a reflection designates (the spike subset of
/// spec/reflection.md §2.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reflectee {
    /// The null reflection.
    Null,
    /// A type (canonical).
    Type(QualType),
    /// A declaration: variable, function, field, namespace or class
    /// (canonical declaration).
    Decl(DeclId),
}

/// Interned reflections (spec/reflection.md §2.2).
#[derive(Debug)]
pub struct ReflectionTable {
    items: Vec<Reflectee>,
    map: FxHashMap<Reflectee, ReflectionId>,
}

impl Default for ReflectionTable {
    fn default() -> Self {
        let mut map = FxHashMap::default();
        map.insert(Reflectee::Null, ReflectionId::new(1));
        ReflectionTable { items: vec![Reflectee::Null], map }
    }
}

impl ReflectionTable {
    /// The id of `r`, interning it.
    ///
    /// # Panics
    ///
    /// If more than `u32::MAX - 1` reflections are interned.
    pub fn intern(&mut self, r: Reflectee) -> ReflectionId {
        if let Some(&id) = self.map.get(&r) {
            return id;
        }
        let id =
            ReflectionId::new(u32::try_from(self.items.len() + 1).expect("reflection table full"));
        self.items.push(r);
        self.map.insert(r, id);
        id
    }

    /// The entity `id` designates (`Null` for an id this table did not
    /// hand out, e.g. a raw value forged by a bit cast).
    pub fn get(&self, id: ReflectionId) -> Reflectee {
        self.items.get(id.index() as usize - 1).copied().unwrap_or(Reflectee::Null)
    }

    /// The number of interned reflections (including null).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Always false: the null reflection is always present.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl HeapSize for ReflectionTable {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("items", &self.items));
        r.push(MemRow::of_map("map", &self.map));
        r
    }
}

impl ReflectionId {
    /// The null reflection (a value-initialized `std::meta::info`).
    pub const NULL: ReflectionId = ReflectionId::new(1);

    /// The Oven's 8-byte value of this reflection (`index - 1`, so null
    /// is all zeros).
    pub fn bits(self) -> u64 {
        u64::from(self.index() - 1)
    }

    /// The reflection whose Oven value is `v` (`None` past the id space:
    /// a value no table handed out).
    pub fn from_bits(v: u128) -> Option<ReflectionId> {
        u32::try_from(v).ok().and_then(|v| v.checked_add(1)).and_then(ReflectionId::try_new)
    }
}
