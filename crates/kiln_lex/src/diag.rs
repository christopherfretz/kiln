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

    // ----- escape sequences and literal values (recorded by `literal`) -----
    /// `\q`: an escape sequence that is not one of [lex.ccon]'s; the
    /// character stands for itself (`ext_unknown_escape`, a warning).
    UnknownEscape {
        /// The character after the backslash.
        ch: char,
    },
    /// `\e`, `\E`, `\(`, `\{`, `\[`, `\%`: GNU escapes accepted as
    /// conditional-escape-sequences (`ext_nonstandard_escape`).
    NonstandardEscape {
        /// The character after the backslash.
        ch: char,
    },
    /// `\x`, `\u` or `\U` with no hex digit after it
    /// (`err_hex_escape_no_digits`).
    EscapeNoDigits {
        /// `x`, `u` or `U`.
        esc: char,
    },
    /// `\o` or `\N` not followed by `{` (`err_delimited_escape_missing_brace`).
    EscapeMissingBrace {
        /// `o` or `N`.
        esc: char,
    },
    /// `\x{}`, `\o{}`, `\u{}` or `\N{}` (`err_delimited_escape_empty`).
    DelimitedEscapeEmpty,
    /// A delimited escape without its closing `}` (`err_expected` `}`).
    DelimitedEscapeUnterminated,
    /// A character that is not a digit of the escape's base inside
    /// `\x{…}`, `\o{…}` or `\u{…}` (`err_delimited_escape_invalid`).
    DelimitedEscapeInvalidDigit {
        /// The character.
        ch: char,
    },
    /// `\x{…}` or `\o{…}` before C++23 (`ext_delimited_escape_sequence`).
    DelimitedEscapeExtension,
    /// A hexadecimal escape whose value does not fit the code unit
    /// (`err_hex_escape_too_large`).
    HexEscapeTooLarge,
    /// An octal escape whose value does not fit the code unit
    /// (`err_octal_escape_too_large`).
    OctalEscapeTooLarge,
    /// `\u`/`\U` with too few digits, or `\N{` without `}`, inside a
    /// literal (`err_ucn_escape_incomplete`).
    UcnEscapeIncomplete,
    /// `\N{name}` whose name matches a character only under UAX #44-LM2
    /// loose matching (`err_invalid_ucn_name` +
    /// `note_invalid_ucn_name_loose_matching`); recovers with `ch`.
    UcnNameLooseMatch {
        /// The character the name loosely matches.
        ch: char,
    },
    /// A character that needs more than one code unit in a character
    /// literal (`err_character_too_large`).
    CharTooLarge,
    /// An ordinary character literal with 2, 3 or more than 4 c-chars
    /// (`warn_multichar_character_literal`).
    MultiCharLiteral,
    /// An ordinary character literal with exactly 4 c-chars
    /// (`warn_four_char_character_literal`, off by default in clang).
    FourCharLiteral,
    /// A multicharacter literal with more c-chars than `int` holds
    /// (`warn_char_constant_too_large`).
    CharConstantTooLong,
    /// More than one c-char in a `L`, `u8`, `u` or `U` character literal
    /// (`err_multichar_character_literal`).
    MultiCharNotAllowed,
    /// An encoding prefix on an unevaluated string
    /// (`err_unevaluated_string_prefix`).
    UnevaluatedStringPrefix,
    /// A numeric or unknown escape sequence in an unevaluated string
    /// (`err_unevaluated_string_invalid_escape_sequence`).
    UnevaluatedStringEscape {
        /// The character after the backslash.
        ch: char,
    },
    /// A ud-suffix on an unevaluated string (`err_unevaluated_string_udl`).
    UnevaluatedStringUdSuffix,

    // ----- header names -----
    /// `'`, `\`, `//` or `/*` in a header name, or `"` in a `<…>` one:
    /// conditionally-supported ([lex.header]/2); kiln treats them as
    /// ordinary characters. clang accepts them silently.
    HeaderNameSpecialChars {
        /// The character or sequence.
        seq: &'static str,
    },

    // ----- numbers (recorded by `literal::classify_number`) -----
    /// A digit (or hex letter) not valid for the literal's base
    /// (`err_invalid_digit`).
    InvalidDigit {
        /// The character.
        ch: char,
        /// 2, 8 or 10.
        radix: u8,
    },
    /// A suffix that is neither standard nor a valid ud-suffix
    /// (`err_invalid_suffix_constant`).
    InvalidSuffix {
        /// Whether the literal is a floating one.
        floating: bool,
    },
    /// `e`/`E`/`p`/`P` with no digits (`err_exponent_has_no_digits`).
    ExponentNoDigits,
    /// A hexadecimal floating literal without a `p` exponent
    /// (`err_hex_constant_requires` exponent).
    HexFloatNoExponent,
    /// `0x.p1`: no significand digits (`err_hex_constant_requires`
    /// significand).
    HexFloatNoSignificand,
    /// A `'` not between two digits (`err_digit_separator_not_between_digits`).
    DigitSeparatorPosition {
        /// At the end of a digit sequence (else at the start).
        at_end: bool,
    },
    /// `0b…` before C++14 (`ext_binary_literal_cxx14`).
    BinaryLiteralExtension,
    /// A hexadecimal floating literal before C++17 (`ext_hex_literal_invalid`).
    HexFloatExtension,
    /// A `z`/`Z` suffix before C++23 (`ext_cxx23_size_t_suffix`).
    SizeSuffixExtension,
    /// An integer literal whose value does not fit any integer type
    /// (`err_integer_literal_too_large`); recorded by callers of
    /// [`crate::literal::IntegerLiteral::value`].
    IntegerTooLarge,

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
            | K::UcnUnknownName
            | K::EscapeNoDigits { .. }
            | K::EscapeMissingBrace { .. }
            | K::DelimitedEscapeEmpty
            | K::DelimitedEscapeUnterminated
            | K::DelimitedEscapeInvalidDigit { .. }
            | K::HexEscapeTooLarge
            | K::OctalEscapeTooLarge
            | K::UcnEscapeIncomplete
            | K::UcnNameLooseMatch { .. }
            | K::CharTooLarge
            | K::MultiCharNotAllowed
            | K::UnevaluatedStringPrefix
            | K::UnevaluatedStringEscape { .. }
            | K::UnevaluatedStringUdSuffix
            | K::InvalidDigit { .. }
            | K::InvalidSuffix { .. }
            | K::ExponentNoDigits
            | K::HexFloatNoExponent
            | K::HexFloatNoSignificand
            | K::DigitSeparatorPosition { .. }
            | K::IntegerTooLarge => LexSeverity::Error,
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
            | K::SpaceshipCompat
            | K::UnknownEscape { .. }
            | K::MultiCharLiteral
            | K::CharConstantTooLong => LexSeverity::Warning,
            K::UnicodeWhitespace { .. }
            | K::BackslashNewlineEof
            | K::NoNewlineAtEof
            | K::MultiLineLineComment
            | K::UcnDelimitedExtension
            | K::UcnNamedExtension
            | K::DollarInIdentifier
            | K::NonstandardEscape { .. }
            | K::DelimitedEscapeExtension
            | K::FourCharLiteral
            | K::HeaderNameSpecialChars { .. }
            | K::BinaryLiteralExtension
            | K::HexFloatExtension
            | K::SizeSuffixExtension => LexSeverity::Extension,
        }
    }
}
