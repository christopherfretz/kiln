//! Token flags and the preprocessor protocol: [lex.pptoken], [cpp.pre],
//! [cpp.include]/1 (header names, [lex.header]).
mod common;
use common::{cxx26, dump_of, lex_driven, lex_header_mode};
use kiln_lex::{Interner, LexDiagKind as D, LexMode, Lexer, TokFlags, TokKind};
use kiln_source::SourceManager;

/// [cpp.pre]/1: `START_OF_LINE` and `LEADING_SPACE` are exact.
#[test]
fn lex_flags_start_of_line_and_leading_space() {
    assert_eq!(
        dump_of("  a b\n\tc\n\n d\\\n e\n"),
        "identifier 'a' [StartOfLine] [LeadingSpace]\nidentifier 'b' [LeadingSpace]\n\
         identifier 'c' [StartOfLine] [LeadingSpace]\nidentifier 'd' [StartOfLine] [LeadingSpace]\n\
         identifier 'e' [LeadingSpace]\n"
    );
}

/// [cpp.pre]: a splice is not whitespace; the next token has no leading space.
#[test]
fn lex_flags_splice_not_space() {
    assert_eq!(dump_of("a\\\n+\n"), "identifier 'a' [StartOfLine]\nplus '+' [UnClean]\n");
}

/// [cpp.pre]/1: while `in_directive` is set, a newline yields `Eod`
/// (which clears it); `Eod` is also produced at end of file.
#[test]
fn lex_directive_eod() {
    let l = lex_driven("#define X 1\nY\n#x", cxx26(), |lx, toks| {
        if toks.last().is_some_and(|t| {
            t.has(TokFlags::START_OF_LINE) && lx.source_manager().span_text(t.span) == Some("#")
        }) {
            lx.set_in_directive(true);
        }
    });
    let ks: Vec<_> = l.tokens.iter().map(|t| t.kind.name()).collect();
    assert_eq!(
        ks,
        [
            "hash",
            "identifier",
            "identifier",
            "numeric_constant",
            "eod",
            "identifier",
            "hash",
            "identifier",
            "eod"
        ]
    );
    // The token after Eod still starts a line.
    assert!(l.tokens[5].has(TokFlags::START_OF_LINE));
}

/// [cpp.pre]: a spliced newline does not end a directive.
#[test]
fn lex_directive_splice_continues() {
    let l = lex_driven("#a \\\n b\nc\n", cxx26(), |lx, toks| {
        if toks.len() == 1 {
            lx.set_in_directive(true);
        }
    });
    let ks: Vec<_> = l.tokens.iter().map(|t| t.kind.name()).collect();
    assert_eq!(ks, ["hash", "identifier", "identifier", "eod", "identifier"]);
}

/// [lex.header]/1: header names in header-name mode.
#[test]
fn lex_header_name() {
    let l = lex_header_mode("<a/b.h> x\n");
    assert_eq!(l.tokens[0].kind, TokKind::HeaderName);
    assert_eq!(l.spellings, ["<a/b.h>", "x"]);
    let l = lex_header_mode("\"a\\b.h\" x\n");
    assert_eq!(l.tokens[0].kind, TokKind::HeaderName);
    assert_eq!(l.spellings[0], "\"a\\b.h\"");
}

/// [lex.header]/2: `'`, `\`, `//` or `/*` in a header name, and `"` in a
/// `<…>` one, are conditionally-supported: kept as ordinary characters,
/// with an extension diagnostic each (spanning the character/sequence).
#[test]
fn lex_header_name_special_chars() {
    let seqs = |src: &str| -> Vec<(D, u32, u32)> { lex_header_mode(src).diags };
    assert_eq!(seqs("<a'b>\n"), [(D::HeaderNameSpecialChars { seq: "'" }, 2, 3)]);
    assert_eq!(seqs("\"a\\b.h\"\n"), [(D::HeaderNameSpecialChars { seq: "\\" }, 2, 3)]);
    assert_eq!(seqs("<a//b>\n"), [(D::HeaderNameSpecialChars { seq: "//" }, 2, 4)]);
    assert_eq!(seqs("\"a/*b\"\n"), [(D::HeaderNameSpecialChars { seq: "/*" }, 2, 4)]);
    assert_eq!(seqs("<a\"b>\n"), [(D::HeaderNameSpecialChars { seq: "\"" }, 2, 3)]);
    assert_eq!(seqs("<a/b.h>\n"), []);
    let l = lex_header_mode("<a//b>\n");
    assert_eq!(l.spellings, ["<a//b>"]);
    assert_eq!(l.tokens[0].kind, TokKind::HeaderName);
    for (d, ..) in &l.diags {
        assert_eq!(d.severity(), kiln_lex::LexSeverity::Extension);
    }
}

/// [lex.header]: header-name mode is one-shot and falls back to normal
/// lexing when no header name is present.
#[test]
fn lex_header_name_fallback() {
    let l = lex_header_mode("<a b\n");
    assert_eq!(l.spellings, ["<", "a", "b"]);
    let l = lex_header_mode("X <a>\n");
    assert_eq!(l.spellings, ["X", "<", "a", ">"]);
}

/// [lex.header]: without the mode, `<a>` is three tokens.
#[test]
fn lex_header_name_mode_off() {
    let l = lex_driven("<a>\n", cxx26(), |_, _| {});
    assert_eq!(l.spellings, ["<", "a", ">"]);
}

/// [lex.phases]: `new_in_range` lexes a sub-range with file-relative spans.
#[test]
fn lex_in_range() {
    let sm = SourceManager::new();
    let f = sm.add_virtual("<r>", "aaa bb cc".to_owned()).unwrap();
    let i = Interner::new();
    let mut lx = Lexer::new_in_range(&sm, &i, f, 4..6, sm.loc(f, 0), cxx26());
    let t = lx.next();
    assert_eq!(lx.spelling(&t), "bb");
    assert_eq!(sm.offset_of(t.span.lo).unwrap().1, 4);
    assert!(lx.next().is_eof());
    assert!(lx.next().is_eof());
    assert_eq!(lx.diagnostics().len(), 0);
    assert_eq!(lx.mode(), LexMode::Normal);
}
