//! Typed 4-byte ids for every AST arena (spec/ast.md §2.1, spec/types.md §2).
//!
//! Every id wraps a `NonZeroU32`, so `Option<Id>` is also 4 bytes. Index 0
//! of each arena is a dummy that is never handed out. One id type per
//! arena makes mixing arenas a type error. Ids are TU-local.
//!
//! This module also defines the ids of the type layer (`TypeId`,
//! `QualType`, `TemplateNameId`, `NnsId`, `ArgListId`, `ProfileId`) and of
//! the constant store (`ConstValueId`, `BlobId`) so that every work package
//! of the AST builds against one set of definitions; the operations on
//! them beyond raw encoding live with their owners (`types/`, `consts.rs`).

use std::fmt;
use std::num::NonZeroU32;

/// Conversion between an id and its raw `u32` index, used where ids are
/// packed into untyped `u32` slots (`IdLists`, the `a`/`b` operands of
/// expression and statement nodes).
pub trait RawId: Copy {
    /// The raw non-zero index.
    fn raw(self) -> u32;
    /// The id with raw index `raw`, or `None` for 0.
    fn from_raw(raw: u32) -> Option<Self>;
    /// Encodes `Option<Self>` as a `u32` (0 for `None`).
    fn raw_opt(id: Option<Self>) -> u32 {
        id.map_or(0, RawId::raw)
    }
}

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident, $max:expr, $what:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(NonZeroU32);

        impl $name {
            /// The largest index this id space can represent.
            pub const MAX_INDEX: u32 = $max;

            /// The id with index `index`.
            ///
            /// # Panics
            /// If `index` is 0 (the reserved dummy) or above
            /// [`Self::MAX_INDEX`]: arena exhaustion (spec/ast.md §2.1).
            #[inline]
            #[track_caller]
            pub const fn new(index: u32) -> Self {
                assert!(index <= Self::MAX_INDEX, concat!($what, " id space exhausted"));
                match NonZeroU32::new(index) {
                    Some(n) => Self(n),
                    None => panic!(concat!($what, " index 0 is the reserved dummy")),
                }
            }

            /// The id with index `index`, or `None` for 0 or an index
            /// above [`Self::MAX_INDEX`].
            #[inline]
            pub const fn try_new(index: u32) -> Option<Self> {
                if index > Self::MAX_INDEX {
                    return None;
                }
                match NonZeroU32::new(index) {
                    Some(n) => Some(Self(n)),
                    None => None,
                }
            }

            /// The index into the arena (never 0).
            #[inline]
            pub const fn index(self) -> u32 {
                self.0.get()
            }
        }

        impl RawId for $name {
            #[inline]
            fn raw(self) -> u32 {
                self.0.get()
            }
            #[inline]
            fn from_raw(raw: u32) -> Option<Self> {
                Self::try_new(raw)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

define_id!(
    /// A declaration (spec/ast.md §4). Capped at 2^31 − 1 because a
    /// lookup entry uses the top bit to tell a single decl from an
    /// overload-set index (§4.3).
    DeclId, (1 << 31) - 1, "DeclId"
);
define_id!(
    /// An expression (spec/ast.md §5).
    ExprId, u32::MAX, "ExprId"
);
define_id!(
    /// A statement (spec/ast.md §6).
    StmtId, u32::MAX, "StmtId"
);
define_id!(
    /// An attribute (spec/ast.md §7).
    AttrId, u32::MAX, "AttrId"
);
define_id!(
    /// An immutable list of `u32` ids in `Ast::lists` (spec/ast.md §2.2).
    /// [`ListId::EMPTY`] is the empty list.
    ListId, u32::MAX, "ListId"
);
define_id!(
    /// An interned constant value in the `ConstStore` (spec/ast.md §9).
    ConstValueId, u32::MAX, "ConstValueId"
);
define_id!(
    /// A byte image in the `ConstStore`'s blob pool (spec/ast.md §9.1,
    /// `Aggregate` values).
    BlobId, u32::MAX, "BlobId"
);
define_id!(
    /// An interned reflection (`std::meta::info`, spec/ast.md §10, M6).
    ReflectionId, u32::MAX, "ReflectionId"
);
define_id!(
    /// The bytes of a string literal in the `StringPool`.
    StringLitId, u32::MAX, "StringLitId"
);
define_id!(
    /// A redeclaration group (spec/ast.md §4.4).
    RedeclGroupId, u32::MAX, "RedeclGroupId"
);
define_id!(
    /// A lambda's side record (closure data, M3).
    LambdaId, u32::MAX, "LambdaId"
);
define_id!(
    /// A type node in the `TypeContext` (spec/types.md §2). 29 bits, so a
    /// [`QualType`] can carry three qualifier bits.
    TypeId, (1 << 29) - 1, "TypeId"
);
define_id!(
    /// An interned template name (spec/types.md §8.1).
    TemplateNameId, u32::MAX, "TemplateNameId"
);
define_id!(
    /// An interned nested-name-specifier (spec/types.md §8.2).
    NnsId, u32::MAX, "NnsId"
);
define_id!(
    /// An interned template argument list (spec/types.md §8.3).
    ArgListId, u32::MAX, "ArgListId"
);
define_id!(
    /// An interned canonical expression profile (spec/types.md §7.2).
    ProfileId, u32::MAX, "ProfileId"
);

define_id!(
    /// A template parameter list in `TemplateTables` (spec/sema-templates.md
    /// §2.1, §11.2).
    TemplateParamListId, u32::MAX, "TemplateParamListId"
);

impl ListId {
    /// The empty list (index 1; index 0 is the dummy). Pushing an empty
    /// slice returns it without allocating.
    pub const EMPTY: ListId = ListId::new(1);
}

/// A type reference: a [`TypeId`] plus the three "fast" qualifier bits
/// (spec/types.md §2). 4 bytes; `Option<QualType>` is 4 bytes.
///
/// Encoding: bits 0..=2 are the qualifiers (`CONST = 1`, `VOLATILE = 2`,
/// `RESTRICT = 4`), bits 3..=31 the `TypeId` index. This module provides
/// only the bit encoding; adding qualifiers is `TypeContext::qualify`
/// (because of the array rule, types.md §2.1), and the `Quals` flag type is
/// the type layer's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualType(NonZeroU32);

impl QualType {
    /// The qualifier bits' mask.
    pub const QUALS_MASK: u32 = 0b111;

    /// `ty` with qualifier bits `quals` (only bits 0..=2 are kept).
    #[inline]
    pub const fn new(ty: TypeId, quals: u8) -> Self {
        let raw = (ty.index() << 3) | (quals as u32 & Self::QUALS_MASK);
        match NonZeroU32::new(raw) {
            Some(n) => Self(n),
            None => unreachable!(),
        }
    }

    /// `ty` without qualifiers.
    #[inline]
    pub const fn unqual(ty: TypeId) -> Self {
        Self::new(ty, 0)
    }

    /// The type node.
    #[inline]
    pub const fn ty(self) -> TypeId {
        TypeId::new(self.0.get() >> 3)
    }

    /// The qualifier bits (`CONST = 1`, `VOLATILE = 2`, `RESTRICT = 4`).
    #[inline]
    pub const fn quals_bits(self) -> u8 {
        (self.0.get() & Self::QUALS_MASK) as u8
    }

    /// True if the `const` bit is set on this reference (not looking
    /// through sugar or array elements; see `TypeContext::effective_quals`).
    #[inline]
    pub const fn has_const_bit(self) -> bool {
        self.0.get() & 1 != 0
    }

    /// The same type without qualifier bits.
    #[inline]
    #[must_use]
    pub const fn without_quals(self) -> Self {
        Self::new(self.ty(), 0)
    }

    /// The raw encoding (never 0).
    #[inline]
    pub const fn to_raw(self) -> u32 {
        self.0.get()
    }

    /// The `QualType` with raw encoding `raw`, or `None` if `raw` has a
    /// zero type index.
    #[inline]
    pub const fn from_raw(raw: u32) -> Option<Self> {
        if raw >> 3 == 0 {
            return None;
        }
        match NonZeroU32::new(raw) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }
}

impl RawId for QualType {
    #[inline]
    fn raw(self) -> u32 {
        self.to_raw()
    }
    #[inline]
    fn from_raw(raw: u32) -> Option<Self> {
        QualType::from_raw(raw)
    }
}

impl fmt::Debug for QualType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QualType({}", self.ty().index())?;
        let q = self.quals_bits();
        for (bit, name) in [(1, " const"), (2, " volatile"), (4, " restrict")] {
            if q & bit != 0 {
                f.write_str(name)?;
            }
        }
        f.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::{DeclId, ListId, QualType, RawId, TypeId};

    #[test]
    fn id_roundtrip_and_limits() {
        assert_eq!(DeclId::new(5).index(), 5);
        assert_eq!(DeclId::try_new(0), None);
        assert_eq!(DeclId::try_new(1 << 31), None);
        assert_eq!(<DeclId as RawId>::from_raw(7), Some(DeclId::new(7)));
        assert_eq!(<DeclId as RawId>::raw_opt(None), 0);
        assert_eq!(TypeId::try_new(1 << 29), None);
        assert_eq!(ListId::EMPTY.index(), 1);
    }

    #[test]
    fn qualtype_bits() {
        let q = QualType::new(TypeId::new(12), 0b101);
        assert_eq!(q.ty(), TypeId::new(12));
        assert_eq!(q.quals_bits(), 0b101);
        assert!(q.has_const_bit());
        assert_eq!(q.without_quals(), QualType::unqual(TypeId::new(12)));
        assert_eq!(QualType::from_raw(q.to_raw()), Some(q));
        assert_eq!(QualType::from_raw(0b111), None);
        assert_eq!(format!("{q:?}"), "QualType(12 const restrict)");
    }

    #[test]
    #[should_panic(expected = "reserved dummy")]
    fn zero_index_panics() {
        let _ = ExprIdZero::make();
    }
    struct ExprIdZero;
    impl ExprIdZero {
        fn make() -> super::ExprId {
            super::ExprId::new(std::hint::black_box(0))
        }
    }
}
