# spec: the kiln AST dump format

Status: **design** (WP-040); §1–§4 implemented (WP-044, `crates/kiln_ast/src/dump/`); §6/§7 implemented (WP-046, `xtask/src/clang_ast/`, `xtask/src/test/`). Normative for M2 (§1–§4, §6, §7); §5 is a
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
Default output is the TU's top-level decls located in the main file
(minus implicit ones). With a filter, every named decl whose qualified
name (`NamedDecl::printQualifiedName`: a function-local name is qualified
by its function's signature, `f(char, int *)::L::x`; unscoped enums and
linkage specs are transparent) contains the substring is printed as its
own tree, preceded by `Dumping <qualname>:` and followed by a blank line,
as clang's `-ast-dump-filter` prints it. The search does not descend into
a printed tree, and still skips implicit decls and (without `all-files`)
top-level decls of other files.

Library API (`kiln_ast::dump`; the driver calls `dump_to_string`):
```
pub fn dump<W: io::Write>(ast: &Ast, opts: &DumpOptions<'_>, out: W) -> io::Result<()>
pub fn dump_to_string(ast: &Ast, opts: &DumpOptions<'_>) -> String
pub struct DumpOptions<'a> {
    ids, locs, canonical_types, implicit, all_files: bool,  // the opts above
    filter: Option<Filter>,
    sources: Option<&'a SourceManager>,  // needed by `locs` and main-file filtering
    main_file: Option<FileId>,           // None: no main-file filtering, every loc names its file
}
pub struct Filter { name: Option<String>, kind: Option<DeclKind> }
```
`Filter::name` is `-fdump-ast-filter`; `Filter::kind` (library only)
additionally requires the decl's kind, and with no name it matches every
decl of that kind (the header still prints the qualified name). Without
`sources` no location prints and nothing is filtered by file. The writer
is fed whole lines from one reused buffer; output is a pure function of
the AST and the options.

## 2. Lines
```
dump   := tree*
tree   := line(root) child*
line   := prefix [label ': '] Class [' #'id] [' @'loc] [' prev=#d'id] (' ' field)*   -- no trailing whitespace
loc    := [file ':'] L ':' C ['[' [file ':'] L ':' C ']'] | '<invalid>'
prefix := ('| ' | '  ')* ('|-' | '`-')                  -- clang's glyphs; empty for a root
field  := flag | 'name' | name | type | '<' CastKind '>' | key '=' value | 'op' | label
type   := "'" printed-type "'" [":'" desugared "'"]     -- only when the strings differ
```
- Ids: `#dN` decls, `#eN` expressions, `#sN` statements, `#aN`
  attributes (`N` is the raw arena index). `prev=#dN` (a redeclaration's
  previous declaration) prints only with `ids`, as clang's `prev 0x...`
  prints only with addresses.
- Locations: `file:` is omitted for the main file (`DumpOptions::main_file`,
  compared after `#line` presumption). A location inside a macro
  expansion prints `expansion[spelling]` (`@3:1[2:15]`: the decl's name
  was spelled in the `#define` on line 2 and expanded on line 3). An
  unresolvable location prints `<invalid>`; an invalid (absent) location
  prints nothing.
- The second half of a type pair is clang's `desugaredQualType`: the type
  desugared **at the top level only** (`QualType::getSplitDesugaredType`),
  so `'Ptr':'const Int *'` keeps the pointee's sugar, and `'struct
  Anon':'Anon'` names an unnamed class by its typedef. When both strings
  are equal (sugar that prints like its target) one string is printed;
  clang 19's JSON never contains an equal pair. With `canonical`, the
  second half is the canonical type and is always printed.
- An lvalue or xvalue expression that designates a bit-field prints
  `bitfield` after its category (`MemberExpr 'int' lvalue bitfield .b`);
  the property propagates through `ParenExpr`, the left side of `=` and
  compound assignment, prefix `++`/`--`, the right side of `,` and both
  arms of `?:`.
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
  delimited escape `\x{hh}` in lowercase hex without leading zeros
  (`u8"\x{c3}\x{a9}"`, `u"\x{d83d}\x{de00}"`; `u8`/`u`/`U`/`L` prefix as
  written). Concatenated literals print as one. clang instead prints
  bytes and small code units in octal (`\303\251`) and large code points
  as `\U0001F600`.
- Floats and other values print per ast.md §9.3.
- Types print per spec/types.md §12 (as clang).

## 3. M2 node lines (normative)
Every decl line starts `Class [parent] [implicit] [used|referenced]
[invalid]` (`parent`: the semantic context differs from the lexical one —
an out-of-line member definition, a befriended declaration; with `ids`
`parent=#dN`; clang's `isImplicit`, `isUsed` (of the first declaration),
`isThisDeclarationReferenced`, `isInvalidDecl`; `used`
wins); this is `<head>` below, after the §2 ids and location.
Dependence flags are not printed (clang prints them only on dependent
expressions, M3). Every expression
line starts `Class 'type' [lvalue|xvalue [bitfield]]` (§2; prvalues print
no category).
| class | line (fields in this order; `[]` = optional) |
|---|---|
| `TranslationUnitDecl` | `TranslationUnitDecl` |
| `TypedefDecl`, `TypeAliasDecl` | `TypedefDecl <head> Name 'type'` (no type-tree children) |
| `NamespaceDecl` | `NamespaceDecl <head> [Name] [inline] [original Namespace 'Name']` (`original` on a reopening names the first declaration) |
| `NamespaceAliasDecl` | `NamespaceAliasDecl <head> Name Namespace 'Target'` or `... NamespaceAlias 'Target'` (the namespace or alias as written) |
| `UsingDirectiveDecl` | `UsingDirectiveDecl <head> Namespace 'Name'` (an unnamed namespace's implicit directive prints `implicit` and `Namespace ''`) |
| `UsingDecl` | `UsingDecl <head> qualifier::name` (the nested-name-specifier as written, `::N::x` or `N::x`) |
| `UsingShadowDecl` | `UsingShadowDecl <head> Kind 'name'[ 'type'[:'canonical']]` (always `implicit`, so printed only with implicit nodes; the target's kind, name and, for a value or typedef, its type) |
| `UsingEnumDecl` | `UsingEnumDecl <head> Enum 'Name'` |
| `CXXRecordDecl` | `CXXRecordDecl <head> [friend_undeclared] struct\|class\|union [Name] [definition]`; children: base-specifiers, attributes, `FinalAttr final` (the class-virt-specifier; clang adds it after the head's attributes), members. `friend_undeclared`: first declared by a `friend` declaration (it prints under its `FriendDecl`) |
| base-specifier | `[virtual ]public\|protected\|private 'type'[...]` (no class name; the semantic access: defaulted is `private` in a `class`, else `public`) |
| `FieldDecl` | `FieldDecl <head> [name] 'type' [mutable]` [child: bit-width `ConstantExpr`] [child: default member initializer] (an unnamed bit-field has no name) |
| `EnumDecl` | `EnumDecl <head> [class\|struct] Name ['fixed-underlying-type']` |
| `EnumConstantDecl` | `EnumConstantDecl <head> Name 'type' value=N` [child: initializer] |
| `LinkageSpecDecl` | `LinkageSpecDecl <head> C\|C++` (no flag for the braced form) |
| `FunctionDecl`, `CXXMethodDecl`, `CXXConstructorDecl`, `CXXDestructorDecl`, `CXXConversionDecl` | `Class <head> [constexpr\|consteval\|immediate] [friend\|friend_undeclared] name 'type' [static\|extern] [inline] [virtual] [pure] [default\|default_delete] [delete] [trivial] [noexcept-unevaluated] [implicit-inline]`; children: [`delete message:` `StringLiteral unevaluated "msg"`], [`Overrides:` line], `ParmVarDecl`s, `CXXCtorInitializer`s, body, attributes (clang's order: written attributes, then `OverrideAttr`, then `FinalAttr final`). `immediate` = an escalated immediate function. `friend` = declared by a friend declaration and also declared elsewhere, `friend_undeclared` = first declared by one (it prints under its `FriendDecl`). `virtual` = the specifier was written; `default` = `= default` (`default_delete`: found deleted); `delete` = `= delete`; `trivial` = a trivial special member; `noexcept-unevaluated` = the exception specification is not yet computed (an implicit or defaulted special member; with `ids` `noexcept-unevaluated #dN` names the member it belongs to). `inline` = the specifier was written; `implicit-inline` = inline without it (constexpr, consteval, deleted, defaulted on its first declaration, defined in its class, or a redeclaration of an inline function) |
| overridden methods | `Overrides: [ B::f 'type', C::f 'type' ]` (each overridden method by its class's simple name; the sugared type) |
| `CXXCtorInitializer` | `CXXCtorInitializer Field\|IndirectField 'name' 'type'` (a member), `CXXCtorInitializer 'B'` (a base, the canonical type) or `CXXCtorInitializer 'D'` (delegating); child: the initializer. Every member and base with an initialization has one, implicit ones included, in initialization order |
| `AccessSpecDecl` | `AccessSpecDecl <head> public\|protected\|private` |
| `FriendDecl` | `FriendDecl <head> ['type']` (a friend type; with no child unless the type is first declared here) or `FriendDecl <head>` with the befriended declaration as its child |
| `ParmVarDecl` | `ParmVarDecl <head> [name] 'type' [cinit] [pack]` (`cinit`: has a default argument, its child) |
| `VarDecl` | `VarDecl <head> name 'type' [static\|extern\|register] [tls\|tls_dynamic] [nrvo] [inline] [constexpr] [cinit\|callinit\|listinit\|parenlistinit] [value=V]` (the init style only with an initializer; `constinit` is an attribute in clang and is not printed); `value=` only on a `constexpr` variable with an initializer (a `const int` initialized by a constant prints none, as in clang) |
| `StaticAssertDecl` | `StaticAssertDecl <head>` children: condition, [message (`StringLiteral unevaluated "msg"`)] |
| attribute | `<Name>Attr [#aN] [@loc] [Inherited] [Implicit]` with clang's class name (`[[nodiscard]]` → `WarnUnusedResultAttr`); a kind without one prints `UnknownAttr kind=N`. Arguments are not printed (M2), except: `AsmLabelAttr [Inherited] "label" IsLiteralLabel` (the label's bytes unescaped, as clang) and `BuiltinAttr Implicit N` (the builtin id; the implicit builtin decl and its `extern "C"` `LinkageSpecDecl` print only with `implicit`). Implicit attributes print only with `implicit` |
| `IntegerLiteral` | `IntegerLiteral 'type' N` |
| `FloatingLiteral` | `FloatingLiteral 'type' 1.5e0` |
| `CharacterLiteral` | `CharacterLiteral 'type' N` (the code unit value, decimal) |
| `StringLiteral` | `StringLiteral 'type' lvalue "text"` (as an initializer of a `char` array clang 19 prints it as a prvalue) |
| `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr` | `... 'bool' true`, `... 'std::nullptr_t'` |
| `DeclRefExpr` | `DeclRefExpr 'type' [lvalue] Kind 'name' 'type' [(UsingShadow 'name')] [non_odr_use_constant\|non_odr_use_unevaluated\|non_odr_use_discarded]` (`(UsingShadow 'name')`: the name was found through a using-declaration, clang's `FoundDecl`) |
| `MemberExpr` | `MemberExpr 'type' [lvalue [bitfield]] .name\|->name` |
| `ImplicitCastExpr` | `ImplicitCastExpr 'type' [lvalue\|xvalue] <Kind> [part_of_explicit_cast]` |
| `CStyleCastExpr` | `CStyleCastExpr 'type' [lvalue\|xvalue] <Kind>` |
| `CXXFunctionalCastExpr` | `CXXFunctionalCastExpr 'type' [lvalue\|xvalue] functional cast to T <Kind>` (`T` is the type as written, the expression's sugared type) |
| `UnaryOperator` | `UnaryOperator 'type' [lvalue [bitfield]] prefix\|postfix 'op' [cannot overflow]` |
| `BinaryOperator` | `BinaryOperator 'type' [lvalue [bitfield]] 'op'` |
| `CompoundAssignOperator` | `CompoundAssignOperator 'type' lvalue [bitfield] 'op' ComputeLHSTy='T' ComputeResultTy='T'` |
| `ConditionalOperator`, `ParenExpr`, `ArraySubscriptExpr`, `CallExpr` | class, type, category; children in source order (callee first) |
| `UnaryExprOrTypeTraitExpr` | `UnaryExprOrTypeTraitExpr 'type' sizeof\|alignof 'operand-type'` (or expression child) |
| `CXXNoexceptExpr` | `CXXNoexceptExpr 'bool'`; one child, the unevaluated operand. The value is not printed (clang prints none) |
| `CXXStaticCastExpr`, `CXXDynamicCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr` | `Class 'type' [lvalue\|xvalue] static_cast<T> <Kind [(path)]>` (the written type, as `'type'` plus `&` for an lvalue and `&&` for an xvalue result) |
| `CXXFunctionalCastExpr` | `CXXFunctionalCastExpr 'type' [lvalue\|xvalue] functional cast to T <Kind>` |
| cast paths | a derived-to-base or base-to-derived cast kind prints its path: `<DerivedToBase (A -> virtual V)>` (each base class, `virtual` on a virtual step), also on `ImplicitCastExpr` and `CStyleCastExpr` |
| `CXXConstructExpr`, `CXXTemporaryObjectExpr` | `Class 'type' 'ctor-type' [list] [std::initializer_list] [zeroing] [immediate-escalating]`; children: the arguments |
| `CXXMemberCallExpr` | `CXXMemberCallExpr 'type' [lvalue\|xvalue]`; children: the `MemberExpr` callee, the arguments |
| `CXXThisExpr` | `CXXThisExpr 'type' [implicit] this` |
| `CXXDefaultArgExpr`, `CXXDefaultInitExpr` | `CXXDefaultArgExpr 'type'`; `CXXDefaultInitExpr 'type' has rewritten init`; child: the parameter's default argument or the member's initializer (printed under every use, as clang) |
| `MaterializeTemporaryExpr` | `MaterializeTemporaryExpr 'type' lvalue\|xvalue [extended by Var 'name' 'type']` |
| `CXXBindTemporaryExpr` | `CXXBindTemporaryExpr 'type' (CXXTemporary)` (clang adds an address) |
| `ExprWithCleanups` | `ExprWithCleanups 'type' [lvalue\|xvalue]` |
| `CXXTypeidExpr` | `CXXTypeidExpr 'type' lvalue [type_operand='T']` (or an expression child; `type_operand` is a kiln addition: clang 19 prints nothing) |
| `CXXParenListInitExpr` | `CXXParenListInitExpr 'type'`; `array_filler:` child first, then elements |
| `CXXInheritedCtorInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `OpaqueValueExpr` | class and type; an `ArrayInitLoopExpr` has the `OpaqueValueExpr` and the per-element initializer as children; an `OpaqueValueExpr` prints its source expression under every use (as clang) |
| `InitListExpr` | `InitListExpr 'type' [field=name]`; `array_filler:` child first, then elements; `field=` names the initialized member of a union |
| `ImplicitValueInitExpr` | `ImplicitValueInitExpr 'type'` |
| WP-064 forms | `CXXScalarValueInitExpr`, `CXXDefaultArgExpr`, `CXXThrowExpr`, `StmtExpr`, `CompoundLiteralExpr`, `OpaqueValueExpr`, `BinaryConditionalOperator`, `OffsetOfExpr`, `CXXTypeidExpr`: class, type, category only. Children: a `CXXDefaultArgExpr` the parameter's default argument; a `CXXThrowExpr` its operand, if any; a `StmtExpr` its `CompoundStmt`; a `CompoundLiteralExpr` its `InitListExpr`; an `OpaqueValueExpr` its source expression; a `BinaryConditionalOperator` the common expression, condition, true arm and false arm; an `OffsetOfExpr` its array index expressions; a `CXXTypeidExpr` its expression operand (a type operand is not printed) |
| `ConstantExpr` | `ConstantExpr 'type' value=V` (M2: integral, `bool` and enumeration values) |
| `CXXNewExpr` | `CXXNewExpr 'T *' [global] [array] Function 'operator new' 'type'` (clang's `operatorNewDecl` printed as a decl reference, kind, name and type; `array` for an array new; no placement, init-style or alignment flags, as clang 19). Children in clang's order: the array size (the first bound converted to `unsigned long`, an `IntegralCast` or a synthesized `IntegerLiteral` for `new T[]{...}`), the initializer (`InitListExpr`, `ImplicitValueInitExpr` for `()`, or the converted expression), then the placement arguments (WP-061a) |
| `CXXDeleteExpr` | `CXXDeleteExpr 'void' [global] [array] Function 'operator delete' 'type'` (`array` when written `delete[]`; clang's `isArrayForm`); one child, the operand converted to a pointer prvalue (WP-061a) |
| statements | `CompoundStmt`, `DeclStmt`, `NullStmt`, `ReturnStmt`, `BreakStmt`, `ContinueStmt`, `DefaultStmt`, `CaseStmt`, `IfStmt [has_init] [has_var] [has_else] [constexpr] [consteval\|!consteval]`, `SwitchStmt [has_init] [has_var]`, `WhileStmt [has_var]`, `DoStmt`, `ForStmt [has_init] [has_var] [has_cond] [has_inc]` |

Notes:
- Since C++23 a `return` of a local variable or parameter is an implicit
  move: the operand is `ImplicitCastExpr 'T' xvalue <NoOp>` around the
  `DeclRefExpr`, under the `LValueToRValue` cast, also for scalars.
- The kiln dump prints `mutable`, attributes and base-specifiers (lines
  above), but they are not in the M2 goldens.
- Namespaces, namespace aliases, using-directives, using-declarations,
  using-shadows and `using enum` are in the dump and the clang
  normalizer since WP-062 (goldens under
  `tests/conformance/ast/{namespace.udecl,namespace.alias,enum.udecl}/`).
  A namespace's members are found by `--filter N::x`; an inline
  namespace is transparent to the filter. Known difference: an
  enumerator's type, and a qualified enumeration type as written, print
  `'E':'N::E'` in kiln and `'N::E'` in clang when the enumeration is in
  a namespace or class (spec/types.md §15 R4), so goldens use global
  enumerations.
- The class-feature lines (member functions, ctor-initializers, access
  and friend declarations, bases, `Overrides:`, named and functional
  casts and the construction and temporary expressions, spec/sema-class.md
  §8.1) are WP-066a; kiln produces them once sema builds them (WP-065,
  066b/c, 078a).
- Not in the dump (an error in the clang normalizer, §6): attributes
  other than `override`/`final`, `thread_local`, overloaded operator calls
  (`CXXOperatorCallExpr`), `new`/`delete`, lambdas,
  `ConstantExpr` values of floating, pointer or lvalue type, and
  `long double` values; also clang's `noexcept-uninstantiated`,
  `ineligible` and `not_selected` member-function words (templates and
  constrained special members, M3) and a block-scope `extern`
  declaration's `parent` (clang prints it; kiln's dump does not model the
  separate semantic context and the normalizer drops it).

## 4. M2 example
This is the conformance test `tests/conformance/ast/dcl.dcl/spec-example.cpp`;
its golden is `cargo xtask clang-ast` of the file, byte for byte. (`ca` is
`const`, not `constexpr`, so it has no `value=`.)
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
|-VarDecl ca 'const Arr':'const int[3]' cinit
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

**Templates as written (landed, WP-069).** The lines below are normative;
specializations other than implicit class instantiations remain the
sketch above (WP-070b/071/072).
- `ClassTemplateDecl` / `FunctionTemplateDecl` / `VarTemplateDecl` /
  `TypeAliasTemplateDecl` `<name>`. The children are the parameters,
  then the pattern (`CXXRecordDecl`, `FunctionDecl`, `VarDecl`,
  `TypeAliasDecl`), then the requires-clause. A trailing
  requires-clause is the pattern function's last child.
- `TemplateTypeParmDecl [referenced] typename|class depth D index I
  [...] [<name>]`. A constrained parameter (`C U`) prints `class` plus a
  `ConceptSpecializationExpr` child. An abbreviated template's invented
  parameter (`C auto x`) prints `implicit` and the name
  `<param>:auto` (`x:auto`, clang's `-ast-dump` name for it).
- `NonTypeTemplateParmDecl [referenced] '<type>' depth D index I [...]
  [<name>]`, with the default argument as its child.
- `TemplateTemplateParmDecl depth D index I [...] [<name>]`. The
  children are the inner parameters (at depth D+1), then the default
  as a `TemplateArgument template` line.
- `ConceptDecl <name>`, with the constraint-expression as its child.
- `ConceptSpecializationExpr 'bool'`. The children are the written
  arguments as `TemplateArgument` lines.
- `RequiresExpr 'bool'`. Its parameters are `ParmVarDecl` children,
  followed by `SimpleRequirement` / `TypeRequirement '<type>'` /
  `CompoundRequirement [noexcept]` / `NestedRequirement`. Each
  requirement prints `dependent` when it is dependent.
- `TemplateArgument type '<type>' | expr | template`. A pack-expansion
  argument prints its pattern, without the `...` (`type 'Ts'`); this
  matches clang.

The dependent expressions use clang's dependent type, `'<dependent
type>'`:
- `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr '…' lvalue
  ./-><name>`, `UnresolvedLookupExpr '…' lvalue [(ADL)] = '<name>'
  [empty]`, `CXXUnresolvedConstructExpr '<type>'`, `PackExpansionExpr`,
  `CXXFoldExpr` and `SizeOfPackExpr 'unsigned long' <pack>`.
- An `UnresolvedLookupExpr` whose candidate set is all function
  templates, with dependent explicit arguments, is `'<dependent type>'`.
  Its overload set is `'<overloaded function type>'`.
- clang prints neither `DependentScopeDeclRefExpr` nor
  `CXXDependentScopeMemberExpr` explicit template arguments as children.
  The dump prints neither.
- **Expressions inside type names.** Wherever a type spells an
  expression (`decltype(e)`, an array bound, `I<e>`), the expression
  prints as clang's `printPretty`:
  - `T::value` and `T::template m<int>`, the latter printed `T::m<int>`;
    see "Known gaps" below.
  - `t.x` and `t->template y<1>`.
  - `f<T>` with the qualifier and arguments as written.
  - `T(a, b)` for an unresolved construct, `sizeof...(Ts)`, and folds as
    `(... + xs)`.

Known gaps:
- A `DependentScopeDeclRefExpr` does not store the `template` keyword.
- A function parameter named inside its own declarator's trailing
  return type (`auto f(T t) -> decltype(t.x)`) is unsupported, so the
  type prints with `...` for the expression.

**Implicit class instantiations (landed, WP-070a).** Normative:
- `ClassTemplateSpecializationDecl <tag> <name>`, plus `definition
  implicit_instantiation` once instantiated, is printed under its
  `ClassTemplateDecl` after the pattern, in creation order. A
  specialization that was named but never instantiated prints only the
  head. Children: one `TemplateArgument` per converted argument
  (`type '<type>'`, or `value '<param type>' <integer>` for an integral
  argument), the bases, then the instantiated members. There is no
  `instantiated` marker; it is the sketch's, not clang's.
- `SubstNonTypeTemplateParmExpr '<type>'` replaces a reference to a
  non-type parameter. Its children are the `NonTypeTemplateParmDecl`,
  then the replacement expression.
- Type names print as written with the canonical type after `:` when
  they differ (`'Arr<1 + 4>':'Arr<5>'`), and canonical argument lists
  omit defaulted arguments as clang 19 does.

## 6. Differential testing against clang
`cargo xtask clang-ast <file> [--compiler=] [--std=] [--implicit]
[--all-files] [--filter=<substr>] [-- <clang args>]` prints clang's AST of
`<file>` in this format: what `kiln -fsyntax-only -fdump-ast[=implicit,
all-files] [-fdump-ast-filter=]` should print. It is the oracle of
`expect: ast` goldens (§7) and how a golden is first written. Its
contract:
- **Input.** `clang -fsyntax-only -Xclang -ast-dump=json` of the file
  (one JSON document, read by an in-tree reader; no dependencies).
- **Mapping table** (`xtask/src/clang_ast/table.rs`). Every clang node
  kind has an explicit disposition: *keep* (printed per §3), *drop*
  (skipped with its subtree: implicit-only nodes, type trees, comments)
  or *unsupported* (an error with the reason: templates, namespaces,
  new/delete, throw, lambdas, ...). A kind missing from the table is an
  error naming it. Each kept kind also lists the JSON keys it maps; an
  unknown key (a flag the dump would silently lose, such as a VarDecl's
  `tls`) is an error naming it. A unit test pins that every M2 kind of
  §3 is *keep*.
- **Rendering.** Addresses, ranges and locations are dropped; clang's
  JSON-only shapes become §3 lines (`{}` absent slots become `has_*`
  flags; `array_filler` is printed first with its label; the
  `'NULL TYPE'` static-assert message becomes `unevaluated`); type pairs
  collapse when equal (§2); strings are re-escaped (§2); the `bitfield`
  flag is derived from the referenced `FieldDecl`'s `isBitfield` (§2);
  enumerator values come from the initializer's `ConstantExpr` (or the
  previous value + 1).
- **Side channel.** The JSON lacks the `value:` of a constexpr variable,
  a member function's `trivial`, `noexcept-unevaluated`, `friend` /
  `friend_undeclared` words, its `Overrides:` line and the exact text of
  a `delete("msg")` message, so when one is present the tool also runs
  `clang -Xclang -ast-dump` and pairs that dump's function lines
  (`FunctionDecl`, `CXXMethodDecl`, `CXXConstructorDecl`,
  `CXXDestructorDecl`, `CXXConversionDecl`) and `VarDecl` lines with the
  JSON's in pre-order, checking each pair by class and name; a
  misalignment is an error. The same pairing cross-checks the derived
  `constexpr`, `consteval` and `implicit-inline` function flags. A word
  the dump does not model (`noexcept-uninstantiated`, `ineligible`,
  `not_selected`) is an error.
- **Reconstructed fields.** clang's JSON has no written type for a named
  cast: the tool prints the cast's type plus `&`/`&&` by value category
  (clang's text prints the type as written, `static_cast<R>` through a
  typedef `R`). A base `CXXCtorInitializer` prints the canonical base
  type. clang's `elidable` construct flag and a `destroyed` bind flag are
  errors (C++17 guaranteed elision leaves no elidable constructs in the
  supported subset). A befriended class that redeclares an earlier
  declaration is an error.
- **Values** print as ast.md §9.3, walking the declared type alongside
  clang's untyped tree (`Int 1` of a `bool` is `true`, of a pointer
  `nullptr`). clang prints floats with 7 significant digits, so a
  floating value is recovered exactly from the floating literals in the
  initializer (and zero, negated or not) when exactly one matches; a
  value that cannot be recovered is an error. `long double` values,
  non-null pointer values and lvalue `ConstantExpr`s are errors.
- **Selection.** The main file is the file as named on the command line;
  top-level decls of other files and implicit decls are dropped unless
  `--all-files`/`--implicit`. `--filter` follows §1; a declaration's
  qualified name is built from its semantic context (`D::s` matches the
  in-class declaration and the out-of-line definition of `s`).

Anything the tool cannot render exactly is an error naming the construct
and the enclosing top-level decl. It never prints a guess, so a mismatch
between kiln and the tool is a real difference. Known-divergent constructs
are rejected in the table, never patched in goldens by hand.

Rejected by the table:
- Template specializations other than the implicit instantiation of a
  non-variadic class template with type and integral arguments
  (`ClassTemplatePartialSpecializationDecl`, explicit specializations,
  `VarTemplate[Partial]SpecializationDecl`, `CXXDeductionGuideDecl`,
  and a `TemplateArgument` outside the template nodes that print
  theirs): WP-070b/071/072. Implicit `ClassTemplateSpecializationDecl`s
  under their `ClassTemplateDecl` and `SubstNonTypeTemplateParmExpr`
  are compared (§5, WP-070a).
- `UnresolvedMemberExpr` (member function template names).
- `ImplicitConceptSpecializationDecl` is dropped. These are clang's
  converted concept arguments; the written ones are the
  `ConceptSpecializationExpr` children.

## 7. Harness outcome
`//@ expect: ast` (optionally `//@ ast-filter: <substr>`), normative in
spec/harness.md "Test kinds". The golden `<test>.expected` is required.
- kiln runs with `-fsyntax-only -fdump-ast [-fdump-ast-filter=<substr>]`.
  Pass iff the exit status is 0, `diag:` directives match as for
  `compile-ok`, and stdout equals the golden up to trailing whitespace
  per line and trailing blank lines.
- clang as the compiler under test runs `-ast-dump=json`; its output is
  normalized in-process (§6) and compared the same way. So the corpus is
  checked against clang now, and against kiln once kiln dumps.
- `oracle: clang` additionally requires clang's normalized dump to
  equal the golden (when the compiler is kiln, or a different clang).
- A missing golden, or a dump the normalizer rejects, makes the test
  ERROR. `--update` rewrites an existing golden with the actual dump.
