//! pp-numbers: [lex.ppnumber].
mod common;
use common::{lex, lex_with, spellings, std};
use kiln_lex::Std;

/// [lex.ppnumber]/1: a pp-number is maximal and need not be a valid
/// numeric literal.
#[test]
fn lex_ppnumber_maximal() {
    assert_eq!(spellings("1.2.3.4 0x1G .5 1..2 12ns 1_a\n"), "1.2.3.4 0x1G .5 1..2 12ns 1_a");
    assert_eq!(lex("1.2.3.4\n").tokens.len(), 1);
}

/// [lex.ppnumber]/1: `e+`, `E-`, `p+`, `P-` continue a pp-number.
#[test]
fn lex_ppnumber_exponent_sign() {
    assert_eq!(spellings("1e+5 1E-5 0x1p-3 1P+2 0xe+1\n"), "1e+5 1E-5 0x1p-3 1P+2 0xe+1");
    assert_eq!(spellings("1f+5\n"), "1f + 5");
}

/// [lex.ppnumber] in C++14: `p+` is not part of a pp-number except in
/// hexadecimal floating literals (clang extension).
#[test]
fn lex_ppnumber_p_sign_pre_cxx17() {
    assert_eq!(lex_with("1p+2 0x1p+2\n", std(Std::Cxx14)).spellings, ["1p", "+", "2", "0x1p+2"]);
}

/// [lex.ppnumber]/1 (C++14): digit separators.
#[test]
fn lex_ppnumber_digit_separator() {
    assert_eq!(spellings("1'000'000 0b1'0 1'a\n"), "1'000'000 0b1'0 1'a");
    assert_eq!(
        lex_with("1'0\n", std(Std::Cxx11)).spellings,
        ["1", "'0\n"].map(|s| s.trim_end_matches('\n'))
    );
}

/// [lex.ppnumber]/1: `'` not followed by a digit or nondigit ends it.
#[test]
fn lex_ppnumber_separator_not_followed() {
    assert_eq!(lex("1'+'\n").spellings, ["1", "'+'"]);
}

/// [lex.ppnumber]/1: a pp-number begins with a digit or `.digit`.
#[test]
fn lex_ppnumber_start() {
    assert_eq!(spellings(".x ..5\n"), ". x . .5");
}

/// [lex.ppnumber]/1: identifier-continue characters, including UCNs.
#[test]
fn lex_ppnumber_identifier_continue() {
    assert_eq!(spellings("1\\u00e9 2é\n"), "1\\u00e9 2é");
}
