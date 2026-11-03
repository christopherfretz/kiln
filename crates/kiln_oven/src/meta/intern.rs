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

/// The interned sizes before a call.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReflMark {
    content: u64,
    entries: u64,
}

impl ReflMark {
    /// The sizes now.
    pub(crate) fn of(b: &mut dyn SemaBridge) -> ReflMark {
        let entries = b.ast().reflections.len() as u64;
        ReflMark { content: b.consts().content_bytes(), entries }
    }
}

impl Oven {
    /// Charges what the translation unit's interned tables grew by since
    /// `m` to the running evaluation.
    ///
    /// # Errors
    /// [`Fault::Memory`] when the bytes exceed the memory limit.
    pub(crate) fn charge_refl(&mut self, b: &mut dyn SemaBridge, m: ReflMark) -> Result<(), Fault> {
        let now = ReflMark::of(b);
        let n = now.content.saturating_sub(m.content)
            + now.entries.saturating_sub(m.entries) * REFL_ENTRY_BYTES;
        if n == 0 {
            return Ok(());
        }
        self.run.counters.refl += n;
        self.mem.charge_refl(n).map_err(|_| Fault::Memory)
    }
}
