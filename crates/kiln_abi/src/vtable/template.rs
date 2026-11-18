//! Construction-group templates (WP-1650, spec/abi.md §8.5 "Per-class
//! and per-method memos").
//!
//! The construction group of a class `md` laid out at `md_offset` in a
//! layout class `L` depends on `L` only through the parameters
//! `P = (md_offset, L's offset of each virtual base of md)`: every
//! layout-class offset the builder computes is an affine form
//! `P[sym] + c` ([`Off`]). While building a construction group, the
//! builder records each component that is a difference of two such
//! offsets ([`Diff`]), its address points as forms, and every comparison
//! between forms of two different symbols with its outcome ([`Guard`]).
//! A later construction group of the same `md` (same `md_virtual`) whose
//! parameters satisfy every guard takes the same decisions, so it is the
//! recorded group with the recorded values re-evaluated
//! ([`Template::instantiate`]): no final-overrider map and no walk.
//!
//! Each offset also carries its concrete value, which every decision and
//! every hash key of the builder uses, so a recorded build computes
//! exactly what an unrecorded one does.

use std::cell::Cell;

use kiln_ast::DeclId;
use kiln_ast::class::VTableComponent;
use kiln_source::fx::FxHashMap;

use super::builder::Built;
use super::classes::Classes;

/// The symbol of the constant 0 (and of every offset in concrete mode).
const ABS: u32 = 0;
/// The symbol of the most-derived class's offset in the layout class.
const MD: u32 = 1;

/// A layout-class offset: `P[sym] + c`, with its concrete value `val`.
/// Compare two only through [`Syms::eq`] (it records the guards).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Off {
    sym: u32,
    c: i64,
    pub val: i64,
}

impl Off {
    /// A concrete offset (no symbol).
    pub fn abs(val: i64) -> Off {
        Off { sym: ABS, c: val, val }
    }

    /// The offset `d` bytes further.
    pub fn add(self, d: i64) -> Off {
        Off { sym: self.sym, c: self.c.wrapping_add(d), val: self.val.wrapping_add(d) }
    }

    /// `self - other` as a recorded component value.
    pub fn minus(self, other: Off) -> Diff {
        Diff { plus: self.sym, minus: other.sym, c: self.c.wrapping_sub(other.c) }
    }

    /// The concrete value of `self - other`.
    pub fn val_minus(self, other: Off) -> i64 {
        self.val.wrapping_sub(other.val)
    }
}

/// A component value `P[plus] - P[minus] + c`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Diff {
    plus: u32,
    minus: u32,
    c: i64,
}

impl Diff {
    fn eval(self, p: &[i64]) -> Option<i64> {
        let a = *p.get(self.plus as usize)?;
        let b = *p.get(self.minus as usize)?;
        Some(a.wrapping_sub(b).wrapping_add(self.c))
    }
}

/// A recorded comparison: `(P[a] + ac == P[b] + bc) == eq`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Guard {
    a: u32,
    ac: i64,
    b: u32,
    bc: i64,
    eq: bool,
}

/// The parameters of one build and what it records.
pub(crate) struct Syms {
    layout_class: DeclId,
    /// Recording (a construction group) or concrete (a complete group:
    /// every offset is `ABS + value`).
    symbolic: bool,
    /// `P`: `[0, md_offset, L(v0), L(v1), ...]` (symbolic mode).
    p: Vec<i64>,
    /// The virtual bases of `md` (first declarations), `v_i` = symbol `2 + i`.
    vbases: Vec<DeclId>,
    guards: Vec<Guard>,
    /// Something not expressible as a form was used: no template.
    poisoned: Cell<bool>,
    /// The component values recorded as forms (by component index).
    pub diffs: Vec<(u32, Diff)>,
}

impl Syms {
    /// Concrete mode over `layout_class` (records nothing).
    pub fn concrete(layout_class: DeclId) -> Syms {
        Syms {
            layout_class,
            symbolic: false,
            p: Vec::new(),
            vbases: Vec::new(),
            guards: Vec::new(),
            poisoned: Cell::new(false),
            diffs: Vec::new(),
        }
    }

    /// Recording mode for the group of `md` at `md_offset` in
    /// `layout_class`.
    pub fn symbolic(
        cx: &mut Classes<'_>,
        md: DeclId,
        md_offset: i64,
        layout_class: DeclId,
    ) -> Syms {
        let md_info = cx.get(md);
        let l_info = cx.get(layout_class);
        let mut p = Vec::with_capacity(2 + md_info.vbase_offsets.len());
        p.push(0);
        p.push(md_offset);
        let mut vbases = Vec::with_capacity(md_info.vbase_offsets.len());
        for &(v, _) in &md_info.vbase_offsets {
            vbases.push(v);
            p.push(l_info.vbase_offset(v));
        }
        Syms {
            layout_class,
            symbolic: true,
            p,
            vbases,
            guards: Vec::new(),
            poisoned: Cell::new(false),
            diffs: Vec::new(),
        }
    }

    pub fn is_symbolic(&self) -> bool {
        self.symbolic
    }

    /// The most-derived class's own offset `md_offset`.
    pub fn md(&self, md_offset: i64) -> Off {
        if self.symbolic {
            Off { sym: MD, c: 0, val: md_offset }
        } else {
            Off { sym: ABS, c: md_offset, val: md_offset }
        }
    }

    /// The offset of virtual base `v` in the layout class.
    pub fn vbase(&self, cx: &mut Classes<'_>, v: DeclId) -> Off {
        if self.symbolic {
            let v = cx.ast().first(v);
            if let Some(i) = self.vbases.iter().position(|&x| x == v)
                && let Some(&val) = self.p.get(i + 2)
                && let Ok(sym) = u32::try_from(i + 2)
            {
                return Off { sym, c: 0, val };
            }
            self.poisoned.set(true);
        }
        let val = cx.get(self.layout_class).vbase_offset(v);
        Off { sym: ABS, c: val, val }
    }

    /// `a == b`, recording the outcome when the forms differ in symbol.
    pub fn eq(&mut self, a: Off, b: Off) -> bool {
        let r = a.val == b.val;
        if self.symbolic && a.sym != b.sym {
            self.guards.push(Guard { a: a.sym, ac: a.c, b: b.sym, bc: b.c, eq: r });
        }
        r
    }

    /// Inserts `(class, off)` into a set keyed by concrete offset; false
    /// if it was there (the same subobject reached as another form: the
    /// coincidence is recorded).
    pub fn insert(
        &mut self,
        set: &mut FxHashMap<(DeclId, i64), Off>,
        class: DeclId,
        off: Off,
    ) -> bool {
        if let Some(&prev) = set.get(&(class, off.val)) {
            self.eq(prev, off);
            return false;
        }
        set.insert((class, off.val), off);
        true
    }

    /// Records component `index` as the value `d`.
    pub fn record(&mut self, index: usize, d: Diff) {
        if !self.symbolic {
            return;
        }
        match u32::try_from(index) {
            Ok(i) => self.diffs.push((i, d)),
            Err(_) => self.poisoned.set(true),
        }
    }

    /// The template of the group `built` this recording produced (none
    /// if something was not expressible).
    pub fn into_template(
        mut self,
        built: &Built,
        md_virtual: bool,
        aps: Vec<((DeclId, Off), u32)>,
    ) -> Option<Template> {
        if !self.symbolic || self.poisoned.get() {
            return None;
        }
        self.guards.sort_unstable();
        self.guards.dedup();
        self.guards.shrink_to_fit();
        self.diffs.shrink_to_fit();
        // The address points are kept as forms only, compactly (an offset
        // beyond `i32` gets no template).
        let mut forms = Vec::with_capacity(aps.len());
        for ((class, o), ap) in aps {
            forms.push(ApForm { class, sym: o.sym, ap, c: i32::try_from(o.c).ok()? });
        }
        Some(Template {
            md_virtual,
            vbases: self.vbases,
            guards: self.guards,
            built: Built {
                most_derived: built.most_derived,
                md_offset: built.md_offset,
                layout_class: built.layout_class,
                components: built.components.clone(),
                vtable_starts: built.vtable_starts.clone(),
                address_points: Vec::new(),
                slot_thunks: built.slot_thunks.clone(),
                thunks: built.thunks.clone(),
                vbase_offset_offsets: built.vbase_offset_offsets.clone(),
                method_indices: built.method_indices.clone(),
            },
            diffs: self.diffs,
            aps: forms,
        })
    }
}

/// An address point of a [`Template`]: subobject `class` at `P[sym] + c`
/// has address point `ap`.
#[derive(Clone, Copy, Debug)]
struct ApForm {
    class: DeclId,
    sym: u32,
    ap: u32,
    c: i32,
}

/// A recorded construction group of one class ([`Syms`]); `Classes`
/// keeps the last one per class. Its `built` has no address points
/// (`aps` holds them as forms).
#[derive(Debug)]
pub(crate) struct Template {
    md_virtual: bool,
    vbases: Vec<DeclId>,
    guards: Vec<Guard>,
    built: Built,
    diffs: Vec<(u32, Diff)>,
    aps: Vec<ApForm>,
}

impl Template {
    /// The group for parameters `syms` (made by [`Syms::symbolic`] for
    /// the same class), if every guard holds.
    pub fn instantiate(&self, syms: &Syms, md_virtual: bool, md_offset: i64) -> Option<Built> {
        if md_virtual != self.md_virtual || syms.vbases != self.vbases {
            return None;
        }
        let p = &syms.p;
        for g in &self.guards {
            let a = p.get(g.a as usize)?.wrapping_add(g.ac);
            let b = p.get(g.b as usize)?.wrapping_add(g.bc);
            if (a == b) != g.eq {
                return None;
            }
        }
        let mut components = self.built.components.clone();
        for &(i, d) in &self.diffs {
            let v = d.eval(p)?;
            match components.get_mut(i as usize)? {
                VTableComponent::VCallOffset(x)
                | VTableComponent::VBaseOffset(x)
                | VTableComponent::OffsetToTop(x) => *x = v,
                _ => return None,
            }
        }
        let mut address_points = Vec::with_capacity(self.aps.len());
        for a in &self.aps {
            let val = p.get(a.sym as usize)?.wrapping_add(i64::from(a.c));
            address_points.push(((a.class, val), a.ap));
        }
        Some(Built {
            most_derived: self.built.most_derived,
            md_offset,
            layout_class: syms.layout_class,
            components,
            vtable_starts: self.built.vtable_starts.clone(),
            address_points,
            slot_thunks: self.built.slot_thunks.clone(),
            thunks: self.built.thunks.clone(),
            vbase_offset_offsets: self.built.vbase_offset_offsets.clone(),
            method_indices: self.built.method_indices.clone(),
        })
    }
}
