//! C mode (spec/lex.md "C mode", WP-1275): the C++-only tokens and literal
//! forms are off, as `clang-19 -x c` lexes them.
mod common;
use common::{lex_with, std};
use kiln_lex::{CStd, LexOptions, Std};

/// Options for C standard `c` (lexing as the C++ standard it maps to).
fn c(c: CStd, gnu: bool) -> LexOptions {
    let lex = match c {
        CStd::C89 | CStd::C99 => Std::Cxx98,
        CStd::C11 | CStd::C17 => Std::Cxx11,
        CStd::C23 => Std::Cxx17,
    };
    LexOptions { c: Some(c), gnu, ..std(lex) }
}

fn spell(text: &str, o: LexOptions) -> String {
    lex_with(text, o).spellings.join(" ")
}

/// C23 6.4.6: `.*`, `->*` and `<=>` are not C punctuators; `::` is one
/// (C23, and clang lexes it in every C mode).
#[test]
fn lex_c_punctuators() {
    let o = c(CStd::C23, false);
    assert_eq!(spell("a.*b p->*q x<=>y", o), "a . * b p -> * q x <= > y");
    assert_eq!(spell("gnu::x", o), "gnu :: x");
    assert_eq!(spell("gnu::x", c(CStd::C89, false)), "gnu :: x");
}

/// [lex.pptoken]/3.2's `<::` rule is C++'s: in C, `<:` is a digraph.
#[test]
fn lex_c_less_colon_colon_is_a_digraph() {
    assert_eq!(spell("a<::b", c(CStd::C17, false)), "a <: : b");
}

/// C23 6.4.5, 6.4.4: no user-defined-literal suffixes in C.
#[test]
fn lex_c_no_ud_suffix() {
    assert_eq!(spell("\"s\"_x 'c'_y", c(CStd::C23, false)), "\"s\" _x 'c' _y");
}

/// Raw strings are a GNU extension of C from gnu99 (clang 19); otherwise
/// `R` is an identifier.
#[test]
fn lex_c_raw_strings_only_in_gnu() {
    let raw = "R\"(a\\b)\"";
    assert_eq!(spell(raw, c(CStd::C11, true)), raw);
    assert_eq!(spell(raw, c(CStd::C11, false)), "R \"(a\\b)\"");
    assert_eq!(spell(raw, c(CStd::C89, true)), "R \"(a\\b)\"");
}

/// C23 6.4.4.1: digit separators and C23 6.4.4.5 `u8` character constants
/// are C23's.
#[test]
fn lex_c23_digit_separators_and_u8_chars() {
    assert_eq!(spell("1'000 u8'a'", c(CStd::C23, false)), "1'000 u8'a'");
    assert_eq!(spell("u8'a'", c(CStd::C17, false)), "u8 'a'");
}

/// C99 6.4.4.2: hexadecimal floating constants (`p+`) lex as one
/// pp-number from C99.
#[test]
fn lex_c99_hex_float() {
    assert_eq!(spell("0x1p+3", c(CStd::C99, false)), "0x1p+3");
}

/// C89 3.1.9: strict C89 has no `//` comments. clang lexes them as
/// comments when compiling (except `//*` before the file's first line
/// comment, which is `/` then a block comment), as two `/` in preprocessed
/// output; gnu89 and later C have them.
#[test]
fn lex_c89_line_comments() {
    let pp = LexOptions { preprocessed_output: true, ..c(CStd::C89, false) };
    assert_eq!(spell("a //**/ b\nc", c(CStd::C89, false)), "a / b c");
    assert_eq!(spell("a // x\nb //**/ c\nd", c(CStd::C89, false)), "a b d");
    assert_eq!(spell("a //**/ b\nc", pp), "a / b c");
    assert_eq!(spell("a // b\n", pp), "a / / b");
    let gnu = LexOptions { preprocessed_output: true, ..c(CStd::C89, true) };
    assert_eq!(spell("a // b\nc", gnu), "a c");
    let c99 = LexOptions { preprocessed_output: true, ..c(CStd::C99, false) };
    assert_eq!(spell("a // b\nc", c99), "a c");
}

/// The -pedantic "// comments are not allowed in this language" extension
/// is reported once per file, at the first line comment; gnu89 has none.
#[test]
fn lex_c89_line_comment_warns_once() {
    use kiln_lex::LexDiagKind;
    let count = |o| {
        let l = lex_with("a // x\nb // y\n", o);
        l.diags
            .iter()
            .filter(|d| d.0 == LexDiagKind::C89LineComment)
            .map(|d| (d.1, d.2))
            .collect::<Vec<_>>()
    };
    assert_eq!(count(c(CStd::C89, false)).len(), 1);
    assert!(count(c(CStd::C89, true)).is_empty());
    assert!(count(c(CStd::C99, false)).is_empty());
}

/// C99 6.4.2.1p3 / Annex D, C11 6.4.2.1p3 / D.1-D.2, C23 6.4.2.1 (XID):
/// which non-ASCII characters an identifier may contain, per standard.
/// U+00A8 is C11-only, U+0660 a C99 "digit" (not initially), U+0300 a
/// combining mark (C11 and XID: not initially; C99: never).
#[test]
fn lex_c_identifier_tables() {
    use kiln_lex::LexDiagKind as D;
    let run = |text: &str, o| {
        let l = lex_with(text, o);
        let d = l.diags.iter().map(|d| d.0);
        let d = d.filter(|d| !matches!(d, D::C99CompatUnicodeId { .. }));
        (l.idents.join(" "), d.collect::<Vec<_>>())
    };
    let c11 = c(CStd::C11, false);
    let c99 = c(CStd::C99, false);
    let c23 = c(CStd::C23, false);
    assert_eq!(run("\u{A8}x a\u{660}\n", c11), ("\u{A8}x a\u{660}".to_string(), vec![]));
    let (ids, d) = run("\u{A8}x\n", c99);
    assert_eq!((ids.as_str(), d), ("x", vec![D::InvalidChar { ch: '\u{A8}' }]));
    let (ids, d) = run("\u{660}x\n", c99);
    assert_eq!(ids, "x");
    assert_eq!(d, [D::CharNotAllowedInIdentifier { ch: '\u{660}', at_start: true }]);
    let (ids, d) = run("q\u{300}\n", c99);
    assert_eq!(ids, "q\u{300}");
    assert_eq!(d, [D::CharNotAllowedInIdentifier { ch: '\u{300}', at_start: false }]);
    assert_eq!(run("q\u{300}\n", c11).1, []);
    // U+1F600 is in C11 D.1 but not XID.
    assert_eq!(run("\u{1F600}\n", c11).0, "\u{1F600}");
    assert_eq!(run("\u{1F600}x\n", c23).1, [D::InvalidChar { ch: '\u{1F600}' }]);
    // -Wc99-compat on characters C99 rejects (here, or initially).
    let compat = |text: &str| {
        lex_with(text, c11)
            .diags
            .iter()
            .filter_map(|d| match d.0 {
                D::C99CompatUnicodeId { start } => Some(start),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(compat("\u{A8}x a\u{A8} \u{660}y a\u{660}\n"), [false, false, true]);
}

/// C89 has no universal character names (clang 19 `-Wunicode`): `\u` at
/// a token start is a lone `\` and an identifier; inside an identifier
/// it ends it.
#[test]
fn lex_c89_no_ucns() {
    use kiln_lex::LexDiagKind as D;
    for gnu in [false, true] {
        let l = lex_with("\\u00AA a\\u00AA\n", c(CStd::C89, gnu));
        assert_eq!(l.spellings, ["\\", "u00AA", "a", "\\", "u00AA"]);
        let d: Vec<_> = l.diags.iter().map(|d| d.0).collect();
        assert_eq!(d, [D::UcnNotValidInC89, D::UcnNotValidInC89]);
    }
    assert_eq!(lex_with("\\u00AA\n", c(CStd::C99, false)).idents, ["\u{AA}"]);
}

/// C11 6.4.3p2 (a UCN shall not name a basic character, in a literal too,
/// until C23) and C89's lack of UCNs (clang 19
/// `warn_ucn_not_valid_in_c89_literal`, decoded anyway).
#[test]
fn lex_c_literal_ucns() {
    use kiln_lex::LexDiagKind as D;
    use kiln_lex::literal::{LiteralOptions, decode_string};
    let lit = |sp: &str, cs: CStd| {
        let o = LiteralOptions { c: Some(cs), ..LiteralOptions::new(c(cs, false).std, Default::default()) };
        let s = decode_string(sp, o);
        (s.units.iter().collect::<Vec<u32>>(), s.diags.iter().map(|d| d.kind).collect::<Vec<_>>())
    };
    for cs in [CStd::C89, CStd::C99, CStd::C11, CStd::C17] {
        assert_eq!(lit(r#""\u0041""#, cs).1, [D::UcnBasicOrControl { cp: 0x41 }], "{cs:?}");
    }
    assert_eq!(lit(r#""\u0041""#, CStd::C23), (vec![0x41], vec![]));
    assert_eq!(lit(r#""\u0024""#, CStd::C11).1, []);
    assert_eq!(lit(r#""\u00AA""#, CStd::C89), (vec![0xC2, 0xAA], vec![D::UcnNotValidInC89Literal]));
    assert_eq!(lit(r#""\uD800""#, CStd::C89).1, [D::UcnInvalidCodePoint { cp: 0xD800 }]);
    assert_eq!(lit(r#""\u00AA""#, CStd::C99).1, []);
}
