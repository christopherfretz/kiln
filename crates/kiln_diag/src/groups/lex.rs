// Warning groups named by `lex_*` ids (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

BackslashNewlineEscape = "backslash-newline-escape" [];
C99Compat = "c99-compat" [];
Comment = "comment" [];
Cxx20Compat = "c++20-compat" [];
DelimitedEscapeSequenceExtension = "delimited-escape-sequence-extension" [];
DollarInIdentifierExtension = "dollar-in-identifier-extension" [];
InvalidPpToken = "invalid-pp-token" [];
MathematicalNotationIdentifierExtension = "mathematical-notation-identifier-extension" [];
Multichar = "multichar" [];
NewlineEof = "newline-eof" [];
NullCharacter = "null-character" [];
ReservedUserDefinedLiteral = "reserved-user-defined-literal" [];
Trigraphs = "trigraphs" [];
Unicode = "unicode" [];
UnicodeWhitespace = "unicode-whitespace" [];
UnknownEscapeSequence = "unknown-escape-sequence" [];
