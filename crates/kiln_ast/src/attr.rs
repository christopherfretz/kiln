//! Attributes (spec/ast.md §7).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Attributes (spec/ast.md §7).
#[derive(Debug, Default)]
pub struct AttrArena {}

impl HeapSize for AttrArena {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
