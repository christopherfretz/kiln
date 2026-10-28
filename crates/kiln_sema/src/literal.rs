//! Literal typing ([lex.icon], [lex.fcon], [lex.ccon], [lex.string]).
//!
//! STUB(WP-049): this file is a placeholder with the agreed signatures;
//! WP-049 owns the real implementation and replaces it wholesale. The
//! stub types literals well enough for the parser's tests (decimal and
//! non-decimal integer ladders, `float`/`double`/`long double`, character
//! and string literal types) and reports no literal warnings.

use kiln_ast::consts::{FloatKind, Scalar};
use kiln_ast::types::Quals;
use kiln_ast::{ConstValueId, QualType};
use kiln_diag::DiagId;
use kiln_lex::Encoding;
use kiln_lex::literal::{
    CharLiteral, CharType, FloatLiteral, FloatSuffix, IntSize, IntegerLiteral, Radix,
};

use crate::Sema;

/// A literal that cannot be typed (`id` is emitted at the literal).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LiteralError {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments.
    pub args: Vec<String>,
}

/// A floating value's encoding: its format and its bits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FloatBits {
    /// The format.
    pub kind: FloatKind,
    /// The bits, zero-extended.
    pub bits: u128,
}

/// An integer literal's type, value and warning.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IntLiteralTyping {
    /// The type.
    pub ty: QualType,
    /// The value.
    pub value: u128,
    /// A warning to emit at the literal, if any.
    pub warning: Option<LiteralError>,
}

/// A floating literal's type, value and warning.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FloatLiteralTyping {
    /// The type.
    pub ty: QualType,
    /// The value.
    pub bits: FloatBits,
    /// A warning to emit at the literal, if any.
    pub warning: Option<LiteralError>,
}

fn bits_of(s: &Sema<'_, '_>, q: QualType) -> u32 {
    let b = s.ast.types.builtin(q).expect("integer literal types are builtin");
    u32::from(s.target.builtin_size_align(b).0) * 8
}

/// Types an integer literal ([lex.icon] table 8).
///
/// # Errors
/// If no integer type can represent the value.
pub fn type_integer_literal(
    s: &Sema<'_, '_>,
    lit: &IntegerLiteral<'_>,
) -> Result<(QualType, u128), LiteralError> {
    let t = classify_integer_literal(s, lit)?;
    Ok((t.ty, t.value))
}

/// Types an integer literal, with any warning.
///
/// # Errors
/// If no integer type can represent the value.
pub fn classify_integer_literal(
    s: &Sema<'_, '_>,
    lit: &IntegerLiteral<'_>,
) -> Result<IntLiteralTyping, LiteralError> {
    let too_large = || LiteralError { id: DiagId::sema_stub_literal_too_large, args: vec![] };
    let value = lit.value().ok_or_else(too_large)?;
    let decimal = lit.radix == Radix::Decimal;
    let u = lit.suffix.unsigned;
    let signed_ladder: &[QualType] = match lit.suffix.size {
        IntSize::None => &[QualType::INT, QualType::LONG, QualType::LONGLONG],
        IntSize::Long => &[QualType::LONG, QualType::LONGLONG],
        IntSize::LongLong => &[QualType::LONGLONG],
        IntSize::Size => &[QualType::LONG],
    };
    let unsigned_of = |q: QualType| match q {
        QualType::INT => QualType::UINT,
        QualType::LONG => QualType::ULONG,
        _ => QualType::ULONGLONG,
    };
    let mut ladder: Vec<QualType> = Vec::new();
    for &q in signed_ladder {
        if !u {
            ladder.push(q);
        }
        if u || !decimal {
            ladder.push(unsigned_of(q));
        }
    }
    for q in ladder {
        let w = bits_of(s, q);
        let signed = !matches!(q, QualType::UINT | QualType::ULONG | QualType::ULONGLONG);
        let max = if signed { (1u128 << (w - 1)) - 1 } else { (1u128 << w) - 1 };
        if value <= max {
            return Ok(IntLiteralTyping { ty: q, value, warning: None });
        }
    }
    // clang: too large for any signed type; typed `unsigned long long`
    // with -Wimplicitly-unsigned-literal (WP-049).
    if value <= u128::from(u64::MAX) {
        return Ok(IntLiteralTyping { ty: QualType::ULONGLONG, value, warning: None });
    }
    Err(too_large())
}

/// Types a floating literal ([lex.fcon]).
///
/// # Errors
/// For suffixes the stub does not support.
pub fn type_float_literal(
    s: &Sema<'_, '_>,
    lit: &FloatLiteral<'_>,
) -> Result<(QualType, FloatBits), LiteralError> {
    let t = classify_float_literal(s, lit)?;
    Ok((t.ty, t.bits))
}

/// Types a floating literal, with any warning.
///
/// # Errors
/// For suffixes the stub does not support.
pub fn classify_float_literal(
    s: &Sema<'_, '_>,
    lit: &FloatLiteral<'_>,
) -> Result<FloatLiteralTyping, LiteralError> {
    let v = lit.parse_f64();
    let (ty, bits) = match lit.suffix {
        FloatSuffix::None | FloatSuffix::F64 => {
            (QualType::DOUBLE, FloatBits { kind: FloatKind::Double, bits: u128::from(v.to_bits()) })
        }
        FloatSuffix::F | FloatSuffix::F32 => {
            #[allow(clippy::cast_possible_truncation, reason = "rounding to float")]
            let f = v as f32;
            (QualType::FLOAT, FloatBits { kind: FloatKind::Single, bits: u128::from(f.to_bits()) })
        }
        FloatSuffix::L => {
            let _ = s;
            (QualType::LONGDOUBLE, FloatBits { kind: FloatKind::Quad, bits: f64_to_quad(v) })
        }
        _ => {
            return Err(LiteralError {
                id: DiagId::parse_unsupported,
                args: vec!["this floating literal suffix".into()],
            });
        }
    };
    Ok(FloatLiteralTyping { ty, bits, warning: None })
}

/// Widens `v` to IEEE binary128 (exact).
fn f64_to_quad(v: f64) -> u128 {
    let b = v.to_bits();
    let sign = u128::from(b >> 63) << 127;
    let exp = (b >> 52) & 0x7ff;
    let man = u128::from(b & ((1u64 << 52) - 1));
    if exp == 0x7ff {
        return sign | (0x7fffu128 << 112) | (man << 60);
    }
    if exp == 0 {
        if man == 0 {
            return sign;
        }
        // Subnormal double: normalize.
        let shift = man.leading_zeros() - (128 - 53);
        let m = (man << shift) & ((1u128 << 52) - 1);
        let e = 16383 - 1022 - u128::from(shift);
        return sign | (e << 112) | (m << 60);
    }
    let e = u128::from(exp) - 1023 + 16383;
    sign | (e << 112) | (man << 60)
}

/// Types a character literal ([lex.ccon]).
///
/// # Errors
/// Never, in the stub.
pub fn type_char_literal(
    s: &Sema<'_, '_>,
    lit: &CharLiteral<'_>,
) -> Result<(QualType, u32), LiteralError> {
    let _ = s;
    let ty = match lit.ty {
        CharType::Char => QualType::CHAR,
        CharType::Int => QualType::INT,
        CharType::WChar => QualType::WCHAR,
        CharType::Char8 => QualType::CHAR8,
        CharType::Char16 => QualType::CHAR16,
        CharType::Char32 => QualType::CHAR32,
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "bit pattern")]
    Ok((ty, lit.value as u32))
}

/// The type of a string literal of `encoding` with `units` code units,
/// including the terminating NUL: `const charT[units]`.
pub fn string_literal_type(s: &mut Sema<'_, '_>, encoding: Encoding, units: u64) -> QualType {
    let elem = match encoding {
        Encoding::Ordinary => QualType::CHAR,
        Encoding::Wide => QualType::WCHAR,
        Encoding::Utf8 => QualType::CHAR8,
        Encoding::Utf16 => QualType::CHAR16,
        Encoding::Utf32 => QualType::CHAR32,
    };
    let t = &mut s.ast.types;
    let c = t.qualify(elem, Quals::CONST);
    t.constant_array(c, units, None)
}

/// Interns a floating constant of type `ty`.
pub fn float_const(s: &mut Sema<'_, '_>, ty: QualType, bits: FloatBits) -> ConstValueId {
    s.ast
        .consts
        .push_scalar(Scalar::Float { ty, kind: bits.kind, bits: bits.bits })
        .expect("float constants intern")
}
