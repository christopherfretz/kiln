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

/// Hashes `v` with `FxHash`, then folds the high half into the low half
/// (spec/types.md §7.1). `FxHash` multiplies last, so bit `k` of its
/// result depends only on bits `0..=k` of the last word mixed in; the
/// table indexes buckets by the low bits, and a key written as one
/// 64-bit word (`[u32; 2]`, a `Pair` key's payload) would otherwise land
/// every key that differs only in its second `u32` in the same bucket
/// (WP-313: the specializations of one template, a probe chain as long
/// as their number).
#[inline]
pub(crate) fn fx_hash<T: Hash + ?Sized>(v: &T) -> u64 {
    let mut h = FxHasher::default();
    v.hash(&mut h);
    let h = h.finish();
    h ^ (h >> 32)
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

    /// The ids in `1..len` that are NOT in the table, ascending: what a
    /// snapshot writes so that [`Self::rebuild`] restores exactly this
    /// membership (spec/ast.md "Snapshots").
    pub(crate) fn absent(&self, len: usize) -> Vec<u32> {
        let mut present = vec![false; len];
        let mut mark = |id: u32| {
            if let Some(p) = present.get_mut(id as usize) {
                *p = true;
            }
        };
        #[cfg(not(feature = "std-intern"))]
        self.table.iter().for_each(|&id| mark(id));
        #[cfg(feature = "std-intern")]
        for &head in self.heads.values() {
            let mut cur = head;
            while cur != 0 {
                mark(cur);
                cur = self.next_same_hash.get(cur as usize).copied().unwrap_or(0);
            }
        }
        (1..len).filter(|&i| !present[i]).map(|i| i as u32).collect()
    }

    /// The table holding every id of `1..len` except `absent` (ascending,
    /// in range: the caller checked), inserted in id order with the hash
    /// `hash` gives (a snapshot's restore, spec/ast.md "Snapshots").
    pub(crate) fn rebuild(len: usize, absent: &[u32], hash: impl Fn(u32) -> u64) -> InternTable {
        timed_rebuild(|| Self::rebuild_untimed(len, absent, hash))
    }

    fn rebuild_untimed(len: usize, absent: &[u32], hash: impl Fn(u32) -> u64) -> InternTable {
        let mut t = InternTable::default();
        #[cfg(not(feature = "std-intern"))]
        {
            t.table.reserve(len.saturating_sub(1 + absent.len()), |&i| hash(i));
        }
        let mut skip = absent.iter().copied().peekable();
        for id in 1..len as u32 {
            if skip.next_if_eq(&id).is_some() {
                continue;
            }
            t.insert(hash(id), id, &hash);
        }
        t
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

    /// Keys that differ only in their second `u32` (one 64-bit word,
    /// `FxHash`'s last) spread over the low bits the table buckets by
    /// (spec/types.md §7.1, WP-313); unfolded, they all share one bucket.
    #[test]
    fn second_word_reaches_the_low_bits() {
        let low: std::collections::HashSet<u64> =
            (0..4096u32).map(|k| fx_hash(&[7u32, k]) & 0xfff).collect();
        assert!(low.len() > 2048, "{} distinct buckets of 4096", low.len());
    }
}

/// One entry of a [`KeyedTable`]: a four-word key, the entry's canonical
/// id and its flags.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
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
    pub(super) intern: InternTable,
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

thread_local! {
    /// The time this thread spent in [`InternTable::rebuild`] since the
    /// last [`take_rebuild_time`]: instrumentation of the snapshot
    /// prototype's load timer (WP-1383), no semantic state.
    static REBUILD_TIME: std::cell::Cell<std::time::Duration> =
        const { std::cell::Cell::new(std::time::Duration::ZERO) };
}

/// Runs `f`, a hash-consing table's rebuild from a snapshot, adding its
/// time to the thread's [`take_rebuild_time`] (WP-1383 load timer).
pub(crate) fn timed_rebuild<T>(f: impl FnOnce() -> T) -> T {
    let started = std::time::Instant::now();
    let t = f();
    REBUILD_TIME.with(|c| c.set(c.get() + started.elapsed()));
    t
}

/// The time this thread spent rebuilding hash-consing tables from a
/// snapshot since the last call, and resets it (WP-1383 load timer).
#[must_use]
pub fn take_rebuild_time() -> std::time::Duration {
    REBUILD_TIME.with(|c| c.replace(std::time::Duration::ZERO))
}
