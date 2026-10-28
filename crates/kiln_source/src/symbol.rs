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
//! take no lock.

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
        let mut map = self.map.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&sym) = map.get(s) {
            return sym;
        }
        assert!(self.arena.len.load(Ordering::Relaxed) < u32::MAX, "interner exhausted");
        let owned: Arc<str> = Arc::from(s);
        let sym = Symbol(self.arena.push(Arc::clone(&owned)));
        map.insert(owned, sym);
        sym
    }

    /// The symbol for `s` if it has been interned.
    pub fn get(&self, s: &str) -> Option<Symbol> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner).get(s).copied()
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
