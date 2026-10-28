//! Reflections (spec/ast.md §10, M6).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Reflections (spec/ast.md §10, M6).
#[derive(Debug, Default)]
pub struct ReflectionTable {}

impl HeapSize for ReflectionTable {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
