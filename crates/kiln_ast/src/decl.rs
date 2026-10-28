//! Declarations: headers, payloads and cold maps (spec/ast.md §4).
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// Declarations: headers, payloads and cold maps (spec/ast.md §4).
#[derive(Debug, Default)]
pub struct DeclArena {}

impl HeapSize for DeclArena {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
