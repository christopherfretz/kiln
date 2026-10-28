//! Memory accounting shared by every arena of the AST (spec/ast.md §2.3,
//! spec/types.md §9).

use std::collections::HashMap;
use std::fmt;
use std::mem::size_of;

/// One accounting row: an arena (or a component of one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemRow {
    /// Dotted name, e.g. `decls.headers`, `types.nodes`.
    pub name: String,
    /// Number of elements (nodes, entries, bytes for byte pools).
    pub count: u64,
    /// Bytes in use: `len * size_of::<T>()` (plus owned heap data).
    pub live_bytes: u64,
    /// Bytes actually held: `capacity * size_of::<T>()` (plus owned heap
    /// data); `>= live_bytes`.
    pub reserved_bytes: u64,
}

impl MemRow {
    /// A row with explicit numbers.
    pub fn new(name: impl Into<String>, count: u64, live_bytes: u64, reserved_bytes: u64) -> Self {
        Self { name: name.into(), count, live_bytes, reserved_bytes }
    }

    /// The row for a `Vec<T>` of plain data (no owned heap per element).
    pub fn of_vec<T>(name: impl Into<String>, v: &Vec<T>) -> Self {
        let sz = size_of::<T>() as u64;
        Self::new(name, v.len() as u64, v.len() as u64 * sz, v.capacity() as u64 * sz)
    }

    /// The row for a `HashMap<K, V, S>` of plain data, estimating the
    /// table as hashbrown lays it out: `(K, V)` slots plus one control byte
    /// per bucket, `capacity` rounded up to buckets at 7/8 load.
    pub fn of_map<K, V, S>(name: impl Into<String>, m: &HashMap<K, V, S>) -> Self {
        let slot = size_of::<(K, V)>() as u64 + 1;
        let cap = m.capacity() as u64;
        let buckets = if cap == 0 { 0 } else { (cap * 8 / 7).next_power_of_two() };
        Self::new(name, m.len() as u64, m.len() as u64 * slot, buckets * slot)
    }

    /// Prefixes the row's name with `prefix.`.
    #[must_use]
    pub fn prefixed(mut self, prefix: &str) -> Self {
        self.name = format!("{prefix}.{}", self.name);
        self
    }
}

/// A memory report: one row per arena.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemReport {
    /// The rows, in a stable order (arena declaration order).
    pub rows: Vec<MemRow>,
}

impl MemReport {
    /// An empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a row.
    pub fn push(&mut self, row: MemRow) {
        self.rows.push(row);
    }

    /// Appends every row of `other`, prefixing names with `prefix.`.
    pub fn extend_prefixed(&mut self, prefix: &str, other: MemReport) {
        self.rows.extend(other.rows.into_iter().map(|r| r.prefixed(prefix)));
    }

    /// The row named `name`, if any.
    pub fn row(&self, name: &str) -> Option<&MemRow> {
        self.rows.iter().find(|r| r.name == name)
    }

    /// Sum of `live_bytes` over rows that are not derived totals (rows
    /// whose name starts with `total` or `instantiation` are excluded, as
    /// the latter is a subset of the others).
    pub fn live_bytes(&self) -> u64 {
        self.rows.iter().filter(|r| !is_derived(&r.name)).map(|r| r.live_bytes).sum()
    }

    /// Sum of `reserved_bytes` over non-derived rows.
    pub fn reserved_bytes(&self) -> u64 {
        self.rows.iter().filter(|r| !is_derived(&r.name)).map(|r| r.reserved_bytes).sum()
    }
}

fn is_derived(name: &str) -> bool {
    name.starts_with("total") || name.starts_with("instantiation")
}

impl fmt::Display for MemReport {
    /// One row per line: `name count live reserved`, then a `total` line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for r in &self.rows {
            writeln!(
                f,
                "{:<32} {:>10} {:>12} {:>12}",
                r.name, r.count, r.live_bytes, r.reserved_bytes
            )?;
        }
        writeln!(
            f,
            "{:<32} {:>10} {:>12} {:>12}",
            "total",
            "",
            self.live_bytes(),
            self.reserved_bytes()
        )
    }
}

/// Implemented by every arena: its memory as report rows.
pub trait HeapSize {
    /// The arena's rows (names relative to the arena; the `Ast` prefixes
    /// them with the field name).
    fn heap_bytes(&self) -> MemReport;
}
