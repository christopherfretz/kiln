//! The snapshot encoding of the [`ConstStore`] (spec/ast.md "Snapshots",
//! spec/snap.md "Hand-written roots").
//!
//! | field | class | how |
//! |---|---|---|
//! | `pools` | state | as data (value headers, words, paths, aggregate records with their stored hashes, image bytes, relocations, padding, union tables) |
//! | `table` | rebuilt | every value is interned: re-inserted in id order with the hash [`Pools::hash_of`] gives (an aggregate's stored hash, a scalar's recomputed one), so a later intern of an existing value returns the old id |
//!
//! Validation on read: every value decodes without a panic (its kind
//! known, its pool indices and aggregate ranges in range, its integer
//! normalized, its float kind, pointer base, member and reflection ids
//! valid) and passes the checks interning applies
//! ([`Pools::check_scalar`], [`Pools::check_image`]).

use hashbrown::HashTable;
use kiln_snap::{Desc, Reader, Schema, Snap, SnapError, Writer};

use super::{
    ConstStore, ConstValue, F_AUX, F_DESIG, F_WIDE, FloatKind, IntBits, Pools, PtrBase, kind,
};
use crate::ids::{ConstValueId, DeclId, ReflectionId};

impl Pools {
    /// Whether value `id` (in range) decodes without a panic.
    fn decodable(&self, id: ConstValueId) -> bool {
        let n = self.node(id);
        let words = |start: u32, len: usize| {
            (start as usize).checked_add(len).is_some_and(|e| e <= self.words.len())
        };
        match n.kind() {
            kind::INT => {
                let width = n.extra();
                if (1..=64).contains(&width) {
                    true
                } else {
                    let len = IntBits::words_for(width);
                    words(n.a, len)
                        && IntBits::from_words(width, &self.words[n.a as usize..n.a as usize + len])
                            .is_some()
                }
            }
            kind::BOOL | kind::NULL => true,
            kind::FLOAT => match u8::try_from(n.extra()).ok().and_then(FloatKind::from_u8) {
                Some(k) => k.bits() <= 64 || words(n.a, 2),
                None => false,
            },
            kind::PTR => {
                let f = n.flags();
                let wide = f & (F_AUX | F_WIDE | F_DESIG) != 0;
                let need = 1 + usize::from(f & F_AUX != 0) + usize::from(f & F_DESIG != 0);
                if wide && !words(n.b, need) {
                    return false;
                }
                if f & F_AUX != 0 {
                    let aux = self.words[n.b as usize + 1] as u32;
                    PtrBase::from_parts_aux(n.extra(), n.a, aux).is_some()
                } else {
                    PtrBase::from_parts(n.extra(), n.a).is_some()
                }
            }
            kind::MEMPTR => {
                DeclId::try_new(n.a).is_some()
                    && (n.b as usize)
                        .checked_add(n.extra() as usize)
                        .is_some_and(|e| e <= self.paths.len())
            }
            kind::REFL => ReflectionId::try_new(n.a).is_some(),
            kind::AGG => (n.a as usize) + 1 < self.aggs.len(),
            _ => false,
        }
    }

    /// The checks of decoded pools (see the module docs).
    fn check_restored(&self) -> Result<(), &'static str> {
        if self.nodes.len() > ConstValueId::MAX_INDEX as usize {
            return Err("too many constant values");
        }
        let Some(last) = self.aggs.last() else {
            return Err("aggregate table without its sentinel");
        };
        let ordered = self.aggs.windows(2).all(|p| {
            p[0].bytes <= p[1].bytes
                && p[0].relocs <= p[1].relocs
                && p[0].padding <= p[1].padding
                && p[0].unions <= p[1].unions
        });
        if !ordered
            || last.bytes as usize > self.bytes.len()
            || last.relocs as usize > self.relocs.len()
            || last.padding as usize > self.padding.len()
            || last.unions as usize > self.unions.len()
        {
            return Err("aggregate ranges out of order or range");
        }
        for i in 1..=self.nodes.len() as u32 {
            let id = ConstValueId::new(i);
            if !self.decodable(id) {
                return Err("constant value out of range");
            }
            let ok = match self.get(id) {
                ConstValue::Scalar(s) => self.check_scalar(&s).is_ok(),
                ConstValue::Aggregate(a) => self.check_image(&a).is_ok(),
            };
            if !ok {
                return Err("malformed constant value");
            }
        }
        Ok(())
    }
}

/// The constant store as a snapshot root; see the module docs.
impl Snap for ConstStore {
    fn put(&self, w: &mut Writer) {
        let ConstStore { pools, table: _ } = self;
        pools.put(w);
    }

    fn get(r: &mut Reader<'_>) -> Result<Self, SnapError> {
        let pools: Pools = r.get().map_err(|e| e.within("ConstStore.pools"))?;
        if let Err(why) = pools.check_restored() {
            return Err(r.invalid::<Self>(why));
        }
        let table = crate::types::intern::timed_rebuild(|| {
            let mut table = HashTable::with_capacity(pools.nodes.len());
            for i in 1..=pools.nodes.len() as u32 {
                let id = ConstValueId::new(i);
                table.insert_unique(pools.hash_of(id), id, |&id| pools.hash_of(id));
            }
            table
        });
        Ok(ConstStore { pools, table })
    }

    fn schema(s: &mut Schema) {
        let d = Desc::new("root").field::<Pools>("pools").skip::<HashTable<ConstValueId>>("table");
        if s.register::<Self>(d) {
            Pools::schema(s);
        }
    }
}
