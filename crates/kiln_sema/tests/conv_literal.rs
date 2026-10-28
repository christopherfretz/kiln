//! Literal typing: integer ([lex.icon]), floating ([lex.fcon]),
//! character ([lex.ccon]) and string ([lex.string]) literals, with
//! clang 19's diagnostics. Spellings go through `kiln_lex`'s classifiers
//! as the parser does. Each `auto x = lit;` in a comment was dumped with
//! clang (x86_64-linux-gnu, -std=c++26) for the expected type.

mod conv_common;

use conv_common::*;
use kiln_ast::QualType;
use kiln_ast::consts::FloatKind;
use kiln_diag::{DiagId, MemorySink};
use kiln_lex::literal::{LiteralOptions, NumberClass, classify_number, decode_char, decode_string};
use kiln_lex::{Encoding, Std};
use kiln_sema::Sema;
use kiln_sema::literal::{
    FloatBits, LiteralError, classify_float_literal, classify_integer_literal, float_const,
    string_literal_type, type_char_literal,
};

/// `(type, value, warning)` for an integer literal spelling.
fn int(
    s: &Sema<'_, '_>,
    sink: &MemorySink,
    sp: &str,
) -> Result<(String, u128, Option<String>), String> {
    let n = classify_number(sp, Std::Cxx26);
    let NumberClass::Integer(lit) = n.class else { panic!("{sp}: not an integer literal") };
    match classify_integer_literal(s, &lit) {
        Ok(t) => {
            let w = t.warning.map(|w| show(s, sink, &w));
            Ok((s.ty_str(t.ty), t.value, w))
        }
        Err(e) => Err(show(s, sink, &e)),
    }
}

fn show(s: &Sema<'_, '_>, sink: &MemorySink, e: &LiteralError) -> String {
    emitted(s, sink, e.diag(s, L)).join("\n")
}

/// `(type, bits, warning)` for a floating literal spelling.
fn float(
    s: &Sema<'_, '_>,
    sink: &MemorySink,
    sp: &str,
) -> Result<(String, FloatBits, Option<String>), String> {
    let n = classify_number(sp, Std::Cxx26);
    let NumberClass::Floating(lit) = n.class else { panic!("{sp}: not a floating literal") };
    match classify_float_literal(s, &lit) {
        Ok(t) => {
            let w = t.warning.map(|w| show(s, sink, &w));
            Ok((s.ty_str(t.ty), t.bits, w))
        }
        Err(e) => Err(show(s, sink, &e)),
    }
}

fn ok_int(s: &Sema<'_, '_>, sink: &MemorySink, sp: &str) -> (String, u128) {
    let (t, v, w) = int(s, sink, sp).unwrap_or_else(|e| panic!("{sp}: {e}"));
    assert_eq!(w, None, "{sp}");
    (t, v)
}

fn double(v: f64) -> FloatBits {
    FloatBits { kind: FloatKind::Double, bits: u128::from(v.to_bits()) }
}

fn single(v: f32) -> FloatBits {
    FloatBits { kind: FloatKind::Single, bits: u128::from(v.to_bits()) }
}

// ---------------------------------------------------------------------------
// [lex.icon]
// ---------------------------------------------------------------------------

/// [lex.icon]/3, table 8: unsuffixed decimal literals take the first of
/// `int`, `long`, `long long` that fits.
/// `auto i1 = 2147483647;` → `IntegerLiteral 'int' 2147483647`
/// `auto i2 = 2147483648;` → `IntegerLiteral 'long' 2147483648`
/// `auto i4 = 9223372036854775807;` → `'long'`
#[test]
fn decimal_unsuffixed_lex_icon_3() {
    with_sema(|s, sink| {
        assert_eq!(ok_int(s, sink, "0"), ("int".into(), 0));
        assert_eq!(ok_int(s, sink, "2147483647"), ("int".into(), 2_147_483_647));
        assert_eq!(ok_int(s, sink, "2147483648"), ("long".into(), 2_147_483_648));
        assert_eq!(
            ok_int(s, sink, "9223372036854775807"),
            ("long".into(), 9_223_372_036_854_775_807)
        );
        assert_eq!(ok_int(s, sink, "1'000'000"), ("int".into(), 1_000_000));
    });
}

/// [lex.icon]/3, table 8: octal, hexadecimal and binary literals also try
/// each unsigned type.
/// `auto i3 = 0x80000000;` → `'unsigned int'`; `auto i16 =
/// 0x8000000000000000;` → `'unsigned long'`; `auto i6 =
/// 0xFFFFFFFFFFFFFFFF;` → `'unsigned long'`; `017777777777` → `'int'`;
/// `020000000000` → `'unsigned int'`; `0b1` → `'int'`
#[test]
fn non_decimal_unsuffixed_lex_icon_3() {
    with_sema(|s, sink| {
        assert_eq!(ok_int(s, sink, "0x7fffffff"), ("int".into(), 0x7fff_ffff));
        assert_eq!(ok_int(s, sink, "0x80000000"), ("unsigned int".into(), 0x8000_0000));
        assert_eq!(ok_int(s, sink, "0x100000000"), ("long".into(), 0x1_0000_0000));
        assert_eq!(ok_int(s, sink, "0x8000000000000000"), ("unsigned long".into(), 1 << 63));
        assert_eq!(
            ok_int(s, sink, "0xFFFFFFFFFFFFFFFF"),
            ("unsigned long".into(), u128::from(u64::MAX))
        );
        assert_eq!(ok_int(s, sink, "017777777777"), ("int".into(), 0o17_777_777_777));
        assert_eq!(ok_int(s, sink, "020000000000"), ("unsigned int".into(), 0o20_000_000_000));
        assert_eq!(ok_int(s, sink, "0b1"), ("int".into(), 1));
        assert_eq!(
            ok_int(s, sink, "0B11111111111111111111111111111111"),
            ("unsigned int".into(), 0xffff_ffff)
        );
    });
}

/// [lex.icon]/3, table 8: suffixes.
/// `1u` → `'unsigned int'`; `4294967296u` → `'unsigned long'`; `1l` →
/// `'long'`; `1ul` → `'unsigned long'`; `1ll` → `'long long'`; `1ull` →
/// `'unsigned long long'`
#[test]
fn suffixes_lex_icon_3() {
    with_sema(|s, sink| {
        assert_eq!(ok_int(s, sink, "1u").0, "unsigned int");
        assert_eq!(ok_int(s, sink, "1U").0, "unsigned int");
        assert_eq!(ok_int(s, sink, "4294967296u").0, "unsigned long");
        assert_eq!(ok_int(s, sink, "1l").0, "long");
        assert_eq!(ok_int(s, sink, "1L").0, "long");
        assert_eq!(ok_int(s, sink, "1ul").0, "unsigned long");
        assert_eq!(ok_int(s, sink, "1LU").0, "unsigned long");
        assert_eq!(ok_int(s, sink, "1ll").0, "long long");
        assert_eq!(ok_int(s, sink, "1ull").0, "unsigned long long");
        assert_eq!(ok_int(s, sink, "0xffffffffffffffffll").0, "unsigned long long");
        assert_eq!(ok_int(s, sink, "9223372036854775807ll").0, "long long");
    });
}

/// [lex.icon]/3 (P0330): `z` gives the signed type corresponding to
/// `size_t`, `uz` gives `size_t`.
/// `1z` → `'long'`; `1uz` → `'unsigned long'`; `0x8000000000000000z` →
/// `'unsigned long'`; `18446744073709551615uz` → `'unsigned long'`
#[test]
fn size_t_suffixes_lex_icon_3() {
    with_sema(|s, sink| {
        assert_eq!(ok_int(s, sink, "1z").0, "long");
        assert_eq!(ok_int(s, sink, "1Z").0, "long");
        assert_eq!(ok_int(s, sink, "1uz").0, "unsigned long");
        assert_eq!(ok_int(s, sink, "1zu").0, "unsigned long");
        assert_eq!(ok_int(s, sink, "0x8000000000000000z").0, "unsigned long");
        assert_eq!(ok_int(s, sink, "18446744073709551615uz").0, "unsigned long");
    });
}

/// [lex.icon]/4: a decimal `z` literal out of range of the signed type.
/// `auto i20 = 9223372036854775808z;` → clang: `error: signed 'size_t'
/// literal is out of range of possible signed 'size_t' values`
#[test]
fn size_t_out_of_range_lex_icon_4() {
    with_sema(|s, sink| {
        assert_eq!(
            int(s, sink, "9223372036854775808z").unwrap_err(),
            "error: signed 'size_t' literal is out of range of possible signed 'size_t' values"
        );
        assert_eq!(
            int(s, sink, "18446744073709551616uz").unwrap_err(),
            "error: integer literal is too large to be represented in any integer type"
        );
    });
}

/// [lex.icon]/4: a decimal literal that fits no signed type: clang types it
/// `unsigned long long` and warns.
/// `auto i5 = 9223372036854775808;` → `IntegerLiteral 'unsigned long long'`
/// with `warning: integer literal is too large to be represented in a
/// signed integer type, interpreting as unsigned`
#[test]
fn implicitly_unsigned_lex_icon_4() {
    with_sema(|s, sink| {
        let (t, v, w) = int(s, sink, "9223372036854775808").unwrap();
        assert_eq!((t.as_str(), v), ("unsigned long long", 1u128 << 63));
        assert_eq!(
            w.as_deref(),
            Some(
                "warning: integer literal is too large to be represented in a signed integer type, interpreting as unsigned"
            )
        );
        // `ll`-suffixed too.
        let (t, _, w) = int(s, sink, "9223372036854775808ll").unwrap();
        assert_eq!(t, "unsigned long long");
        assert!(w.is_some());
    });
}

/// [lex.icon]/4: no integer type can represent the value.
/// `auto i7 = 18446744073709551616;` → clang: `error: integer literal is
/// too large to be represented in any integer type`
#[test]
fn too_large_lex_icon_4() {
    with_sema(|s, sink| {
        for sp in ["18446744073709551616", "0x10000000000000000", "99999999999999999999999999999u"]
        {
            let e = int(s, sink, sp).unwrap_err();
            assert_eq!(
                e, "error: integer literal is too large to be represented in any integer type",
                "{sp}"
            );
        }
        let n = classify_number("18446744073709551616", Std::Cxx26);
        let NumberClass::Integer(lit) = n.class else { panic!() };
        assert_eq!(
            classify_integer_literal(s, &lit).unwrap_err().id,
            DiagId::sema_int_literal_too_large
        );
    });
}

// ---------------------------------------------------------------------------
// [lex.fcon]
// ---------------------------------------------------------------------------

/// [lex.fcon]/3: the type follows the suffix: none → `double`, `f` →
/// `float`, `l` → `long double`, `f16` → `_Float16` (`std::float16_t`).
/// `auto f1 = 1.0;` → `FloatingLiteral 'double' 1.0`; `1.0f` → `'float'`;
/// `1.0L` → `'long double'`; `1.0f16` → `'_Float16'`
#[test]
fn suffix_types_lex_fcon_3() {
    with_sema(|s, sink| {
        let (t, b, w) = float(s, sink, "1.0").unwrap();
        assert_eq!((t.as_str(), b, w), ("double", double(1.0), None));
        let (t, b, _) = float(s, sink, "1.0f").unwrap();
        assert_eq!((t.as_str(), b), ("float", single(1.0)));
        let (t, b, _) = float(s, sink, "1.0L").unwrap();
        assert_eq!(t, "long double");
        // x87 80-bit: sign/exponent 0x3fff, explicit integer bit.
        assert_eq!(b, FloatBits { kind: FloatKind::X87Extended, bits: 0x3fff_8000_0000_0000_0000 });
        let (t, b, _) = float(s, sink, "1.0f16").unwrap();
        assert_eq!(t, "_Float16");
        assert_eq!(b, FloatBits { kind: FloatKind::Half, bits: 0x3c00 });
        let (t, _, _) = float(s, sink, "1.0F16").unwrap();
        assert_eq!(t, "_Float16");
    });
}

/// [lex.fcon]/3: on aarch64 `long double` is IEEE binary128.
#[test]
fn long_double_quad_on_aarch64_lex_fcon_3() {
    with_target(aarch64(), |s, sink| {
        let (t, b, _) = float(s, sink, "1.0L").unwrap();
        assert_eq!(t, "long double");
        assert_eq!(b, FloatBits { kind: FloatKind::Quad, bits: 0x3fff << 112 });
        let (_, b, _) = float(s, sink, "0.1L").unwrap();
        // 0.1 in binary128, rounded to nearest.
        assert_eq!(b.bits, 0x3ffb_9999_9999_9999_9999_9999_9999_999a);
    });
}

/// [lex.fcon]/3: the value is the nearest representable value
/// (round-to-nearest-even), for decimal and hexadecimal spellings.
#[test]
fn correctly_rounded_lex_fcon_3() {
    with_sema(|s, sink| {
        for (sp, v) in [
            ("0.1", 0.1),
            (".5", 0.5),
            ("1e10", 1e10),
            ("1.7976931348623157e308", f64::MAX),
            ("4.9406564584124654e-324", 5e-324),
            ("2.2250738585072014e-308", f64::MIN_POSITIVE),
            ("123456789012345678901234567890.0", 123_456_789_012_345_678_901_234_567_890.0),
            ("0x1p0", 1.0),
            ("0x1.8p1", 3.0),
            ("0x.8p0", 0.5),
            ("0x1.fffffffffffffp1023", f64::MAX),
            ("1'000.5", 1000.5),
            ("0.30000000000000004", 0.300_000_000_000_000_04),
        ] {
            let (_, b, w) = float(s, sink, sp).unwrap();
            assert_eq!(b, double(v), "{sp}");
            assert_eq!(w, None, "{sp}");
        }
        for (sp, v) in [
            ("0.1f", 0.1f32),
            ("3.4028235e38f", f32::MAX),
            ("16777217.0f", 16_777_216.0),
            ("0x1p-149f", 1e-45),
        ] {
            let (_, b, w) = float(s, sink, sp).unwrap();
            assert_eq!(b, single(v), "{sp}");
            assert_eq!(w, None, "{sp}");
        }
    });
}

/// [lex.fcon]/3: a value out of the type's range: clang warns and the
/// value is infinity or zero.
/// `auto f4 = 1e39f;` → `FloatingLiteral 'float' INF`, `warning: magnitude
/// of floating-point constant too large for type 'float'; maximum is
/// 3.40282347E+38`; `1e-50f` → `'float' 0.0`, `... too small for type
/// 'float'; minimum is 1.40129846E-45`; `1e309`, `1e-400` likewise for
/// `double`; `0x1p-150f` underflows.
#[test]
fn out_of_range_lex_fcon_3() {
    with_sema(|s, sink| {
        let (_, b, w) = float(s, sink, "1e39f").unwrap();
        assert_eq!(b, single(f32::INFINITY));
        assert_eq!(
            w.as_deref(),
            Some(
                "warning: magnitude of floating-point constant too large for type 'float'; maximum is 3.40282347E+38"
            )
        );
        let (_, b, w) = float(s, sink, "1e-50f").unwrap();
        assert_eq!(b, single(0.0));
        assert_eq!(
            w.as_deref(),
            Some(
                "warning: magnitude of floating-point constant too small for type 'float'; minimum is 1.40129846E-45"
            )
        );
        let (_, b, w) = float(s, sink, "1e309").unwrap();
        assert_eq!(b, double(f64::INFINITY));
        assert_eq!(
            w.as_deref(),
            Some(
                "warning: magnitude of floating-point constant too large for type 'double'; maximum is 1.7976931348623157E+308"
            )
        );
        let (_, b, w) = float(s, sink, "1e-400").unwrap();
        assert_eq!(b, double(0.0));
        assert_eq!(
            w.as_deref(),
            Some(
                "warning: magnitude of floating-point constant too small for type 'double'; minimum is 4.9406564584124654E-324"
            )
        );
        let (_, b, w) = float(s, sink, "0x1p-150f").unwrap();
        assert_eq!(b, single(0.0));
        assert!(w.is_some());
        // Exactly zero is not an underflow, nor a huge exponent on zero.
        for sp in ["0.0", "0e999999", "0x0p-99999"] {
            let (_, b, w) = float(s, sink, sp).unwrap();
            assert_eq!((b, w), (double(0.0), None), "{sp}");
        }
        // Huge exponents do not hang or overflow the evaluator.
        let (_, b, _) = float(s, sink, "1e99999999999").unwrap();
        assert_eq!(b, double(f64::INFINITY));
    });
}

/// [lex.fcon]: the extended-type suffixes the x86-64 target does not
/// provide (`f32`, `f64`, `f128`, `bf16` without the type support).
/// `auto f10 = 1.0bf16;` → clang: `error: invalid suffix 'bf16' on
/// floating constant`
#[test]
fn unsupported_suffixes_lex_fcon() {
    with_sema(|s, sink| {
        for (sp, suf) in
            [("1.0bf16", "bf16"), ("1.0f32", "f32"), ("1.0f64", "f64"), ("1.0f128", "f128")]
        {
            assert_eq!(
                float(s, sink, sp).unwrap_err(),
                format!("error: invalid suffix '{suf}' on floating constant"),
                "{sp}"
            );
        }
    });
}

/// [lex.fcon]: `float_const` interns the typed value for a
/// `FloatingLiteral` node.
#[test]
fn float_const_interns_lex_fcon() {
    with_sema(|s, _| {
        let c = float_const(s, QualType::DOUBLE, double(2.5));
        let lit = s.ast.builder().float_literal(QualType::DOUBLE, c, L);
        assert_eq!(s.ast.exprs.get(lit).ty(), QualType::DOUBLE);
        let c2 = float_const(s, QualType::DOUBLE, double(2.5));
        assert_eq!(c, c2);
    });
}

// ---------------------------------------------------------------------------
// [lex.ccon]
// ---------------------------------------------------------------------------

/// [lex.ccon]: `'a'` → `CharacterLiteral 'char' 97`; `u8'a'` →
/// `'char8_t'`; `u'a'` → `'char16_t'`; `U'a'` → `'char32_t'`; `L'a'` →
/// `'wchar_t'`; `'ab'` → `'int' 24930` (multicharacter, [lex.ccon]/3)
#[test]
fn character_literal_types_lex_ccon() {
    with_sema(|s, _| {
        for (sp, t, v) in [
            ("'a'", "char", 97),
            ("u8'a'", "char8_t", 97),
            ("u'a'", "char16_t", 97),
            ("U'a'", "char32_t", 97),
            ("L'a'", "wchar_t", 97),
            ("'ab'", "int", 24930),
            ("U'\\U0001F600'", "char32_t", 0x1_F600),
        ] {
            let lit = decode_char(sp, LiteralOptions::default());
            let (ty, val) = type_char_literal(s, &lit).unwrap();
            assert_eq!((s.ty_str(ty).as_str(), val), (t, v), "{sp}");
        }
        // `'\xff'` with a signed char: the value sign-extends
        // (`CharacterLiteral 'char' 4294967295`).
        let lit = decode_char("'\\xff'", LiteralOptions::default());
        assert_eq!(type_char_literal(s, &lit).unwrap(), (QualType::CHAR, 0xffff_ffff));
    });
}

// ---------------------------------------------------------------------------
// [lex.string]
// ---------------------------------------------------------------------------

/// [lex.string]/10: an ordinary string literal of n code units is
/// `const char[n+1]`, and likewise for the other encodings.
/// `auto& s2 = "ab";` → `StringLiteral 'const char[3]' lvalue "ab"`;
/// `u8"ab"` → `'const char8_t[3]'`; `u"ab"` → `'const char16_t[3]'`;
/// `U"ab"` → `'const char32_t[3]'`; `L"ab"` → `'const wchar_t[3]'`
#[test]
fn string_literal_types_lex_string_10() {
    with_sema(|s, _| {
        for (sp, enc, t) in [
            ("\"ab\"", Encoding::Ordinary, "const char[3]"),
            ("u8\"ab\"", Encoding::Utf8, "const char8_t[3]"),
            ("u\"ab\"", Encoding::Utf16, "const char16_t[3]"),
            ("U\"ab\"", Encoding::Utf32, "const char32_t[3]"),
            ("L\"ab\"", Encoding::Wide, "const wchar_t[3]"),
        ] {
            let lit = decode_string(sp, LiteralOptions::default());
            assert_eq!(lit.encoding, enc);
            let n = lit.units.len() as u64 + 1;
            let ty = string_literal_type(s, enc, n);
            assert_eq!(s.ty_str(ty), t, "{sp}");
        }
        // UTF-8 multibyte: `u8"\u00e9"` is two code units: `char8_t[3]`;
        // `u"\U0001F600"` is a surrogate pair: `char16_t[3]`.
        let lit = decode_string("u8\"\\u00e9\"", LiteralOptions::default());
        assert_eq!(lit.units.len(), 2);
        let lit = decode_string("u\"\\U0001F600\"", LiteralOptions::default());
        assert_eq!(lit.units.len(), 2);
        // The empty literal: `const char[1]`.
        let ty = string_literal_type(s, Encoding::Ordinary, 1);
        assert_eq!(s.ty_str(ty), "const char[1]");
    });
}
