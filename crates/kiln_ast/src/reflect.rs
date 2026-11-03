//! Reflections (spec/reflection.md §2).
//!
//! WP-267 spike (`-freflection`): the table interns the entity a
//! `std::meta::info` value designates. A [`ReflectionId`] is a dense index
//! into it, so a value the Oven hands back (a scalar of shadow kind
//! `Reflection`, frozen as `Scalar::Reflection`) is looked up again with
//! [`ReflectionTable::get`]. Ids are 1-based like every arena id; the
//! Oven's 8-byte value is `index - 1` ([`ReflectionId::bits`]), so id 1 is
//! the null reflection and an all-zero (value-initialized)
//! `std::meta::info` is null.
//!
//! Mapping (spec/reflection.md §2.2; the spike subset):
//! - a type-id reflects its `QualType` with alias sugar dropped (the
//!   spike canonicalizes every type; P2996 keeps `^^Alias` distinct);
//! - a variable, function, data member or namespace reflects its
//!   canonical `DeclId`.

use crate::ids::{DeclId, QualType, ReflectionId};
use crate::mem::{HeapSize, MemReport, MemRow};
use kiln_source::fx::FxHashMap;

/// The entity a reflection designates (the spike subset of
/// spec/reflection.md §2.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reflectee {
    /// The null reflection.
    Null,
    /// A type (canonical).
    Type(QualType),
    /// A declaration: variable, function, field, namespace or class
    /// (canonical declaration).
    Decl(DeclId),
}

/// Interned reflections (spec/reflection.md §2.2).
#[derive(Debug)]
pub struct ReflectionTable {
    items: Vec<Reflectee>,
    map: FxHashMap<Reflectee, ReflectionId>,
    /// The most reflections (null included) the table holds.
    limit: usize,
}

/// [`ReflectionTable::intern`] of a new reflection into a full table: the
/// caller diagnoses it (`sema_too_many_reflections`), never a panic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReflectionTableFull {
    /// The table's capacity (null included).
    pub limit: usize,
}

impl Default for ReflectionTable {
    fn default() -> Self {
        // Every id fits a `u32`, and its Oven value `index - 1` too.
        Self::with_limit(u32::MAX as usize)
    }
}

impl ReflectionTable {
    /// An empty table (just the null reflection) that holds at most
    /// `limit` reflections, null included (at least 1, at most
    /// `u32::MAX`, the id space).
    pub fn with_limit(limit: usize) -> Self {
        let mut map = FxHashMap::default();
        map.insert(Reflectee::Null, ReflectionId::NULL);
        let limit = limit.clamp(1, u32::MAX as usize);
        ReflectionTable { items: vec![Reflectee::Null], map, limit }
    }

    /// The id of `r`, interning it; an error when `r` is new and the
    /// table already holds its limit (a reflection interned before still
    /// answers).
    pub fn intern(&mut self, r: Reflectee) -> Result<ReflectionId, ReflectionTableFull> {
        if let Some(&id) = self.map.get(&r) {
            return Ok(id);
        }
        let full = ReflectionTableFull { limit: self.limit };
        if self.items.len() >= self.limit {
            return Err(full);
        }
        let id =
            u32::try_from(self.items.len() + 1).ok().and_then(ReflectionId::try_new).ok_or(full)?;
        self.items.push(r);
        self.map.insert(r, id);
        Ok(id)
    }

    /// The entity `id` designates (`Null` for an id this table did not
    /// hand out, e.g. a raw value forged by a bit cast).
    pub fn get(&self, id: ReflectionId) -> Reflectee {
        self.items.get(id.index() as usize - 1).copied().unwrap_or(Reflectee::Null)
    }

    /// The most reflections (null included) the table holds.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// The number of interned reflections (including null).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Always false: the null reflection is always present.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl HeapSize for ReflectionTable {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("items", &self.items));
        r.push(MemRow::of_map("map", &self.map));
        r
    }
}

impl ReflectionId {
    /// The null reflection (a value-initialized `std::meta::info`).
    pub const NULL: ReflectionId = ReflectionId::new(1);

    /// The Oven's 8-byte value of this reflection (`index - 1`, so null
    /// is all zeros).
    pub fn bits(self) -> u64 {
        u64::from(self.index() - 1)
    }

    /// The reflection whose Oven value is `v` (`None` past the id space:
    /// a value no table handed out).
    pub fn from_bits(v: u128) -> Option<ReflectionId> {
        u32::try_from(v).ok().and_then(|v| v.checked_add(1)).and_then(ReflectionId::try_new)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kiln_source::Loc;
    use kiln_source::symbol::Interner;

    use super::*;
    use crate::types::{BuiltinType, TypeContext};

    /// A full table refuses a new reflection with an error (diagnosed by
    /// sema), and still answers for every reflection it holds.
    #[test]
    fn full_table_refuses_new_reflections() {
        let mut t = ReflectionTable::with_limit(3);
        let a = Reflectee::Decl(DeclId::new(7));
        let b = Reflectee::Type(BuiltinType::Int.qual_type());
        let c = Reflectee::Decl(DeclId::new(9));
        assert_eq!(t.intern(Reflectee::Null), Ok(ReflectionId::NULL));
        let ia = t.intern(a).expect("room for a");
        let ib = t.intern(b).expect("room for b");
        assert_eq!(t.len(), 3);
        assert_eq!(t.intern(c), Err(ReflectionTableFull { limit: 3 }));
        assert_eq!(t.len(), 3, "nothing interned");
        assert_eq!(t.intern(a), Ok(ia), "an interned reflection still answers");
        assert_eq!(t.intern(Reflectee::Null), Ok(ReflectionId::NULL));
        assert_eq!((t.get(ia), t.get(ib)), (a, b));
        let mut one = ReflectionTable::with_limit(0);
        assert_eq!(one.intern(a), Err(ReflectionTableFull { limit: 1 }));
    }

    /// `^^x` past the limit: the builder records no value (`b` = 0) on a
    /// non-dependent expression, which sema diagnoses
    /// (`Sema::build_reflect_expr`).
    #[test]
    fn reflect_expr_past_the_limit_has_no_value() {
        let mut ast = crate::Ast::new(Arc::new(Interner::new()), TypeContext::default());
        ast.reflections = ReflectionTable::with_limit(2);
        let info = BuiltinType::MetaInfo.qual_type();
        let int = Reflectee::Type(BuiltinType::Int.qual_type());
        let tu = Reflectee::Decl(ast.tu);
        let ok = ast.builder().reflect_expr(int, info, Loc::INVALID, Loc::INVALID);
        let full = ast.builder().reflect_expr(tu, info, Loc::INVALID, Loc::INVALID);
        assert!(ast.exprs.reflect_value(ok).is_some());
        assert_eq!(ast.exprs.reflect_value(full), None);
        assert!(!ast.exprs.get(full).is_value_dependent());
        assert_eq!(ast.reflections.len(), 2);
    }
}
