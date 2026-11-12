// Diagnostic ids `lit_*` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

lit_bad_char_encoding = Error (0) "illegal character encoding in character literal";
lit_bad_char_encoding_warn = ExtWarn [InvalidSourceEncoding] (0) "illegal character encoding in character literal";
lit_bad_string_encoding = Error (0) "illegal character encoding in string literal";
lit_bad_string_encoding_warn = ExtWarn [InvalidSourceEncoding] (0) "illegal character encoding in string literal";
lit_binary_ext = Extension [Cxx14BinaryLiteral] (0) "binary integer literals are a C++14 extension";
// WP-1331: a `wb`/`uwb` suffix from C23 on, off by default (clang 19/22
// `warn_c23_compat_bitint_suffix`).
lit_bit_int_suffix_compat = WarningOff [CPre23Compat] (0) "'_BitInt' suffix for literals is incompatible with C standards before C23";
// WP-1331: a `wb`/`uwb` suffix before C23 (`ext_c23_bitint_suffix`).
lit_bit_int_suffix_ext = ExtWarn [C23Extensions] (0) "'_BitInt' suffix for literals is a C23 extension";
lit_char_constant_too_long = Warning (0) "character constant too long for its type";
lit_char_too_large = Error (0) "character too large for enclosing character literal type";
lit_delimited_escape_empty = Error (0) "delimited escape sequence cannot be empty";
lit_delimited_escape_invalid = Error (1) "invalid digit '{0}' in escape sequence";
lit_digit_separator = Error (1) "digit separator cannot appear at {0:start|end} of digit sequence";
lit_escape_missing_brace = Error (1) "expected '{{' after '\\{0}' escape sequence";
lit_escape_no_digits = Error (1) "\\{0} used with no following hex digits";
lit_exponent_no_digits = Error (0) "exponent has no digits";
lit_float_suffix_ext = Extension [Cxx23Extensions] (1) "'{0}' suffix on a floating literal is a C++23 extension";
lit_float_suffix_gnu = Extension (0) "non-standard suffix on floating constant";
lit_four_char = WarningOff [FourByteMultiChar] (0) "multi-character character constant";
lit_hex_escape_too_large = Error (0) "hex escape sequence out of range";
lit_hex_float_ext = Extension [Cxx17Extensions] (0) "hexadecimal floating literals are a C++17 feature";
lit_hex_float_requires = Error (1) "hexadecimal floating literal requires {0:an exponent|a significand}";
lit_invalid_digit = Error (2) "invalid digit '{0}' in {1:binary|octal|decimal} constant";
lit_invalid_suffix = Error (2) "invalid suffix '{0}' on {1:integer|floating} constant";
lit_multichar = Warning [Multichar] (0) "multi-character character constant";
lit_multichar_not_allowed = Error (1) "{0:wide|Unicode} character literals may not contain multiple characters";
lit_nonstandard_escape = Extension (1) "use of non-standard escape character '\\{0}'";
lit_octal_escape_too_large = Error (0) "octal escape sequence out of range";
lit_size_t_ext = ExtWarn [Cxx23Extensions] (0) "'size_t' suffix for literals is a C++23 extension";
lit_ucn_escape_incomplete = Error (0) "incomplete universal character name";
lit_unevaluated_escape = Error (1) "invalid escape sequence '{0}' in an unevaluated string literal";
lit_unevaluated_prefix = Error (0) "an unevaluated string literal cannot have an encoding prefix";
lit_unevaluated_udl = Error (0) "an unevaluated string literal cannot be a user-defined literal";
lit_unknown_escape = ExtWarn [UnknownEscapeSequence] (1) "unknown escape sequence '\\{0}'";
