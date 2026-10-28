//! Identifiers: [lex.name], [lex.universal.char], [lex.charset].
mod common;
use common::{cxx26, diag_kinds, kinds, lex, lex_with};
use kiln_lex::{LexDiagKind as D, LexOptions, TokFlags};

/// [lex.name]/1: ASCII identifiers, including `_` and digits.
#[test]
fn lex_name_ascii() {
    assert_eq!(lex("_a1 B_2 __x\n").idents, ["_a1", "B_2", "__x"]);
}

/// [lex.name]/1: `XID_Start` / `XID_Continue` in UTF-8.
#[test]
fn lex_name_utf8_xid() {
    let l = lex("été a\u{0301}\n");
    assert_eq!(l.idents, ["été", "a\u{0301}"]);
    assert_eq!(kinds("été\n"), "identifier");
}

/// [lex.name]/1: a non-XID_Start character cannot begin an identifier;
/// it is a lone ill-formed pp-token ([lex.pptoken]/2).
#[test]
fn lex_name_xid_continue_at_start() {
    let l = lex("\u{0300}x\n");
    assert_eq!(l.spellings, ["\u{0300}", "x"]);
    assert_eq!(l.diags[0].0, D::CharNotAllowedInIdentifier { ch: '\u{0300}' });
    let l = lex("\\u0300y\n");
    assert_eq!(l.spellings, ["\\u0300", "y"]);
    assert_eq!(kinds("\\u0300y\n"), "unknown identifier");
}

/// [lex.universal.char]: UCNs in identifiers name the same identifier as
/// the character itself ([lex.name]/1).
#[test]
fn lex_universal_char_in_identifier() {
    let l = lex("\\u00e9t\\U000000E9 \\u{e9} \\N{LATIN SMALL LETTER E WITH ACUTE}\n");
    assert_eq!(l.idents[0], "étÉ".replace('É', "é"));
    assert_eq!(l.idents[1], "é");
    assert_eq!(l.idents[2], "é");
    assert!(l.tokens[0].has(TokFlags::HAS_UCN));
    // Spelling keeps the UCN as written.
    assert_eq!(l.spellings[0], "\\u00e9t\\U000000E9");
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
}

/// [lex.universal.char]: `\N{}` with an algorithmic CJK name.
#[test]
fn lex_universal_char_named_cjk() {
    let l = lex("\\N{CJK UNIFIED IDEOGRAPH-4E00}\n");
    assert_eq!(l.idents, ["\u{4E00}"]);
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
}

/// [lex.universal.char]/2: surrogates and out-of-range code points.
#[test]
fn lex_universal_char_invalid_code_points() {
    assert!(diag_kinds("\\uD800\n", cxx26()).contains(&D::UcnInvalidCodePoint { cp: 0xD800 }));
    assert!(
        diag_kinds("\\U00110000\n", cxx26()).contains(&D::UcnInvalidCodePoint { cp: 0x11_0000 })
    );
}

/// [lex.universal.char]/3 (C++23 relaxed): basic-set characters via UCN
/// outside literals are ill-formed (except U+0024, U+0040 and U+0060).
#[test]
fn lex_universal_char_basic_set() {
    assert!(diag_kinds("a\\u0041\n", cxx26()).contains(&D::UcnBasicOrControl { cp: 0x41 }));
}

/// [lex.universal.char]: a backslash that does not start a UCN is a lone
/// `\` other token.
#[test]
fn lex_universal_char_not_a_ucn() {
    assert_eq!(kinds("\\ \\q\n"), "other other identifier");
    assert!(diag_kinds("a\\u12\n", cxx26()).contains(&D::UcnIncomplete));
}

/// [lex.name]/1 (extension): `$` in identifiers, diagnosed.
#[test]
fn lex_name_dollar() {
    let l = lex("$a b$\n");
    assert_eq!(l.idents, ["$a", "b$"]);
    assert!(l.diags.iter().all(|d| d.0 == D::DollarInIdentifier));
    let off = LexOptions { dollar_in_identifiers: false, ..cxx26() };
    assert_eq!(lex_with("$a\n", off).spellings, ["$", "a"]);
}

/// [lex.name]/1: identifiers must be in NFC.
#[test]
fn lex_name_not_nfc() {
    // U+0340 COMBINING GRAVE TONE MARK has NFC_QC=No.
    assert!(diag_kinds("a\u{0340}\n", cxx26()).contains(&D::IdentifierNotNfc));
}

/// [lex.name]: interning makes equal spellings equal symbols.
#[test]
fn lex_name_interned() {
    let l = lex("abc x abc\n");
    assert_eq!(l.tokens[0].kind, l.tokens[2].kind);
    assert_ne!(l.tokens[0].kind, l.tokens[1].kind);
}

/// [lex.universal.char]/3 (C++23): `\N{}` names in identifiers: exact
/// names, aliases and Hangul syllables resolve; a name that only matches
/// loosely (UAX44-LM2) is ill-formed but recovers to the character; an
/// unknown name is diagnosed. Matches clang 19 `-dump-tokens`.
#[test]
fn lex_universal_char_named_in_identifier() {
    // Alias (correction) of U+01A2 LATIN CAPITAL LETTER OI.
    let l = lex("\\N{LATIN CAPITAL LETTER GHA}\n");
    assert_eq!(l.idents, ["\u{1A2}"]);
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
    let l = lex("\\N{HANGUL SYLLABLE GAG}\n");
    assert_eq!(l.idents, ["\u{AC01}"]);
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
    let l = lex("\\N{latin_small_letter_e-with_acute}\n");
    assert_eq!(l.idents, ["\u{E9}"]);
    assert_eq!(l.diags[0].0, D::UcnNameLooseMatch { ch: '\u{E9}' });
    assert_eq!(diag_kinds("a\\N{ABC}\n", cxx26()), [D::UcnUnknownName]);
    // A basic character by name is still a basic character.
    assert_eq!(
        diag_kinds("\\N{LATIN SMALL LETTER A}\n", cxx26()),
        [D::UcnBasicOrControl { cp: 0x61 }]
    );
}

/// [lex.universal.char]: a malformed `\N` is not a UCN: `\` lexes as a
/// lone token (clang's recovery and warnings).
#[test]
fn lex_universal_char_named_malformed() {
    let l = lex("a\\N{LATIN\nb\n");
    assert_eq!(l.spellings, ["a", "\\", "N", "{", "LATIN", "b"]);
    assert_eq!(l.diags[0].0, D::UcnDelimitedIncomplete);
    let l = lex("a\\N{}\n");
    assert_eq!(l.spellings, ["a", "\\", "N", "{", "}"]);
    assert_eq!(l.diags[0].0, D::UcnDelimitedEmpty);
    assert_eq!(diag_kinds("a\\N\n", cxx26()), [D::UcnIncomplete]);
}

/// [lex.name]/1: `NFC_QC=Maybe` characters need the full NFC check: a
/// combining mark that composes with the preceding starter is not NFC,
/// one that does not compose is.
#[test]
fn lex_name_not_nfc_maybe() {
    // a + U+0301 composes to U+00E1.
    assert_eq!(diag_kinds("a\u{301}\n", cxx26()), [D::IdentifierNotNfc]);
    // x + U+0301 and U+00E9 + U+0301 have no composition.
    assert_eq!(diag_kinds("x\u{301}\n", cxx26()), []);
    assert_eq!(diag_kinds("\u{E9}\u{301}\n", cxx26()), []);
    // Via a UCN too.
    assert_eq!(diag_kinds("a\\u0301\n", cxx26()), [D::IdentifierNotNfc]);
}
