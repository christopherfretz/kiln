//! `DeclContext` lookup tables (spec/ast.md §4.3).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// `DeclContext` lookup tables (spec/ast.md §4.3).
#[derive(Debug, Default)]
pub struct LookupTables {}

impl HeapSize for LookupTables {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
