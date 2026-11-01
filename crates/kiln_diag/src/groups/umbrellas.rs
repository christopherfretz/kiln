// Warning groups with subgroups, and `Pedantic` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

All = "all" [Most, Switch, SwitchBool];
Asm = "asm" [AsmOperandWidths];
Attributes = "attributes" [UnknownAttributes, IgnoredAttributes];
BitfieldConstantConversion = "bitfield-constant-conversion" [SingleBitBitfieldConstantConversion];
C99Extensions = "c99-extensions" [C99Designator];
ConstantConversion = "constant-conversion" [BitfieldConstantConversion];
Conversion = "conversion" [LiteralConversion, ConstantConversion];
Cxx11Extensions = "c++11-extensions" [Cxx11LongLong];
Cxx14Extensions = "c++14-extensions" [Cxx14BinaryLiteral];
Dangling = "dangling" [DanglingField, ReturnStackAddress];
Deprecated = "deprecated" [DeprecatedDeclarations, DeprecatedRedundantConstexprStaticDef, DeprecatedThisCapture];
Extra = "extra" [InitializerOverrides, UnusedButSetParameter];
Gnu = "gnu" [GnuCaseRange, GnuDesignator, GnuFoldingConstant, GnuImaginaryConstant, GnuIncludeNext, GnuLineMarker, GnuPointerArith, GnuStringLiteralOperatorTemplate, GnuZeroLineDirective, GnuZeroVariadicMacroArguments, VlaExtension];
LoopAnalysis = "loop-analysis" [ForLoopAnalysis, RangeLoopAnalysis];
Most = "most" [Comment, UnknownPragmas, OverloadedVirtual, UnevaluatedExpression, Reorder, Uninitialized, RangeLoopConstruct, Unused];
Narrowing = "narrowing" [Cxx11Narrowing];
OverrideInit = "override-init" [InitializerOverrides];
Pedantic = "pedantic" [];
PointerArith = "pointer-arith" [GnuPointerArith];
Pragmas = "pragmas" [UnknownPragmas, IgnoredPragmas];
RangeLoopAnalysis = "range-loop-analysis" [RangeLoopConstruct, RangeLoopBindReference];
Reorder = "reorder" [ReorderCtor, ReorderInitList];
UnevaluatedExpression = "unevaluated-expression" [PotentiallyEvaluatedExpression];
Uninitialized = "uninitialized" [StaticSelfInit];
UnreachableCode = "unreachable-code" [UnreachableCodeFallthrough, UnreachableCodeLoopIncrement];
UnreachableCodeAggressive = "unreachable-code-aggressive" [UnreachableCode, UnreachableCodeBreak, UnreachableCodeReturn];
Unused = "unused" [UnusedValue, UnusedVariable, UnusedButSetVariable, UnusedFunction];
UnusedFunction = "unused-function" [UnneededInternalDeclaration];
UnusedTemplate = "unused-template" [UnneededInternalDeclaration];
UnusedValue = "unused-value" [UnusedComparison, UnusedResult];
UnusedVariable = "unused-variable" [UnusedConstVariable];
VlaExtension = "vla-extension" [VlaCxxExtension];
