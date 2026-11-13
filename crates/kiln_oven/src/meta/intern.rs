//! Reflection bytes (`refl_bytes`; spec/oven.md "Reflection bytes",
//! WP-479). Reflection values, the content they designate
//! (`reflect_constant`, `reflect_constant_array`, names) and the
//! reflections themselves are interned for the translation unit: the
//! constant store by content, the reflection table by reflectee. What
//! an intrinsic call adds to either is charged to the running
//! evaluation's live bytes, so it counts into `peak_bytes` and the
//! `-fconstexpr-memory` limit, and is reported as `refl_bytes`. A call
//! that finds its result already interned adds, and charges, nothing.

use kiln_ast::ReflectionId;
use kiln_ast::reflect::Reflectee;

use crate::vm::Fault;
use crate::{Oven, SemaBridge};

/// The bytes one reflection table entry holds: its reflectee, and its
/// key and value in the intern map (one control byte).
pub(crate) const REFL_ENTRY_BYTES: u64 =
    (2 * size_of::<Reflectee>() + size_of::<ReflectionId>() + 1) as u64;

/// Interned growth attributed to intrinsic calls so far (charged or
/// not), cumulative over the Oven's lifetime: an outer call subtracts
/// what the calls nested inside it (in evaluations its bridge callback
/// ran) already attributed, so no byte is charged twice.
#[derive(Clone, Copy, Debug, Default, kiln_snap::Snap)]
pub(crate) struct ReflSeen {
    content: u64,
    entries: u64,
}

/// The interned sizes before a call.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReflMark {
    content: u64,
    entries: u64,
    seen: ReflSeen,
}

impl Oven {
    /// The sizes now.
    pub(crate) fn refl_mark(&self, b: &mut dyn SemaBridge) -> ReflMark {
        let entries = b.ast().reflections.len() as u64;
        ReflMark { content: b.consts().content_bytes(), entries, seen: self.refl_seen }
    }

    /// Charges what the translation unit's interned tables grew by since
    /// `m`, less what calls nested in this one attributed, to the running
    /// evaluation. New reflection entries count for every op; constant
    /// store content only when `content` (the value and name families,
    /// whose results are interned by content): what an op's template
    /// instantiation or class completion interns (`substitute`,
    /// `members_of`) is not reflection storage.
    ///
    /// # Errors
    /// [`Fault::Memory`] when the bytes exceed the memory limit.
    pub(crate) fn charge_refl(
        &mut self,
        b: &mut dyn SemaBridge,
        m: ReflMark,
        content: bool,
    ) -> Result<(), Fault> {
        let now = self.refl_mark(b);
        let nested = |grew: u64, seen_now: u64, seen_then: u64| {
            grew.saturating_sub(seen_now.saturating_sub(seen_then))
        };
        let own_content =
            nested(now.content.saturating_sub(m.content), self.refl_seen.content, m.seen.content);
        let own_entries =
            nested(now.entries.saturating_sub(m.entries), self.refl_seen.entries, m.seen.entries);
        self.refl_seen.content += own_content;
        self.refl_seen.entries += own_entries;
        let n = if content { own_content } else { 0 } + own_entries * REFL_ENTRY_BYTES;
        if n == 0 {
            return Ok(());
        }
        self.run.counters.refl += n;
        self.mem.charge_refl(n).map_err(|_| Fault::Memory)
    }
}
