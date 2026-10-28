//! String literal bytes.
//!
//! (Skeleton from the WP-041 stub commit; filled in by WP-041.)

use crate::mem::{HeapSize, MemReport};

/// String literal bytes.
#[derive(Debug, Default)]
pub struct StringPool {}

impl HeapSize for StringPool {
    fn heap_bytes(&self) -> MemReport {
        MemReport::new()
    }
}
