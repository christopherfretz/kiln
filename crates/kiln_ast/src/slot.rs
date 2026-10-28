//! Write-once slots and cold side maps (spec/ast.md §2.4, §4.2).
//!
//! The mutation discipline of §2.4 is enforced by API: every late-bound
//! slot has a setter that refuses a second write with [`AlreadySet`]
//! (callers that consider a second write a bug `expect` it, which panics
//! with the slot's name). Cold data (fields used by few nodes of a kind)
//! lives in [`ColdMap`]s, which are insert-only.
#![warn(missing_docs)]

use std::collections::hash_map::Entry;
use std::fmt;
use std::hash::Hash;

use kiln_source::fx::FxHashMap;

use crate::mem::MemRow;

/// A second write to a write-once slot (spec/ast.md §2.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AlreadySet {
    /// The slot, e.g. `"FunctionData.body"`.
    pub slot: &'static str,
    /// The raw value already stored.
    pub existing: u32,
}

impl fmt::Display for AlreadySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "write-once slot {} already set (to raw value {})", self.slot, self.existing)
    }
}

impl std::error::Error for AlreadySet {}

/// Stores `new` into an `Option` slot that must still be empty.
pub(crate) fn set_once<T: Copy>(
    slot: &mut Option<T>,
    new: T,
    name: &'static str,
    raw: impl Fn(T) -> u32,
) -> Result<(), AlreadySet> {
    if let Some(old) = *slot {
        return Err(AlreadySet { slot: name, existing: raw(old) });
    }
    *slot = Some(new);
    Ok(())
}

/// An insert-only sparse map (`FxHashMap`) for cold per-node data.
#[derive(Clone, Debug)]
pub struct ColdMap<K, V> {
    name: &'static str,
    map: FxHashMap<K, V>,
}

impl<K: Copy + Eq + Hash, V> ColdMap<K, V> {
    /// An empty map whose writes report `name` on conflicts.
    pub fn new(name: &'static str) -> Self {
        Self { name, map: FxHashMap::default() }
    }

    /// The value for `k`.
    pub fn get(&self, k: K) -> Option<&V> {
        self.map.get(&k)
    }

    /// True if `k` has a value.
    pub fn contains(&self, k: K) -> bool {
        self.map.contains_key(&k)
    }

    /// Stores `v` for `k`; refuses if `k` already has a value.
    pub fn insert(&mut self, k: K, v: V) -> Result<(), AlreadySet> {
        match self.map.entry(k) {
            Entry::Occupied(_) => Err(AlreadySet { slot: self.name, existing: 0 }),
            Entry::Vacant(e) => {
                e.insert(v);
                Ok(())
            }
        }
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The entries, in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.map.iter().map(|(k, v)| (*k, v))
    }

    /// The memory row of this map (named after the map).
    pub fn mem_row(&self) -> MemRow {
        MemRow::of_map(self.name, &self.map)
    }
}
