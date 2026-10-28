//! Maps lexer diagnostics ([`LexDiag`]) to `kiln_diag` ids with clang's
//! wording.

use kiln_diag::{Diag, DiagId};
use kiln_lex::literal::LitDiag;
use kiln_lex::{Encoding, LexDiag, LexDiagKind, LexOptions, Token};
use kiln_source::{SourceManager, Span};

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
            Diag::new(DiagId::lex_char_not_allowed_in_identifier, span)
                .arg(format!("{:04X}", u32::from(ch)))
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
        K::EscapedNewlineBlockCommentEnd => {
            Diag::new(DiagId::lex_escaped_newline_block_comment_end, span)
        }
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
        K::RawDelimiterInvalidChar { ch } => {
            Diag::new(DiagId::lex_raw_delim_invalid_char, span).arg(ch)
        }
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
            let name = line
                .find('{')
                .map(|i| &line[i + 1..])
                .and_then(|r| r.split('}').next())
                .unwrap_or("");
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

/// The name in `\N{name}` (or the whole range if it has no braces).
fn ucn_name(range: &str) -> &str {
    match range.find('{') {
        Some(i) => range[i + 1..].split('}').next().unwrap_or(""),
        None => range,
    }
}

/// The diagnostics for the problems `diags` found decoding `spelling`, the
/// spelling of literal token `tok` (`enc`: a character literal's
/// encoding), with clang's wording and locations (clang
/// `LiteralSupport.cpp`). Shared by the `#if` evaluator and the parser
/// (`spec/pp.md`, `spec/parse.md` §2); `tok` may be a macro or scratch
/// token. Kinds kiln has no id for are dropped.
pub fn literal_diags(
    sm: &SourceManager,
    tok: &Token,
    spelling: &str,
    diags: &[LitDiag],
    enc: Option<Encoding>,
    opts: &LexOptions,
) -> Vec<Diag> {
    use LexDiagKind as K;
    let mut out = Vec::new();
    if diags.is_empty() {
        return out;
    }
    let text = sm.span_text(tok.span).map(str::to_owned);
    let text = text.as_deref().unwrap_or(spelling);
    for d in diags {
        let span = d.locate(tok, text, opts).span;
        let at = Span::new(span.lo, span.lo);
        let start = Span::new(tok.span.lo, tok.span.lo);
        let end = Span::new(span.hi, span.hi);
        // The backslash of the escape sequence the range is in.
        let backslash = || {
            let b =
                spelling[..(d.range.start as usize).min(spelling.len())].rfind('\\').unwrap_or(0);
            let b = u32::try_from(b).unwrap_or(0);
            let l = LitDiag { kind: d.kind, range: b..b };
            let lo = l.locate(tok, text, opts).span.lo;
            Span::new(lo, lo)
        };
        let range = spelling.get(d.range.start as usize..d.range.end as usize).unwrap_or("");
        let diag = match d.kind {
            K::InvalidDigit { ch, radix } => {
                Diag::new(DiagId::lit_invalid_digit, at).arg(ch).arg(match radix {
                    2 => 0,
                    8 => 1,
                    _ => 2,
                })
            }
            K::InvalidSuffix { floating } => {
                Diag::new(DiagId::lit_invalid_suffix, at).arg(range).arg(i32::from(floating))
            }
            K::ExponentNoDigits => Diag::new(DiagId::lit_exponent_no_digits, at),
            // clang points at the end of the digits read.
            K::HexFloatNoExponent => Diag::new(DiagId::lit_hex_float_requires, end).arg(0),
            K::HexFloatNoSignificand => Diag::new(DiagId::lit_hex_float_requires, end).arg(1),
            K::DigitSeparatorPosition { at_end } => {
                Diag::new(DiagId::lit_digit_separator, at).arg(i32::from(at_end))
            }
            K::BinaryLiteralExtension => Diag::new(DiagId::lit_binary_ext, start),
            K::HexFloatExtension => Diag::new(DiagId::lit_hex_float_ext, start),
            K::SizeSuffixExtension => Diag::new(DiagId::lit_size_t_ext, start),
            K::IntegerTooLarge => Diag::new(DiagId::sema_int_literal_too_large, start),
            K::UnknownEscape { ch } => Diag::new(DiagId::lit_unknown_escape, at).arg(ch),
            K::NonstandardEscape { ch } => Diag::new(DiagId::lit_nonstandard_escape, at).arg(ch),
            K::EscapeNoDigits { esc } => Diag::new(DiagId::lit_escape_no_digits, at).arg(esc),
            K::EscapeMissingBrace { esc } => {
                Diag::new(DiagId::lit_escape_missing_brace, at).arg(esc)
            }
            K::DelimitedEscapeEmpty => Diag::new(DiagId::lit_delimited_escape_empty, at),
            K::DelimitedEscapeUnterminated => Diag::new(DiagId::pp_expected, at).arg("'}'"),
            K::DelimitedEscapeInvalidDigit { ch } => {
                Diag::new(DiagId::lit_delimited_escape_invalid, backslash()).arg(ch)
            }
            K::DelimitedEscapeExtension | K::UcnDelimitedExtension => {
                Diag::new(DiagId::lex_delimited_escape_ext, at).arg(0)
            }
            K::UcnNamedExtension => Diag::new(DiagId::lex_delimited_escape_ext, at).arg(1),
            K::HexEscapeTooLarge => Diag::new(DiagId::lit_hex_escape_too_large, at),
            K::OctalEscapeTooLarge => Diag::new(DiagId::lit_octal_escape_too_large, at),
            K::UcnEscapeIncomplete => Diag::new(DiagId::lit_ucn_escape_incomplete, at),
            K::UcnUnknownName => Diag::new(DiagId::lex_ucn_unknown_name, at).arg(ucn_name(range)),
            K::UcnNameLooseMatch { .. } => Diag::new(DiagId::lex_ucn_unknown_name, at)
                .arg(ucn_name(range))
                .with_note(Diag::new(DiagId::note_ucn_name_loose_match, at)),
            K::CharTooLarge => Diag::new(DiagId::lit_char_too_large, start),
            K::MultiCharLiteral => Diag::new(DiagId::lit_multichar, start),
            K::FourCharLiteral => Diag::new(DiagId::lit_four_char, start),
            K::CharConstantTooLong => Diag::new(DiagId::lit_char_constant_too_long, start),
            K::MultiCharNotAllowed => Diag::new(DiagId::lit_multichar_not_allowed, start)
                .arg(i32::from(enc != Some(Encoding::Wide))),
            // [lex.string.uneval]; clang reports the prefix and the escape
            // sequence as ranges, the ud-suffix at the token.
            K::UnevaluatedStringPrefix => Diag::new(DiagId::lit_unevaluated_prefix, start),
            K::UnevaluatedStringEscape { ch } => {
                let esc =
                    if range.starts_with('\\') { range.to_owned() } else { format!("\\{ch}") };
                Diag::new(DiagId::lit_unevaluated_escape, at).arg(esc)
            }
            K::UnevaluatedStringUdSuffix => Diag::new(DiagId::lit_unevaluated_udl, start),
            _ => {
                let lex = LexDiag { kind: d.kind, span };
                match lex_diag(sm, &lex) {
                    Some(diag) => diag,
                    None => continue,
                }
            }
        };
        out.push(diag);
    }
    out
}
