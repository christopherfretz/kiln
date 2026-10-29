//! The empty-subobject map (Itanium ABI §2.4 "empty subobject"
//! conflicts; clang `EmptySubobjectMap`): two distinct subobjects of the
//! same empty class type must not share an address ([intro.object]/9),
//! so a base or field whose empty subobjects would land on an offset
//! already holding an empty subobject of the same type is moved.
//!
//! A class subobject's own subobjects are its non-virtual bases (at
//! their offsets in its layout) and its fields of class or array-of-class
//! type; bit-fields are skipped. Only offsets below the size of the
//! largest empty subobject are recorded, except while placing an empty
//! base or a `[[no_unique_address]]` field (clang's cut-off: a later
//! subobject is only ever tried at 0 or at `dsize` onwards).
//!
//! The traversals use explicit stacks, so deeply nested classes cannot
//! exhaust the native stack.

use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;

use super::{FieldTarget, Layouter};

/// Offsets (bytes) at which empty class subobjects have been placed.
#[derive(Debug)]
pub(super) struct EmptySubobjectMap {
    offsets: FxHashMap<u64, Vec<DeclId>>,
    /// clang `MaxEmptyClassOffset`; 0 initially.
    max_offset: u64,
    /// clang `SizeOfLargestEmptySubobject` (bytes).
    largest: u64,
}

impl EmptySubobjectMap {
    pub(super) fn new(largest: u64) -> Self {
        EmptySubobjectMap { offsets: FxHashMap::default(), max_offset: 0, largest }
    }

    /// clang `AnyEmptySubobjectsBeyondOffset`.
    fn any_beyond(&self, off: u64) -> bool {
        off <= self.max_offset
    }

    /// clang `CanPlaceBaseAtOffset`: checks the base subobject of class
    /// `class` at byte offset `off` and, on success, records its empty
    /// subobjects (all of them when the base itself is empty).
    pub(super) fn can_place_base(&mut self, lay: &Layouter<'_>, class: DeclId, off: u64) -> bool {
        // A class without empty subobjects cannot conflict.
        if self.largest == 0 {
            return true;
        }
        let target = FieldTarget::Record(class);
        if !self.can_place(lay, target, off) {
            return false;
        }
        let empty = lay.facts(class).is_some_and(|f| f.is_empty);
        self.update(lay, target, off, empty);
        true
    }

    /// clang `CanPlaceFieldAtOffset`: checks, and on success records the
    /// field's empty subobjects (all of them when `overlapping`, i.e. for
    /// a `[[no_unique_address]]` field).
    pub(super) fn can_place_field(
        &mut self,
        lay: &Layouter<'_>,
        target: FieldTarget,
        off: u64,
        overlapping: bool,
    ) -> bool {
        if !self.can_place(lay, target, off) {
            return false;
        }
        self.update(lay, target, off, overlapping);
        true
    }

    fn push_target(
        lay: &Layouter<'_>,
        stack: &mut Vec<(DeclId, u64)>,
        target: FieldTarget,
        off: u64,
        mut keep: impl FnMut(u64) -> bool,
    ) {
        match target {
            FieldTarget::None => {}
            FieldTarget::Record(r) => {
                if keep(off) {
                    stack.push((r, off));
                }
            }
            FieldTarget::Array(r, n) => {
                let step = lay.record_size(r);
                let n = if step == 0 { n.min(1) } else { n };
                let mut eo = off;
                for _ in 0..n {
                    if !keep(eo) {
                        break;
                    }
                    stack.push((r, eo));
                    eo = eo.saturating_add(step);
                }
            }
        }
    }

    fn can_place(&self, lay: &Layouter<'_>, target: FieldTarget, off: u64) -> bool {
        let mut stack = Vec::new();
        Self::push_target(lay, &mut stack, target, off, |o| self.any_beyond(o));
        while let Some((r, off)) = stack.pop() {
            if !self.any_beyond(off) {
                continue;
            }
            let Some(facts) = lay.facts(r) else { continue };
            if facts.is_empty && self.offsets.get(&off).is_some_and(|v| v.contains(&r)) {
                return false;
            }
            for &(base, bo) in &facts.layout.bases {
                let bo = off.saturating_add(bo);
                if self.any_beyond(bo) {
                    stack.push((base, bo));
                }
            }
            for (i, f) in facts.field_targets.iter().enumerate() {
                let fo = off.saturating_add(facts.layout.fields.get(i).copied().unwrap_or(0) / 8);
                Self::push_target(lay, &mut stack, *f, fo, |o| self.any_beyond(o));
            }
        }
        true
    }

    fn update(&mut self, lay: &Layouter<'_>, target: FieldTarget, off: u64, ov: bool) {
        let largest = self.largest;
        let keep = |o: u64| ov || o < largest;
        let mut stack = Vec::new();
        Self::push_target(lay, &mut stack, target, off, keep);
        while let Some((r, off)) = stack.pop() {
            if !keep(off) {
                continue;
            }
            let Some(facts) = lay.facts(r) else { continue };
            if facts.is_empty {
                let v = self.offsets.entry(off).or_default();
                if !v.contains(&r) {
                    v.push(r);
                    self.max_offset = self.max_offset.max(off);
                }
            }
            for &(base, bo) in &facts.layout.bases {
                let bo = off.saturating_add(bo);
                if keep(bo) {
                    stack.push((base, bo));
                }
            }
            for (i, f) in facts.field_targets.iter().enumerate() {
                let fo = off.saturating_add(facts.layout.fields.get(i).copied().unwrap_or(0) / 8);
                Self::push_target(lay, &mut stack, *f, fo, keep);
            }
        }
    }
}
