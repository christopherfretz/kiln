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
kiln++ -fsyntax-only -fdump-ast[=<opt>,...] [-fdump-ast-filter=<substr>] t.cpp   # to stdout
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

The search does not enter a `UsingShadowDecl` /
`ConstructorUsingShadowDecl`: clang 22's JSON nests the shadow's target
under it, but the visitor does not reach the target through the shadow,
so `xtask clang-ast` skips it too (WP-394;
`ast/class.inhctor.init/shadow-implicit.cpp`).

The search follows clang's `RecursiveASTVisitor`, which does not visit
template instantiations ([temp.spec], [temp.expl.spec], [temp.explicit]):
- A template's parameters and its pattern are searched; its implicit
  specializations are not. An explicit (or partial) specialization is
  searched where it is declared. An explicitly instantiated class or
  variable is matched by its own name but its members are not searched;
  an explicitly instantiated function is not searched at all.
- A member of a class template specialization is named by the
  specialization with its arguments (`n::X<int>::N`, `printTemplate
  ArgumentList`), trailing arguments equal to their parameter's default
  dropped (`SuppressDefaultTemplateArgs`; none when one is a pack; a
  non-type argument only when its default is a non-value-dependent
  integral constant expression of the same value, clang's
  `isSubstitutedTemplateArgument`, so `= N` and `= Max<L>::value` keep
  it; WP-297; an omitted type argument is always dropped, a written one
  only when clang's structural comparison finds it equal to the
  substituted default: never through an array or function declarator in
  the default, a reference collapsed through a type parameter, or
  cv-qualifiers added to a reference argument, so `Y<int &&, int &>` for
  `R = T &` and `Z<int &, int &>` for `R = const T` keep it; WP-437); a
  template template argument prints its (qualified) name, an
  enumeration argument its enumerator or, naming none, its value
  (`G<K::b>`, `G<7>`). A member of a partial specialization is named by
  its canonical arguments (`X<type-parameter-0-0 *, true>::C`), with the
  same default trimming: a default naming an earlier parameter
  (`class U = T`) is compared with that parameter's argument, a literal
  default with the argument's value. A member of a template's pattern
  is named by the template's name (`n::A::B::C`).
- A template parameter is named by its template's context: a class
  template's class (`n::A::T`), a partial specialization's
  specialization; a function template's parameter is unqualified (the
  function is its context); a variable template's, alias template's or
  concept's parameter by the enclosing scope (`n::T`).

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
- Cycles (WP-121): the dumper keeps the chain of declarations it is
  inside. A declaration reached again inside itself (a friend template
  redeclaration cycle through specialization lists, `A` befriends `B`
  and `B` befriends `A`) prints its line with ` <cycle>` and is not
  descended into. Nesting is also capped at 4096 levels (deeper children
  are not printed). clang never prints these; a test whose golden
  contains them is a kiln bug.
- Floats and other values print per ast.md §9.3.
- Types print per spec/types.md §12 (as clang).
- **clang 22 printing (`-fclang-compat=22`, WP-355).** The goldens are
  clang 22's (spec/harness.md "Two-oracle policy"), and at level 22 the
  dumper follows its type printer; level 19 keeps the rules above:
  - a tag type as written is not sugar: the second half of a pair stops
    at it and prints it as written (unscoped `'S'`), while a tag reached
    through canonicalizing sugar (substituted template parameter, deduced
    type, `decltype`, `UsingType`, specialization name) prints scoped
    (`'T':'q::S'`); the implicit `__builtin_va_list` typedef's tag prints
    scoped (`PrintOptions::written_tags`);
  - `sizeof`/`alignof`/`new` bounds and pointer differences carry the
    `__size_t` / `__signed_size_t` / `__ptrdiff_t` sugar
    (`'__size_t':'unsigned long'`);
  - an alias template keeps its arguments as written and a sugared
    substitution prints as is (`'Ptr<I>':'I *'`, not 19's desugared
    `'type-parameter-0-0 **'`); a `PackExpansionExpr` prints its pattern's
    type; a member pointer's class prints as written (`int P::*`);
  - a synthesized `<=>`/`==` (class.compare.default) casts each operand
    `DerivedToBase` to the cv-qualified base; `std::tuple_element`'s index
    in a structured binding is the literal `0UL` (spec/sema.md
    "Structured bindings"); a captured binding referenced in a
    non-mutable lambda is `const` ([expr.prim.id.unqual]/3).
  These rules change only how types print; tree differences between the
  levels come from sema's compat-22 rules (spec/sema.md).
  Known gaps are `xfail(kiln)` goldens listed in
  docs/status/wp-184-oracle-switch.md.

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
| `UnresolvedUsingTypenameDecl` | `UnresolvedUsingTypenameDecl <head> name` (the unqualified member name; WP-179). A use of the name prints `'name':'Class::name'` (clang's `UnresolvedUsingType`) |
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
| `DecompositionDecl` | `DecompositionDecl <head> 'type' [static\|extern] [tls\|tls_dynamic] [cinit\|callinit\|listinit]` (a structured binding declaration's unnamed variable `e`, printed as a `VarDecl` without a name); children: the initializer, then its `BindingDecl`s (WP-122) |
| `BindingDecl` | `BindingDecl <head> name 'type'` (the binding's type; `'<dependent type>'` in a dependent pattern); children: the holding `VarDecl` (tuple-like only; `implicit`, but printed even without `implicit` nodes, as clang's text dump does), then the binding expression (`ArraySubscriptExpr`, `MemberExpr`, or a `DeclRefExpr` to the holding variable). A `DeclRefExpr` to the variable `e` prints kind `Decomposition` and name `''`; one to a binding prints kind `Binding` and is `bitfield` when the binding expression is |
| `StaticAssertDecl` | `StaticAssertDecl <head>` children: condition, [message (`StringLiteral unevaluated "msg"`)] |
| `ConstevalBlockDecl` | `ConstevalBlockDecl <head>` child: the call of the block's `consteval` lambda (`ExprWithCleanups` > `CXXOperatorCallExpr`), whose `LambdaExpr` holds the block's compound statement. kiln-only golden (P3289R1, spec/consteval-blocks.md) |
| attribute | `<Name>Attr [#aN] [@loc] [Inherited] [Implicit]` with clang's class name (`[[nodiscard]]` → `WarnUnusedResultAttr`); a kind without one prints `UnknownAttr kind=N`. Arguments are not printed (M2), except: `AsmLabelAttr [Inherited] "label" IsLiteralLabel` (the label's bytes unescaped, as clang) and `BuiltinAttr Implicit N` (the builtin id; the implicit builtin decl and its `extern "C"` `LinkageSpecDecl` print only with `implicit`). Implicit attributes print only with `implicit` |
| annotation (P3394, WP-481) | `CXX26AnnotationAttr 'T' value=<v>` with `T` the underlying constant's type and `<v>` its value printed as §9.3 of spec/ast.md; `CXX26AnnotationAttr 'T' dependent` with the operand as its one child while it is dependent (`T` is then the operand's type as written). One line per annotation, in source order, after the declaration's other children and attributes; a declaration prints only the annotations written on it (an entity's merged list is the reflection query's, spec/annotations.md §4). clang 19 and 22 do not implement annotations, so these lines are kiln-only and never compared with an oracle |
| `IntegerLiteral` | `IntegerLiteral 'type' N` |
| `FloatingLiteral` | `FloatingLiteral 'type' 1.5e0` |
| `CharacterLiteral` | `CharacterLiteral 'type' N` (the code unit value, decimal) |
| `StringLiteral` | `StringLiteral 'type' lvalue "text"` (as an initializer of a `char` array clang 19 prints it as a prvalue) |
| `CXXBoolLiteralExpr`, `CXXNullPtrLiteralExpr` | `... 'bool' true`, `... 'std::nullptr_t'` |
| `DeclRefExpr` | `DeclRefExpr 'type' [lvalue] Kind 'name' 'type' [(UsingShadow 'name')] [non_odr_use_constant\|non_odr_use_unevaluated\|non_odr_use_discarded]` (`(UsingShadow 'name')`: the name was found through a using-declaration, clang's `FoundDecl`) |
| `MemberExpr` | `MemberExpr 'type' [lvalue [bitfield]] .name\|->name [non_odr_use_unevaluated]` (`non_odr_use_unevaluated`: the access is in an unevaluated operand, clang's `NOUR_Unevaluated`) |
| `CXXPseudoDestructorExpr` | `CXXPseudoDestructorExpr '<bound member function type>'`; one child, the object expression (`p->~T` on a scalar or dependent object, WP-121). The destroyed type is not printed (clang prints none) |
| `ImplicitCastExpr` | `ImplicitCastExpr 'type' [lvalue\|xvalue] <Kind> [part_of_explicit_cast]` |
| `CStyleCastExpr` | `CStyleCastExpr 'type' [lvalue\|xvalue] <Kind>` |
| `CXXFunctionalCastExpr` | `CXXFunctionalCastExpr 'type' [lvalue\|xvalue] functional cast to T <Kind>` (`T` is the type as written, alias and cv kept: `CI{1}` with `using CI = const int;` prints `'int' functional cast to CI`) |
| `UnaryOperator` | `UnaryOperator 'type' [lvalue [bitfield]] prefix\|postfix 'op' [cannot overflow]` |
| `BinaryOperator` | `BinaryOperator 'type' [lvalue [bitfield]] 'op'` |
| `CompoundAssignOperator` | `CompoundAssignOperator 'type' lvalue [bitfield] 'op' ComputeLHSTy='T' ComputeResultTy='T'` |
| `ConditionalOperator`, `ParenExpr`, `ArraySubscriptExpr` | class, type, category; children in source order |
| `CallExpr` | `CallExpr 'type' [lvalue\|xvalue] [adl]` (`adl`: the callee was found by argument-dependent lookup, clang's `usesADL`; WP-122); children: callee, arguments. A type-dependent pseudo-destructor call, which kiln types dependent (spec/sema-class.md, "Dependent pseudo-destructor calls"), prints `'void'` as clang's, and so does a `ParenExpr` around one (WP-372) |
| `UnaryExprOrTypeTraitExpr` | `UnaryExprOrTypeTraitExpr 'type' sizeof\|alignof 'operand-type'` (or expression child) |
| `CXXNoexceptExpr` | `CXXNoexceptExpr 'bool'`; one child, the unevaluated operand. The value is not printed (clang prints none) |
| `VAArgExpr` | `VAArgExpr 'T'` (the written type; a prvalue); one child, the `va_list` operand |
| `CXXStaticCastExpr`, `CXXDynamicCastExpr`, `CXXConstCastExpr`, `CXXReinterpretCastExpr` | `Class 'type' [lvalue\|xvalue] static_cast<T> <Kind [(path)]>` (`T` is the type as written: `static_cast<R>(x)` with `using R = int&;` prints `'int' lvalue static_cast<R>`. kiln rebuilds it from `'type'` plus `&`/`&&` by value category, and sema stores it on the node (a cold map, WP-223) only when that rebuild would differ) |
| `CXXFunctionalCastExpr` | `CXXFunctionalCastExpr 'type' [lvalue\|xvalue] functional cast to T <Kind>` (`T` as above) |
| cast paths | a derived-to-base or base-to-derived cast kind prints its path: `<DerivedToBase (A -> virtual V)>` (each base class, `virtual` on a virtual step), also on `ImplicitCastExpr` and `CStyleCastExpr` |
| `CXXConstructExpr`, `CXXTemporaryObjectExpr` | `Class 'type' 'ctor-type' [list] [std::initializer_list] [zeroing] [immediate-escalating]`; children: the arguments |
| `CXXMemberCallExpr` | `CXXMemberCallExpr 'type' [lvalue\|xvalue]`; children: the `MemberExpr` callee, the arguments |
| `CXXThisExpr` | `CXXThisExpr 'type' [implicit] this` |
| `CXXDefaultArgExpr`, `CXXDefaultInitExpr` | `CXXDefaultArgExpr 'type'`; `CXXDefaultInitExpr 'type' has rewritten init`; child: the parameter's default argument or the member's initializer (printed under every use, as clang) |
| `MaterializeTemporaryExpr` | `MaterializeTemporaryExpr 'type' lvalue\|xvalue [extended by Var 'name' 'type']` |
| `CXXStdInitializerListExpr` | `CXXStdInitializerListExpr 'type'`; child: the `MaterializeTemporaryExpr` of the backing array `const E[N]` (WP-106) |
| `CXXBindTemporaryExpr` | `CXXBindTemporaryExpr 'type' (CXXTemporary)` (clang adds an address) |
| `ExprWithCleanups` | `ExprWithCleanups 'type' [lvalue\|xvalue]` |
| `CXXTypeidExpr` | `CXXTypeidExpr 'type' lvalue [type_operand='T']` (or an expression child; `type_operand` is a kiln addition: clang 19 prints nothing) |
| `CXXParenListInitExpr` | `CXXParenListInitExpr 'type'`; `array_filler:` child first, then elements |
| `CXXInheritedCtorInitExpr`, `ArrayInitLoopExpr`, `ArrayInitIndexExpr`, `OpaqueValueExpr` | class and type; an `ArrayInitLoopExpr` has the `OpaqueValueExpr` and the per-element initializer as children; an `OpaqueValueExpr` prints its source expression under every use (as clang) |
| `InitListExpr` | `InitListExpr 'type' [field=name]`; `array_filler:` child first, then elements; `field=` names the initialized member of a union |
| `ImplicitValueInitExpr` | `ImplicitValueInitExpr 'type'` |
| `DesignatedInitUpdateExpr` | `DesignatedInitUpdateExpr 'type'`; children: the base initializer, then the updater `InitListExpr` of the same type (WP-196) |
| `NoInitExpr` | `NoInitExpr 'type'` (an updater slot or filler that keeps the base's value) |
| `DesignatedInitExpr` | `DesignatedInitExpr 'void' [lvalue\|xvalue]` (a designated clause of a dependent `InitListExpr`, in template patterns only; WP-192); children: the initializer, then the array designators' index expressions. The designators are not printed (clang 19 prints none) |
| WP-064 forms | `CXXScalarValueInitExpr`, `CXXDefaultArgExpr`, `CXXThrowExpr`, `StmtExpr`, `CompoundLiteralExpr`, `OpaqueValueExpr`, `BinaryConditionalOperator`, `OffsetOfExpr`, `CXXTypeidExpr`: class, type, category only. Children: a `CXXDefaultArgExpr` the parameter's default argument; a `CXXThrowExpr` its operand, if any; a `StmtExpr` its `CompoundStmt`; a `CompoundLiteralExpr` its `InitListExpr`; an `OpaqueValueExpr` its source expression; a `BinaryConditionalOperator` the common expression, condition, true arm and false arm; an `OffsetOfExpr` its array index expressions; a `CXXTypeidExpr` its expression operand (a type operand is not printed) |
| `__builtin_choose_expr` (WP-316) | `ChooseExpr`: class, type, category only (the chosen operand's; `<dependent type>` prvalue while the condition is dependent). Children: the condition (a `ConstantExpr` once evaluated), then both operands as written (`tests/conformance/ast/ext.gnu.choose.expr/`) |
| `_Generic` (WP-1114, WP-1131) | `GenericSelectionExpr 'type' [lvalue\|xvalue] [result_dependent] [type_operand='T']`: the selected association's type and category (`<dependent type>` prvalue and `result_dependent` while a type-dependent controlling operand or association type leaves it unresolved, clang's `isResultDependent`); `type_operand=` is the controlling type of the type-name form, as written (clang prints it as a type-tree child, as it does the controlling expression's type, which kiln does not print). Children: the controlling expression (expression form), then one line per association in source order, `case 'T'[:'D'] [selected]` or `default [selected]` (clang's association lines; no id or location), holding the association expression (`tests/conformance/ast/ext.c11.generic/`). clang's JSON gives a `case` type and the controlling type only as a type tree: the clang-ast normalizer takes a qualified one's pair from its `QualType` node and reads any other one's desugared half off the tree's sugar chain |
| `ConstantExpr` | `ConstantExpr 'type' value=V` (M2: integral, `bool` and enumeration values; WP-133: aggregates of those, `{1, {2, 3}}`); no `value=` on an immediate invocation not evaluated in place (nested in another, or in a constant initializer; WP-133) |
| `CXXNewExpr` | `CXXNewExpr 'T *' [global] [array] Function 'operator new' 'type'` (clang's `operatorNewDecl` printed as a decl reference, kind, name and type; `array` for an array new; no placement, init-style or alignment flags, as clang 19). Children in clang's order: the array size (the first bound converted to `unsigned long`, an `IntegralCast` or a synthesized `IntegerLiteral` for `new T[]{...}`), the initializer (`InitListExpr`, `ImplicitValueInitExpr` for `()`, or the converted expression), then the placement arguments (WP-061a) |
| `CXXDeleteExpr` | `CXXDeleteExpr 'void' [global] [array] Function 'operator delete' 'type'` (`array` when written `delete[]`; clang's `isArrayForm`); one child, the operand converted to a pointer prvalue (WP-061a) |
| statements | `CompoundStmt`, `DeclStmt`, `NullStmt`, `ReturnStmt`, `BreakStmt`, `ContinueStmt`, `DefaultStmt`, `CaseStmt`, `IfStmt [has_init] [has_var] [has_else] [constexpr] [consteval\|!consteval]`, `SwitchStmt [has_init] [has_var]`, `WhileStmt [has_var]`, `DoStmt`, `ForStmt [has_init] [has_var] [has_cond] [has_inc]`, `CXXForRangeStmt [has_init]` (WP-089: children init, `__range`/`__begin`/`__end` `DeclStmt`s, cond, inc, loop-variable `DeclStmt`, body; the absent slots of a dependent pattern are not printed), `CXXExpansionStmt <dependent\|enumerating\|iterating\|destructuring> [has_init] [expanded]` (WP-447; kiln-only, the clang-ast normalizer does not map it: an expanded statement's children are init, the prelude `DeclStmt`s, then one `CompoundStmt` per element; a pattern's are init, the for-range-declaration as a `DeclStmt`, the expansion-init-list's expressions or the expansion-initializer, body; `CXXExpansionSelectExpr` is the declaration's initializer in a pattern) |

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
- Overloaded operator calls (WP-068b): `CXXOperatorCallExpr '<op>'`
  (plus ` adl`), whose spelling the normalizer takes from the callee's
  name, and `CXXRewrittenBinaryOperator` with its semantic form as the
  only child, as in clang's text dump
  (`tests/conformance/ast/{over.*,class.compare}/`).
- Not in the dump (an error in the clang normalizer, §6): attributes
  other than `override`/`final`, `thread_local`, `new`/`delete`, lambdas,
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
sketch above (WP-071/072).
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
- `CXXDeductionGuideDecl [used] <deduction guide for N> 'auto (<params>)
  -> <ret>'` (WP-134), a user-declared guide, as a function line with
  `ParmVarDecl` children; a template guide is the pattern of its
  `FunctionTemplateDecl <deduction guide for N>`. `explicit` is not
  printed (clang prints none). `used` marks the guide CTAD selected.
  Implicit guides ([over.match.class.deduct]/1) are not printed (clang
  marks theirs `implicit`). The variable a qualified placeholder
  declares keeps the qualifier on the deduced type (`N::S s(1)`:
  `'N::S<int>'`, clang's `ElaboratedType` around the deduced type).
- `ConceptDecl <name>`, with the constraint-expression as its child.
- `ConceptSpecializationExpr 'bool'`. The children are the written
  arguments as `TemplateArgument` lines.
- `RequiresExpr 'bool' [satisfied|unsatisfied]`. A non-dependent
  requires-expression prints its value (`satisfied`/`unsatisfied`);
  a dependent one prints neither. Its parameters are `ParmVarDecl`
  children, followed by `SimpleRequirement` / `TypeRequirement
  '<type>'[:'<desugared>']` / `CompoundRequirement [noexcept]` /
  `NestedRequirement`. Each requirement prints `dependent` when it is
  dependent. A requirement's own satisfaction is not printed (clang
  prints it; kiln records only the expression's value, so the
  clang-ast normalizer drops it). An unqualified type requirement's
  type is `ElaboratedType` sugar (`'typename S':'S'`), as in clang.
- `TemplateArgument type '<type>' | expr | template`. A pack-expansion
  argument prints its pattern, without the `...` (`type 'Ts'`); this
  matches clang.
- A template parameter of a redeclaration that inherits its default
  from an earlier declaration ([temp.param]/12) prints the inherited
  `TemplateArgument` as its default child. That line's first child is
  `inherited from <Kind> '<name>' ['<type>']`, naming the parameter
  that wrote the default (`TemplateTypeParm 'T'`,
  `NonTypeTemplateParm 'N' 'int'`, `TemplateTemplateParm 'TT'`; with
  `ids`, its `#dN`). The rest of the argument's children follow. This
  is clang's `getInheritedFrom`. Only earlier declarations pass a
  default on, from the immediately previous declaration's parameter
  (`TemplateTables::inherited_defaults`). clang's `previous` label (a
  default merged across modules) has no kiln equivalent, and the
  normalizer rejects it. Test: temp.param/inherited-default-ast.cpp.

The dependent expressions use clang's dependent type, `'<dependent
type>'`:
- `DependentScopeDeclRefExpr`, `CXXDependentScopeMemberExpr '…' lvalue
  ./-><name>`, `UnresolvedLookupExpr '…' lvalue [(ADL)] = '<name>'
  [empty]`, `CXXUnresolvedConstructExpr '<type>'`, `PackExpansionExpr`,
  `CXXFoldExpr` and `SizeOfPackExpr 'unsigned long' <pack>`.
- `CXXSpliceExpr '<dependent type>' lvalue [.|->]` (P2996, WP-474): a
  splice-expression `[: r :]` whose operand is value-dependent, with the
  operand as its child; a member splice `e.[: r :]` / `e->[: r :]` adds
  ` .` or ` ->` and has the object expression as its first child. A
  splice of a non-dependent operand never leaves this node: it is
  replaced by the expression it designates (a `DeclRefExpr`, a
  `MemberExpr`, ...). Test: temp.dep/splice-ast-dump.cpp.
- An `UnresolvedLookupExpr` whose candidate set is all function
  templates, with dependent explicit arguments, is `'<dependent type>'`.
  Its overload set is `'<overloaded function type>'`.
- clang prints neither `DependentScopeDeclRefExpr` nor
  `CXXDependentScopeMemberExpr` explicit template arguments as children.
  The dump prints neither.
- **Expressions inside type names.** Wherever a type spells an
  expression (`decltype(e)`, an array bound, `I<e>`), the expression
  prints as clang's `printPretty`:
  - `T::value` and `T::template m<int>`.
  - A `template` keyword written after a nested-name-specifier prints
    as written (`N::template v<T>`, `S::template w<T>`), as clang's
    `hasTemplateKeyword`, for a `DeclRefExpr`, an
    `UnresolvedLookupExpr` and a `DependentScopeDeclRefExpr`. The parser
    records it (`TemplateTables::template_kw`, set by
    `Sema::set_template_kw`). Substitution keeps it on a name that is
    still unresolved, and drops it once the name resolves to a
    `DeclRefExpr` (clang: `N::v<S>`). A substituted `T::template w<U>`
    keeps its substituted qualifier (`S::template w<U>`) (WP-308).
  - `t.x` and `t->template y<1>`.
  - `f<T>` with the qualifier and arguments as written.
  - `N::v<int>`, a variable template specialization named through a
    nested-name-specifier, with the qualifier as written: the
    `DeclRefExpr` records it (`ExprArena::set_decl_ref_qualifier`, set
    by `Sema::set_template_id_qualifier`), and substitution of a
    dependent `N::v<T>` carries it over (WP-300).
  - `T(a, b)` for an unresolved construct, `sizeof...(Ts)`, and folds as
    `(... + xs)`.
  - `sizeof e` / `alignof e` with an expression operand print the
    keyword, a space and the operand as written, adding no parentheses
    (`alignof((g))` stays `alignof((g))`, `sizeof x` stays `sizeof x`;
    WP-312, `tests/conformance/dcl.pre/static-assert-alignof-operand.cpp`).
  - `v.xy` for an ext-vector element access.
- **Trailing return types survive substitution** (WP-318): a member
  function or member function template of an implicit class
  instantiation declared `auto f(U) -> T` prints as written with the
  substituted type (`auto (U) -> int`, `auto (U, int) -> int *`), as
  clang; the substituted declarator keeps the trailing-return bit and a
  plain `auto` base (`templ/subst/decls.rs` `function_decl`)
  (`tests/conformance/temp.inst/member-trailing-return-ast.cpp`).
- **`decltype` expressions in substituted members** (WP-330): a
  `decltype` trailing return type of a member of an implicit class
  instantiation prints its substituted expression as clang's
  `StmtPrinter`: a substituted `T()` (`CXXScalarValueInitExpr`) prints
  as the type followed by `()` (`decltype(u + int())`), and a parameter
  named in its own declarator's trailing return type prints by name
  (`decltype(t.x)`)
  (`tests/conformance/temp.inst/member-decltype-return-ast.cpp`).

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

**Specializations of class and variable templates (landed, WP-070b).**
Normative. The order and placement are clang 19's.
- An implicit instantiation prints its full tree under its template
  (`ClassTemplateDecl` or `VarTemplateDecl`), after the pattern, in
  creation order.
- An explicit specialization, explicit instantiation or partial
  specialization prints its full tree at its own position in its
  enclosing context. Its template then holds a reference line in its
  place:
  - `ClassTemplateSpecialization '<name>'`;
  - `VarTemplateSpecialization '<name>' '<type>'`.
- A `VarTemplateSpecializationDecl` that clang prints at a top-level
  position (after the first declaration that uses it) is also printed
  there in full.
- A template's specializations print under its first declaration not
  introduced by a friend declaration (else its first declaration), as
  clang does (WP-121). Every other redeclaration that a friend
  declaration did not introduce lists each of them as a reference line
  (WP-311). A friend redeclaration of a template prints
  `friend` / `friend_undeclared` only when the friend declaration
  introduced it directly.
- `ClassTemplateSpecializationDecl <tag> <name> [definition]
  <kind>`. The kinds are `implicit_instantiation`,
  `explicit_specialization`, `explicit_instantiation_definition` and
  `explicit_instantiation_declaration`.
- `ClassTemplatePartialSpecializationDecl <tag> <name> [definition]
  explicit_specialization`. Its children are the written arguments as
  `TemplateArgument` lines, in canonical spelling
  (`type 'type-parameter-0-0 *'`), then its template parameters, then
  its members.
- `VarTemplateSpecializationDecl [used|referenced] <name> '<type>'
  <kind> [constexpr] [cinit] [value=…]`. Its children are the
  `TemplateArgument` lines, then the initializer.
- `VarTemplatePartialSpecializationDecl <name> '<type>'
  explicit_specialization …`. Its children are its template
  parameters, then its `TemplateArgument` lines, then the initializer.
  This is the opposite order to the class form, as in clang.
- A reference to a variable template specialization is `DeclRefExpr …
  VarTemplateSpecialization '<name>' '<type>' (VarTemplate '<name>')`.
- Member templates of a class specialization (`ClassTemplateDecl`,
  `TypeAliasTemplateDecl`) print under the instantiated class. Their
  parameters are renumbered to the remaining depth. A member class
  template that is not instantiated prints its pattern without
  `definition`. Its specializations print under it.

Known gaps:
- A partial specialization of a `constexpr` variable template cannot
  be compared, because clang's JSON lacks its value.
- Explicit specializations of members of an implicit specialization are
  not implemented (spec/sema-templates.md §11.2, WP-070b).

**Landed (WP-140).** `FileScopeAsmDecl` (no attributes; its literal is
the child) and `GCCAsmStmt` (no attributes; the output then input
operand expressions are the children; the template, constraints and
clobbers are not printed, as in clang's text dump). Both are kept by the
`xtask clang-ast` normalizer. `tests/conformance/ast/dcl.asm/`.

**Landed (WP-140).** `PredefinedExpr` prints its value category and
spelling (`'const char[2]' lvalue __func__`) with the `StringLiteral` as
its child (none when dependent), as clang does; kept by the normalizer.
`tests/conformance/ast/dcl.fct.def.general/`.

**Landed (WP-117).** A diagnostic's printed member call keeps its explicit
template arguments (`s.get<0>().f(0)`); they are stored on the
`MemberExpr`.

**Landed (WP-160).** `LabelStmt 'name'` (the labelled statement is its
child), `GotoStmt 'name'` (the target label; clang's text dump adds its
address, the JSON names only the `LabelDecl` id, which the normalizer
resolves through the `LabelStmt`), and `AttributedStmt` for
`[[assume(e)]]`: its children are the `CXXAssumeAttr`s (each with the
assumption as its child) then the statement. `asm goto` labels are not
children of the `GCCAsmStmt`, as in clang's text dump. **WP-424**
`AddrLabelExpr 'void *' name` (GNU `&&name`; clang's text dump adds the
label's address, the JSON its `labelDeclId`, which the normalizer
drops). **WP-182**
keeps `[[likely]]`, `[[unlikely]]` and `[[fallthrough]]` too
(`LikelyAttr`, `UnlikelyAttr`, `FallThroughAttr`, without children).
Other statement attributes are not yet kept (kiln drops them; the
normalizer rejects their `Attr` kinds). `tests/conformance/ast/stmt.label/`,
`tests/conformance/ast/dcl.attr.assume/`,
`tests/conformance/ast/dcl.attr.likelihood/`.

## 6. Differential testing against clang
`cargo xtask clang-ast <file> [--compiler=] [--std=] [--implicit]
[--all-files] [--filter=<substr>] [-- <clang args>]` prints clang's AST of
`<file>` in this format: what `kiln++ -fsyntax-only -fdump-ast[=implicit,
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
  flag is derived from the referenced `FieldDecl`'s `isBitfield` (§2),
  or for a `DeclRefExpr` to a `BindingDecl` from that binding's
  expression (its last child, WP-122); a `BindingDecl`'s type, which the
  JSON lacks, is its last child's type (`'<dependent type>'` without
  one), and its holding `VarDecl` is printed although implicit;
  enumerator values come from the initializer's `ConstantExpr` (or the
  previous value + 1).
- **Side channel.** The JSON lacks the `value:` of a constexpr variable,
  a member function's `trivial`, `noexcept-unevaluated`, `friend` /
  `friend_undeclared` words, its `Overrides:` line and the exact text of
  a `delete("msg")` message, and the type as written of a named or
  functional cast (clang prints it with the C printing policy, so the
  tool respells `_Bool` as `bool` and `(void)` as `()`, and drops a
  `struct`/`class`/`union`/`enum` keyword the expression's JSON type
  does not have), so when one is present the tool also runs
  `clang -Xclang -ast-dump` and pairs that dump's function lines
  (`FunctionDecl`, `CXXMethodDecl`, `CXXConstructorDecl`,
  `CXXDestructorDecl`, `CXXConversionDecl`) and `VarDecl` lines with the
  JSON's in pre-order, checking each pair by class and name; a
  misalignment is an error. Its named and functional cast lines are
  paired the same way, by class. The same pairing cross-checks the derived
  `constexpr`, `consteval` and `implicit-inline` function flags. A word
  the dump does not model (`noexcept-uninstantiated`, `ineligible`,
  `not_selected`) is an error.
- **Reconstructed fields.** A base `CXXCtorInitializer` prints the canonical base
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
  in-class declaration and the out-of-line definition of `s`). A
  specialization context's arguments come from the JSON (types and
  integral values printed by their parameter's type) or, for what it
  omits (a template template argument's name, an enumeration
  argument's enumerator), from the side channel's `TemplateArgument`
  lines of the specialization; pack elements print one by one the same
  way (an empty pack prints nothing), and an argument of a deduced
  (`auto`) parameter prints exactly as the side channel spells it
  (`(signed char)'\xfd'`, `U'\U0001f600'`). A character-typed argument
  of a declared parameter (`char`, `signed char`, `unsigned char`,
  `char8_t`, `char16_t`, `char32_t`, `wchar_t`) prints as clang's
  `CharacterLiteral::print` from the JSON value truncated to the type's
  width (spec/ast.md §9.3; WP-437). A default argument
  compared for trimming (clang drops trailing arguments equal to their
  defaults) is a non-dependent type, a literal, an earlier type
  parameter of the same template, or such a parameter under pointer,
  reference (collapsed), cv, array and function declarators (`U = T *`,
  `U = const T &`, `U = T (*)[4]`, `U = void (*)(T *, T[2])`), spelled as
  clang nests declarators with function parameter types adjusted
  (`void (*)(long *, long *)`, `char *const[3]`). Member pointers,
  dependent template-ids (`U = W<T>`) and function types with an
  exception specification, method qualifiers or a calling convention are
  errors. Where the comparison depends on whether the argument was
  written (above; the JSON does not say), each possible spelling of the
  specialization is a candidate and the one among the `Dumping …:`
  headers of clang's text dump filtered by the same string is used (one
  more clang run, only then); none naming the declaration means it is not
  dumped, several are an error. A template template
  parameter's default is equal when the side channel's declaration
  address of the default (under its `TemplateTemplateParmDecl`, counted
  only under a template-declaring parent) is the argument's. Any other
  argument or default is an error.

Anything the tool cannot render exactly is an error naming the construct
and the enclosing top-level decl. It never prints a guess, so a mismatch
between kiln and the tool is a real difference. Known-divergent constructs
are rejected in the table, never patched in goldens by hand.

Rejected by the table:
- Function template specializations (so a template deduction guide
  that deduction succeeded on), variadic specializations, and a
  `TemplateArgument` outside the
  template nodes that print theirs: WP-071/072. Class and variable
  template specializations (implicit, explicit and partial), their
  reference lines and `SubstNonTypeTemplateParmExpr` are compared (§5;
  WP-070a, WP-070b).
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
