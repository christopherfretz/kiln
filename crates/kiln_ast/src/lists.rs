//! `IdLists`: immutable lists of `u32` ids (spec/ast.md §2.2).
//!
//! All lists share one `Vec<u32>`; a [`ListId`] names a `(start, len)`
//! span of it. Lists are written once, at creation, and never change.

use crate::ids::{ListId, RawId};
use crate::mem::{HeapSize, MemReport, MemRow};

/// The list arena. Index 0 is the dummy; index 1 is [`ListId::EMPTY`].
#[derive(Debug, Clone)]
pub struct IdLists {
    spans: Vec<(u32, u32)>,
    data: Vec<u32>,
}

impl Default for IdLists {
    fn default() -> Self {
        Self::new()
    }
}

impl IdLists {
    /// An arena holding only the dummy and the empty list.
    pub fn new() -> Self {
        Self { spans: vec![(0, 0), (0, 0)], data: Vec::new() }
    }

    /// Stores `words` as a new list. An empty slice returns
    /// [`ListId::EMPTY`] without allocating.
    ///
    /// # Panics
    /// If the shared data vector would exceed `u32::MAX` words (arena
    /// exhaustion, spec/ast.md §2.1).
    pub fn push_raw(&mut self, words: &[u32]) -> ListId {
        if words.is_empty() {
            return ListId::EMPTY;
        }
        let start = u32::try_from(self.data.len()).expect("IdLists data space exhausted");
        let len = u32::try_from(words.len()).expect("IdLists list too long");
        assert!(start.checked_add(len).is_some(), "IdLists data space exhausted");
        self.data.extend_from_slice(words);
        let id = ListId::new(u32::try_from(self.spans.len()).expect("ListId space exhausted"));
        self.spans.push((start, len));
        id
    }

    /// Stores a list of typed ids.
    pub fn push<I: RawId>(&mut self, ids: &[I]) -> ListId {
        if ids.is_empty() {
            return ListId::EMPTY;
        }
        let start = self.data.len();
        self.data.extend(ids.iter().map(|i| i.raw()));
        let words = self.data.split_off(start);
        // `split_off` + `push_raw` keeps one code path for the bounds checks.
        self.push_raw(&words)
    }

    /// Stores the ids yielded by `iter`.
    pub fn push_iter<I: RawId>(&mut self, iter: impl IntoIterator<Item = I>) -> ListId {
        let v: Vec<u32> = iter.into_iter().map(RawId::raw).collect();
        self.push_raw(&v)
    }

    /// The raw words of `list`.
    ///
    /// # Panics
    /// If `list` was not created by this arena.
    pub fn get(&self, list: ListId) -> &[u32] {
        let (start, len) = self.spans[list.index() as usize];
        &self.data[start as usize..(start + len) as usize]
    }

    /// The number of elements of `list`.
    pub fn len(&self, list: ListId) -> usize {
        self.spans[list.index() as usize].1 as usize
    }

    /// True if `list` has no elements.
    pub fn is_empty(&self, list: ListId) -> bool {
        self.len(list) == 0
    }

    /// The elements of `list` as typed ids.
    ///
    /// # Panics
    /// (While iterating) if an element is 0, i.e. the list was not built
    /// from ids of type `I`.
    pub fn iter<I: RawId>(&self, list: ListId) -> impl ExactSizeIterator<Item = I> + '_ {
        self.get(list).iter().map(|&w| I::from_raw(w).expect("IdLists: zero word read as an id"))
    }

    /// The `i`-th element of `list` as a typed id.
    pub fn at<I: RawId>(&self, list: ListId, i: usize) -> Option<I> {
        self.get(list).get(i).and_then(|&w| I::from_raw(w))
    }

    /// Number of lists created (excluding the dummy and the empty list).
    pub fn count(&self) -> usize {
        self.spans.len() - 2
    }

    #[allow(dead_code, reason = "used by Ast::memory once the builder lands")]
    pub(crate) fn live_bytes(&self) -> u64 {
        (self.spans.len() * 8 + self.data.len() * 4) as u64
    }
}

impl HeapSize for IdLists {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("spans", &self.spans));
        r.push(MemRow::of_vec("data", &self.data));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::IdLists;
    use crate::ids::{DeclId, ListId};

    #[test]
    fn push_and_read() {
        let mut l = IdLists::new();
        assert_eq!(l.push::<DeclId>(&[]), ListId::EMPTY);
        let a = l.push(&[DeclId::new(3), DeclId::new(9)]);
        let b = l.push_raw(&[7]);
        assert_ne!(a, b);
        assert_eq!(l.iter::<DeclId>(a).collect::<Vec<_>>(), [DeclId::new(3), DeclId::new(9)]);
        assert_eq!(l.get(b), [7]);
        assert_eq!(l.len(ListId::EMPTY), 0);
        assert_eq!(l.at::<DeclId>(a, 1), Some(DeclId::new(9)));
        assert_eq!(l.count(), 2);
    }
}
