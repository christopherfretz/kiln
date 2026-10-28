//! Maps lexer diagnostics ([`LexDiag`]) to `kiln_diag` ids with clang's
//! wording.

use kiln_diag::{Diag, DiagId};
use kiln_lex::{LexDiag, LexDiagKind};
use kiln_source::SourceManager;

/// Source text from `d.span.lo` to the end of its line (for arguments
/// that the lexer does not record, such as a raw-string delimiter).
fn rest_of_line<'s>(sm: &'s SourceManager, d: &LexDiag) -> &'s str {
    let Some((file, off)) = sm.offset_of(sm.spelling_loc(d.span.lo)) else {
        return "";
    };
    let text = sm.text(file);
    let start = (off as usize).min(text.len());
    let rest = &text[start..];
    rest.split(['\n', '\r']).next().unwrap_or("")
}

/// The diagnostic for `d`, or `None` for kinds kiln has no id for yet
/// (they are dropped; see `spec/pp.md`).
pub(crate) fn lex_diag(sm: &SourceManager, d: &LexDiag) -> Option<Diag> {
    use LexDiagKind as K;
    let span = d.span;
    let diag = match d.kind {
        K::InvalidChar { ch } => {
            Diag::new(DiagId::lex_invalid_char, span).arg(format!("{:04X}", u32::from(ch)))
        }
        K::CharNotAllowedInIdentifier { ch } => {
            Diag::new(DiagId::lex_char_not_allowed_in_identifier, span).arg(format!("{:04X}", u32::from(ch)))
        }
        K::UnicodeWhitespace { .. } => Diag::new(DiagId::lex_unicode_whitespace, span),
        K::NullCharacter => Diag::new(DiagId::lex_null_character, span),
        K::BackslashNewlineSpace => Diag::new(DiagId::lex_backslash_newline_space, span),
        K::BackslashNewlineEof => Diag::new(DiagId::lex_backslash_newline_eof, span),
        K::TrigraphIgnored { .. } => Diag::new(DiagId::lex_trigraph_ignored, span),
        K::TrigraphConverted { ch } => Diag::new(DiagId::lex_trigraph_converted, span).arg(ch),
        K::NoNewlineAtEof => Diag::new(DiagId::lex_no_newline_eof, span),
        K::UnterminatedBlockComment => Diag::new(DiagId::lex_unterminated_comment, span),
        K::NestedBlockComment => Diag::new(DiagId::lex_nested_block_comment, span),
        K::EscapedNewlineBlockCommentEnd => Diag::new(DiagId::lex_escaped_newline_block_comment_end, span),
        K::MultiLineLineComment => Diag::new(DiagId::lex_multi_line_line_comment, span),
        K::UnterminatedString => Diag::new(DiagId::lex_unterminated_string, span),
        K::UnterminatedChar => Diag::new(DiagId::lex_unterminated_char, span),
        K::UnterminatedRawString => {
            let line = rest_of_line(sm, d);
            let delim = line
                .find("R\"")
                .map(|i| &line[i + 2..])
                .and_then(|r| r.split('(').next())
                .unwrap_or("");
            Diag::new(DiagId::lex_unterminated_raw_string, span).arg(delim)
        }
        K::EmptyCharLiteral => Diag::new(DiagId::lex_empty_character, span),
        K::RawDelimiterTooLong => Diag::new(DiagId::lex_raw_delim_too_long, span),
        K::RawDelimiterInvalidChar { ch } => Diag::new(DiagId::lex_raw_delim_invalid_char, span).arg(ch),
        K::ReservedUdSuffix => Diag::new(DiagId::lex_reserved_ud_suffix, span),
        K::UcnNoDigits => {
            let c = rest_of_line(sm, d).chars().nth(1).unwrap_or('u');
            Diag::new(DiagId::lex_ucn_no_digits, span).arg(c)
        }
        K::UcnIncomplete => Diag::new(DiagId::lex_ucn_incomplete, span),
        K::UcnDelimitedEmpty => Diag::new(DiagId::lex_ucn_delimited_empty, span),
        K::UcnDelimitedIncomplete => Diag::new(DiagId::lex_ucn_delimited_incomplete, span),
        K::UcnTooLarge => Diag::new(DiagId::lex_ucn_too_large, span),
        K::UcnBasicOrControl { cp } => {
            if cp < 0x20 || (0x7F..0xA0).contains(&cp) {
                Diag::new(DiagId::lex_ucn_control_character, span)
            } else {
                Diag::new(DiagId::lex_ucn_basic_scs, span).arg(char::from_u32(cp).unwrap_or('?'))
            }
        }
        K::UcnInvalidCodePoint { .. } => Diag::new(DiagId::lex_ucn_invalid, span),
        K::UcnUnknownName => {
            let line = rest_of_line(sm, d);
            let name = line.find('{').map(|i| &line[i + 1..]).and_then(|r| r.split('}').next()).unwrap_or("");
            Diag::new(DiagId::lex_ucn_unknown_name, span).arg(name)
        }
        K::UcnDelimitedExtension => Diag::new(DiagId::lex_delimited_escape_ext, span).arg(0),
        K::UcnNamedExtension => Diag::new(DiagId::lex_delimited_escape_ext, span).arg(1),
        K::IdentifierNotNfc => Diag::new(DiagId::lex_identifier_not_nfc, span),
        K::DollarInIdentifier => Diag::new(DiagId::lex_dollar_in_identifier, span),
        K::SpaceshipCompat => Diag::new(DiagId::lex_spaceship_compat, span),
        _ => return None,
    };
    Some(diag)
}
