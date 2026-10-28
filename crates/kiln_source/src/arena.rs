//! A safe, append-only vector that can grow through `&self`.
//!
//! `SourceManager` is shared by reference between the lexer, the
//! preprocessor and the diagnostics engine while the preprocessor keeps
//! appending files and expansion entries. Elements therefore must never
//! move once pushed, so that `&T` handed out through `&self` stays valid.
//! Storage is a fixed array of chunks with doubling sizes; each slot is a
//! `OnceLock`, so the whole structure is safe Rust and `Sync`.

use std::fmt;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// log2 of the first chunk's size.
const FIRST_CHUNK_BITS: u32 = 6;
/// Enough chunks to address every `u32` index: chunk `k` holds
/// `2^(FIRST_CHUNK_BITS + k)` slots.
const CHUNKS: usize = (u32::BITS - FIRST_CHUNK_BITS + 1) as usize;

pub(crate) struct AppendVec<T> {
    len: AtomicU32,
    chunks: [OnceLock<Box<[OnceLock<T>]>>; CHUNKS],
}

/// Maps a flat index to (chunk, slot within chunk).
fn locate(idx: u32) -> (usize, usize) {
    let n = (u64::from(idx) >> FIRST_CHUNK_BITS) + 1;
    let k = n.ilog2();
    let start = ((1u64 << k) - 1) << FIRST_CHUNK_BITS;
    (k as usize, (u64::from(idx) - start) as usize)
}

impl<T> AppendVec<T> {
    pub(crate) fn new() -> Self {
        Self { len: AtomicU32::new(0), chunks: [const { OnceLock::new() }; CHUNKS] }
    }

    /// Number of fully published elements.
    pub(crate) fn len(&self) -> u32 {
        self.len.load(Ordering::Acquire)
    }

    /// Appends `value` and returns its index.
    ///
    /// Pushes must be serialized by the caller (`SourceManager` holds its
    /// allocation mutex); concurrent readers are fine.
    pub(crate) fn push(&self, value: T) -> u32 {
        let idx = self.len.load(Ordering::Relaxed);
        let (k, slot) = locate(idx);
        let chunk = self.chunks[k].get_or_init(|| {
            (0..1usize << (FIRST_CHUNK_BITS as usize + k)).map(|_| OnceLock::new()).collect()
        });
        if chunk[slot].set(value).is_err() {
            unreachable!("AppendVec::push: pushes must be serialized by the caller");
        }
        self.len.store(idx + 1, Ordering::Release);
        idx
    }

    pub(crate) fn get(&self, idx: u32) -> Option<&T> {
        if idx >= self.len() {
            return None;
        }
        let (k, slot) = locate(idx);
        self.chunks[k].get()?.get(slot)?.get()
    }

    /// Index of the first element for which `pred` is false, assuming the
    /// elements are partitioned (all `true` before all `false`).
    ///
    /// Searches the chunks by their first element, then the one chunk
    /// that holds the boundary as a slice: one atomic length load and
    /// `O(log n)` slot reads.
    pub(crate) fn partition_point(&self, mut pred: impl FnMut(&T) -> bool) -> u32 {
        let len = self.len();
        let Some(last) = len.checked_sub(1) else { return 0 };
        let (last_k, last_slot) = locate(last);
        let first_ok = |k: usize, pred: &mut dyn FnMut(&T) -> bool| {
            self.chunks[k].get().and_then(|c| c.first()).and_then(OnceLock::get).is_some_and(pred)
        };
        let (mut lo, mut hi) = (0, last_k + 1);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if first_ok(mid, &mut pred) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let Some(k) = lo.checked_sub(1) else { return 0 };
        let Some(chunk) = self.chunks[k].get() else { return 0 };
        let filled = if k == last_k { last_slot + 1 } else { chunk.len() };
        let i = chunk[..filled].partition_point(|slot| slot.get().is_some_and(&mut pred));
        chunk_start(k) + i as u32
    }
}

/// Flat index of the first slot of chunk `k`.
fn chunk_start(k: usize) -> u32 {
    (((1u64 << k) - 1) << FIRST_CHUNK_BITS) as u32
}

impl<T> fmt::Debug for AppendVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppendVec").field("len", &self.len()).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{AppendVec, locate};

    #[test]
    fn locate_is_contiguous() {
        let mut expect = (0usize, 0usize);
        for idx in 0..10_000u32 {
            let got = locate(idx);
            if got.0 == expect.0 {
                assert_eq!(got, expect, "idx {idx}");
            } else {
                assert_eq!(got, (expect.0 + 1, 0), "idx {idx}");
            }
            expect = (got.0, got.1 + 1);
        }
        assert!(locate(u32::MAX).0 < super::CHUNKS);
    }

    #[test]
    fn push_get_partition() {
        let v = AppendVec::new();
        for i in 0..1000u32 {
            assert_eq!(v.push(i * 2), i);
        }
        assert_eq!(v.len(), 1000);
        assert_eq!(v.get(999), Some(&1998));
        assert_eq!(v.get(1000), None);
        assert_eq!(v.partition_point(|&x| x <= 501), 251);
        for t in [0, 1, 126, 127, 128, 129, 1997, 1998, 1999, 5000] {
            let want = (0..1000u32).filter(|i| i * 2 <= t).count() as u32;
            assert_eq!(v.partition_point(|&x| x <= t), want, "t {t}");
        }
        assert_eq!(AppendVec::<u32>::new().partition_point(|_| true), 0);
    }

    #[test]
    fn chunk_start_matches_locate() {
        for k in 0..20 {
            assert_eq!(locate(super::chunk_start(k)), (k, 0));
        }
    }
}
