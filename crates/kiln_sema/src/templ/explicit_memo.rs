//! The explicit-argument substitution memo of deduction step 1 (WP-661;
//! spec/sema-templates.md §6.1).
//!
//! Step 1 substitutes a call's explicit template arguments into the
//! template's function parameter types (and, for an address or a
//! declaration, its whole function type) on every deduction attempt. The
//! generated binding code repeats the same `f<X>(...)` call shape thousands of
//! times, and almost every repeat substitutes the same arguments into the
//! same types again. This memo answers such a repeat with the earlier
//! result.
//!
//! A result is kept only when its substitution provably did nothing but
//! compute types from types that already existed: it succeeded with no
//! SFINAE error, held back or emitted no diagnostic, created no AST node (no
//! declaration, expression, statement, list or lookup entry: the
//! instantiation report's `nodes` and `bytes` did not move), and pushed no
//! instantiation frame below its own. Such a substitution has nothing left
//! that could answer differently later: every class it looked into was
//! already complete (completing one creates nodes), and the names it
//! found cannot be unfound. A failure is never kept, so every
//! substitution-failure diagnostic is still produced where and as often
//! as before, and a template declared in a local scope (whose access
//! context is the caller's) is never kept. The deduction's own
//! `ExplicitTemplateArgumentSubstitution` frame is still pushed and popped
//! on a memo answer, so the instantiation report keeps every row, with
//! the same counts, nodes and bytes.
//!
//! The memo is direct-mapped with a fixed number of slots, allocated on
//! the first kept result: its memory is bounded whatever the translation
//! unit's size (a slot holds up to [`MAX_TYS`] types in and out; longer
//! lists are not kept). `Sema::full_deduction` turns it off.

use kiln_ast::ids::ArgListId;
use kiln_ast::{DeclId, QualType};

/// The slot count (a power of two).
const SLOTS: usize = 2048;
/// The longest parameter type list a slot holds, in and out.
pub(crate) const MAX_TYS: usize = 4;

/// The inputs a substitution is keyed by: the template, its parameter
/// depth, the (sugared) explicit argument list of that level, the
/// template's function type when the whole type is substituted, and the
/// parameter types substituted into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ExplicitKey {
    pub ft: DeclId,
    pub depth: u16,
    pub args: ArgListId,
    pub whole: Option<QualType>,
    n: u8,
    tys: [Option<QualType>; MAX_TYS],
}

impl ExplicitKey {
    /// The key, `None` when `tys` is too long to keep.
    pub(crate) fn new(
        ft: DeclId,
        depth: u16,
        args: ArgListId,
        whole: Option<QualType>,
        tys: &[QualType],
    ) -> Option<Self> {
        if tys.len() > MAX_TYS {
            return None;
        }
        let mut a = [None; MAX_TYS];
        for (s, &t) in a.iter_mut().zip(tys) {
            *s = Some(t);
        }
        Some(ExplicitKey { ft, depth, args, whole, n: tys.len() as u8, tys: a })
    }

    fn slot(&self) -> usize {
        use std::hash::{Hash, Hasher};
        let mut h = kiln_source::fx::FxHasher::default();
        (self.ft, self.args, self.whole, self.depth).hash(&mut h);
        // Fx's high bits are its best mixed.
        (h.finish() >> 40) as usize & (SLOTS - 1)
    }
}

/// A kept substitution: the key and the substituted parameter types,
/// whole function type and count of expanded non-trailing packs.
#[derive(Clone, Copy, Debug)]
struct Entry {
    key: ExplicitKey,
    n: u8,
    kept: u8,
    out: [Option<QualType>; MAX_TYS],
    whole: Option<QualType>,
}

/// The memo (see the module documentation).
#[derive(Default, Debug)]
pub(crate) struct ExplicitMemo {
    slots: Vec<Option<Entry>>,
}

impl ExplicitMemo {
    /// The kept result for `key`.
    pub(crate) fn get(
        &self,
        key: &ExplicitKey,
    ) -> Option<(Vec<QualType>, Option<QualType>, usize)> {
        let e = self.slots.get(key.slot())?.as_ref()?;
        (e.key == *key).then(|| {
            let v = e.out[..usize::from(e.n)].iter().map(|t| t.expect("kept")).collect();
            (v, e.whole, usize::from(e.kept))
        })
    }

    /// Keeps `out`, `whole` and `kept` for `key` (replacing the slot's
    /// entry); nothing when `out` is too long.
    pub(crate) fn put(
        &mut self,
        key: ExplicitKey,
        out: &[QualType],
        whole: Option<QualType>,
        kept: usize,
    ) {
        let (Ok(kept), true) = (u8::try_from(kept), out.len() <= MAX_TYS) else { return };
        if self.slots.is_empty() {
            self.slots = vec![None; SLOTS];
        }
        let mut o = [None; MAX_TYS];
        for (s, &t) in o.iter_mut().zip(out) {
            *s = Some(t);
        }
        let i = key.slot();
        self.slots[i] = Some(Entry { key, n: out.len() as u8, kept, out: o, whole });
    }

    /// Heap bytes (the memory report's `sema.explicit_memo` row).
    pub(crate) fn heap_bytes(&self) -> usize {
        self.slots.capacity() * size_of::<Option<Entry>>()
    }

    /// Slots in use.
    pub(crate) fn len(&self) -> usize {
        self.slots.iter().filter(|e| e.is_some()).count()
    }
}
