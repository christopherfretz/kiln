// Warning groups with subgroups, and `Pedantic` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

All = "all" [Most, Switch, SwitchBool, VlaCxxExtension];
Asm = "asm" [AsmOperandWidths];
Attributes = "attributes" [UnknownAttributes, IgnoredAttributes];
BitfieldConstantConversion = "bitfield-constant-conversion" [SingleBitBitfieldConstantConversion];
C99Extensions = "c99-extensions" [C99Designator];
ConstantConversion = "constant-conversion" [BitfieldConstantConversion];
Conversion = "conversion" [LiteralConversion, ConstantConversion, NullConversion];
ConversionNull = "conversion-null" [NullConversion];
Cxx11Extensions = "c++11-extensions" [Cxx11LongLong];
Cxx14Extensions = "c++14-extensions" [Cxx14BinaryLiteral];
CxxPre20CompatPedantic = "pre-c++20-compat-pedantic" [CxxPre20Compat];
Dangling = "dangling" [DanglingField, ReturnStackAddress];
Deprecated = "deprecated" [DeprecatedArrayCompare, DeprecatedDeclarations, DeprecatedLiteralOperator, DeprecatedMissingCommaVariadicParameter, DeprecatedRedundantConstexprStaticDef, DeprecatedThisCapture, DeprecatedVolatile];
Extra = "extra" [InitializerOverrides, UnusedButSetParameter];
Format = "format" [FormatExtraArgs, FormatInsufficientArgs];
Gnu = "gnu" [GnuAlignofExpression, GnuCaseRange, GnuDesignator, GnuEmptyStruct, GnuFlexibleArrayInitializer, GnuFlexibleArrayUnionMember, GnuVariableSizedTypeNotAtEnd, VlaExtension, GnuFoldingConstant, GnuImaginaryConstant, GnuIncludeNext, GnuLineMarker, GnuPointerArith, GnuStringLiteralOperatorTemplate, GnuZeroLineDirective, GnuZeroVariadicMacroArguments, ZeroLengthArray];
LoopAnalysis = "loop-analysis" [ForLoopAnalysis, RangeLoopAnalysis];
Most = "most" [Comment, Format, IntInBoolContext, Multichar, RangeLoopConstruct, Reorder, ReturnType, TautologicalCompare, Trigraphs, Uninitialized, UnknownPragmas, Unused, OverloadedVirtual];
Narrowing = "narrowing" [Cxx11Narrowing, FloatConversionRank];
OverrideInit = "override-init" [InitializerOverrides];
Pedantic = "pedantic" [];
PointerArith = "pointer-arith" [GnuPointerArith];
Pragmas = "pragmas" [UnknownPragmas, IgnoredPragmas];
RangeLoopAnalysis = "range-loop-analysis" [RangeLoopConstruct, RangeLoopBindReference];
Reorder = "reorder" [ReorderCtor, ReorderInitList];
TautologicalCompare = "tautological-compare" [TautologicalConstantCompare];
UnevaluatedExpression = "unevaluated-expression" [PotentiallyEvaluatedExpression];
Uninitialized = "uninitialized" [StaticSelfInit];
UnreachableCode = "unreachable-code" [UnreachableCodeFallthrough, UnreachableCodeLoopIncrement];
UnreachableCodeAggressive = "unreachable-code-aggressive" [UnreachableCode, UnreachableCodeBreak, UnreachableCodeReturn];
Unused = "unused" [UnusedValue, UnusedVariable, UnusedButSetVariable, UnusedFunction];
UnusedFunction = "unused-function" [UnneededInternalDeclaration];
UnusedTemplate = "unused-template" [UnneededInternalDeclaration];
UnusedValue = "unused-value" [UnusedComparison, UnusedResult, UnevaluatedExpression];
UnusedVariable = "unused-variable" [UnusedConstVariable];
VlaExtension = "vla-extension" [VlaCxxExtension];
