//! The snapshot encoding of [`Interner`] (spec/source.md "Snapshots",
//! spec/snap.md "Hand-written roots").
//!
//! | field | class | how |
//! |---|---|---|
//! | `arena` | state | the strings in symbol order |
//! | `map` | rebuilt | by interning the strings in order into a fresh interner |
//! | `INTERN_CACHE` (thread-local) | transient | hints verified on use; nothing to carry |

use std::sync::atomic::Ordering;

use kiln_snap::{Desc, Reader, Schema, SchemaHash, Snap, SnapError, Writer};

use super::{Arena, Interner};

/// The strings in symbol order. Decoding interns them in that order into a
/// fresh interner, so every string gets its old [`Symbol`](super::Symbol)
/// and a later `intern` of it returns that symbol. Restore the interner
/// before anything else interns (spec/source.md "Snapshots").
impl Snap for Interner {
    fn put(&self, w: &mut Writer) {
        let Interner { map: _, arena } = self;
        let Arena { len, chunks: _ } = arena;
        let n = len.load(Ordering::Acquire);
        w.length(n as usize);
        for i in 0..n {
            w.str(arena.get(i).unwrap_or_default());
        }
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let n = r.seq_len::<String>()?;
        if n >= u32::MAX as usize {
            return Err(r.invalid::<Self>("more strings than symbols"));
        }
        let interner = Interner::new();
        for k in 0..n {
            let s = r.str()?;
            if interner.intern(s).index() as usize != k {
                return Err(r.invalid::<Self>(format!("string {k} interned twice")));
            }
        }
        Ok(interner)
    }

    const SCHEMA_HASH: u64 =
        SchemaHash::new("root", "Interner").field::<Vec<String>>("strings").finish();
    fn schema(s: &mut Schema) {
        let d = Desc::new("root").hash(Self::SCHEMA_HASH).field::<Vec<String>>("strings");
        if s.register::<Self>(d) {
            Vec::<String>::schema(s);
        }
    }
}
