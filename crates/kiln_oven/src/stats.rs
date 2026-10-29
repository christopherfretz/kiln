//! Per-evaluation counters (spec/oven.md "Limits and metrics").

use kiln_source::Loc;

use crate::EvalKind;

/// The counters of one top-level evaluation (one report line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvalRecord {
    /// The evaluated construct.
    pub loc: Loc,
    /// Why it was evaluated.
    pub kind: EvalKind,
    /// KBC steps executed (memoized calls count their recorded steps).
    pub steps: u64,
    /// Abstract-memory bytes allocated (cumulative).
    pub bytes: u64,
    /// Maximum abstract-memory bytes live at once.
    pub peak_bytes: u64,
    /// Function calls (memoized hits count their recorded calls).
    pub calls: u64,
    /// Dynamic allocations (`new`, `std::allocator<T>::allocate`).
    pub allocs: u64,
    /// Maximum abstract-memory address space held (frame top plus heap
    /// top: live bytes plus fragmentation); at least `peak_bytes`.
    pub peak_arena: u64,
    /// Bytes this evaluation added to the `ConstStore` (newly interned
    /// content only).
    pub frozen_bytes: u64,
    /// Wall-clock microseconds (not deterministic).
    pub wall_us: u64,
    /// Whether the evaluation produced a value.
    pub ok: bool,
}

/// The counters summed over a translation unit's evaluations (`peak_bytes`
/// and `peak_arena` are the maximum).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvalTotals {
    /// Sum of `steps`.
    pub steps: u64,
    /// Sum of `bytes`.
    pub bytes: u64,
    /// Maximum of `peak_bytes`.
    pub peak_bytes: u64,
    /// Sum of `calls`.
    pub calls: u64,
    /// Sum of `allocs`.
    pub allocs: u64,
    /// Maximum of `peak_arena`.
    pub peak_arena: u64,
    /// Sum of `frozen_bytes`.
    pub frozen_bytes: u64,
    /// Sum of `wall_us`.
    pub wall_us: u64,
    /// Number of evaluations.
    pub evaluations: u64,
}

/// Every top-level evaluation of a translation unit, in finishing order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvalStats {
    records: Vec<EvalRecord>,
    totals: EvalTotals,
}

impl EvalStats {
    /// Records an evaluation.
    pub fn push(&mut self, r: EvalRecord) {
        let t = &mut self.totals;
        t.steps += r.steps;
        t.bytes += r.bytes;
        t.peak_bytes = t.peak_bytes.max(r.peak_bytes);
        t.calls += r.calls;
        t.allocs += r.allocs;
        t.peak_arena = t.peak_arena.max(r.peak_arena);
        t.frozen_bytes += r.frozen_bytes;
        t.wall_us += r.wall_us;
        t.evaluations += 1;
        self.records.push(r);
    }

    /// The evaluations.
    pub fn records(&self) -> &[EvalRecord] {
        &self.records
    }

    /// The totals.
    pub fn totals(&self) -> EvalTotals {
        self.totals
    }

    /// The most recent evaluation.
    pub fn last(&self) -> Option<&EvalRecord> {
        self.records.last()
    }
}
