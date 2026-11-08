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
    /// Static-space bytes held when the evaluation ended (live static
    /// objects plus the free ranges between them; the space persists
    /// across evaluations, so this is the translation unit's so far).
    pub static_bytes: u64,
    /// Calls answered from the memo table (spec/oven.md "Memoization").
    pub memo_hits: u64,
    /// Bytes this evaluation added to the reflection table and the
    /// interned content reflections designate (spec/oven.md "Reflection
    /// bytes"); part of `peak_bytes`.
    pub refl_bytes: u64,
    /// Wall-clock microseconds (not deterministic).
    pub wall_us: u64,
    /// Whether the evaluation produced a value.
    pub ok: bool,
}

/// The counters summed over a translation unit's evaluations (`peak_bytes`
/// and `peak_arena` are the maximum; `static_bytes` is a high-water mark).
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
    /// The most static space held at any point of the translation unit,
    /// including work no record covers (`Oven::has_constant_destruction`,
    /// `try_fold`); at least every record's `static_bytes`.
    pub static_bytes: u64,
    /// Sum of `memo_hits`.
    pub memo_hits: u64,
    /// Sum of `refl_bytes`.
    pub refl_bytes: u64,
    /// Sum of `wall_us`.
    pub wall_us: u64,
    /// Number of evaluations.
    pub evaluations: u64,
    /// Splices whose operand sema read as a frozen reflection without an
    /// evaluation (the splice fast path, spec/reflection.md §5): work the
    /// Oven was spared, so no record carries it.
    pub splice_fast: u64,
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
        t.static_bytes = t.static_bytes.max(r.static_bytes);
        t.memo_hits += r.memo_hits;
        t.refl_bytes += r.refl_bytes;
        t.wall_us += r.wall_us;
        t.evaluations += 1;
        self.records.push(r);
    }

    /// Raises the `static_bytes` total to the static space's high-water
    /// mark `n`.
    pub fn note_static_peak(&mut self, n: u64) {
        self.totals.static_bytes = self.totals.static_bytes.max(n);
    }

    /// Adds `n` splices read through the fast path (no evaluation).
    pub fn note_splice_fast(&mut self, n: u64) {
        self.totals.splice_fast = self.totals.splice_fast.saturating_add(n);
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
