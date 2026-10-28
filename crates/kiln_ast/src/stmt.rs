//! Statements (spec/ast.md §6).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Statements (spec/ast.md §6).
#[derive(Debug, Default)]
pub struct StmtArena {}

impl HeapSize for StmtArena {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
