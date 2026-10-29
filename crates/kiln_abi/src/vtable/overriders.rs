//! Final overriders per base-class subobject (clang `FinalOverriders`
//! over `CXXRecordDecl::getFinalOverriders`, [class.virtual]/2).

use std::rc::Rc;

use kiln_ast::DeclId;
use kiln_source::fx::FxHashMap;

use super::VTableError;
use super::classes::{Classes, parent};

/// The final overrider of one virtual function in one subobject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OverriderInfo {
    /// The overriding method (first declaration).
    pub method: DeclId,
    /// The virtual base whose subobject holds the overrider.
    pub vbase: Option<DeclId>,
    /// The overrider's class subobject offset in the layout class.
    pub offset: i64,
}

/// clang `UniqueVirtualMethod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Uvm {
    method: DeclId,
    subobject: u32,
    in_virtual: Option<DeclId>,
}

/// clang `CXXFinalOverriderMap`: per virtual function, per subobject
/// number, the overriders.
type OvMap = FxHashMap<DeclId, Vec<(u32, Vec<Uvm>)>>;

fn add(map: &mut OvMap, m: DeclId, sub: u32, uvm: Uvm) {
    let entries = map.entry(m).or_default();
    match entries.iter_mut().find(|(s, _)| *s == sub) {
        Some((_, list)) => {
            if !list.contains(&uvm) {
                list.push(uvm);
            }
        }
        None => entries.push((sub, vec![uvm])),
    }
}

fn replace_all(map: &mut OvMap, m: DeclId, uvm: Uvm) {
    if let Some(entries) = map.get_mut(&m) {
        for (_, list) in entries {
            list.clear();
            list.push(uvm);
        }
    }
}

/// The final overriders of every virtual function in every subobject of
/// a most-derived class, keyed by (function, subobject offset in the
/// most-derived class).
pub(crate) struct FinalOverriders {
    map: FxHashMap<(DeclId, i64), OverriderInfo>,
}

impl FinalOverriders {
    /// The final overriders of `most_derived` laid out at
    /// `md_offset` within `layout_class` (equal to `most_derived` and 0
    /// outside construction vtables).
    pub fn new(
        cx: &mut Classes<'_>,
        most_derived: DeclId,
        md_offset: i64,
        layout_class: DeclId,
    ) -> Result<Self, VTableError> {
        let mut offs = Offsets {
            md: most_derived,
            layout_class,
            sub: FxHashMap::default(),
            layout: FxHashMap::default(),
            counts: FxHashMap::default(),
        };
        offs.compute(cx, most_derived, false, 0, md_offset);
        let mut collector =
            Collector { counts: FxHashMap::default(), virtuals: FxHashMap::default() };
        let mut ov = OvMap::default();
        collector.collect(cx, most_derived, false, None, &mut ov);
        // Weed out overriders in virtual bases hidden along another
        // path ([class.member.lookup]/10 for final overriders).
        for entries in ov.values_mut() {
            for (_, list) in entries.iter_mut() {
                if list.len() < 2 {
                    continue;
                }
                let mut hidden = Vec::with_capacity(list.len());
                for (i, m) in list.iter().enumerate() {
                    let h = m.in_virtual.is_some_and(|v| {
                        list.iter().enumerate().any(|(j, op)| {
                            j != i && cx.get(parent(cx.ast(), op.method)).has_vbase(v)
                        })
                    });
                    hidden.push(h);
                }
                let mut k = 0;
                list.retain(|_| {
                    k += 1;
                    !hidden[k - 1]
                });
            }
        }
        let ast = cx.ast();
        let mut map = FxHashMap::default();
        for (&md, entries) in &ov {
            for (sub, list) in entries {
                let Some(&base_offset) = offs.sub.get(&(parent(ast, md), *sub)) else {
                    return Err(VTableError::Incomplete);
                };
                let [uvm] = list.as_slice() else {
                    return Err(VTableError::Incomplete);
                };
                let Some(&offset) = offs.layout.get(&(parent(ast, uvm.method), uvm.subobject))
                else {
                    return Err(VTableError::Incomplete);
                };
                map.insert(
                    (md, base_offset),
                    OverriderInfo { method: uvm.method, vbase: uvm.in_virtual, offset },
                );
            }
        }
        Ok(FinalOverriders { map })
    }

    /// The final overrider of `method` in the subobject at `base_offset`
    /// of the most-derived class.
    pub fn get(&self, method: DeclId, base_offset: i64) -> Result<OverriderInfo, VTableError> {
        self.map.get(&(method, base_offset)).copied().ok_or(VTableError::Incomplete)
    }
}

/// clang `FinalOverriders::ComputeBaseOffsets`.
struct Offsets {
    md: DeclId,
    layout_class: DeclId,
    /// (class, subobject number) → offset in the most-derived class.
    sub: FxHashMap<(DeclId, u32), i64>,
    /// (class, subobject number) → offset in the layout class.
    layout: FxHashMap<(DeclId, u32), i64>,
    counts: FxHashMap<DeclId, u32>,
}

impl Offsets {
    fn compute(
        &mut self,
        cx: &mut Classes<'_>,
        rd: DeclId,
        is_virtual: bool,
        off: i64,
        layout_off: i64,
    ) {
        let num = if is_virtual {
            0
        } else {
            let c = self.counts.entry(rd).or_insert(0);
            *c += 1;
            *c
        };
        self.sub.insert((rd, num), off);
        self.layout.insert((rd, num), layout_off);
        let info = cx.get(rd);
        for &(b, v) in &info.bases {
            // Non-polymorphic subtrees hold no virtual functions.
            if !cx.get(b).polymorphic {
                continue;
            }
            let (bo, blo) = if v {
                if self.sub.contains_key(&(b, 0)) {
                    continue;
                }
                (cx.get(self.md).vbase_offset(b), cx.get(self.layout_class).vbase_offset(b))
            } else {
                let o = info.base_offset(b);
                (off + o, layout_off + o)
            };
            self.compute(cx, b, v, bo, blo);
        }
    }
}

/// clang `FinalOverriderCollector`.
struct Collector {
    counts: FxHashMap<DeclId, u32>,
    virtuals: FxHashMap<DeclId, Rc<OvMap>>,
}

impl Collector {
    fn collect(
        &mut self,
        cx: &mut Classes<'_>,
        rd: DeclId,
        virtual_base: bool,
        in_virtual: Option<DeclId>,
        out: &mut OvMap,
    ) {
        let num = if virtual_base {
            0
        } else {
            let c = self.counts.entry(rd).or_insert(0);
            *c += 1;
            *c
        };
        let info = cx.get(rd);
        for &(b, v) in &info.bases {
            if !cx.get(b).polymorphic {
                continue;
            }
            if v {
                let base = if let Some(m) = self.virtuals.get(&b) {
                    Rc::clone(m)
                } else {
                    let mut m = OvMap::default();
                    self.collect(cx, b, true, Some(b), &mut m);
                    let m = Rc::new(m);
                    self.virtuals.insert(b, Rc::clone(&m));
                    m
                };
                merge(out, &base);
            } else if out.is_empty() {
                self.collect(cx, b, false, in_virtual, out);
            } else {
                let mut m = OvMap::default();
                self.collect(cx, b, false, in_virtual, &mut m);
                merge(out, &m);
            }
        }
        let ast = cx.ast();
        let me = |m: DeclId| Uvm { method: m, subobject: num, in_virtual };
        for &m in &info.virtuals {
            let direct = super::classes::overridden(ast, m);
            if !direct.is_empty() {
                let mut stack = direct;
                let mut seen: Vec<DeclId> = Vec::new();
                while let Some(om) = stack.pop() {
                    if seen.contains(&om) {
                        continue;
                    }
                    seen.push(om);
                    replace_all(out, om, me(m));
                    stack.extend(super::classes::overridden(ast, om));
                }
            }
            add(out, m, num, me(m));
        }
    }
}

fn merge(out: &mut OvMap, from: &OvMap) {
    for (&m, entries) in from {
        for (sub, list) in entries {
            for &uvm in list {
                add(out, m, *sub, uvm);
            }
        }
    }
}
