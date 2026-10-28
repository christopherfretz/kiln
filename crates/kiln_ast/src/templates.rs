//! Template tables (spec/ast.md §8, M3).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Template tables (spec/ast.md §8, M3).
#[derive(Debug, Default)]
pub struct TemplateTables {}

impl HeapSize for TemplateTables {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
