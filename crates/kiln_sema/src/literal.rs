//! Literal typing ([lex.icon], [lex.fcon], [lex.ccon], [lex.string]),
//! with clang's diagnostics for literals no type can hold.
//!
//! Floating literals are rounded correctly (to nearest, ties to even)
//! from their exact decimal or hexadecimal value, in the literal's
//! format on the target (`long double` per `TargetTypes::long_double`).

use kiln_ast::consts::{FloatKind, Scalar};
use kiln_ast::types::{BuiltinType, Quals};
use kiln_ast::{ConstValueId, QualType};
use kiln_diag::DiagId;
use kiln_lex::Encoding;
use kiln_lex::literal::{
    CharLiteral, CharType, FloatLiteral, FloatSuffix, IntSize, IntegerLiteral, Radix,
};

use crate::Sema;
use crate::conv::narrow::float_kind;
use crate::conv::rank::{builtin_signed, builtin_width};
use crate::conv::soft::{self, Big};

/// A literal that cannot be typed, or a warning about one: `id` with
/// `args`, emitted at the literal.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LiteralError {
    /// The diagnostic.
    pub id: DiagId,
    /// Its arguments, in order.
    pub args: Vec<String>,
}

impl LiteralError {
    fn new(id: DiagId, args: Vec<String>) -> LiteralError {
        LiteralError { id, args }
    }

    /// The diagnostic, with its caret at `loc` (the literal's).
    pub fn diag(&self, s: &Sema<'_, '_>, loc: kiln_source::Loc) -> kiln_diag::Diag {
        self.args.iter().fold(s.diag(self.id, loc), kiln_diag::Diag::arg)
    }
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

fn width_of(s: &Sema<'_, '_>, q: QualType) -> u32 {
    s.ast.types.builtin(q).map_or(64, |b| builtin_width(s, b))
}

fn signed_of(s: &Sema<'_, '_>, q: QualType) -> bool {
    s.ast.types.builtin(q).is_some_and(|b| builtin_signed(s, b))
}

fn fits(s: &Sema<'_, '_>, q: QualType, v: u128) -> bool {
    let w = width_of(s, q);
    let bits = if signed_of(s, q) { w - 1 } else { w };
    bits >= 128 || v < (1u128 << bits)
}

fn unsigned_of(q: QualType) -> QualType {
    match q {
        QualType::INT => QualType::UINT,
        QualType::LONG => QualType::ULONG,
        QualType::LONGLONG => QualType::ULONGLONG,
        other => other,
    }
}

fn signed_counterpart(q: QualType) -> QualType {
    match q {
        QualType::UINT => QualType::INT,
        QualType::ULONG => QualType::LONG,
        QualType::ULONGLONG => QualType::LONGLONG,
        QualType::USHORT => QualType::SHORT,
        other => other,
    }
}

/// Types an integer literal ([lex.icon] table 8): its type and value.
///
/// # Errors
/// As [`classify_integer_literal`].
pub fn type_integer_literal(
    s: &Sema<'_, '_>,
    lit: &IntegerLiteral<'_>,
) -> Result<(QualType, u128), LiteralError> {
    let t = classify_integer_literal(s, lit)?;
    Ok((t.ty, t.value))
}

/// Types an integer literal ([lex.icon]/3, table 8): the first type of
/// its suffix's list that can represent the value, where the list for
/// an unsuffixed (or `l`/`ll`) decimal literal holds only signed types
/// and non-decimal literals also try each unsigned counterpart. A `z`
/// suffix gives the signed type corresponding to `size_t` (`uz`:
/// `size_t`).
///
/// A decimal literal too large for every signed type but within
/// `unsigned long long` gets that type and the
/// `sema_int_literal_implicitly_unsigned` warning, as in clang.
///
/// # Errors
/// - `sema_int_literal_too_large` when the value exceeds 64 bits
///   (literals never get an extended integer type);
/// - `sema_size_t_literal_too_large` for a `z`/`uz` literal out of
///   range.
pub fn classify_integer_literal(
    s: &Sema<'_, '_>,
    lit: &IntegerLiteral<'_>,
) -> Result<IntLiteralTyping, LiteralError> {
    let too_large = || LiteralError::new(DiagId::sema_int_literal_too_large, vec![]);
    let value = lit.value().ok_or_else(too_large)?;
    let ull_w = width_of(s, QualType::ULONGLONG);
    if ull_w < 128 && value >= (1u128 << ull_w) {
        return Err(too_large());
    }
    let allow_unsigned = lit.suffix.unsigned || lit.radix != Radix::Decimal;
    let u = lit.suffix.unsigned;
    if lit.suffix.size == IntSize::Size {
        let size_t = QualType::unqual(s.target.size_t);
        let signed_size = signed_counterpart(size_t);
        if !u && fits(s, signed_size, value) {
            return Ok(IntLiteralTyping { ty: signed_size, value, warning: None });
        }
        if allow_unsigned && fits(s, size_t, value) {
            return Ok(IntLiteralTyping { ty: size_t, value, warning: None });
        }
        return Err(LiteralError::new(
            DiagId::sema_size_t_literal_too_large,
            vec![if u { "1" } else { "0" }.into()],
        ));
    }
    let signed_ladder: &[QualType] = match lit.suffix.size {
        IntSize::None => &[QualType::INT, QualType::LONG, QualType::LONGLONG],
        IntSize::Long => &[QualType::LONG, QualType::LONGLONG],
        _ => &[QualType::LONGLONG],
    };
    for &q in signed_ladder {
        if !u && fits(s, q, value) {
            return Ok(IntLiteralTyping { ty: q, value, warning: None });
        }
        let uq = unsigned_of(q);
        if allow_unsigned && fits(s, uq, value) {
            return Ok(IntLiteralTyping { ty: uq, value, warning: None });
        }
    }
    Ok(IntLiteralTyping {
        ty: QualType::ULONGLONG,
        value,
        warning: Some(LiteralError::new(DiagId::sema_int_literal_implicitly_unsigned, vec![])),
    })
}

/// Types a floating literal ([lex.fcon]): its type and value.
///
/// # Errors
/// As [`classify_float_literal`].
pub fn type_float_literal(
    s: &Sema<'_, '_>,
    lit: &FloatLiteral<'_>,
) -> Result<(QualType, FloatBits), LiteralError> {
    let t = classify_float_literal(s, lit)?;
    Ok((t.ty, t.bits))
}

/// The largest finite and smallest positive values of a format, as
/// clang prints them in `-Wliteral-range`.
fn range_strings(kind: FloatKind) -> (&'static str, &'static str) {
    match kind {
        FloatKind::Half => ("65504", "5.9605E-8"),
        FloatKind::BFloat16 => ("3.39E+38", "9.184E-41"),
        FloatKind::Single => ("3.40282347E+38", "1.40129846E-45"),
        FloatKind::Double => ("1.7976931348623157E+308", "4.9406564584124654E-324"),
        FloatKind::X87Extended => ("1.18973149535723176502E+4932", "3.64519953188247460253E-4951"),
        FloatKind::Quad => (
            "1.18973149535723176508575932662800702E+4932",
            "6.47517511943802511092443895822764655E-4966",
        ),
    }
}

/// The exact value of `lit` rounded to `kind`.
fn round_literal(kind: FloatKind, lit: &FloatLiteral<'_>) -> soft::Rounded {
    let zero = soft::Rounded { bits: 0, overflow: false, inexact: false, underflow_to_zero: false };
    let frac_len = i64::try_from(lit.frac_digits.len()).unwrap_or(i64::MAX);
    if lit.radix == Radix::Hex {
        let mut all = String::with_capacity(lit.int_digits.len() + lit.frac_digits.len());
        all.push_str(lit.int_digits.trim_start_matches('0'));
        all.push_str(&lit.frac_digits);
        let all = all.trim_end_matches('0');
        let stripped =
            (lit.int_digits.trim_start_matches('0').len() + lit.frac_digits.len()) - all.len();
        let num = Big::from_hex(all);
        if num.is_zero() {
            return zero;
        }
        // value = all * 16^(stripped - frac_len) * 2^exponent
        let scale =
            lit.exponent.saturating_add(4 * (i64::try_from(stripped).unwrap_or(0) - frac_len));
        return soft::round_rational(kind, false, &num, &Big::from_u128(1), scale);
    }
    let int = lit.int_digits.trim_start_matches('0');
    let mut all = String::with_capacity(int.len() + lit.frac_digits.len());
    all.push_str(int);
    all.push_str(&lit.frac_digits);
    let lead = all.len() - all.trim_start_matches('0').len();
    let body = all[lead..].trim_end_matches('0');
    if body.is_empty() {
        return zero;
    }
    let stripped = i64::try_from(all.len() - lead - body.len()).unwrap_or(0);
    // value = body * 10^e10
    let e10 = lit.exponent.saturating_add(stripped).saturating_sub(frac_len);
    let digits = i64::try_from(body.len()).unwrap_or(i64::MAX);
    // Magnitude in [10^(digits+e10-1), 10^(digits+e10)): far outside
    // every format's range, short-circuit.
    let mag = digits.saturating_add(e10);
    if mag > 5000 {
        return soft::Rounded {
            bits: soft::infinity(kind, false),
            overflow: true,
            inexact: true,
            underflow_to_zero: false,
        };
    }
    if mag < -5000 {
        return soft::Rounded { bits: 0, overflow: false, inexact: true, underflow_to_zero: true };
    }
    let mut num = Big::from_decimal(body);
    if e10 >= 0 {
        #[allow(clippy::cast_sign_loss, reason = "checked non-negative")]
        num.mul_pow10(e10 as u64);
        soft::round_rational(kind, false, &num, &Big::from_u128(1), 0)
    } else {
        let mut den = Big::from_u128(1);
        den.mul_pow10(e10.unsigned_abs());
        soft::round_rational(kind, false, &num, &den, 0)
    }
}

/// Types a floating literal ([lex.fcon]/2): `double` unsuffixed,
/// `float` for `f`, `long double` for `l`, `_Float16` for `f16`; the
/// value is the literal's exact value rounded to nearest.
///
/// A value that overflows to infinity or underflows to zero comes back
/// with clang's `-Wliteral-range` warning.
///
/// # Errors
/// `sema_invalid_float_suffix` for the suffixes of the extended types
/// the target does not provide here (`f32`, `f64`, `f128`, `bf16`).
pub fn classify_float_literal(
    s: &Sema<'_, '_>,
    lit: &FloatLiteral<'_>,
) -> Result<FloatLiteralTyping, LiteralError> {
    let ty = match lit.suffix {
        FloatSuffix::None => QualType::DOUBLE,
        FloatSuffix::F => QualType::FLOAT,
        FloatSuffix::L => QualType::LONGDOUBLE,
        FloatSuffix::F16 => QualType::FLOAT16,
        FloatSuffix::F32 | FloatSuffix::F64 | FloatSuffix::F128 | FloatSuffix::BF16 => {
            let spelled = match lit.suffix {
                FloatSuffix::F32 => "f32",
                FloatSuffix::F64 => "f64",
                FloatSuffix::F128 => "f128",
                _ => "bf16",
            };
            return Err(LiteralError::new(DiagId::sema_invalid_float_suffix, vec![spelled.into()]));
        }
    };
    let b = s.ast.types.builtin(ty).unwrap_or(BuiltinType::Double);
    let kind = float_kind(s, b).unwrap_or(FloatKind::Double);
    let r = round_literal(kind, lit);
    let (max, min) = range_strings(kind);
    let warning = if r.overflow {
        Some(LiteralError::new(DiagId::sema_float_literal_overflow, vec![s.ty_arg(ty), max.into()]))
    } else if r.underflow_to_zero {
        Some(LiteralError::new(
            DiagId::sema_float_literal_underflow,
            vec![s.ty_arg(ty), min.into()],
        ))
    } else {
        None
    };
    Ok(FloatLiteralTyping { ty, bits: FloatBits { kind, bits: r.bits }, warning })
}

/// Types a character literal ([lex.ccon]): `char` for an ordinary
/// single-character literal, `int` for a multicharacter one, else the
/// encoding's character type; the value as the literal's bit pattern.
///
/// # Errors
/// Never; kept fallible for the parser's interface.
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
/// including the terminating NUL ([lex.string]/10): `const charT[units]`.
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
///
/// # Panics
/// If the constant store is exhausted (as for every interned constant).
pub fn float_const(s: &mut Sema<'_, '_>, ty: QualType, bits: FloatBits) -> ConstValueId {
    s.ast
        .consts
        .push_scalar(Scalar::Float { ty, kind: bits.kind, bits: bits.bits })
        .expect("float constants intern")
}
