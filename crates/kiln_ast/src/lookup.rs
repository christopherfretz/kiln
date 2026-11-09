//! `DeclContext` lookup tables (spec/ast.md §4.3).
//!
//! One global table keyed by `(primary context, name)`. An entry is a
//! single `DeclId` or an index into the overload-set pool; sets hold raw
//! decl ids with [`LookupFlags`] in the top bits: entries visible only
//! through an inline namespace (`VIA_INLINE`, [namespace.def]/7) and
//! hidden friends visible only to argument-dependent lookup (`ADL_ONLY`,
//! [namespace.memdef]/3, spec/sema-overload.md §2.2). The *rules*
//! (hiding, using-directives, ADL, two-phase lookup) are sema's; this
//! module stores what each context declares and answers
//! [`LookupTables::lookup_local`].
#![warn(missing_docs)]

use kiln_source::fx::FxHashMap;

use crate::ids::DeclId;
use crate::journal::Journal;
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::names::DeclName;

const SET_BIT: u32 = 1 << 31;
const VIA_INLINE: u32 = LookupFlags::VIA_INLINE.0;
const FLAG_MASK: u32 = LookupFlags::VIA_INLINE.0 | LookupFlags::ADL_ONLY.0;

/// Per-entry flags of a lookup-table entry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct LookupFlags(u32);

impl LookupFlags {
    /// No flags: visible to every lookup.
    pub const NONE: LookupFlags = LookupFlags(0);
    /// Entered in an enclosing namespace of an inline namespace.
    pub const VIA_INLINE: LookupFlags = LookupFlags(1 << 31);
    /// Visible only to argument-dependent lookup: a friend function first
    /// declared in a class ([namespace.memdef]/3, [class.friend]/7).
    /// Ordinary lookup skips it; a later declaration of the same entity
    /// without the flag replaces the entry and makes it visible.
    pub const ADL_ONLY: LookupFlags = LookupFlags(1 << 30);

    /// The raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
    /// True if every flag of `f` is set.
    pub const fn contains(self, f: LookupFlags) -> bool {
        self.0 & f.0 == f.0
    }
    /// The union.
    #[must_use]
    pub const fn with(self, f: LookupFlags) -> Self {
        LookupFlags(self.0 | f.0)
    }
    /// True if no flag is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A table entry: top bit clear = one `DeclId`, set = an overload-set
/// index.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LookupEntry(u32);

/// The tables.
#[derive(Debug, Default)]
pub struct LookupTables {
    map: FxHashMap<(DeclId, DeclName), LookupEntry>,
    sets: Vec<Vec<u32>>,
    /// The summed lengths and capacities of `sets`, kept as the sets
    /// grow: the memory meter reads them at every instantiation frame
    /// (WP-313), so summing the sets there was quadratic.
    set_words: usize,
    set_cap: usize,
    inline_children: FxHashMap<DeclId, Vec<DeclId>>,
    /// The namespaces nominated by the using-directives of a namespace or
    /// the translation unit (primary context), in declaration order
    /// ([namespace.udir]; spec/sema.md §3.1).
    using_dirs: FxHashMap<DeclId, Vec<DeclId>>,
    /// Bumped by every insertion (see [`LookupTables::generation`]).
    generation: u64,
    /// `(set, position)` of every set slot replaced in place, and
    /// `(set, u32::MAX)` for a set an entry was removed from (its later
    /// positions moved): what sema's redeclaration index re-reads
    /// (WP-658; see [`Journal`]).
    pub journal: Journal<(u32, u32)>,
}

/// The declarations found by [`LookupTables::lookup_local`], in order of
/// first declaration; each is the latest declaration of its entity.
#[derive(Clone, Copy, Debug)]
pub enum LookupResult<'a> {
    /// Nothing.
    Empty,
    /// One declaration (not via an inline namespace).
    One(DeclId),
    /// Several, or ones with [`LookupFlags`] (raw words: decl id with the
    /// flag bits on top).
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
                s.get(i).map(|&w| (DeclId::new(w & !FLAG_MASK), w & VIA_INLINE != 0))
            }
        }
    }

    /// The `i`-th declaration and its flags.
    pub fn get_with_flags(&self, i: usize) -> Option<(DeclId, LookupFlags)> {
        match self {
            LookupResult::Empty => None,
            LookupResult::One(d) => (i == 0).then_some((*d, LookupFlags::NONE)),
            LookupResult::Many(s) => {
                s.get(i).map(|&w| (DeclId::new(w & !FLAG_MASK), LookupFlags(w & FLAG_MASK)))
            }
        }
    }

    /// The declarations with their flags.
    pub fn iter_with_flags(&self) -> impl ExactSizeIterator<Item = (DeclId, LookupFlags)> + 'a {
        let this = *self;
        (0..this.len())
            .map(move |i| this.get_with_flags(i).expect("invariant: the index is below len()"))
    }

    /// The declarations visible to ordinary (non-ADL) lookup: all but
    /// [`LookupFlags::ADL_ONLY`] entries.
    pub fn iter_ordinary(&self) -> impl Iterator<Item = DeclId> + 'a {
        self.iter_with_flags().filter(|(_, f)| !f.contains(LookupFlags::ADL_ONLY)).map(|(d, _)| d)
    }

    /// The single declaration, if exactly one was found.
    pub fn single(&self) -> Option<DeclId> {
        if self.len() == 1 { self.get(0).map(|(d, _)| d) } else { None }
    }

    /// The declarations.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = DeclId> + 'a {
        let this = *self;
        (0..this.len())
            .map(move |i| this.get(i).map(|(d, _)| d).expect("invariant: the index is below len()"))
    }

    /// The declarations with their `via_inline` flag.
    pub fn iter_with_inline(&self) -> impl ExactSizeIterator<Item = (DeclId, bool)> + 'a {
        let this = *self;
        (0..this.len()).map(move |i| this.get(i).expect("invariant: the index is below len()"))
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
        let flags = if via_inline { LookupFlags::VIA_INLINE } else { LookupFlags::NONE };
        self.insert_with_flags(ctx, name, d, replaces, flags);
    }

    /// [`LookupTables::insert`] with explicit [`LookupFlags`]. Replacing an
    /// entry keeps its `VIA_INLINE` and takes `ADL_ONLY` from `flags` (a
    /// visible redeclaration of a hidden friend makes it visible).
    ///
    /// # Panics
    /// If `d`'s index does not fit below the flag bits.
    pub fn insert_with_flags(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        d: DeclId,
        replaces: Option<DeclId>,
        flags: LookupFlags,
    ) {
        assert!(d.index() & FLAG_MASK == 0, "DeclId collides with lookup flag bits");
        self.generation += 1;
        let word = d.index() | flags.0;
        let plain = flags.is_empty();
        let key = (ctx, name);
        let Some(&LookupEntry(w)) = self.map.get(&key) else {
            let e = if plain { LookupEntry(d.index()) } else { self.new_set(vec![word]) };
            self.map.insert(key, e);
            return;
        };
        if w & SET_BIT == 0 {
            if w == d.index() && plain {
                return;
            }
            if replaces.is_some_and(|p| p.index() == w) && plain {
                self.map.insert(key, LookupEntry(d.index()));
                return;
            }
            let e = self.new_set(vec![w]);
            self.map.insert(key, e);
        }
        let LookupEntry(w) = self.map[&key];
        let set = &mut self.sets[(w & !SET_BIT) as usize];
        if let Some(p) = replaces
            && let Some(pos) = set.iter().position(|s| *s & !FLAG_MASK == p.index())
        {
            let slot = &mut set[pos];
            *slot = (*slot & VIA_INLINE) | (word & LookupFlags::ADL_ONLY.0) | d.index();
            self.journal.push((w & !SET_BIT, pos as u32));
            return;
        }
        if !set.iter().any(|s| *s & !FLAG_MASK == d.index()) {
            let cap = set.capacity();
            set.push(word);
            self.set_words += 1;
            self.set_cap += set.capacity() - cap;
        }
    }

    /// Removes `d` from `(ctx, name)`'s entry, if it is there: sema's
    /// recovery from a class or enumeration redefinition, which is not
    /// visible to later lookups (clang makes it anonymous; spec/sema.md
    /// §4.4).
    pub fn remove(&mut self, ctx: DeclId, name: DeclName, d: DeclId) {
        let key = (ctx, name);
        let Some(&LookupEntry(w)) = self.map.get(&key) else {
            return;
        };
        self.generation += 1;
        if w & SET_BIT == 0 {
            if w == d.index() {
                self.map.remove(&key);
            }
            return;
        }
        let set = &mut self.sets[(w & !SET_BIT) as usize];
        let before = set.len();
        set.retain(|s| *s & !FLAG_MASK != d.index());
        if set.len() != before {
            self.journal.push((w & !SET_BIT, u32::MAX));
        }
        self.set_words -= before - set.len();
    }

    /// The overload set `(ctx, name)` names, if its entry is a set (see
    /// [`LookupTables::set_entry`]); a set keeps its id for good.
    pub fn set_id(&self, ctx: DeclId, name: DeclName) -> Option<u32> {
        match self.map.get(&(ctx, name)) {
            Some(&LookupEntry(w)) if w & SET_BIT != 0 => Some(w & !SET_BIT),
            _ => None,
        }
    }

    /// The length of overload set `set`.
    pub fn set_len(&self, set: u32) -> usize {
        self.sets[set as usize].len()
    }

    /// The declaration at position `pos` of overload set `set`, and its
    /// flags.
    pub fn set_entry(&self, set: u32, pos: usize) -> Option<(DeclId, LookupFlags)> {
        self.sets[set as usize]
            .get(pos)
            .map(|&w| (DeclId::new(w & !FLAG_MASK), LookupFlags(w & FLAG_MASK)))
    }

    fn new_set(&mut self, v: Vec<u32>) -> LookupEntry {
        let i = self.sets.len() as u32;
        assert!(i < SET_BIT, "lookup overload-set space exhausted");
        self.set_words += v.len();
        self.set_cap += v.capacity();
        self.sets.push(v);
        LookupEntry(SET_BIT | i)
    }

    /// Records inline namespace `child` (primary) of namespace `parent`
    /// (primary), for ADL's associated namespaces.
    pub fn add_inline_child(&mut self, parent: DeclId, child: DeclId) {
        self.generation += 1;
        let v = self.inline_children.entry(parent).or_default();
        if !v.contains(&child) {
            v.push(child);
        }
    }

    /// The inline namespaces directly nested in `parent` (primary).
    pub fn inline_children(&self, parent: DeclId) -> &[DeclId] {
        self.inline_children.get(&parent).map_or(&[], Vec::as_slice)
    }

    /// Records that namespace (or translation unit) `ctx` (primary)
    /// contains a using-directive nominating namespace `nominated`
    /// (primary). A repeated directive is recorded once.
    pub fn add_using_directive(&mut self, ctx: DeclId, nominated: DeclId) {
        self.generation += 1;
        let v = self.using_dirs.entry(ctx).or_default();
        if !v.contains(&nominated) {
            v.push(nominated);
        }
    }

    /// The namespaces nominated by the using-directives of `ctx`
    /// (primary), in declaration order. Those of `ctx`'s inline
    /// namespaces are not included.
    pub fn using_directives(&self, ctx: DeclId) -> &[DeclId] {
        self.using_dirs.get(&ctx).map_or(&[], Vec::as_slice)
    }

    /// True if no namespace-scope using-directive has been recorded.
    pub fn has_no_using_directives(&self) -> bool {
        self.using_dirs.is_empty()
    }

    /// A counter that changes whenever an entry may have changed: two
    /// equal values bracket a span in which every lookup through these
    /// tables gives the same answer (the parser's template-argument scan
    /// memo, spec/parse.md §2.5).
    pub fn generation(&self) -> u64 {
        self.generation
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
        let (words, cap) = (self.set_words, self.set_cap);
        let outer = MemRow::of_vec("sets", &self.sets);
        MemRow::new(
            "sets",
            self.sets.len() as u64,
            outer.live_bytes + words as u64 * 4,
            outer.reserved_bytes + cap as u64 * 4,
        )
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        let sets = crate::mem::VecDims::of("sets", &self.sets).live_bytes();
        crate::mem::map_live_bytes(&self.map) + sets + self.set_words as u64 * 4
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
        let dirs: usize = self.using_dirs.values().map(Vec::capacity).sum();
        let m = MemRow::of_map("using_dirs", &self.using_dirs);
        r.push(MemRow::new(
            "using_dirs",
            m.count,
            m.live_bytes + dirs as u64 * 4,
            m.reserved_bytes + dirs as u64 * 4,
        ));
        r
    }
}

#[cfg(test)]
mod tests {
    use super::{LookupFlags, LookupTables};
    use crate::ids::DeclId;
    use crate::mem::HeapSize;
    use crate::names::DeclName;
    use kiln_source::symbol::Interner;

    /// The kept sums (WP-313) equal the sums over the sets after
    /// singleton entries, set creation, appends, replacements and
    /// duplicates.
    #[test]
    fn set_meter_tracks_the_sets() {
        let i = Interner::new();
        let mut t = LookupTables::new();
        let ctx = DeclId::new(1);
        for n in 0..40u32 {
            let name = DeclName::ident(i.intern(&format!("f{}", n % 7)));
            let d = DeclId::new(2 + n);
            t.insert(ctx, name, d, None, n % 3 == 0);
            t.insert(ctx, name, d, None, false);
            if n % 5 == 0 {
                t.insert(ctx, name, DeclId::new(100 + n), Some(d), false);
            }
            t.insert_with_flags(ctx, name, DeclId::new(200 + n), None, LookupFlags::ADL_ONLY);
            let words: usize = t.sets.iter().map(Vec::len).sum();
            let cap: usize = t.sets.iter().map(Vec::capacity).sum();
            assert_eq!((t.set_words, t.set_cap), (words, cap));
            // The instantiation meter's allocation-free sum (WP-645) is
            // the report rows' live bytes (spec/ast.md §2.3).
            let rows = t.heap_bytes();
            let report =
                rows.row("table").unwrap().live_bytes + rows.row("sets").unwrap().live_bytes;
            assert_eq!(t.live_bytes(), report);
        }
    }
}
