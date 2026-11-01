//! Where dependent names were written (spec/sema-templates.md §5.8):
//! kiln's types carry no source locations (they are interned), so a
//! diagnostic found while substituting a dependent name (`T::type`,
//! `T::N::`, `X<T>`) would otherwise sit at the declaration being
//! instantiated. Each dependent type or nested-name-specifier the parser
//! forms records its written location here; the substituter reports at
//! the occurrence nearest to its anchor (clang keeps a `TypeLoc` per
//! occurrence and reports at the name).

use kiln_ast::ids::{NnsId, QualType};
use kiln_source::Loc;

use crate::Sema;

/// A dependent entity whose written locations are recorded.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum WrittenKey {
    /// A dependent type (`typename T::type`, `X<T>`).
    Type(QualType),
    /// A dependent nested-name-specifier component (`T::N::`).
    Nns(NnsId),
}

impl Sema<'_, '_> {
    /// Records that `key` was written at `loc` (kept sorted, so the
    /// lookup is a binary search). Nothing while instantiating: only the
    /// pattern as written is recorded.
    pub(crate) fn note_written_loc(&mut self, key: WrittenKey, loc: Loc) {
        if !loc.is_valid() || !self.inst.stack.is_empty() {
            return;
        }
        let v = self.templ.written_locs.entry(key).or_default();
        let i = v.partition_point(|&l| l < loc);
        if v.get(i) != Some(&loc) {
            v.insert(i, loc);
        }
    }

    /// The written location of `key` nearest to `anchor` (the location a
    /// substitution was given: the declarator, expression or use being
    /// instantiated), or `anchor` when none was recorded.
    pub(crate) fn written_loc(&self, key: WrittenKey, anchor: Loc) -> Loc {
        let Some(v) = self.templ.written_locs.get(&key) else { return anchor };
        let i = v.partition_point(|&l| l <= anchor);
        let before = i.checked_sub(1).and_then(|j| v.get(j)).copied();
        let after = v.get(i).copied();
        let dist = |l: Loc| l.to_raw().abs_diff(anchor.to_raw());
        match (before, after) {
            (Some(b), Some(a)) => {
                if dist(a) < dist(b) {
                    a
                } else {
                    b
                }
            }
            (Some(l), None) | (None, Some(l)) => l,
            (None, None) => anchor,
        }
    }
}
