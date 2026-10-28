//! The identifier interner: [`Symbol`] and [`Interner`].
//!
//! Lives in `kiln_source` (the leaf crate) so that `kiln_ast` can store
//! symbols without depending on the lexer; `kiln_lex` re-exports both
//! types (spec/types.md §17 T2).
//!
//! One interner is shared by the lexer, the preprocessor and semantic
//! analysis, so it is used through `&Interner` and is `Sync`. Interned
//! strings never move: a lock-free append-only arena maps `Symbol -> &str`
//! and a mutex-protected hash map maps `&str -> Symbol`. Lookups by symbol
//! take no lock. A small per-thread direct-mapped cache in front of the map
//! answers repeated `intern` calls without taking the lock: a hit is
//! verified against the arena, so it is never wrong (see [`Interner::intern`]).

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use crate::fx::FxBuildHasher;

/// An interned identifier: a 4-byte handle into an [`Interner`].
///
/// Two symbols from the same interner are equal exactly when their strings
/// are equal. Symbols from different interners must not be mixed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

impl Symbol {
    /// The symbol's index (dense, starting at 0, in interning order).
    pub const fn index(self) -> u32 {
        self.0
    }

    /// The symbol with dense index `index`: the inverse of [`Symbol::index`],
    /// for decoding packed encodings (such as `kiln_ast`'s `DeclName`).
    /// The result is only meaningful for the interner that produced
    /// `index`; [`Interner::as_str`] panics on an index it never issued.
    pub const fn from_index(index: u32) -> Symbol {
        Symbol(index)
    }

    /// The interned string. Equivalent to [`Interner::as_str`].
    ///
    /// # Panics
    /// If `self` was not created by `interner`.
    pub fn as_str(self, interner: &Interner) -> &str {
        interner.as_str(self)
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

type FxBuild = FxBuildHasher;

const FIRST_CHUNK_BITS: u32 = 8;
const CHUNKS: usize = (u32::BITS - FIRST_CHUNK_BITS + 1) as usize;

fn locate(idx: u32) -> (usize, usize) {
    let n = (u64::from(idx) >> FIRST_CHUNK_BITS) + 1;
    let k = n.ilog2();
    let start = ((1u64 << k) - 1) << FIRST_CHUNK_BITS;
    (k as usize, (u64::from(idx) - start) as usize)
}

/// One chunk of the arena: slots filled once, in order.
type Chunk = Box<[OnceLock<Arc<str>>]>;

/// Append-only `Symbol -> string` storage readable without locking.
struct Arena {
    len: AtomicU32,
    chunks: [OnceLock<Chunk>; CHUNKS],
}

impl Arena {
    fn new() -> Self {
        Self { len: AtomicU32::new(0), chunks: [const { OnceLock::new() }; CHUNKS] }
    }

    /// Callers serialize pushes (the interner's map mutex).
    fn push(&self, s: Arc<str>) -> u32 {
        let idx = self.len.load(Ordering::Relaxed);
        let (k, slot) = locate(idx);
        let chunk = self.chunks[k].get_or_init(|| {
            (0..1usize << (FIRST_CHUNK_BITS as usize + k)).map(|_| OnceLock::new()).collect()
        });
        if chunk[slot].set(s).is_err() {
            unreachable!("interner arena pushes are serialized by the map lock");
        }
        self.len.store(idx + 1, Ordering::Release);
        idx
    }

    fn get(&self, idx: u32) -> Option<&str> {
        if idx >= self.len.load(Ordering::Acquire) {
            return None;
        }
        let (k, slot) = locate(idx);
        self.chunks[k].get()?.get(slot)?.get().map(|s| &**s)
    }
}

/// Slots of the per-thread intern cache (a power of two).
const CACHE_SLOTS: usize = 4096;
/// An empty cache slot (`u32::MAX` is never a symbol).
const CACHE_EMPTY: u64 = u64::MAX;

thread_local! {
    /// Direct-mapped `hash -> (tag, symbol)` cache shared by every interner
    /// used on this thread. A slot holds `tag << 32 | symbol`, `tag` being
    /// the high half of the string's hash. Entries are hints only: a hit is
    /// accepted only if the interner's arena holds exactly the looked-up
    /// string at that symbol, so slots written by another interner (or
    /// stale ones) can cost a probe but never give a wrong answer.
    static INTERN_CACHE: [Cell<u64>; CACHE_SLOTS] =
        const { [const { Cell::new(CACHE_EMPTY) }; CACHE_SLOTS] };
}

/// A cheap hash for the cache only (the map keeps `FxHash`): the first
/// and last eight bytes and the length, folded by one wide multiply.
/// Weak keys only cost cache misses, never wrong answers.
fn str_hash(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let len = bytes.len();
    // Fixed-size reads only: a variable-length copy is a `memcpy` call.
    let word8 = |at: usize| {
        let mut w = [0u8; 8];
        w.copy_from_slice(&bytes[at..at + 8]);
        u64::from_le_bytes(w)
    };
    let word4 = |at: usize| {
        let mut w = [0u8; 4];
        w.copy_from_slice(&bytes[at..at + 4]);
        u64::from(u32::from_le_bytes(w))
    };
    let (head, tail) = match len {
        8.. => (word8(0), word8(len - 8)),
        4..8 => (word4(0), word4(len - 4)),
        1..4 => {
            let three = u64::from(bytes[0]) << 16
                | u64::from(bytes[len / 2]) << 8
                | u64::from(bytes[len - 1]);
            (three, 0)
        }
        0 => (0, 0),
    };
    let len_mix = (len as u64).wrapping_mul(0xe703_7ed1_a0b4_28db);
    let product = u128::from(head ^ 0xa076_1d64_78bd_642f)
        * u128::from(tail ^ len_mix ^ 0x8ebc_6af0_9c88_c6e3);
    #[allow(clippy::cast_possible_truncation, reason = "folding the halves")]
    let folded = (product as u64) ^ (product >> 64) as u64;
    folded
}

fn cache_slot(hash: u64) -> usize {
    #[allow(clippy::cast_possible_truncation, reason = "masked to CACHE_SLOTS")]
    let i = hash as usize;
    i & (CACHE_SLOTS - 1)
}

/// A thread-safe string interner, shared through `&Interner`.
pub struct Interner {
    map: Mutex<HashMap<Arc<str>, Symbol, FxBuild>>,
    arena: Arena,
}

impl Interner {
    /// Creates an empty interner.
    pub fn new() -> Self {
        Self { map: Mutex::new(HashMap::default()), arena: Arena::new() }
    }

    /// Interns `s`, returning its symbol (allocating only the first time
    /// `s` is seen).
    ///
    /// # Panics
    /// If more than `u32::MAX` distinct strings are interned.
    pub fn intern(&self, s: &str) -> Symbol {
        let hash = str_hash(s);
        if let Some(sym) = self.cached(s, hash) {
            return sym;
        }
        let sym = {
            let mut map = self.map.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(&sym) = map.get(s) {
                sym
            } else {
                assert!(self.arena.len.load(Ordering::Relaxed) < u32::MAX, "interner exhausted");
                let owned: Arc<str> = Arc::from(s);
                let sym = Symbol(self.arena.push(Arc::clone(&owned)));
                map.insert(owned, sym);
                sym
            }
        };
        Self::remember(hash, sym);
        sym
    }

    /// The symbol for `s` if it has been interned.
    pub fn get(&self, s: &str) -> Option<Symbol> {
        let hash = str_hash(s);
        if let Some(sym) = self.cached(s, hash) {
            return Some(sym);
        }
        let sym = self.map.lock().unwrap_or_else(PoisonError::into_inner).get(s).copied()?;
        Self::remember(hash, sym);
        Some(sym)
    }

    /// The per-thread cache's answer for `s`, verified: strings are unique
    /// within an interner, so if this interner's arena holds `s` at the
    /// cached symbol, that symbol is `s`'s.
    #[inline]
    fn cached(&self, s: &str, hash: u64) -> Option<Symbol> {
        let v = INTERN_CACHE.with(|c| c[cache_slot(hash)].get());
        if v == CACHE_EMPTY || (v >> 32) != (hash >> 32) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, reason = "low half of the slot")]
        let sym = v as u32;
        (self.arena.get(sym) == Some(s)).then_some(Symbol(sym))
    }

    fn remember(hash: u64, sym: Symbol) {
        let v = (hash >> 32) << 32 | u64::from(sym.0);
        INTERN_CACHE.with(|c| c[cache_slot(hash)].set(v));
    }

    /// The string of `sym`.
    ///
    /// # Panics
    /// If `sym` was not created by this interner.
    pub fn as_str(&self, sym: Symbol) -> &str {
        self.arena.get(sym.0).unwrap_or_else(|| panic!("{sym:?} was not created by this Interner"))
    }

    /// Number of distinct strings interned.
    pub fn len(&self) -> usize {
        self.arena.len.load(Ordering::Acquire) as usize
    }

    /// True if nothing has been interned.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Interner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interner").field("len", &self.len()).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{Interner, locate};

    #[test]
    fn locate_is_contiguous() {
        let mut expect = (0usize, 0usize);
        for idx in 0..5000u32 {
            let got = locate(idx);
            if got.0 == expect.0 {
                assert_eq!(got, expect);
            } else {
                assert_eq!(got, (expect.0 + 1, 0));
            }
            expect = (got.0, got.1 + 1);
        }
        assert!(locate(u32::MAX).0 < super::CHUNKS);
    }

    #[test]
    fn intern_roundtrip_and_dedup() {
        let i = Interner::new();
        let a = i.intern("alpha");
        let b = i.intern("beta");
        assert_ne!(a, b);
        assert_eq!(i.intern("alpha"), a);
        assert_eq!(i.as_str(a), "alpha");
        assert_eq!(b.as_str(&i), "beta");
        assert_eq!(i.get("beta"), Some(b));
        assert_eq!(i.get("gamma"), None);
        assert_eq!(i.len(), 2);
        for n in 0..2000 {
            let s = format!("id{n}");
            let sym = i.intern(&s);
            assert_eq!(i.as_str(sym), s);
        }
        assert_eq!(i.len(), 2002);
    }

    /// The per-thread cache is shared by all interners on a thread: a slot
    /// written by one never answers for another with a wrong symbol.
    #[test]
    fn intern_cache_is_per_interner_safe() {
        let a = Interner::new();
        let b = Interner::new();
        for n in 0..3000 {
            a.intern(&format!("a{n}"));
        }
        // `b` has different symbols for the same strings.
        let b0 = b.intern("zzz");
        for n in (0..3000).rev() {
            let s = format!("a{n}");
            let sb = b.intern(&s);
            assert_eq!(b.as_str(sb), s);
            let sa = a.intern(&s);
            assert_eq!(a.as_str(sa), s);
            assert_eq!(b.get(&s), Some(sb));
        }
        assert_eq!(b.as_str(b0), "zzz");
        assert_eq!(a.get("zzz"), None);
        assert_eq!(b.len(), 3001);
        assert_eq!(a.len(), 3000);
    }

    #[test]
    fn interner_is_sync() {
        fn assert_sync<T: Sync + Send>() {}
        assert_sync::<Interner>();
        let i = Interner::new();
        std::thread::scope(|s| {
            for t in 0..4 {
                let i = &i;
                s.spawn(move || {
                    for n in 0..500 {
                        let name = format!("x{}", n % 100 + t);
                        assert_eq!(i.as_str(i.intern(&name)), name);
                    }
                });
            }
        });
        assert_eq!(i.len(), 103);
    }
}
