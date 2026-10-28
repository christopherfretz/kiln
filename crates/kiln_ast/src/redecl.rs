//! Redeclaration groups (spec/ast.md §4.4).
#![warn(missing_docs)]

use crate::ids::{DeclId, RawId, RedeclGroupId};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::slot::AlreadySet;

/// One entity's declarations: shared by all of them (12 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RedeclGroup {
    /// The first declaration (types and template keys refer to it).
    pub first: DeclId,
    /// The latest declaration (what lookup returns).
    pub latest: DeclId,
    /// The definition, once seen (what codegen emits).
    pub definition: Option<DeclId>,
}

/// All redeclaration groups. Index 0 is a dummy.
#[derive(Debug)]
pub struct RedeclTable {
    groups: Vec<RedeclGroup>,
}

impl Default for RedeclTable {
    fn default() -> Self {
        let d = DeclId::new(DeclId::MAX_INDEX);
        Self { groups: vec![RedeclGroup { first: d, latest: d, definition: None }] }
    }
}

impl RedeclTable {
    /// A table holding only the dummy.
    pub fn new() -> Self {
        Self::default()
    }

    /// A new group whose first (and latest) declaration is `first`.
    pub fn new_group(&mut self, first: DeclId) -> RedeclGroupId {
        let id = RedeclGroupId::new(self.groups.len() as u32);
        self.groups.push(RedeclGroup { first, latest: first, definition: None });
        id
    }

    /// The group `g`.
    pub fn get(&self, g: RedeclGroupId) -> &RedeclGroup {
        &self.groups[g.index() as usize]
    }

    /// Records `d` as the latest declaration of `g`.
    pub fn push_redecl(&mut self, g: RedeclGroupId, d: DeclId) {
        self.groups[g.index() as usize].latest = d;
    }

    /// Records `d` as the definition of `g` (once; a second definition is
    /// an ODR violation sema diagnoses, [basic.def.odr]/1).
    pub fn set_definition(&mut self, g: RedeclGroupId, d: DeclId) -> Result<(), AlreadySet> {
        let grp = &mut self.groups[g.index() as usize];
        if let Some(old) = grp.definition {
            return Err(AlreadySet { slot: "RedeclGroup.definition", existing: old.raw() });
        }
        grp.definition = Some(d);
        Ok(())
    }

    /// The number of groups.
    pub fn len(&self) -> usize {
        self.groups.len() - 1
    }

    /// True if there are no groups.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        MemRow::of_vec("groups", &self.groups).live_bytes
    }
}

impl HeapSize for RedeclTable {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("groups", &self.groups));
        r
    }
}
