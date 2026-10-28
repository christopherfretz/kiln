//! Comments: [lex.comment], [lex.phases]/1.3.
mod common;
use common::{cxx26, diag_kinds, dump_of, lex, lex_driven};
use kiln_lex::{LexDiagKind as D, TokFlags, TokKind};

/// [lex.phases]/1.3: a comment is replaced by one space.
#[test]
fn lex_comment_is_whitespace() {
    assert_eq!(
        dump_of("a/**/b//x\nc"),
        "identifier 'a' [StartOfLine]\nidentifier 'b' [LeadingSpace]\nidentifier 'c' [StartOfLine]\n"
    );
}

/// [lex.comment]/1: a multi-line block comment does not start a new line.
#[test]
fn lex_comment_block_multiline_no_start_of_line() {
    assert_eq!(
        dump_of("a /* x\n */ z\n"),
        "identifier 'a' [StartOfLine]\nidentifier 'z' [LeadingSpace]\n"
    );
}

/// [lex.comment]/1: `/*` comments do not nest; `*/` ends at the first one.
#[test]
fn lex_comment_block_does_not_nest() {
    let l = lex("/* /* */ a */\n");
    assert_eq!(l.spellings, ["a", "*", "/"]);
    assert_eq!(l.diags.iter().map(|d| d.0).collect::<Vec<_>>(), [D::NestedBlockComment]);
}

/// [lex.comment]/1: `/*/` does not close the comment it opens.
#[test]
fn lex_comment_slash_star_slash() {
    assert_eq!(lex("/*/ a */b\n").spellings, ["b"]);
}

/// [lex.comment]/1 with [lex.phases]/1.2: `*\<newline>/` ends a comment.
#[test]
fn lex_comment_block_end_spliced() {
    let l = lex("/* x *\\\n/b\n");
    assert_eq!(l.spellings, ["b"]);
    assert_eq!(l.diags.iter().map(|d| d.0).collect::<Vec<_>>(), [D::EscapedNewlineBlockCommentEnd]);
}

/// [lex.comment]/1: a `//` comment continued by a splice.
#[test]
fn lex_comment_line_spliced() {
    let l = lex("// a \\\nb\nc\n");
    assert_eq!(l.spellings, ["c"]);
    assert_eq!(l.diags.iter().map(|d| d.0).collect::<Vec<_>>(), [D::MultiLineLineComment]);
}

/// [lex.comment]: unterminated block comment runs to end of file.
#[test]
fn lex_comment_unterminated() {
    let l = lex("a /* b\n");
    assert_eq!(l.spellings, ["a"]);
    assert_eq!(l.diags[0].0, D::UnterminatedBlockComment);
    assert_eq!((l.diags[0].1, l.diags[0].2), (2, 4));
}

/// [lex.comment]: `//` and `/*` inside literals are not comments.
#[test]
fn lex_comment_inside_literal() {
    assert_eq!(lex("\"/*\" '//' \"//\"\n").spellings, ["\"/*\"", "'//'", "\"//\""]);
}

/// [lex.comment] (`-C` support): comments as tokens when kept.
#[test]
fn lex_comment_kept() {
    let l = lex_driven("a /* b */ // c\nd\n", cxx26(), |lx, _| lx.set_keep_comments(true));
    let ks: Vec<_> = l.tokens.iter().map(|t| t.kind).collect();
    assert_eq!(ks[1], TokKind::Comment);
    assert_eq!(l.spellings, ["a", "/* b */", "// c", "d"]);
    assert!(l.tokens[1].has(TokFlags::LEADING_SPACE));
    assert!(l.tokens[3].has(TokFlags::START_OF_LINE));
}

/// [lex.comment]: no diagnostics for ordinary comments.
#[test]
fn lex_comment_no_diags() {
    assert_eq!(diag_kinds("/**/ /***/ /* * / */ // /* \n", cxx26()).len(), 0);
}
