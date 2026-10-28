//! Lexer and literal diagnostics as `kiln_diag` diagnostics.
//!
//! A stopgap (spec/parse.md §8): the preprocessor (WP-012) owns the full
//! `LexDiagKind` mapping. Until it lands, the parser lexes the main file
//! itself and reports what it can: the kinds with a `lex_*` id of their own
//! use it, other errors use `lex_literal_error` with clang's wording, and
//! other warnings `lex_literal_warning`. Extensions are not reported.

use kiln_diag::{Diag, DiagId};
use kiln_lex::{LexDiag, LexDiagKind as K, LexSeverity};
use kiln_source::SourceManager;

/// The diagnostic for `d`, if kiln reports it.
pub fn lex_diag(sm: &SourceManager, d: &LexDiag) -> Option<Diag> {
    let text = || sm.span_text(d.span).unwrap_or_default().to_owned();
    let (id, args): (DiagId, Vec<String>) = match d.kind {
        K::InvalidChar { ch } => (DiagId::lex_invalid_char, vec![format!("{:04X}", u32::from(ch))]),
        K::UnterminatedBlockComment => (DiagId::lex_unterminated_comment, vec![]),
        K::UnterminatedString => (DiagId::lex_unterminated_string, vec![]),
        K::UnterminatedChar => (DiagId::lex_unterminated_char, vec![]),
        K::UnterminatedRawString => (DiagId::lex_unterminated_raw_string, vec![String::new()]),
        K::RawDelimiterTooLong => (DiagId::lex_raw_delim_too_long, vec![]),
        K::BackslashNewlineSpace => (DiagId::lex_backslash_newline_space, vec![]),
        K::EmptyCharLiteral => (DiagId::lex_empty_char, vec![]),
        K::InvalidDigit { ch, radix } => {
            let r = match radix {
                2 => "binary",
                8 => "octal",
                16 => "hexadecimal",
                _ => "decimal",
            };
            (DiagId::lex_invalid_digit, vec![ch.to_string(), r.to_owned()])
        }
        K::InvalidSuffix { floating } => {
            (DiagId::lex_invalid_suffix, vec![text(), if floating { "1" } else { "0" }.to_owned()])
        }
        K::IntegerTooLarge => (DiagId::sema_int_literal_too_large, vec![]),
        K::UnknownEscape { ch } => (DiagId::lex_unknown_escape, vec![ch.to_string()]),
        K::MultiCharLiteral => (DiagId::lex_multichar, vec![]),
        kind => match kind.severity() {
            LexSeverity::Error => (DiagId::lex_literal_error, vec![error_text(kind)]),
            LexSeverity::Warning => (DiagId::lex_literal_warning, vec![warning_text(kind)]),
            LexSeverity::Extension => return None,
        },
    };
    let mut diag = Diag::new(id, d.span);
    for a in args {
        diag = diag.arg(a);
    }
    Some(diag)
}

/// clang's wording for the errors without an id of their own.
fn error_text(kind: K) -> String {
    match kind {
        K::CharNotAllowedInIdentifier { ch } => {
            format!("character <U+{:04X}> not allowed in an identifier", u32::from(ch))
        }
        K::RawDelimiterInvalidChar { ch } => {
            format!("invalid character '{ch}' in raw string delimiter")
        }
        K::UcnTooLarge | K::UcnInvalidCodePoint { .. } => "invalid universal character".to_owned(),
        K::UcnBasicOrControl { .. } => {
            "universal character name refers to a control character".to_owned()
        }
        K::UcnUnknownName => "invalid Unicode character name".to_owned(),
        K::EscapeNoDigits { esc } => format!("\\{esc} used with no following hex digits"),
        K::EscapeMissingBrace { esc } => format!("expected '{{' after '\\{esc}' escape sequence"),
        K::DelimitedEscapeEmpty => "delimited escape sequence cannot be empty".to_owned(),
        K::DelimitedEscapeUnterminated => "expected '}'".to_owned(),
        K::DelimitedEscapeInvalidDigit { ch } => {
            format!("invalid digit '{ch}' in escape sequence")
        }
        K::HexEscapeTooLarge => "hex escape sequence out of range".to_owned(),
        K::OctalEscapeTooLarge => "octal escape sequence out of range".to_owned(),
        K::UcnEscapeIncomplete => "incomplete universal character name".to_owned(),
        K::UcnNameLooseMatch { .. } => "incorrect Unicode character name".to_owned(),
        K::CharTooLarge => "character too large for enclosing character literal type".to_owned(),
        K::MultiCharNotAllowed => {
            "Unicode character literals may not contain multiple characters".to_owned()
        }
        K::UnevaluatedStringPrefix => {
            "an unevaluated string literal cannot have an encoding prefix".to_owned()
        }
        K::UnevaluatedStringEscape { ch } => {
            format!("invalid escape sequence '\\{ch}' in an unevaluated string literal")
        }
        K::UnevaluatedStringUdSuffix => {
            "an unevaluated string literal cannot be a user-defined literal".to_owned()
        }
        K::ExponentNoDigits => "exponent has no digits".to_owned(),
        K::HexFloatNoExponent => "hexadecimal floating literal requires an exponent".to_owned(),
        K::HexFloatNoSignificand => {
            "hexadecimal floating constant requires a significand".to_owned()
        }
        K::DigitSeparatorPosition { at_end: true } => {
            "digit separator cannot appear at end of digit sequence".to_owned()
        }
        K::DigitSeparatorPosition { at_end: false } => {
            "digit separator cannot appear at start of digit sequence".to_owned()
        }
        other => format!("invalid token ({other:?})"),
    }
}

/// clang's wording for the warnings without an id of their own.
fn warning_text(kind: K) -> String {
    match kind {
        K::NullCharacter => "null character ignored".to_owned(),
        K::TrigraphIgnored { ch } => format!("trigraph ignored (??{ch})"),
        K::TrigraphConverted { ch } => format!("trigraph converted to '{ch}' character"),
        K::NestedBlockComment => "'/*' within block comment".to_owned(),
        K::EscapedNewlineBlockCommentEnd => {
            "escaped newline between */ characters at block end".to_owned()
        }
        K::ReservedUdSuffix => {
            "user-defined literal suffixes not starting with '_' are reserved".to_owned()
        }
        K::UcnNoDigits | K::UcnIncomplete | K::UcnDelimitedEmpty | K::UcnDelimitedIncomplete => {
            "incomplete universal character name; treating as '\\' followed by identifier"
                .to_owned()
        }
        K::IdentifierNotNfc => "identifier is not in Normalization Form C".to_owned(),
        K::SpaceshipCompat => "'<=>' is a single token in C++20".to_owned(),
        K::CharConstantTooLong => "character constant too long for its type".to_owned(),
        other => format!("{other:?}"),
    }
}
