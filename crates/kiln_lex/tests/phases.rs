//! Translation phases 1 and 2: [lex.phases]/1.1-1.2, [lex.charset].
mod common;
use common::{cxx26, diag_kinds, dump_of, lex, lex_with, spellings, std};
use kiln_lex::{LexDiagKind as D, LexOptions, Std, TokFlags};

/// [lex.phases]/1.2: a backslash-newline is deleted, even inside a token.
#[test]
fn lex_phases_2_splice_inside_identifier() {
    let l = lex("ab\\\ncd ef\n");
    assert_eq!(l.idents, ["abcd", "ef"]);
    assert_eq!(l.spellings, ["abcd", "ef"]);
    assert!(l.tokens[0].has(TokFlags::NEEDS_CLEANING));
    assert!(!l.tokens[1].has(TokFlags::NEEDS_CLEANING));
    assert_eq!(l.diags.len(), 0);
}

/// [lex.phases]/1.2: CRLF and lone CR are line terminators for splicing.
#[test]
fn lex_phases_2_splice_crlf_and_cr() {
    assert_eq!(lex("a\\\r\nb").idents, ["ab"]);
    assert_eq!(lex("a\\\rb").idents, ["ab"]);
}

/// [lex.phases]/1.2: splices inside punctuators and pp-numbers.
#[test]
fn lex_phases_2_splice_inside_punct_and_number() {
    assert_eq!(
        dump_of("<\\\n<=\\\n 1\\\n2\n"),
        "lesslessequal '<<=' [StartOfLine] [UnClean]\nnumeric_constant '12' [LeadingSpace] [UnClean]\n"
    );
}

/// [lex.phases]/1.2 (clang extension): whitespace between `\` and the
/// newline still splices, with a warning.
#[test]
fn lex_phases_2_backslash_space_newline() {
    let l = lex("a\\  \nb\n");
    assert_eq!(l.idents, ["ab"]);
    assert_eq!(l.diags.iter().map(|d| d.0).collect::<Vec<_>>(), [D::BackslashNewlineSpace]);
}

/// [lex.phases]/1.2: a splice at end of file is diagnosed.
#[test]
fn lex_phases_2_backslash_newline_at_eof() {
    let k = diag_kinds("a\n\\\n", cxx26());
    assert!(k.contains(&D::BackslashNewlineEof), "{k:?}");
    let l = lex("a\n\\\n");
    assert_eq!(l.idents, ["a"]);
    assert_eq!(l.tokens.len(), 1);
}

/// [lex.phases]/1.2: a file not ending in a newline is accepted
/// (diagnosed as a pedantic extension, like clang's `-Wnewline-eof`).
#[test]
fn lex_phases_2_no_newline_at_eof() {
    assert_eq!(diag_kinds("a", cxx26()), [D::NoNewlineAtEof]);
    assert_eq!(diag_kinds("a\n", cxx26()).len(), 0);
    assert_eq!(diag_kinds("", cxx26()).len(), 0);
}

/// [lex.phases]/1.1: a UTF-8 BOM at the start is not part of the text.
#[test]
fn lex_phases_1_bom_skipped() {
    let l = lex("\u{FEFF}a\n");
    assert_eq!(l.dump, "identifier 'a' [StartOfLine]\n");
    assert_eq!(l.diags.len(), 0);
}

/// [lex.phases] (C++14 and earlier): trigraphs are replaced when enabled.
#[test]
fn lex_phases_1_trigraphs_enabled() {
    let opts = LexOptions { trigraphs: true, ..std(Std::Cxx14) };
    let l = lex_with("??=define x ??( ??) ??< ??> ??' ??! ??-\n", opts);
    assert_eq!(l.spellings, ["#", "define", "x", "[", "]", "{", "}", "^", "|", "~"]);
    assert!(l.diags.iter().all(|d| matches!(d.0, D::TrigraphConverted { .. })));
    assert_eq!(l.diags.len(), 8);
}

/// [lex.phases] (C++14 and earlier): `??/` followed by a newline splices.
#[test]
fn lex_phases_1_trigraph_backslash_splice() {
    let opts = LexOptions { trigraphs: true, ..std(Std::Cxx14) };
    assert_eq!(lex_with("a??/\nb\n", opts).idents, ["ab"]);
}

/// [diff.cpp14.lex]: trigraphs are not replaced in C++17; `??=` is three
/// punctuators, and in literals they are diagnosed as ignored.
#[test]
fn lex_phases_1_trigraphs_disabled() {
    let l = lex("??= \"??=\"\n");
    assert_eq!(l.spellings, ["?", "?", "=", "\"??=\""]);
    assert!(l.diags.iter().any(|d| matches!(d.0, D::TrigraphIgnored { ch: '#' })));
}

/// [lex.phases]/1.3 (implementation): NUL is whitespace, diagnosed.
#[test]
fn lex_phases_null_is_whitespace() {
    let l = lex("a\0b\n");
    assert_eq!(l.idents, ["a", "b"]);
    assert!(l.tokens[1].has(TokFlags::LEADING_SPACE));
    assert_eq!(l.diags.iter().map(|d| d.0).collect::<Vec<_>>(), [D::NullCharacter]);
}

/// [lex.charset]: Unicode whitespace separates tokens (diagnosed).
#[test]
fn lex_phases_unicode_whitespace() {
    let l = lex("a\u{3000}b\n");
    assert_eq!(l.idents, ["a", "b"]);
    assert!(l.tokens[1].has(TokFlags::LEADING_SPACE));
    assert_eq!(l.diags[0].0, D::UnicodeWhitespace { ch: '\u{3000}' });
}

/// [lex.phases]/1.2: spellings drop splices; spans still cover the source.
#[test]
fn lex_phases_2_spelling_and_span() {
    let l = lex("x\\\ny\n");
    assert_eq!(l.spellings, ["xy"]);
    assert_eq!(spellings("  foo\\\n\\\nbar  "), "foobar");
}
