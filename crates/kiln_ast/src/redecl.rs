//! Redeclaration chains (spec/ast.md §4.4).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Redeclaration chains (spec/ast.md §4.4).
#[derive(Debug, Default)]
pub struct RedeclTable {}

impl HeapSize for RedeclTable {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
