//! pp-number classification ([lex.icon], [lex.fcon]), following clang's
//! `NumericLiteralParser` so diagnostics match.

use std::borrow::Cow;

use super::{Diags, LitDiag};
use crate::diag::LexDiagKind;
use crate::options::Std;

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
    /// `f32` / `F32` (C++23): `std::float32_t`.
    F32,
    /// `f64` / `F64` (C++23): `std::float64_t`.
    F64,
    /// `f128` / `F128` (C++23): `std::float128_t`.
    F128,
    /// `bf16` / `BF16` (C++23): `std::bfloat16_t`.
    BF16,
}

/// An integer literal.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct IntegerLiteral<'s> {
    /// The base.
    pub radix: Radix,
    /// The digits, without base prefix and digit separators (the leading
    /// `0` of an octal literal is included).
    pub digits: Cow<'s, str>,
    /// The integer-suffix (all `false`/`None` with a ud-suffix).
    pub suffix: IntSuffix,
    /// The ud-suffix as spelled, if any.
    pub ud_suffix: Option<&'s str>,
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
fn is_valid_ud_suffix(s: &str, std: Std) -> bool {
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
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one pass mirroring clang's parser"
)]
pub fn classify_number(spelling: &str, std: Std) -> Number<'_> {
    let mut p = Parser { s: spelling, b: spelling.as_bytes(), std, diags: Diags::default() };
    let b = p.b;
    let invalid = |p: Parser<'_>| {
        let has_error = p.diags.error;
        Number { class: NumberClass::Invalid, diags: p.diags.list, has_error }
    };
    let mut radix = Radix::Decimal;
    let mut saw_period = false;
    let mut saw_exponent = false;
    let mut int_range = 0..0;
    let mut frac_range = 0..0;
    let mut exponent = 0i64;
    let mut i;
    if b.first() == Some(&b'0')
        && matches!(b.get(1), Some(b'x' | b'X'))
        && b.get(2).is_some_and(|&c| c.is_ascii_hexdigit() || c == b'.')
    {
        radix = Radix::Hex;
        let start = 2;
        i = p.skip(start, 16);
        int_range = start..i;
        let mut has_sig = p.has_digits(start, i);
        if b.get(i) == Some(&b'.') {
            i += 1;
            saw_period = true;
            let fstart = i;
            i = p.skip(i, 16);
            frac_range = fstart..i;
            if p.has_digits(fstart, i) {
                has_sig = true;
                if !p.check_sep(fstart, false) {
                    return invalid(p);
                }
            }
        }
        if !has_sig {
            p.diags.push(LexDiagKind::HexFloatNoSignificand, 0, i);
            return invalid(p);
        }
        if matches!(b.get(i), Some(b'p' | b'P')) {
            if !p.check_sep(i, true) {
                return invalid(p);
            }
            saw_exponent = true;
            let Some((e, end)) = p.exponent(i, 2) else {
                return invalid(p);
            };
            exponent = e;
            if std < Std::Cxx17 {
                p.diags.push(LexDiagKind::HexFloatExtension, 0, end);
            }
            i = end;
        } else if saw_period {
            p.diags.push(LexDiagKind::HexFloatNoExponent, 0, i);
            return invalid(p);
        }
    } else if b.first() == Some(&b'0')
        && matches!(b.get(1), Some(b'b' | b'B'))
        && matches!(b.get(2), Some(b'0' | b'1'))
    {
        radix = Radix::Binary;
        if std < Std::Cxx14 {
            p.diags.push(LexDiagKind::BinaryLiteralExtension, 0, 2);
        }
        i = p.skip(2, 2);
        int_range = 2..i;
        if p.bad_digit(i, 2) {
            return invalid(p);
        }
    } else {
        let start = 0;
        if b.first() == Some(&b'0') {
            radix = Radix::Octal;
            i = p.skip(1, 8);
            if b.get(i).is_some_and(u8::is_ascii_digit) {
                let end_dec = p.skip(i, 10);
                if matches!(b.get(end_dec), Some(b'.' | b'e' | b'E')) {
                    i = end_dec;
                    radix = Radix::Decimal;
                }
            }
        } else {
            i = p.skip(0, 10);
        }
        int_range = start..i;
        let r = if radix == Radix::Octal { 8 } else { 10 };
        if p.bad_digit(i, r) {
            return invalid(p);
        }
        if b.get(i) == Some(&b'.') {
            if !p.check_sep(i, true) {
                return invalid(p);
            }
            i += 1;
            radix = Radix::Decimal;
            saw_period = true;
            if !p.check_sep(i, false) {
                return invalid(p);
            }
            let fstart = i;
            i = p.skip(i, 10);
            frac_range = fstart..i;
        }
        if matches!(b.get(i), Some(b'e' | b'E')) {
            if !p.check_sep(i, true) {
                return invalid(p);
            }
            radix = Radix::Decimal;
            saw_exponent = true;
            let Some((e, end)) = p.exponent(i, 10) else {
                return invalid(p);
            };
            exponent = e;
            i = end;
        }
    }
    let floating = saw_period || saw_exponent;
    // Suffix.
    let suffix_begin = i;
    if !p.check_sep(i, true) {
        return invalid(p);
    }
    let mut unsigned = false;
    let mut size = IntSize::None;
    let mut fsuffix = FloatSuffix::None;
    let mut has_size = false;
    let cxx23 = std >= Std::Cxx23;
    while let Some(&c) = b.get(i) {
        let rest = &b[i..];
        let ok = match c {
            b'f' | b'F' if floating && !has_size => {
                has_size = true;
                let (s, n) = match &rest[1..] {
                    [b'1', b'6', ..] => (FloatSuffix::F16, 3),
                    [b'3', b'2', ..] if cxx23 => (FloatSuffix::F32, 3),
                    [b'6', b'4', ..] if cxx23 => (FloatSuffix::F64, 3),
                    [b'1', b'2', b'8', ..] if cxx23 => (FloatSuffix::F128, 4),
                    _ => (FloatSuffix::F, 1),
                };
                fsuffix = s;
                i += n;
                true
            }
            b'b' | b'B' if floating && !has_size && cxx23 => {
                let upper = c == b'B';
                let want: &[u8] = if upper { b"BF16" } else { b"bf16" };
                if rest.starts_with(want) {
                    has_size = true;
                    fsuffix = FloatSuffix::BF16;
                    i += 4;
                    true
                } else {
                    false
                }
            }
            b'u' | b'U' if !floating && !unsigned => {
                unsigned = true;
                i += 1;
                true
            }
            b'l' | b'L' if !has_size => {
                has_size = true;
                if rest.get(1) == Some(&c) {
                    if floating {
                        false
                    } else {
                        size = IntSize::LongLong;
                        i += 2;
                        true
                    }
                } else {
                    if floating {
                        fsuffix = FloatSuffix::L;
                    } else {
                        size = IntSize::Long;
                    }
                    i += 1;
                    true
                }
            }
            b'z' | b'Z' if !floating && !has_size => {
                has_size = true;
                size = IntSize::Size;
                i += 1;
                true
            }
            _ => false,
        };
        if !ok {
            break;
        }
    }
    let mut ud_suffix = None;
    if i < b.len() {
        let suffix = &spelling[suffix_begin..];
        if is_valid_ud_suffix(suffix, std) {
            ud_suffix = Some(suffix);
            unsigned = false;
            size = IntSize::None;
            fsuffix = FloatSuffix::None;
        } else {
            p.diags.push(LexDiagKind::InvalidSuffix { floating }, suffix_begin, b.len());
            return invalid(p);
        }
    } else if size == IntSize::Size && std < Std::Cxx23 {
        p.diags.push(LexDiagKind::SizeSuffixExtension, suffix_begin, b.len());
    }
    let class = if floating {
        NumberClass::Floating(FloatLiteral {
            radix,
            int_digits: digits(&spelling[int_range]),
            frac_digits: digits(&spelling[frac_range]),
            exponent,
            suffix: fsuffix,
            ud_suffix,
        })
    } else {
        NumberClass::Integer(IntegerLiteral {
            radix,
            digits: digits(&spelling[int_range]),
            suffix: IntSuffix { unsigned, size },
            ud_suffix,
        })
    };
    let has_error = p.diags.error;
    if has_error {
        return invalid(p);
    }
    Number { class, diags: p.diags.list, has_error }
}
