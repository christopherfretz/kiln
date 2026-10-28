//! Declaration names (spec/ast.md §3).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Declaration names (spec/ast.md §3).
#[derive(Debug, Default)]
pub struct NameTable {}

impl HeapSize for NameTable {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
