//! Escape sequences in character and string literals: [lex.ccon]/3-8,
//! [lex.string], [lex.universal.char]. Diagnostics match clang 19
//! (`-fsyntax-only -pedantic`) unless noted.
use kiln_lex::literal::{CodeUnits, LiteralOptions, decode_char, decode_string};
use kiln_lex::{LexDiagKind as D, LexSeverity, Std};

fn opts(std: Std) -> LiteralOptions {
    LiteralOptions { std, ..LiteralOptions::default() }
}

/// Diagnostic kinds of a string literal (C++26).
fn diags(sp: &str) -> Vec<D> {
    diags_in(sp, Std::Cxx26)
}

fn diags_in(sp: &str, std: Std) -> Vec<D> {
    decode_string(sp, opts(std)).diags.iter().map(|d| d.kind).collect()
}

/// The code units of a string literal (C++26).
fn units(sp: &str) -> Vec<u32> {
    decode_string(sp, LiteralOptions::default()).units.iter().collect()
}

/// [lex.ccon]/3 (Table 9): simple escape sequences.
#[test]
fn lex_ccon_simple_escapes() {
    assert_eq!(
        units(r#""\'\"\?\\\a\b\f\n\r\t\v""#),
        [0x27, 0x22, 0x3F, 0x5C, 7, 8, 0x0C, 0x0A, 0x0D, 9, 0x0B]
    );
    assert_eq!(diags(r#""\'\"\?\\\a\b\f\n\r\t\v""#), []);
}

/// [lex.ccon]/3: octal escapes take at most three digits.
#[test]
fn lex_ccon_octal_escape() {
    assert_eq!(units(r#""\0\12\1234""#), [0, 0o12, 0o123, u32::from(b'4')]);
    // `\8` is not an octal escape: unknown escape, value '8'.
    assert_eq!(units(r#""\8""#), [u32::from(b'8')]);
    assert_eq!(diags(r#""\8""#), [D::UnknownEscape { ch: '8' }]);
}

/// [lex.ccon]/3, /8: hexadecimal escapes take every following hex digit;
/// a value that does not fit the code unit is ill-formed.
#[test]
fn lex_ccon_hex_escape() {
    assert_eq!(units(r#""\x41\x0042g""#), [0x41, 0x42, u32::from(b'g')]);
    assert_eq!(diags(r#""\x""#), [D::EscapeNoDigits { esc: 'x' }]);
    assert_eq!(diags(r#""\x100""#), [D::HexEscapeTooLarge]);
    assert_eq!(diags(r#""\400""#), [D::OctalEscapeTooLarge]);
    assert_eq!(diags(r#"u"\xFFFF""#), []);
    assert_eq!(diags(r#"u"\x10000""#), [D::HexEscapeTooLarge]);
    assert_eq!(diags(r#"U"\xFFFFFFFF""#), []);
    assert_eq!(diags(r#"U"\x100000000""#), [D::HexEscapeTooLarge]);
    // u8 numeric escapes give one code unit each, not UTF-8.
    assert_eq!(units(r#"u8"\xff""#), [0xFF]);
}

/// [lex.ccon]/3 (C++23): delimited `\o{}`, `\x{}`, `\u{}` escapes; an
/// extension before C++23.
#[test]
fn lex_ccon_delimited_escapes() {
    assert_eq!(units(r#""\o{101}\x{42}\u{43}""#), [0x41, 0x42, 0x43]);
    assert_eq!(diags(r#""\o{101}\x{42}\u{43}""#), []);
    assert_eq!(
        diags_in(r#""\o{101}\x{42}\u{43}""#, Std::Cxx20),
        [D::DelimitedEscapeExtension, D::DelimitedEscapeExtension, D::UcnDelimitedExtension]
    );
    assert_eq!(diags(r#""\x{}""#), [D::DelimitedEscapeEmpty]);
    assert_eq!(diags(r#""\u{}""#), [D::DelimitedEscapeEmpty]);
    assert_eq!(diags(r#""\x{41""#), [D::DelimitedEscapeUnterminated]);
    assert_eq!(diags(r#""\o{9}""#), [D::DelimitedEscapeInvalidDigit { ch: '9' }]);
    assert_eq!(diags(r#""\x{ 41}""#), [D::DelimitedEscapeInvalidDigit { ch: ' ' }]);
    assert_eq!(diags(r#""\o""#), [D::EscapeMissingBrace { esc: 'o' }]);
    assert_eq!(diags(r#""\x{100}""#), [D::HexEscapeTooLarge]);
    assert_eq!(diags(r#""\x{100000000}""#), [D::HexEscapeTooLarge]);
}

/// [lex.ccon]/3: an unknown escape is conditionally-supported; clang
/// warns and uses the character. GNU `\e` and `(`, `{`, `[`, `%` are
/// extensions.
#[test]
fn lex_ccon_unknown_and_nonstandard_escapes() {
    assert_eq!(units(r#""\q\e\E\(""#), [u32::from(b'q'), 0x1B, 0x1B, u32::from(b'(')]);
    assert_eq!(
        diags(r#""\q\e\E\(""#),
        [
            D::UnknownEscape { ch: 'q' },
            D::NonstandardEscape { ch: 'e' },
            D::NonstandardEscape { ch: 'E' },
            D::NonstandardEscape { ch: '(' }
        ]
    );
    assert_eq!(D::UnknownEscape { ch: 'q' }.severity(), LexSeverity::Warning);
}

/// [lex.universal.char]/2-4: universal-character-names in literals.
#[test]
fn lex_universal_char_in_literals() {
    assert_eq!(
        units(r#""\u00e9\U0001F600\u{1F600}""#),
        [0xC3, 0xA9, 0xF0, 0x9F, 0x98, 0x80, 0xF0, 0x9F, 0x98, 0x80]
    );
    assert_eq!(diags(r#""\u12""#), [D::UcnEscapeIncomplete]);
    assert_eq!(diags(r#""\u""#), [D::EscapeNoDigits { esc: 'u' }]);
    assert_eq!(diags(r#""\uD800""#), [D::UcnInvalidCodePoint { cp: 0xD800 }]);
    assert_eq!(diags(r#""\U00110000""#), [D::UcnInvalidCodePoint { cp: 0x11_0000 }]);
    assert!(matches!(diags(r#""\u{110000}""#)[..], [D::UcnInvalidCodePoint { .. }]));
    // C++11 and later: control and basic characters are allowed in literals.
    assert_eq!(units(r#""\u0000\u0041""#), [0, 0x41]);
    assert_eq!(diags(r#""\u0000\u0041""#), []);
}

/// C++03 [lex.charset]/2: a UCN may not name a control or basic source
/// character, even in a literal (`$`, `@`, `` ` `` excepted, as clang).
#[test]
fn lex_charset_cxx03_ucn_restrictions() {
    assert_eq!(diags_in(r#""\u0041""#, Std::Cxx03), [D::UcnBasicOrControl { cp: 0x41 }]);
    assert_eq!(diags_in(r#""\u0007""#, Std::Cxx03), [D::UcnBasicOrControl { cp: 7 }]);
    assert_eq!(diags_in(r#""\u0024\u00e9""#, Std::Cxx03), []);
}

/// [lex.universal.char]/3 (C++23): named universal characters.
#[test]
fn lex_universal_char_named_in_literals() {
    assert_eq!(
        units(r#""\N{LATIN CAPITAL LETTER A}\N{GRINNING FACE}""#),
        [0x41, 0xF0, 0x9F, 0x98, 0x80]
    );
    assert_eq!(diags_in(r#""\N{LATIN CAPITAL LETTER A}""#, Std::Cxx20), [D::UcnNamedExtension]);
    assert_eq!(diags(r#""\N{ABC}""#), [D::UcnUnknownName]);
    assert_eq!(diags(r#""\N{ABC""#), [D::UcnEscapeIncomplete]);
    assert_eq!(diags(r#""\N{}""#), [D::DelimitedEscapeEmpty]);
    assert_eq!(diags(r#""\N""#), [D::EscapeMissingBrace { esc: 'N' }]);
    // Loose matching recovers with the character but is an error.
    let s = decode_string(r#""\N{latin small letter a}""#, LiteralOptions::default());
    assert_eq!(s.units, CodeUnits::U8(vec![0x61]));
    assert_eq!(s.diags[0].kind, D::UcnNameLooseMatch { ch: 'a' });
    assert!(s.has_error);
}

/// [lex.ccon]/3: escapes in character literals; diagnostics carry byte
/// ranges into the spelling.
#[test]
fn lex_ccon_escape_ranges() {
    let c = decode_char(r"'\x100'", LiteralOptions::default());
    assert_eq!(c.diags.len(), 1);
    assert_eq!(c.diags[0].kind, D::HexEscapeTooLarge);
    assert_eq!(c.diags[0].range, 1..6);
    let s = decode_string(r#"u8"ab\qc""#, LiteralOptions::default());
    assert_eq!(s.diags[0].range, 5..7);
}

/// [lex.string]: raw strings have no escapes.
#[test]
fn lex_string_raw_no_escapes() {
    assert_eq!(units(r#"R"(\n\x)""#), [0x5C, u32::from(b'n'), 0x5C, u32::from(b'x')]);
    assert_eq!(diags(r#"R"(\q)""#), []);
}

/// [lex.ccon]/8 diagnostics in clang's order: once a literal has had a
/// "hard" error, later too-large `\x`/`\o{}` escapes are not diagnosed;
/// `\x` without digits, an empty `\x{}`/`\o{}` and a too-large plain octal
/// escape do not count as hard errors.
#[test]
fn lex_ccon_error_gating_matches_clang() {
    assert_eq!(diags(r#""\x100\x100""#), [D::HexEscapeTooLarge]);
    assert_eq!(diags(r#""\o{400}\o{400}""#), [D::OctalEscapeTooLarge]);
    assert_eq!(diags(r#""\400\400""#), [D::OctalEscapeTooLarge, D::OctalEscapeTooLarge]);
    assert_eq!(diags(r#""\o\x100""#), [D::EscapeMissingBrace { esc: 'o' }]);
    assert_eq!(diags(r#""\x\x100""#), [D::EscapeNoDigits { esc: 'x' }, D::HexEscapeTooLarge]);
    assert_eq!(diags(r#""\x{}\x{}""#), [D::DelimitedEscapeEmpty, D::DelimitedEscapeEmpty]);
    assert_eq!(diags(r#""\400\x100""#), [D::OctalEscapeTooLarge, D::HexEscapeTooLarge]);
    // clang leaves the `}` of an empty `\x{}` as a character of its own.
    let c = decode_char(r"'\x{}'", LiteralOptions::default());
    let kinds: Vec<D> = c.diags.iter().map(|d| d.kind).collect();
    assert_eq!(kinds, [D::DelimitedEscapeEmpty, D::MultiCharLiteral]);
    let c = decode_char(r"L'\x{}'", LiteralOptions::default());
    let kinds: Vec<D> = c.diags.iter().map(|d| d.kind).collect();
    assert_eq!(kinds, [D::DelimitedEscapeEmpty, D::MultiCharNotAllowed]);
    // A failed escape still counts as a character of a char literal.
    let c = decode_char(r"'\377\u{}'", LiteralOptions::default());
    let kinds: Vec<D> = c.diags.iter().map(|d| d.kind).collect();
    assert_eq!(kinds, [D::DelimitedEscapeEmpty, D::MultiCharLiteral]);
    assert!(c.diags.iter().all(|d| d.kind != D::CharConstantTooLong));
}
