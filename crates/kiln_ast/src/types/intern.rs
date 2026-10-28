//! The hash-consing table (spec/types.md §7.1).
//!
//! An [`InternTable`] stores only `u32` ids; the keys live in the owner's
//! arenas and are re-read on probe (through the `eq` closure) and on
//! growth (through the `rehash` closure), so no key is stored twice.
//!
//! By default the table is a `hashbrown::HashTable<u32>` (≈ 4.6 B per
//! entry at 7/8 load, plus one control byte). With the `std-intern`
//! feature, the std-only fallback maps each hash to the
//! most recent id with that hash and chains older ids through a
//! `next_same_hash` vector indexed by id (spec/types.md §7.1, §17 T1).

use std::hash::{Hash, Hasher};

use kiln_source::fx::FxHasher;

/// Hashes `v` with `FxHash`.
#[inline]
pub(crate) fn fx_hash<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut h = FxHasher::default();
    v.hash(&mut h);
    h.finish()
}

/// An id-only hash table over keys stored elsewhere.
#[derive(Default)]
pub(crate) struct InternTable {
    #[cfg(not(feature = "std-intern"))]
    table: hashbrown::HashTable<u32>,
    #[cfg(feature = "std-intern")]
    heads: std::collections::HashMap<u64, u32, kiln_source::fx::FxBuildHasher>,
    #[cfg(feature = "std-intern")]
    next_same_hash: Vec<u32>,
    #[cfg(feature = "std-intern")]
    len: usize,
}

impl std::fmt::Debug for InternTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InternTable").field("len", &self.len()).finish()
    }
}

impl InternTable {
    /// Finds the id with hash `hash` for which `eq` holds.
    #[inline]
    pub(crate) fn find(&self, hash: u64, mut eq: impl FnMut(u32) -> bool) -> Option<u32> {
        #[cfg(not(feature = "std-intern"))]
        {
            self.table.find(hash, |&id| eq(id)).copied()
        }
        #[cfg(feature = "std-intern")]
        {
            let mut cur = self.heads.get(&hash).copied().unwrap_or(0);
            while cur != 0 {
                if eq(cur) {
                    return Some(cur);
                }
                cur = self.next_same_hash.get(cur as usize).copied().unwrap_or(0);
            }
            None
        }
    }

    /// Inserts `id` (not present, non-zero) with hash `hash`. `rehash`
    /// recomputes the hash of a stored id when the table grows.
    #[inline]
    pub(crate) fn insert(&mut self, hash: u64, id: u32, rehash: impl Fn(u32) -> u64) {
        #[cfg(not(feature = "std-intern"))]
        {
            self.table.insert_unique(hash, id, |&i| rehash(i));
        }
        #[cfg(feature = "std-intern")]
        {
            let _ = &rehash;
            let idx = id as usize;
            if self.next_same_hash.len() <= idx {
                self.next_same_hash.resize(idx + 1, 0);
            }
            let prev = self.heads.insert(hash, id).unwrap_or(0);
            self.next_same_hash[idx] = prev;
            self.len += 1;
        }
    }

    /// Number of entries.
    pub(crate) fn len(&self) -> usize {
        #[cfg(not(feature = "std-intern"))]
        {
            self.table.len()
        }
        #[cfg(feature = "std-intern")]
        {
            self.len
        }
    }

    /// Bytes of heap held by the table (allocation, not just entries).
    pub(crate) fn heap_bytes(&self) -> u64 {
        #[cfg(not(feature = "std-intern"))]
        {
            self.table.allocation_size() as u64
        }
        #[cfg(feature = "std-intern")]
        {
            // std's SwissTable: (key + value) per bucket plus one control
            // byte; buckets are the next power of two above capacity·8/7.
            let cap = self.heads.capacity();
            let buckets = if cap == 0 { 0 } else { (cap * 8 / 7).next_power_of_two() };
            (buckets * (size_of::<(u64, u32)>() + 1) + self.next_same_hash.capacity() * 4) as u64
        }
    }

    /// Bytes of the live entries (ids only), for the `live` column.
    pub(crate) fn live_bytes(&self) -> u64 {
        (self.len() * size_of::<u32>()) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::{InternTable, fx_hash};

    #[test]
    fn find_insert_and_collisions() {
        let keys: Vec<u64> = (0..1000).map(|i| i * 7).collect();
        let mut t = InternTable::default();
        // Force collisions: hash only the key's low 3 bits.
        let h = |k: u64| fx_hash(&(k & 7));
        for (i, &k) in keys.iter().enumerate() {
            let id = i as u32 + 1;
            assert_eq!(t.find(h(k), |j| keys[j as usize - 1] == k), None);
            t.insert(h(k), id, |j| h(keys[j as usize - 1]));
        }
        assert_eq!(t.len(), 1000);
        for (i, &k) in keys.iter().enumerate() {
            assert_eq!(t.find(h(k), |j| keys[j as usize - 1] == k), Some(i as u32 + 1));
        }
        assert!(t.heap_bytes() >= t.live_bytes());
    }
}

/// One entry of a [`KeyedTable`]: a four-word key, the entry's canonical
/// id and its flags.
#[derive(Clone, Copy, Debug)]
pub(crate) struct KeyedEntry {
    pub(crate) key: [u32; 4],
    pub(crate) canon: u32,
    pub(crate) flags: u8,
}

/// An interned table of fixed four-word keys (template names,
/// nested-name-specifiers). Index 0 is a dummy.
#[derive(Debug)]
pub(crate) struct KeyedTable {
    pub(crate) entries: Vec<KeyedEntry>,
    intern: InternTable,
}

impl KeyedTable {
    pub(crate) fn new() -> KeyedTable {
        KeyedTable {
            entries: vec![KeyedEntry { key: [0; 4], canon: 0, flags: 0 }],
            intern: InternTable::default(),
        }
    }

    pub(crate) fn find(&self, key: &[u32; 4]) -> Result<u32, u64> {
        let hash = fx_hash(key);
        let entries = &self.entries;
        self.intern.find(hash, |id| entries[id as usize].key == *key).ok_or(hash)
    }

    /// Pushes a new entry (the caller found `key` absent; `hash` is from
    /// [`Self::find`]). `canon == None` makes the entry its own canonical
    /// form. `None` when the id space is exhausted.
    pub(crate) fn push(
        &mut self,
        key: [u32; 4],
        hash: u64,
        flags: u8,
        canon: Option<u32>,
    ) -> Option<u32> {
        let id = u32::try_from(self.entries.len()).ok().filter(|&i| i != u32::MAX)?;
        self.entries.push(KeyedEntry { key, canon: canon.unwrap_or(id), flags });
        let entries = &self.entries;
        self.intern.insert(hash, id, |i| fx_hash(&entries[i as usize].key));
        Some(id)
    }

    #[inline]
    pub(crate) fn get(&self, id: u32) -> &KeyedEntry {
        &self.entries[id as usize]
    }

    pub(crate) fn memory(&self, name: &str, r: &mut crate::mem::MemReport) {
        use crate::mem::MemRow;
        r.push(MemRow::of_vec(format!("{name}.entries"), &self.entries));
        r.push(MemRow::new(
            format!("{name}.intern"),
            self.intern.len() as u64,
            self.intern.live_bytes(),
            self.intern.heap_bytes(),
        ));
    }
}
