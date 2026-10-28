//! `DeclContext` lookup tables (spec/ast.md §4.3).
//!
//! One global table keyed by `(primary context, name)`. An entry is a
//! single `DeclId` or an index into the overload-set pool; sets hold raw
//! decl ids with the top bit marking entries visible only through an
//! inline namespace (`via_inline`, [namespace.def]/7). The *rules*
//! (hiding, using-directives, ADL, two-phase lookup) are sema's; this
//! module stores what each context declares and answers
//! [`LookupTables::lookup_local`].
#![warn(missing_docs)]

use kiln_source::fx::FxHashMap;

use crate::ids::DeclId;
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::names::DeclName;

const SET_BIT: u32 = 1 << 31;
const VIA_INLINE: u32 = 1 << 31;

/// A table entry: top bit clear = one `DeclId`, set = an overload-set
/// index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LookupEntry(u32);

/// The tables.
#[derive(Debug, Default)]
pub struct LookupTables {
    map: FxHashMap<(DeclId, DeclName), LookupEntry>,
    sets: Vec<Vec<u32>>,
    inline_children: FxHashMap<DeclId, Vec<DeclId>>,
}

/// The declarations found by [`LookupTables::lookup_local`], in order of
/// first declaration; each is the latest declaration of its entity.
#[derive(Clone, Copy, Debug)]
pub enum LookupResult<'a> {
    /// Nothing.
    Empty,
    /// One declaration (not via an inline namespace).
    One(DeclId),
    /// Several, or ones found via an inline namespace (raw words: decl id
    /// with the top bit = `via_inline`).
    Many(&'a [u32]),
}

impl<'a> LookupResult<'a> {
    /// The number of declarations found.
    pub fn len(&self) -> usize {
        match self {
            LookupResult::Empty => 0,
            LookupResult::One(_) => 1,
            LookupResult::Many(s) => s.len(),
        }
    }

    /// True if nothing was found.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The `i`-th declaration and whether it was found via an inline
    /// namespace.
    pub fn get(&self, i: usize) -> Option<(DeclId, bool)> {
        match self {
            LookupResult::Empty => None,
            LookupResult::One(d) => (i == 0).then_some((*d, false)),
            LookupResult::Many(s) => {
                s.get(i).map(|&w| (DeclId::new(w & !VIA_INLINE), w & VIA_INLINE != 0))
            }
        }
    }

    /// The single declaration, if exactly one was found.
    pub fn single(&self) -> Option<DeclId> {
        if self.len() == 1 { self.get(0).map(|(d, _)| d) } else { None }
    }

    /// The declarations.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = DeclId> + 'a {
        let this = *self;
        (0..this.len()).map(move |i| this.get(i).map(|(d, _)| d).expect("in range"))
    }

    /// The declarations with their `via_inline` flag.
    pub fn iter_with_inline(&self) -> impl ExactSizeIterator<Item = (DeclId, bool)> + 'a {
        let this = *self;
        (0..this.len()).map(move |i| this.get(i).expect("in range"))
    }

    /// The declarations, collected.
    pub fn to_vec(&self) -> Vec<DeclId> {
        self.iter().collect()
    }
}

impl LookupTables {
    /// Empty tables.
    pub fn new() -> Self {
        Self::default()
    }

    /// The declarations named `name` in (primary) context `ctx`.
    pub fn lookup_local(&self, ctx: DeclId, name: DeclName) -> LookupResult<'_> {
        match self.map.get(&(ctx, name)) {
            None => LookupResult::Empty,
            Some(&LookupEntry(w)) if w & SET_BIT == 0 => LookupResult::One(DeclId::new(w)),
            Some(&LookupEntry(w)) => LookupResult::Many(&self.sets[(w & !SET_BIT) as usize]),
        }
    }

    /// Enters `d` under `(ctx, name)`. If `replaces` (the previous
    /// declaration of the same entity) is in the entry, `d` takes its place
    /// (keeping overload-set order = order of first declaration);
    /// otherwise `d` is appended. `via_inline` marks an entry made in an
    /// enclosing namespace of an inline namespace.
    pub fn insert(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        d: DeclId,
        replaces: Option<DeclId>,
        via_inline: bool,
    ) {
        let word = d.index() | if via_inline { VIA_INLINE } else { 0 };
        let key = (ctx, name);
        let Some(&LookupEntry(w)) = self.map.get(&key) else {
            let e = if via_inline { self.new_set(vec![word]) } else { LookupEntry(d.index()) };
            self.map.insert(key, e);
            return;
        };
        if w & SET_BIT == 0 {
            if w == d.index() && !via_inline {
                return;
            }
            if replaces.is_some_and(|p| p.index() == w) && !via_inline {
                self.map.insert(key, LookupEntry(d.index()));
                return;
            }
            let e = self.new_set(vec![w]);
            self.map.insert(key, e);
        }
        let LookupEntry(w) = self.map[&key];
        let set = &mut self.sets[(w & !SET_BIT) as usize];
        if let Some(p) = replaces
            && let Some(slot) = set.iter_mut().find(|s| **s & !VIA_INLINE == p.index())
        {
            *slot = (*slot & VIA_INLINE) | d.index();
            return;
        }
        if !set.iter().any(|s| *s & !VIA_INLINE == d.index()) {
            set.push(word);
        }
    }

    fn new_set(&mut self, v: Vec<u32>) -> LookupEntry {
        let i = self.sets.len() as u32;
        assert!(i < SET_BIT, "lookup overload-set space exhausted");
        self.sets.push(v);
        LookupEntry(SET_BIT | i)
    }

    /// Records inline namespace `child` (primary) of namespace `parent`
    /// (primary), for ADL's associated namespaces.
    pub fn add_inline_child(&mut self, parent: DeclId, child: DeclId) {
        let v = self.inline_children.entry(parent).or_default();
        if !v.contains(&child) {
            v.push(child);
        }
    }

    /// The inline namespaces directly nested in `parent` (primary).
    pub fn inline_children(&self, parent: DeclId) -> &[DeclId] {
        self.inline_children.get(&parent).map_or(&[], Vec::as_slice)
    }

    /// The number of `(context, name)` entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn set_row(&self) -> MemRow {
        let words: usize = self.sets.iter().map(Vec::len).sum();
        let cap: usize = self.sets.iter().map(Vec::capacity).sum();
        let outer = MemRow::of_vec("sets", &self.sets);
        MemRow::new(
            "sets",
            self.sets.len() as u64,
            outer.live_bytes + words as u64 * 4,
            outer.reserved_bytes + cap as u64 * 4,
        )
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        MemRow::of_map("table", &self.map).live_bytes + self.set_row().live_bytes
    }
}

impl HeapSize for LookupTables {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_map("table", &self.map));
        r.push(self.set_row());
        let kids: usize = self.inline_children.values().map(Vec::capacity).sum();
        let m = MemRow::of_map("inline_children", &self.inline_children);
        r.push(MemRow::new(
            "inline_children",
            m.count,
            m.live_bytes + kids as u64 * 4,
            m.reserved_bytes + kids as u64 * 4,
        ));
        r
    }
}
