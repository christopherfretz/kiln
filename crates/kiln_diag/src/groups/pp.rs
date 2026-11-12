// Warning groups named by `pp_*` ids (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

BuiltinMacroRedefined = "builtin-macro-redefined" [];
C23Extensions = "c23-extensions" [];
// WP-1331: clang's C pre-C23 compatibility warnings (`#embed`, `wb`).
CPre23Compat = "pre-c23-compat" [];
Cxx11LongLong = "c++11-long-long" [];
DateTime = "date-time" [];
DisabledMacroExpansion = "disabled-macro-expansion" [];
EmbeddedDirective = "embedded-directive" [];
ExpansionToDefined = "expansion-to-defined" [];
ExtraTokens = "extra-tokens" [];
GnuIncludeNext = "gnu-include-next" [];
GnuLineMarker = "gnu-line-marker" [];
GnuZeroLineDirective = "gnu-zero-line-directive" [];
GnuZeroVariadicMacroArguments = "gnu-zero-variadic-macro-arguments" [];
IncludeNextAbsPath = "include-next-absolute-path" [];
IncludeNextOutsideHeader = "include-next-outside-header" [];
KeywordAsMacro = "keyword-macro" [];
MacroRedefined = "macro-redefined" [];
MalformedWarningCheck = "malformed-warning-check" [];
PoundPragmaMessage = "#pragma-messages" [];
PoundWarning = "#warnings" [];
PragmaOnceOutsideHeader = "pragma-once-outside-header" [];
PragmaSystemHeaderOutsideHeader = "pragma-system-header-outside-header" [];
ReservedMacroIdentifier = "reserved-macro-identifier" [];
Undef = "undef" [];
UnknownPragmas = "unknown-pragmas" [];
UnusedMacros = "unused-macros" [];
VariadicMacros = "variadic-macros" [];
