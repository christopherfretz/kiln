//! The empty-subobject map (Itanium ABI §2.4 "empty subobject"
//! conflicts; clang `EmptySubobjectMap`): two distinct subobjects of the
//! same empty class type must not share an address ([intro.object]/9),
//! so a base or field whose empty subobjects would land on an offset
//! already holding an empty subobject of the same type is moved.
//!
//! A base subobject is walked through the class's base-subobject tree
//! ([`BaseInfos`]): its non-virtual bases, the virtual primary base it
//! claimed (at its own offset), and its fields of class or
//! array-of-class type. A field's class is walked through layouts: its
//! non-virtual bases and, for the field's own class only, all its
//! virtual bases at their offsets (clang `CanPlaceFieldSubobjectAtOffset`
//! with `RD == Class`). Bit-fields are skipped. Only offsets below the
//! size of the largest empty subobject are recorded, except while placing
//! an empty base or a `[[no_unique_address]]` field (clang's cut-off: a
//! later subobject is only ever tried at 0 or at `dsize` onwards).
//!
//! The traversals use explicit stacks, so deeply nested classes cannot
//! exhaust the native stack.

use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;

use super::base_info::BaseInfos;
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

/// One subobject still to walk.
#[derive(Clone, Copy)]
enum Node {
    /// A node of the base-subobject tree at a byte offset.
    Info(u32, u64),
    /// A class at a byte offset; `top` when it is the class of a field
    /// (or array element), whose virtual bases are walked too.
    Class(DeclId, u64, bool),
}

/// Pushes the subobjects a field of `target` at `off` starts with.
fn push_target(
    lay: &Layouter<'_>,
    stack: &mut Vec<Node>,
    target: FieldTarget,
    off: u64,
    keep: &dyn Fn(u64) -> bool,
) {
    match target {
        FieldTarget::None => {}
        FieldTarget::Record(r) => {
            if keep(off) {
                stack.push(Node::Class(r, off, true));
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
                stack.push(Node::Class(r, eo, true));
                eo = eo.saturating_add(step);
            }
        }
    }
}

/// Visits every class subobject reachable from `stack` whose offset
/// passes `keep` (subtrees below a rejected offset are skipped), stopping
/// as soon as `visit` returns false. Returns false if it stopped.
fn walk(
    lay: &Layouter<'_>,
    infos: &BaseInfos,
    mut stack: Vec<Node>,
    keep: &dyn Fn(u64) -> bool,
    visit: &mut dyn FnMut(DeclId, bool, u64) -> bool,
) -> bool {
    while let Some(node) = stack.pop() {
        let (class, off, info, top) = match node {
            Node::Info(i, off) => (infos.node(i).class, off, Some(i), false),
            Node::Class(c, off, top) => (c, off, None, top),
        };
        if !keep(off) {
            continue;
        }
        let Some(facts) = lay.facts(class) else { continue };
        if !visit(class, facts.is_empty, off) {
            return false;
        }
        let layout = &facts.layout;
        let base_off =
            |b: DeclId| layout.bases.iter().find(|(d, _)| *d == b).map_or(0, |&(_, o)| o);
        if let Some(i) = info {
            let n = infos.node(i);
            for &c in &n.bases {
                let child = infos.node(c);
                if !child.is_virtual {
                    stack.push(Node::Info(c, off.saturating_add(base_off(child.class))));
                }
            }
            if let Some(p) = n.primary_vbase
                && infos.node(p).derived == Some(i)
            {
                stack.push(Node::Info(p, off));
            }
        } else {
            for &(b, bo) in &layout.bases {
                stack.push(Node::Class(b, off.saturating_add(bo), false));
            }
            if top {
                for &(v, vo) in &layout.vbases {
                    stack.push(Node::Class(v, off.saturating_add(vo), false));
                }
            }
        }
        for (i, f) in facts.field_targets.iter().enumerate() {
            let fo = off.saturating_add(layout.fields.get(i).copied().unwrap_or(0) / 8);
            push_target(lay, &mut stack, *f, fo, keep);
        }
    }
    true
}

impl EmptySubobjectMap {
    pub(super) fn new(largest: u64) -> Self {
        EmptySubobjectMap { offsets: FxHashMap::default(), max_offset: 0, largest }
    }

    /// clang `CanPlaceBaseAtOffset`: checks the base subobject `info`
    /// at byte offset `off` and, on success, records its empty
    /// subobjects (all of them when the base itself is empty).
    pub(super) fn can_place_base(
        &mut self,
        lay: &Layouter<'_>,
        infos: &BaseInfos,
        info: u32,
        off: u64,
    ) -> bool {
        // A class without empty subobjects cannot conflict.
        if self.largest == 0 {
            return true;
        }
        let start = vec![Node::Info(info, off)];
        if !self.can_place(lay, infos, start.clone()) {
            return false;
        }
        let empty = lay.facts(infos.node(info).class).is_some_and(|f| f.is_empty);
        self.update(lay, infos, start, empty);
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
        let infos = BaseInfos::default();
        let max = self.max_offset;
        let mut start = Vec::new();
        push_target(lay, &mut start, target, off, &|o| o <= max);
        if !self.can_place(lay, &infos, start) {
            return false;
        }
        let largest = self.largest;
        let mut start = Vec::new();
        push_target(lay, &mut start, target, off, &|o| overlapping || o < largest);
        self.update(lay, &infos, start, overlapping);
        true
    }

    fn can_place(&self, lay: &Layouter<'_>, infos: &BaseInfos, start: Vec<Node>) -> bool {
        // clang `AnyEmptySubobjectsBeyondOffset`.
        let max = self.max_offset;
        let offsets = &self.offsets;
        walk(lay, infos, start, &|o| o <= max, &mut |class, empty, off| {
            !(empty && offsets.get(&off).is_some_and(|v| v.contains(&class)))
        })
    }

    fn update(&mut self, lay: &Layouter<'_>, infos: &BaseInfos, start: Vec<Node>, all: bool) {
        let largest = self.largest;
        let offsets = &mut self.offsets;
        let max_offset = &mut self.max_offset;
        walk(lay, infos, start, &|o| all || o < largest, &mut |class, empty, off| {
            if empty {
                let v = offsets.entry(off).or_default();
                if !v.contains(&class) {
                    v.push(class);
                    *max_offset = (*max_offset).max(off);
                }
            }
            true
        });
    }
}
