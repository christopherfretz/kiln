//! `FxHash` (the rustc hasher): fast, non-cryptographic, good for short
//! keys such as interned ids. Shared by the interner here and by the
//! hash tables of `kiln_ast` (lookup tables, cold side maps, interning).
//!
//! Not `HashDoS`-resistant: keys are compiler-internal ids and identifier
//! strings of the translation unit being compiled.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// The `FxHash` hasher state.
#[derive(Default, Clone, Copy, Debug)]
pub struct FxHasher {
    hash: u64,
}

const FX_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(FX_SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for c in chunks {
            self.add(u64::from_le_bytes(*c));
        }
        if !rest.is_empty() {
            let mut w = [0u8; 8];
            w[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(w));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// `BuildHasher` for [`FxHasher`].
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;
/// A `HashMap` using [`FxHasher`].
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;
/// A `HashSet` using [`FxHasher`].
pub type FxHashSet<T> = HashSet<T, FxBuildHasher>;
