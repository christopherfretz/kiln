//! Literal *tokens* (spelling and extent only): [lex.ccon], [lex.string],
//! [lex.ext]. Literal contents are WP-011.
mod common;
use common::{cxx26, diag_kinds, kinds, kinds_with, lex, lex_with, spellings, std};
use kiln_lex::{Encoding, LexDiagKind as D, Std, TokFlags, TokKind};

/// [lex.ccon]/1, [lex.string]/1: encoding prefixes.
#[test]
fn lex_literal_encoding_prefixes() {
    assert_eq!(
        kinds("'a' L'a' u8'a' u'a' U'a' \"\" L\"\" u8\"\" u\"\" U\"\"\n"),
        "char_constant wide_char_constant utf8_char_constant utf16_char_constant utf32_char_constant \
         string_literal wide_string_literal utf8_string_literal utf16_string_literal utf32_string_literal"
    );
    let l = lex("u8\"x\"\n");
    assert_eq!(l.tokens[0].kind, TokKind::StringLit(Encoding::Utf8));
}

/// [diff.cpp03.lex], [diff.cpp14.lex]: prefixes are identifiers in
/// standards that lack them.
#[test]
fn lex_literal_prefix_gating() {
    assert_eq!(kinds_with("u8'a'\n", std(Std::Cxx14)), "identifier char_constant");
    assert_eq!(
        kinds_with("u\"a\" R\"(x)\"\n", std(Std::Cxx03)),
        "identifier string_literal identifier string_literal"
    );
    assert_eq!(kinds_with("L'a'\n", std(Std::Cxx98)), "wide_char_constant");
}

/// [lex.string]/1: a prefix must be adjacent; otherwise it is an identifier.
#[test]
fn lex_literal_prefix_not_adjacent() {
    assert_eq!(kinds("u8 \"x\" Lx\"y\"\n"), "identifier string_literal identifier string_literal");
}

/// [lex.string]/1: escapes do not end the literal.
#[test]
fn lex_literal_escapes() {
    assert_eq!(spellings("\"a\\\"b\" '\\'' \"\\\\\"\n"), "\"a\\\"b\" '\\'' \"\\\\\"");
}

/// [lex.pptoken]/3.1, [lex.string]: raw strings revert phase 1-2 in the body.
#[test]
fn lex_literal_raw_string() {
    let l = lex("R\"d(a\\\nb)\" )d\" x\n");
    assert_eq!(l.spellings, ["R\"d(a\\\nb)\" )d\"", "x"]);
    assert_eq!(l.tokens[0].kind, TokKind::RawStringLit(Encoding::Ordinary));
    assert!(!l.tokens[0].has(TokFlags::NEEDS_CLEANING));
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
    assert_eq!(
        kinds("LR\"(a)\" u8R\"(b)\" uR\"()\" UR\"x()x\"\n"),
        "wide_raw_string_literal utf8_raw_string_literal utf16_raw_string_literal utf32_raw_string_literal"
    );
}

/// [lex.string]/1: splices before the body are still processed.
#[test]
fn lex_literal_raw_string_splice_in_prefix() {
    let l = lex("R\\\n\"(a\\\n)\"\n");
    assert_eq!(l.spellings, ["R\"(a\\\n)\""]);
    assert!(l.tokens[0].has(TokFlags::NEEDS_CLEANING));
}

/// [lex.string]/2: delimiters are at most 16 d-chars.
#[test]
fn lex_literal_raw_delimiter() {
    assert!(
        diag_kinds("R\"12345678901234567(x)12345678901234567\"\n", cxx26())
            .contains(&D::RawDelimiterTooLong)
    );
    assert!(
        diag_kinds("R\"a b(x)a b\"\n", cxx26())
            .iter()
            .any(|d| matches!(d, D::RawDelimiterInvalidChar { ch: ' ' }))
    );
    assert_eq!(kinds("R\"a b(x)a b\" y\n"), "unknown identifier");
}

/// [lex.string]: unterminated raw string runs to end of file.
#[test]
fn lex_literal_raw_unterminated() {
    let l = lex("R\"x(abc\ndef\n");
    assert_eq!(kinds("R\"x(abc\ndef\n"), "unknown");
    assert_eq!(l.diags[0].0, D::UnterminatedRawString);
}

/// [lex.ext]: a user-defined suffix is part of the literal.
#[test]
fn lex_ext_ud_suffix() {
    let l = lex("\"x\"_y 'c'_z \"s\"s u8\"t\"sv R\"(r)\"_w\n");
    assert_eq!(l.spellings, ["\"x\"_y", "'c'_z", "\"s\"s", "u8\"t\"sv", "R\"(r)\"_w"]);
    assert!(l.tokens.iter().all(|t| t.has(TokFlags::UD_SUFFIX)));
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
}

/// [lex.ext], [usrlit.suffix] (clang behavior): a reserved suffix not
/// starting with `_` is not lexed as a suffix and is diagnosed.
#[test]
fn lex_ext_reserved_suffix() {
    let l = lex("\"abc\"xyz\n");
    assert_eq!(l.spellings, ["\"abc\"", "xyz"]);
    assert_eq!(l.diags[0].0, D::ReservedUdSuffix);
    assert_eq!(kinds_with("\"a\"_b\n", std(Std::Cxx03)), "string_literal identifier");
}

/// [lex.ext] (clang behavior): after a string literal, every standard
/// library suffix is lexed as a ud-suffix from C++14 (`operator""h`
/// declares the `<chrono>` operators), `d` and `y` from C++20.
#[test]
fn lex_ext_library_suffixes_after_string() {
    let src = "\"\"h \"\"min \"\"ms \"\"us \"\"ns \"\"il \"\"i \"\"if \"\"s \"\"sv\n";
    let l = lex_with(src, std(Std::Cxx14));
    assert_eq!(l.tokens.len(), 10);
    assert!(l.tokens.iter().all(|t| t.has(TokFlags::UD_SUFFIX)));
    assert_eq!(l.diags.len(), 0, "{:?}", l.diags);
    assert_eq!(kinds_with("\"\"d \"\"y\n", std(Std::Cxx20)), "string_literal string_literal");
    let l = lex_with("\"\"d\n", std(Std::Cxx17));
    assert_eq!(l.spellings, ["\"\"", "d"]);
    assert_eq!(l.diags[0].0, D::ReservedUdSuffix);
    let l = lex_with("\"\"h\n", std(Std::Cxx11));
    assert_eq!(l.diags[0].0, D::ReservedUdSuffix);
}

/// [lex.ccon], [lex.string]: unterminated literals end at the newline.
#[test]
fn lex_literal_unterminated() {
    let l = lex("\"abc\nx 'q\ny\n");
    assert_eq!(l.spellings, ["\"abc", "x", "'q", "y"]);
    assert_eq!(kinds("\"abc\nx 'q\ny\n"), "unknown identifier unknown identifier");
    let ks: Vec<_> = l.diags.iter().map(|d| d.0).collect();
    assert_eq!(ks, [D::UnterminatedString, D::UnterminatedChar]);
}

/// [lex.ccon]/1: `''` is ill-formed.
#[test]
fn lex_literal_empty_char() {
    assert_eq!(kinds("'' x\n"), "unknown identifier");
    assert_eq!(diag_kinds("''\n", cxx26()), [D::EmptyCharLiteral]);
}

/// [lex.string]: a splice inside an ordinary literal is removed.
#[test]
fn lex_literal_spliced() {
    let l = lex("\"a\\\nb\"\n");
    assert_eq!(l.spellings, ["\"ab\""]);
    assert!(l.tokens[0].has(TokFlags::NEEDS_CLEANING));
}
