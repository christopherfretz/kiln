//! Canonical expression profiles (spec/types.md §7.2).
//!
//! A profile is the interned sequence of `u32` words that `profile_expr`
//! (`kiln_sema`, which sees expressions) produces for an expression inside a
//! type. Two expressions are equivalent ([temp.over.link]) iff their
//! profiles are equal, so dependent types containing expressions
//! canonicalize to a [`ProfileId`] and stay `u32`-comparable. The entry
//! keeps the first expression seen (for printing the canonical type) and
//! the expression's dependence flags.

use super::intern::{InternTable, fx_hash};
use super::kinds::TypeFlags;
use crate::ids::{ExprId, ProfileId};
use crate::mem::{MemReport, MemRow};

#[derive(Clone, Copy, Debug)]
struct ProfileEntry {
    start: u32,
    len: u32,
    first_expr: Option<ExprId>,
    flags: TypeFlags,
}

/// The profile arena and its intern table.
#[derive(Debug)]
pub(crate) struct ProfileTable {
    words: Vec<u32>,
    entries: Vec<ProfileEntry>,
    intern: InternTable,
}

impl ProfileTable {
    pub(crate) fn new() -> ProfileTable {
        ProfileTable {
            words: Vec::new(),
            entries: vec![ProfileEntry {
                start: 0,
                len: 0,
                first_expr: None,
                flags: TypeFlags::NONE,
            }],
            intern: InternTable::default(),
        }
    }

    fn range(e: &ProfileEntry) -> std::ops::Range<usize> {
        e.start as usize..e.start as usize + e.len as usize
    }

    /// Interns `words`. On a hit the stored first expression and flags are
    /// kept (the flags of equivalent expressions are equal by
    /// construction). Returns `None` when the arena is exhausted.
    pub(crate) fn intern(
        &mut self,
        words: &[u32],
        first_expr: Option<ExprId>,
        flags: TypeFlags,
    ) -> Option<ProfileId> {
        let hash = fx_hash(words);
        let (store, entries) = (&self.words, &self.entries);
        if let Some(id) = self.intern.find(hash, |id| {
            let e = &entries[id as usize];
            &store[Self::range(e)] == words
        }) {
            return ProfileId::try_new(id);
        }
        if !super::node::Store::fits(self.words.len(), words.len()) {
            return None;
        }
        let id = ProfileId::try_new(u32::try_from(self.entries.len()).ok()?)?;
        let start = self.words.len() as u32;
        self.words.extend_from_slice(words);
        self.entries.push(ProfileEntry {
            start,
            len: words.len() as u32,
            first_expr,
            flags: flags.without(TypeFlags::SUGAR),
        });
        let (store, entries) = (&self.words, &self.entries);
        self.intern.insert(hash, id.index(), |i| {
            let e = &entries[i as usize];
            fx_hash(&store[Self::range(e)])
        });
        Some(id)
    }

    pub(crate) fn words(&self, p: ProfileId) -> &[u32] {
        let e = &self.entries[p.index() as usize];
        &self.words[Self::range(e)]
    }

    pub(crate) fn first_expr(&self, p: ProfileId) -> Option<ExprId> {
        self.entries[p.index() as usize].first_expr
    }

    pub(crate) fn flags(&self, p: ProfileId) -> TypeFlags {
        self.entries[p.index() as usize].flags
    }

    pub(crate) fn memory(&self, r: &mut MemReport) {
        r.push(MemRow::of_vec("profiles.words", &self.words));
        r.push(MemRow::of_vec("profiles.entries", &self.entries));
        r.push(MemRow::new(
            "profiles.intern",
            self.intern.len() as u64,
            self.intern.live_bytes(),
            self.intern.heap_bytes(),
        ));
    }
}
