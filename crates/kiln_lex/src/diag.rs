//! Lexer diagnostics, recorded as data.
//!
//! `kiln_lex` does not depend on `kiln_diag`: it records a [`LexDiag`] per
//! problem and the preprocessor maps [`LexDiagKind`] to diagnostic ids
//! (and drops them in skipped `#if` groups, see
//! [`crate::Lexer::take_diagnostics`]).

use kiln_source::Span;

/// One lexer diagnostic.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LexDiag {
    /// What went wrong, with arguments.
    pub kind: LexDiagKind,
    /// Where: the offending characters (empty for a point location).
    pub span: Span,
}

/// Default severity class of a [`LexDiagKind`], matching clang's default.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LexSeverity {
    /// Ill-formed: an error by default.
    Error,
    /// Well-formed but suspicious: a warning by default.
    Warning,
    /// A kiln/clang extension or a newer-standard feature used in an older
    /// mode: an extension warning (`-pedantic`) by default.
    Extension,
}

/// Every diagnostic the lexer can record. The comment on each variant
/// gives the clang diagnostic it corresponds to.
#[non_exhaustive]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LexDiagKind {
    // ----- characters and whitespace -----
    /// A character that cannot start any token and is not an identifier
    /// character (`err_non_ascii` / `ext_unicode_whitespace` analogue).
    InvalidChar {
        /// The character.
        ch: char,
    },
    /// A non-ASCII character that is not allowed where it appears: inside
    /// an identifier but not `XID_Continue` (kept in the identifier, as
    /// clang does; `err_character_not_allowed_identifier`), or at the
    /// start of a token, `XID_Continue` but not `XID_Start` (a lone
    /// token; `err_character_not_allowed`).
    CharNotAllowedInIdentifier {
        /// The character.
        ch: char,
    },
    /// Non-ASCII whitespace treated as whitespace (`ext_unicode_whitespace`).
    UnicodeWhitespace {
        /// The character.
        ch: char,
    },
    /// A NUL byte in the source, ignored as whitespace (`null_in_file`).
    NullCharacter,

    // ----- phases 1-2 -----
    /// Whitespace between `\` and the newline of a line splice
    /// (`backslash_newline_space`).
    BackslashNewlineSpace,
    /// The file ends with a line splice (`ext_backslash_newline_eof`).
    BackslashNewlineEof,
    /// A trigraph that was not translated because trigraphs are disabled
    /// (`trigraph_ignored`).
    TrigraphIgnored {
        /// The character the trigraph would stand for.
        ch: char,
    },
    /// A trigraph that was translated (`trigraph_converted`).
    TrigraphConverted {
        /// The character it stands for.
        ch: char,
    },
    /// The file does not end in a newline (`ext_no_newline_eof`; C++11
    /// made this well-formed, so it is only a `-pedantic` remark).
    NoNewlineAtEof,

    // ----- comments -----
    /// `/*` without a matching `*/` (`err_unterminated_block_comment`).
    UnterminatedBlockComment,
    /// `/*` inside a block comment (`nested_block_comment`).
    NestedBlockComment,
    /// The `*/` ending a block comment is split by a line splice
    /// (`escaped_newline_block_comment_end`).
    EscapedNewlineBlockCommentEnd,
    /// A `//` comment continued by a line splice (`ext_multi_line_line_comment`).
    MultiLineLineComment,

    // ----- literals (lexed to their end only; contents are WP-011) -----
    /// A string literal with no closing `"` on its line
    /// (`ext_unterminated_char_or_string`, an error in C++).
    UnterminatedString,
    /// A character literal with no closing `'` on its line.
    UnterminatedChar,
    /// A raw string literal with no `)delimiter"` before end of input
    /// (`err_unterminated_raw_string`).
    UnterminatedRawString,
    /// `''` (`err_empty_character`).
    EmptyCharLiteral,
    /// A raw string delimiter longer than 16 characters
    /// (`err_raw_delim_too_long`).
    RawDelimiterTooLong,
    /// A character not allowed in a raw string delimiter
    /// (`err_invalid_char_raw_delim`).
    RawDelimiterInvalidChar {
        /// The character.
        ch: char,
    },
    /// A literal followed by an identifier not starting with `_` that is
    /// not a standard suffix; the identifier is lexed as a separate token
    /// (`ext_reserved_user_defined_literal`).
    ReservedUdSuffix,

    // ----- universal-character-names -----
    /// `\u` / `\U` with no hex digits (`warn_ucn_escape_no_digits`).
    UcnNoDigits,
    /// `\u` / `\U` with too few hex digits (`warn_ucn_escape_incomplete`).
    UcnIncomplete,
    /// `\u{}` (`warn_delimited_ucn_empty`).
    UcnDelimitedEmpty,
    /// `\u{` or `\N{` without the closing `}` (`warn_delimited_ucn_incomplete`).
    UcnDelimitedIncomplete,
    /// A delimited UCN whose value does not fit (`err_escape_too_large`).
    UcnTooLarge,
    /// A UCN naming a basic or control character (`err_ucn_escape_basic_scs`
    /// / `err_ucn_control_character`).
    UcnBasicOrControl {
        /// The code point.
        cp: u32,
    },
    /// A UCN naming a surrogate or a value above U+10FFFF, or a character
    /// not allowed in an identifier (`err_ucn_escape_invalid`).
    UcnInvalidCodePoint {
        /// The code point (`0x110000` and above for out-of-range values).
        cp: u32,
    },
    /// `\N{name}` with an unknown name (`err_invalid_ucn_name`).
    UcnUnknownName,
    /// `\u{...}` before C++23 (`ext_delimited_escape_sequence`).
    UcnDelimitedExtension,
    /// `\N{...}` before C++23 (`ext_delimited_escape_sequence`).
    UcnNamedExtension,

    // ----- identifiers and tokens -----
    /// An identifier not in Normalization Form C (`warn_utf8_symbol_zero_width`
    /// analogue; clang does not check this).
    IdentifierNotNfc,
    /// `$` in an identifier (`ext_dollar_in_identifier`).
    DollarInIdentifier,
    /// `<=>` before C++20, lexed as `<=` `>` (`warn_cxx20_compat_spaceship`).
    SpaceshipCompat,
}

impl LexDiagKind {
    /// clang's default severity for this diagnostic.
    pub const fn severity(self) -> LexSeverity {
        use LexDiagKind as K;
        match self {
            K::InvalidChar { .. }
            | K::CharNotAllowedInIdentifier { .. }
            | K::UnterminatedBlockComment
            | K::UnterminatedString
            | K::UnterminatedChar
            | K::UnterminatedRawString
            | K::EmptyCharLiteral
            | K::RawDelimiterTooLong
            | K::RawDelimiterInvalidChar { .. }
            | K::UcnTooLarge
            | K::UcnBasicOrControl { .. }
            | K::UcnInvalidCodePoint { .. }
            | K::UcnUnknownName => LexSeverity::Error,
            K::NullCharacter
            | K::BackslashNewlineSpace
            | K::TrigraphIgnored { .. }
            | K::TrigraphConverted { .. }
            | K::NestedBlockComment
            | K::EscapedNewlineBlockCommentEnd
            | K::ReservedUdSuffix
            | K::UcnNoDigits
            | K::UcnIncomplete
            | K::UcnDelimitedEmpty
            | K::UcnDelimitedIncomplete
            | K::IdentifierNotNfc
            | K::SpaceshipCompat => LexSeverity::Warning,
            K::UnicodeWhitespace { .. }
            | K::BackslashNewlineEof
            | K::NoNewlineAtEof
            | K::MultiLineLineComment
            | K::UcnDelimitedExtension
            | K::UcnNamedExtension
            | K::DollarInIdentifier => LexSeverity::Extension,
        }
    }
}
