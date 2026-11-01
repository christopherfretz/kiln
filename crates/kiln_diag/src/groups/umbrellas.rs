// Warning groups with subgroups, and `Pedantic` (spec/diag.md §2.4).
// One row per line, sorted by name; a comment directly above a row
// belongs to it. `cargo xtask diag-split` maintains this layout.

All = "all" [Most, Switch, SwitchBool, VlaCxxExtension];
Asm = "asm" [AsmOperandWidths];
Attributes = "attributes" [UnknownAttributes, IgnoredAttributes];
C99Extensions = "c99-extensions" [C99Designator];
Conversion = "conversion" [LiteralConversion, ConstantConversion];
Cxx11Extensions = "c++11-extensions" [Cxx11LongLong];
Cxx14Extensions = "c++14-extensions" [Cxx14BinaryLiteral];
Dangling = "dangling" [DanglingField, ReturnStackAddress];
Deprecated = "deprecated" [DeprecatedDeclarations, DeprecatedRedundantConstexprStaticDef, DeprecatedThisCapture];
Gnu = "gnu" [GnuCaseRange, GnuDesignator, VlaExtension, GnuFoldingConstant, GnuImaginaryConstant, GnuIncludeNext, GnuLineMarker, GnuPointerArith, GnuStringLiteralOperatorTemplate, GnuZeroLineDirective, GnuZeroVariadicMacroArguments];
Most = "most" [Comment, Multichar, RangeLoopConstruct, Reorder, ReturnType, Trigraphs, Uninitialized, UnknownPragmas, Unused, OverloadedVirtual];
Narrowing = "narrowing" [Cxx11Narrowing];
OverrideInit = "override-init" [InitializerOverrides];
Pedantic = "pedantic" [];
PointerArith = "pointer-arith" [GnuPointerArith];
Pragmas = "pragmas" [UnknownPragmas, IgnoredPragmas];
RangeLoopAnalysis = "range-loop-analysis" [RangeLoopConstruct, RangeLoopBindReference];
Reorder = "reorder" [ReorderCtor, ReorderInitList];
UnevaluatedExpression = "unevaluated-expression" [PotentiallyEvaluatedExpression];
Uninitialized = "uninitialized" [StaticSelfInit];
UnreachableCode = "unreachable-code" [UnreachableCodeFallthrough];
UnreachableCodeAggressive = "unreachable-code-aggressive" [UnreachableCode];
Unused = "unused" [UnusedValue, UnusedVariable, UnusedButSetVariable];
UnusedValue = "unused-value" [UnusedComparison, UnusedResult, UnevaluatedExpression];
UnusedVariable = "unused-variable" [UnusedConstVariable];
VlaExtension = "vla-extension" [VlaCxxExtension];
