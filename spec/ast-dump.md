# spec: the kiln AST dump format

Status: **design** (WP-040). Normative for M2 (§1–§4, §6, §7); §5 is a
sketch finalized by the M3 dump work package. The dump renders the AST of
`spec/ast.md`; types print per `spec/types.md` §12. Node class names,
children order and tree glyphs are clang 19's (`-ast-dump`), so clang is
usable as an oracle (§6).

## 1. Purpose and invocation
The dump is the **test contract** between the parser+sema and everything
else: conformance goldens, the dump loader (ast.md §12, WP E), and
differential tests against clang. It is deterministic, address-free,
location-free by default, and stable: a format change is a spec change.
```
kiln -fsyntax-only -fdump-ast[=<opt>,...] [-fdump-ast-filter=<substr>] t.cpp   # to stdout
  opts: ids        print node ids (#d12, #e5, #s3) and decl references by id
        locs       print @L:C (main file) / @file:L:C (other files) after the class name
        canonical  always print 'sugared':'canonical' pairs
        implicit   include implicit decls (implicit typedefs, injected class names, implicit members)
        all-files  include top-level decls from included files (default: main file only)
```
Default output is the TU's top-level decls located in the main file.
With a filter, every named decl whose qualified name contains the
substring is printed as its own tree, preceded by `Dumping <qualname>:`.

## 2. Lines
```
dump   := tree*
tree   := line(root) child*
line   := prefix Class [' #'id] [' @'loc] (' ' field)*   -- no trailing whitespace
prefix := ('| ' | '  ')* ('|-' | '`-')                  -- clang's glyphs; empty for a root
field  := flag | 'name' | name | type | '<' CastKind '>' | key '=' value | 'op' | label
type   := "'" printed-type "'" [":'" canonical "'"]     -- canonical only when the strings differ
```
- A child slot that is absent is not printed; statements with optional
  slots print `has_init`, `has_var`, `has_cond`, `has_inc`, `has_else`
  flags so the remaining children are unambiguous (clang does this for
  `IfStmt`/`SwitchStmt`/`WhileStmt`; kiln also for `ForStmt` instead of
  clang's `<<<NULL>>>` lines).
- Labeled children print `label: ` before the class name
  (`array_filler: ImplicitValueInitExpr 'int'`).
- `ExprStmt` is elided (its expression is printed in its place).
- Decl names print bare; references to decls print `Kind 'name' 'type'`
  (`ParmVar 'x' 'int'`); with `ids`, `ParmVar #d4 'x' 'int'`.
- Strings print in double quotes with clang's escapes for `\\ \" \n \t
  \r \0 \a \b \f \v`, printable ASCII verbatim, every other code unit as a
  delimited escape `\x{HH}` (`u8`/`u`/`U`/`L` prefix as written).
- Floats and other values print per ast.md §9.3.
- Types print per spec/types.md §12 (as clang).

## 3. M2 node lines (normative)
| class | line (fields in this order; `[]` = optional) |
|---|---|
| `TranslationUnitDecl` | `TranslationUnitDecl` |
| `TypedefDecl` | `TypedefDecl [implicit] [referenced] Name 'type'` (no type-tree children) |
| `CXXRecordDecl` | `CXXRecordDecl [implicit] [used\|referenced] struct\|class\|union [Name] [definition]` |
| `FieldDecl` | `FieldDecl [referenced] name 'type'` [child: bit-width `ConstantExpr`] |
| `EnumDecl` | `EnumDecl [class\|struct] Name ['fixed-underlying-type']` |
| `EnumConstantDecl` | `EnumConstantDecl [referenced] Name 'type' value=N` [child: initializer] |
| `LinkageSpecDecl` | `LinkageSpecDecl C\|C++` |
| `FunctionDecl` | `FunctionDecl [implicit] [used\|referenced] [constexpr\|consteval] name 'type' [static\|extern] [inline] [implicit-inline] [deleted\|default]`; children: `ParmVarDecl`s, body |
| `ParmVarDecl` | `ParmVarDecl [used\|referenced] [name] 'type'` |
| `VarDecl` | `VarDecl [used\|referenced] name 'type' [static\|extern] [constexpr] [cinit\|callinit\|listinit] [value=V]` (value printed when `VarData.eval` holds one) |
| `StaticAssertDecl` | `StaticAssertDecl` children: condition, message (`StringLiteral unevaluated "msg"`) |
| `IntegerLiteral` | `IntegerLiteral 'type' N` |
| `FloatingLiteral` | `FloatingLiteral 'type' 1.5e0` |
| `CharacterLiteral` | `CharacterLiteral 'type' N` |
| `StringLiteral` | `StringLiteral 'type' lvalue "text"` |
| `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr` | `... 'bool' true`, `... 'std::nullptr_t'` |
| `DeclRefExpr` | `DeclRefExpr 'type' [lvalue] Kind 'name' 'type'` |
| `MemberExpr` | `MemberExpr 'type' [lvalue] .name\|->name` |
| `ImplicitCastExpr` | `ImplicitCastExpr 'type' [lvalue\|xvalue] <Kind> [part_of_explicit_cast]` |
| `CStyleCastExpr` | `CStyleCastExpr 'type' <Kind>` |
| `UnaryOperator` | `UnaryOperator 'type' [lvalue] prefix\|postfix 'op' [cannot overflow]` |
| `BinaryOperator` | `BinaryOperator 'type' [lvalue] 'op'` |
| `CompoundAssignOperator` | `CompoundAssignOperator 'type' lvalue 'op' ComputeLHSTy='T' ComputeResultTy='T'` |
| `ConditionalOperator`, `ParenExpr`, `ArraySubscriptExpr`, `CallExpr` | class, type, category; children in source order (callee first) |
| `UnaryExprOrTypeTraitExpr` | `UnaryExprOrTypeTraitExpr 'type' sizeof\|alignof 'operand-type'` (or expression child) |
| `InitListExpr` | `InitListExpr 'type' [field=name]`; `array_filler:` child first, then elements |
| `ImplicitValueInitExpr` | `ImplicitValueInitExpr 'type'` |
| `ConstantExpr` | `ConstantExpr 'type' [lvalue] value=V` |
| statements | `CompoundStmt`, `DeclStmt`, `NullStmt`, `ReturnStmt`, `BreakStmt`, `ContinueStmt`, `DefaultStmt`, `CaseStmt`, `IfStmt [has_init] [has_var] [has_else]`, `SwitchStmt [has_init] [has_var]`, `WhileStmt [has_var]`, `DoStmt`, `ForStmt [has_init] [has_var] [has_cond] [has_inc]` |

## 4. M2 example
Each line below was checked against clang 19's dump of the same file
after applying the deviations listed after it.
```cpp
struct P { int x; int y; };
enum E { A, B = 5 };
constexpr int sq(int x) { return x * x; }
static_assert(sq(3) == 9, "msg");
typedef int Arr[3];
const Arr ca = {1, 2, 3};
long f(char c, int *p) {
  int a[3] = {1};
  P q = {1, 2};
  for (int i = 0; i < 3; ++i) a[i] += c;
  if (p) return q.x;
  return (long)sizeof(P) + f(c, nullptr);
}
```
```
TranslationUnitDecl
|-CXXRecordDecl referenced struct P definition
| |-FieldDecl referenced x 'int'
| `-FieldDecl y 'int'
|-EnumDecl E
| |-EnumConstantDecl A 'E' value=0
| `-EnumConstantDecl B 'E' value=5
|   `-ImplicitCastExpr 'unsigned int' <IntegralCast>
|     `-ConstantExpr 'int' value=5
|       `-IntegerLiteral 'int' 5
|-FunctionDecl used constexpr sq 'int (int)' implicit-inline
| |-ParmVarDecl used x 'int'
| `-CompoundStmt
|   `-ReturnStmt
|     `-BinaryOperator 'int' '*'
|       |-ImplicitCastExpr 'int' <LValueToRValue>
|       | `-DeclRefExpr 'int' lvalue ParmVar 'x' 'int'
|       `-ImplicitCastExpr 'int' <LValueToRValue>
|         `-DeclRefExpr 'int' lvalue ParmVar 'x' 'int'
|-StaticAssertDecl
| |-BinaryOperator 'bool' '=='
| | |-CallExpr 'int'
| | | |-ImplicitCastExpr 'int (*)(int)' <FunctionToPointerDecay>
| | | | `-DeclRefExpr 'int (int)' lvalue Function 'sq' 'int (int)'
| | | `-IntegerLiteral 'int' 3
| | `-IntegerLiteral 'int' 9
| `-StringLiteral unevaluated "msg"
|-TypedefDecl referenced Arr 'int[3]'
|-VarDecl ca 'const Arr':'const int[3]' cinit value={1, 2, 3}
| `-InitListExpr 'const Arr':'const int[3]'
|   |-IntegerLiteral 'int' 1
|   |-IntegerLiteral 'int' 2
|   `-IntegerLiteral 'int' 3
`-FunctionDecl referenced f 'long (char, int *)'
  |-ParmVarDecl used c 'char'
  |-ParmVarDecl used p 'int *'
  `-CompoundStmt
    |-DeclStmt
    | `-VarDecl used a 'int[3]' cinit
    |   `-InitListExpr 'int[3]'
    |     |-array_filler: ImplicitValueInitExpr 'int'
    |     `-IntegerLiteral 'int' 1
    |-DeclStmt
    | `-VarDecl used q 'P' cinit
    |   `-InitListExpr 'P'
    |     |-IntegerLiteral 'int' 1
    |     `-IntegerLiteral 'int' 2
    |-ForStmt has_init has_cond has_inc
    | |-DeclStmt
    | | `-VarDecl used i 'int' cinit
    | |   `-IntegerLiteral 'int' 0
    | |-BinaryOperator 'bool' '<'
    | | |-ImplicitCastExpr 'int' <LValueToRValue>
    | | | `-DeclRefExpr 'int' lvalue Var 'i' 'int'
    | | `-IntegerLiteral 'int' 3
    | |-UnaryOperator 'int' lvalue prefix '++'
    | | `-DeclRefExpr 'int' lvalue Var 'i' 'int'
    | `-CompoundAssignOperator 'int' lvalue '+=' ComputeLHSTy='int' ComputeResultTy='int'
    |   |-ArraySubscriptExpr 'int' lvalue
    |   | |-ImplicitCastExpr 'int *' <ArrayToPointerDecay>
    |   | | `-DeclRefExpr 'int[3]' lvalue Var 'a' 'int[3]'
    |   | `-ImplicitCastExpr 'int' <LValueToRValue>
    |   |   `-DeclRefExpr 'int' lvalue Var 'i' 'int'
    |   `-ImplicitCastExpr 'int' <IntegralCast>
    |     `-ImplicitCastExpr 'char' <LValueToRValue>
    |       `-DeclRefExpr 'char' lvalue ParmVar 'c' 'char'
    |-IfStmt
    | |-ImplicitCastExpr 'bool' <PointerToBoolean>
    | | `-ImplicitCastExpr 'int *' <LValueToRValue>
    | |   `-DeclRefExpr 'int *' lvalue ParmVar 'p' 'int *'
    | `-ReturnStmt
    |   `-ImplicitCastExpr 'long' <IntegralCast>
    |     `-ImplicitCastExpr 'int' <LValueToRValue>
    |       `-MemberExpr 'int' lvalue .x
    |         `-DeclRefExpr 'P' lvalue Var 'q' 'P'
    `-ReturnStmt
      `-BinaryOperator 'long' '+'
        |-CStyleCastExpr 'long' <NoOp>
        | `-ImplicitCastExpr 'long' <IntegralCast> part_of_explicit_cast
        |   `-UnaryExprOrTypeTraitExpr 'unsigned long' sizeof 'P'
        `-CallExpr 'long'
          |-ImplicitCastExpr 'long (*)(char, int *)' <FunctionToPointerDecay>
          | `-DeclRefExpr 'long (char, int *)' lvalue Function 'f' 'long (char, int *)'
          |-ImplicitCastExpr 'char' <LValueToRValue>
          | `-DeclRefExpr 'char' lvalue ParmVar 'c' 'char'
          `-ImplicitCastExpr 'int *' <NullToPointer>
            `-CXXNullPtrLiteralExpr 'std::nullptr_t'
```
Deliberate differences from clang 19 `-ast-dump`: no addresses, ranges or
locations; no implicit decls (builtin typedefs, the injected
`CXXRecordDecl implicit struct P`) and no `DefinitionData` block (both
behind `implicit`); `value=` is inline on `ConstantExpr`,
`EnumConstantDecl` and `VarDecl` instead of a `value: Int 5` child;
`ForStmt` flags instead of `<<<NULL>>>`; the static-assert message is
`unevaluated` instead of `'NULL TYPE' lvalue`; no `TypedefDecl` type
tree; floats print `1.5e0`, not `1.500000e+00`. (`used`/`referenced`
are clang's, which marks a function `used` only from outside its own body;
they feed `-Wunused-*`, so kiln computes them identically.)

## 5. Templates and lambdas (sketch, M3; finalized by the M3 dump WP)
```
|-ClassTemplateDecl S
| |-TemplateTypeParmDecl referenced class depth 0 index 0 T
| |-NonTypeTemplateParmDecl referenced 'int' depth 0 index 1 N
| |-CXXRecordDecl struct S definition                      <- the pattern
| | `-FieldDecl a 'T[N]'
| `-ClassTemplateSpecializationDecl struct S definition implicit_instantiation
|   |-TemplateArgument type 'int'
|   |-TemplateArgument value 'int' 2
|   `-FieldDecl a 'int[2]' instantiated
|-FunctionTemplateDecl id
| |-TemplateTypeParmDecl referenced typename depth 0 index 0 T
| |-FunctionDecl id 'T (T)'                                <- the pattern
| `-FunctionDecl used id 'int (int)' implicit_instantiation
|   |-TemplateArgument type 'int'
|   `-...
`-...LambdaExpr '(lambda at t.cpp:3:35)'
    |-CXXRecordDecl implicit class definition lambda       <- closure, always printed
    | |-CXXMethodDecl used constexpr operator() 'int (int) const' inline
    | `-FieldDecl implicit 'int'                           <- one per capture
    |-ImplicitCastExpr 'int' <LValueToRValue>              <- capture initializers
    | `-DeclRefExpr 'int' lvalue Var 'k' 'int'
    `-CompoundStmt ...                                     <- body
```
Rules: a template decl prints its parameters, its pattern, then its
specializations in creation order (ast.md §8.3) — each
specialization's full tree is printed once, under its template; `instantiated` marks decls with
an `instantiated_from` link (with `ids`: `instantiated_from=#d17`);
dependent types print with the parameter names (sugar), and
`SubstTemplateTypeParm` sugar is transparent (prints the replacement).
`TemplateArgument value` covers clang's `integral`/`declaration`/
`nullptr`/`structural value` lines.

## 6. Differential testing against clang
`xtask clang-ast` converts `clang -Xclang -ast-dump=json` into this text
format (drop addresses/ranges and implicit nodes, fold `value:` children
into fields, reprint floats, apply the §4 deviations). A differential
test compares `kiln -fdump-ast` with the normalized clang
dump; known-divergent node classes are listed in the tool, never patched
in goldens by hand.

## 7. Harness outcome (proposal; amends spec/harness.md)
`//@ expect: ast` (optionally `//@ ast-filter: <substr>`): kiln runs with
`-fsyntax-only -fdump-ast`; pass iff exit status 0, `diag:` directives
match as for `compile-ok`, and stdout equals the golden `<test>.expected`
(refreshed by `--update`). Kiln-specific (SKIP under clang), except when
`oracle: clang` asks for the §6 differential comparison instead of a
golden.
