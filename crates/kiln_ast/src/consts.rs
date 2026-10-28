//! PLACEHOLDER from the WP-041 stub commit: owned by WP-043 (const store),
//! which replaces this file wholesale (spec/ast.md §9). The `Ast` needs
//! only `ConstStore: Default + HeapSize`; the ids it hands out
//! (`ConstValueId`, `BlobId`) are defined in `ids.rs`.

use crate::mem::{HeapSize, MemReport};

/// Placeholder for the interned constant-value store (spec/ast.md §9.1).
#[derive(Debug, Default)]
pub struct ConstStore {}

impl HeapSize for ConstStore {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
