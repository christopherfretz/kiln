//! The VTT (Itanium §2.6.2): a port of clang 19's `VTTBuilder`
//! (`VTTBuilder.cpp`).

use kiln_ast::DeclId;
use kiln_source::fx::FxHashSet;

use super::VTableError;
use super::classes::Classes;

/// A base subobject: its class and its offset in the most-derived class.
pub(crate) type Base = (DeclId, i64);

/// clang `VTTBuilder`'s output.
#[derive(Debug, Default)]
pub(crate) struct VttPlan {
    /// The vtable groups the VTT points into: index 0 is the complete
    /// object's own group, the others construction groups, with
    /// whether the base is virtual.
    pub vtables: Vec<(Base, bool)>,
    /// The entries: (vtable group index, subobject whose address point
    /// the entry holds).
    pub components: Vec<(u32, Base)>,
}

/// Lays out the VTT of `md` (empty without virtual bases).
pub(crate) fn plan(cx: &mut Classes<'_>, md: DeclId) -> Result<VttPlan, VTableError> {
    let mut b = VttBuilder { md, plan: VttPlan::default() };
    b.layout_vtt(cx, (md, 0), false)?;
    Ok(b.plan)
}

struct VttBuilder {
    md: DeclId,
    plan: VttPlan,
}

impl VttBuilder {
    fn layout_vtt(
        &mut self,
        cx: &mut Classes<'_>,
        base: Base,
        is_virtual: bool,
    ) -> Result<(), VTableError> {
        cx.tick()?;
        let info = cx.get(base.0);
        if !info.has_vbases() {
            return Ok(());
        }
        let idx = u32::try_from(self.plan.vtables.len()).map_err(|_| VTableError::TooComplex)?;
        self.plan.vtables.push((base, is_virtual));
        self.plan.components.push((idx, base));
        for &(b, v) in &info.bases {
            if !v {
                self.layout_vtt(cx, (b, base.1 + info.base_offset(b)), false)?;
            }
        }
        let mut seen = FxHashSet::default();
        self.secondary_virtual_pointers(cx, base, false, idx, &mut seen)?;
        if base.0 == self.md {
            let mut seen = FxHashSet::default();
            let mut explored = FxHashSet::default();
            self.layout_virtual_vtts(cx, self.md, &mut seen, &mut explored)?;
        }
        Ok(())
    }

    fn secondary_virtual_pointers(
        &mut self,
        cx: &mut Classes<'_>,
        base: Base,
        morally_virtual: bool,
        idx: u32,
        seen: &mut FxHashSet<DeclId>,
    ) -> Result<(), VTableError> {
        cx.tick()?;
        let info = cx.get(base.0);
        if !info.has_vbases() && !morally_virtual {
            return Ok(());
        }
        for &(b, v) in &info.bases {
            let binfo = cx.get(b);
            if !binfo.dynamic {
                continue;
            }
            let (bmv, nv_primary, bo) = if v {
                if !seen.insert(b) {
                    continue;
                }
                (true, false, cx.get(self.md).vbase_offset(b))
            } else {
                (morally_virtual, info.primary == Some((b, false)), base.1 + info.base_offset(b))
            };
            if !nv_primary && (binfo.has_vbases() || bmv) {
                self.plan.components.push((idx, (b, bo)));
            }
            self.secondary_virtual_pointers(cx, (b, bo), bmv, idx, seen)?;
        }
        Ok(())
    }

    fn layout_virtual_vtts(
        &mut self,
        cx: &mut Classes<'_>,
        rd: DeclId,
        seen: &mut FxHashSet<DeclId>,
        explored: &mut FxHashSet<DeclId>,
    ) -> Result<(), VTableError> {
        cx.tick()?;
        let info = cx.get(rd);
        for &(b, v) in &info.bases {
            if v && seen.insert(b) {
                let bo = cx.get(self.md).vbase_offset(b);
                self.layout_vtt(cx, (b, bo), true)?;
            }
            if cx.get(b).has_vbases() && explored.insert(b) {
                self.layout_virtual_vtts(cx, b, seen, explored)?;
            }
        }
        Ok(())
    }
}
