//! The redeclaration index (WP-658, spec/sema.md §3): a side index over
//! the large overload sets of the lookup tables, so that the redeclaration
//! search of a function declaration (`pick_prev_function`) visits only
//! the entries that can affect its outcome instead of every same-name
//! declaration of the scope (the hidden friends of N class templates in
//! one namespace made that scan quadratic).
//!
//! Each position of an indexed set has a key:
//! - a function (`PayloadClass::Function`) whose type is a prototype:
//!   a hash of "function", variadic and its canonical parameter types;
//! - a function template whose latest pattern's type is a prototype: the
//!   same hash over that pattern's type, marked "template";
//! - anything else ([`ALWAYS`]): non-function entities, using-shadows,
//!   other templates, a declaration without a type or with a
//!   non-prototype (error) type.
//!
//! A query for new declaration `new` (a prototype type) returns, in set
//! order, the positions keyed like `new` plus the [`ALWAYS`] positions:
//! a subsequence of the set that keeps every entry `pick_prev_function`
//! does not skip or reject before `fn_corresponds`'s parameter comparison
//! (which reads exactly the data the key hashes), so its result — `prev`,
//! the first invalid corresponding entry, the newest other entity — is
//! that of the full scan. The hash is a filter; `fn_corresponds` decides.
//! Flags (`INVALID`, the lookup flags) are read live, never keyed.
//!
//! A key reads the slot's declaration, its kind (fixed), a template's
//! pattern (fixed), the latest declaration of that pattern and the type of
//! the declaration it compares. The AST journals the mutations of the
//! last three (`kiln_ast::journal`): a slot replaced or a set shrunk
//! (`LookupTables::journal`), a declaration retyped
//! (`DeclArena::ty_journal`), a group's latest moved
//! (`RedeclTable::journal`). Each query drains them first and re-keys the
//! positions that read a journaled declaration; a shrunk set, or an
//! overflowed journal, drops the index (rebuilt on the next query).
//! Positions appended since the last query are keyed then.

use kiln_ast::decl::{DeclKind, PayloadClass};
use kiln_ast::lookup::LookupFlags;
use kiln_ast::names::DeclName;
use kiln_ast::templates::is_template_kind;
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::FxHashMap;

use super::redecl::NewFn;
use crate::Sema;

/// Sets shorter than this are scanned in full (not indexed).
pub(crate) const MIN_LEN: usize = 32;

/// The key of positions every query returns.
const ALWAYS: u64 = 0;
/// No position / no source declaration.
const NIL: u32 = u32::MAX;
const TEMPLATE_BIT: u64 = 1 << 1;

/// One indexed overload set: positions `0..keys.len()` are keyed.
#[derive(Debug, Default, kiln_snap::Snap)]
struct SetIndex {
    /// The key of each position.
    keys: Vec<u64>,
    /// The declaration each position's key read the type of (`NIL`:
    /// none).
    src: Vec<u32>,
    /// The previous position with the same key (`NIL`: none): each key's
    /// positions form a chain, newest first.
    prev_same: Vec<u32>,
    /// The newest position of each key.
    head: FxHashMap<u64, u32>,
}

impl SetIndex {
    /// Links position `pos` (keyed `keys[pos]`) into its key's chain,
    /// which stays in descending position order.
    fn link(&mut self, pos: u32) {
        let key = self.keys[pos as usize];
        let head = self.head.get(&key).copied().unwrap_or(NIL);
        if head == NIL || head < pos {
            self.prev_same[pos as usize] = head;
            self.head.insert(key, pos);
            return;
        }
        let mut at = head;
        while self.prev_same[at as usize] != NIL && self.prev_same[at as usize] > pos {
            at = self.prev_same[at as usize];
        }
        self.prev_same[pos as usize] = self.prev_same[at as usize];
        self.prev_same[at as usize] = pos;
    }

    /// Takes position `pos` out of its key's chain.
    fn unlink(&mut self, pos: u32) {
        let key = self.keys[pos as usize];
        let next = self.prev_same[pos as usize];
        let head = self.head[&key];
        if head == pos {
            if next == NIL {
                self.head.remove(&key);
            } else {
                self.head.insert(key, next);
            }
            return;
        }
        let mut at = head;
        while self.prev_same[at as usize] != pos {
            at = self.prev_same[at as usize];
        }
        self.prev_same[at as usize] = next;
    }

    fn heap_bytes(&self) -> u64 {
        (self.keys.capacity() * 8 + self.src.capacity() * 4 + self.prev_same.capacity() * 4) as u64
            + kiln_ast::mem::map_live_bytes(&self.head)
    }
}

/// The index: the indexed sets, and which positions read which
/// declaration (to re-key them when the journals name it).
#[derive(Debug, Default, kiln_snap::Snap)]
pub(crate) struct RedeclIndex {
    sets: FxHashMap<u32, SetIndex>,
    /// Source declaration → one `(set << 32) | pos` reading it.
    watch: FxHashMap<u32, u64>,
    /// Further positions reading the same declaration (rare: an entry
    /// also entered in the namespace enclosing an inline namespace).
    watch_more: FxHashMap<u32, Vec<u64>>,
    started: bool,
}

/// The key of a declaration of type `ty` (`None` when it has none) for a
/// function (`template` false) or a function template's pattern.
fn type_key(ast: &Ast, ty: Option<QualType>, template: bool) -> u64 {
    let t = &ast.types;
    let Some(p) = ty.and_then(|q| t.proto(q)) else { return ALWAYS };
    let mut h: u64 = if template { TEMPLATE_BIT } else { 0 };
    if p.info().variadic() {
        h ^= 1 << 2;
    }
    for &q in t.params(p) {
        h = (h.rotate_left(5) ^ u64::from(t.canonical(q).to_raw())).wrapping_mul(K);
    }
    // Never `ALWAYS`, and the template bit kept.
    (h & !TEMPLATE_BIT) | 1 | if template { TEMPLATE_BIT } else { 0 }
}

const K: u64 = 0x517c_c1b7_2722_0a95;

/// The key of set entry `d`, and the declaration whose type it read.
fn entry_key(ast: &Ast, d: DeclId) -> (u64, u32) {
    let k = ast.decls.get(d).kind();
    if is_template_kind(k) {
        if k != DeclKind::FunctionTemplate {
            return (ALWAYS, NIL);
        }
        let Some(p) = ast.template_info(d).and_then(|i| i.pattern) else {
            return (ALWAYS, NIL);
        };
        let p = ast.latest(p);
        return (type_key(ast, ast.decls.get(p).ty(), true), p.index());
    }
    if k.payload() == PayloadClass::Function {
        return (type_key(ast, ast.decls.get(d).ty(), false), d.index());
    }
    (ALWAYS, NIL)
}

impl RedeclIndex {
    /// Approximate heap bytes (for the bytes-per-entry measurement).
    pub(crate) fn heap_bytes(&self) -> u64 {
        self.sets.values().map(SetIndex::heap_bytes).sum::<u64>()
            + kiln_ast::mem::map_live_bytes(&self.sets)
            + kiln_ast::mem::map_live_bytes(&self.watch)
            + kiln_ast::mem::map_live_bytes(&self.watch_more)
    }

    /// The number of keyed positions.
    pub(crate) fn entries(&self) -> usize {
        self.sets.values().map(|s| s.keys.len()).sum()
    }

    fn watch(&mut self, src: u32, sp: u64) {
        if src == NIL {
            return;
        }
        if let Some(&first) = self.watch.get(&src) {
            if first != sp {
                self.watch_more.entry(src).or_default().push(sp);
            }
        } else {
            self.watch.insert(src, sp);
        }
    }

    fn unwatch(&mut self, src: u32, sp: u64) {
        if src == NIL {
            return;
        }
        if self.watch.get(&src) == Some(&sp) {
            match self.watch_more.get_mut(&src).and_then(Vec::pop) {
                Some(next) => {
                    self.watch.insert(src, next);
                }
                None => {
                    self.watch.remove(&src);
                }
            }
            if self.watch_more.get(&src).is_some_and(Vec::is_empty) {
                self.watch_more.remove(&src);
            }
        } else if let Some(v) = self.watch_more.get_mut(&src) {
            v.retain(|&x| x != sp);
            if v.is_empty() {
                self.watch_more.remove(&src);
            }
        }
    }

    /// Re-keys position `pos` of set `set` from the current AST.
    fn rekey(&mut self, ast: &Ast, set: u32, pos: u32) {
        let Some(si) = self.sets.get(&set) else { return };
        if pos as usize >= si.keys.len() {
            return;
        }
        let Some((d, _)) = ast.lookup.set_entry(set, pos as usize) else { return };
        let (key, src) = entry_key(ast, d);
        let si = self.sets.get_mut(&set).expect("indexed set");
        if key != si.keys[pos as usize] {
            si.unlink(pos);
            si.keys[pos as usize] = key;
            si.link(pos);
        }
        let old = std::mem::replace(&mut si.src[pos as usize], src);
        if old != src {
            let sp = (u64::from(set) << 32) | u64::from(pos);
            self.unwatch(old, sp);
            self.watch(src, sp);
        }
    }

    /// Forgets set `set` (its positions moved).
    fn drop_set(&mut self, set: u32) {
        let Some(si) = self.sets.remove(&set) else { return };
        for (pos, &src) in si.src.iter().enumerate() {
            self.unwatch(src, (u64::from(set) << 32) | pos as u64);
        }
    }

    /// Applies the AST journals.
    fn sync(&mut self, ast: &mut Ast) {
        #[cfg(feature = "op-stats")]
        let _t = crate::templ::deduce_stats::timer("redecl-index-sync");
        if !self.started {
            self.started = true;
            ast.lookup.journal.start();
            ast.decls.ty_journal.start();
            ast.redecls.journal.start();
            return;
        }
        let (slots, o1) = ast.lookup.journal.drain();
        let (retyped, o2) = ast.decls.ty_journal.drain();
        let (relinked, o3) = ast.redecls.journal.drain();
        if o1 || o2 || o3 {
            *self = RedeclIndex { started: true, ..RedeclIndex::default() };
            return;
        }
        #[cfg(feature = "op-stats")]
        let stat = (slots.len(), retyped.len(), relinked.len());
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::index_sync_more(
            retyped
                .iter()
                .filter(|&&d| ast.decls.get(d).kind().payload() == PayloadClass::Function)
                .count(),
            retyped.len(),
            slots.len(),
        );
        for (set, pos) in slots {
            if pos == NIL {
                self.drop_set(set);
            } else {
                self.rekey(ast, set, pos);
            }
        }
        let mut sps = Vec::new();
        for d in retyped.into_iter().chain(relinked) {
            let d = d.index();
            if let Some(&sp) = self.watch.get(&d) {
                sps.push(sp);
                if let Some(more) = self.watch_more.get(&d) {
                    sps.extend_from_slice(more);
                }
            }
        }
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::index_sync(stat.0, stat.1, stat.2, sps.len());
        for sp in sps {
            self.rekey(ast, (sp >> 32) as u32, sp as u32);
        }
    }

    /// Keys the positions of `set` appended since the last query.
    fn extend(&mut self, ast: &Ast, set: u32) {
        let len = ast.lookup.set_len(set);
        let from = self.sets.get(&set).map_or(0, |s| s.keys.len());
        for pos in from..len {
            let (d, _) = ast.lookup.set_entry(set, pos).expect("in range");
            let (key, src) = entry_key(ast, d);
            let si = self.sets.entry(set).or_default();
            si.keys.push(key);
            si.src.push(src);
            si.prev_same.push(NIL);
            si.link(pos as u32);
            self.watch(src, (u64::from(set) << 32) | pos as u64);
        }
    }

    /// The positions of `set` keyed `key` or [`ALWAYS`], ascending.
    fn query(&self, set: u32, key: u64) -> Vec<u32> {
        let si = &self.sets[&set];
        let start = |k: u64| si.head.get(&k).copied().unwrap_or(NIL);
        let (mut a, mut b) = (start(key), start(ALWAYS));
        let mut out = Vec::new();
        while a != NIL || b != NIL {
            // NIL is u32::MAX: take the larger of the live heads.
            let take_a = b == NIL || (a != NIL && a > b);
            if take_a {
                out.push(a);
                a = si.prev_same[a as usize];
            } else {
                out.push(b);
                b = si.prev_same[b as usize];
            }
        }
        out.reverse();
        out
    }
}

impl Sema<'_, '_> {
    /// The entries of `(ctx, name)`'s lookup entry (with their flags, in
    /// order) that can take part in the redeclaration search of `new`
    /// (`pick_prev_function`): all of them for a small entry, a
    /// non-prototype `new` or under [`Sema::full_redecl_scan`]; else the
    /// subsequence the redeclaration index keeps (module docs).
    pub(crate) fn redecl_candidates(
        &mut self,
        ctx: DeclId,
        name: DeclName,
        new: &NewFn,
    ) -> Vec<(DeclId, LookupFlags)> {
        let full = |s: &Self| s.ast.lookup_local(ctx, name).iter_with_flags().collect();
        if self.full_redecl_scan {
            return full(self);
        }
        let table = self.ast.primary_context(self.ast.redecl_context(ctx));
        let Some(set) = self.ast.lookup.set_id(table, name) else { return full(self) };
        if self.ast.lookup.set_len(set) < MIN_LEN {
            return full(self);
        }
        let key = type_key(&self.ast, Some(new.ty), new.head.is_some());
        if key == ALWAYS {
            return full(self);
        }
        let mut ix = std::mem::take(&mut self.redecl_index);
        ix.sync(&mut self.ast);
        ix.extend(&self.ast, set);
        let out = ix
            .query(set, key)
            .into_iter()
            .map(|pos| self.ast.lookup.set_entry(set, pos as usize).expect("in range"))
            .collect::<Vec<_>>();
        #[cfg(feature = "op-stats")]
        crate::templ::deduce_stats::index_query(out.len(), ix.entries(), ix.heap_bytes());
        self.redecl_index = ix;
        out
    }
}
