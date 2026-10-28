//! Type properties the conversions are defined over: integer widths,
//! signedness and conversion ranks ([conv.rank]), the promoted types
//! ([conv.prom]) and the floating-point value-set order
//! ([conv.rank]/2, [basic.extended.fp]).

use kiln_ast::QualType;
use kiln_ast::types::{BuiltinType, FloatFormat, TypeView};

use crate::Sema;

/// The width in bits of builtin `b` on the target, with the LP64 widths
/// when the target leaves a size unset (`bool` is 1 bit wide for value
/// purposes, as in clang's `getIntWidth`).
pub(crate) fn builtin_width(s: &Sema<'_, '_>, b: BuiltinType) -> u32 {
    if b == BuiltinType::Bool {
        return 1;
    }
    let bytes = s.target.builtin_size_align(b).0;
    if bytes != 0 {
        return u32::from(bytes) * 8;
    }
    match b {
        BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar | BuiltinType::Char8 => 8,
        BuiltinType::Short
        | BuiltinType::UShort
        | BuiltinType::Char16
        | BuiltinType::Float16
        | BuiltinType::BFloat16
        | BuiltinType::Half => 16,
        BuiltinType::Int
        | BuiltinType::UInt
        | BuiltinType::WChar
        | BuiltinType::Char32
        | BuiltinType::Float => 32,
        BuiltinType::Int128 | BuiltinType::UInt128 | BuiltinType::Float128 => 128,
        BuiltinType::LongDouble => match s.target.long_double {
            FloatFormat::IeeeDouble => 64,
            FloatFormat::X87Extended => 80,
            _ => 128,
        },
        _ => 64,
    }
}

/// True if builtin integral `b` is signed on the target.
pub(crate) fn builtin_signed(s: &Sema<'_, '_>, b: BuiltinType) -> bool {
    match b {
        BuiltinType::Char => s.target.char_signed,
        BuiltinType::WChar => s.target.wchar_signed,
        _ => b.is_signed_integer(),
    }
}

/// The enumeration data of `q`'s enum (through its definition).
pub(crate) fn enum_data<'s>(
    s: &'s Sema<'_, '_>,
    q: QualType,
) -> Option<&'s kiln_ast::decl::EnumData> {
    let d = s.ast.types.as_enum(q)?;
    let def = s.ast.definition(d).unwrap_or(d);
    s.ast.decls.enumeration(def).or_else(|| s.ast.decls.enumeration(d))
}

/// True if `q` is an unscoped enumeration type.
pub fn is_unscoped_enum(s: &Sema<'_, '_>, q: QualType) -> bool {
    enum_data(s, q).is_some_and(|e| !e.is_scoped())
}

/// True if `q` is a scoped enumeration type.
pub fn is_scoped_enum(s: &Sema<'_, '_>, q: QualType) -> bool {
    enum_data(s, q).is_some_and(kiln_ast::decl::EnumData::is_scoped)
}

/// True for integral types and unscoped enumerations (clang's
/// `isIntegralOrUnscopedEnumerationType`).
pub fn is_integral_or_unscoped_enum(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.is_integral(q) || is_unscoped_enum(s, q)
}

/// True for arithmetic types and unscoped enumerations (clang's
/// `isArithmeticType` in C++).
pub fn is_arithmetic_or_unscoped_enum(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.is_arithmetic(q) || is_unscoped_enum(s, q)
}

/// The integer type standing for `q` in width/sign questions: `q`
/// itself for integral types, an enumeration's underlying type.
fn integer_repr(s: &Sema<'_, '_>, q: QualType) -> Option<QualType> {
    if s.ast.types.as_enum(q).is_some() {
        return enum_data(s, q)?.int_type;
    }
    s.ast.types.is_integral(q).then_some(q)
}

/// The value width of integral or enumeration type `q` in bits.
pub fn int_width(s: &Sema<'_, '_>, q: QualType) -> Option<u32> {
    let r = integer_repr(s, q)?;
    match s.ast.types.canonical_view(r) {
        TypeView::Builtin(b) => Some(builtin_width(s, b)),
        TypeView::BitInt { width, .. } => Some(width),
        _ => None,
    }
}

/// True if integral or enumeration type `q` is signed (clang's
/// `isSignedIntegerOrEnumerationType`).
pub fn int_signed(s: &Sema<'_, '_>, q: QualType) -> bool {
    let Some(r) = integer_repr(s, q) else { return false };
    match s.ast.types.canonical_view(r) {
        TypeView::Builtin(b) => builtin_signed(s, b),
        TypeView::BitInt { signed, .. } => signed,
        _ => false,
    }
}

/// The integer conversion rank of `q` ([conv.rank]/1) as an ordinal:
/// greater means greater rank, equal means equal rank. `None` for
/// non-integral types.
///
/// The rank is `width * 8 + sub`, where `sub` orders the standard types
/// of equal width (`bool` < `char` < `short` < `int` < `long` <
/// `long long` < `__int128`); `char8_t`, `char16_t`, `char32_t` and
/// `wchar_t` rank with their underlying types ([conv.rank]/1.9), and a
/// `_BitInt(N)` ranks below a standard type of the same width
/// ([conv.rank]/1.4 via C23 6.3.1.1).
pub fn integer_rank(s: &Sema<'_, '_>, q: QualType) -> Option<u32> {
    match s.ast.types.canonical_view(q) {
        TypeView::Builtin(b) if b.is_integral() => {
            let w = builtin_width(s, b);
            let sub = match b {
                BuiltinType::Bool => 1,
                BuiltinType::Char | BuiltinType::SChar | BuiltinType::UChar => 2,
                BuiltinType::Short | BuiltinType::UShort => 3,
                BuiltinType::Int | BuiltinType::UInt => 4,
                BuiltinType::Long | BuiltinType::ULong => 5,
                BuiltinType::LongLong | BuiltinType::ULongLong => 6,
                BuiltinType::Int128 | BuiltinType::UInt128 => 7,
                // The underlying type's rank: the first standard type of
                // that width.
                _ => standard_sub_for_width(s, w),
            };
            Some((w << 3) + sub)
        }
        TypeView::BitInt { width, .. } => Some(width << 3),
        _ => None,
    }
}

fn standard_sub_for_width(s: &Sema<'_, '_>, w: u32) -> u32 {
    const LADDER: [(BuiltinType, u32); 6] = [
        (BuiltinType::UChar, 2),
        (BuiltinType::UShort, 3),
        (BuiltinType::UInt, 4),
        (BuiltinType::ULong, 5),
        (BuiltinType::ULongLong, 6),
        (BuiltinType::UInt128, 7),
    ];
    LADDER.iter().find(|(b, _)| builtin_width(s, *b) == w).map_or(4, |&(_, sub)| sub)
}

/// The unsigned integer type corresponding to signed `q` ([basic.fundamental]/2).
pub(crate) fn unsigned_counterpart(s: &mut Sema<'_, '_>, q: QualType) -> QualType {
    match s.ast.types.canonical_view(q) {
        TypeView::Builtin(b) => match b {
            BuiltinType::Char | BuiltinType::SChar => QualType::UCHAR,
            BuiltinType::Short => QualType::USHORT,
            BuiltinType::Int => QualType::UINT,
            BuiltinType::Long => QualType::ULONG,
            BuiltinType::LongLong => QualType::ULONGLONG,
            BuiltinType::Int128 => QualType::UINT128,
            BuiltinType::WChar | BuiltinType::Char32 => {
                if builtin_width(s, b) == builtin_width(s, BuiltinType::UInt) {
                    QualType::UINT
                } else {
                    QualType::ULONG
                }
            }
            _ => q.without_quals(),
        },
        TypeView::BitInt { width, .. } => s.ast.types.bit_int(false, width),
        _ => q.without_quals(),
    }
}

/// The first of `int`, `unsigned int`, `long`, `unsigned long`,
/// `long long`, `unsigned long long` that can represent every value of
/// a `width`-bit integer of signedness `signed` ([conv.prom]/2).
fn first_fitting(s: &Sema<'_, '_>, width: u32, signed: bool) -> Option<QualType> {
    const LADDER: [(BuiltinType, QualType); 6] = [
        (BuiltinType::Int, QualType::INT),
        (BuiltinType::UInt, QualType::UINT),
        (BuiltinType::Long, QualType::LONG),
        (BuiltinType::ULong, QualType::ULONG),
        (BuiltinType::LongLong, QualType::LONGLONG),
        (BuiltinType::ULongLong, QualType::ULONGLONG),
    ];
    LADDER.iter().find_map(|&(b, q)| {
        let (w, sg) = (builtin_width(s, b), b.is_signed_integer());
        let fits = if sg == signed {
            w >= width
        } else if sg {
            w > width
        } else {
            false
        };
        fits.then_some(q)
    })
}

/// The type `q` is promoted to by an integral promotion ([conv.prom]),
/// or `None` if `q` is not promotable. `bit_field` is the width of the
/// bit-field the value is read from, if any ([conv.prom]/5).
///
/// - `bool` and integral types of rank below `int` promote to `int`, or
///   to `unsigned int` if `int` cannot hold all their values (/1, /6);
/// - `wchar_t`, `char8_t`, `char16_t`, `char32_t` promote to the first
///   of `int` .. `unsigned long long` holding all their values (/2);
/// - an unscoped enumeration promotes to its promotion type: the first
///   such type holding all values of the enumeration (/3); one with a
///   fixed underlying type to the underlying type's promoted type (/4);
/// - a bit-field narrower than `int`, or as wide and signed, to `int`;
///   one as wide as `int` and unsigned to `unsigned int` (/5, clang:
///   enumeration bit-fields promote as their type).
pub fn promoted_type(s: &Sema<'_, '_>, q: QualType, bit_field: Option<u32>) -> Option<QualType> {
    if s.ast.types.as_enum(q).is_some() {
        let e = enum_data(s, q)?;
        if e.is_scoped() {
            return None;
        }
        if e.is_fixed() {
            let u = e.int_type?;
            return Some(promoted_type(s, u, None).unwrap_or_else(|| u.without_quals()));
        }
        return e.promotion_type.map(QualType::without_quals);
    }
    let int_w = builtin_width(s, BuiltinType::Int);
    if let Some(w) = bit_field
        && matches!(s.ast.types.canonical_view(q), TypeView::Builtin(_))
        && s.ast.types.is_integral(q)
    {
        if w < int_w || (w == int_w && int_signed(s, q)) {
            return Some(QualType::INT);
        }
        if w == int_w {
            return Some(QualType::UINT);
        }
    }
    let TypeView::Builtin(b) = s.ast.types.canonical_view(q) else { return None };
    match b {
        BuiltinType::Bool => Some(QualType::INT),
        BuiltinType::WChar | BuiltinType::Char8 | BuiltinType::Char16 | BuiltinType::Char32 => {
            first_fitting(s, builtin_width(s, b), builtin_signed(s, b))
        }
        BuiltinType::Char
        | BuiltinType::SChar
        | BuiltinType::UChar
        | BuiltinType::Short
        | BuiltinType::UShort => {
            let w = builtin_width(s, b);
            if w < int_w || (w == int_w && builtin_signed(s, b)) {
                Some(QualType::INT)
            } else {
                Some(QualType::UINT)
            }
        }
        _ => None,
    }
}

/// The value-set format of floating builtin `b`.
pub(crate) fn float_format(s: &Sema<'_, '_>, b: BuiltinType) -> Option<FloatFormat> {
    Some(match b {
        BuiltinType::Half | BuiltinType::Float16 => FloatFormat::IeeeHalf,
        BuiltinType::BFloat16 => FloatFormat::BFloat,
        BuiltinType::Float => FloatFormat::IeeeSingle,
        BuiltinType::Double => FloatFormat::IeeeDouble,
        BuiltinType::LongDouble => s.target.long_double,
        BuiltinType::Float128 => FloatFormat::IeeeQuad,
        _ => return None,
    })
}

/// Precision (with the leading bit), maximum and minimum normal exponent
/// of a format.
pub(crate) fn format_params(f: FloatFormat) -> (u32, i32, i32) {
    match f {
        FloatFormat::IeeeHalf => (11, 15, -14),
        FloatFormat::BFloat => (8, 127, -126),
        FloatFormat::IeeeSingle => (24, 127, -126),
        FloatFormat::IeeeDouble => (53, 1023, -1022),
        FloatFormat::X87Extended => (64, 16383, -16382),
        FloatFormat::IeeeQuad => (113, 16383, -16382),
        // Approximated by its double-double precision and double range.
        FloatFormat::PpcDoubleDouble => (106, 1023, -1022),
    }
}

/// True if every value of format `a` is a value of format `b`.
pub(crate) fn format_subset(a: FloatFormat, b: FloatFormat) -> bool {
    let (pa, xa, na) = format_params(a);
    let (pb, xb, nb) = format_params(b);
    // Subnormals: the smallest positive value is 2^(emin - p + 1).
    pa <= pb && xa <= xb && na - (pa as i32) >= nb - (pb as i32) && na >= nb
}

/// How two floating-point types' conversion ranks compare
/// ([conv.rank]/2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatOrder {
    /// The first has lesser rank (or equal rank and lesser subrank).
    Less,
    /// Same rank and subrank.
    Equal,
    /// The first has greater rank (or equal rank and greater subrank).
    Greater,
    /// Neither value set contains the other.
    Unordered,
}

fn is_standard_float(b: BuiltinType) -> bool {
    matches!(b, BuiltinType::Float | BuiltinType::Double | BuiltinType::LongDouble)
}

fn standard_float_index(b: BuiltinType) -> u8 {
    match b {
        BuiltinType::Float => 0,
        BuiltinType::Double => 1,
        _ => 2,
    }
}

/// Compares the floating-point conversion ranks, then subranks, of
/// floating builtins `a` and `b` ([conv.rank]/2): by value set; for
/// equal value sets the standard types keep their order (`float` <
/// `double` < `long double`) and an extended type has greater subrank
/// than a standard one.
pub fn float_order(s: &Sema<'_, '_>, a: BuiltinType, b: BuiltinType) -> FloatOrder {
    let (Some(fa), Some(fb)) = (float_format(s, a), float_format(s, b)) else {
        return FloatOrder::Unordered;
    };
    if a == b {
        return FloatOrder::Equal;
    }
    match (format_subset(fa, fb), format_subset(fb, fa)) {
        (true, false) => FloatOrder::Less,
        (false, true) => FloatOrder::Greater,
        (false, false) => FloatOrder::Unordered,
        (true, true) => match (is_standard_float(a), is_standard_float(b)) {
            (true, true) => {
                if standard_float_index(a) < standard_float_index(b) {
                    FloatOrder::Less
                } else {
                    FloatOrder::Greater
                }
            }
            (true, false) => FloatOrder::Less,
            (false, true) => FloatOrder::Greater,
            (false, false) => FloatOrder::Equal,
        },
    }
}
