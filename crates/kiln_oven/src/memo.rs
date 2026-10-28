//! The memoization cache (spec/oven.md "Memoization"): a bounded LRU of
//! the results of calls to pure functions with scalar arguments and a
//! scalar result, keyed by `(function, arguments)`.
//!
//! An entry records what the call cost (steps, calls, bytes, relative
//! peak memory and depth), so a hit charges exactly what executing the
//! call would have: every counter, and every budget decision, is the same
//! whether the cache is warm or cold. The table is allocated once; a full
//! cache evicts its least recently used entry, so steady-state use
//! allocates nothing.

use kiln_source::fx::FxHashMap;

/// Most arguments a memoized function may take.
pub(crate) const MAX_ARGS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct MemoKey {
    pub func: u32,
    pub args: [u128; MAX_ARGS],
}

/// What a call produced and cost.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MemoVal {
    pub result: u128,
    pub steps: u64,
    pub calls: u64,
    pub bytes: u64,
    /// Peak frame bytes above the live bytes at the call.
    pub peak: u64,
    /// Deepest nesting below the call (1: the call itself).
    pub depth: u32,
}

const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
struct Entry {
    key: MemoKey,
    val: MemoVal,
    prev: u32,
    next: u32,
}

#[derive(Debug)]
pub(crate) struct Memo {
    cap: usize,
    map: FxHashMap<MemoKey, u32>,
    entries: Vec<Entry>,
    /// Most recently used.
    head: u32,
    /// Least recently used.
    tail: u32,
    pub hits: u64,
    pub misses: u64,
}

impl Memo {
    pub fn new(cap: usize) -> Self {
        Memo {
            cap,
            map: FxHashMap::default(),
            entries: Vec::new(),
            head: NONE,
            tail: NONE,
            hits: 0,
            misses: 0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.cap > 0
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    fn unlink(&mut self, i: u32) {
        let Entry { prev, next, .. } = self.entries[i as usize];
        if prev == NONE {
            self.head = next;
        } else {
            self.entries[prev as usize].next = next;
        }
        if next == NONE {
            self.tail = prev;
        } else {
            self.entries[next as usize].prev = prev;
        }
    }

    fn push_front(&mut self, i: u32) {
        self.entries[i as usize].prev = NONE;
        self.entries[i as usize].next = self.head;
        if self.head != NONE {
            self.entries[self.head as usize].prev = i;
        }
        self.head = i;
        if self.tail == NONE {
            self.tail = i;
        }
    }

    pub fn get(&mut self, key: &MemoKey) -> Option<MemoVal> {
        let Some(&i) = self.map.get(key) else {
            self.misses += 1;
            return None;
        };
        self.hits += 1;
        if self.head != i {
            self.unlink(i);
            self.push_front(i);
        }
        Some(self.entries[i as usize].val)
    }

    pub fn insert(&mut self, key: MemoKey, val: MemoVal) {
        if self.cap == 0 {
            return;
        }
        if self.map.capacity() < self.cap {
            self.map.reserve(self.cap);
            self.entries.reserve_exact(self.cap);
        }
        if let Some(&i) = self.map.get(&key) {
            self.entries[i as usize].val = val;
            return;
        }
        let i = if self.entries.len() < self.cap {
            let i = self.entries.len() as u32;
            self.entries.push(Entry { key, val, prev: NONE, next: NONE });
            i
        } else {
            let i = self.tail;
            self.unlink(i);
            let old = self.entries[i as usize].key;
            self.map.remove(&old);
            self.entries[i as usize] = Entry { key, val, prev: NONE, next: NONE };
            i
        };
        self.map.insert(key, i);
        self.push_front(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(n: u128) -> MemoKey {
        MemoKey { func: 0, args: [n, 0, 0, 0] }
    }

    // spec/oven.md "Memoization": the cache is a bounded LRU.
    #[test]
    fn lru_evicts_least_recent() {
        let mut m = Memo::new(2);
        m.insert(k(1), MemoVal { result: 1, ..MemoVal::default() });
        m.insert(k(2), MemoVal { result: 2, ..MemoVal::default() });
        assert_eq!(m.get(&k(1)).map(|v| v.result), Some(1));
        m.insert(k(3), MemoVal { result: 3, ..MemoVal::default() });
        assert!(m.get(&k(2)).is_none());
        assert_eq!(m.get(&k(1)).map(|v| v.result), Some(1));
        assert_eq!(m.get(&k(3)).map(|v| v.result), Some(3));
        assert_eq!(m.len(), 2);
    }
}
