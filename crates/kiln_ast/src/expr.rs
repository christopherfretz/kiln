//! Expressions (spec/ast.md §5).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Expressions (spec/ast.md §5).
#[derive(Debug, Default)]
pub struct ExprArena {}

impl HeapSize for ExprArena {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
