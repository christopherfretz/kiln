//! The definition-context lookup sets of dependent operator expressions
//! (spec/ast.md §8.5, WP-2021; [temp.dep.candidate]/1,
//! [over.match.oper]/3.2).
//!
//! A unary, binary or compound-assignment operator expression with a
//! type-dependent operand, built in a template, keeps the non-member
//! operator functions the unqualified lookup of its operator name (and,
//! for a comparison, of its rewritten forms' names) found at the
//! definition. Its instantiation uses that set plus argument-dependent
//! lookup at the point of instantiation, never a new unqualified lookup.
//! The set lives here, in the `Ast`, so it survives the end of the
//! translation unit for tooling clients, and a snapshot or a module image
//! carries it with the expression.
//!
//! Sets are interned by content: one `<iostream>` template body holds
//! many `os << x` expressions whose sets of `operator<<` are equal, and
//! each is stored once.

use std::hash::{Hash, Hasher};

use kiln_source::fx::{FxHashMap, FxHasher};

use crate::ids::{DeclId, ExprId};
use crate::mem::{MemReport, MemRow};
use crate::slot::{AlreadySet, ColdMap};

/// One non-member operator function a definition's lookup found.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct OperCandidate {
    /// The function or function template.
    pub decl: DeclId,
    /// What lookup found: the `UsingShadow` that names `decl` (`using
    /// N::operator+;`), else `decl`.
    pub found: DeclId,
}

/// An interned candidate set (an index into [`OperLookups`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct OperSetId(u32);

/// Where an interned set's candidates are.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
struct SetSpan {
    start: u32,
    len: u32,
}

/// The definition-context operator lookup sets, by dependent operator
/// expression (spec/ast.md §8.5). Insert-only.
#[derive(Debug, kiln_snap::Snap)]
pub struct OperLookups {
    /// The set of each dependent operator expression that has one.
    by_expr: ColdMap<ExprId, OperSetId>,
    /// The interned sets.
    sets: Vec<SetSpan>,
    /// Their candidates, back to back.
    cands: Vec<OperCandidate>,
    /// The content hash of each set (the first set of a hash; another set
    /// of the same hash is stored again): rebuilt after a snapshot is
    /// read, by [`Self::catch_up`].
    #[snap(skip)]
    index: FxHashMap<u64, OperSetId>,
    /// How many of `sets` `index` covers.
    #[snap(skip)]
    indexed: u32,
}

impl Default for OperLookups {
    fn default() -> Self {
        Self {
            by_expr: ColdMap::new("oper_lookup"),
            sets: Vec::new(),
            cands: Vec::new(),
            index: FxHashMap::default(),
            indexed: 0,
        }
    }
}

fn content_hash(cands: &[OperCandidate]) -> u64 {
    let mut h = FxHasher::default();
    cands.hash(&mut h);
    h.finish()
}

impl OperLookups {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// The interned set equal to `cands` (in order), added if new.
    pub fn intern(&mut self, cands: &[OperCandidate]) -> OperSetId {
        self.catch_up();
        let h = content_hash(cands);
        if let Some(&id) = self.index.get(&h)
            && self.candidates(id) == cands
        {
            return id;
        }
        let id = OperSetId(u32::try_from(self.sets.len()).expect("operator lookup sets exhausted"));
        let start = u32::try_from(self.cands.len()).expect("operator lookup sets exhausted");
        let len = u32::try_from(cands.len()).expect("operator lookup set too large");
        self.cands.extend_from_slice(cands);
        self.sets.push(SetSpan { start, len });
        self.index.entry(h).or_insert(id);
        self.indexed = self.sets.len() as u32;
        id
    }

    /// Indexes the sets a snapshot brought in, so equal content interns to
    /// them.
    fn catch_up(&mut self) {
        while (self.indexed as usize) < self.sets.len() {
            let id = OperSetId(self.indexed);
            let h = content_hash(self.candidates(id));
            self.index.entry(h).or_insert(id);
            self.indexed += 1;
        }
    }

    /// Records `set` as the definition-context set of the dependent
    /// operator expression `e`. Write-once.
    pub fn set(&mut self, e: ExprId, set: OperSetId) -> Result<(), AlreadySet> {
        debug_assert!((set.0 as usize) < self.sets.len(), "invariant: an interned set");
        self.by_expr.insert(e, set)
    }

    /// The set of `e`, if one was recorded.
    pub fn set_of(&self, e: ExprId) -> Option<OperSetId> {
        self.by_expr.get(e).copied()
    }

    /// The candidates of the interned set `id`, in lookup order.
    pub fn candidates(&self, id: OperSetId) -> &[OperCandidate] {
        let s = self.sets[id.0 as usize];
        &self.cands[s.start as usize..(s.start + s.len) as usize]
    }

    /// The candidates recorded for `e` ([`Self::set_of`]).
    pub fn of(&self, e: ExprId) -> Option<&[OperCandidate]> {
        self.set_of(e).map(|id| self.candidates(id))
    }

    /// The number of expressions with a set.
    pub fn len(&self) -> usize {
        self.by_expr.len()
    }

    /// True if no expression has a set.
    pub fn is_empty(&self) -> bool {
        self.by_expr.is_empty()
    }

    /// The number of distinct interned sets.
    pub fn set_count(&self) -> usize {
        self.sets.len()
    }

    /// The memory rows (spec/ast.md §2.3).
    pub fn mem_rows(&self, r: &mut MemReport) {
        r.push(self.by_expr.mem_row().prefixed("templ"));
        r.push(MemRow::of_vec("templ.oper_sets", &self.sets));
        r.push(MemRow::of_vec("templ.oper_cands", &self.cands));
        r.push(MemRow::of_map("templ.oper_index", &self.index));
    }
}

impl crate::Ast {
    /// The non-member operator functions the unqualified lookup at the
    /// template's definition found for the dependent operator expression
    /// `e` (§8.5): `None` when none was recorded (a non-dependent
    /// expression, an operator without non-member candidates such as
    /// `=`, or an expression that is not an operator). An empty slice is a
    /// lookup that found nothing; the instantiation then has
    /// argument-dependent lookup only.
    pub fn dependent_oper_candidates(&self, e: ExprId) -> Option<&[OperCandidate]> {
        self.templates.oper_lookups.of(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(d: u32, f: u32) -> OperCandidate {
        OperCandidate { decl: DeclId::new(d), found: DeclId::new(f) }
    }

    #[test]
    fn equal_sets_are_stored_once() {
        let mut table = OperLookups::new();
        let first = table.intern(&[c(1, 1), c(2, 3)]);
        let again = table.intern(&[c(1, 1), c(2, 3)]);
        let empty = table.intern(&[]);
        let reordered = table.intern(&[c(2, 3), c(1, 1)]);
        assert_eq!(first, again);
        assert_ne!(first, reordered);
        assert_eq!(table.set_count(), 3);
        assert!(table.candidates(empty).is_empty());
        table.set(ExprId::new(5), first).expect("fresh");
        table.set(ExprId::new(6), empty).expect("fresh");
        assert!(table.set(ExprId::new(5), reordered).is_err(), "write-once");
        assert_eq!(table.of(ExprId::new(5)), Some(&[c(1, 1), c(2, 3)][..]));
        assert_eq!(table.of(ExprId::new(6)), Some(&[][..]));
        assert_eq!(table.of(ExprId::new(7)), None);
    }

    #[test]
    fn index_is_rebuilt_after_a_snapshot() {
        let mut t = OperLookups::new();
        let a = t.intern(&[c(4, 4)]);
        // What a snapshot read leaves: the sets, no index.
        t.index.clear();
        t.indexed = 0;
        assert_eq!(t.intern(&[c(4, 4)]), a);
        assert_eq!(t.set_count(), 1);
    }
}
