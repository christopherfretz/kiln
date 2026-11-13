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

/// The snapshot id spaces of the AST's ids (`kiln_snap::Reader::note_id`,
/// spec/ast.md "Snapshots"): the decoder records the largest index of each
/// id type it reads, and the restored AST checks it against the arena the
/// id indexes (`Ast::check_ids`).
pub mod space {
    pub const DECL: usize = 0;
    pub const EXPR: usize = 1;
    pub const STMT: usize = 2;
    pub const ATTR: usize = 3;
    pub const LIST: usize = 4;
    pub const CONST_VALUE: usize = 5;
    pub const BLOB: usize = 6;
    pub const REFLECTION: usize = 7;
    pub const DMS: usize = 8;
    pub const ANN: usize = 9;
    pub const STRING_LIT: usize = 10;
    pub const REDECL_GROUP: usize = 11;
    pub const LAMBDA: usize = 12;
    /// `TypeId`, and the type index of a `QualType`.
    pub const TYPE: usize = 13;
    pub const TEMPLATE_NAME: usize = 14;
    pub const NNS: usize = 15;
    pub const ARG_LIST: usize = 16;
    pub const PROFILE: usize = 17;
    pub const TEMPLATE_PARAM_LIST: usize = 18;
    /// The number of spaces.
    pub const COUNT: usize = 19;
}

/// Reads the raw index of an id of `space` whose largest index is `max`:
/// above `max` is a decoding error, and every index but `max` itself (the
/// filler of the arenas' dummy entries, never a real id) is recorded for
/// the range check.
#[inline]
fn get_index(
    r: &mut kiln_snap::Reader<'_>,
    space: usize,
    max: u32,
    what: &'static str,
) -> Result<u32, kiln_snap::SnapError> {
    let at = r.offset();
    let v = <u32 as kiln_snap::Snap>::get(r)?;
    if v > max {
        return Err(kiln_snap::SnapError::new(
            kiln_snap::ErrorKind::BadValue("id above its id space"),
            what,
            at,
        ));
    }
    if v != max {
        r.note_id(space, v);
    }
    Ok(v)
}

/// A zero index where an id (not an `Option`) is encoded.
#[cold]
fn zero_id(r: &kiln_snap::Reader<'_>, what: &'static str) -> kiln_snap::SnapError {
    kiln_snap::SnapError::new(kiln_snap::ErrorKind::BadValue("zero NonZero"), what, r.offset() - 4)
}

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident, $max:expr, $what:literal, $space:expr) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(NonZeroU32);

        /// As the `u32` index (`Option<Self>` too, 0 for `None`); decoding
        /// rejects an index above [`Self::MAX_INDEX`] and records the index
        /// for the restored root's range check (`space`).
        impl kiln_snap::Snap for $name {
            // The descriptor's one item (the id space is a check on decode,
            // not part of the encoding).
            const SCHEMA_HASH: u64 = kiln_snap::SchemaHash::new("id", $what).finish();
            const SIZE: Option<usize> = Some(4);
            const OPTION_SIZE: Option<usize> = Some(4);
            #[inline]
            fn put(&self, w: &mut kiln_snap::Writer) {
                kiln_snap::Snap::put(&self.0.get(), w);
            }
            #[inline]
            fn get(r: &mut kiln_snap::Reader<'_>) -> Result<Self, kiln_snap::SnapError> {
                let v = get_index(r, $space, $max, $what)?;
                NonZeroU32::new(v).map(Self).ok_or_else(|| zero_id(r, $what))
            }
            fn schema(s: &mut kiln_snap::Schema) {
                s.register::<Self>(kiln_snap::Desc::new("id").hash(Self::SCHEMA_HASH));
            }
            #[inline]
            fn put_option(v: Option<&Self>, w: &mut kiln_snap::Writer) {
                kiln_snap::Snap::put(&v.map_or(0, |x| x.0.get()), w);
            }
            #[inline]
            fn get_option(
                r: &mut kiln_snap::Reader<'_>,
            ) -> Result<Option<Self>, kiln_snap::SnapError> {
                Ok(NonZeroU32::new(get_index(r, $space, $max, $what)?).map(Self))
            }
        }

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
    DeclId, (1 << 31) - 1, "DeclId", space::DECL
);
define_id!(
    /// An expression (spec/ast.md §5).
    ExprId, u32::MAX, "ExprId", space::EXPR
);
define_id!(
    /// A statement (spec/ast.md §6).
    StmtId, u32::MAX, "StmtId", space::STMT
);
define_id!(
    /// An attribute (spec/ast.md §7).
    AttrId, u32::MAX, "AttrId", space::ATTR
);
define_id!(
    /// An immutable list of `u32` ids in `Ast::lists` (spec/ast.md §2.2).
    /// [`ListId::EMPTY`] is the empty list.
    ListId, u32::MAX, "ListId", space::LIST
);
define_id!(
    /// An interned constant value in the `ConstStore` (spec/ast.md §9).
    ConstValueId, u32::MAX, "ConstValueId", space::CONST_VALUE
);
define_id!(
    /// A byte image in the `ConstStore`'s blob pool (spec/ast.md §9.1,
    /// `Aggregate` values).
    BlobId, u32::MAX, "BlobId", space::BLOB
);
define_id!(
    /// An interned reflection (`std::meta::info`, spec/reflection.md §2, M6).
    ReflectionId, u32::MAX, "ReflectionId", space::REFLECTION
);
define_id!(
    /// A data member description (`std::meta::data_member_spec`, §10;
    /// reserved for M6-08, never handed out yet).
    DmsId, u32::MAX, "DmsId", space::DMS
);
define_id!(
    /// An annotation (P3394 `[[=expr]]`, §10; reserved, never handed out
    /// yet).
    AnnId, u32::MAX, "AnnId", space::ANN
);
define_id!(
    /// The bytes of a string literal in the `StringPool`.
    StringLitId, u32::MAX, "StringLitId", space::STRING_LIT
);
define_id!(
    /// A redeclaration group (spec/ast.md §4.4).
    RedeclGroupId, u32::MAX, "RedeclGroupId", space::REDECL_GROUP
);
define_id!(
    /// A lambda's side record (closure data, M3).
    LambdaId, u32::MAX, "LambdaId", space::LAMBDA
);
define_id!(
    /// A type node in the `TypeContext` (spec/types.md §2). 29 bits, so a
    /// [`QualType`] can carry three qualifier bits.
    TypeId, (1 << 29) - 1, "TypeId", space::TYPE
);
define_id!(
    /// An interned template name (spec/types.md §8.1).
    TemplateNameId, u32::MAX, "TemplateNameId", space::TEMPLATE_NAME
);
define_id!(
    /// An interned nested-name-specifier (spec/types.md §8.2).
    NnsId, u32::MAX, "NnsId", space::NNS
);
define_id!(
    /// An interned template argument list (spec/types.md §8.3).
    ArgListId, u32::MAX, "ArgListId", space::ARG_LIST
);
define_id!(
    /// An interned canonical expression profile (spec/types.md §7.2).
    ProfileId, u32::MAX, "ProfileId", space::PROFILE
);

define_id!(
    /// A template parameter list in `TemplateTables` (spec/sema-templates.md
    /// §2.1, §11.2).
    TemplateParamListId, u32::MAX, "TemplateParamListId", space::TEMPLATE_PARAM_LIST
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

/// As the raw encoding (`Option<Self>` too, 0 for `None`); decoding
/// rejects a zero type index and records the type index for the range
/// check (`space::TYPE`).
impl kiln_snap::Snap for QualType {
    const SCHEMA_HASH: u64 = kiln_snap::SchemaHash::new("id", "QualType").finish();
    const SIZE: Option<usize> = Some(4);
    const OPTION_SIZE: Option<usize> = Some(4);
    #[inline]
    fn put(&self, w: &mut kiln_snap::Writer) {
        kiln_snap::Snap::put(&self.0.get(), w);
    }
    #[inline]
    fn get(r: &mut kiln_snap::Reader<'_>) -> Result<Self, kiln_snap::SnapError> {
        Self::get_option(r)?.ok_or_else(|| zero_id(r, "QualType"))
    }
    fn schema(s: &mut kiln_snap::Schema) {
        s.register::<Self>(kiln_snap::Desc::new("id").hash(Self::SCHEMA_HASH));
    }
    #[inline]
    fn put_option(v: Option<&Self>, w: &mut kiln_snap::Writer) {
        kiln_snap::Snap::put(&v.map_or(0, |x| x.0.get()), w);
    }
    #[inline]
    fn get_option(r: &mut kiln_snap::Reader<'_>) -> Result<Option<Self>, kiln_snap::SnapError> {
        let at = r.offset();
        let v = <u32 as kiln_snap::Snap>::get(r)?;
        if v == 0 {
            return Ok(None);
        }
        let ty = v >> 3;
        if ty == 0 {
            return Err(kiln_snap::SnapError::new(
                kiln_snap::ErrorKind::BadValue("QualType with a zero type index"),
                "QualType",
                at,
            ));
        }
        if ty != TypeId::MAX_INDEX {
            r.note_id(space::TYPE, ty);
        }
        Ok(NonZeroU32::new(v).map(Self))
    }
}

impl QualType {
    /// The qualifier bits' mask.
    pub const QUALS_MASK: u32 = 0b111;

    /// `ty` with qualifier bits `quals` (only bits 0..=2 are kept).
    #[inline]
    pub const fn new(ty: TypeId, quals: u8) -> Self {
        let raw = (ty.index() << 3) | (quals as u32 & Self::QUALS_MASK);
        match NonZeroU32::new(raw) {
            Some(n) => Self(n),
            // Invariant: a `TypeId` index is never 0, so `raw` is nonzero.
            None => panic!("invariant: a TypeId index is never 0"),
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
