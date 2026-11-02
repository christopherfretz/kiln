//! Module ownership of declarations (spec/modules.md §5): the units of an
//! invocation, their imports, and the owner of every declaration. Written
//! by sema, read by lookup, the mangler and codegen. A BMI (part B) would
//! serialise this table with the AST.

use crate::ids::DeclId;

/// The kind of a translation unit or imported unit ([module.unit]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnitKind {
    /// A translation unit with no module declaration.
    #[default]
    NonModule,
    /// A header unit ([module.import]/5).
    Header,
    /// `export module M;`.
    Interface,
    /// `module M;`.
    Implementation,
    /// `export module M:P;`.
    PartitionInterface,
    /// `module M:P;`.
    PartitionImplementation,
}

impl UnitKind {
    /// A unit of a named module.
    pub fn is_named(self) -> bool {
        !matches!(self, UnitKind::NonModule | UnitKind::Header)
    }

    /// A module interface unit ([module.unit]/2).
    pub fn is_interface(self) -> bool {
        matches!(self, UnitKind::Interface | UnitKind::PartitionInterface)
    }
}

/// One unit: the main file (unit 0) or an imported unit.
#[derive(Clone, Debug, Default)]
pub struct UnitInfo {
    /// The key the preprocessor gave the unit (module name, `M:P`, or `"`
    /// and a header path); empty for unit 0.
    pub key: String,
    /// Its kind, known after its module declaration.
    pub kind: UnitKind,
    /// The named module (`M.N`) of a named module unit.
    pub module: Option<String>,
    /// The partition (`P`) of a partition unit.
    pub partition: Option<String>,
    /// Imported unit keys, with `export import`.
    pub imports: Vec<(String, bool)>,
    /// Declaration id ranges `[lo, hi)` of other units this unit saw again
    /// in its own text (spec/modules.md §6): visible as if declared here.
    pub merged: Vec<(u32, u32)>,
}

/// The owner of a declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Owner {
    /// The unit (index into [`ModuleTable::units`]).
    pub unit: u16,
    /// In the unit's module purview ([module.unit]/5).
    pub purview: bool,
    /// Exported ([module.interface]).
    pub exported: bool,
}

/// Units and declaration ownership.
#[derive(Clone, Debug, Default)]
pub struct ModuleTable {
    /// The units; unit 0 is the main file.
    pub units: Vec<UnitInfo>,
    /// `(first declaration index, owner)`, sorted: the owner of every
    /// declaration from that index on.
    points: Vec<(u32, Owner)>,
    /// Modules are in use (a module or import directive was seen, or the
    /// main file may hold one): ownership is tracked.
    pub active: bool,
    /// `(new declaration, previous declaration)` redeclaration links made
    /// while active, for sema's [basic.link]/11 check (drained by it).
    pub links: Vec<(DeclId, DeclId)>,
}

impl ModuleTable {
    /// The owner of `d` (unit 0, not in a purview, before any change).
    pub fn owner(&self, d: DeclId) -> Owner {
        let i = d.index();
        match self.points.partition_point(|&(at, _)| at <= i) {
            0 => Owner::default(),
            n => self.points[n - 1].1,
        }
    }

    /// Declarations from index `from` on are owned by `o`.
    pub fn set_owner_from(&mut self, from: DeclId, o: Owner) {
        let from = from.index();
        if self.points.last().map_or(Owner::default(), |p| p.1) == o {
            return;
        }
        while self.points.last().is_some_and(|p| p.0 >= from) {
            self.points.pop();
        }
        if self.points.last().map_or(Owner::default(), |p| p.1) != o {
            self.points.push((from, o));
        }
    }

    /// The named module `d` is attached to by its owner (not considering
    /// linkage specifications): `Some` for a declaration in a purview.
    pub fn purview_module(&self, d: DeclId) -> Option<&str> {
        let o = self.owner(d);
        if !o.purview {
            return None;
        }
        self.units.get(usize::from(o.unit))?.module.as_deref()
    }

    /// True if `d` was declared in the purview of an imported named
    /// module unit (not the main file, not a header unit, not a global
    /// module fragment): its definition belongs to that unit's object.
    pub fn is_imported_module_decl(&self, d: DeclId) -> bool {
        let o = self.owner(d);
        o.purview
            && o.unit != 0
            && self.units.get(usize::from(o.unit)).is_some_and(|u| u.kind.is_named())
    }

    /// Heap bytes.
    pub fn heap_bytes(&self) -> usize {
        self.points.capacity() * size_of::<(u32, Owner)>()
            + self.units.iter().map(|u| u.merged.capacity() * 8).sum::<usize>()
    }
}

impl crate::Ast {
    /// The named module `d` is attached to ([module.unit]/7): its owner's
    /// module when declared in a purview, unless inside a linkage
    /// specification (`extern "C++"` attaches to the global module). The
    /// walk follows lexical contexts: a namespace first opened in the
    /// purview and reopened inside `extern "C++"` attaches what that
    /// reopening declares to the global module.
    pub fn module_attachment(&self, d: DeclId) -> Option<&str> {
        use crate::decl::DeclKind;
        let m = self.modules.purview_module(d)?;
        let mut c = self.decls.lexical_ctx(d);
        while let Some(x) = c {
            match self.decls.get(x).kind() {
                DeclKind::LinkageSpec => return None,
                DeclKind::Export | DeclKind::Namespace => c = self.decls.lexical_ctx(x),
                _ => break,
            }
        }
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_change_points() {
        let mut t = ModuleTable::default();
        let a = Owner { unit: 1, purview: true, exported: false };
        let b = Owner { unit: 1, purview: true, exported: true };
        t.set_owner_from(DeclId::new(10), a);
        t.set_owner_from(DeclId::new(20), b);
        t.set_owner_from(DeclId::new(30), a);
        assert_eq!(t.owner(DeclId::new(5)), Owner::default());
        assert_eq!(t.owner(DeclId::new(10)), a);
        assert_eq!(t.owner(DeclId::new(25)), b);
        assert_eq!(t.owner(DeclId::new(31)), a);
        // A change at the same index replaces the previous one.
        t.set_owner_from(DeclId::new(30), b);
        assert_eq!(t.owner(DeclId::new(31)), b);
    }
}
