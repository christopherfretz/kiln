//! The logical view of constant values: what goes into the
//! [`ConstStore`](super::ConstStore) and what comes back out of it
//! (spec/ast.md §9.1).
//!
//! These types borrow; they are the freeze/thaw boundary, not the storage.
//! The storage is the store's 16-byte nodes plus pools (see the module
//! docs of [`consts`](super)).

use std::fmt;

use crate::ids::{ConstValueId, DeclId, ExprId, QualType, ReflectionId, StringLitId};

/// Two's-complement integer bits of a fixed width (1 to
/// [`IntBits::MAX_WIDTH`] bits), little-endian `u64` words.
///
/// The representation is **normalized**: exactly `ceil(width / 64)` words
/// and every bit at or above `width` is zero. Signedness is not part of the
/// bits (it is a property of the value, [`Scalar::Int::signed`]).
/// Widths up to 128 bits are held inline; wider `_BitInt`s borrow their
/// words.
#[derive(Clone, Copy)]
pub struct IntBits<'a> {
    width: u32,
    repr: BitsRepr<'a>,
}

#[derive(Clone, Copy)]
enum BitsRepr<'a> {
    Inline([u64; 2]),
    Words(&'a [u64]),
}

impl<'a> IntBits<'a> {
    /// The widest integer the store accepts: 2^24 − 1 bits (clang's
    /// `_BitInt` limit is 2^23).
    pub const MAX_WIDTH: u32 = (1 << 24) - 1;

    /// Number of `u64` words for `width` bits.
    pub const fn words_for(width: u32) -> usize {
        width.div_ceil(64) as usize
    }

    /// The low `width` bits of `value` (zero-extension / truncation).
    ///
    /// # Panics
    /// If `width` is not in `1..=128`.
    pub fn from_u128(width: u32, value: u128) -> Self {
        assert!((1..=128).contains(&width), "IntBits::from_u128: width {width} not in 1..=128");
        let v = if width == 128 { value } else { value & ((1u128 << width) - 1) };
        IntBits { width, repr: BitsRepr::Inline([v as u64, (v >> 64) as u64]) }
    }

    /// The two's-complement encoding of `value` in `width` bits (sign
    /// extension / truncation).
    ///
    /// # Panics
    /// If `width` is not in `1..=128`.
    pub fn from_i128(width: u32, value: i128) -> Self {
        Self::from_u128(width, value as u128)
    }

    /// Borrows already-normalized words; `None` if `width` is out of range,
    /// `words.len() != words_for(width)`, or a bit at or above `width` is
    /// set.
    pub fn from_words(width: u32, words: &'a [u64]) -> Option<Self> {
        if width == 0 || width > Self::MAX_WIDTH || words.len() != Self::words_for(width) {
            return None;
        }
        let rem = width % 64;
        if rem != 0 && words[words.len() - 1] >> rem != 0 {
            return None;
        }
        if width <= 128 {
            let mut w = [0u64; 2];
            w[..words.len()].copy_from_slice(words);
            return Some(IntBits { width, repr: BitsRepr::Inline(w) });
        }
        Some(IntBits { width, repr: BitsRepr::Words(words) })
    }

    /// The bit width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The normalized words (`words_for(width)` of them, little-endian).
    pub fn words(&self) -> &[u64] {
        match &self.repr {
            BitsRepr::Inline(w) => &w[..Self::words_for(self.width)],
            BitsRepr::Words(w) => w,
        }
    }

    /// The value zero-extended to `u128`, if every bit above 127 is zero.
    pub fn to_u128(&self) -> Option<u128> {
        let w = self.words();
        if w.len() > 2 && w[2..].iter().any(|&x| x != 0) {
            return None;
        }
        let lo = u128::from(w[0]);
        let hi = w.get(1).map_or(0, |&h| u128::from(h));
        Some(lo | (hi << 64))
    }

    /// The value as a signed `width`-bit integer, sign-extended to `i128`,
    /// if it fits.
    pub fn to_i128(&self) -> Option<i128> {
        if self.width <= 128 {
            let v = self.to_u128()?;
            let shift = 128 - self.width;
            return Some(((v << shift) as i128) >> shift);
        }
        let neg = self.sign_bit();
        let w = self.words();
        let fill = if neg { u64::MAX } else { 0 };
        let rem = self.width % 64;
        // Every word above the low two must be the sign fill (masked in the
        // top word), and bit 127 must agree with the sign.
        for (i, &x) in w.iter().enumerate().skip(2) {
            let want = if i == w.len() - 1 && rem != 0 { fill & ((1u64 << rem) - 1) } else { fill };
            if x != want {
                return None;
            }
        }
        let v = u128::from(w[0]) | (u128::from(w[1]) << 64);
        if (v >> 127 != 0) != neg {
            return None;
        }
        Some(v as i128)
    }

    /// Bit `width − 1` (the sign bit when the value is signed).
    pub fn sign_bit(&self) -> bool {
        let i = self.width - 1;
        self.words()[(i / 64) as usize] >> (i % 64) & 1 != 0
    }

    /// True if every bit is zero.
    pub fn is_zero(&self) -> bool {
        self.words().iter().all(|&w| w == 0)
    }
}

impl PartialEq for IntBits<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width && self.words() == other.words()
    }
}
impl Eq for IntBits<'_> {}

impl fmt::Debug for IntBits<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "i{}:0x", self.width)?;
        for w in self.words().iter().rev() {
            write!(f, "{w:016x}")?;
        }
        Ok(())
    }
}

/// A binary floating-point format. Values are stored as their exact bit
/// pattern in this format, never rounded through `f64` (so `-0.0`, NaN
/// payloads and `long double` precision are preserved).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum FloatKind {
    /// IEEE binary16 (`_Float16`, `__fp16`).
    Half = 0,
    /// bfloat16 (`__bf16`).
    BFloat16 = 1,
    /// IEEE binary32 (`float`).
    Single = 2,
    /// IEEE binary64 (`double`; `long double` on targets where it is
    /// `double`).
    Double = 3,
    /// x87 80-bit extended precision with an explicit integer bit
    /// (`long double` on x86); 80 significant bits.
    X87Extended = 4,
    /// IEEE binary128 (`__float128`; `long double` on 64-bit Arm Linux).
    Quad = 5,
}

impl FloatKind {
    /// All kinds, in discriminant order.
    pub const ALL: [FloatKind; 6] = [
        FloatKind::Half,
        FloatKind::BFloat16,
        FloatKind::Single,
        FloatKind::Double,
        FloatKind::X87Extended,
        FloatKind::Quad,
    ];

    /// Number of significant bits in the encoding (16, 16, 32, 64, 80, 128).
    pub const fn bits(self) -> u32 {
        match self {
            FloatKind::Half | FloatKind::BFloat16 => 16,
            FloatKind::Single => 32,
            FloatKind::Double => 64,
            FloatKind::X87Extended => 80,
            FloatKind::Quad => 128,
        }
    }

    /// Number of image bytes the encoding's significant bits occupy
    /// (`bits / 8`; the in-memory object may be larger, e.g. 16 bytes for
    /// x87 on x86-64, with the tail as padding).
    pub const fn value_bytes(self) -> u32 {
        self.bits() / 8
    }

    pub(crate) fn from_u8(v: u8) -> Option<Self> {
        Self::ALL.get(usize::from(v)).copied()
    }
}

/// The kind of a null pointer value; it determines how many bytes the value
/// occupies inside an aggregate image ([`ConstStore::reloc_width`](super::ConstStore::reloc_width)).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum NullKind {
    /// A null object or function pointer, a `std::nullptr_t` value.
    Pointer = 0,
    /// A null pointer to data member (Itanium: one pointer-sized word).
    MemberData = 1,
    /// A null pointer to member function (Itanium: two pointer-sized words).
    MemberFunction = 2,
}

/// Whether a pointer to member designates a data member or a member
/// function.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum MemberKind {
    /// Pointer to data member (`&S::x`).
    Data,
    /// Pointer to member function (`&S::f`).
    Function,
}

/// What a non-null pointer (or reference) value points into.
///
/// Pointers are `(base, byte offset)` as in the Oven's memory model
/// (spec/oven.md "Abstract memory"): the designator path is recovered from
/// the base's type and the offset through the record layout, never stored.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PtrBase {
    /// A variable or template parameter object (the first declaration).
    Decl(DeclId),
    /// A function (pointer to function / function reference; offset 0).
    Function(DeclId),
    /// A lifetime-extended temporary (`LifetimeExtendedTemporaryDecl`).
    Temporary(DeclId),
    /// A string literal object (offset in bytes from its first code unit).
    ///
    /// Each evaluation of a string literal may create a distinct object
    /// ([lex.string]/9), so the object is identified by the
    /// `StringLiteral` expression that created it (`origin`); `lit` is its
    /// contents. Two pointers into equal literals from different
    /// expressions are different values, and comparing them is
    /// unspecified ([expr.const]/5.24).
    StringLit {
        /// The contents.
        lit: StringLitId,
        /// The `StringLiteral` expression whose evaluation created the
        /// object.
        origin: ExprId,
    },
    /// A subobject of another frozen value of this store (an object with
    /// static storage that has no declaration of its own).
    Const(ConstValueId),
    /// The `std::type_info` object of a type (`&typeid(T)`).
    TypeInfo(QualType),
}

impl PtrBase {
    pub(crate) fn code(self) -> u32 {
        match self {
            PtrBase::Decl(_) => 0,
            PtrBase::Function(_) => 1,
            PtrBase::Temporary(_) => 2,
            PtrBase::StringLit { .. } => 3,
            PtrBase::Const(_) => 4,
            PtrBase::TypeInfo(_) => 5,
        }
    }

    pub(crate) fn raw(self) -> u32 {
        match self {
            PtrBase::Decl(d) | PtrBase::Function(d) | PtrBase::Temporary(d) => d.index(),
            PtrBase::StringLit { lit, .. } => lit.index(),
            PtrBase::Const(c) => c.index(),
            PtrBase::TypeInfo(t) => t.to_raw(),
        }
    }

    /// The second id of a base that has one (a string literal's origin).
    pub(crate) fn aux(self) -> Option<u32> {
        match self {
            PtrBase::StringLit { origin, .. } => Some(origin.index()),
            _ => None,
        }
    }

    /// The base from its code, id and second id ([`PtrBase::aux`]).
    pub(crate) fn from_parts_aux(code: u32, raw: u32, aux: u32) -> Option<Self> {
        match code {
            3 => Some(PtrBase::StringLit {
                lit: StringLitId::try_new(raw)?,
                origin: ExprId::try_new(aux)?,
            }),
            _ => Self::from_parts(code, raw),
        }
    }

    pub(crate) fn from_parts(code: u32, raw: u32) -> Option<Self> {
        Some(match code {
            0 => PtrBase::Decl(DeclId::try_new(raw)?),
            1 => PtrBase::Function(DeclId::try_new(raw)?),
            2 => PtrBase::Temporary(DeclId::try_new(raw)?),
            // 3 was the retired string-literal base (now `Const`).
            4 => PtrBase::Const(ConstValueId::try_new(raw)?),
            5 => PtrBase::TypeInfo(QualType::from_raw(raw)?),
            _ => return None,
        })
    }
}

/// A scalar constant value.
///
/// Every scalar carries its type, which must be the **canonical,
/// cv-unqualified** type (the store does not canonicalize; equal values of
/// different types are different values, as [temp.type] requires).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scalar<'a> {
    /// An integer, enumeration or character value (not `bool`).
    Int {
        /// The type.
        ty: QualType,
        /// Whether the bits are interpreted as two's-complement signed.
        signed: bool,
        /// The bits; the width is the type's width (`_BitInt(N)` → N).
        bits: IntBits<'a>,
    },
    /// A `bool`.
    Bool {
        /// The type.
        ty: QualType,
        /// The value.
        value: bool,
    },
    /// A floating-point value as its exact bit pattern.
    Float {
        /// The type.
        ty: QualType,
        /// The format.
        kind: FloatKind,
        /// The encoding, zero-extended; bits at or above `kind.bits()` must
        /// be zero.
        bits: u128,
    },
    /// A null pointer, null pointer to member, or `nullptr`.
    Null {
        /// The type.
        ty: QualType,
        /// Which kind of null (determines the image width).
        kind: NullKind,
    },
    /// A non-null pointer or a reference.
    Pointer {
        /// The pointer (or reference) type.
        ty: QualType,
        /// What it points into.
        base: PtrBase,
        /// Byte offset from the start of `base` (0 for functions).
        offset: u64,
    },
    /// A non-null pointer to member.
    MemberPtr {
        /// The member pointer type.
        ty: QualType,
        /// The `FieldDecl` or `CXXMethodDecl`.
        member: DeclId,
        /// Data member or member function.
        kind: MemberKind,
        /// clang's `isMemberPointerToDerivedMember`: the path walks from the
        /// member's class towards derived classes.
        derived: bool,
        /// The classes traversed by base/derived conversions (clang's
        /// `getMemberPointerPath`), outermost first; empty if none.
        path: &'a [DeclId],
    },
    /// A `std::meta::info` value (M6).
    Reflection {
        /// The type (`std::meta::info`).
        ty: QualType,
        /// The reflection.
        id: ReflectionId,
    },
}

impl Scalar<'_> {
    /// The value's type.
    pub fn ty(&self) -> QualType {
        match *self {
            Scalar::Int { ty, .. }
            | Scalar::Bool { ty, .. }
            | Scalar::Float { ty, .. }
            | Scalar::Null { ty, .. }
            | Scalar::Pointer { ty, .. }
            | Scalar::MemberPtr { ty, .. }
            | Scalar::Reflection { ty, .. } => ty,
        }
    }
}

/// A half-open byte range `[start, start + len)` of an image.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ByteRange {
    /// First byte.
    pub start: u32,
    /// Length in bytes (non-zero in a stored image).
    pub len: u32,
}

impl ByteRange {
    /// One past the last byte.
    pub fn end(self) -> u64 {
        u64::from(self.start) + u64::from(self.len)
    }
}

/// A relocation: the pointer-like subobject at byte `at` of an image has
/// the value `value` (a [`Scalar::Pointer`], [`Scalar::Null`],
/// [`Scalar::MemberPtr`] or [`Scalar::Reflection`]). The image bytes it
/// covers ([`ConstStore::reloc_width`](super::ConstStore::reloc_width)) are
/// zero.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Reloc {
    /// Byte offset in the image.
    pub at: u32,
    /// The interned pointer-like value.
    pub value: ConstValueId,
}

/// The active member of the union subobject at byte `at`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UnionMember {
    /// Byte offset of the union subobject in the image.
    pub at: u32,
    /// The active member's `FieldDecl` (identifies the union too, so
    /// nested unions at one offset are unambiguous).
    pub field: DeclId,
}

/// An aggregate value (class, array, union, `_Complex`, vector) as the
/// Oven freezes it: the object's bytes in target layout plus side tables.
///
/// Canonical form (required by
/// [`ConstStore::push_aggregate`](super::ConstStore::push_aggregate), so
/// that equal values have equal images and interning is a byte compare):
/// - `bytes` is the complete object (`sizeof(T)` bytes), little-endian.
/// - `relocs` sorted by `at`, non-overlapping, each within the image, image
///   bytes under each relocation zero. **Every** pointer-like subobject has
///   one, including null pointers.
/// - `padding` sorted, non-empty, non-adjacent (coalesced), within the
///   image, disjoint from relocations, bytes zero. Padding is every byte
///   that is not part of a value: alignment/tail padding, unused bytes of
///   `long double`, and the bytes of a union not covered by its active
///   member (all of them when no member is active).
/// - `unions` sorted by `(at, field)`, one entry per union subobject with
///   an active member.
///
/// Every other byte holds an initialized scalar (the result of a constant
/// expression is fully initialized, [expr.const]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ByteImage<'a> {
    /// The aggregate's canonical, cv-unqualified type.
    pub ty: QualType,
    /// The object representation.
    pub bytes: &'a [u8],
    /// Pointer-like subobjects.
    pub relocs: &'a [Reloc],
    /// Bytes that are not part of any value.
    pub padding: &'a [ByteRange],
    /// Active members of union subobjects.
    pub unions: &'a [UnionMember],
}

/// A constant value as stored: a scalar or an aggregate image.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstValue<'a> {
    /// A scalar.
    Scalar(Scalar<'a>),
    /// An aggregate.
    Aggregate(ByteImage<'a>),
}

impl ConstValue<'_> {
    /// The value's type.
    pub fn ty(&self) -> QualType {
        match self {
            ConstValue::Scalar(s) => s.ty(),
            ConstValue::Aggregate(a) => a.ty,
        }
    }
}

/// The discriminant of a stored value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ValueKind {
    /// [`Scalar::Int`].
    Int,
    /// [`Scalar::Bool`].
    Bool,
    /// [`Scalar::Float`].
    Float,
    /// [`Scalar::Null`].
    Null,
    /// [`Scalar::Pointer`].
    Pointer,
    /// [`Scalar::MemberPtr`].
    MemberPtr,
    /// [`Scalar::Reflection`].
    Reflection,
    /// [`ConstValue::Aggregate`].
    Aggregate,
}

/// What a run of image bytes holds ([`ConstStore::shadow`](super::ConstStore::shadow)).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShadowKind {
    /// Initialized scalar bytes (the bytes are the value).
    Value,
    /// Padding: not part of any value (zero in the image; uninitialized
    /// when thawed).
    Padding,
    /// A pointer-like subobject with this value (zero in the image).
    Pointer(ConstValueId),
}

/// A maximal run of image bytes with one [`ShadowKind`]; a `Pointer` run
/// is exactly one relocation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShadowRun {
    /// The bytes.
    pub range: ByteRange,
    /// What they hold.
    pub kind: ShadowKind,
}

/// Why a value could not be stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConstError {
    /// An id space or a 4 GiB pool is exhausted (a fatal diagnostic for
    /// the caller, spec/ast.md §2.1).
    Capacity(&'static str),
    /// The value violates the canonical-form contract (an internal error in
    /// the caller; the message names the rule).
    Malformed(&'static str),
}

impl fmt::Display for ConstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstError::Capacity(what) => write!(f, "constant store capacity exhausted: {what}"),
            ConstError::Malformed(what) => write!(f, "malformed constant value: {what}"),
        }
    }
}

impl std::error::Error for ConstError {}
