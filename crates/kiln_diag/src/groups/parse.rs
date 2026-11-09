// Warning groups named by `parse_*` ids (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

AmbiguousEllipsis = "ambiguous-ellipsis" [];
BitIntExtension = "bit-int-extension" [];
C11Extensions = "c11-extensions" [];
Cxx20Extensions = "c++20-extensions" [];
Cxx23LambdaAttributes = "c++23-lambda-attributes" [];
Cxx26Extensions = "c++26-extensions" [];
CxxAttributeExtension = "cxx-attribute-extension" [];
DeprecatedMissingCommaVariadicParameter = "deprecated-missing-comma-variadic-parameter" [];
DeprecatedType = "deprecated-type" [];
DuplicateDeclSpecifier = "duplicate-decl-specifier" [];
EmptyDecomposition = "empty-decomposition" [];
ExpansionStmtMissingBraces = "expansion-stmt-missing-braces" [];
GnuAutoType = "gnu-auto-type" [];
GnuCaseRange = "gnu-case-range" [];
GnuLabelAsValue = "gnu-label-as-value" [];
IgnoredPragmas = "ignored-pragmas" [];
MissingTemplateArgListAfterTemplateKw = "missing-template-arg-list-after-template-kw" [];
UnknownAttributes = "unknown-attributes" [];
VexingParse = "vexing-parse" [];
