// Diagnostic ids `lex_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

lex_backslash_newline_eof = Extension (0) "backslash-newline at end of file";
lex_backslash_newline_space = Warning [BackslashNewlineEscape] (0) "backslash and newline separated by space";
lex_char_not_allowed_in_identifier = Error (1) "character <U+{0}> not allowed in an identifier";
lex_delimited_escape_ext = Extension [DelimitedEscapeSequenceExtension] (1) "{0:delimited|named} escape sequences are a C++23 extension";
lex_dollar_in_identifier = Extension [DollarInIdentifierExtension] (0) "'$' in identifier";
lex_empty_char = Error (0) "empty character constant";
lex_empty_character = ExtWarn [InvalidPpToken] (0) "empty character constant";
lex_escaped_newline_block_comment_end = Warning [Comment] (0) "escaped newline between */ characters at block comment end";
lex_identifier_not_nfc = WarningOff (0) "identifier is not in Normalization Form C";
lex_invalid_char = Error (1) "unexpected character <U+{0}>";
lex_invalid_digit = Error (2) "invalid digit '{0}' in {1} constant";
lex_invalid_suffix = Error (2) "invalid suffix '{0}' on {1:integer|floating} constant";
lex_invalid_utf8 = Error (0) "source file is not valid UTF-8";
lex_literal_error = Error (1) "{0}";
lex_literal_warning = Warning (1) "{0}";
lex_multi_line_line_comment = Extension [Comment] (0) "multi-line // comment";
lex_multichar = Warning [Multichar] (0) "multi-character character constant";
lex_nested_block_comment = Warning [Comment] (0) "'/*' within block comment";
lex_no_newline_eof = WarningOff [NewlineEof] (0) "no newline at end of file";
lex_null_character = Warning [NullCharacter] (0) "null character ignored";
lex_raw_delim_invalid_char = Error (1) "invalid character '{0}' character in raw string delimiter; use PREFIX( )PREFIX to delimit raw string";
lex_raw_delim_too_long = Error (0) "raw string delimiter longer than 16 characters; use PREFIX( )PREFIX to delimit raw string";
lex_reserved_ud_suffix = WarningError [ReservedUserDefinedLiteral] (0) "invalid suffix on literal; C++11 requires a space between literal and identifier";
lex_spaceship_compat = WarningOff [Cxx20Compat] (0) "'<=>' is a single token in C++20; add a space to avoid a change in behavior";
lex_string_concat_mixed_suffix = Error (2) "differing user-defined suffixes ('{0}' and '{1}') in string literal concatenation";
lex_trigraph_converted = Warning [Trigraphs] (1) "trigraph converted to '{0}' character";
lex_trigraph_ignored = Warning [Trigraphs] (0) "trigraph ignored";
lex_ucn_basic_scs = Error (1) "character '{0}' cannot be specified by a universal character name";
lex_ucn_control_character = Error (0) "universal character name refers to a control character";
lex_ucn_delimited_empty = Warning [Unicode] (0) "empty delimited universal character name; treating as '\\' 'u' '{{' '}}'";
lex_ucn_delimited_incomplete = Warning [Unicode] (0) "incomplete delimited universal character name; treating as '\\' 'u' '{{' identifier";
lex_ucn_incomplete = Warning [Unicode] (0) "incomplete universal character name; treating as '\\' followed by identifier";
lex_ucn_invalid = Error (0) "invalid universal character";
lex_ucn_no_digits = Warning [Unicode] (1) "\\{0} used with no following hex digits; treating as '\\' followed by identifier";
lex_ucn_too_large = Error (0) "hex escape sequence out of range";
lex_ucn_unknown_name = Error (1) "'{0}' is not a valid Unicode character name";
lex_unicode_whitespace = ExtWarn [UnicodeWhitespace] (0) "treating Unicode character as whitespace";
lex_unknown_escape = Warning [UnknownEscapeSequence] (1) "unknown escape sequence '\\{0}'";
lex_unterminated_char = ExtWarn [InvalidPpToken] (0) "missing terminating ' character";
lex_unterminated_comment = Error (0) "unterminated /* comment";
lex_unterminated_raw_string = Error (1) "raw string missing terminating delimiter ){0}\"";
lex_unterminated_string = ExtWarn [InvalidPpToken] (0) "missing terminating '\"' character";
