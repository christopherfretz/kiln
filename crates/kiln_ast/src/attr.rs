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
    /// GNU `format(archetype, F, A)`: `a` = the archetype (0 `printf`),
    /// `b` = `F | A << 16`, the one-based format-string and first data
    /// argument indices (`A` = 0 for a `va_list`). Only the implicit
    /// attribute of the printf-family builtins is stored (WP-282).
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
    /// Clang `ext_vector_type(N)` on a typedef: `a` = the element-count
    /// `ExprId`. Like `VECTOR_SIZE`, it is consumed into the declared type
    /// (a `Vector` / `DependentVector`) and never attached to the decl.
    pub const EXT_VECTOR_TYPE: AttrKind = AttrKind(90);
    /// GNU `malloc` on a function returning a pointer (clang's
    /// `RestrictAttr`): the result is `noalias` (WP-170).
    pub const MALLOC: AttrKind = AttrKind(97);
    /// GNU `alloc_size(n[, m])` (clang `AllocSizeAttr`): `a` / `b` are
    /// `IntegerLiteral`s holding the one-based parameter indices n / m
    /// (`b` = 0 when absent) (WP-170).
    pub const ALLOC_SIZE: AttrKind = AttrKind(98);
    /// GNU `alloc_align(n)` (clang `AllocAlignAttr`): `a` is an
    /// `IntegerLiteral` holding the one-based parameter index (WP-170).
    pub const ALLOC_ALIGN: AttrKind = AttrKind(99);
    /// `clang::using_if_exists` on a using-declaration (WP-062).
    pub const USING_IF_EXISTS: AttrKind = AttrKind(100);
    /// GNU `hot` on a function (clang `HotAttr`): the declaration and its
    /// direct call sites are `hot` (WP-172).
    pub const HOT: AttrKind = AttrKind(101);
    /// `flag_enum` / `clang::flag_enum` on an enumeration (clang
    /// `FlagEnumAttr`): its values are bit flags, so a `case` label made
    /// of its single-bit enumerators' bits is in the enumeration (WP-230).
    pub const FLAG_ENUM: AttrKind = AttrKind(102);
    /// `enum_extensibility(open|closed)` on an enumeration (clang
    /// `EnumExtensibilityAttr`): `a` is 1 for `open` (values outside the
    /// enumerators are expected), 0 for `closed` (WP-230).
    pub const ENUM_EXTENSIBILITY: AttrKind = AttrKind(103);
    /// `clang::coro_only_destroy_when_complete` on a class (clang
    /// `CoroOnlyDestroyWhenCompleteAttr`): a coroutine returning it is
    /// only destroyed once suspended at its final suspend point, so its
    /// LLVM function carries `coro_only_destroy_when_complete` (WP-370).
    pub const CORO_ONLY_DESTROY_WHEN_COMPLETE: AttrKind = AttrKind(104);
    /// `optnone` / `clang::optnone` on a function (clang
    /// `OptimizeNoneAttr`): its definition is `optnone noinline` at every
    /// optimization level; it wins over `always_inline` and `minsize`
    /// (WP-405).
    pub const OPTNONE: AttrKind = AttrKind(105);
    /// `minsize` / `clang::minsize` on a function (clang `MinSizeAttr`):
    /// its definition is `minsize` (WP-405).
    pub const MINSIZE: AttrKind = AttrKind(106);
    /// GNU `naked` on a function (clang `NakedAttr`): no prologue or
    /// epilogue; the body holds only asm statements (WP-405).
    pub const NAKED: AttrKind = AttrKind(107);
    /// GNU `nonnull` on a function or a parameter (clang `NonNullAttr`):
    /// `a` is the `ListId` of `IntegerLiteral`s holding the one-based
    /// parameter indices it names (as written until the declaration
    /// checks them); the empty list names every parameter (WP-421).
    pub const NONNULL: AttrKind = AttrKind(108);
    /// GNU `returns_nonnull` on a function (clang `ReturnsNonNullAttr`)
    /// (WP-421).
    pub const RETURNS_NONNULL: AttrKind = AttrKind(109);
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

impl AttrNode {
    /// `[[assume]]` flag (bit 2): the assumption has possible side
    /// effects and is ignored (clang `-Wassume`): neither assumed by
    /// codegen nor checked by constant evaluation. For `ASSUME`, `a` is
    /// the assumption (`ExprId`) and `b` the raw `Loc` of its clang
    /// `getExprLoc`, where constant evaluation reports it false.
    pub const ASSUME_IGNORED: u8 = 4;
    /// `[[fallthrough]]` flag (bit 2): written with an attribute scope
    /// (`[[clang::fallthrough]]`, `[[gnu::fallthrough]]`), so not a C++17
    /// extension before C++17 (clang `handleFallThroughAttr`).
    pub const FALLTHROUGH_SCOPED: u8 = 4;
    /// Flag (bit 3, any kind): written with the `gnu::` attribute scope
    /// (`[[gnu::noreturn]]`). With `AttrSyntax::Gnu` it selects the GNU
    /// spelling of `NODISCARD` (`warn_unused_result`) and `NORETURN`, and
    /// after a function declarator it makes a misplaced attribute a
    /// warning rather than an error (clang `isGNUScope`).
    pub const GNU_SCOPED: u8 = 8;

    /// True for the GNU spelling: `__attribute__((x))` or `[[gnu::x]]`.
    pub const fn is_gnu_spelling(&self) -> bool {
        matches!(self.syntax, AttrSyntax::Gnu) || self.flags & Self::GNU_SCOPED != 0
    }
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

    /// Sets the flag bits `flags` on attribute `a` (while it is being
    /// built, before it is attached).
    pub fn set_flags(&mut self, a: AttrId, flags: u8) {
        self.nodes[a.index() as usize].flags |= flags;
    }

    /// Replaces the argument words of attribute `a` (while it is being
    /// built, before it is attached).
    pub fn set_args(&mut self, a: AttrId, x: u32, y: u32) {
        let n = &mut self.nodes[a.index() as usize];
        n.a = x;
        n.b = y;
    }

    /// The number of attributes.
    pub fn len(&self) -> usize {
        self.nodes.len() - 1
    }

    /// True if there are no attributes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether any attribute of kind `kind` exists (a linear scan).
    pub fn has_kind(&self, kind: AttrKind) -> bool {
        self.nodes[1..].iter().any(|a| a.kind == kind)
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
