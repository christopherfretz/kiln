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
