//! pp-number classification ([lex.icon], [lex.fcon]), following clang's
//! `NumericLiteralParser` so diagnostics match.

use std::borrow::Cow;
use std::ops::Range;

use super::{Diags, LitDiag};
use crate::diag::LexDiagKind;
use crate::options::{CStd, Std};

/// The base of a numeric literal.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Radix {
    /// `0b…`.
    Binary,
    /// `0…` (including a lone `0`).
    Octal,
    /// No prefix.
    Decimal,
    /// `0x…`.
    Hex,
}

impl Radix {
    /// 2, 8, 10 or 16.
    pub const fn value(self) -> u32 {
        match self {
            Radix::Binary => 2,
            Radix::Octal => 8,
            Radix::Decimal => 10,
            Radix::Hex => 16,
        }
    }
}

/// The size part of an integer-suffix.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum IntSize {
    /// No size suffix.
    #[default]
    None,
    /// `l` / `L`.
    Long,
    /// `ll` / `LL`.
    LongLong,
    /// `z` / `Z` (C++23).
    Size,
    /// `wb` / `WB` (C23 6.4.4.1, a C23 extension in older C; C only, as
    /// clang): the type is the narrowest `_BitInt` holding the value.
    BitInt,
}

/// An integer-suffix ([lex.icon]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct IntSuffix {
    /// `u` / `U`.
    pub unsigned: bool,
    /// The size suffix.
    pub size: IntSize,
}

/// A floating-point-suffix ([lex.fcon]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum FloatSuffix {
    /// None: `double`.
    #[default]
    None,
    /// `f` / `F`: `float`.
    F,
    /// `l` / `L`: `long double`.
    L,
    /// `f16` / `F16`: `std::float16_t`.
    F16,
    /// `f32` / `F32` (C++23; earlier as an extension): `std::float32_t`.
    F32,
    /// `f64` / `F64` (C++23; earlier as an extension): `std::float64_t`.
    F64,
    /// `f128` / `F128` (C++23; earlier as an extension): `std::float128_t`.
    F128,
    /// `bf16` / `BF16` (C++23; earlier as an extension): `std::bfloat16_t`.
    BF16,
    /// `f32x` / `F32x` (GCC extension, lower-case `x` only): `_Float32x`.
    F32x,
    /// `f64x` / `F64x` (GCC extension, lower-case `x` only): `_Float64x`.
    F64x,
    /// `q` / `Q` (GNU extension): `__float128`. libstdc++'s `<numbers>`
    /// spells `__float128` constants this way whenever `__FLOAT128__` is
    /// predefined (clang `NumericLiteralParser::isFloat128`).
    Q,
}

/// An integer literal.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct IntegerLiteral<'s> {
    /// The base.
    pub radix: Radix,
    /// The digits, without base prefix and digit separators (the leading
    /// `0` of an octal literal is included).
    pub digits: Cow<'s, str>,
    /// The integer-suffix (all `false`/`None` with a ud-suffix, unless
    /// `imaginary`).
    pub suffix: IntSuffix,
    /// The ud-suffix as spelled, if any.
    pub ud_suffix: Option<&'s str>,
    /// A GNU `i`/`j` imaginary suffix was read (clang
    /// `NumericLiteralParser::isImaginary`): with a ud-suffix (`1i`,
    /// `1il`), sema falls back to the GNU imaginary literal when literal
    /// operator lookup finds nothing.
    pub imaginary: bool,
}

impl IntegerLiteral<'_> {
    /// The value, or `None` if it does not fit 128 bits (record
    /// [`LexDiagKind::IntegerTooLarge`]; whether it fits a type with the
    /// suffix is sema's / the preprocessor's question).
    pub fn value(&self) -> Option<u128> {
        let radix = self.radix.value();
        self.digits.chars().try_fold(0u128, |v, c| {
            v.checked_mul(u128::from(radix))?.checked_add(u128::from(c.to_digit(radix)?))
        })
    }

    /// The value's low 128 bits, and whether it is wider (clang's
    /// truncated recovery value for a too-wide `wb` literal).
    pub fn wrapping_value(&self) -> (u128, bool) {
        let radix = self.radix.value();
        let mut wide = false;
        let mut v = 0u128;
        for d in self.digits.chars().filter_map(|c| c.to_digit(radix)) {
            let (m, o1) = v.overflowing_mul(u128::from(radix));
            let (a, o2) = m.overflowing_add(u128::from(d));
            wide |= o1 || o2;
            v = a;
        }
        (v, wide)
    }
}

/// A floating literal.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FloatLiteral<'s> {
    /// [`Radix::Decimal`] or [`Radix::Hex`].
    pub radix: Radix,
    /// Digits before the `.` (separators removed; may be empty).
    pub int_digits: Cow<'s, str>,
    /// Digits after the `.` (separators removed; may be empty).
    pub frac_digits: Cow<'s, str>,
    /// The exponent value (decimal for `e`, binary for `p`; 0 if absent),
    /// saturated to `±i64::MAX`.
    pub exponent: i64,
    /// The floating-point-suffix.
    pub suffix: FloatSuffix,
    /// The ud-suffix as spelled, if any.
    pub ud_suffix: Option<&'s str>,
    /// A GNU `i`/`j` imaginary suffix was read (see
    /// [`IntegerLiteral::imaginary`]).
    pub imaginary: bool,
}

impl FloatLiteral<'_> {
    /// The value correctly rounded (to nearest, ties to even) to `f64`,
    /// ignoring the suffix. Infinity on overflow. Exact evaluation for
    /// other formats (`long double`, `std::float128_t`, …) is sema's and
    /// the oven's.
    pub fn parse_f64(&self) -> f64 {
        if self.radix == Radix::Hex {
            return match round_hex(
                &self.int_digits,
                &self.frac_digits,
                self.exponent,
                53,
                -1022,
                1023,
            ) {
                Rounded::Inf => f64::INFINITY,
                Rounded::Finite { mant, exp } => {
                    let bits = if mant >> 52 == 0 {
                        mant // subnormal or zero
                    } else {
                        ((exp + 1023) as u64) << 52 | (mant & ((1 << 52) - 1))
                    };
                    f64::from_bits(bits)
                }
            };
        }
        self.decimal_text().parse().unwrap_or(f64::NAN)
    }

    /// The value correctly rounded to `f32`, ignoring the suffix.
    pub fn parse_f32(&self) -> f32 {
        if self.radix == Radix::Hex {
            return match round_hex(
                &self.int_digits,
                &self.frac_digits,
                self.exponent,
                24,
                -126,
                127,
            ) {
                Rounded::Inf => f32::INFINITY,
                Rounded::Finite { mant, exp } => {
                    let bits = if mant >> 23 == 0 {
                        mant as u32
                    } else {
                        ((exp + 127) as u32) << 23 | (mant as u32 & ((1 << 23) - 1))
                    };
                    f32::from_bits(bits)
                }
            };
        }
        self.decimal_text().parse().unwrap_or(f32::NAN)
    }

    fn decimal_text(&self) -> String {
        let int = if self.int_digits.is_empty() { "0" } else { &self.int_digits };
        format!("{int}.{}e{}", self.frac_digits, self.exponent)
    }
}

enum Rounded {
    Inf,
    /// `mant * 2^(exp - (bits - 1))`; `mant < 2^(bits-1)` means subnormal
    /// (then `exp` is the minimum exponent) or zero.
    Finite {
        mant: u64,
        exp: i64,
    },
}

/// Rounds `0x<int>.<frac>p<exp>` to a binary format with `bits`
/// significand bits and exponent range `emin..=emax`.
fn round_hex(int: &str, frac: &str, exp: i64, bits: u32, emin: i64, emax: i64) -> Rounded {
    // Collect up to 64 significant bits and a sticky bit.
    let mut m: u64 = 0;
    let mut e2: i64 = exp;
    let mut sticky = false;
    let mut started = false;
    for (i, c) in int.chars().chain(frac.chars()).enumerate() {
        let d = u64::from(c.to_digit(16).unwrap_or(0));
        let in_frac = i >= int.len();
        if !started && d == 0 {
            if in_frac {
                e2 = e2.saturating_sub(4);
            }
            continue;
        }
        started = true;
        if m >> 60 == 0 {
            m = m << 4 | d;
            if in_frac {
                e2 = e2.saturating_sub(4);
            }
        } else {
            sticky |= d != 0;
            if !in_frac {
                e2 = e2.saturating_add(4);
            }
        }
    }
    if m == 0 {
        return Rounded::Finite { mant: 0, exp: emin };
    }
    // Normalize so that bit 63 is set: value = m * 2^e2.
    let lz = m.leading_zeros();
    m <<= lz;
    e2 = e2.saturating_sub(i64::from(lz));
    // Unbiased exponent of the leading bit.
    let lead = e2.saturating_add(63);
    if lead > emax {
        return Rounded::Inf;
    }
    // Number of significand bits kept: `bits`, fewer when subnormal.
    let keep = if lead >= emin { i64::from(bits) } else { i64::from(bits) - (emin - lead) };
    if keep < 0 {
        return Rounded::Finite { mant: 0, exp: emin };
    }
    let drop = 64 - keep; // 64 - bits ..= 64
    let (mut mant, half, rest) = if drop >= 64 {
        (0u64, m >> 63 == 1, m << 1 != 0 || sticky)
    } else {
        let mant = m >> drop;
        let half = (m >> (drop - 1)) & 1 == 1;
        let rest = m & ((1u64 << (drop - 1)) - 1) != 0 || sticky;
        (mant, half, rest)
    };
    if half && (rest || mant & 1 == 1) {
        mant += 1;
    }
    let mut exp = lead.max(emin);
    if mant >> bits != 0 {
        // Carried into a new leading bit.
        mant >>= 1;
        exp += 1;
        if exp > emax {
            return Rounded::Inf;
        }
    } else if lead < emin && mant >> (bits - 1) != 0 {
        // A subnormal rounded up to the smallest normal.
        exp = emin;
    }
    Rounded::Finite { mant, exp }
}

/// What a pp-number is.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum NumberClass<'s> {
    /// An integer literal.
    Integer(IntegerLiteral<'s>),
    /// A floating literal.
    Floating(FloatLiteral<'s>),
    /// Neither: the diagnostics say why.
    Invalid,
}

/// A classified pp-number.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Number<'s> {
    /// The classification.
    pub class: NumberClass<'s>,
    /// Problems found, with ranges into the spelling.
    pub diags: Vec<LitDiag>,
    /// Whether any diagnostic is an error (then `class` is `Invalid`).
    pub has_error: bool,
}

fn is_digit_of(b: u8, radix: u32) -> bool {
    char::from(b).is_digit(radix)
}

/// clang's `isValidUDSuffix`: `_` not followed by `_`, or (C++14) a
/// standard library suffix.
pub fn is_valid_ud_suffix(s: &str, std: Std) -> bool {
    if std < Std::Cxx11 || s.is_empty() {
        return false;
    }
    if s.starts_with('_') && !s.starts_with("__") {
        return true;
    }
    if std < Std::Cxx14 {
        return false;
    }
    matches!(s, "h" | "min" | "s" | "ms" | "us" | "ns" | "il" | "i" | "if")
        || (std >= Std::Cxx20 && matches!(s, "d" | "y"))
}

/// Strips digit separators.
fn digits(s: &str) -> Cow<'_, str> {
    if s.contains('\'') { Cow::Owned(s.replace('\'', "")) } else { Cow::Borrowed(s) }
}

struct Parser<'s> {
    s: &'s str,
    b: &'s [u8],
    std: Std,
    /// The C standard, `None` for C++.
    c: Option<CStd>,
    diags: Diags,
}

impl<'s> Parser<'s> {
    fn skip(&self, mut i: usize, radix: u32) -> usize {
        while let Some(&c) = self.b.get(i) {
            if c == b'\'' || is_digit_of(c, radix) {
                i += 1;
            } else {
                break;
            }
        }
        i
    }

    fn has_digits(&self, from: usize, to: usize) -> bool {
        self.b[from..to].iter().any(|&c| c != b'\'')
    }

    /// clang's `checkSeparator`: `at_end` checks the byte before `i`,
    /// otherwise the byte at `i`. Returns `false` after an error.
    fn check_sep(&mut self, i: usize, at_end: bool) -> bool {
        let pos = if at_end { i.checked_sub(1) } else { Some(i) };
        match pos {
            Some(p) if self.b.get(p) == Some(&b'\'') => {
                self.diags.push(LexDiagKind::DigitSeparatorPosition { at_end }, p, p + 1);
                false
            }
            _ => true,
        }
    }

    fn rest(&self, i: usize) -> &'s str {
        self.s.get(i..).unwrap_or("")
    }

    /// A hex digit where the base does not allow it (`e`/`E` start an
    /// exponent except in binary): `err_invalid_digit`, unless the rest
    /// is a valid ud-suffix.
    fn bad_digit(&mut self, i: usize, radix: u8) -> bool {
        match self.b.get(i) {
            Some(&c)
                if c.is_ascii_hexdigit()
                    && (radix == 2 || !matches!(c, b'e' | b'E'))
                    && !is_valid_ud_suffix(self.rest(i), self.std) =>
            {
                self.diags.push(LexDiagKind::InvalidDigit { ch: char::from(c), radix }, i, i + 1);
                true
            }
            _ => false,
        }
    }

    /// The `e` exponent at `i` (which is `e`/`E`); returns its value and
    /// the end, or `None` after an error.
    fn exponent(&mut self, i: usize, radix: u32) -> Option<(i64, usize)> {
        let mut j = i + 1;
        let neg = match self.b.get(j) {
            Some(b'+') => {
                j += 1;
                false
            }
            Some(b'-') => {
                j += 1;
                true
            }
            _ => false,
        };
        let end = self.skip(j, 10);
        if !self.has_digits(j, end) {
            self.diags.push(LexDiagKind::ExponentNoDigits, i, i + 1);
            return None;
        }
        let _ = radix;
        if !self.check_sep(j, false) || !self.check_sep(end, true) {
            return None;
        }
        let mut v: i64 = 0;
        for &c in &self.b[j..end] {
            if c != b'\'' {
                v = v.saturating_mul(10).saturating_add(i64::from(c - b'0'));
            }
        }
        Some((if neg { -v } else { v }, end))
    }
}

/// Classifies a pp-number spelling ([lex.ppnumber]) as an integer or
/// floating literal ([lex.icon], [lex.fcon]) with its suffix, matching
/// clang's diagnostics. Digit separators are removed from the digits.
///
/// Deviations: the C++23 extended floating suffixes `f32`, `f64`, `f128`,
/// `bf16` (and upper-case forms) are accepted from C++23 on (clang 19
/// rejects them); `f16` in every mode (clang: targets with `_Float16`);
/// GNU `q`/`w` and MS `i64` suffixes, fixed-point and `_BitInt` suffixes
/// are not supported. A ud-suffix's universal-character-names are not
/// expanded.
pub fn classify_number(spelling: &str, std: Std) -> Number<'_> {
    classify_number_in(spelling, std, None)
}

/// [`classify_number`] for a language mode: in C (`c` is `Some`) the
/// `wb`/`uwb` `_BitInt` suffixes (C23 6.4.4.1; WP-1331) are integer
/// suffixes, with clang's extension warning before C23 and its
/// `-Wpre-c23-compat` warning from C23 on. C++ has no such suffix (clang
/// 19/22: "invalid suffix 'wb' on integer constant").
pub fn classify_number_in(spelling: &str, std: Std, c: Option<CStd>) -> Number<'_> {
    let mut p = Parser { s: spelling, b: spelling.as_bytes(), std, c, diags: Diags::default() };
    let b = p.b;
    let invalid = |p: Parser<'_>| {
        let has_error = p.diags.error;
        Number { class: NumberClass::Invalid, diags: p.diags.list, has_error }
    };
    let body = if b.first() == Some(&b'0')
        && matches!(b.get(1), Some(b'x' | b'X'))
        && b.get(2).is_some_and(|&c| c.is_ascii_hexdigit() || c == b'.')
    {
        p.hex_body()
    } else if b.first() == Some(&b'0')
        && matches!(b.get(1), Some(b'b' | b'B'))
        && matches!(b.get(2), Some(b'0' | b'1'))
    {
        p.binary_body()
    } else {
        p.decimal_body()
    };
    let Some(body) = body else {
        return invalid(p);
    };
    let floating = body.saw_period || body.saw_exponent;
    // Suffix.
    let suffix_begin = body.end;
    if !p.check_sep(suffix_begin, true) {
        return invalid(p);
    }
    let mut sx = p.suffixes(suffix_begin, floating);
    let mut ud_suffix = None;
    let suffix = &spelling[suffix_begin..];
    if sx.imaginary && is_valid_ud_suffix(suffix, std) {
        // clang: `1i`, `1.0if`, `1il` (and the integer `1if`) keep the
        // parsed suffix pieces and are also ud-suffixed (lookup failure
        // falls back to the imaginary literal).
        ud_suffix = Some(suffix);
    } else if sx.end < b.len() {
        if is_valid_ud_suffix(suffix, std) {
            ud_suffix = Some(suffix);
            sx = Suffixes { end: sx.end, ..Suffixes::default() };
        } else {
            p.diags.push(LexDiagKind::InvalidSuffix { floating }, suffix_begin, b.len());
            return invalid(p);
        }
    } else if sx.size == IntSize::Size && std < Std::Cxx23 {
        p.diags.push(LexDiagKind::SizeSuffixExtension, suffix_begin, b.len());
    } else if sx.size == IntSize::BitInt {
        // clang points at the literal's start.
        let kind = if c.is_some_and(|c| c >= CStd::C23) {
            LexDiagKind::BitIntSuffixCompat
        } else {
            LexDiagKind::BitIntSuffixExtension
        };
        p.diags.push(kind, 0, b.len());
    }
    // WP-361: as GCC, the P1467 suffixes are accepted before C++23 too
    // (libstdc++'s `<limits>` and `<bits/c++config.h>` spell `0.5F32` and
    // `0.0bf16` in every mode), with a pedantic extension warning.
    if floating
        && ud_suffix.is_none()
        && std < Std::Cxx23
        && matches!(
            sx.fsuffix,
            FloatSuffix::F32 | FloatSuffix::F64 | FloatSuffix::F128 | FloatSuffix::BF16
        )
    {
        p.diags.push(LexDiagKind::FloatSuffixExtension, suffix_begin, b.len());
    }
    // WP-380: GCC's `_FloatNx` suffixes, in every mode, with GCC's
    // pedantic warning at the literal.
    if floating
        && ud_suffix.is_none()
        && matches!(sx.fsuffix, FloatSuffix::F32x | FloatSuffix::F64x)
    {
        p.diags.push(LexDiagKind::FloatSuffixGnu, suffix_begin, b.len());
    }
    let class = if floating {
        NumberClass::Floating(FloatLiteral {
            radix: body.radix,
            int_digits: digits(&spelling[body.int_range]),
            frac_digits: digits(&spelling[body.frac_range]),
            exponent: body.exponent,
            suffix: sx.fsuffix,
            ud_suffix,
            imaginary: sx.imaginary,
        })
    } else {
        NumberClass::Integer(IntegerLiteral {
            radix: body.radix,
            digits: digits(&spelling[body.int_range]),
            suffix: IntSuffix { unsigned: sx.unsigned, size: sx.size },
            ud_suffix,
            imaginary: sx.imaginary,
        })
    };
    let has_error = p.diags.error;
    if has_error {
        return invalid(p);
    }
    Number { class, diags: p.diags.list, has_error }
}

/// The digits of a pp-number before its suffix: the radix, the integer
/// and fraction digit ranges (separators included), the exponent value
/// and where the suffix begins.
#[derive(Debug)]
struct Body {
    radix: Radix,
    saw_period: bool,
    saw_exponent: bool,
    int_range: Range<usize>,
    frac_range: Range<usize>,
    exponent: i64,
    end: usize,
}

/// The standard suffix pieces of a pp-number and where they end.
#[derive(Debug, Default)]
struct Suffixes {
    unsigned: bool,
    size: IntSize,
    fsuffix: FloatSuffix,
    imaginary: bool,
    end: usize,
}

impl Parser<'_> {
    /// `0x` digits, an optional `.` fraction and the `p` exponent
    /// ([lex.fcon] hexadecimal-floating-point-literal); `None` after an
    /// error.
    fn hex_body(&mut self) -> Option<Body> {
        let b = self.b;
        let start = 2;
        let mut i = self.skip(start, 16);
        let mut body = Body {
            radix: Radix::Hex,
            saw_period: false,
            saw_exponent: false,
            int_range: start..i,
            frac_range: 0..0,
            exponent: 0,
            end: 0,
        };
        let mut has_sig = self.has_digits(start, i);
        if b.get(i) == Some(&b'.') {
            i += 1;
            body.saw_period = true;
            let fstart = i;
            i = self.skip(i, 16);
            body.frac_range = fstart..i;
            if self.has_digits(fstart, i) {
                has_sig = true;
                if !self.check_sep(fstart, false) {
                    return None;
                }
            }
        }
        if !has_sig {
            self.diags.push(LexDiagKind::HexFloatNoSignificand, 0, i);
            return None;
        }
        if matches!(b.get(i), Some(b'p' | b'P')) {
            if !self.check_sep(i, true) {
                return None;
            }
            body.saw_exponent = true;
            let (e, end) = self.exponent(i, 2)?;
            body.exponent = e;
            if self.std < Std::Cxx17 {
                self.diags.push(LexDiagKind::HexFloatExtension, 0, end);
            }
            i = end;
        } else if body.saw_period {
            self.diags.push(LexDiagKind::HexFloatNoExponent, 0, i);
            return None;
        }
        body.end = i;
        Some(body)
    }

    /// `0b` digits ([lex.icon] binary-literal); `None` after an error.
    fn binary_body(&mut self) -> Option<Body> {
        if self.std < Std::Cxx14 {
            self.diags.push(LexDiagKind::BinaryLiteralExtension, 0, 2);
        }
        let i = self.skip(2, 2);
        if self.bad_digit(i, 2) {
            return None;
        }
        Some(Body {
            radix: Radix::Binary,
            saw_period: false,
            saw_exponent: false,
            int_range: 2..i,
            frac_range: 0..0,
            exponent: 0,
            end: i,
        })
    }

    /// Octal or decimal digits, an optional `.` fraction and `e`
    /// exponent (a leading-`0` spelling with a fraction or exponent is
    /// decimal); `None` after an error.
    fn decimal_body(&mut self) -> Option<Body> {
        let b = self.b;
        let mut radix = Radix::Decimal;
        let mut i;
        if b.first() == Some(&b'0') {
            radix = Radix::Octal;
            i = self.skip(1, 8);
            if b.get(i).is_some_and(u8::is_ascii_digit) {
                let end_dec = self.skip(i, 10);
                if matches!(b.get(end_dec), Some(b'.' | b'e' | b'E')) {
                    i = end_dec;
                    radix = Radix::Decimal;
                }
            }
        } else {
            i = self.skip(0, 10);
        }
        let mut body = Body {
            radix,
            saw_period: false,
            saw_exponent: false,
            int_range: 0..i,
            frac_range: 0..0,
            exponent: 0,
            end: 0,
        };
        let r = if radix == Radix::Octal { 8 } else { 10 };
        if self.bad_digit(i, r) {
            return None;
        }
        if b.get(i) == Some(&b'.') {
            if !self.check_sep(i, true) {
                return None;
            }
            i += 1;
            body.radix = Radix::Decimal;
            body.saw_period = true;
            if !self.check_sep(i, false) {
                return None;
            }
            let fstart = i;
            i = self.skip(i, 10);
            body.frac_range = fstart..i;
        }
        if matches!(b.get(i), Some(b'e' | b'E')) {
            if !self.check_sep(i, true) {
                return None;
            }
            body.radix = Radix::Decimal;
            body.saw_exponent = true;
            let (e, end) = self.exponent(i, 10)?;
            body.exponent = e;
            i = end;
        }
        body.end = i;
        Some(body)
    }

    /// The standard suffix pieces from `i` (clang's `NumericLiteralParser`
    /// suffix loop): `u`, `l`/`ll`, `z`, `f`/`f16`/`f32`/`f64`/`f128`,
    /// GCC's `f32x`/`f64x`, `bf16` and the GNU imaginary `i`/`j`, each at
    /// most once.
    fn suffixes(&self, mut i: usize, floating: bool) -> Suffixes {
        let b = self.b;
        let mut sx = Suffixes::default();
        let mut has_size = false;
        while let Some(&c) = b.get(i) {
            let rest = &b[i..];
            let ok = match c {
                b'f' | b'F' if floating && !has_size => {
                    has_size = true;
                    let (s, n) = match &rest[1..] {
                        [b'1', b'6', ..] => (FloatSuffix::F16, 3),
                        [b'3', b'2', b'x', ..] => (FloatSuffix::F32x, 4),
                        [b'6', b'4', b'x', ..] => (FloatSuffix::F64x, 4),
                        [b'3', b'2', ..] => (FloatSuffix::F32, 3),
                        [b'6', b'4', ..] => (FloatSuffix::F64, 3),
                        [b'1', b'2', b'8', ..] => (FloatSuffix::F128, 4),
                        _ => (FloatSuffix::F, 1),
                    };
                    sx.fsuffix = s;
                    i += n;
                    true
                }
                b'b' | b'B' if floating && !has_size => {
                    let upper = c == b'B';
                    let want: &[u8] = if upper { b"BF16" } else { b"bf16" };
                    if rest.starts_with(want) {
                        has_size = true;
                        sx.fsuffix = FloatSuffix::BF16;
                        i += 4;
                        true
                    } else {
                        false
                    }
                }
                b'q' | b'Q' if floating && !has_size => {
                    has_size = true;
                    sx.fsuffix = FloatSuffix::Q;
                    i += 1;
                    true
                }
                b'u' | b'U' if !floating && !sx.unsigned => {
                    sx.unsigned = true;
                    i += 1;
                    true
                }
                b'l' | b'L' if !has_size => {
                    has_size = true;
                    if rest.get(1) == Some(&c) {
                        if floating {
                            false
                        } else {
                            sx.size = IntSize::LongLong;
                            i += 2;
                            true
                        }
                    } else {
                        if floating {
                            sx.fsuffix = FloatSuffix::L;
                        } else {
                            sx.size = IntSize::Long;
                        }
                        i += 1;
                        true
                    }
                }
                // `wb` / `WB` (not `wB` / `Wb`), C only.
                b'w' | b'W'
                    if !floating
                        && !has_size
                        && self.c.is_some()
                        && rest.get(1) == Some(&if c == b'w' { b'b' } else { b'B' }) =>
                {
                    has_size = true;
                    sx.size = IntSize::BitInt;
                    i += 2;
                    true
                }
                b'z' | b'Z' if !floating && !has_size => {
                    has_size = true;
                    sx.size = IntSize::Size;
                    i += 1;
                    true
                }
                // GNU imaginary constants (once).
                b'i' | b'I' | b'j' | b'J' if !sx.imaginary => {
                    sx.imaginary = true;
                    i += 1;
                    true
                }
                _ => false,
            };
            if !ok {
                break;
            }
        }
        sx.end = i;
        sx
    }
}
