//! Attributes: the 16-byte [`AttrNode`] (spec/ast.md §7).
#![warn(missing_docs)]

use kiln_source::Loc;

use crate::ids::{AttrId, DeclId, ListId};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::slot::ColdMap;

/// An attribute kind. A newtype over `u16` so the list grows without
/// renumbering; unknown attributes are diagnosed and never stored.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AttrKind(pub u16);

#[allow(missing_docs, reason = "each constant is named after the attribute it denotes")]
impl AttrKind {
    // standard ([dcl.attr])
    pub const ALIGNED: AttrKind = AttrKind(1);
    pub const ASSUME: AttrKind = AttrKind(2);
    pub const CARRIES_DEPENDENCY: AttrKind = AttrKind(3);
    pub const DEPRECATED: AttrKind = AttrKind(4);
    pub const FALLTHROUGH: AttrKind = AttrKind(5);
    pub const INDETERMINATE: AttrKind = AttrKind(6);
    pub const LIKELY: AttrKind = AttrKind(7);
    pub const UNLIKELY: AttrKind = AttrKind(8);
    pub const MAYBE_UNUSED: AttrKind = AttrKind(9);
    pub const NODISCARD: AttrKind = AttrKind(10);
    pub const NORETURN: AttrKind = AttrKind(11);
    pub const NO_UNIQUE_ADDRESS: AttrKind = AttrKind(12);
    /// P3394 annotation: `a` = `ConstValueId` (M6).
    pub const ANNOTATION: AttrKind = AttrKind(13);
    // GNU / vendor
    pub const ALWAYS_INLINE: AttrKind = AttrKind(64);
    pub const NOINLINE: AttrKind = AttrKind(65);
    pub const PACKED: AttrKind = AttrKind(66);
    pub const USED: AttrKind = AttrKind(67);
    pub const UNUSED: AttrKind = AttrKind(68);
    pub const WEAK: AttrKind = AttrKind(69);
    pub const VISIBILITY: AttrKind = AttrKind(70);
    pub const SECTION: AttrKind = AttrKind(71);
    pub const FORMAT: AttrKind = AttrKind(72);
    pub const CONST: AttrKind = AttrKind(73);
    pub const PURE: AttrKind = AttrKind(74);
    pub const COLD: AttrKind = AttrKind(75);
    pub const NOTHROW: AttrKind = AttrKind(76);
    pub const ENABLE_IF: AttrKind = AttrKind(77);
    pub const DIAGNOSE_IF: AttrKind = AttrKind(78);
    pub const TRIVIAL_ABI: AttrKind = AttrKind(79);
    pub const PREFERRED_NAME: AttrKind = AttrKind(80);
    pub const EXCLUDE_FROM_EXPLICIT_INSTANTIATION: AttrKind = AttrKind(81);
    pub const ABI_TAG: AttrKind = AttrKind(82);
    pub const INTERNAL_LINKAGE: AttrKind = AttrKind(83);
    pub const LIFETIMEBOUND: AttrKind = AttrKind(84);
    pub const NO_SANITIZE: AttrKind = AttrKind(85);
    pub const MODE: AttrKind = AttrKind(86);
    pub const VECTOR_SIZE: AttrKind = AttrKind(87);
    /// GNU asm label (`asm("name")` after a declarator): `a` = the
    /// label's `StringLitId`. Keyword syntax.
    pub const ASM_LABEL: AttrKind = AttrKind(88);
    /// The implicit attribute of a lazily declared builtin function
    /// (clang's `BuiltinAttr`): `a` = its builtin id
    /// ([`crate::decl::builtin`]). Always implicit.
    pub const BUILTIN: AttrKind = AttrKind(89);
}

/// Attribute syntax.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
#[repr(u8)]
pub enum AttrSyntax {
    /// `[[attr]]`.
    #[default]
    Cxx11 = 0,
    /// `__attribute__((attr))`.
    Gnu = 1,
    /// A keyword (`alignas`, `_Noreturn`).
    Keyword = 2,
    /// `__declspec(attr)`.
    Declspec = 3,
}

/// The 16-byte attribute node (spec/ast.md §7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttrNode {
    /// The kind.
    pub kind: AttrKind,
    /// The syntax.
    pub syntax: AttrSyntax,
    /// Per-kind flags (bit 0: implicit; bit 1: inherited from a previous
    /// declaration).
    pub flags: u8,
    /// The attribute-token location.
    pub loc: Loc,
    /// Argument word `a` (an `ExprId`, `Symbol`, `ConstValueId` or side
    /// index, per kind).
    pub a: u32,
    /// Argument word `b`.
    pub b: u32,
}

/// Attribute nodes and the declaration → attributes map.
#[derive(Debug)]
pub struct AttrArena {
    nodes: Vec<AttrNode>,
    /// Attributes of declarations that have any (`HAS_ATTRS`).
    pub decl_attrs: ColdMap<DeclId, ListId>,
}

impl Default for AttrArena {
    fn default() -> Self {
        Self {
            nodes: vec![AttrNode {
                kind: AttrKind(0),
                syntax: AttrSyntax::Cxx11,
                flags: 0,
                loc: Loc::INVALID,
                a: 0,
                b: 0,
            }],
            decl_attrs: ColdMap::new("decl_attrs"),
        }
    }
}

impl AttrArena {
    /// An arena holding only the dummy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an attribute.
    pub fn push(&mut self, a: AttrNode) -> AttrId {
        let id = AttrId::new(self.nodes.len() as u32);
        self.nodes.push(a);
        id
    }

    /// The attribute `a`.
    pub fn get(&self, a: AttrId) -> &AttrNode {
        &self.nodes[a.index() as usize]
    }

    /// The number of attributes.
    pub fn len(&self) -> usize {
        self.nodes.len() - 1
    }

    /// True if there are no attributes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The attribute list of `d` ([`ListId::EMPTY`] if none).
    pub fn of_decl(&self, d: DeclId) -> ListId {
        self.decl_attrs.get(d).copied().unwrap_or(ListId::EMPTY)
    }

    pub(crate) fn live_bytes(&self) -> u64 {
        MemRow::of_vec("nodes", &self.nodes).live_bytes + self.decl_attrs.mem_row().live_bytes
    }
}

impl HeapSize for AttrArena {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("nodes", &self.nodes));
        r.push(self.decl_attrs.mem_row());
        r
    }
}
