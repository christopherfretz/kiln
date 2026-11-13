# spec: kiln_sema

Status: **implemented for M2** (WP-048). Normative for `kiln_sema`
apart from the conversion machinery, which spec/sema-conv.md
(WP-049) covers. §10 lists what is still to be implemented.

Standard: [basic], [conv], [expr], [stmt], [dcl], [class], [namespace].
Reference: clang's `Sema` (clang 22, the oracle; clang 19 at
`-fclang-compat=19`, §1.1), with diagnostics checked against
`clang++-22 -fsyntax-only` and ASTs against `cargo xtask clang-ast`.

## 1. State
`Sema<'a, 'sm>` holds the semantic state of one translation unit.

There are two lifetimes because `DiagEngine<'sm>` is invariant in
`'sm` and has drop glue: `'a` is the borrow of the engine and `'sm` the
source manager it reports against.

| field | meaning |
|---|---|
| `ast: Ast` | the AST under construction; `kiln_ast` arenas, built through `AstBuilder` |
| `diags` | the diagnostics engine |
| `target: TargetTypes` | builtin type properties (sema-conv.md) |
| `abi: Target` | the full target, used for layout (`sizeof`, `alignof`, bit-fields, constant evaluation) |
| `scopes` | the scope stack (§2) |
| `ctx` | the current semantic `DeclContext` |
| `func: Option<FnState>` | the function body being analysed: declaration, return type, labels, switches, breakables |
| `unevaluated` | depth of unevaluated operands |
| `pending_odr` | DeclRefs of the current full-expression that may be odr-uses (§5.2) |
| `defining` | classes being defined (incomplete until `}`) |
| `enums` | enumerations being defined (`EnumBuild`: running value, best type) |
| `local_externs` | block-scope `extern` variables and functions, keyed by enclosing namespace and name (§3) |
| `exceptions`, `rtti` | `-f[no-]exceptions`, `-f[no-]rtti` (default on; §5.5, §5.6) |
| `braced_args` | braced-init-list call arguments awaiting their parameter types (§6.1) |
| `prototype_params` | the parameters (`PrototypeFrame`: `ParmVar` ids) of each parameter-declaration-clause being parsed (§4.3) |
| `unowned_params` | prototype parameters not yet adopted by a function (§4.3) |
| `declared_returns` | the declared (placeholder) function type of a deduced-return function, for its redeclarations (§4.3.1) |
| `group_placeholders` | placeholder types of instantiation-pattern variables whose declarator group passed the one-deduction check, for the re-check at instantiation (§4.3.1) |
| `pragma_packs` | `#pragma pack` directives seen, with their location and argument text (recorded for the class layout, sema-class.md §6.1) |
| `immediate` | depth of immediate function contexts ([expr.const]/16) entered by `if consteval` and `consteval` bodies (§5.10) |
| `imm` | immediate invocations and immediate-function references of the current full-expression, and why escalated functions are immediate (§5.10) |
| `oven` | the constant evaluator and its budget |
| `classes`, `implicit_declared`, `delayed_access` (+ `delayed_access_depth`), `cleanup`, `extend_collector`, `unparsed_exc` | class state, sema-class.md §8.3 (`FnState` gains `this_ty`/`this_override`, `Scope` gains `nrvo`) |

Constructors:
- `Sema::new(ast, diags, TargetTypes)` picks the layout target from the
  builtin types.
- `Sema::for_target(ast, diags, Target)` takes the target directly.
  The parser uses this one.

### 1.0 Language mode (`c`, WP-1275)

`Sema::c: Option<kiln_lex::CStd>` is the C standard of a C translation
unit (`C89`, `C99`, `C11`, `C17`, `C23`, ordered); `None`, the default,
is C++. The parser copies it from `ParseOptions::c`, which the driver
fills from the input's language and `-std=` (spec/driver.md "Language
mode"). Code asks `Sema::is_c()` (C at all) and
`Sema::c_at_least(CStd::C11)` (C of that standard or later); it never
infers C from `Sema::std`, which in C mode is only the C++ standard whose
lexical rules C shares. C mode reuses the C++ semantics of the common
subset (structs, unions, enums, functions, pointers, arrays, statements);
the C-specific rules (tag namespaces, `void *` conversions, the type of
a character literal, tentative definitions, K&R definitions, VLAs,
compound literals) are WP-1276's.

#### 1.0.1 C array declarators, VLAs, compound literals and transparent unions (WP-1287)
- **Array parameters** (C17 6.7.6.2p1, 6.7.6.3p7; `vla.rs`): the
  qualifiers and `static` inside a parameter's outermost `[ ]` (parsed
  by `parse_bracket_quals`, spec/parse.md) qualify the pointer it is
  adjusted to (`int a[restrict const static 4]` is `int *const
  restrict`); either outside a function parameter's outermost bound is
  an error, as in clang. `-Warray-parameter` (in `-Wall`) warns when a
  redeclaration's array parameter has a different constant bound or
  form, with the previous declaration's note.
- **VLAs** (6.7.6.2p4-p5): a non-constant bound makes a `VariableArray`
  (flagged `VARIABLY_MODIFIED`, never interned); `[*]` (only in a
  prototype; in a definition an error) is the shared star bound
  (`TypeContext::vla_star_bound`, printed `[*]`). A function type's
  parameters are star-decayed, so `void f(int n, int a[n][n])` and
  `void f(int, int a[*][*])` are compatible; the `ParmVarDecl` keeps the
  written type, whose bounds codegen evaluates in the prologue. Pointer
  compatibility (`conv::c_assign::compatible`, 6.2.7) treats a VLA
  bound as matching any bound, which also gives `composite_pointer`
  its C rule (pointers to compatible types compare and convert). A
  `goto` or `switch` jump into the scope of a VLA is an error
  (spec/sema.md §7; `tests/c/stmt/jump-into-vla-scope.c`). `-Wvla` is
  off by default, `-Wvla-extension` warns before C99 under
  `-pedantic`. In C++ the same VLAs are the GNU extension
  (`-Wvla-cxx-extension`), as in clang. A variably modified expression
  never folds: the Oven traps it (spec/oven.md).
- **Compound literals** (6.5.2.5p4-p5): in C every literal is an lvalue
  `CompoundLiteralExpr`, never a temporary (no `maybe_bind_temporary`,
  no `MaterializeTemporaryExpr`): inside a function it names an
  automatic object living to the end of its block (`c_compound_literal`),
  at file scope an implicit internal static variable for every type, not
  only arrays (`file_scope_compound_literal`, after the constant
  initializer check). Use sites see the usual lvalue conversion or
  decay. C++ keeps the prvalue rules of §5.8.
- **Subscripts** (6.5.2.1p2): in C `E1[E2]` is always an lvalue, also on
  the array member of a structure returned by value (`c_subscript`).
- **Compound assignment** (6.5.16.2p3): an arithmetic left operand
  suffices in C, so `e |= X` on an enumeration and `d += 1.0i` on a
  real operand are valid; the result converts back to the left
  operand's type. C++ rejects both (clang's incompatible-type error).
- **`transparent_union`** (clang `handleTransparentUnionAttr`,
  `transparent_union.rs`): in C the attribute is kept on a union
  definition (after the class-key, after the closing brace, or on a
  typedef of a complete union, glibc's `__SOCKADDR_ARG`) whose fields
  pass the WP-809 field checks; elsewhere it is dropped with clang's
  warnings. An argument for a parameter of such a union that is not
  the union itself initializes the first member that takes it (a
  pointer member a `void *` or null pointer constant, any member a
  diagnostic-free assignment; clang
  `CheckTransparentUnionArgumentConstraints`), built as an
  `InitListExpr` naming that member; the ABI passes the union as its
  first field (`kiln_abi::call`). C++ keeps WP-809's ignored warning.
  A parameter of such a union is compatible with one whose type is
  compatible with an unqualified member (clang
  `mergeFunctionParameterTypes`, WP-1301): in a redeclaration, whose
  composite takes the member type (`c_transparent_param_merge`), and
  between function pointer types (`c_assign::compatible`). A
  definition's prologue does not demote between the union and the
  member, which arrive alike.

#### 1.0.2 C predefined library functions, GNU attributes and `common` (WP-1301)

- **Implicit library declarations** (C89 3.3.2.2; clang
  `LazilyCreateBuiltin`, `builtins.rs` `c_implicit_lib_function_decl`):
  in C, a call to an undeclared name that is one of clang's predefined
  library functions (the `LIB` rows of the builtin table that the
  standard and `-fno-builtin[-f]` allow, and whose type needs no
  missing `FILE`) declares it at file scope, implicit and `extern`,
  with its library type and attributes, not `int ()`: before C99 the
  warning "implicitly declaring library function 'f' with type T",
  from C99 the DefaultError "call to undeclared library function 'f'
  with type T; ISO C99 and later do not support implicit function
  declarations", both with the note naming the header
  (`builtin::lib_header`). Otherwise the C89 `int ()` rule applies.
- **Incompatible library redeclaration** (clang `MergeFunctionDecl`):
  a later declaration of another type than such an implicit one is
  the warning -Wincompatible-library-redeclaration with the note
  "'f' is a builtin with type T" at the implicit declaration, not
  "conflicting types"; it is a valid redeclaration with its own type.
  Neither takes the non-prototype diagnostics. The implicit declaration
  is then hidden from lookup (`unregister_name`), so the new
  declaration is a separate, ordinary function and not the library
  builtin: a call of it is emitted, never folded (WP-1311).
- **A first declaration redeclares the library function** (clang
  `LazilyCreateBuiltin` on redeclaration lookup, WP-1311,
  `builtins.rs` `c_lib_function_redecl_prev`): a C file-scope,
  non-`static`, non-`overloadable` first declaration of a predefined
  library function's name (`unsigned long strlen();`) silently creates
  the library declaration and redeclares it. A compatible one merges
  (the composite type; a GNU `nothrow` on only one side is ignored,
  since C has no exception specifications) and is the library builtin;
  an incompatible one takes the rule above.
- **GNU attributes on a later declarator** (clang `ParseDeclGroup`,
  `ParseStructDeclaration`; spec/parse.md): `int a, __attribute__((x)) b;`
  and the same in a member declaration, in C and C++.
- **`_Alignas` is a declaration specifier** (C11 6.7.5; C23 `alignas`):
  it may follow a type specifier (`int _Alignas(16) x;`).
- **`common`** (clang `handleCommonAttr`): kept on a C variable (an
  error only in C++); a tentative definition carrying it has common
  linkage even under `-fno-common` (`isVarDeclStrongDefinition`).

#### 1.0.3 C static initializers (WP-1295, WP-1304)

- C23 6.7.11p4 (clang `CheckForConstantInitializer`, `gnu.rs`
  `check_c_static_init`): the initializer of a C object with static or
  thread storage duration (not `__thread`, `check_static_tls`, nor C23
  `constexpr`, `check_constexpr_init`) must be constant. Each element of
  the initializer list, recursively (implicit value-initializations are
  constant), is folded (`folds_as_static_init`) as codegen folds the
  whole initializer: with `set_fold_addr_int`, `set_fold_float_ub`,
  `set_fold_leaks` and `set_fold_c_static` on (spec/oven.md "Address
  constants", "C static initializers", "Label addresses"). The first
  element that does not fold is "initializer element is not a
  compile-time constant", at `element_culprit_loc`. What folds is what
  clang folds: IEEE floating results, `__builtin_constant_p` as 0,
  compound literal values, address constants with offsets (also past
  the object), address-as-integer arithmetic, pointer differences
  within one object, offsetof's null-member form and label address
  differences, and an out-of-range floating to integer conversion,
  which takes `APFloat::convertToInteger`'s saturated value (NaN is 0;
  clang `HandleFloatToIntCast`; the Oven saturates only under
  `set_fold_c_static`, WP-1311). Codegen never initializes a C static
  dynamically: a C static local or global whose initializer did not
  fold is an internal error, never a guard variable or
  `__cxx_global_var_init` (`global_init.rs`). Tests:
  `tests/c/run/static-init-*.c`,
  `tests/c/decl/static-init-not-constant*.c`.

#### 1.0.5 C mode release-review gaps (WP-1331)

- **`(...)` before C23** (C17 6.7.6p1): a parameter list of only `...`
  is `ISO C requires a named parameter before '...'` before C23, at the
  `...`, except on an `overloadable` function, in every C dialect, as
  clang's `GetFullTypeForDeclarator` (clang's `<tgmath.h>` declares
  `__tg_promote(...)` so). In a declaration (file, block, member
  context) the parser records the `...` in `Declarator::c_ellipsis_first`
  and `act_on_declarator` reports it unless the decl-specifiers or the
  declarator carry `overloadable` (the attribute after one declarator
  covers that declarator only); in a type-id or a parameter the parser
  reports it at once. Tests:
  `tests/c/decl/ellipsis-first-param-overloadable.c`,
  `tests/c/run/tgmath-gnu{89,99,11,17,23}.c` (sqrt, pow, fabs, creal on
  float, double, long double and complex arguments, against clang).
- **`_BitInt` literals** (C23 6.4.4.1p3): a `wb` literal
  (`IntSize::BitInt`, spec/lex.md) is `_BitInt(N)` with `N` the value's
  bits plus a sign bit, at least 2 (`0wb` is `_BitInt(2)`, `255wb`
  `_BitInt(9)`); a `uwb` literal is `unsigned _BitInt(N)` with `N` the
  value's bits, at least 1 (`literal::bit_int_literal_width`,
  `Sema::bit_int_literal`; clang `ActOnNumericConstant`). Wider than the
  target's `BITINT_MAXWIDTH` (128 on `AArch64`) is
  `sema_int_literal_too_large` (unsigned) or
  `sema_int_literal_too_large_signed` ("... in any signed integer type"),
  recovering as the widest `_BitInt` with the value truncated, as clang.
  Deviation: kiln's literal values are 128-bit, so on x86-64 (maximum
  8388608) a literal over 128 bits is "too large" where clang types it
  (`tests/c/c23/bitint-literal-wide-x86_64.c`, xfail); on x86-64 a signed 129-bit
  literal (`0xffff...ffffwb`, 32 digits) is supported (`Sema::uint_const`
  zero-extends it). In `#if` the literal is `intmax_t` (`wb`) or
  `uintmax_t` (`uwb`), with clang's too-large error above 64 bits. Tests:
  `tests/c/c23/bitint-literal-*.c` (types by `_Generic`, sizes by
  `sizeof`, run against clang), `tests/c/pp/bitint-literal-if.c`,
  `tests/c/cxx-only/bitint-literal-suffix.cpp`.

#### 1.0.4 C mode correctness batch (WP-1311)

- **Prototype-scope tags and enumerators** (C23 6.2.1p4): an
  enumeration defined in a parameter list has prototype scope, and so
  do its enumerators (`PrototypeFrame::enums`, `enumerators`;
  `c_declare_proto_enumerator`). They do not leak to the enclosing
  scope; a definition's parameter-scope enumerators are visible in its
  body, as its tags are. Tests: `tests/c/decl/param-scope-enumerators*.c`.
- **A failed redeclaration is not the entity** (clang marks it
  invalid): in C, "static declaration follows non-static" leaves the
  new function out of the redeclaration chain without a second
  "redefinition" error, and a block-scope variable redefined with
  another type is invalid and does not replace the first in scope
  (`merge_local_var` reports the conflict). C++ is unchanged.
- **Enumeration promotion** (C17 6.3.1.1p2, 6.7.2.2p4): an enumeration
  without a negative enumerator is compatible with `unsigned int`, and
  in C its promotion type is that unsigned type (clang
  `computeBestEnumTypes`: `|| !CPlusPlus`), not the [conv.prom]/3
  `int`. Enumerators keep type `int`. Test:
  `tests/c/expr/enum-promotion-unsigned.c`.
- **Library builtins in an integer constant expression**: a library
  function clang's evaluator knows (`strlen`, `strcmp`, ...) folds in a
  C ICE, with no "non-constexpr function" note (`SemaBridge::is_c`;
  clang `VisitBuiltinCallExpr` tests `CPlusPlus11`). Test:
  `tests/c/expr/library-builtin-ice.c`.
- **No functional cast in C**: `T(x)`, `int(1)` and `typeof(T){...}`
  outside a compound-literal parenthesis are "expected expression"
  errors (`kiln_parse` `parse_name_primary`). Test:
  `tests/c/expr/no-functional-cast.c`.
- **C23 `va_start(ap, ...)`** (C23 7.16.1.4): a second argument that is
  an integer constant expression of value 0 skips the last-parameter
  check (clang `BuiltinVAStart`; <stdarg.h> passes `0`). Test:
  `tests/c/c23/va-start-no-named-parameter.c`.
- **`[[noreturn]]` on a later declaration** is valid in C; the
  [dcl.attr.noreturn]/1 first-declaration error is C++'s only
  (`check_noreturn_on_first_decl`, standard spelling only).

#### 1.0.5 C diagnostics (WP-1312)

C-only warnings and wording, matched against clang 19 and 22. None of
them changes what is accepted, rejected or emitted.

- `[static N]` array parameters (C23 6.7.7.4p7, clang
  `CheckStaticArrayArgument`, `expr/c_ops.rs` `c_check_static_array_arg`).
  A parameter declared with `static` in its array bound has
  `ParamData::STATIC_ARRAY`. An argument that is a null pointer constant
  gives -Wnonnull ("null passed to a callee that requires a non-null
  argument"; `nonnull` is in `format`, as in clang). An array argument
  smaller than the bound gives -Warray-bounds: elements are compared when
  the element types match, bytes otherwise. Both add the note "callee
  declares array parameter as static here".
- `sizeof` warnings (clang `CheckUnaryExprOrTypeTraitOperand`):
  - -Wsizeof-array-argument: `sizeof` of a parameter declared as an
    array.
  - -Wsizeof-array-decay: `sizeof` of a pointer operation whose operand
    is a decayed array.
  - Both are in `most` and also apply in C++.
- Pedantic forms:
  - `&` of a `void` expression is "ISO C forbids taking the address of
    an expression of type 'void'", unless the operand is `*p` in C99 or
    later. `*` on a `void *` directly under `&` is silent in C99 or
    later, as is any `*` in an unevaluated operand.
  - GNU `?:` with the middle operand omitted is
    -Wgnu-conditional-omitted-operand (in `gnu`).
  - A tag defined in the type of `__builtin_offsetof` before C23 is
    -Wc23-extensions ("defining a type within '__builtin_offsetof'";
    it says 'offsetof' when the call comes from the macro of that name).
  - Before C11, an ordered comparison of pointers to compatible types
    where one is complete and the other incomplete is -Wc11-extensions.
- Library redeclarations (`builtins.rs` `warn_incompatible_lib_first_decl`):
  a first, file-scope, non-static declaration of a predefined library
  function with an incompatible type gives
  -Wincompatible-library-redeclaration even if the function was never
  called before it.
- `switch` (`stmt.rs` `c_case_overflow`): a case value that changes when
  converted to the unpromoted condition type gives -Wswitch, "overflow
  converting case value to switch condition type".
- Wording:
  - A static function used in an extern inline function reads "using
    static … is a C2y extension" at clang 22; the clang 19 text is kept
    as a `_19` row.
  - "static declaration follows non-static" says "previous definition"
    when the earlier declaration is a definition.
  - An incomplete-type variable with an initializer is reported at its
    name.
  - A type keyword where an expression is expected is "expected
    expression".
  - A declarator without a name is "expected identifier or '('".
  - A C23 call of an undeclared name is reported before its arguments
    are parsed.
  - A compound literal's diagnostic range ends at its `}`.

Tests: `tests/c/expr/{static-array-argument,sizeof-array-decay,void-deref-address-c89,void-deref-address-c17,gnu-conditional-omitted-operand,offsetof-type-definition,pointer-compare-complete-incomplete}.c`,
`tests/c/parse/type-keyword-not-expression.c`,
`tests/c/stmt/switch-case-value-overflow.c`,
`tests/c/decl/{incomplete-variable-braced-init,static-follows-non-static-note,static-in-inline-19,static-in-inline-22,incompatible-library-redeclaration}.c`
and the group 1 tests listed in docs/status/c-mode.md.

#### 1.0.6 C behaviour fixes (WP-1318)

Accept/reject decisions matched against clang 19 and 22 at gnu17 and
gnu23.

- Excess initializers (C23 6.7.11p2, clang `ext_excess_initializers`;
  `init.rs` `excess_c`). A clause left over in a braced list of an
  array, structure, union, scalar, vector or `_Complex` object, a
  compound literal included, is `sema_excess_initializers_c` (ExtWarn,
  -Wexcess-initializers: on by default, an error under
  `-pedantic-errors`). It is reported at the first excess clause, also
  for a scalar (C++ reports a scalar's at the entity). The excess
  clauses were checked when parsed; they are left out of the
  initialization, which does not fail, so the object keeps its declared
  size, nothing evaluates them, and the variable stays valid for later
  warnings (-Wunused-variable). A char array's excess clause after its
  string is `sema_excess_initializers_in_char_array_c`, after the
  string's own diagnostic. A too-long string
  (`sema_init_string_too_long_c`) is an ExtWarn too. C++ keeps the
  hard errors.
- `extern "C"` (C has no linkage specification): the parser takes no
  linkage-specification branch in C, so `extern` is a storage class and
  the string literal a malformed declarator, "expected identifier or
  '('" at the string; the declaration is skipped through its `;` or its
  braced body, as clang.
- `?:` is never an lvalue in C (C17 6.5.15 footnote 110;
  `Builder::c_conditional`), whatever its operands: `&(c ? *p : *p)`
  with `p` a `void *` and `&(c ? s1 : s2)` are "cannot take the address
  of an rvalue", and assigning to a structure `?:` is "expression is
  not assignable".
- Unary `+`, `-`, `~` (C23 6.5.4.3p1; clang `CreateBuiltinUnaryOp`): an
  array or function operand is converted to a pointer first, so it is
  rejected as "invalid argument type 'int *' to unary expression" (the
  pointer type) and `sizeof(+a)` is no longer the array's size. C++
  allows the pointer for unary `+` only ([expr.unary.op]/7: `+fn` is
  valid there). `!` of an array or function is valid.

Tests: `tests/c/run/excess-initializers-ignored.c`,
`tests/c/decl/excess-initializers-pedantic-errors.c`,
`tests/c/parse/extern-c-linkage-spec.c`,
`tests/c/expr/{conditional-not-lvalue,unary-arithmetic-operand-decay}.c`,
`tests/c/run/conditional-not-lvalue.c`,
`tests/conformance/expr.unary.op/unary-operand-array-function-decay.cpp`.

### 1.1 Clang compatibility level (`clang_compat`, WP-355)
`Sema::clang_compat` is the driver's `-fclang-compat=<major>` (19 or
22; spec/driver.md). It selects which clang release's observable output
kiln reproduces where clang 19 and 22 differ on the same program; the
language accepted is the same at both levels. The harness passes the
oracle's major (spec/harness.md "Two-oracle policy"). At level 22:
- **Names.** A closure type in a scope chain prints `(lambda)`, not
  `(anonymous class)`, in every diagnostic and predefined name
  (`'f()::(lambda)::operator()<int>'`; `SemaNames::closure_as_lambda`).
- **Sugar.** `size_t`, the signed `size_t` and `ptrdiff_t` carry clang
  22's `__size_t` / `__signed_size_t` / `__ptrdiff_t` sugar
  (`Sema::size_type`; diagnostics print `'__size_t' (aka 'unsigned
  long')`); an alias template specialization substitutes its arguments
  as written (`templ/class.rs`); a class template specialization keeps
  a qualified template template argument as first written
  (`X<int>::apply`; `first_written_template_args`), and a template-id
  naming that parameter substitutes to `X<int>::apply<int>` (an
  elaborated type with the argument's qualifier; `ttp_arg_qualifier`,
  WP-413); a `PackExpansionExpr` keeps its
  pattern's type; a synthesized `<=>`/`==` converts each operand to the
  cv-qualified base ([class.compare.default]).
- **Structured bindings** (§4.9): `std::tuple_element`'s index is a
  `size_t` integer literal (`tuple_element<0UL, E>`), converted to the
  template parameter's type at both compat levels; a binding
  captured by a non-mutable lambda and named in its body is `const`
  ([expr.prim.id.unqual]/3).
- **Statement expressions** (GNU): only the statement's own `;` may
  precede the `})`, so a trailing null statement is the last statement
  and the expression is `void` (clang 19 skipped null statements).
- **Deprecations.** `-Wdeprecated-literal-operator` (CWG2521,
  [over.literal]/1, [depr.lit]): `operator "" _x`, the suffix
  identifier preceded by whitespace, warns by default
  (`sema_deprecated_literal_operator_id`; clang 19 has the warning off).
  `__is_trivially_relocatable` warns `-Wdeprecated-builtins` with the
  replacement `__builtin_is_cpp_trivially_relocatable`
  (spec/sema-templates.md §9.1).
- **Flow and counts** (WP-377). `-Wswitch` counts a deprecated
  enumerator as one a switch must handle (only `[[maybe_unused]]` ones
  may be omitted; `switch_enum.rs`), which also decides whether the
  switch covers every value for `-Wreturn-type`. An `if consteval` is a
  branch on an unknown condition in the CFG (clang 22
  builds its CFG; clang 19 builds none and checks nothing; §7.5.1). `array is too large (N elements)` groups N's
  digits with `'` (`18'446'744'073'709'551'615`;
  `element_count_text`). Wording-only changes are `_19` diag rows
  (spec/diag.md §2.5), e.g. `case ranges are a GNU extension`, `you need
  to include <typeinfo> or import std before using the 'typeid'
  operator`.
- **Attributes and `nodiscard`** (WP-377). A `[[scope::name]]`
  attribute is named in diagnostics as written, with its scope
  normalized and `__x__` stripped as below (`'gnu::aligned'`, `'clang::x'`;
  `attr_shown`, `decl_attrs.rs`); clang 19 prints the bare name. An
  unused call whose *type* (not the callee) is `[[nodiscard]]`, or an
  unused temporary of such a type built by a constructor without its own
  attribute, warns `ignoring {return value|temporary} of type 'T'
  declared with 'nodiscard' attribute` under `-Wunused-value`
  (`unused_type_warning`, `expr.rs`), at the operator call's first token
  for an overloaded operator. Attributes on a block-scope
  using-declaration or using-directive are diagnosed as statement
  attributes (`act_on_decl_stmt_attrs`).
- **Template notes** (WP-377). A conversion error in a non-type template
  argument (`not implicitly convertible`, `conversion ... is not allowed
  in a converted constant expression`, `would bind reference to a
  temporary`; [temp.arg.nontype]/2) is followed by `template parameter
  is declared here` (`note_nttp_param`, `templ/nttp.rs`). A conversion
  function template's deduction mismatch ([temp.deduct.conv]) notes the
  written P (`'T *'`, not `'type-parameter-0-0 *'`). A pack-index
  expression's pack name is quoted (`for pack 'Ts'`), and a single-argument
  concept note quotes a non-type argument too.
- **Type printing** (WP-394; `kiln_ast/src/types/print.rs`,
  `TypeNames::clang_compat`). A canonical tag type prints fully scoped,
  inside template arguments too (`'const J<ns::S> &'`), and a written
  tag or template-id keeps its keyword and qualifier, which are not sugar
  (`'O::J<SA>' (aka 'O::J<ns::S>')`; `'O::S'` has no `aka`).
- **Locations and notes** (WP-394). A class's "defined here" note, the
  deduction and default-argument notes, a dependent nested-name-specifier
  template-id's point of instantiation (the specifier's begin), a scoped
  enum's attribute fix-it, a declaration with no declarators and an
  in-class static member initializer are located as clang 22 locates
  them; an alias template's note is at its `using`; a macro argument's
  backtrace is elided as clang 22's `rangesInsideSameMacroArgExpansion`.
- **Checks clang 22 adds or drops** (WP-394). The self-comparison
  warning in an instantiation; a `void` return operand is a failed
  conversion and an erroneous operand drops the return; CWG2369
  (constraints before substitution) order; P2280 `typeid` of an unknown
  reference; namespace attributes; CTAD parentheses; NSDMI re-reporting
  scope and the failed-NSDMI aggregate report; a discarded branch's
  variable initializer stays quiet; `__builtin_assume_aligned` on a
  non-pointer, a `nullptr_t` vararg passed as `void *`, the signed
  `size_t` and an ext-vector shuffle's result type; vector lane
  evaluation and reductions, `ext_vector_type` on any declaration, the
  atomic memory-order check order, string-literal overlap and the
  `std::allocator` allocation site.
- **Template template arguments** (WP-394; spec/sema-templates.md
  §3.2.1): P's parameters are matched as arguments to A, with
  deduction, constraint subsumption and clang 22's partial-ordering
  context notes (`templ/ttarg22.rs`).
- **Member access and redeclarations** (WP-394). A qualified member
  access `x.B::m` casts the object to the base as written
  (`member_qualifier_type`, `class/member_expr.rs`); the injected
  parameters of a redeclared class template are the definition's
  (`injected_params_template`, `templ/actions.rs`), except in an
  implicit deduction guide, which keeps the first declaration's
  (`first_injected_params_template`; guide printing below).
- **Builtins** (WP-394; spec/sema-templates.md §9.1).
  `__is_referenceable` is an ordinary identifier;
  `__reference_binds_to_temporary` is `__reference_constructs_from_temporary`
  and warns `-Wdeprecated-builtins`.
- Codegen's `-Watomic-alignment` text follows the level
  (spec/codegen.md); the AST dump's type printing, spec/ast-dump.md §2.

kiln++ defaults to level 22 since WP-394 (spec/driver.md), and so does
the library since WP-413: `ClangCompat::default()`, `PredefConfig`,
`ClangFeatures`, `Sema`, `SemaNames`, `ParseOptions`, `DiagOptions`,
`Oven`, `CodegenOptions` and `IrTypes` all start at 22. A test about
clang 19 behaviour passes `ClangCompat::Clang19` (or `19`) explicitly.
Where clang 22's output differs and kiln has not followed it yet, the
test pins clang 19 with `requires: clang<22` and the comment `WP-355
residual` (docs/status/wp-184-oracle-switch.md).

### 1.2 Error recovery (WP-826, WP-888)

After an error, kiln bounds its work the way clang 19 does, so a
translation unit that fails costs about what it costs clang:
- **A fatal error stops instantiation** for the rest of the TU: the
  error limit's "too many errors emitted" (spec/diag.md §7.3), a missing
  `#include`, `-Wfatal-errors`, the depth limit. clang's
  `InstantiatingTemplate` refuses every instantiation once
  `hasFatalErrorOccurred()`; kiln's `Sema::instantiation_stopped` is
  asked by `push_inst_frame`, `instantiate_function_definition` and the
  end-of-TU drain (spec/sema-templates.md §10.3). The parser already
  stops at a fatal error (spec/parse.md).
- **A failed specialization is not redone**: an invalid class or
  function template specialization stays `INVALID` and a later use is
  no new error (spec/sema-templates.md §5.5, §5.6).
- **Erroneous expressions do not cascade**: a use of a failed local is
  an erroneous expression, and a statement built over it keeps its
  sub-statements, which are still diagnosed as in clang
  (spec/sema-templates.md §5.5).
- **A failed call keeps its variable** (WP-849). clang builds a
  `RecoveryExpr` for a call that fails overload resolution (no viable,
  ambiguous or deleted best function; `FinishOverloadedCallExpr`, also
  for a one-candidate set found by unqualified lookup) and for a member
  function call whose arguments do not convert
  (`BuildCallToMemberFunction`), never under SFINAE. Such a failure
  records `Sema::call_recovery` (`note_call_recovery`: the
  instantiation depth and the recovery's type, clang's
  `chooseRecoveryType`: the best candidate's return type, else the one
  return type of the viable candidates, else of all of them, else
  type-dependent). A variable instantiated with an initializer that
  failed that way, at that depth, stays valid (clang's
  `InstantiateVariableInitializer` gets a usable initializer): its
  initializer is erroneous, and later uses are checked with its declared
  type. A placeholder type is still invalid
  (`DeduceVariableDeclarationType` over an initializer that contains
  errors), and so is a variable whose failed call clang does not
  recover (a qualified call of a single function, an operator). When the
  failed call is the whole initializer and the recovery's type is `void`,
  the variable is initialized from a `void` prvalue, so clang's "cannot
  initialize a variable of type 'int' with an rvalue of type 'void'"
  follows (`templ/subst/decls.rs` `var_init`, the Substituter's
  `recovered_call`). Other typed recoveries (a class-type variable from
  an `int` recovery, a recovery nested in an operator) stay erroneous
  and silent. `temp.inst/failed-call-initializer-keeps-variable.cpp`.
- **A failed block-scope typedef is `int`** (WP-848). clang's
  `InstantiateTypedefNameDecl` declares a typedef or alias-declaration
  whose type fails to substitute as an invalid `int` typedef. At block
  scope kiln does the same (`typedef_decl_in`), so the declaration
  statement stays valid and uses of the name are checked with `int`
  ("no viable conversion from 'X' to 'bad' (aka 'int')"). A member
  typedef keeps the error type (its specialization is invalid and its
  uses are silent, spec/sema-templates.md §5.5).
  `dcl.typedef/failed-local-typedef-recovers-as-int.cpp`.
- **Recovery by call** (WP-832). clang's `tryToRecoverWithCall`: a
  bound member function or an overload set of non-static member
  functions used where a value is needed ("reference to non-static
  member function must be called", with the hint "did you mean to call
  it with no arguments?" exactly when the zero-argument call works) is
  replaced by that call. kiln recovers the same way
  (`class/member_expr.rs` `recover_not_called`) where clang runs
  `CheckPlaceholderExpr`: the operands of binary, unary (not `&`) and
  conditional operators, a cast's operand, a member access's object, a
  braced-init-list element (at parse, `act_on_init_clause`), `sizeof`
  and `alignof`, `decltype`, and an implicit conversion. Follow-on
  diagnostics are then about the call's result ("cannot initialize ...
  with an rvalue of type 'int'"). Placeholder deduction diagnoses a copy
  of the initializer, so `auto a = g.f;` reports "must be called" twice,
  as clang does (`Diag::allow_duplicate`). The diagnostic is at the
  member access's location (`(g.f)` at its `(`). Under SFINAE nothing is
  recovered. `expr.ref/bound-member-recovery-by-call.cpp`.
- **Nothing fails silently** (WP-1054). A declaration kiln cannot form
  produces an error, a SFINAE failure or a "not yet implemented" note,
  never nothing: recovery may stay quiet only because an earlier error
  already covers the failure. For class template instantiation this is
  checked, not just promised: `Sema::emit` counts every error-class
  emission (printed, captured under SFINAE, or suppressed), and a member
  whose instantiation fails with no new emission, in a TU without errors,
  is reported as `kiln: internal error: member 'x' of 'A<int>' was not
  instantiated and no diagnostic says why` (`failure_mark` /
  `failed_silently_since`, `class_body_with`; spec/sema-templates.md §4.6
  "No silent drop"). A member declaration's substitution failure is a
  hard error, never SFINAE ([temp.inst]/3, [temp.deduct.general]/8).

## 2. Scopes (`scope.rs`)
- **Kinds:** `ScopeKind` is one of TranslationUnit, Namespace,
  LinkageSpec, Record, Enum, Prototype, Function, Block or Control.
  - `is_local` is true for Function, Block, Control and Prototype.
- **Where names live:**
  - Names in context scopes (translation unit, namespace, record,
    enumeration) live in the AST's per-context lookup tables
    (`AstBuilder::register_name`).
  - Local names live in the scope's own `locals`, which plays the
    role of clang's `DeclsInScope` and `IdResolver` together.
    Block-scope operator function declarations live in `local_ops`
    instead, which only a local class's friend redeclaration searches
    (sema-class.md, friend functions of a local class; WP-329).
  - Context scopes also record their `using namespace` directives.
- **Who pushes which scope:**
  - **Function:** pushed by `act_on_start_function`. It holds the
    parameters, and is also the body's outermost block, so redeclaring
    a parameter there is an error ([basic.scope.block]/2).
  - **Control:** pushed by the parser for the init-statement and
    condition of `if`, `switch`, `while` and `for`.
  - **Block:** pushed for a compound statement, and for a
    sub-statement that is not one (parse.md §6.1).

## 3. Lookup (`lookup.rs`)
- **Modules.** With C++20 modules active, every namespace-scope lookup
  result, ADL candidate and redeclaration candidate also passes the module
  visibility predicate of spec/modules.md §5 (`Sema::module_visible`,
  `module_adl_visible`, `module_redecl_candidate` in
  `kiln_sema::modules`); a TU without module or import directives skips
  it entirely.
- **Unqualified lookup** ([basic.lookup.unqual]) walks the stack
  innermost first:
  - A local scope searches its `locals`.
  - A context scope searches its entity's table, then the namespaces
    nominated by its `using namespace` directives, transitively, as
    [namespace.udir]/2 requires.
  - If several *different* entities are found at one level, lookup
    reports `reference to 'x' is ambiguous`, with candidate notes.
    The notes follow clang's order (`ambiguity_order`, WP-179): for an
    unqualified lookup that ends at a namespace scope, the names declared
    in that open namespace definition come first, newest first (clang's
    `IdentifierResolver` chain; `Scope::first_decl` marks the first
    declaration created in the scope instance), then the namespace's
    lookup table, and duplicates are removed by moving the last entry
    into the duplicate's slot (`LookupResult::resolveKind`). A qualified
    lookup lists table order. An unscoped enumerator is ordered as a
    member of the enumeration's parent, and a set found in one class's
    table while the current context lies lexically within that class
    (an in-class member body; clang's scope chain) is listed newest
    first; from outside it (`S::m`, `s.m`, an out-of-line member body)
    in table order (`lexically_within`, WP-929).
  - **Unscoped enumeration scopes** ([dcl.enum]/11): an unscoped
    enumeration's body is no lookup scope of its own; its enumerators
    are found in the enclosing scope, together with whatever else that
    scope declares under the name. Inside `enum { f = 1, g = f + 1 };`
    after `void f(int); void f(double);`, `f` is ambiguous, as in clang
    (WP-929). A block-scope enumerator found with block-scope functions
    of the same name (`block_scope_set`) is ambiguous too; the
    candidate notes' names differ from clang's (follow-up).
  - **Identity** ([basic.lookup.general]/1, [class.member.lookup]/3;
    WP-197): declarations of one entity, and type declarations that
    designate the same type (two typedefs of `int`, a typedef and the
    class it names), are one result (`is_ambiguous_lookup` compares
    `designated_type`, canonical, then first declarations; clang's
    `resolveKind` UniqueTypes). A set that is all functions is an
    overload set, and an `UnresolvedUsing*` candidate defers the check
    to instantiation. Member lookup's subobject merge (`class_hit`)
    keys declarations the same way, and its candidate note names a
    record by its canonical type.
  - **Invalid declarations** (clang `resolveKind`, WP-371): an
    id-expression whose lookup finds several declarations, at most one
    of them valid, names the valid one, else the last one found, with
    no ambiguity. A variable declared without a type specifier
    ([dcl.type.general]/2; `const c = 2;` after `const int c = 1;`,
    `y; y;`) is invalid and not merged with a previous declaration
    (clang's `MergeVarDecl` returns at an invalid new declaration), so
    it reports no redefinition and `c` still names the valid `c`.
    `dcl.type/missing-type-specifier-implicit-int.cpp`.
    A variable redefinition (`redefinition of 'y'`, or `... with a
    different type`), in the same or a reopened namespace, is invalid
    too (clang `setInvalidDecl`), and `is_ambiguous_lookup` ignores
    invalid declarations as `resolveKind` does, so a later use of the
    name, at the start of a statement or through a using-directive
    included, finds the first `y` rather than an ambiguous set (WP-405;
    `basic.def.odr/redefinition-reopened-namespace.cpp`).
    Block scope follows the same rule (`prefer_valid_local`, WP-396). An
    invalid local gives way to the latest valid declaration of its name
    in the same scope. So in `R::X x; R::f(x);`, the unknown type `R::f`
    makes the second `x` an invalid redeclaration, and later uses still
    name the first `x` with its errors reported, as in clang 19 and 22.
    `basic.lookup/invalid-redecl-recovery.cpp`,
    `basic.lookup.argdep/module-example.cpp`.
    A function whose declaration is invalid (an unknown return or
    parameter type) is erroneous when named outside a call (clang's
    `BuildDeclarationNameExpr` makes such a name an error; a call
    resolves it through its overload set and is still checked, so
    `k()` reports "no matching function"): `&A::f` and `&k`
    (`Sema::address_of`) and an implicit conversion of `k`
    (`conv::implicit_convert`, `Sema::names_invalid_function`) recover
    silently, with no "cannot initialize" after the first error
    (WP-379, `expr.unary.op/address-of-invalid-member-function.cpp`).
    `&` of a parenthesized member name ([expr.unary.op]/4, WP-392,
    clang `CheckAddressOfOperand`): `&(C::f)` / `&(f)` naming an
    instance member function (or an overload set of them) is "call to
    non-static member function without an object argument" at the
    name, and `&b.f` / `&this->f` (a bound member function) is "cannot
    create a non-constant pointer to member function" at the `&`; both
    recover silently, with no "cannot take the address of an rvalue"
    (`expr.unary.op/addr-of-parenthesized-member-function.cpp`).
    A member named without an object is held pending
    (`Sema::member_refs`) until the end of its full-expression
    (`flush_member_refs`); a nested full-expression's end flushes the
    enclosing one's pending names too, which changes only when the
    (already ill-formed) use is reported (WP-445: no observable
    difference found). An implicit instantiation runs with them set
    aside (`in_inst_context`), so completing an associated class for
    argument-dependent lookup (`&(S::m)`, `g(S::n)`) neither reports
    them from inside it nor consumes them; and `&C::m` never looks up
    an `operator&` (clang's `isQualifiedMemberAccess`), so the class of
    `m` is not completed at all (`&S::m` of a `std::map<int, int>`
    member instantiated `std::pair<const int, int>`;
    `expr.unary.op/member-pointer-to-specialization-member.cpp`,
    `temp.inst/instantiation-keeps-pending-member-name.cpp`).
    A variable's initializer that is such a pending member function
    name (`auto p = C::f;`, `void (*p)() = C::tm<long>;` in a static
    member function) is reported when the initializer is attached
    (`reject_objectless_method_init`) and recovered as erroneous, so no
    conversion or deduction of it is diagnosed (WP-888,
    `dcl.spec.auto/member-template-id-in-template-body.cpp`).
  - An ambiguous *type* name (`S s;` with `S` in the namespace and in
    one of its inline namespaces) is diagnosed by the parser through
    `diagnose_ambiguous_name` (spec/parse.md); Sema's recovery
    (`ambiguous_type_recovery`) follows clang: the declaration is
    dropped and the statement skipped.
- **Using-directives** ([namespace.udir]/2-4, WP-339): a namespace-scope
  `using namespace N;` is recorded on its namespace (or the translation
  unit) in `LookupTables::using_dirs` (`Ast::add_using_directive`), so it
  outlives the namespace body and holds for every later reopening and
  lookup into it; a directive inside an inline namespace is also
  recorded on each enclosing namespace up to the first non-inline one
  ([namespace.def.general]/7). A block-scope directive stays on its
  scope. Unqualified lookup (`visible_using_directives`) collects the
  directives of the scopes on the stack, then closes them transitively
  over the nominated namespaces' own directives; each nominated
  namespace `N` is searched at the nearest enclosing namespace that
  contains both the directive's scope and `N` (`common_namespace`). At
  that level its members are *joined* with the namespace's own (clang's
  `CppNamespaceLookup`): `k(0L)` sees both `S::k(int)` and
  `S::T::k(long)` from inside `S::U` with `using namespace T;`. Literal
  operators ([lex.ext]/3-8) are found the same way, through
  `std::literals::chrono_literals` and `std::chrono`'s directive.
- **Qualified lookup** ([basic.lookup.qual], [namespace.qual]/2) searches
  one context's table; only if that finds nothing (after the lookup
  kind's filter) does it search the namespaces nominated by the
  context's recorded directives, recursively, each once: S'(X, m) is the
  union, over nominated namespaces, of their own members, else of their
  own nominated namespaces' S'. Two different entities so found are
  ambiguous.
- **The name before `::`** ([basic.lookup.qual]/1) is looked up for
  namespaces, types and type templates only (`LookupKind::Nested`). The
  parser's lookahead classifies such a component with
  `Sema::classify_nested_name` (in `scan_name_qual_at` and the
  member-pointer scan), so a variable does not hide the namespace or
  class: `std::ifstream fs(fs::path("x"))` with a namespace alias `fs`
  is a functional cast (WP-336).
- **The class-name of a base-specifier** ([class.derived.general]/2) is a
  type-only lookup (`Sema::classify_base_class_name`): a namespace in an
  inner scope does not hide a class of an outer one (clang 22; WP-958).
- **Argument-dependent lookup** ([basic.lookup.argdep]; WP-068a,
  `overload/adl.rs`, spec/sema-overload.md §2.2): a call's unqualified,
  unparenthesized callee whose ordinary lookup found only namespace-scope
  functions or function templates (no block-scope declaration, no class
  member, no implicit builtin: `call_uses_adl`) also searches the
  arguments' associated namespaces and classes (`assoc_arg`), ignoring
  using-directives. A friend first declared in a class is entered in its
  namespace's table with `LookupFlags::ADL_ONLY`: ordinary and qualified
  lookup skip it, ADL finds it when its class is associated. Results
  already found are compared by first declaration. For overload candidate
  order the unqualified results are listed newest first
  (`overload_name_ref`); `lookup_name` itself keeps storage order.
- **Using-shadows** (§4.8) are looked through. `Found` has two parallel
  vectors:
  - `decls`: the entities, shadows stripped (`AstBuilder::strip_shadow`).
  - `found`: what lookup actually found (the `UsingShadow`, or the
    declaration itself), clang's `FoundDecl`; `Found::shadow(i)` gives
    the shadow, if any.
  - The same entity reached by two paths (a declaration and a
    using-declaration naming it) collapses to one result, the later
    path.
  - An unresolved `using_if_exists` target (`UnresolvedUsingIfExists`)
    is dropped whenever anything else is found.
- **Lookup kinds:** `Ordinary` (non-tags hide tags,
  [basic.scope.hiding]/2), `Tag`, `Nested` (the name before `::`:
  namespaces, namespace aliases and types) and `All` (a
  using-declarator's target: every declaration, no hiding).
- **Namespace aliases** are resolved wherever a namespace is expected:
  `classify_name` reports an alias as `NameClass::Namespace` of the
  aliased namespace, and `act_on_nested_name` resolves an alias
  qualifier. `act_on_nested_name_spelled` also returns the declaration
  as written, for `nns_component`.
- **Redeclaration lookup** (`lookup_current_scope`) searches only the
  current scope. The members of a namespace's inline namespaces are in
  other scopes ([basic.scope.scope]/4, [namespace.def.general]/6): an
  unqualified declaration in the enclosing namespace never redeclares
  one, it declares a distinct entity, and a later use that finds both
  is ambiguous (clang's `FilterLookupForScope` without
  `AllowInlineNamespace`; WP-179). A qualified declaration (`int I::x
  = 1;`, `void N::f() {}`) and the template lookup of an explicit
  specialization (`current_scope_with_inline_dn`, [temp.expl.spec]/3)
  do see them. A qualified *variable* declarator whose lookup finds
  more than one variable or function entity (`int Z::t;` with `t` in
  both `Z` and an inline namespace of `Z`) is ambiguous:
  `redefinition of 't' as different kind of symbol`, note `previous
  definition is here` at the first-declared entity, and the new
  declaration is invalid (`ambiguous_qualified_prev`, clang's
  `MergeVarDecl` on an ambiguous `Previous`; WP-235, test
  tests/conformance/namespace.def/inline-qualified-redefinition.cpp). Redeclaration lookup leaves out using-shadows
  (`current_scope_shadows` returns those, for §4.8's conflict checks). If the current scope is the outermost block of a
  control statement (a Block directly above a Control scope), it also
  searches the Control scope, so `if (int x = 0) { int x; }` is
  diagnosed as [stmt.pre]/5 requires.
- **Redeclaration index** (`overload/redecl_index.rs`, WP-658). The
  redeclaration search of an unqualified friend function (and of a
  friend instantiated from a class template's pattern) in a namespace
  whose entry for the name holds 32 or more declarations visits only a
  subsequence of it: the functions and function templates whose
  parameter-type list (canonical parameter types, variadic, template or
  not) hashes like the new declaration's, plus every entry the hash
  cannot judge (non-functions, using-shadows, other templates,
  declarations without a type or with a non-prototype error type).
  Lookup order is kept, flags (`INVALID`, inline, ADL-only) are read
  when the search runs, and `pick_prev_function` still decides, so the
  previous declaration, the invalid and other-kind outcomes, the
  using-declarator checks and every diagnostic are those of the full
  scan; `ParseOptions::full_redecl_scan` turns the index off for tests.
  The index follows the AST through journals of the mutations its keys
  read (`kiln_ast::journal`: a lookup slot replaced or a set shrunk, a
  declaration retyped, a redeclaration group's latest moved).
- **Block-scope `extern` declarations** are recorded in `local_externs`.
  A later namespace-scope declaration of the same name redeclares them,
  per [basic.link]/6 and clang's local extern decls. The key is the
  namespace's first block, so a declaration in any later block of the
  same namespace matches (`namespace s { void t() { int k(int); } }
  namespace s { void k(int); }` is an overload on the return type, an
  error); an inline namespace is a namespace of its own, so neither a
  block-scope declaration in it and a later one in its enclosing
  namespace nor the reverse link, as in clang. Test:
  tests/conformance/basic.link/local-extern-namespace-blocks.cpp
  (WP-1293). A declaration in a function template pattern is not recorded: only its instantiations
  redeclare the entity ([temp.spec.general]/2), so the pattern's `extern
  T e;` is never the previous declaration of `extern int e;` in
  `f<int>`, and a namespace-scope declaration after the pattern does not
  conflict with it. The pattern itself still looks its name up, so a
  non-dependent `extern int g;` in it conflicts with an earlier `extern
  char g;`. A block-scope redeclaration whose type (or the earlier
  declaration's type) is dependent is checked only at instantiation
  (clang's `MergeVarDeclTypes` for a local variable). An instantiated
  block-scope `extern` declaration keeps its function as its context but
  names the namespace-scope entity, which the mangler resolves
  (spec/abi.md §8.27, WP-462). Test:
  tests/conformance/basic.link/local-extern-template-pattern.cpp (WP-330).
- **`classify_name`** gives the parser a name's category: type,
  namespace, value, or undeclared.
  Landed (WP-140): a non-dependent typedef or alias qualifier is
  replaced by the class or enumeration it names before the lookup, as
  `act_on_nested_name` does, so `typedef D J; J::T x;` declares `x`
  (libc++ `<typeinfo>`'s `__impl::__type_name_t`;
  `tests/conformance/basic.lookup.qual/typedef-qualifier-decl.cpp`).
  When that class is not instantiated yet (a typedef of a member class
  of a class template specialization, `typedef L<int>::P Q; Q::I x;`, or
  of an undefined specialization, `typedef L<int> D; D::P::I x;`),
  `classify_name` cannot instantiate it, so it classifies the name in
  the pattern (the member class's pattern definition; the template
  through `classify_template_member` for a specialization, the WP-276
  rule in spec/parse.md), as for a template-id qualifier;
  parsing the nested-name-specifier then instantiates the class
  ([temp.inst]/2) and the real lookup takes the instantiated member
  (WP-288; libc++ `rand.dist.norm.lognormal/param_types.pass.cpp`;
  `tests/conformance/temp.res/qualified-name-through-typedef.cpp`,
  `qualified-name-through-typedef-run.cpp`).
  A member enumeration of a specialization that is not instantiated
  yet is classified in its pattern the same way, so `N c(N::US::Lazy);`
  (with `US` a member of the base `G<N>`) finds the enumerator and
  declares a variable ([dcl.ambig.res]/1; WP-782,
  `dcl.ambig.res/member-enum-of-base-specialization-enumerator-run.cpp`).
  A name that qualified lookup does not find in a class with a
  dependent base (the current instantiation named by its
  injected-class-name, `X::B::type` in `X<T> : B<T>`) is `Dependent`: a
  member of an unknown specialization ([temp.dep.type]/6), so it is a
  type in a type-only context without `typename` (WP-789;
  `tests/conformance/temp.dep.type/current-instantiation-qualifier-unknown-member-alias-run.cpp`).
- A nested-name-specifier component naming a class or enumeration that
  is declared but not defined ([basic.lookup.qual]/1, [class.mem]/1:
  a class is complete only after its definition) is "incomplete type
  'N::F' named in nested name specifier" (`require_complete_nns_ctx`,
  clang's `RequireCompleteDeclContext`): the type is printed fully
  qualified (through a typedef, the class it names); a class takes the
  "forward declaration of 'F'" note, an opaque enumeration none. At
  compat 22 it is at the start of the whole specifier, at 19 at the
  incomplete component. The parser skips the rest of the specifier; a
  declarator so qualified declares nothing (later uses are undeclared),
  a type specifier is the error type. It applies in every context
  (declarations, `friend`, `sizeof`, expressions, alias and default
  template arguments, out-of-line definitions); not to a dependent
  qualifier, a member of a template (`DEPENDENT_CTX`), a specialization
  or instantiation (`require_complete_nns`), a pointer-to-member's
  `F::*`, or the class being defined. `class F::H;` is first
  "forward declaration of class cannot have a nested name specifier".
  Tests: `basic.lookup.qual/incomplete-nested-name-specifier{,-location,-location-clang19}.cpp`.

## 4. Declarations (`decl.rs`, `declspec.rs`)

**Entity identity** (`entity.rs`, WP-471). Two declarations denote the
same entity exactly when they are in one redeclaration chain
([basic.link], [basic.def]/1): a redeclaration, a definition after a
declaration, an out-of-line member definition ([class.mfct]/1) and a
friend declaration of a function ([class.friend]; looked through to the
function it declares) are all the entity their first declaration
introduced. An instantiated specialization is a distinct entity from its
template or pattern ([temp.spec.general]), and so is an explicit
specialization. `Sema::same_entity(a, b)` is the only test of this
(`Sema::entity_decl` gives the representative: friend looked through,
then `Ast::first`); code that means "same entity" never compares
`DeclId`s, which differ between redeclarations. Reflections are interned
by `entity_decl`, so `info` equality is `same_entity`. `cargo test -p
xtask` lints the reflection code (`kiln_sema/src/reflect.rs`,
`reflect_operand.rs`, `reflect_limit.rs`, `kiln_sema/src/meta/`,
`kiln_oven/src/meta/`) for raw `DeclId` `==`,
`!=` and `assert_eq!`; a line that really means the declaration carries
`// same_entity: raw ok`.

### 4.1 DeclSpec and Declarator
- The parser fills a `DeclSpec` and a `Declarator` (clang's classes of
  the same names).
- `type_from_declspec_at` checks the specifier combination
  ([dcl.type.general]/2, [dcl.spec]) with clang's wording, and gives
  the base type.
- `Declarator` carries:
  - the name and an optional qualifier
  - `chunks`, innermost first. `Chunk::Function` carries the
    parameters, `variadic`, the parenthesis locations and `exc`, the
    exception specification Sema built from the declarator (§4.7);
    `ExcSpec::None` when none was written (WP-058 added the field)
  - attributes, bit-width, `init_style`, and context
  - `parens` and `after_comma` (spec/parse.md §4): applying the chunks
    wraps the type in `Paren` sugar for each pair of parentheses around
    exactly the chunks applied so far (`declarator_parens`; none around
    the outermost type or a function or array type, spec/types.md)

### 4.1.1 Nullability qualifiers (`nullability.rs`, WP-398)
A clang extension. A nullability qualifier (`_Nonnull`, `_Nullable`,
`_Null_unspecified`, `_Nullable_result`; spec/parse.md §4) is recorded
as `Attributed` sugar (`TypeAttrKind::{Nonnull, Nullable,
NullUnspecified, NullableResult}`) over the type it qualifies, as
clang's `AttributedType`; the canonical type is unchanged
(`__is_same(int* _Nonnull, int*)`), and the type prints as clang does
(`int * _Nonnull`). `apply_nullability` is clang's
`CheckNullabilityTypeSpecifier`:
- the type must be able to have nullability (`can_have_nullability`:
  canonically a pointer or member pointer, or a dependent type), else
  `sema_nullability_nonpointer` ("nullability specifier '_Nonnull'
  cannot be applied to non-pointer type 'int'") and the qualifier is
  dropped; a reference is a non-pointer. A class marked `class _Nullable
  X` (nullability on classes, WP-554; `AttrKind::TYPE_NULLABLE`, spec/parse.md
  §4 "Tags") also can, as clang 19's `canHaveNullability` decides: a
  class that is not a specialization when any redeclaration carries the
  mark, and a class template specialization, dependent or not, only when
  a redeclaration of its primary template's pattern does. A mark on a
  partial or explicit specialization does not count. Neither does the
  injected-class-name, nor `X<T>` as the current instantiation inside
  `X` (clang's `InjectedClassNameType`). As clang's
  `inferNullableClassAttribute` does, a class or class template pattern
  named `auto_ptr`, `coroutine_handle`, `exception_ptr`, `function`,
  `move_only_function`, `shared_ptr` or `unique_ptr` declared directly
  in `std` (inline namespaces skipped) counts as marked
  (`std_nullable_class`, WP-686; test
  `tests/conformance/dcl.type.class/nullability-std-smart-pointers.cpp`)
- the same qualifier written twice is `sema_nullability_duplicate`
  (warning, `-Wnullability`); a different one already written on the
  type, or carried through sugar (a typedef), is
  `sema_nullability_conflicting` (an error, the new one dropped), with
  `note_nullability_here` at the typedef that carries it
- order: a declarator chunk's qualifiers apply right after the chunk
  (`chunk_nullability`, clang's `processTypeAttrs` on the chunk).

GNU `__restrict` (`__restrict__`) after a function declarator's
parameter list is a method qualifier like `const` (`FuncInfo::this_quals`,
WP-1091): in a non-static member function `this` is `T * __restrict`
(`this_pointer_type`; restrict qualifies the pointer, not the object, so
the implicit object parameter keeps only the cv-qualifiers,
`method_object_quals`); it is no basis for overloading (`void f()
__restrict; void f();` in a class is "class member cannot be
redeclared"); clang's diagnostics spell it `__restrict` ("non-member
function cannot have 'const __restrict' qualifier"); a repeated one is
`-Wduplicate-decl-specifier` ("duplicate 'restrict' declaration
specifier"). Its mangling is spec/codegen.md's. Test:
class.this/restrict-method-qualifier-run.cpp.

A nullability keyword after a function declarator's parameter list
(`void f() _Nonnull;`, `using F = void() _Nullable;`) is parsed and
dropped, as clang keeps it with the method qualifiers outside the type
(`Declarator::fn_nullability`, WP-1091). The qualifier checks of a
static member function ([class.static.mfct]/2), a non-member function
and an explicit object member function ([dcl.fct]/6) see it: when it is
the only qualifier the error quotes an empty spelling ("non-member
function cannot have '' qualifier") at the start of the declaration's
decl-specifiers (`diag_nullability_only_fn_quals`); with a cv- or
ref-qualifier it is not quoted. Tests:
dcl.fct/explicit-object-restrict-nullability-qualifier.cpp,
dcl.fct/fn-restrict-nullability-qualifier-context.cpp.

Decl-specifier qualifiers apply to the decl-specifier type when it can
have nullability (`declspec_nullability`; `_Nonnull NP x` with `NP` a
pointer typedef). Otherwise, as clang's `distributeNullabilityTypeAttr`,
they move to the declarator's outermost pointer or member-pointer chunk
(through arrays, not through references; past a function's return
type to a function pointer) with the `-Wnullability-declspec`
warning-as-error `sema_nullability_declspec` ("...; did you mean to
apply the specifier to the pointer?"), unless that chunk already has
one. An array parameter (it decays) accepts one directly. With no
target, the non-pointer error stands. A qualifier on an undeduced
`auto` is dropped (clang applies it after deduction; follow-up).

Instantiation (`templ/subst/types.rs`, clang's
`TransformAttributedType`): a qualifier on a dependent type that
substitutes to a non-pointer is `sema_nullability_nonpointer` and a
substitution failure (a SFINAE note in deduction), reported at the
keyword as clang does (it keeps a TypeLoc): `apply_nullability` records
a dependent result's keyword location in `InstState::nullability_locs`
(keyed by the attributed type) and `nullability_loc` picks the one
nearest the substituted declaration (WP-404;
`tests/conformance/ext.clang.nullability/subst-location.cpp`).

Array brackets (WP-404, clang `ParseBracketDeclarator`; kiln_parse
`parse_bracket_quals`): a nullability qualifier inside `[...]` is
recorded against the array chunk. On a parameter's outermost array
(chunk 0 of a `Prototype` declarator) it is accepted and moves to the
decayed pointer (`adjust_param_type` re-wraps it: `int a[_Nonnull]` is
`int * _Nonnull`); on any other array it is the non-pointer error
(`'int[3]'`, `'int[]'`). C99 `static` and cv/`restrict` qualifiers
there are errors in C++ at the `[`: `parse_c99_array_usage_cxx`
("static array size / qualifier in array size is a C99 feature, not
permitted in C++") on a parameter's outermost array, else
`parse_array_static_outside_prototype` / `parse_array_qual_outside_prototype`
(the declarator is invalid). `[*]` is not parsed.
(`tests/conformance/ext.clang.nullability/array-brackets.cpp`.)

Redeclarations (WP-404, clang `mergeParamDeclAttributes`;
`check_param_nullability`): each parameter's outermost nullability is
compared with the previous declaration's; a difference is the
`-Wnullability` warning `sema_nullability_mismatched_redecl`
("nullability specifier '_Nullable' conflicts with existing specifier
'_Nonnull'") at the new parameter, with `note_previous_declaration` at
the latest previous one. An unwritten one inherits (clang copies it onto
the type; kiln finds the nearest earlier declaration that wrote one).
Return types are not compared in C++, nor nested pointers
(`tests/conformance/ext.clang.nullability/redecl-conflict.cpp`).
Not implemented: copying the inherited nullability into the
redeclaration's parameter type, `-Wnonnull` ("null returned from
function that requires a non-null return value"),
`-Wnullability-extension` (pedantic), the `-Wnullability-completeness`
family, and the `nonnull` attribute spellings.

### 4.2 Declarator to declaration
`act_on_declarator` turns a `Declarator` into a declaration:
1. It applies the chunks from the base type outwards ([dcl.meaning]),
   checking:
   - arrays of references, functions, `void` or abstract types
   - functions returning arrays or functions
   - references to references, pointers to references
   - array bounds: converted constant expressions of integral,
     unscoped enumeration or `_Atomic` integral type, which must be
     positive. A bound that is not a constant expression is a VLA, which
     kiln does not support (`sema_vla_unsupported`), except at namespace
     or class scope, where a bound that `OvenBridge::fold_rvalue_int`
     folds (lax mode: e.g. a `const _Atomic(int)` or `const` variable
     with a non-constant-expression initializer that is still a constant)
     gives clang's `TryToFixInvalidVariablyModifiedType` result: a
     constant array, `-Wvla-cxx-extension` at the bound (with the strict
     evaluation's notes) and the ExtWarn "variable length array folded
     to constant array as an extension" [-Wgnu-folding-constant] at the
     declarator name (WP-165). One that does not fold there (WP-407,
     `vla_at_file_scope`) is `-Wvla-cxx-extension` at the bound with the
     notes, then an error at the declarator name as clang's declaration
     checks give it: "function declaration cannot have variably modified
     type" when the declarator's outermost chunk is a function; "fields
     must have a constant size: ..." for a non-static data member
     (`Sema::declarator_field`); "variable length array declaration not
     allowed at file scope" when every chunk outside the bound is an
     array (variables, static data members, typedefs); "variably
     modified type declaration not allowed at file scope" otherwise
     (`int (*p)[n]`). Before that error each array chunk around the
     bound is a VLA too and warns (`int z[2][n]`: at the outer `[`
     without notes; `int w[n][n]`: at the outer bound with its notes),
     up to the first chunk that is not an array. An alias-declaration's
     type-id at namespace or class scope (`DeclSite::Alias`) is a
     typedef's declarator: it folds or is rejected the same way, the
     folding ExtWarn or the error deferred to the alias name
     (`Sema::pending_alias_vla`, emitted first by `act_on_alias_decl`;
     WP-415,
     `tests/conformance/dcl.array/vla-namespace-scope-alias-and-outer-arrays.cpp`).
     Not yet modelled: another namespace-scope type-id (`sizeof(int[n])`
     in an initializer, a cast) still says `sema_vla_unsupported`
   - variable length arrays in a function (WP-183, clang `BuildArrayType`):
     a non-constant bound of a block-scope declarator (variable, typedef,
     alias, type-id) makes a `VariableArray` even when it would fold.
     `-Wvla-cxx-extension` is emitted at the bound with the strict
     evaluation's notes (an array of a VLA, `int k[3][n]`, warns once at
     the outer `[`, without notes). In a GNU dialect (`-std=gnu++NN`,
     `Sema::gnu`) the warning is clang's `ext_vla_cxx_in_gnu_mode`, an
     Extension that is off by default (`sema_vla_cxx_extension_gnu`;
     `-Wall` enables it, `vla-cxx-extension` being in `all`, WP-230).
     `check_vla_var` then handles the variable: `extern` is an error
     ("cannot have 'extern' linkage"); `static`/`thread_local` folds the
     bound (`fold_rvalue_int`) to a constant array with the
     -Wgnu-folding-constant ExtWarn at the name, or is an error ("cannot
     have 'static' storage duration"). Any initializer is the error
     "variable-sized object may not be initialized" at the bound. A VLA
     variable counts as complete. Template substitution rebuilds a
     `VariableArray` with the substituted bound. Kiln's diagnostic
     dedupe keeps an instantiation from repeating the warning (clang
     repeats it), and `sizeof(T[n])` warns once (clang twice).
2. It moves `constexpr`'s implied `const` onto the variable's type.
3. It looks up a previous declaration (§3), checks that redeclarations
   are compatible, and links the redeclaration chain. A declaration
   found invalid there (ambiguous or missing qualified member, a
   redefinition, a type mismatch, an invalid storage class) is marked
   invalid, and a later redeclaration skips invalid variables to the
   latest valid one (`valid_prev_var`, clang does not link an invalid
   redeclaration): `R::v; int R::v;` redeclares `extern int v`, and
   after `long R::t;` against `int t`, `int R::t;` is "redefinition of
   't'" noting the original (WP-421,
   `tests/conformance/dcl.meaning/qualified-implicit-int-redeclaration.cpp`).
4. A variable of type `void` that has an initializer ([basic.def]/5,
   [dcl.init.general]/7: `void x(1);`, `void v(T::m);`, `void h(void
   (*fp)(X<T>::m), int);`) is "variable has incomplete type 'void'" at
   its name when declared (`check_void_init_var`), before the initializer
   is parsed, as in clang; the variable is invalid and its initializer is
   discarded. A `void` deduced from `auto x = f();` is reported when the
   initializer is attached (WP-291,
   `tests/conformance/dcl.ambig.res/dependent-qualified-param-or-init.cpp`).

The resulting declaration depends on the declarator:

| the declarator is | the result is |
|---|---|
| a `typedef` | `TypedefDecl` |
| an alias declaration | `TypeAliasDecl` |
| a function type | `FunctionDecl` |
| in a record | `FieldDecl` |
| anything else | `VarDecl` |

Redeclaration and linkage rules:
- **Differing types:** a mismatch is
  `redefinition of 'x' with a different type` when the new declaration
  is not `extern`, else
  `redeclaration of 'x' with a different type`. The note says
  `previous definition is here` if the previous declaration was a
  definition, else `previous declaration is here`. Array types that
  differ only in the presence of a major bound are not a mismatch
  ([basic.link]/11), in either order, also for the out-of-class
  definition of a static data member (which therefore redeclares the
  member rather than adding a second one to the class); a redeclaration
  that omits the bound takes the earlier declaration's bound
  ([dcl.array]/8; `redecl_var_type`, clang's `MergeVarDeclTypes`):
  `extern int a[4]; int a[] = {1};` and `struct S { static int b[4]; };
  int S::b[] = {1};` both declare `int[4]`
  (tests/conformance/class.static.data/omitted-bound-redecl.cpp, WP-310).
- **Linkage** ([basic.link]): `static`, `extern`, `extern "C"`, members
  of unnamed namespaces, and `const` variables at namespace scope.
- **Block-scope variables** (`merge_local_var`, clang's `MergeVarDecl`;
  [basic.scope.block]/2, [dcl.stc]/5, [basic.link]/6-7): a variable
  redeclaring a variable or parameter of the same block is checked in
  clang's order, stopping at the first error:
  1. an `extern` that also finds a declaration with linkage in the
     enclosing namespace (or an earlier block-scope `extern` of the name)
     when the block's declaration has none: `redefinition of 'x' as
     different kind of symbol`;
  2. differing types (as above; skipped when the earlier declaration is
     invalid, e.g. `static Q(a);` of incomplete `Q`, or when either type
     is dependent);
  3. `static` after an `extern`: `static declaration of 'x' follows
     non-static declaration`;
  4. non-`static` after `static` (unless an `extern` redeclaring a
     declaration with linkage): `non-static declaration of 'x' follows
     static declaration`;
  5. `extern` after a declaration without linkage: `extern declaration
     of 'x' follows non-extern declaration`;
  6. non-`extern` after a declaration with linkage: `non-extern
     declaration of 'x' follows extern declaration`;
  7. otherwise `redefinition of 'x'`, unless both are `extern`.
  The note is at the earlier declaration (`previous definition` unless it
  is `extern`), even when that declarator was invalid. A new declarator
  that is itself invalid is not merged. A qualified declarator rejected
  in a block (`T(::a);`, "cannot name the global scope") still declares
  `a` in the block, invalid and with linkage (`dropped_qualified_locals`),
  as clang does. Test:
  tests/conformance/basic.scope.block/redeclaration-merge.cpp (WP-235).
- **Namespace-scope variable redeclarations** (clang's `MergeVarDecl`;
  [dcl.stc]/4, [basic.link]/3, [basic.link]/6): after the type check and
  before the `thread_local` and redefinition checks, a `static`
  declaration after one with external linkage (a block-scope `extern`
  that redeclared nothing included: `inline int g() { extern int v;
  return v; } static int v;`) is `static declaration of 'x' follows
  non-static declaration`, and a declaration that is neither `static`
  nor `extern` after a first declaration that is `static` is
  `non-static declaration of 'x' follows static declaration`. The new
  declaration is invalid and not merged; the note is at the declaration
  found (`previous definition` when it is one). An earlier declaration
  with internal linkage otherwise (an unnamed namespace, a namespace
  scope `const`) is checked for redefinition only. Test:
  tests/conformance/dcl.stc/static-after-non-static-var.cpp (WP-1293).
- **Exception specifications** ([except.spec]/5): function types that
  differ only in their exception specification are not "different
  types". A new declaration that writes a specification incompatible
  with the previous one's is the error `exception specification in
  declaration does not match previous declaration` with a `previous
  declaration is here` note, and is marked invalid. Compatible
  spellings (`noexcept`, `noexcept(true)`, `throw()`) compare equal
  through the canonical type, and no specification matches
  `noexcept(false)`. A new declaration that writes **none** where the
  previous declaration has one follows "Exception specifications on
  redeclarations" (§4.7).
- **GNU asm labels** (`act_on_asm_label`, clang's
  `ActOn{Function,Variable}Declarator` and `mergeDeclAttributes`): a
  function (also a member function, WP-449, and a constructor,
  destructor or conversion function, WP-462) or variable declarator's
  label becomes an `AsmLabel`
  attribute, first in the declaration's attribute list, and its
  `StringLitId` is recorded in the `asm_label` cold data of the entity's
  first declaration, which the mangler reads (spec/abi.md). A typedef's
  label is ignored, as clang ignores it.
  - A redeclaration without a label inherits the previous declaration's
    `AsmLabel` (a copy with the inherited flag, ahead of its own
    attributes, as clang orders them).
  - A label differing from the previous declaration's is
    `conflicting asm label`, with a `previous declaration is here` note
    at the old label.
  - A label first written on a redeclaration of an entity that is
    already used is `cannot apply asm label to {variable|function}
    after its first use`.
  - A label on an automatic variable (local, no storage class, not
    `thread_local`) warns `ignored asm label '…' on automatic variable`.
- **Initializing a non-variable** (clang's `AddInitializerToDecl`): an
  initializer on a function or typedef declarator (reachable after an
  asm label, where `{` is not a body) is
  `illegal initializer (only variables can be initialized)` at the
  declaration, which becomes invalid.
- **An `extern` variable with an initializer** is an error at block
  scope; elsewhere it warns `'extern' variable has an initializer`
  (-Wextern-initializer) unless its base element type is const, it is
  `extern "C"`, or it is a template instantiation (clang
  `AddInitializerToDecl`). Explicit and partial specializations warn.

- **Redundant parentheses** (`vexing.rs`, clang
  `warnAboutRedundantParens`, WP-191): when the outermost declarator
  operator is a pair of grouping parentheses (`T(x);`), a block-scope,
  condition or for-init declarator of class or dependent type with only
  a type specifier, a simple name that ordinary lookup finds, no
  function, reference, member-pointer or unbounded array chunk, and no
  preceding comma could be a function-style cast: "parentheses were
  disambiguated as redundant parentheses around declaration of variable
  named 'x'" [-Wvexing-parse] with clang's notes (`add a variable name
  to declare a ...`, unless the class is complete with a trivial
  destructor; `add enclosing parentheses to perform a function-style
  cast`; `remove parentheses to silence this warning`). Otherwise the
  off-by-default "redundant parentheses surrounding declarator"
  [-Wredundant-parens], except when the declarator-id's
  nested-name-specifier begins with the global `::` (`A (::B)`,
  `A (::N::B)`: `Declarator::global_qualifier`, set by the parser; clang
  walks the specifier's prefixes for `Global`). Not in an instantiation.
  Parameter declarators, exception declarations and the abstract
  declarators of type-ids (`sizeof(int (*))`, `W<int (*)>`, a template
  parameter) cannot be expressions and only get -Wredundant-parens
  (WP-235); a parenthesized new-type-id whose declarator has a function
  chunk does not warn (its parentheses are required). Test:
  `tests/conformance/dcl.ambig.res/redundant-parens-abstract.cpp`.
- **Qualified declarator-ids** (`qualified_decl.rs`, clang
  `diagnoseQualifiedDeclaration`, WP-191): a qualified name must be
  declared in a scope that encloses its qualifier's: "definition or
  redeclaration of 'a' cannot name the global scope" (`T(::a);`), "...
  not allowed inside a function", "cannot define or redeclare 'a' here
  because namespace 'N' does not enclose namespace 'M'", and for members
  "non-friend class member 'f' cannot have a qualified name" / "extra
  qualification on member 'f'". The declaration is then dropped, or (for
  the recoverable ones) declared unqualified. Every declarator-id form
  is checked (WP-235): identifiers, operator-function-ids, conversion-
  function-ids (`operator long`), literal-operator-ids (`operator""_y`),
  constructor and destructor names; the scopes print unqualified (`'M'
  does not enclose namespace 'C'`). In a class, a qualified conversion
  function or literal operator is left to the other checks (clang
  rejects both while parsing). A function template explicit
  specialization outside a namespace enclosing its template is "function
  template specialization of 'h' not in a namespace enclosing 'N'" with
  "explicitly specialized declaration is here" (`check_spec_scope`).
  Test: `tests/conformance/dcl.meaning/qualified-declarator-special-names.cpp`.
  **Exemptions (WP-1174)**, as clang: `diagnoseQualifiedDeclaration`
  (SemaDecl.cpp) skips the does-not-enclose check only when given a
  template-id or `IsMemberSpecialization`. `HandleDeclarator` passes the
  declarator's template-id and `IsMemberSpecialization = false`, so of
  the variables and functions only a template-id declarator
  (`template<> void A::f<int>()`, `template<> int A::vt<char>`) is
  exempt; its scope is checked as a specialization's
  (`CheckTemplateSpecializationScope`, kiln `check_spec_scope`). A
  function template, member function template, variable template, a
  member of a class template, an explicit specialization without a
  template-id (`template<> void A::g(int)`) and a member specialization
  (`template<> void A::S<int>::g()`) all get the scope error. Tags
  (`diagnose_qualified_tag`; clang's `ActOnTag` with `TemplateId =
  nullptr, IsMemberSpecialization` and `CheckClassTemplate` with
  neither): a class, enumeration, class template, member class of a class
  template or nested class template defined or redeclared out of its
  scope is diagnosed the same way and still declared (clang marks it
  invalid and goes on); only a member class specialization (`template<>
  struct A::S<int>::N {}`, also with an extraneous `template<>`) is
  exempt (checked as a member specialization, below), and a class
  template (partial) specialization, a template-id, is checked by
  `check_spec_scope`. In a namespace a tag's qualifier
  naming that namespace is `-Wextra-qualification`. Directly in an
  export, `export struct ::X {}` is "cannot name the global scope" (clang
  checks `TranslationUnitDecl` before `ExportDecl`) and the redeclaration
  is still checked for its linkage (spec/modules.md). Tests:
  `tests/conformance/dcl.meaning/qualified-template-not-enclosing.cpp`,
  `qualified-template-enclosing-run.cpp`,
  `tests/conformance/module.interface/export-qualified-tag-errors.cpp`.
  A class template specialization scope prints with its arguments
  (`'A<int>'`, clang's `getNameForDiagnostic`; WP-1178).
  **Member specializations (WP-1178)**, clang's
  `CheckTemplateSpecializationScope` ([temp.expl.spec]/3,
  [temp.spec.partial.general]/4): an entity whose context is a class is
  specialized in that class or in a namespace enclosing it
  (`check_member_spec_scope`), else "%select{class template|class
  template partial|variable template|variable template partial|function
  template|...|member class|member enumeration} specialization of 'X'
  not in class 'C' or an enclosing namespace" with "explicitly
  specialized declaration is here" at the specialized member. A member
  class or enumeration specialization (`template<> struct N::A<int>::I`,
  scoped or unscoped, opaque or defined, a declaration or a definition;
  clang's `CheckMemberSpecialization`, after its after-instantiation
  check) names the pattern's class (`'A'`); an explicit or partial
  specialization of a member class, function or variable template
  (`template<> template<> struct N::A<int>::C<int>`) names the
  specialization (`'A<int>'`). An inline namespace of `N`, or one nested
  in it, does not enclose `N::A`; the global scope and `N` do. In another
  class the qualifier is "non-friend class member 'I' cannot have a
  qualified name" first, and the specialization is not declared (kiln:
  an invalid, hidden tag). At namespace scope a diagnosed specialization
  is still declared, as clang's recovery, for namespace templates
  (`check_spec_scope`) too. A member function or static data member
  specialization, and a member template declared without a template-id
  (`template<> template<class U> struct N::A<int>::C {}`, clang's
  `CheckClassTemplate`), get the does-not-enclose error above instead.
  Tests: `tests/conformance/temp.expl.spec/member-spec-not-enclosing-errors.cpp`,
  `member-template-spec-not-enclosing-errors.cpp`,
  `member-spec-enclosing-scope-run.cpp`.
- **Default-initialization without a default constructor**
  (`check_class_default_init`, WP-191): a class variable with no
  initializer whose default constructor is missing is resolved as
  `T x()`'s overload set, so the error is "no matching constructor for
  initialization of 'S'" with a note per candidate (clang
  `InitializationSequence` with no arguments).

### 4.3 Functions
- `act_on_start_function` and `act_on_finish_function` bracket a body.
- The body is checked for:
  - redefinition
  - incomplete parameter and return types (with a forward-declaration
    note)
  - falling off the end of a non-`void` function (`falls_off`, the
    clang CheckFallThrough port on the CFG's `exit_kinds`, §7.5). This
    warning is suppressed when the function had errors.
- `main` must return `int` ([basic.start.main]/2). Falling off the end
  of `main` returns 0, so it never warns.
- **Default arguments** (`act_on_param_default_init`,
  [dcl.fct.default]): copy-initialization of the parameter's type,
  from an expression or a braced list, stored in the `ParamInfo` as
  `DefaultArg::Parsed`; `act_on_declarator` merges them with the previous
  declaration's and sets each `ParmVar`'s. (A member function's default
  argument saved for the complete-class context is `DefaultArg::Unparsed`
  and replayed through `act_on_param_default_arg`, sema-class.md §2.7.)
  Calls fill omitted trailing arguments with `CXXDefaultArgExpr`.
  A function parameter pack after a defaulted parameter needs no
  default argument (`merge_default_args`, `lambda_default_args`). A
  specialization's default arguments stay pending until a call uses
  them ([temp.inst]/12, `defer_default_args`) also when a pack
  expanded: the pattern's parameters pair with the specialization's
  around the expansion (`param_pairs`), so `f(C c = C{}, A&&... a)`
  called as `f(x)` never substitutes `C{}` (WP-690). Test:
  `dcl.fct.default/pack-after-default.cpp`.
- **Erroneous default arguments** (WP-293; clang's
  `ActOnParamDefaultArgumentError`). A default argument parsed in place
  whose expression is erroneous — including an explicit cast that fails
  after its diagnostic (`(int)"s"`: a pointer to a smaller integer, a
  vector size mismatch; `reinterpret` returns `Err(())` for these
  instead of a cast kind) — makes the parameter and the function invalid
  (see *Invalid function declarations*, §4.7). Uses are not diagnosed
  again: a call to a non-template function that would use it, a
  constant-evaluated call, and a member function's delayed default
  argument (replayed in the complete-class context, which leaves the
  member valid) are all silent. A function template that is invalid
  fails deduction with `Tdk::Invalid` (WP-308's `deduce_spec` check;
  clang's `TemplateDeductionResult::Invalid`) for every call, so the call is `no
  matching function for call to 'f'` and the template is not listed as a
  candidate. A redeclaration of an invalid function adds no `missing
  default argument on parameter` error (`merge_default_args` with
  `trailing == false`); a call that would need the invalid default
  argument is then measured against the redeclaration's arity. Test:
  `dcl.fct.default/invalid-cast-default-arg.cpp`. A non-template
  function invalid only because of an invalid parameter
  (`Sema::invalid_by_param`) is still named and called normally (WP-373;
  clang resolves the call through its overload set): the call's
  arguments are converted and their errors diagnosed (`m(1, {2})`
  through an explicit constructor), and only then is the call itself
  erroneous and silent (a recovery expression, never constant-evaluated).
  A function invalid for another reason (its instantiated body failed)
  keeps the earlier silent recovery. Test:
  `dcl.fct.default/invalid-default-arg-still-callable.cpp`.
- **`register`** ([dcl.stc]/1; removed in C++17 by P0001R1, clang's
  `ext_register_storage_class`). `register` is `ISO C++17 does not allow
  'register' storage class specifier` (`sema_register_storage_class`,
  -Wregister, an error by default) at the specifier on a parameter
  (`act_on_param_declarator`, before the type is checked; lambda and
  function template parameters too) and on a variable or structured
  binding at block scope, in a for-init-statement and as a range-for
  variable (after `loop variable 'x' may not be declared 'register'`;
  after `decomposition declaration cannot be declared 'register'` for a
  binding), and the declaration continues. A variable with an asm label
  (`register int r asm("x1")`) is exempt (its register is checked under
  *GNU register variables*). At namespace scope a variable,
  structured binding or variable template additionally gets `illegal
  storage class on file-scoped variable` (`sema_illegal_storage_class_on_var`)
  at its name and is invalid (`register_at_file_scope`); its initializer
  is discarded unchecked. Another storage class on a parameter is
  `invalid storage class specifier in function declarator`. A condition
  declaration gets the parser's storage-class error instead (spec/parse.md
  §4) and a member `storage class specified for a member declaration`.
  Tests: `dcl.stc/register-storage-class.cpp`,
  `dcl.stc/register-storage-class-namespace.cpp`.
- **Where a storage class is rejected** ([dcl.stc], WP-327). Every
  context, with its diagnostic:
  - a type-specifier-seq (type-id, new-type-id, conversion-type-id,
    functional cast, condition, exception-declaration): the parser's
    `type name does not allow storage class to be specified`
    (`parse_typename_storage_class`; spec/parse.md §4). Test:
    `dcl.stc/storage-class-type-id.cpp`;
  - a friend declaration: `'…' is invalid in friend declarations`
    (below);
  - a parameter: `invalid storage class specifier in function
    declarator` (`register` instead gets -Wregister, above);
  - a member declaration: `storage class specified for a member
    declaration` (`register`, `extern`), `'mutable' cannot be applied to
    functions` / `references`, `'mutable' and 'const' cannot be mixed`;
  - a function outside a member declaration with `mutable` or
    `register`: `illegal storage class on function` (§4.7);
  - `register` on a variable at namespace scope: `illegal storage class
    on file-scoped variable` (above);
  - **`mutable`** ([dcl.stc]/9) on a variable that is not a non-static
    data member — namespace or block scope, a for/range-for variable, a
    structured binding (after `decomposition declaration cannot be
    declared 'mutable'`), a variable template, an out-of-class static
    data member definition: `'mutable' can only be applied to member
    variables` (`sema_mutable_nonmember`, clang `err_mutable_nonmember`)
    at the name (a binding's `[`), from `var_spec`; the variable is
    invalid (`storage_invalidates_var`; the static member in
    `act_on_static_member`), so its initializer is not checked. Test:
    `dcl.stc/mutable-nonmember.cpp`;
  - an **out-of-class static data member definition**
    ([class.static.data]/2-3, `act_on_static_member`): `static` is
    `'static' can only be specified inside the class definition`;
    `extern` and `register` (after -Wregister) are `static data member
    definition cannot specify a storage class`
    (`sema_storage_class_for_static_member`, clang
    `err_storage_class_for_static_member`), at the specifier; the storage
    class is dropped and the definition stays valid. Test:
    `class.static.data/out-of-class-storage-class.cpp`.
- **GNU register variables** (`register T v asm("reg")`, [dcl.asm];
  clang `ActOnVariableDeclarator`; `check_register_variable`, WP-327).
  At block scope the name must be one the target knows
  (`kiln_abi::asm::is_valid_register_name`): else `unknown register name
  '…' in asm` at the label. At namespace scope, in order: an unknown
  name is that error; a name the target cannot hold a global variable
  in (`global_register_size_mismatch` returns `None`: AArch64 accepts
  `sp` and names starting `x` (64 bits) or `w` (32 bits); x86_64 `rsp`,
  `rbp` (64) and `esp`, `ebp` (32), as written, so `%rsp` is
  unsuitable) is `register '…' unsuitable for global register variables
  on this target`; a variable whose size differs is `size of register
  '…' does not match variable size` (clang 19 checks sizes only for
  globals; a dependent type's size is not checked). Then a type that is
  neither integral (enums are not) nor a pointer is `bad type for named
  register variable` at the decl-specifiers, and the variable is
  invalid. Tests (pinned targets): `dcl.asm/global-register-variable-aarch64.cpp`,
  `dcl.asm/global-register-variable-x86_64.cpp`.
- **Storage classes in friend declarations** ([class.friend]/6, WP-293):
  diagnosed by the parser as `'mutable' is invalid in friend
  declarations` (likewise `static`, `extern`, `register`, `typedef`,
  `thread_local`, `__thread`, `'static thread_local'`) and dropped
  before Sema sees the DeclSpec (spec/parse.md §4), so the friend
  function is declared as without it; a friend `typedef` or variable is
  then `friends can only be classes or functions`. Test:
  `class.friend/friend-storage-class.cpp`.
- **Trailing return types** (`Chunk::Function::trailing`,
  [dcl.fct]/2): the function's return type, and an error
  (`function with trailing return type must specify return type 'auto'`)
  unless the decl-specifier is a lone `auto`. A return type of array or
  function type ([dcl.fct]/11, `function cannot return function type`)
  is diagnosed at that `auto` type-specifier when the return type is
  trailing (the inner `auto` of a nested `auto (*)() -> F`; also after
  substitution), otherwise at the function declarator's `(`, as clang
  (`dcl.fct/return-function-type-trailing.cpp`). A lambda has no
  `auto` to point at: its trailing-return checks use the lambda
  declarator's `(`.
- **Volatile return types** ([depr.volatile.type]/2, C++20 and later):
  a function declarator whose return type is volatile-qualified (also
  through a typedef; not a pointer to volatile) warns
  `volatile-qualified return type T is deprecated`
  [-Wdeprecated-volatile] (`sema_deprecated_volatile_return`, in
  `-Wdeprecated`), at the same location as the array/function return
  check. After template substitution (WP-392, `check_volatile_return`)
  a return type that became volatile warns at the return type's begin:
  at the class template's instantiation for a member, at the alias's
  use, and for a function template specialization formed by deduction
  at its first use (held back by SFINAE and replayed, sema-templates.md
  §5.7). A non-dependent volatile return type is not re-warned, and a
  deduced `auto` / `decltype(auto)` return never warns. Tests:
  `depr.volatile.type/volatile-return{,-cxx17,-subst,-subst-19}.cpp`.
- **Volatile parameter types** ([depr.volatile.type]/2, C++20 and
  later; WP-417): a parameter whose declared type is volatile-qualified
  (before [dcl.fct]/5 drops the qualifier; a typedef keeps its sugar; a
  pointer to volatile is not) warns `volatile-qualified parameter type
  T is deprecated` [-Wdeprecated-volatile]
  (`sema_deprecated_volatile_param`) at the parameter
  (`check_volatile_params`). After substitution a parameter that became
  volatile warns as a volatile return type does: at a class member's
  instantiation, and for a deduced function template specialization at
  its first use. Tests: `depr.volatile.type/volatile-param{,-19,-cxx17}.cpp`.
- **Volatile increment, decrement, assignment and structured bindings**
  ([depr.volatile.type]/1-3, C++20 and later; WP-432, clang
  `CheckIncrementDecrementOperand` / `CheckAssignmentOperands` /
  `ActOnDecompositionDeclarator`):
  - `++` / `--` (prefix or postfix) of a volatile-qualified object warns
    `increment|decrement of object of volatile-qualified type T is
    deprecated` at the operator, after the modifiable check
    (`check_volatile_inc_dec`, `sema_deprecated_volatile_incdec`);
  - a simple assignment to a volatile-qualified object outside an
    unevaluated operand records its left operand (`note_volatile_assign`);
    discarding the assignment's value (`ignored_value_conversions`, after
    parentheses and implicit casts) drops it (`discard_volatile_assign`);
    one still recorded when the function body, a namespace-scope
    initializer or the translation unit ends warns `use of result of
    assignment to object of volatile-qualified type T is deprecated` at
    the left operand's begin (`sema_deprecated_volatile_assign`). A
    compound assignment never warns (clang 19 and 22 do not);
  - a `volatile` structured binding declaration warns `volatile
    qualifier in structured binding declaration is deprecated` at the
    `volatile` keyword (`sema_deprecated_volatile_binding`).
  All [-Wdeprecated-volatile]. Tests:
  `depr.volatile.type/volatile-incdec-assign{,-cxx17}.cpp`.
- **A `...` in a non-parameter declarator** ([temp.variadic]/5): the
  parser keeps it in `Declarator::ellipsis`; `type_for_declarator_at`
  reports `only function and template parameters can be parameter
  packs` (`sema_ellipsis_in_declarator_not_parameter`) at it for every
  context but a prototype or a type-name, and the declared type keeps
  its unexpanded pack. Test `temp.variadic/non-parameter-pack-declarator.cpp`.
- **Prototype scope** ([basic.scope.param]): the parser brackets each
  parameter-declaration-clause with `act_on_start_prototype`,
  `act_on_prototype_param(&mut ParamInfo)` and `act_on_end_prototype`.
  `act_on_prototype_param` creates the `ParmVar` at its point of
  declaration (`AstBuilder::prototype_param`, provisionally owned by the
  enclosing context; `ParamInfo.decl`) and pushes it on
  `prototype_params`; unqualified lookup finds it in the rest of the
  declarator: later parameters, default arguments, the exception
  specification and the trailing return type. When the function is
  declared, `create_params` adopts the prototype parameters
  (`DeclArena::set_param_owner`), so the `DeclRefExpr`s in the
  declarator name the function's own parameters; a parameter left
  unadopted (a declarator that is not a function declaration) stays in
  `unowned_params`.
- **Default arguments** ([dcl.fct.default]/9): `check_default_arg_refs`
  reports `default argument references parameter 'x'` for a potentially
  evaluated reference (not inside `sizeof`, `decltype`, … ), on the
  argument as written and on one replayed in the complete-class context.
- **`this` in the declarator** ([expr.prim.this]/2):
  `act_on_start_this_scope(member, quals)` / `act_on_end_this_scope`
  bracket a member function's exception specification and trailing
  return type, where `this` has type `cv X*` (not a friend function's:
  clang's `InitCXXThisScopeForDeclaratorIfRelevant`, WP-669,
  `expr.prim.this/p2-not-in-friend-declarator.cpp`); member access into the
  class being defined is allowed there (and in NSDMIs, default
  arguments and bodies, which run after `}`). Anywhere else before the
  class's `}` (a static data member initializer, an array bound, an
  alias, a `decltype` member type, a `static_assert`) `((S*)0)->m()`
  is `member access into incomplete type 'S'` with the note
  `definition of 'S' is not complete until the closing '}'` (clang's
  `isThisOutsideMemberFunctionBody`; `Sema::this_outside_member_body`;
  WP-280; test `expr.ref/member-access-class-being-defined.cpp`).
  A member of incomplete type (`field has incomplete type`, the class
  itself or another incomplete class) makes the class invalid
  (`Sema::invalid_defining`, clang's `CheckFieldDecl` marking the record
  invalid); until its `}` a later use that needs it complete (member
  access, another such member, `sizeof`) fails without a diagnostic
  (`incomplete_note` suppresses it), as in clang. Unrelated errors in the
  class are still reported (WP-290,
  `expr.ref/member-access-invalid-class-quiet.cpp`).
- **`-Wvexing-parse`**: a function declarator the parser chose over a
  direct initializer by [dcl.ambig.res]/1 warns as clang does, with
  its fix-it notes (`add a pair of parentheses`, `replace parentheses
  with an initializer`, `remove parentheses`).
- **Function qualifiers where none are allowed** ([dcl.fct]/6, clang
  `err_invalid_qualified_function_type`, WP-435): a cv- or
  ref-qualifier on a non-member or friend function is "non-member
  function cannot have 'Q' qualifier", on an explicit-object member
  function "explicit object member function cannot have 'Q'
  qualifier", both at the first qualifier (`Declarator::fn_qual_locs`)
  with every qualifier spelled; the qualifiers are dropped from the
  function type, so an instantiation does not report them again and
  `void f() const; void f() {}` declares one function. A free function
  with an explicit object parameter reports that first, a friend after;
  an explicit-object static or virtual member reports the qualifier
  first, then "cannot appear in a static/virtual function" (the
  declaration is invalid). A static member function's qualifier error
  is also at its first qualifier. Tests
  `dcl.fct/nonmember-function-qualifiers.cpp`,
  `dcl.fct/explicit-object-qualifiers-template.cpp`.
- **Qualified function types through a typedef-name** ([dcl.fct]/6,
  CWG1417, WP-459, `decl.rs` `check_qualified_fn_type`): a function type
  with a cv-qualifier-seq or ref-qualifier named by a typedef-name or
  `decltype` (no function declarator of its own) is only the type of a
  non-static member function, of a typedef or alias-declaration, of a
  template type argument (a type trait's operand parses as one), of a
  type parameter's default, the operand of `^^` (it declares nothing;
  GCC 16.1 agrees, WP-477), or the pointee of a pointer to member. As a
  non-member function at namespace or block scope (`extern` included), a
  static member function, a friend, a parameter, a handler, or in a
  type-id (`sizeof`, `new`, a trailing return type) it is clang's
  `err_invalid_qualified_function_type` in its "of type" form: "non-member
  function of type 'F' (aka 'int () const') cannot have 'const'
  qualifier" ("static member function of type ..." for a static member)
  at the start of the declaration or type-id, and the qualifiers are
  dropped (follow-on errors print `int ()`). A pointer or reference to
  such a type stays `check_qualified_function`'s (WP-435). In a template
  instantiation, a data member, variable or static data member whose type
  substitutes to a function type (qualified or not) is "data member /
  variable / static data member instantiated with function type T" at its
  name (clang `VisitFieldDecl`/`VisitVarDecl`, [temp.inst]/17); a member
  function's parameter of that type is not diagnosed (it adjusts to a
  pointer). `friend F;` naming a qualified function typedef (plain or
  nested-name-qualified, `friend N::R;`) is the "non-member function of
  type" form at the start of the declaration (`class/friend.rs`
  `declare_friend_type`, WP-469); an unqualified function typedef is an
  ignored non-class friend type ([class.friend]/3), and a dependent
  `friend T;` substituting to a qualified function type is not diagnosed
  (a non-dependent one in a template is reported once, in the pattern;
  clang 19 and 22). Tests
  `class.friend/friend-qualified-function-typedef.cpp`,
  `dcl.fct/qualified-function-typedef.cpp`,
  `dcl.fct/qualified-function-type-id.cpp`,
  `dcl.fct/qualified-function-instantiation.cpp`,
  `dcl.decl.general/qualified-function-decltype.cpp`.
- A function declared through a typedef of function type (`typedef int
  G(int) const; struct S { G g; };`, [dcl.fct]/13) has no parameter
  declarations of its own; `act_on_function` synthesizes one unnamed
  parameter per prototype parameter (clang `BuildParmVarDeclForTypedef`),
  so a later out-of-class definition's parameters map onto them and its
  body instantiates with them bound. In a class template the member
  substitutes through the sugared function type (`subst/decls.rs`
  `function_decl`: no declarator chunk, no trailing return; WP-469), for a
  plain, cv-qualified, static, nested-class or dependent (`typedef T
  H(T); H h;`) typedef. Test `temp.inst/member-function-typedef.cpp`.

### 4.3.1 `decltype` and placeholder types (`deduce.rs`)
- A decl-specifier `restrict`/`__restrict` (C99 6.7.3p2, clang
  `BuildQualifiedType`; WP-371) on a type that is not a pointer,
  reference or member pointer (and not dependent or an error) is
  "restrict requires a pointer or reference ('int' is invalid)" at the
  qualifier, which is dropped. A placeholder is checked before deduction,
  so `auto __restrict p = &i;` is "('auto' is invalid)" and a
  parameter's `auto`/`C auto` (`void g(auto __restrict x)`, `this auto
  __restrict self`) is "('C auto' is invalid)" before it becomes an
  invented template parameter (`reject_placeholder_restrict`, WP-1091)
  (`type_from_declspec_at`, so `__restrict int *p;` qualifies `int` and
  is diagnosed). As clang forms each declarator's type anew
  ([dcl.decl.general]/3), it is reported once per declarator: the
  group's type is formed once, and the parser repeats the error for each
  later declarator (`take_declspec_restrict_error`,
  `report_restrict_not_pointer`; `__restrict int e, *f;` reports it
  twice). A `__restrict` ptr-operator qualifies its own declarator only
  (`int *__restrict a, *b;`: `b` is `int *`; WP-379,
  `dcl.decl/restrict-per-declarator.cpp`). `_Atomic int g();` (friend or
  not) is accepted, as clang 19 and 22 do;
  `class.friend/restrict-atomic-decl-specifiers.cpp`.
- `act_on_decltype(e)` ([dcl.type.decltype]): `DecltypeType` sugar
  over the declared type of an unparenthesized id-expression or member
  access, else `T&&`/`T&`/`T` by value category. The operand is
  unevaluated. An operand of `decltype` or `noexcept` that is a bound
  member function (`obj.*pmf`, `obj.f`) or a builtin function name is
  ill-formed ([expr.mptr.oper]/6, [expr.ref]): "reference to non-static
  member function must be called" (with clang's "did you mean to call it
  with no arguments?" when the member function, or the pointee function
  type of `pmf`, takes none) or "builtin functions must be directly
  called", so substituting one into a return type is a deduction failure
  (`check_builtin_fn_use`, WP-669; `expr.mptr.oper/p6-bound-member-*.cpp`).
- `auto` and `decltype(auto)` variables ([dcl.type.auto.deduct]): a
  top-level placeholder deduces the decayed, cv-stripped initializer
  type plus the written cv-qualifiers, or `decltype(e)`. A
  placeholder under pointer, member-pointer and reference declarators
  (`auto *`, `auto Q::*`, `const auto &`, `auto &&`) is deduced as for a function call
  ([temp.deduct.call]/2-4) with `auto` as the template parameter.
  A type-constraint on the placeholder (`C<A> decltype(auto) x = e;`)
  is checked against the deduced placeholder type
  ([dcl.type.auto.deduct]/2.2, /4), not the variable's type: the
  `const` that `constexpr` adds ([dcl.constexpr]/10) is not part of it,
  so `constexpr same_as<const bool> decltype(auto) r = X::v;` (with
  `decltype(X::v)` = `const bool`) is satisfied and `constexpr
  same_as<const int> decltype(auto) d = 1;` is `deduced type 'int' does
  not satisfy 'same_as<const int>'` (`deduce_var_auto`, WP-335).
  The checked concept-id `C<T, A...>` is completed with `C`'s default
  template arguments first, as for a written concept-id
  ([temp.arg.general]/9): with `template <class S, class E = env>
  concept K = sender_in<S, E>;`, `K auto f()` checks `K<T, env>` for
  variables, return types and parameters alike
  (`check_placeholder_constraint`, WP-789). The
  variable's type still carries that `const` unless `decltype(e)` is a
  reference: `constexpr decltype(auto) b = 1;` is `const int`,
  `constexpr decltype(auto) r = (g);` is `const int &` for a `constexpr
  int g` (`deduce_placeholder`).
  A copy-list-initializer (`auto l = {...}`, and the range of a
  range-based `for` over a braced list) deduces `std::initializer_list<U>`
  from each element; a nested braced-init-list element is a non-deduced
  context ([temp.deduct.call]/1) and is left out (`auto_list_items`), so
  `{P{0, 1}, {2, 3}}` deduces `U = P<int, int>` and `{{1}}` deduces
  nothing (`cannot deduce actual type ... from initializer list`), as
  does a designated element (WP-339).
- Deduced return types ([dcl.spec.auto.general]/9-11): the first
  `return` deduces, later ones must agree (clang's
  `'auto' in return type deduced as ... here but deduced as ...`),
  and a use before deduction is `function 'f' with deduced return type
  cannot be used before it is defined`. Every use checks this
  (`check_undeduced_use`), `&C::f` included (`member_ref_without_object`):
  a specialization instantiates its definition first, so
  `&C::f<int>` of a member function template has the deduced type
  (WP-690,
  `tests/conformance/dcl.spec.auto/member-template-address-deduced.cpp`).
  A member access `x.f` or `this->f` naming a static member function
  checks it too (`member_of_object`), so the call through it has the
  deduced type (WP-690,
  `tests/conformance/dcl.spec.auto/static-member-through-dependent-base.cpp`).
  When the declaration the use names has no instantiable definition,
  another redeclaration instantiated from a friend definition is
  instantiated instead ([temp.inst]/4): a friend declared without a body
  in `key<N>` and defined with a deduced return type in `value<T>` is one
  function, so once `value<T>` exists `decltype(lookup(key<N>()))` has
  its deduced type (clang's `isDefined` counts a declaration instantiated
  from a friend definition; the type map of std::execution
  implementations, WP-789,
  `tests/conformance/temp.friend/deduced-friend-defined-by-other-specialization.cpp`).
  A redeclaration
  ([dcl.spec.auto.general]/13) is matched against the declared
  placeholder type (`declared_fn_type`, kept in `declared_returns`), not
  the deduced one, and then adopts the deduced type
  (`adopt_deduced_return`), so `auto f(); auto f() { return 1; } auto
  f();` is valid and `int f();` after it is not.
- Deduced return types in templated functions (WP-122,
  [dcl.spec.auto.general]/12): when the body is in a dependent context
  (`in_template()`), `begin_function_body` gives the pattern the
  return type `dependent_auto(declared)` (clang's AutoType "deduced as
  dependent", printed `auto`, canonical to itself so it mangles `Da`/
  `Dc`) and `deduce_return` checks nothing. The pattern's returns are
  left unconverted, `return;` / `return void();` are not diagnosed
  against the dependent type, and there is no NRVO candidate
  (`copy_elision_candidate`), as in clang. Deduction runs when the body
  is instantiated, so a `return` in a discarded `if constexpr` branch
  never participates (libc++ `__get_comp_type`) and only a real
  conflict between instantiated returns is reported. In a
  non-templated function a `return` inside a discarded statement
  (`in_discarded_statement`) likewise takes no part in deduction
  ([dcl.spec.auto.general]/9) and its operand is left unconverted, as
  in clang (WP-792). `return {…}` is
  still an error in the pattern. A call `x.g()` to a member whose
  return type is not yet deduced goes through `check_undeduced_use`
  (instantiating it when it is a specialization member) before
  member-call building. A use in a class template of a member function
  template's specialization, before the member template's body is
  parsed (`static constexpr auto v = compute<true>();`), is
  type-dependent like a use of a member function of the class
  (`undeduced_dependent_use` also takes a specialization whose template
  is in a dependent context; WP-787,
  `tests/conformance/dcl.spec.auto/deduced-member-template-used-in-class-template.cpp`).
- Declarator groups ([dcl.spec.auto.general]/3, /8): the parser records
  each declarator of a simple-declaration or member-declaration with
  `act_on_group_declarator(&mut AutoGroup, d)` and closes the group with
  `act_on_end_group(group, &ds)`, which reports, at the end of the group
  as clang does, a function with a deduced return type or a trailing
  return type that is not alone in its group
  (`sema_auto_non_deduced_not_alone`) and placeholders deduced to
  different types (`check_auto_group`). A local group in a template is
  checked again on each instantiation (`group_placeholders`); a class
  template's static members are not (as in clang).
- `-Wunevaluated-expression` ([expr.context]/1; `side_effects.rs`, in
  `-Wmost`): an operand of `sizeof`, `alignof`, `decltype`, `noexcept`
  or an unevaluated `typeid` with side effects (increment, assignment,
  `new`, `delete`, `throw`, `co_await`/`co_yield`, a potentially
  evaluated `typeid`; calls only through their arguments) warns
  `expression with side effects has no effect in an unevaluated
  context`, at clang's expression location. Not in instantiations, not
  for type- or value-dependent operands, not for macro locations.
- **`[[assume(e)]]`** ([dcl.attr.assume]; WP-160, `stmt.rs`
  `act_on_attributed_stmt`). Clang's order is followed for each attribute:
  - On a declaration: `'assume' attribute cannot be applied to a
    declaration`.
  - On a statement that is not null: `'assume' attribute only applies to
    empty statements`.
  - Otherwise, `build_cxx_assume_expr` contextually converts the
    assumption to `bool` and makes it a full-expression. It warns
    `assumption is ignored because it contains (potential) side-effects`
    (`-Wassume`, at the expression's begin location) when
    `side_effects.rs` `has_possible_side_effects` holds. That predicate is
    clang's `HasSideEffects(IncludePossibleEffects=true)`: a call to a
    function that is not `const`/`pure`, a volatile read, a `dynamic_cast`
    to a reference, a non-trivial construction, a temporary with a
    destructor, `new`/`delete`/`throw`, assignment, increment or
    decrement, any dependent or unresolved node, and, through
    default-argument, default-member-initializer and lambda-capture
    initializers, whatever they contain.
  - Before C++23, `-pedantic` adds `use of the 'assume' attribute is a C++23
    extension` (`-Wc++23-attribute-extensions`, which is not under
    `-Wc++23-extensions`).
  - The result is an `AttributedStmt` with one `ASSUME` attribute per
    assumption kept: `a` is the expression and `b` is clang's
    `getExprLoc`. `AttrNode::ASSUME_IGNORED` marks a side-effecting or
    dependent assumption, and codegen and the oven skip those.
  - A dependent assumption is converted and checked again on
    instantiation (`templ/subst/stmts.rs`).
- **`[[likely]]`, `[[unlikely]]`, `[[fallthrough]]`**
  ([dcl.attr.likelihood], [dcl.attr.fallthrough]; WP-182, `stmt.rs`
  `act_on_attributed_stmt`). They are kept in the same `AttributedStmt`
  (clang `ProcessStmtAttributes`), in written order:
  - On a declaration: `'likely' attribute cannot be applied to a
    declaration` (each name).
  - `fallthrough` on a statement that is not null: `'fallthrough'
    attribute only applies to empty statements`; outside a switch of the
    current function (a lambda body has its own): `fallthrough annotation
    is outside switch statement` (clang `handleFallThroughAttr`, checked
    in a template definition). `[[clang::fallthrough]]` is accepted
    (WP-203).
  - **Placement (WP-203; `fallthrough.rs`, clang
    `DiagnoseSwitchLabelsFallthrough`)**: when the body built a
    fallthrough statement (`FnState::has_fallthrough`; an instantiation
    copies them and is not checked, as clang's `HasFallthroughStmt` is
    only set by `ProcessStmtAttributes`), `end_function_body` checks,
    after the return-type check and before the throw-in-nothrow check
    and like them only with no error so far and outside a template,
    that each one directly precedes a switch label: `fallthrough
    annotation does not directly precede switch label` at the
    `AttributedStmt`. A reachable one must be followed, past null
    statements and empty blocks and out of a compound, branch, try block,
    handler or inner switch, by a `case`/`default` label (a loop body
    flows back, a named label starts a new block); an unreachable one
    (`unreachable_attributed_stmts`, every case label reachable, clang
    `fillReachableBlocks`) needs only be the last fallthrough of a
    straight-line run (expression and declaration statements) that
    falls into a label. With no CFG (`if consteval`) nothing is checked.
    The errors come in clang's `SmallPtrSet<_, 8>` order: source order,
    each valid one erased (by its label's order) moving the last element
    into its hole; past 8, source order (clang's is by pointer).
  - **Unannotated fall-through (WP-217; `implicit_fallthrough.rs`,
    clang `DiagnoseSwitchLabelsFallthrough` / `FallthroughMapper`)**,
    run first in `late_analysis` (so after the queued runtime warnings
    and before the placement errors), not in a template definition or
    once an error occurred, but in each instantiation:
    - A label's predecessors are found structurally (clang reads the
      graph's block predecessors; the two agree on the corpus):
      the statements before it in its block, walked back past empty
      ones (`;`, `{}`, an empty label block, which clang's
      `getLastStmt` counts as the predecessor itself), the branches of
      an `if` ending there (plus the false edge of an `if` with no
      else), and the condition of an `if`/`while`/`for` whose branch or
      body the label starts. A label starting a `do` body (WP-234) is
      entered from the statements before the `do` (walked as above) and
      from the condition (clang's empty loop-back block, whose
      predecessor ends in the `do`), counted unannotated when the
      condition is reached (the body falls off its end or a `continue`
      of that loop is reached) and is not a constant false; the
      placement check likewise lets a `[[fallthrough]];` before such a
      `do` precede that label. Reachability comes from the body's CFG
      (`Cfg::stmt_flow`, §7.5.1): each statement's entry and exit
      position is live when a live edge reaches it from the entry or
      from any case block (clang `fillReachableBlocks`). A switch dispatch, a label directly
      before (`case 1: case 2:`, `L: case 2:`) and an unreachable
      predecessor are good; a reachable `[[fallthrough]];` is
      annotated; anything else is unannotated.
    - At each label with an unannotated predecessor:
      `unannotated fall-through between switch labels`
      [-Wimplicit-fallthrough, off by default, not in -Wall/-Wextra] at
      the `case`/`default`. In an instantiation the instantiation note
      comes next. Then, when no predecessor is annotated and the label
      is not spelled by a macro: `insert '<spelling>;' to silence this
      warning` unless the label's block holds
      only `break`, then `insert 'break;' to avoid fall-through`, each
      with an insertion fix-it at the label. The spelling (WP-234, clang
      `getFallthroughAttrSpelling`) is the name of the latest-defined
      object-like macro in effect at the label whose replacement list is
      `[[fallthrough]]` or `[[clang::fallthrough]]` (before C++17,
      `Sema::std`, the clang spelling is tried first; from C++17 the
      standard one), else `[[fallthrough]]` (`[[clang::fallthrough]]`
      before C++17). The macro history comes from the preprocessor's
      `attr_macros` (spec/pp.md), copied in by the parser through
      `Sema::sync_attr_macros` before each function or lambda body is
      finished; locations are ordered by their include chains, and a
      macro from outside the main file's include tree (predefines, `-D`)
      precedes everything.
    - An unreachable `[[fallthrough]];` that falls into a label (found by
      scanning the unreachable straight-line run backwards): `fallthrough
      annotation in unreachable code` [-Wunreachable-code-fallthrough, in
      -Wunreachable-code] at the attribute, before that label's warning;
      it counts as annotated. Not in an instantiation.
    - Gaps: a label in a range-`for` body, a `try` block or a handler
      is not followed (jumping there is ill-formed, so clang stops at
      the error); -Wimplicit-fallthrough-per-function is not
      implemented; the macro history a function's labels see is the one
      at the end of its body (a later `#define` cannot precede a label,
      so only a lookahead past the `}` is visible, and it is filtered by
      location).
  - A `likely` after an `unlikely` (or the reverse) in the same
    sequence: `'unlikely' and 'likely' attributes are not compatible` at
    the second, with `note: conflicting attribute is here` at the first;
    the second is dropped.
  - `-pedantic` before C++17 (`fallthrough`) or C++20 (`likely`,
    `unlikely`): `use of the 'x' attribute is a C++17/C++20 extension`
    (`-Wc++17-attribute-extensions`, `-Wc++20-attribute-extensions`,
    not under `-Wc++NN-extensions`). A scoped `[[clang::fallthrough]]`
    / `[[gnu::fallthrough]]` (`AttrNode::FALLTHROUGH_SCOPED`) is no
    extension (WP-203).
  - `act_on_if`: both branches `AttributedStmt`s with the same
    likelihood warn `conflicting attributes 'likely' are ignored`
    (`-Wignored-attributes`) at the then-branch attribute, with the
    note at the else-branch one. As clang's `Diags.Report`, it goes
    straight to the engine (`allow_duplicate`, no instantiation notes),
    so each instantiation repeats it, as clang 19 does (checked, WP-203:
    `conflict-per-instantiation.cpp`).
  - Instantiation keeps these attributes unchanged
    (`templ/subst/stmts.rs`); they are not checked again.
  - The parser rejects an argument list on a standard attribute that
    takes none (`carries_dependency` before C++26, `fallthrough`, `noreturn`,
    `no_unique_address`, `likely`, `unlikely`, `maybe_unused`,
    `indeterminate` from C++26, even
    `()`): `attribute 'likely' cannot have an argument list` at the `(`,
    and drops the attribute (clang `ParseCXX11AttributeArgs`).
  - Codegen ignores them: clang 19 consults likelihood only when
    optimizing (no `!prof` weights or `llvm.expect` at `-O0`;
    `tests/codegen/likelihood_attrs.cpp`). The oven evaluates an
    `AttributedStmt` as its statement with one step, and a switch body
    child that is an `AttributedStmt` directly on a `case`/`default`
    label (`[[likely]] case 1:`) is a link in the label chain
    (`attributed_label`), charging its step on the way in, as clang's
    `EvaluateStmt`.
- Braced initializers (`auto x = {1}`, `std::initializer_list`) are
  §10.

**Landed (WP-119).** `decltype(e)::` as a nested-name-specifier
([expr.prim.id.qual]/1). `act_on_nested_name_decltype(ty, loc) ->
Option<(DeclId, NnsId)>` builds `Nns::Type { prefix: None, ty, .. }`
over the `Decltype` sugar (printed `decltype(s)::`, dumped per clang)
and returns the scope it names:
- a non-dependent class or enumeration type: its definition, after
  `require_complete_nns` (an undefined specialization is instantiated,
  an incomplete class diagnosed);
- any other non-dependent type: `'decltype(i)' (aka 'int') is not a
  class, namespace, or enumeration` (`sema_expected_class_or_namespace`,
  at the `decltype`), `None`;
- a dependent type: one shared dependent marker per TU
  (`TemplState::decltype_scope`, an implicit unnamed `TypeAlias` of
  `QualType::DEPENDENT` made by `AstBuilder::unlinked_alias`), for which
  `is_dependent_ctx` is true. Names after it therefore take the existing
  dependent paths (`DependentNameType`, `DependentScopeDeclRefExpr`,
  `Nns::Identifier`), and the Substituter's `Nns::Type` arm substitutes
  the decltype and resolves the scope at instantiation (`nns_context`:
  `type 'decltype(int{})' (aka 'int') cannot be used prior to '::'`).
- The Substituter keeps the written keyword of a substituted
  `DependentNameType` (`typename S::type`, as clang's
  `ElaboratedType`); it was dropped before.

### 4.3.2 Class template argument deduction (`templ/ctad.rs`, WP-071b, WP-134)
[dcl.type.class.deduct], [over.match.class.deduct], [temp.deduct.guide].

**Implemented.**
- **Guides parse** at namespace and class scope: `[explicit] X(params)
  -> X<…>;`, as a template or not, with a requires-clause. The
  declarator name is `SpecialName::DeductionGuide(template)`, the
  declaration a `CXXDeductionGuide` (`act_on_deduction_guide`). Guides
  are found by lookup of that name in the template's context
  (`user_guides`).
- **Placeholder.** A class template name used as a type specifier is a
  `DeducedTemplateSpecialization` (DTS) type with no deduced type
  (`ctad_placeholder`). The parser accepts one in a simple-declaration,
  after `new`, and before `(`/`{` in an expression (functional cast).
- **Sites.** Each site calls `deduce_ctad(declared, args, braced, how,
  loc)` with `how` one of `CtadInit::{Direct, DirectList, Copy, CopyList}`:
  - variables (`act_on_init`; `act_on_uninit` for `X x;`);
  - `new X(…)`, `new X{…}`, `new X`;
  - `X(…)`, `X{…}`, `X()` (`ctad_cast_type`,
    `act_on_ctad_list_cast_type`).

  The result is the DTS with the deduced type (printed as the
  specialization, as clang), with the declared cv-qualifiers.
- **Guide set** (`ctad_guides`). The implicit guides are cached per
  template in `TemplState::ctad_guides` once the class has a
  definition; `TemplState::ctad_origin` records each one's origin
  (`GuideOrigin::{Ctor, CtorTemplate, Copy, Aggregate}`). The set is:
  - one implicit guide per non-template constructor of the primary
    definition (a hypothetical `X()` when there is none). An implicit
    guide is a detached `FunctionTemplate` over the class template's
    own parameter list (`AstContext::detached_template_decl`) with
    return type `X<T…>`, the constructor's parameters (names) and
    trailing requires-clause. A parameter with a default argument gets
    a source-less `OpaqueValueExpr` of its type as the guide's default
    (`guide_default_placeholder`, WP-152; clang's
    `transformFunctionTypeParam`, printed `<null expr>`): the guide has
    the constructor's arity but its default arguments are never
    substituted or evaluated (the constructor's expression names the
    constructor's template parameters, not the guide's). Substituting a
    guide specialization's declaration (`Substituter::default_args`)
    gives it another placeholder of the substituted type;
  - one guide per constructor template (`ctor_template_guide`, WP-134):
    the class parameters followed by the constructor template's,
    re-depthed one level shallower and renumbered after them
    (`ctad_redepth_param`; the class level is substituted by null
    arguments, which retain it); its constraints are the class
    template's and the constructor template's, conjoined;
  - the copy deduction candidate `X(X<T…>) -> X<T…>`;
  - P1816 (WP-134): when `X` has no user guides and the initializer is
    a non-empty braced or parenthesized list, the aggregate deduction
    candidate (`aggregate_guide`): one parameter per element the
    initializers initialize (bases, then fields), brace elision assumed
    only for a non-dependent sub-aggregate or an array whose bound is
    not value-dependent; a braced element or string literal for an
    array of dependent bound is `T(&&)[N]` / `const T(&)[N]`. Not cached: it depends on the initializer. Call
    deduction deduces `P'[N]` from a braced list (each element deduces
    `P'`, the length `N`);
  - the user guides (`user_guides`).
- **Member templates** (WP-134). A member class template of a class
  template specialization (`Outer<int>::In`, redeclared at
  instantiation, [temp.mem]) has no definition until one of its
  specializations is instantiated. Its implicit guides
  (`member_ctor_guides`) take the original member's non-template
  constructors, their parameter types substituted with
  `base_levels(tmpl)` plus the redeclared template's own parameters as
  arguments. Its member deduction guides are redeclared with it: the
  Substituter maps a `DeductionGuide(t)` name to the instantiated `t`.
  Each constructor template of the original member gives a guide
  (WP-152, `ctor_template_guide` with the member levels): the
  redeclared template's parameters followed by the constructor
  template's, retained at the redeclared template's depth.
  The member is named as a placeholder through an alias of the
  specialization (`using O = Outer<int>; O::In`) or through the
  template-id itself (`Outer<int>::In`, WP-152): the lookahead finds
  `In` in `Outer`'s primary pattern, so `parse_ctad_placeholder`
  classifies the name again in the context the parsed
  nested-name-specifier names and takes the instantiated member.
- **Alias templates (P1814, WP-134).** For `A<…>` naming an alias
  template whose defining-type-id is a simple-template-id `C<…>`, each
  guide `g` of `C` is transformed (`build_alias_guide`): `g`'s return
  type is deduced from the RHS (packs deduce element-wise,
  `DeducedArgs::expansions`); the alias parameters used by the
  deductions are kept (renumbered when only some are); the deductions
  are resugared to them; `g`'s undeduced parameters follow. The guide
  carries `ctad_alias[g'] = (alias, rhs)`, and `deduce_call` checks
  clang's `__is_deducible(A, R)` on the deduced return type
  (`note_alias_not_deducible`). An RHS that is not a simple-template-id
  is `sema_ctad_non_deducible_template`. Details (WP-311):
  - The deduction of `g`'s return type from the RHS may fail part way
    (`pair<A, B>`'s `tuple<A, B>` against `tuple<int, T...>`: `B` against
    the expansion `T...`); as clang 19, the guide is still formed with what
    was deduced, and `__is_deducible` decides at the call.
  - The alias parameters used are those the deductions name (clang
    `MarkUsedTemplateParameters`, `params_named`): each element of a
    deduced pack, an element that is an expansion by its pattern, probed
    by deducing it from itself. So `tuple<T..., U>` uses both `T` and `U`
    although its list is a non-deduced context.
  - A function parameter pack of `g` whose template parameter pack was
    deduced as a list with an expansion element (`const U&...` with
    `U = <int, T...>`) expands to `const int&, const T&...`: the
    expansion element is substituted as its pattern and the result
    re-formed as an expansion (`expand_pattern_rewrap`).
  - The guide keeps `g`'s implicitness: one formed from a user-declared
    guide is not implicit, so it beats the alias guide of the matching
    constructor ([over.match.best.general]/2.11) rather than tying with
    it. Every alias guide's notes include `note_implicit_deduction_guide`,
    whose `requires __is_deducible(A, R)` names the guide's return type
    `R` (clang `NoteImplicitDeductionGuide`).
  - Tests: `over.match.class.deduct/alias-pack.cpp`,
    `alias-pack-non-trailing.cpp`; `std::tuple` through
    `template<class... T> using TT = std::tuple<int, T...>` deduces.
- **Resolution.** The usual candidate machinery with `deduce_call`, then
  `order_template_candidates` and `best_viable`.
  - List-initialization runs [over.match.list] phase 1 first (WP-134):
    the initializer-list guides (first parameter (reference to cv)
    `std::initializer_list<E>`, dependent or not; any others defaulted;
    `is_init_list_guide`) with the whole list as one argument. A
    viable or ambiguous phase 1 decides; otherwise phase 2 runs all
    guides on the elements. A single element whose type is a
    specialization of `X` or derived from one skips phase 1
    (`ctad_list_of_spec`).
  - At clang 21+ (`clang_compat >= 22`) each resolution is two-phase
    like every other deferring candidate set (spec/sema-overload.md
    §4.4, `overload/defer.rs`): template guides are recorded as
    `Deferred::Guide`, and a perfect match among the non-template
    guides is the result without deducing any template guide, which so
    instantiates nothing. Otherwise the template guides are deduced and
    every guide is resolved, as at 19. Tests
    `templates/ctad/over.match.class.deduct-perfect-guide{,-clang19}.cpp`,
    `temp.deduct.guide-ast-dump.cpp`.
  - In copy-initialization (`=` and `= {…}`), explicit guides and
    constructors are non-viable (`CandidateFailure::Explicit`).
  - In copy-list-initialization, an explicit winner is
    `sema_ctad_explicit_copy_list` plus `note_ctad_explicit_here`.
  - Tie-breakers in `overload/best.rs`
    ([over.match.best.general]/2.11-2.13): a user guide beats an
    implicit one; the copy deduction candidate beats the others; a
    non-template constructor's guide beats a constructor template's.
- **Diagnostics** use clang's wording: `sema_ctad_no_viable`,
  `sema_ctad_ambiguous`, and the explicit-copy-list error and note
  above. Each implicit-guide candidate note is followed by
  `implicit deduction guide declared as '<sig>'`
  (`implicit_guide_signature`; parameter names as clang's DeclPrinter
  places them, `Ts ...xs`, and an inherited default argument as
  `= <null expr>`). Parameter types print as the constructor wrote
  them (member typedefs, `typename X<T>::type`,
  `typename T::template rebind<int>::other`): a guide's function type
  is built from the sugared prototype (`TypeTable::proto_of`), and an
  alias template's guide substitutes the alias's arguments into that
  sugar; its `__is_deducible(A, R)` prints the guide's sugared return
  type `R` (WP-311; test
  `over.match.class.deduct/guide-signature-as-written.cpp`).
- **Guide printing** (WP-320), as clang:
  - The injected class type of an implicit guide (`-> vector<_Tp,
    _Alloc>`) uses the template parameters of the class template's
    first declaration (`first_injected_params_template`, at every
    compat level): a forward
    declaration's names, `type-parameter-0-N` for unnamed ones. A friend
    declaration inside a templated class does not count (clang links it
    only at instantiation); one in a non-template class does.
  - A type-constraint prints with its written arguments in the guide's
    parameter names (`template <Compat<T> R>`, `Var<T, int>`), not as
    `Compat<>`.
  - The simple `C() -> C<T>` guide and the copy guide sit at the class
    template's definition.
  - Tests `over.match.class.deduct/implicit-guide-constraint-args.cpp`,
    `implicit-guide-first-declaration-params.cpp`,
    `implicit-guide-locations-init-list.cpp`.
  - Gap: a qualified concept name in a type-constraint loses its
    qualifier (`template <NC R>` where clang prints `N::NC R`).
  - At `-fclang-compat=22` (WP-377, clang 22): the guide's class type
    is the injected specialization named by its scope (`-> N::S<T>`,
    `-> Outer<int>::Q<U>`; inline and anonymous namespaces elided;
    `guide_class_type`); a default argument the constructor's class
    template inherits from an earlier declaration is cloned as
    inherited, so the guide's printed head omits it (`class _Alloc`,
    only the parameter's own default prints); and a
    deduced-template-specialization placeholder prints the template
    name as written at that use, not its first spelling
    (`TypeContext::dtst_first_spelling`). Tests: the `-clang22.cpp`
    copies of `building-guides-note`,
    `implicit-guide-first-declaration-params`,
    `implicit-guide-locations-init-list`,
    `dcl.type.class.deduct/placeholder-spelling{,-first-use}` and
    `templates/ctad/over.match.class.deduct-{member-template-diag,member-template-qualified-diag,no-viable-diag}`.
- **Building the implicit guides** runs under a
  `FrameKind::BuildingDeductionGuides` frame (not an instantiation;
  clang `BuildingDeductionGuides`), so an error forming them (the
  enclosing arguments substituted into a member template's
  constructor) is followed by `while building implicit deduction guide
  first needed here` at the template name of the placeholder that first
  needed them. The parser records each placeholder's template-name
  location by its type-specifier location (`ctad_placeholder`,
  `TemplState::ctad_name_locs`); a variable's type-specifier location is
  kept per variable (`ctad_var_type_locs`), and an expression form
  (functional cast, new-expression) deduces at its type-specifier. A
  variable instantiated from a template notes the variable instead.
  Test `over.match.class.deduct/building-guides-note.cpp`.
- **Array new.** `new X[n]{…}` with a placeholder element type is
  `sema_ctad_compound_type` ("cannot form array of deduced class
  template specialization type") at the innermost bound, as clang.
- **AST dump.** A user-declared `CXXDeductionGuideDecl` is compared
  against clang (spec/ast-dump.md); implicit guides are not dumped.
- **Dependent initializers.** A type-dependent argument gives the DTS
  deduced as `QualType::DEPENDENT`. The Substituter maps it back to the
  undeduced placeholder, so deduction is redone for the instantiation.
  A non-dependent use in a template is deduced at definition.

- **Inherited-constructor guides (P2582, WP-359).** See
  spec/sema-templates.md §6.6.1; neither clang 19 nor clang 22 implements
  them, so their tests are `skip(clang)` and the standard's example is
  the contract.

**Deferred (no candidate, or diagnosed as unsupported).**
- Guides from constructor templates of a member class template of an
  instantiated class template (only its non-template constructors and
  user guides are used).
- Qualified CTAD through a template-id, `Outer<int>::In x(…)`: the
  parser classifies `In` as a dependent name, giving a variable of a
  dependent type with no diagnostic (name classification, not CTAD;
  `using O = Outer<int>; O::In x(…)` works).
- `sema_ctad_deleted` (a deleted constructor selected) and
  `sema_ctad_not_allowed` (a placeholder in a non-deducible position)
  are reserved ids that nothing emits yet.

### 4.4 Tags, enumerations, bit-fields and anonymous records
- **`act_on_tag(kind, name, TagUse, …)`** implements [dcl.type.elab]
  and [class.name]:
  - A reference finds or injects the tag. An unqualified reference
    whose lookup finds tags of two namespaces (an inline namespace's
    and its parent's, [namespace.def.general]/7) is "reference to 'X'
    is ambiguous" with candidate notes, recovering with the last tag;
    the parser reports a class template-id `X<int>` the same way.
  - `struct N::X;` (or an opaque `enum N::E : int;`) outside an
    explicit specialization or instantiation is "forward declaration
    of struct cannot have a nested name specifier" at the qualifier
    ([dcl.type.elab]/1, [dcl.enum]/1), reported before the lookup.
  - A declaration redeclares the tag in the current scope. A class
    redeclaration `struct N; struct N;` ([class.name]/2) is a new
    CXXRecordDecl in N's chain (clang `ActOnTag`; the dump's `prev`), as
    a class template redeclaration is, and a later definition joins
    that chain. An enumeration redeclaration creates no decl. Test:
    class.name/redeclaration-ast.cpp.
  - A definition checks for redefinition and kind mismatch. A
    redefinition ([basic.def.odr]/1) is diagnosed once and recovered
    from as clang's `ActOnTag` does (it makes the new tag anonymous and
    invalid): the new decl is created `INVALID` and taken out of every
    lookup table (`AstBuilder::unregister_name`, also its class
    template), so later uses find the first definition rather than an
    ambiguity — at namespace scope, in a reopened, unnamed or inline
    namespace, in a class and in a block alike. The parser skips its
    body (`Sema::tag_body_skipped`; no member is checked, no enumerator
    declared) and its declarators get an error type. Gap: clang still
    parses a base-clause there (`expected class name`). Test:
    basic.def.odr/class-redefinition-recovery.cpp.
  - A type name marks referenced only the declaration lookup found (the
    latest in its chain; clang `isThisDeclarationReferenced`). An
    elaborated-type-specifier that finds a tag (`struct N *a`) marks
    nothing (clang `ActOnTag`).
  - A tag defined in a parameter type ([dcl.fct]/17: a function or
    lambda parameter, a requires-expression parameter) is "'S' cannot be
    defined in a parameter type" at its name, or "(null) cannot be
    defined…" at the keyword of an unnamed one (clang streams the empty
    name), and the declaration specifiers are invalid, so the parameter,
    and with it the function, is invalid: no follow-on "out-of-line
    definition does not match" for a member or a constructor. The parser
    reports it after the tag's definition (`diag_type_defined_in`; the
    other forbidden contexts are §4.4.1 "Forbidden type definitions").
    The tag itself is declared in the current context, except that a
    complete class's member list is frozen (spec/ast.md §2.4): in the
    parameter clause of an out-of-line member definition it goes to the
    nearest enclosing context that is not a complete class (`tag_home`). Tests:
    `tests/conformance/dcl.fct/type-defined-in-{parameter,parameter-kinds,member-parameter-crash}.cpp`.
- **Enumerations** ([dcl.enum]):
  - enumerators are typed as clang types them
  - a fixed underlying type gets converted-constant-expression checks
    and narrowing diagnostics; a non-constant initializer is
    `enumerator value is not a constant expression` (clang
    `CheckEnumConstant`, WP-230)
  - an unfixed one's initializer is an integral constant expression that
    may fold (`fold_not_ice`, below)
  - an unfixed one gets its type from the enumerators' range (the best
    type), and the enumerators are then retyped to the enumeration; a
    `packed` one (clang `computeBestEnumTypes` with `IsPacked`, WP-432)
    takes the smallest of `signed char` / `short` (with negative values)
    or `unsigned char` / `unsigned short` that holds the range, else
    the usual best type, and still promotes to `int`. A member
    enumeration of a class template gets its attributes at
    instantiation. Tests `dcl.enum/packed-underlying-type.cpp`,
    `tests/codegen/enum_packed.cpp`
  - `replace_enumerator_init` records the converted initializer
  - **enumerator vs. overload set** (WP-929, clang `ActOnEnumConstant`
    `LookupSingleName` + `getAsSingle`): an enumerator is a redefinition
    only when the current scope's lookup of its name finds exactly one
    non-template entity (redeclarations of one function count once;
    `lookup_current_scope_outside_enum`). After two or more functions,
    a function template, or functions plus an earlier enumerator of
    another enumeration, the enumerator is declared with no error; a
    later use that finds both is `reference to 'f' is ambiguous`
    (§3), a qualified `S::m`, a member access `s.m` and a call
    included. A scoped enumeration's enumerators never clash. Printed
    names skip an unscoped enumeration (`N::f`, not `N::E::f`; clang
    `printNestedNameSpecifier`): at compat 19 only an enumerator's own
    enumeration (a type defined in an enumerator's initializer prints
    `E::S`), at 22 every unscoped one (`S`). Tests:
    `basic.scope.scope/enumerator-after-overload-set{,-ambiguous}.cpp`.
- **Bit-fields** ([class.bit]; `bit_width`, clang `VerifyBitField`):
  - a named bit-field's type must be integral or an enumeration, checked
    even when the width is value-dependent (`bit-field 'f' has
    non-integral type 'float'`, underlining the width)
  - the width must be an integral constant expression; one that folds
    laxly is accepted (`fold_not_ice`, below). A width whose type is not
    integral or an unscoped enumeration (`2.0`, `SE::a`, `nullptr`) is
    `integral constant expression must have integral or unscoped
    enumeration type, not 'T'` at the width. A width whose
    full-expression reported an error (a member named without an
    object) is dropped without a further error
  - it must not be negative
  - it must not be zero-width with a name (underlining the width)
  - it must not exceed the type's width (`-Wbitfield-width`; never for
    `bool`)
  - a member declarator with a width that declares no field
    (`non_field_bit_width`, [class.bit]/3) is `static member 's' cannot
    be a bit-field`, `typedef member 'td' cannot be a bit-field`, or the
    non-integral error for a function; the member becomes invalid
  - `mark_bitfield` records the width
  - Tests `class.bit/width-and-member-kinds.cpp`; the underlines are
    checked by `kiln_driver` test
    `bit_width_and_static_assert_ranges_like_clang` (WP-320).
- **Anonymous unions and structs** ([class.union.anon]):
  - `act_on_decl_spec_only` creates the unnamed member, or the implicit
    variable outside a class, and returns it.
  - `IndirectField`s inject the members into the enclosing scope.
  - Conflicts are diagnosed (`member of anonymous union redeclares 'x'`).
  - Anonymous unions outside a class must be `static` at namespace
    scope.
  - An anonymous union or struct with no member (no `Field`; an unnamed
    bit-field counts as one) declares no name ([dcl.pre]/5,
    [class.mem.general]/3). That is "declaration does not declare
    anything" [-Wmissing-declarations] at the decl-specifier-seq, after
    the scope errors above (clang `BuildAnonymousStructOrUnion`). Its
    range covers the specifiers written before the class-key (`static
    const`), read from the source (`specifiers_before_type`). A
    template's pattern is diagnosed when it is parsed, and an
    instantiation is not diagnosed again. Test:
    class.union.anon/empty-anonymous-record-declares-nothing.cpp.
  - An anonymous struct outside a class is an error.
  - `act_on_finish_record_definition(r, then_semi)` sets `ANONYMOUS`
    and `HAS_VARIANT_MEMBERS`. The parser passes `then_semi` false for
    a class defined in a type-id: `using I = struct {...};` names an
    ordinary unnamed class, so a member of type `I` keeps its default
    member initializer (WP-782). Test:
    class.union.anon/alias-of-unnamed-class-is-not-anonymous-run.cpp.
- **Unnamed tags named by a typedef** record their typedef with
  `set_typedef_for_anon` ([dcl.typedef]/9).
- **Elaborated-type-specifier checks** (WP-1017, [dcl.type.elab]):
  - *Wrong class-key or `enum`*. `use of 'X' with tag type that does not
    match previous declaration` is reported at the class-key, with
    `previous use is here` at the previous declaration
    (`Sema::diag_wrong_tag`). This covers references, friends, friend
    class templates, specializations and references to specializations
    (named `'B<float>'`, as clang 19 does), and the instantiation of a
    dependent `struct T::X`. A reference recovers to the declaration it
    found. Test: `dcl.type.elab/wrong-tag-kind.cpp`.
  - *`enum class E` as a reference or friend* (no enum-base). The error
    `reference to enumeration must use 'enum' not 'enum class'`
    (`-Welaborated-enum-class`, an error by default) is reported at the
    `class`/`struct` keyword. Recovery is a plain `enum E` reference.
    Test: `dcl.type.elab/elaborated-enum-class.cpp`.
  - *`friend enum E;`* gets `elaborated enum specifier cannot be declared
    as a friend` (`-Wfriend-enum`) + `remove 'enum' to befriend an enum`,
    at `enum`. It also fires for a dependent `friend enum T::E;`. Test:
    `class.friend/friend-enum.cpp`.
  - *Ambiguous qualified lookup* (`struct N::x` with `x` found through
    two using-directives) gets `reference to 'x' is ambiguous`. The
    candidates are listed latest nominated first, as clang does. A
    qualified reference that finds no tag declares a hidden, invalid
    recovery tag, so later uses get no cascade. Test:
    `dcl.type.elab/ambiguous-qualified-elab.cpp`.
  - *Tag hidden by a type name* (WP-957, [basic.lookup.elab]/1,
    [dcl.type.elab]/2). The elaborated lookup ignores only non-type names,
    so an unqualified `struct A` reference whose tag lookup found an outer
    class while a nearer scope declares `A` as a typedef, type alias,
    class template or alias template gets `typedef 'A' cannot be
    referenced with the 'struct' specifier` (`type alias`, `template`,
    `alias template`) + `declared here`
    (`diagnose_tag_hidden_by_type_name`). A typedef naming a class `A` of
    its own scope (`typedef struct A A;`) is looked through, as clang's
    `ActOnTag` does. A `friend class-key A;` whose ordinary lookup finds
    such a type name in the innermost enclosing namespace gets the same
    error (`diagnose_friend_tag_non_tag`); one outside it declares a new
    class there. Tests: `dcl.type.elab/tag-hidden-by-type-name{,-ok}.cpp`.
  - *Forward reference to an enumeration* (WP-1078, [dcl.enum]/2,
    [dcl.type.elab]/3; clang `ext_forward_ref_enum` as an error, C++
    mode without `-fms-compatibility`, which kiln does not have). An
    unscoped enum without an enum-base that is not a definition, `enum
    F;` or a first-use `enum K *`, gets `ISO C++ forbids forward
    references to 'enum' types` at the name (`check_forward_ref_enum`),
    unless it names an enumeration already defined or already declared
    with a fixed underlying type (scoped ones are fixed). The tag is
    still declared, so later uses do not cascade. It covers members,
    parameters, typedefs, `friend enum E;` (after `-Wfriend-enum`), a
    local declaration hiding an outer enumeration and every repeat of
    `enum Z;`; a class template reports it once, at the template.
    Not done: clang's pedantic `-Wgnu-redeclared-enum` for `enum E;`
    after a definition. Tests: `dcl.enum/forward-ref-enum{,-ok}.cpp`.
  - *Unknown specialization*. `struct A<T>::K` naming a member of another
    class template's unknown specialization ([temp.dep.type]/6) is
    dependent and is not looked up. It used to be rejected when the
    primary has no `K`. Test:
    `temp.dep.type/elaborated-unknown-specialization.cpp`.

### 4.4.1 Declaration rules (WP-1015)
Rules clang enforces at declaration time that kiln accepted (found by the
clang negative-test run, docs/status/accepts-invalid.md). Each follows
clang 19's wording, location and recovery. None is re-run on
instantiation unless stated.
- **Members named as their class** ([class.mem.general]/22,
  `class/member_name.rs`): a static data member, member type, member
  template, unscoped enumerator or using-declaration named as the class is
  "member 'X' has the same name as its class" at the name
  (`diagnose_class_name_shadow`). A non-static data member is reported only
  when the class has a user-declared constructor, once, as the definition
  completes (`check_field_named_as_class`). Tests:
  class.mem/member-named-as-class{,-ok}.cpp.
- **Allocation function declarations** ([basic.stc.dynamic.allocation]/1,
  [basic.stc.dynamic.deallocation]/1-2, `alloc_decl.rs`): not in a
  namespace other than the global one and not `static` at global scope; the
  return type is `void *` (`void`) and not dependent; a template has at
  least two parameters; the first parameter is `std::size_t` (`void *`)
  and not dependent; an allocation function's first parameter has no
  default argument. The first violation in clang's order is reported at
  the name. A destroying operator delete (WP-1040,
  [basic.stc.dynamic.deallocation]/5) is a class member whose first
  parameter is `C *` (cv-unqualified pointee; in a class template the
  injected-class-name's type, other dependent types checked at
  instantiation), followed only by `std::destroying_delete_t`, an
  optional `std::size_t` and an optional `std::align_val_t`, and is not a
  template. A delete-expression that finds one considers only destroying
  candidates, rejects a deleted selection, accepts a cv-qualified operand
  and (CWG2889, clang 20+) does not need a non-virtual destructor to be
  accessible. Type-aware forms (P2719, WP-1088; §5.9) are checked by
  `check_type_aware_signature`. Undeduced return types are not checked.
  Tests: basic.stc.dynamic/, expr.delete/destroying-delete-*.cpp.
- **The main function** ([basic.start.main], `main_decl.rs`): the
  parameter forms `()`, `(int, char**)`, `(int, char**, char**)` (with
  clang's `const` extension) and no more than three; not a template; not
  deleted; not overloaded (another signature redeclares it, so the merge
  reports conflicting types); a variable named `main` at global scope, or
  a block-scope `extern` one in a global function, is ill-formed, and so
  (WP-1040) is a C-linkage variable named `main` in any namespace (clang 22
  at its decl-specifiers; clang 19 only the global one); a variable
  template named `main` stays valid, as in clang. A C-linkage function named `main` in a namespace
  is `main` for clang 22. clang's `-Wmain` warnings: `extern "C"` on
  `main` (clang 22, P3618R0), a single parameter, a variadic `main`, a
  `bool` literal returned from `main`, and (under `-pedantic`) a use of
  `main` in an expression. All off under `-ffreestanding`. Tests:
  basic.start.main/.
- **Union, bit-field and static data member rules** (`class/member_rules.rs`):
  - [class.static.data]/4: an initializer on the namespace-scope
    definition of a member initialized in-class is reported at the new
    initializer with a note at the first and dropped;
  - [class.union.general]/6 (CWG1460): a second variant member with a
    default member initializer (anonymous members searched), with a note
    at the first;
  - a union has no virtual functions; an anonymous bit-field takes no
    qualifiers.
  Tests: class.static.data/, class.union/, class.bit/.
- **Ref-qualifier overloads** ([over.load]/2.3, CWG2496,
  `overload/redecl.rs`): before C++23 a member function with the
  parameter-type-list of an earlier one whose ref-qualifier differs, one
  of them having none, is an error at the name with a note; both stay
  declared. From C++23 the two correspond when their cv-qualifiers agree,
  which makes the later one a member redeclaration. The virtual-override
  form reports once per base function and neither overrides; clang 22
  checks it in every language mode, clang 19 (`-fclang-compat=19`) only
  before C++23. Conversion functions and copy assignment operators are
  checked too (WP-1041; a conversion to a dependent type at
  instantiation; a conversion function template is not checked, as
  clang). CWG2553: a function with an explicit object parameter cannot
  override (at `this`, with a note at the overridden function; the
  declaration is invalid), and an allocation or deallocation function
  cannot take one. Tests: over.load/ref-qualifier-*.cpp,
  class.virtual/override-ref-qualifier-*.cpp,
  class.conv.fct/ref-qualifier-*.cpp, class.virtual/explicit-object-override*.cpp.
- **Member redeclarations**: a member class or enumeration redeclared
  without a definition is `-Wredeclared-class-member` (an extension
  warning) with a note; a qualified member tag is "extra qualification"
  (qualifier naming the class) or an error otherwise; a dependent
  using-declarator repeated with the same canonical qualifier and kind is
  a redeclaration ([namespace.udecl]/10, `using.rs`). Tests:
  class.mem.general/member-redeclared-tag*.cpp, dcl.meaning/qualified-member-tag*.cpp,
  namespace.udecl/dependent-using-*.cpp.
- **Default arguments** ([dcl.fct.default], `decl.rs`):
  - /3: a default argument on a parameter of a function declarator that
    is not the declaration's own clause (a function pointer, reference,
    typedef or alias, a type-id, a nested parameter, a return type) is
    "default arguments can only be specified for parameters in a function
    declaration" at the parameter name, or where it would be; the
    argument is dropped. The parser calls `act_on_nonfunc_default_arg`;
    a lambda's own clause, a requires-expression parameter and an
    exception-declaration (as clang) are not checked.
  - /8: `this` does not appear, even unevaluated; a lambda-expression
    captures nothing (the first written or implicit capture is reported
    at its location; init-capture initializers are searched, lambda
    bodies are not) (`check_default_arg_refs`). A braced default
    argument and its nested lists are searched too (WP-1041).
  - /9: a local variable (any block-scope variable, `static`, `extern`
    and `thread_local` included, or a structured binding of one) is not
    odr-used. The check runs on the converted full-expression, so a
    constant read by value is accepted while `&c` or binding a reference
    is not; in a template definition the constant non-odr-uses are
    computed from the potential results (`check_default_arg_locals`).
    Every use is reported and the parameter becomes invalid.
- **Block-scope inline** ([dcl.inline]/5): `inline` on a block-scope
  function declaration (also in a lambda body or a template) is "inline
  declaration of 'g' not allowed in block scope" at `inline`; members of
  local classes are unaffected.
- **Lambda parameter shadowing a capture** ([expr.prim.lambda.capture]/5,
  `lambda/capture.rs`): a parameter named as an explicit capture (simple,
  init or pack capture) is "a lambda parameter cannot shadow an explicitly
  captured entity" at the parameter with the note "variable 'x' is
  explicitly captured here"; implicit captures do not count. clang's
  follow-on "reference to 'w' is ambiguous" for an init-capture is not
  reproduced.
- **Friend template defaults** ([temp.param]/12, `class/friend.rs`): a
  default template argument on a friend template declaration that is not
  a function template definition is "default template argument not
  permitted on a friend template" at each defaulted parameter (its name,
  or the `=` when unnamed); the default no longer counts for the
  redeclaration rules. A friend class template in a dependent context is
  not checked (clang's `CheckClassTemplate`).
- **Template declaration scope** ([temp.pre]/2, [temp.mem]/2; WP-1063,
  WP-1041): `template<...> enum E` is "enumeration cannot be a template"
  after the name and enum-base, with "declaration does not declare
  anything" at `enum`; a template-declaration in a block is clang 22's
  "templates can only be declared in namespace or class scope" at
  `template` and is dropped (clang 19: "expected expression"); when the
  declaration is a function template definition (its declarator's
  parameter list is followed by `{`) clang 22 reports "function
  definition is not allowed here" at the `{` instead and skips the body
  (spec/parse.md §2.4, WP-1091); a friend
  class template declared in a local class is "templates cannot be
  declared inside of a local class". Tests: temp.pre/, temp.mem/p2-*.cpp.
- **inline after definition** ([dcl.inline]/6, `Sema::inline_after_definition`,
  WP-1042): a redeclaration declared `inline` after a non-inline function
  definition (not implicitly inline, deleted or defaulted) is "inline
  declaration of 'f' follows non-inline definition" at the name with a
  note at the definition; also for friends, function templates, qualified
  and block-scope redeclarations. Tests: dcl.fct.spec/inline-after-definition*.cpp.
- **Template parameter shadowing** ([temp.local]/6, `check_template_param_shadow`,
  WP-1042): besides the declarations checked before, an alias-declaration
  or alias template, a nested template head's parameter, a function
  parameter, a handler's exception-declaration, a namespace alias and a
  friend class template or class named like an enclosing template
  parameter is "declaration of 'T' shadows template parameter". A
  `friend struct T` does not find a non-type template parameter (tag
  lookup). Not re-checked at instantiation. An unqualified elaborated
  reference `struct T *p` (in a member, a function body or an operand)
  whose ordinary lookup finds a template type or template template
  parameter is the same error at the name, with the note (clang
  `ActOnTag`); the reference then names the tag `T` already in the
  enclosing namespace, else declares one there, so a second reference
  is not diagnosed again and does not make `T` ambiguous
  (`diagnose_tag_ref_template_param`, WP-1091). A lambda capture
  (simple, by-reference, init- or pack capture) named like a template
  parameter of the lambda's own template-parameter-list
  ([expr.prim.lambda.capture]/5) is the same error at the capture with
  the note at the parameter (`check_capture_shadows_template_param`,
  once the explicit or invented head is complete; an enclosing
  template's parameters do not count; WP-1091). Tests:
  temp.local/p6-shadow-alias-params*.cpp,
  temp.local/p6-elaborated-ref-shadows-template-param*.cpp,
  expr.prim.lambda.capture/capture-shadows-template-param*.cpp.
- **C++03 union members** ([class.union]/1 in C++03, `cxx03_nontrivial_member`,
  WP-1042): before C++11 a union member, or a member of an anonymous
  struct, of (an array of) a class type with a non-trivial copy
  constructor, default constructor (or none), copy assignment operator
  or destructor, checked in that order, is "union member 'm' has a
  non-trivial ..." ("anonymous struct member ...") at the name; a union's
  field becomes invalid. Checked again at instantiation. clang's
  explanatory "because ..." notes are not produced. Tests:
  class.union/cxx03-nontrivial-member*.cpp.
- **Using-declaration conflicts** ([namespace.udecl]/11 per CWG101,
  `using_conflicts`, WP-1042): a function and a using-declared function
  conflict when their parameter-type-lists correspond, whatever their
  return types, in either order. As clang and gcc, a block scope's
  declarations are not compared again at its instantiation. Tests:
  namespace.udecl/p11-return-type-conflict*.cpp.
- **Forbidden type definitions** ([dcl.type.general]/3, [dcl.fct]/17,
  [stmt.pre]/2, [temp.alias]/2, [dcl.enum]/2; WP-946, clang's `ActOnTag`
  and `GetDeclSpecTypeForDeclarator`). A class or enumeration may be
  defined only in the decl-specifier-seq of a simple-declaration or
  member-declaration (any declarators, `typedef` included), a
  for-init-statement, or a non-template alias-declaration; a lambda body
  is a fresh context anywhere. The parser classifies the site
  (`DeclSpec::tag_site`, `TagSite`) and `diag_type_defined_in` reports, at
  the tag's name (the keyword when unnamed), one of:
  - "cannot be defined in a type specifier": a type-id (`sizeof`,
    `alignof`, `typeid`, casts, `__builtin_offsetof`, `decltype`, `new
    (T)`, template arguments, a type-parameter default), a non-type
    template parameter type, an exception-declaration, a dynamic
    exception specification (its rejected types are dropped, so `throw(S
    {})` becomes `throw()` with no C++17 error) and a conversion-type-id;
  - "in an enumeration": an enumerator's initializer;
  - "in a parameter type": a tag declared in a function prototype scope
    (a parameter, a default argument, a noexcept operand). clang streams
    the identifier, so an unnamed enumeration prints `(null)`; an unnamed
    class is not a prototype-scope declaration and is a type specifier;
  - "in a condition": a condition declaration, including a for condition
    (a range declaration is "types may not be defined in a for range
    declaration");
  - "in a type alias template";
  - "in the result type of a function": once per function declarator
    chunk, so `struct S {} f(), g();` reports twice at the same location
    (`allow_duplicate`); a nested function type reports this after the
    site's own error.
  A rejected enumeration's body is skipped and a rejected class's is
  parsed; either way the declaration specifiers become an error type and
  no follow-on mismatch is reported. A tag nested in another declaration
  (a parameter, a type-id, a condition) takes none of the open template
  heads (`hide_open_heads`), so its members are ordinary members and a
  nested template-id definition is "template specialization requires
  'template<>'". A `new struct S {}` allocates the elaborated type (the
  braces are the initializer). The name is the tag's type printed fully
  scoped (`'C::S'`); a scope stops at the first enclosing function, as in
  clang's `TypePrinter::AppendScope`, so local types print unqualified. A
  tag inside an enumeration is scoped by it at 19 (`'E::S'`); clang 22
  drops an unscoped enumeration's scope (`'S'`) and keeps a scoped one.
  Not reproduced: clang's `type-id cannot have a name` and its "expected
  '>'" recovery for `W<struct S {}>`, the class scope clang gives an
  unnamed tag in an out-of-line static member initializer
  (`'A::(unnamed struct …)'`), and clang 22's `struct (unnamed at …)`
  printing. Tests: dcl.type/type-defined-in-type-specifier.cpp,
  dcl.type/type-definition-allowed-contexts-run.cpp,
  dcl.fct/type-defined-in-{default-argument,result-type}.cpp,
  stmt.pre/type-defined-in-condition.cpp,
  temp.alias/type-defined-in-alias-template.cpp,
  except.handle/type-defined-in-handler.cpp,
  class.conv.fct/type-defined-in-conversion-type.cpp,
  expr.new/type-defined-in-new-type-id.cpp,
  dcl.enum/type-defined-in-enumeration{,-19}.cpp,
  temp.param/lambda-local-class-in-parameter-type.cpp.
- **Template-declarations define no declarators** ([temp.pre]/5): when
  any template head is open (a class template, a partial or explicit
  specialization, an out-of-line member class, an explicit instantiation
  recovered as a specialization; `in_template_declaration`), a class
  definition must be followed by `;`: clang's "expected ';' after struct"
  at the end of the body, after which the rest parses as a new
  declaration. A concept definition consumes its head before its
  constraint expression (`act_on_concept_head`), so a tag there is a
  type specifier. Test: temp.pre/class-definition-takes-no-declarator.cpp.
- **Incomplete result type** ([dcl.fct.def.general]/2): a function
  definition whose return type is an incomplete class is "incomplete
  result type 'X' in function definition" at the name with the forward
  declaration note, checked before the parameters, and the function is
  invalid. A dependent, `void`, reference, undeduced or error return type,
  and a deleted or already invalid function, are skipped. Test:
  dcl.fct.def.general/incomplete-result-type.cpp.

### 4.5 Attributes
- **Policy: kiln never accepts a semantics-changing construct without
  implementing it** (WP-805). An attribute clang knows that kiln does not
  implement is triaged by `attr_triage::TABLE`: class A (semantics, ABI,
  layout) is the error `'X' attribute is not supported by kiln yet`,
  which system headers and `-w` do not suppress; class B (optimization,
  instrumentation, debug, hardening) and class C (diagnostics or
  analyses only) are accepted silently. A name clang does not know keeps
  the `unknown attribute` warning. spec/landed/sema/WP-805.md.
- **Truthful feature tests** (WP-805): in compile modes
  `__has_attribute` / `__has_cpp_attribute` answer 0 for a class-A
  name, and `__has_builtin` answers 0 for a builtin codegen cannot lower
  (`kiln_codegen::UNLOWERED_BUILTINS`); a header that tests for a feature
  takes the path kiln implements. `-E` keeps clang's answers. A reported
  builtin is declared on the target (WP-1057): `__builtin_readcyclecounter`
  (`ULLi`, every target; not constant, lowered to `llvm.readcyclecounter`)
  is declared, and the `__builtin_` keywords the parser lacks
  (`kiln_driver::pp_cmd::UNPARSED_KEYWORD_BUILTINS`:
  `__builtin_available`, `__builtin_omp_required_simd_align`,
  `__builtin_ptrauth_type_discriminator`, `__builtin_vectorelements`)
  answer 0. `kiln_driver/tests/builtin_lowering.rs` calls every reported
  name in the reserved `__` namespace on aarch64 and x86_64 at compat 19
  and 22 and requires none to be undeclared (header-declared library
  builtins and non-`__builtin_` keywords excepted).
- `act_on_attribute(syntax, scope, name, args, loc)` maps a parsed
  attribute to an `AttrKind`, or diagnoses it as above and returns
  `None`.
- **Recognised kinds:**
  - `nodiscard`/`warn_unused_result`
  - `maybe_unused`/`unused`
  - `deprecated`, `noreturn`, `fallthrough`
  - `likely`/`unlikely`
  - `no_unique_address`
  - `alignas`/`aligned`, `packed`. `alignas(type-id)` checks its operand as
    `alignof` does: `invalid application of 'alignas' to an incomplete type
    'T'` ([dcl.align]/3; WP-1017, `dcl.align/alignas-void.cpp`). A
    dependent operand is checked when the declaration is instantiated
    (`copy_attrs`; WP-957, `dcl.align/alignas-dependent-incomplete.cpp`).
  - `used`, `noinline`, `always_inline`, `weak`, `const`, `pure`, `cold`,
    `hot` (`AttrKind::HOT`, WP-172)
  - `optnone` / `minsize` (GNU syntax or `clang::`; `gnu::` and
    unscoped standard spellings are unknown) and `naked` (GNU syntax or
    `gnu::`), WP-405, as clang 19/22. Each takes no arguments (`'X'
    attribute takes no arguments`). On a non-function, `optnone` and
    `naked` warn (`only applies to functions and Objective-C methods` /
    `only applies to functions`) and `minsize` is an error with
    `optnone`'s text. `check_optnone_conflicts` (after
    `check_decl_attrs`): with `optnone` on this declaration or a prior
    one, `always_inline` / `minsize` on this declaration, or (when this
    declaration brings the first `optnone`) on prior ones, get `'X'
    attribute ignored` [-Wignored-attributes] with `conflicting attribute
    is here` at the `optnone`, in source order, and are dropped from this
    declaration. `check_naked_body` (at the end of a definition, clang
    `ActOnFinishFunctionBody`): the first body statement that is not an
    asm statement, a null statement or a declaration of local register
    variables without initializers is `non-ASM statement in naked
    function is not supported` with `attribute is here` at `naked`, and
    the function becomes invalid. An asm operand naming a parameter or
    `this` (clang `CheckNakedParmReference`, last child first) is
    `parameter references not allowed in naked functions` / `'this'
    pointer references not allowed in naked functions` with the same
    note, and the asm statement is dropped. A naked function gets no
    fall-off (-Wreturn-type) or noreturn-returns warning. Test:
    `tests/conformance/dcl.attr/gnu-optnone-minsize-naked.cpp`.
  - `disable_tail_calls` (`AttrKind::DISABLE_TAIL_CALLS`; GNU syntax or
    `clang::`, as `minsize`), WP-682: takes no arguments; on a
    non-function it warns `only applies to functions and Objective-C
    methods` and is dropped. `check_naked_conflicts` (after
    `check_optnone_conflicts`; clang `MutualExclusions<[Naked,
    DisableTailCalls]>`): `naked` or `disable_tail_calls` on this
    declaration, when the other is on a prior declaration or earlier on
    this one, is the error `'X' and 'Y' attributes are not compatible`
    (X the later) with `conflicting attribute is here` at the earlier, and
    is dropped. Test: `tests/conformance/dcl.attr/gnu-disable-tail-calls.cpp`.
  - `no_builtin` (`AttrKind::NO_BUILTIN`; GNU syntax or `clang::`),
    WP-808, clang `handleNoBuiltinAttr`: `no_builtin_attribute` reads
    its arguments, each a string literal (else `expected string literal
    as argument of 'no_builtin' attribute` at the argument, and the
    attribute is dropped); none is the wildcard `*`. A name is valid
    when it is a library builtin (`Sema::lib_builtin_name`, the
    parser's `kiln_pp::is_library_builtin`: a `NoBuiltin`-overlay row of
    the feature tables at the `-fclang-compat` level) other than the
    seven standard ones (`move`, `forward`, `addressof`, ...), or
    `std-` followed by one of those; an invalid one (`*`, `foo`,
    `__builtin_memcpy`, `move`) warns `'N' is not a valid builtin name
    for 'no_builtin'` [-Winvalid-no-builtin-names] at its literal and is
    left out. `a` is the interned, sorted, deduplicated `,`-joined
    names. On a non-function it warns `'no_builtin' attribute only
    applies to functions` [-Wignored-attributes].
    `check_no_builtin_attrs` merges the attributes of one declaration
    (in order) into one, and each merge that leaves the wildcard among
    other names is `empty 'no_builtin' cannot be composed with named
    ones` at the declaration's name. The declaration then waits in
    `Sema::no_builtin_pending`: its body (`act_on_start_function_def`)
    clears it, `= default` / `= delete` is `no_builtin attribute has no
    effect on defaulted or deleted functions` at the attribute, and at
    the end of the unit (`report_no_builtin_declarations`, skipping
    instantiations and definitions) it is `no_builtin attribute is
    permitted on definitions only` (clang reports both at the
    declarator; the harness compares unordered). Codegen reads the
    definition's attribute (spec/codegen.md "no-builtin"). Test:
    `tests/conformance/dcl.attr.no-builtin/errors.cpp`.
  - `gnu_inline` (GNU syntax or `gnu::`; **GNU inline functions**,
    WP-585, clang `handleGNUInlineAttr`; `AttrKind::GNU_INLINE`): on a
    non-function `'gnu_inline' attribute only applies to functions`
    [-Wignored-attributes]. On a function whose declaration does not
    spell `inline` (implicit inline, `constexpr` or in-class, does not
    count) it is ignored with `'gnu_inline' attribute requires function
    to be marked 'inline', attribute ignored` [-Wignored-attributes];
    otherwise it is kept, and without `extern` it also warns
    `'gnu_inline' attribute without 'extern' in C++ treated as
    externally available, this changed in Clang 10`
    [-Wgnu-inline-cpp-without-extern]. The kept attribute makes the
    definition not externally visible (codegen rule in spec/codegen.md
    "GNU inline functions"). Test:
    `tests/conformance/ext.gnu.inline/attribute-diags.cpp`.
  - `artificial` (GNU syntax or `gnu::`; WP-789, clang `ArtificialAttr`,
    `SubjectList<[InlineFunction]>`; `AttrKind::ARTIFICIAL`): kept on a
    function whose declaration spells `inline`; elsewhere (a
    non-function, or a function that is only implicitly inline) it warns
    `'artificial' attribute only applies to inline functions`
    [-Wignored-attributes] and is dropped. It concerns debug information
    only, so it has no effect on the IR. Test:
    `tests/conformance/dcl.attr/gnu-artificial.cpp`.
  - `alias("sym")`, `ifunc("resolver")`, `weakref` / `weakref("sym")`
    (GNU syntax or `gnu::`; WP-763, clang `handleAliasAttr`,
    `handleIFuncAttr`, `handleWeakRefAttr`; `AttrKind::ALIAS`,
    `AttrKind::IFUNC`, `AttrKind::WEAKREF`). The argument is one string
    literal (concatenation allowed), the target's *mangled* name, kept
    as `AttrNode.a` with `AttrNode.b` the location one past the
    attribute's `)` (for codegen's fix-its). `'X' attribute takes one
    argument` / `expected string literal as argument of 'X' attribute`
    otherwise. `ifunc` on anything but a function is `'ifunc' attribute
    only applies to functions` [-Wignored-attributes] and dropped.
    `alias` and `ifunc` make the declaration a definition (clang
    `hasDefiningAttr`): on a function definition, on a variable
    definition (one with an initializer, or a non-`extern` one), or with
    a second `alias`/`ifunc` on the same declaration, the later one is
    `definition 'x' cannot also be an {alias|ifunc}`; a definition of the
    entity before or after a declaration carrying one is `redefinition of
    'x'` with `previous definition is here` (`check_alias_redefines`,
    `check_alias_definition`). A later variable redeclaration counts the
    alias declaration as its previous definition (WP-777, clang
    `MergeVarDecl` / `getDefinition`): `int v;` after `extern int v
    __attribute__((alias("t")));` is `redefinition of 'v'`, and `extern
    int v = 2;` is too, without `'extern' variable has an initializer`
    (the variable is invalid and its initializer dropped, as clang's
    `AddInitializerToDecl` stops); an `extern "C"` redeclaration's
    language-linkage note reads `previous definition is here`. Test:
    `tests/conformance/ext.gnu.alias/alias-is-definition-redefinition.cpp`.
    `weakref` must have internal
    linkage (`weakref declaration must have internal linkage`) and
    namespace scope, and needs a target. The target checks (defined in
    the TU, no cycle, resolver returns a pointer) are codegen's
    (spec/codegen.md "Aliases and ifuncs"), as in clang. Not diagnosed:
    clang's -Wgcc-compat `GCC does not allow 'X' attribute in this
    position on a function definition`. Test:
    `tests/conformance/ext.gnu.alias/alias-diagnostics.cpp`,
    `tests/conformance/ext.gnu.weak/weakref-diagnostics.cpp`.
  - `may_alias` (GNU syntax or `gnu::`; WP-423, clang `MayAliasAttr`):
    kept on any declaration without a diagnostic (no subject list); with
    arguments `'may_alias' attribute takes no arguments`. A GNU
    declaration attribute after a ptr-operator (`int
    *__attribute__((may_alias)) p`) is the declarator's, and a
    `[[gnu::X]]` declaration attribute after a ptr-operator or an array
    declarator is `attribute 'X' ignored, because it cannot be applied
    to a type` (a warning, as after a function declarator). Codegen:
    spec/codegen.md "TBAA". Test: `tests/conformance/dcl.attr/gnu-may-alias.cpp`.
  - `using_if_exists` (`__attribute__((using_if_exists))` or
    `[[clang::using_if_exists]]`; other `clang::` attributes are still
    ignored), on using-declarations (§4.8)
  - `exclude_from_explicit_instantiation` (GNU syntax or `clang::`,
    `AttrKind::EXCLUDE_FROM_EXPLICIT_INSTANTIATION`, WP-214): an explicit
    instantiation definition of the enclosing class does not instantiate
    the member (`instantiate_members_of`), and codegen keeps it an
    implicit instantiation (spec/codegen.md §7h). It is not copied to
    instantiated members; `Ast::excluded_from_explicit_instantiation`
    follows the pattern chain.
  - `ext_vector_type(N)` / `vector_size(N)` (GNU syntax; `vector.rs`).
    They take exactly one expression and are consumed into the declared
    type, never attached. `ext_vector_type` applies only to a typedef or
    alias-declaration (`'ext_vector_type' attribute only applies to
    typedefs`); `vector_size` also to the type of a variable, field or
    parameter. Among the decl-specifiers they modify the decl-specifier
    type (so a function can return a vector); after a declarator, the
    whole declared type (on a function, pointer or array declarator:
    an invalid element type, after which the type is left unchanged,
    as clang). The element
    type must be integral or floating (`vector_size`: a non-`bool`
    builtin), else `invalid vector element type 'T'`; N must be an
    integral constant (`ext_vector_type attribute requires an integer
    constant`), nonzero (`zero vector size`) and, for `vector_size`, a
    multiple of the element size (`vector size not an integral multiple
    of component size`). A value-dependent N (or a dependent element of
    a `vector_size` vector) gives a `DependentVector` (spec/types.md);
    after substitution it is diagnosed at the size operand (clang: at
    the attribute name).
    In a type-id (`sizeof`/`alignof`, a cast, a template argument, a
    new-type-id; not an alias-declaration, whose attributes go to the
    alias) the decl-specifier attributes modify the decl-specifier type
    as type attributes (`Sema::apply_type_id_vector_attrs`, clang
    `processTypeAttrs`), so `ext_vector_type` is accepted at clang 19
    too (WP-187; `ext.vector/type-id-run.cpp`).
  - `mode(M)` (GNU syntax; `vector.rs`, WP-799; clang `handleModeAttr`,
    `parseModeAttrArg`) takes one identifier (`__M__` is `M`) and is
    consumed into the declared type like `vector_size`. Modes:
    `QI`/`HI`/`SI`/`DI`/`TI` are integer modes of 8/16/32/64/128 bits
    (`byte` 8; `word`, `pointer`, `unwind_word` the pointer width);
    `HF`/`SF`/`DF`/`XF`/`TF`/`KF`/`IF` floating modes of 16/32/64/96/128
    bits and `SC`/`DC`/`XC`/`TC`/`KC`/`IC` their complex forms. The
    floating type is clang's `getRealTypeByWidth`: `__fp16`, `float`,
    `double`; 96 bits `long double` only if it is x87 (x86-64); `TF`
    `long double` if that is IEEE quad (AArch64), else `__float128`;
    `KF` `__float128` only where the target has it (x86-64); `IF` never
    (`unsupported machine mode 'M'`). `V<N>M`, N a power of two, is a
    GNU vector of N elements of mode M, with the -Wdeprecated-attributes
    warning `specifying vector types with the 'mode' attribute is
    deprecated; use the 'vector_size' attribute instead` (any other
    `V...` is the whole name). Other names (`CQI`, `libgcc_cmp_return`,
    ..., as in clang 19) are `unknown machine mode 'M'`. An integral or
    enumeration type becomes the integer type of the mode's width with
    the same signedness, a floating type the mode's floating type, a
    complex floating type with a complex mode its complex type (with a
    real mode the real type); the result is unqualified (clang replaces
    the declaration's type). On a vector type a scalar mode replaces the
    element type, keeping the size and kind; a vector mode builds the
    vector of the new type. A non-arithmetic type (a complex integer
    type included) is `mode attribute only supported for integer and
    floating-point types`; an integer mode on a floating type, a
    floating mode on an integer type or a complex mode on a real one is
    `type of machine mode does not match type of base type`; as in
    clang the mode's type is still installed. A vector mode on an
    enumeration type is `mode 'M' is not supported for enumeration
    types`, a complex mode on a vector type also `type of machine mode
    does not support base vector types` (type unchanged). In a system
    header the same: `mode` changes size and layout, so it is never
    ignored (glibc's x86-64 `<bits/floatn.h>` declares `__cfloat128`
    with `__mode__ (__TC__)`). On an enumeration declaration an integer
    mode sets the underlying type, from the fixed type or else `int`
    (clang `AddModeAttr` on an `EnumDecl`), so `enum E {X}
    __attribute__((mode(QI)))` has underlying type `signed char`. On a
    dependent type the attribute stays on the declaration
    (`AttrNode::MODE_DEFERRED`) and applies to the substituted type of
    the instantiated variable, field, typedef or function parameter (a
    pack's elements included; the instantiated function type is built
    from those parameters), clang `instantiateDependentModeAttr`
    (WP-807, WP-809). `mode` is a declaration attribute: in a type-id
    that declares nothing it is dropped (`sizeof(char
    __attribute__((mode(DI))))` is 1), with clang `ActOnTypeName`'s
    -Wignored-attributes warning `'mode' attribute ignored when parsing
    type` in `sizeof`/`alignof`, a C-style cast, a template argument and
    a trailing return type, silently in a named cast, a new-type-id and
    an alias-declaration's type-id (the alias's own attributes, after
    its name, still apply); `ext.gnu.mode/mode-type-id-run.cpp`.
    Diagnostics (WP-187, `ext.vector/*-diagnostics.cpp`):
    - A type argument whose desugaring reaches a vector typedef stops at
      that typedef as written, with its scope: `'B' (aka 'X::v')`,
      `'A<long>' (aka 't1<long>::v')` (clang 22, at
      `-fclang-compat=22`, keeps a dependent name's keyword: `(aka
      'typename t1<long>::v')`)
      (`kiln_ast::types::vector_diag_arg`, used by `Sema::ty_arg` and
      codegen; clang's `Desugar` keeps the Elaborated name of a vector).
    - Comparing an integer vector with itself is
      `-Wtautological-compare` (`self-comparison always evaluates to a
      constant`), as for a scalar; a floating vector is not.
    - A scalar converted to a vector (a splat) is checked as a conversion
      to the element type, naming the vector: a lossy floating literal is
      `-Wliteral-conversion`, an integer constant that does not fit is
      `-Wconstant-conversion`. As in clang, a floating value is measured
      against the whole vector's width, so `1e20` into a vector of 4 `int`
      is silent.
    - A designated list for a vector warns `-Wc99-designator` at each
      array designator (and about mixing designated and non-designated
      elements) and is then `initialization of non-aggregate type T with
      a designated initializer list` (a vector is not an aggregate), at
      the variable or at a nested list's brace.
    - A list whose leading element is an unbraced vector copy-initializes
      the whole vector from it (clang `CheckVectorType`, non-OpenCL), so
      `v4{v2, v2}` with a 2-lane `v2` is `cannot initialize a value of
      type 'v4' ... with an lvalue of type 'v2'` in clang 19 and 22 alike;
      kiln does not split a shorter vector into lanes (the OpenCL rule).
      Test: `tests/conformance/ext.vector/list-init-shorter-vector-element-rejected.cpp`
      (WP-802).
  - GNU `malloc` (function with a pointer result, else `'malloc'
    attribute only applies to return values that are pointers`; on a
    non-function `... only applies to functions`), `alloc_size(n[, m])`
    and `alloc_align(n)` (`vector.rs::alloc_attribute`, checked in
    `decl.rs::check_alloc_attr` as clang's `handleAllocSizeAttr` /
    `AddAllocAlignAttr`): integer-constant one-based indices (`requires
    parameter N to be an integer constant`; arity: `takes at least 1
    argument` / `takes no more than 2 arguments` / `takes one argument`)
    naming an integer parameter, or for `alloc_align` one of type
    `std::align_val_t` (an enumeration of that name in namespace `std`,
    through sugar; clang `Type::isAlignValT`; libstdc++ 16 `<new>`
    `operator new(size_t, align_val_t)`; WP-525,
    `dcl.attr/gnu-alloc-align-val-t.cpp`) (`parameter N is out of bounds`, `argument
    may only refer to a function parameter of integer type` at the
    argument, a dependent parameter included; index 1 of an implicit
    object member function: `is invalid for the implicit this
    argument`); the result must be a pointer (`alloc_align`: or a
    reference), else a `-Wignored-attributes` warning; on a non-function
    `only applies to non-K&R-style functions`. Codegen: spec/codegen.md
    "Allocation functions".
  - `flag_enum` and `enum_extensibility(open|closed)` (GNU syntax or
    `clang::`, `AttrKind::FLAG_ENUM` / `ENUM_EXTENSIBILITY`, WP-230), on
    enumerations (else `'x' attribute only applies to enums`
    [-Wignored-attributes]). `flag_enum` takes no arguments;
    `enum_extensibility` takes exactly one, an identifier (the parser
    passes a leading identifier as `AttrArg::Ident`, clang
    `attributeHasIdentifierArg`): `takes one argument`, `requires
    parameter 0 to be an identifier`, and for another identifier
    `'enum_extensibility' attribute argument not supported: 'x'`
    [-Wignored-attributes]; an argument already in error drops it
    silently. They change only the switch checks (§7 Statements). Not
    yet: -Wflag-enum (an enumerator out of range of the flags).
  - `coro_only_destroy_when_complete` (GNU syntax or `clang::`,
    `AttrKind::CORO_ONLY_DESTROY_WHEN_COMPLETE`, WP-370), on classes and
    unions, class template patterns included (it is copied onto each
    specialization with the other class attributes); elsewhere `'x'
    attribute only applies to classes` [-Wignored-attributes]; takes no
    arguments. A coroutine whose return type's class carries it gets the
    LLVM function attribute of that name (spec/codegen.md §7o).
  - **`unavailable`, `require_constant_initialization`, `annotate`**
    (GNU syntax or `clang::`, `AttrKind` 276 to 278, WP-954;
    spec/landed/sema/WP-954.md):
    - `unavailable` / `unavailable("msg")` (`annotate_attr.rs`, clang
      `handleAttrWithMessage`): at most one argument (`'X' attribute
      takes no more than 1 argument`), a string literal (`expected
      string literal as argument of 'unavailable' attribute`, the
      unscoped name, at the argument). A use of the entity is `'f' is
      unavailable` (`: msg` appended) with the note `'f' has been
      explicitly marked unavailable here` at the redeclaration that
      carries it, through the `deprecated` machinery (`deprecated.rs`):
      the same uses (names, members, types, enumerators), inherited by
      later redeclarations, delayed with the deprecations of a
      declaration. A use inside an unavailable function or class is no
      error (clang `isDeclUnavailable` on the context).
    - `require_constant_initialization` (`const_init_attr.rs`, clang
      `ConstInitAttr`): only a variable with static or thread storage
      duration (`only applies to global variables`, an error, and the
      attribute is dropped). Its initializing declaration has a
      constant initializer ([basic.start.static]/2), checked as for
      `constinit` (`check_constinit` falls back to it, and its
      immediate invocations are checked with the whole initializer,
      `consteval.rs` `checked_var`, WP-465);
      else `variable does not have a constant initializer` at the
      variable, the note `required by 'require_constant_initialization'
      attribute here` at the attribute, then the evaluation's notes
      (a destruction failure is not one). Before C++11 the initializer
      is checked as clang's `Expr::isConstantInitializer`: aggregates,
      trivial default, copy and move construction and the no-op casts
      are walked, any other subexpression must fold, and the one that
      does not gets the single note `subexpression not valid in a
      constant expression`. Added on a redeclaration after the
      initializing declaration (or the definition) that lacks it: the
      warning `'require_constant_initialization' attribute added after
      initialization of variable` [-Wignored-attributes], the note `add
      the 'require_constant_initialization' attribute to the
      initializing declaration here`, and it is dropped. A constexpr
      variable is not checked again.
    - `annotate("str", args...)` (`annotate_attr.rs`, clang
      `handleAnnotateAttr` / `ConstantFoldAttrArgs`): at least one
      argument (`takes at least 1 argument`), the first a string
      literal (`expected string literal as argument of 'annotate'
      attribute`). The others are decayed (arrays kept) and each must
      be a constant expression, evaluated by the Oven (a `void` one as
      `e, true`): else `'X' attribute requires parameter N to be a
      constant expression` (N 1-based among them) with the
      evaluation's notes, and the attribute is dropped. If one is
      value-dependent all are checked when the declaration is
      instantiated (`copy_attrs`); an argument may be a pack expansion
      (`args...`, clang `AcceptsExprPack`), and a pack named outside one
      is `expression contains unexpanded parameter pack` ([temp.variadic]/12,
      the attribute is dropped). On a statement clang 22 accepts it
      and kiln ignores it; clang 19 (`-fclang-compat=19`) keeps `cannot
      be applied to a statement`. An instantiated variable, local or
      static, keeps it (`var_align_attrs`). Codegen: spec/codegen.md
      "clang `annotate`" (WP-1113).
  - **Class B attributes** (WP-813, `class_b_attrs.rs`, `AttrKind` 240
    to 273): the hardening, instrumentation, inlining and pointer-fact
    attributes (`no_stack_protector`, `patchable_function_entry`,
    `zero_call_used_regs`, `[no_]speculative_load_hardening`,
    `no_split_stack`, `no_instrument_function`,
    `no_profile_instrument_function`, `xray_*`,
    `disable_sanitizer_instrumentation`, `nocf_check`, `function_return`,
    `cfi_canonical_jump_table`, `force_align_arg_pointer`, `flatten`,
    `leaf`, `nomerge`, `not_tail_called`, `noduplicate`, `convergent`,
    `nouwtable`, `assume_aligned`, `align_value`, `callback`, `noescape`,
    `maybe_undef`, `code_align`, `loader_uninitialized`,
    `lto_visibility_public`, `omp::assume`, `standalone_debug`,
    `coro_await_elidable[_argument]` from clang 22) are spelled as
    clang's `Attr.td` (`kind_of`, the sorted spelling table) and their
    arguments, subjects and targets are checked with clang 19's
    diagnostics (`SemaDeclAttr.cpp` handlers; the x86-only ones are
    unknown elsewhere, `nocf_check` is ignored without
    `-fcf-protection=branch|full`). `speculative_load_hardening` /
    `no_speculative_load_hardening` and `always_inline` /
    `not_tail_called` exclude each other across redeclarations, as
    `naked` / `disable_tail_calls` do (`check_exclusive_pair`: `'X' and
    'Y' attributes are not compatible` with `conflicting attribute is
    here`, the later one dropped). The statement forms
    `[[clang::nomerge]]`, `[[clang::noinline]]`,
    `[[clang::always_inline]]` and `[[clang::code_align(N)]]` stay on the
    `AttributedStmt`. What codegen emits for each is
    spec/landed/codegen/WP-813.md; `__has_attribute` /
    `__has_cpp_attribute` are 1 for each. Not done (xfail repros in
    `tests/conformance/dcl.attr.class-b/`): `noescape` on a function
    type's parameter is not kept in the type, and clang 22's
    `malloc_span` is accepted silently (a WP-805 triage row), without
    clang's subject and argument checks.
- **Known and ignored, no diagnostic: the thread-safety family**
  (WP-606; clang `Attr.td`, the `-Wthread-safety` attributes, which
  libraries enable under `#if defined(__clang__)`; table
  `THREAD_SAFETY_ATTRS` in `decl_attrs.rs`, checked first in
  `attribute_node`). Each is recognised and attaches nothing: no
  `AttrKind`, no subject or argument-count check, no diagnostic.
  | spelling | names |
  |---|---|
  | GNU (`__attribute__((x))`, `__x__`) and `[[clang::x]]` / `[[_Clang::x]]` (clang `Clang<"x">`) | `capability`, `shared_capability`, `scoped_lockable`, `guarded_var`, `pt_guarded_var`, `requires_capability`, `requires_shared_capability`, `exclusive_locks_required`, `shared_locks_required`, `acquire_capability`, `acquire_shared_capability`, `try_acquire_capability`, `try_acquire_shared_capability`, `release_capability`, `release_shared_capability`, `release_generic_capability`, `unlock_function`, `assert_capability`, `assert_shared_capability`, `no_thread_safety_analysis`; `reentrant_capability` from `-fclang-compat=22` only (unknown at 19, as in clang 19) |
  | GNU only (clang `GNU<"x">`; `[[clang::x]]` is unknown) | `lockable`, `guarded_by`, `pt_guarded_by`, `acquired_before`, `acquired_after`, `exclusive_lock_function`, `shared_lock_function`, `exclusive_trylock_function`, `shared_trylock_function`, `assert_exclusive_lock`, `assert_shared_lock`, `locks_excluded`, `lock_returned` |

  `[[gnu::x]]` and unscoped `[[x]]` are unknown for every member. The
  parser skips their arguments as balanced tokens (`takes_expr_args` is
  false), so an argument may name a member declared later in the class,
  `this`, `!mu`, `*f()` or `mu.operator!()` and is never looked up or
  evaluated. Accepted in every position an attribute is: class heads,
  data members, variables, before a declarator, after a function's
  parameter list / cv / ref / `noexcept`, constructors, destructors,
  out-of-line definitions, templates, lambdas. `__has_attribute(x)` /
  `__has_cpp_attribute(clang::x)` answer from the generated clang table
  (`kiln_pp` `features_table.rs`): 1 for each GNU spelling, 1 for each
  `Clang<"x">` name under `clang::`, 0 for the GNU-only names there.
  Not yet (accepts what clang rejects): clang looks the arguments up —
  late, at class completion, for a GNU attribute in a class; where
  written for `[[clang::x]]` and outside classes — so an undeclared name
  is an error there; clang's subject warnings (`'guarded_by' attribute
  only applies to non-static data members and global variables`
  [-Wignored-attributes]), argument-count errors, and `'x' attribute
  cannot be applied to types` for `[[clang::x]]` after a function
  declarator are not diagnosed. Tests:
  `tests/conformance/dcl.attr.grammar/thread-safety-*.cpp`.
- **Sanitizer attributes** (WP-675; `class_b_attrs.rs`, triage in
  `attr_triage.rs`): `no_sanitize("<name>", ...)`, `no_sanitize_address`
  (and `no_address_safety_analysis`), `no_sanitize_thread`,
  `no_sanitize_memory` and `disable_sanitizer_instrumentation`, in
  clang's spellings (`__attribute__((x))` for every name;
  `[[clang::x]]` for `no_sanitize` and `no_sanitize_memory`;
  `[[gnu::x]]` for `no_sanitize_address`, `no_sanitize_thread`,
  `no_address_safety_analysis`, and `no_sanitize` from
  `-fclang-compat=22`), attach a sanitizer mask that codegen reads
  (spec/codegen.md §5.2). Names expand with the driver's groups
  (`kiln_ast::sanitize`); an unknown name is `unknown sanitizer '<x>'
  ignored` [-Wunknown-sanitizers]; on a global variable a name other
  than the address/hwaddress/memtag family is `'no_sanitize' attribute
  argument '<x>' not supported on a global variable`
  [-Wignored-attributes]; clang's subject warnings apply. Tests:
  `tests/conformance/dcl.attr.grammar/sanitizer-*.cpp`,
  `gnu-no-sanitize-clang{19,22}.cpp`.
- **Lifetime attributes** (WP-674; `class_b_attrs.rs`,
  `decl_attrs.rs`; the analysis is spec/sema-class.md §5.4 "Lifetime
  analysis"):
  - `[[clang::lifetimebound]]` / `__attribute__((lifetimebound))`
    (`AttrKind::LIFETIMEBOUND`) on a parameter (`a == 0`) or, written
    after a member function's declarator, on its implicit object
    parameter (`a == 1`). clang 19's subject checks: `'lifetimebound'
    attribute only applies to parameters and implicit object
    parameters` (an error) elsewhere, `'lifetimebound' attribute
    cannot be applied to types` in a type position, and the errors
    `'lifetimebound' attribute cannot be applied to a
    {constructor|destructor}` and `'lifetimebound' attribute cannot be
    applied; {static |non-}member function has no implicit object
    parameter`. Builtin `__builtin_addressof`'s parameter carries it
    implicitly.
  - `[[gsl::Owner]]` / `[[gsl::Pointer]]` (`GSL_OWNER`, `GSL_POINTER`)
    with an optional type argument (the owned / pointed-to type): only
    on classes and class templates (`'Owner' attribute only applies to
    structs` [-Wignored-attributes] on a union, enum or variable); `a reference type|an array type ... is an
    invalid argument to attribute` for a reference or array argument;
    `'Owner' and 'Pointer' attributes are not compatible` (with a note at
    the other) when a class has both, and the same error for a repeat
    naming a different type (as written) across the class's
    redeclarations. The implicit `std` annotations (clang
    `inferGslOwnerPointerAttribute`) are computed on demand by
    `Sema::gsl_kind` (`class/temporary/gsl.rs`).
- **Ignored with a warning:**
  - unknown attributes: `unknown attribute 'x' ignored`. Scoped names
    print without their scope, as clang 19 does.
- **Visibility attributes** (GNU extension, WP-544; clang
  `handleVisibilityAttr`, `mergeVisibilityAttr`,
  `AddPushedVisibilityAttribute`; `visibility.rs`): `visibility(S)` and
  `type_visibility(S)` in every GNU spelling (`__attribute__`,
  `[[gnu::]]`, `__visibility__`) are `AttrKind::VISIBILITY` /
  `TYPE_VISIBILITY` with `a` 0 default, 1 hidden (also `internal`),
  2 protected. In clang's order: exactly one argument (`'visibility'
  attribute takes one argument`), a string literal (`expected string
  literal as argument of 'visibility' attribute`) naming a known
  visibility (else `'visibility' attribute argument not supported: S`
  [-Wignored-attributes], dropped); ignored on a typedef (`'visibility'
  attribute ignored`); `type_visibility` only on a class, enumeration or
  namespace (`'type_visibility' attribute only applies to types and
  namespaces`). Two different values on one declaration, or a value that
  differs from a previous declaration's, are `visibility does not match
  previous declaration` (with `previous attribute is here`); an
  attribute first added on a redeclaration after the definition is
  `attribute declaration must precede definition` [-Wignored-attributes].
  `#pragma GCC visibility push(V)`/`pop` keeps a stack
  (`Sema::vis_stack`); an externally visible namespace-scope function or
  variable, or a class, declared under a push with no attribute on it or
  a previous declaration gets the pushed visibility as an implicit
  attribute. A namespace definition with a `visibility` attribute
  suspends the stack in its body (a marker, pushed before its `{` is
  consumed and popped before its `}` is, since consuming either lexes a
  following pragma): a `pop` there is `#pragma visibility pop with no
  matching #pragma visibility push` with `surrounding namespace with
  visibility attribute starts here`, and a push still open at its `}` is
  `#pragma visibility push with no matching #pragma visibility pop` (at
  the innermost push) with `surrounding namespace with visibility
  attribute ends here`, and is popped. `#pragma weak name` (clang
  `ActOnPragmaWeakID`, `ProcessPragmaWeak`) gives an implicit `weak`
  attribute to the function or variable `name` found by ordinary lookup
  in the translation unit's scope; when there is none it is recorded
  (`Sema::weak_pending`) and applied to the first `extern "C"` function
  or variable later declared with that name (WP-803;
  `tests/conformance/cpp.pragma/weak-run.cpp`). The attributes of an explicit
  instantiation of a function, variable or class template
  specialization ([temp.explicit]) are checked the same way and become
  the specialization's own (clang `ActOnExplicitInstantiation`); one that
  differs from an earlier explicit instantiation's replaces it after the
  mismatch error, reported at the earlier one. What the attributes mean is
  codegen's: spec/codegen.md §8. Tests:
  `tests/conformance/ext.gnu.visibility/`,
  `tests/conformance/cpp.pragma/gcc-visibility{,-syntax,-namespace}.cpp`,
  `tests/codegen/visibility_explicit_inst.cpp`.
- **Attribute namespaces** ([dcl.attr.grammar]/6, WP-373; clang
  `normalizeAttrScopeName`): the reserved spellings `__gnu__` and
  `_Clang` name the `gnu` and `clang` namespaces (`norm_attr_scope` in
  `decl.rs`), so libstdc++'s `[[__gnu__::__always_inline__]]` is
  `always_inline` with the GNU flag. `[[__clang__::x]]` reaches sema as
  `_Clang` (the parser reads the expanded `__clang__` macro as the
  scope, spec/parse.md §6.2, WP-891). Test:
  `tests/conformance/dcl.attr/reserved-attribute-namespace-spelling.cpp`.
- **Attribute names** (WP-678; clang `normalizeAttrName`,
  `kiln_ast::attr::normalize_attr_name` / `attr_name_normalizes`): for
  every attribute, in GNU syntax and in `[[...]]` with no scope or the
  (normalized) `gnu` or `clang` scope, a name written `__x__` is looked
  up as `x`: exactly one leading and one trailing `__`, removed only when
  both are present on a name of at least four characters. `__noinline`,
  `noinline__` and `____noinline____` are unknown attributes (named as
  written), `[[msvc::__noinline__]]` keeps its name, and clang's own
  one-sided spellings `__const` (`GCC<"__const">`) and
  `__clang_arm_builtin_alias` are matched as such. The parser's
  argument-shape tables, `__has_attribute` / `__has_cpp_attribute` and
  the diagnostic name (`attr_shown`) use the same rule. Test:
  `tests/conformance/dcl.attr.grammar/gnu-attribute-name-normalization.cpp`.
- **Mutual exclusions** (WP-890/WP-891; clang Attr.td `MutualExclusions`):
  `check_mutual_exclusions` (from `check_decl_attrs`) adds `hot`/`cold`
  on a function and `no_destroy`/`always_destroy` on a variable, with
  the same rule as `check_naked_conflicts`: the later attribute (on this
  declaration, or on this one when the other is on a prior one) is the
  error `'X' and 'Y' attributes are not compatible` with `conflicting
  attribute is here`, and is dropped. The other clang exclusions are
  already covered (`likely`/`unlikely`, `naked`/`disable_tail_calls`,
  `always_inline`/`not_tail_called`, gsl `Owner`/`Pointer`,
  `speculative_load_hardening`/`no_speculative_load_hardening`,
  `optnone` conflicts, the `target` family) or do not apply to C++ on
  kiln's targets (CUDA, Mips, Objective-C, `randomize_layout`); the
  class-C `cf_audited_transfer`/`cf_unknown_transfer` pair is accepted
  silently (list: spec/landed/sema/WP-891.md). Test:
  `tests/conformance/dcl.attr.grammar/mutually-exclusive-attributes.cpp`.
  - GNU `section("name")` (WP-716, clang `handleSectionAttr`): exactly
    one string literal (`expected string literal as argument of
    'section' attribute`, `'section' attribute takes one argument`), on
    functions and variables with static storage duration (clang's
    subject-list error otherwise); kept as `AttrKind::SECTION` with the
    literal, for codegen (spec/codegen.md §8 "Sections").
    Section type conflicts (WP-728, clang `Sema::UnifySection`,
    `section.rs`): each declaration placed in a named section gives it
    flags, read|execute for a function (each declaration that carries
    the attribute, when its attributes are checked), read or read|write
    for a variable with static or thread storage duration (at its
    definition, the attribute possibly inherited from an earlier
    declaration). A variable is read-only when its storage is constant
    (clang `isNonConstantStorage(ExcludeCtor=true, ExcludeDtor=false)`
    empty: a reference, or a `const` type or element type that, if a
    class, has no `mutable` member and a trivial destructor) and its
    initialization is a constant initialization (no initializer counts;
    a dependent one is not checked). The first declaration fixes the
    section's flags; a later one whose flags differ is an error `'b'
    causes a section type conflict with 'a'` (unqualified names) with a
    `declared here` note at the first. Nothing is placed inside a
    template instantiation, so a pattern counts and its specializations
    do not. `#pragma section` and `__declspec(allocate)` (clang
    `PSF_Implicit`) are not modelled. Test:
    `tests/conformance/ext.gnu.section/section-conflict.cpp`.
    (`visibility`: see "Visibility attributes")
  - GNU `format(archetype, F, A)` (WP-186, clang `handleFormatAttr`, in
    its order): exactly three arguments (`'format' attribute requires
    exactly 3 arguments`), the archetype an identifier (`requires
    parameter 1 to be an identifier`), `__x__` normalized; `printf`
    (and `gnu_printf` at clang_compat >= 22), `strftime`, and `scanf`,
    `printf0`, `strfmon`, `cmn_err`, `vcmn_err`, `zcmn_err`, `kprintf`,
    `freebsd_kprintf`, `os_trace`, `os_log`, `NSString`, `CFString`
    (and `gnu_scanf` at >= 22) are stored, `gcc_*diag` are accepted and
    dropped, anything else warns `'format' attribute argument not
    supported: X`. `F` and `A` integer constants (`requires parameter N
    to be an integer constant`); on a function only (`only applies to
    Objective-C methods, blocks, and non-K&R-style functions`, a
    warning), `F` in 1..=params (+1 for an implicit object member
    function, whose index 1 is `this`: `format attribute cannot specify
    the implicit this argument as the format string`), parameter `F` a
    pointer to (possibly cv) `char`/`signed char`/`unsigned char`
    (`format argument not a string type`), a non-variadic function with
    `A` != 0 warns `GCC requires a function with the 'format' attribute
    to be variadic` (`-Wgcc-compat`) at the declaration, `strftime`
    needs `A` = 0 (`strftime format attribute requires 3rd parameter to
    be 0`), others `A` = 0 or the `...` position (`parameter 3 is out of
    bounds`). Errors drop the attribute. Not modelled: GCC's position
    warning on a definition (`-Wgcc-compat`). Test:
    `tests/conformance/ext.gnu.builtins/format-attribute-written.cpp`,
    `format-gnu-archetypes-clang22.cpp`.
- **Argument encoding** (`Attr.a`, `Attr.b`; see spec/abi.md §3.1):

  | kind | `a` | `b` |
  |---|---|---|
  | ALIGNED | `ExprId` of the alignment expression | raw `QualType` of `alignas(type-id)` |
  | NODISCARD, DEPRECATED | `ExprId` of the string-literal message | — |
  | ASM_LABEL | raw `StringLitId` of the label (flags bit 1: inherited) | — |
  | BUILTIN | the builtin id (`kiln_ast::decl::builtin`; implicit, §5.4) | — |
  | FORMAT | the archetype: 0 `printf`, 1 another checked-by-clang kind (not checked by kiln), 2 `strftime` (`FORMAT_*` in `vector.rs`) | `F \| A << 16`, one-based format and first data argument indices, each saturated at 0xfffe (`A` = 0: a `va_list`); flags bit 0: implicit (§5.4) |
  | MALLOC | — | — |
  | ALLOC_SIZE | `ExprId` of an `IntegerLiteral` holding n | same for m, or 0 |
  | ALLOC_ALIGN | `ExprId` of an `IntegerLiteral` holding n | — |
  | ENUM_EXTENSIBILITY | 1 for `open`, 0 for `closed` | — |
  | INIT_PRIORITY | the checked priority (`u32`; the raw `ExprId` before `check_decl_attrs`) | — |
  | UNAVAILABLE | `ExprId` of the string-literal message, or 0 | — |
  | ANNOTATE | `ExprId` of the string literal | `ListId` of the other arguments (`ExprId`s, decayed), or 0 |
  | ENABLE_IF | `ExprId` of the condition (contextually converted to `bool` unless type-dependent) | `ExprId` of the string-literal message |

- **Semantics:**
  - `nodiscard` applies to functions, records and enumerations. A
    discarded call, or a discarded by-value result of such a type,
    warns. The callee is clang's `getReferencedDeclOfCallee` (WP-203):
    a name or member access (`m.f()`, `p->f()`, `m.operator-(1)`, a
    static member through an object), under parens, implicit casts and
    substituted template parameters, through `.*`/`->*` (the right
    operand) and unary `*`/`&`/`+`. A call without one never warns. The
    result type's attribute wins over the callee's (clang
    `getUnusedResultAttr`); a call returning a reference has no by-value
    result. Not for a `void` call. Reported at the callee's start.
  - `deprecated` takes an optional message string; the GNU spelling
    `__attribute__((deprecated(msg, replacement)))` also a replacement
    string (a fix-it's text, otherwise unused; LLVM's `LLVM_DEPRECATED`).
    A second argument in `[[deprecated]]`, `[[gnu::deprecated]]` or
    `__declspec` is `'deprecated' attribute takes no more than 1
    argument` and the attribute still applies with its message; a third,
    `takes no more than 2 arguments`; a non-string one, `expected string
    literal as argument of 'deprecated' attribute` at it (clang
    `handleDeprecatedAttr`, WP-730).
  - `deprecated` warns on use: a name, and a member named by a member
    access or an implicit `this->` (clang `DiagnoseUseOfDecl` in
    `BuildMemberReferenceExpr`, WP-203). Unlike clang, a non-dependent
    use in a template is not diagnosed again at instantiation (nor is
    a non-dependent discarded nodiscard call).
    Type names warn too (WP-297, `deprecated.rs`): a class, enumeration,
    typedef or alias named as a type (`check_type_name_access`), an
    elaborated-type-specifier that refers to a class, and a template-id
    naming a class template specialization (`class_template_type`, at
    the template-name, on parse and on substitution, the latter with
    instantiation notes). The deprecation is the decl's own attribute
    (on any redeclaration), else an undeclared specialization's
    pattern's (an explicit or partial specialization has its own class-head
    attributes); a typedef/alias looks through to its class or enum and
    an enumerator to its enumeration (clang
    `ShouldDiagnoseAvailabilityOfDecl`), the note naming that entity.
    A class specialization is named with its written arguments plus
    defaults (`D<N::X, int>` prints `D<N::X>` as clang does); a function
    specialization with its converted arguments, every name scoped
    (`visit_format_arg<(lambda at …), std::basic_format_context<char *,
    char>>`, clang `getNameForDiagnostic`; WP-303); both print typed
    arguments for placeholder parameters (`A<(n::E)5>`). No warning in a
    deprecated context: the current context or any enclosing semantic
    context is deprecated (clang `ShouldDiagnoseAvailabilityInContext`).
    Warnings for uses in a declaration's decl-specifiers, declarator and
    initializer wait for that declaration (clang's delayed
    diagnostics): every access delay point is also a deprecation delay
    point, and the parser opens one alone around a member-declaration's
    decl-specifiers and first declarator (`begin_delayed_deprecation`);
    they are dropped when the declared entity is deprecated or in a
    deprecated context, discarded with a dropped access delay point
    (specialization class-head arguments), and otherwise join the
    enclosing point. Delay applies only outside instantiation.
    Namespaces (WP-303, [dcl.attr.deprecated]/4, [namespace.def]/1):
    `act_on_namespace_attrs` attaches a namespace definition's
    attributes (both positions, `namespace [[a]] N [[b]] {`) to that
    definition only; the attribute is not inherited by a reopening, and
    lookup finds the latest definition, so a use after an unattributed
    reopening does not warn (clang). A use warns at the
    nested-name-specifier's `::` (`act_on_nns_component_use`), at the
    name of a using-directive and of a namespace alias's target
    (`act_on_namespace_name_use`). On an unnamed namespace the attribute
    is dropped with "'deprecated' attribute on anonymous namespace
    ignored" [-Wignored-attributes]. A deprecated namespace is a
    deprecated context.
  - `maybe_unused` suppresses unused warnings.
  - `alignas` is validated as a power of two and fed into layout
    (`check_alignment_expr`). `requested alignment is not a power of 2`
    and `requested alignment must be ... or smaller` are reported at the
    attribute token, as clang's `AddAlignedAttr` does, not at the
    expression.
  - `alignas` operands ([dcl.align]/1, [temp.variadic]/5,
    `act_on_alignas_operand`, WP-302): `alignas(T...)` / `alignas(e...)`
    build a pack expansion (a type operand through
    `act_pack_expansion_type`, so an expansion with no pack is `pack
    expansion does not contain any unexpanded parameter packs` at the
    `...`). An operand that contains an unexpanded pack without `...` is
    `expression contains unexpanded parameter pack 'Ts'` at the operand's
    begin (`sema_unexpanded_pack`, `diagnose_unexpanded_pack` in
    `templ/pack.rs`; a type-id operand counts as an `alignof` expression;
    two names print `packs 'A' and 'B'`, more `packs 'A', 'B', ...`), and
    the attribute is dropped. Instantiation expands the operand and
    attaches one `ALIGNED` attribute per element (clang's
    `instantiateDependentAlignedAttr`), so the strictest wins and an empty
    pack leaves the natural alignment; each element is re-checked at the
    attribute token (spec/sema-templates.md §2.7). On variables the
    attributes are copied too (`var_align_attrs`), and a variable
    template specialization copies the pattern's alignment-specifiers
    with a dependent operand substituted (`templ/var.rs`, clang
    `InstantiateAttrs`).
  - Variable alignment ([dcl.align]/5, WP-312): an `alignas`/`aligned`
    attribute on a variable or data member sets the alignment of its
    storage and of `alignof` applied to an expression naming it (clang
    `getDeclAlign`; `kiln_abi::layout::decl_align`, spec/abi.md §3.2).
    Constant evaluation reads it through `SemaBridge::alignof_decl`;
    codegen through `var_align` (spec/codegen.md §3). Tests:
    `tests/conformance/dcl.align/{variable,member}-alignas.cpp`,
    `tests/codegen/alignas_variable.cpp`.
  - `alignas` subjects ([dcl.align]/1, `check_alignas_subject`):
    allowed on a variable, a non-bit-field data member and a class. On
    a function parameter, a `register` variable, a `catch` variable, a
    bit-field or an enumeration it is `'alignas' attribute cannot be
    applied to {a function parameter|...}`; on any other declaration
    `'alignas' attribute only applies to variables, data members and
    tag types`; in an attribute-declaration (`alignas(8);`) it
    appertains to nothing. The specifier is dropped. Written where only
    type attributes may appear (`char alignas(8) c;`) it is `'alignas'
    attribute cannot be applied to types` (the type-position rule
    below).
    A leading `alignas` follows the same placement rules as any C++11
    attribute (spec/parse.md, attribute placement): `an attribute list
    cannot appear here` before a namespace-definition, static_assert,
    alias-declaration and the rest, in every scope; before a
    using-directive or using-declaration it is subject-checked like any
    other attribute. Tests: `tests/conformance/dcl.align/alignas-*.cpp`,
    `tests/conformance/dcl.attr.grammar/leading-attribute-placement*.cpp`.
  - Leading attributes on declarations without declarators
    ([dcl.pre]/5, [dcl.attr.grammar]/5; WP-324):
    - attribute-declaration at namespace scope (`[[x]];`):
      `act_on_decl_attrs` checks the attributes against the enclosing
      context, so `alignas(4);` is the alignas subject error and
      `[[noreturn]];` the noreturn one; no `declaration does not
      declare anything`.
    - a declaration that declares only a named tag (`[[x]] struct S
      { };`, `struct S { } [[x]];`): each named attribute gets
      `attribute 'X' is ignored, place it after "struct" to apply
      attribute to type declaration` (`-Wignored-attributes`,
      `warn_free_standing_tag_attrs`; the keyword is `struct`, `class`,
      `union`, `enum`, `enum class` or `enum struct`), after the
      parser's error for the position, if any. Not for an anonymous
      record.
    - GNU attributes right after a class definition's closing brace
      (`struct S { ... } __attribute__((packed));`, WP-416) are not
      decl-specifier attributes: as clang's `ParseCXXMemberSpecification`
      the parser reads them there and `act_on_record_trailing_attrs`
      checks them against the class and appends them to its own
      attributes before `act_on_finish_record_definition`, so `packed` /
      `aligned` there shape its layout (also for a class template and
      its specializations), with no warning, whether or not declarators
      follow. An enumeration's closing brace takes them the same way
      (WP-432, before `act_on_finish_enum_definition`). Test:
      `tests/conformance/dcl.attr.grammar/gnu-attributes-after-class-body.cpp`.
    - on a non-declaration statement: `'X' attribute cannot be applied
      to a statement` per declaration attribute
      (`act_on_stmt_decl_attrs`); on a block-scope asm-declaration
      clang's `'X' attribute ignored` warning instead.
    - `[[deprecated]]` on a using-declaration: `'deprecated' currently
      has no effect on a using declaration` (`-Wignored-attributes`),
      dropped.
  - Alignment across redeclarations ([dcl.align]/6, `align_redecl.rs`
    `merge_aligned_redecl`, clang `mergeAlignedAttrs`; WP-324), for
    variables, data members and classes. The old declaration's
    alignment attributes are its own plus those it inherited
    (`Sema::inherited_aligned`). When both the old and the new
    declaration carry an `alignas` and their largest requests differ
    (`alignas(0)` counts as the natural alignment), it is `redeclaration
    has different alignment requirement (N vs M)` (new, old) at the new
    `alignas`, with `previous declaration is here` at the old one. A
    definition without `alignas` after a declaration with one is
    `'alignas' must be specified on definition if it is specified on any
    declaration` at the definition's name, with `declared with 'alignas'
    attribute here` at the old `alignas`; so is an `alignas` added on a
    redeclaration after a definition that had none (the attribute is
    then dropped). A redeclaration without `alignas` after one with it is
    fine and inherits the old `alignas`, and one with a weaker request
    inherits the strictest. A class redeclaration that names an existing
    class (`struct alignas(8) S;`) is checked against it
    (`merge_aligned_folded_redecl`). A dependent request skips the
    check. The earlier definition (`definition_before`, a walk back
    along the chain) is looked up only when the new declaration has an
    `alignas` of its own, the one case it matters (WP-1076). Test:
    `tests/conformance/dcl.align/redecl-alignment-diag.cpp`;
    `tests/stress/redecl-chain-65536.cpp` pins that merging 65,536
    redeclarations stays linear (visibility attributes and the
    out-of-class static data member check also skip the chain walk when
    the declaration has nothing to merge or is not a member).
  - Under-alignment ([dcl.align]/5, `check_alignas_underalignment`,
    clang `CheckAlignasUnderalignment`; WP-324): when a declaration has
    an `alignas` (own or inherited) and the largest requested alignment
    (`alignas` and GNU `aligned`, whose bare form is the target's
    biggest alignment; `alignas(0)` requests nothing) is nonzero and
    less than the natural alignment of its type, it is `requested
    alignment is less than minimum alignment of N for type 'T'` at the
    last `alignas`. Checked for a variable or data member when its
    declarator is finished (`finish_decl_alignment`) and for a class or
    union when its definition completes, including one written in a
    typedef or alias-declaration and a class template specialization
    (at the pattern's `alignas`, with the instantiation note); the
    type's natural alignment ignores the request itself. Test:
    `tests/conformance/dcl.align/underalignment-diag.cpp`.
  - `alignof` of an expression ([expr.alignof]/1 takes a type-id) is
    the GNU extension `'alignof' applied to an expression is a GNU
    extension` (`-Wgnu-alignof-expression`, in `-Wgnu`, at the keyword;
    not for `__alignof`/`__alignof__`), after the operand's own errors
    and even when the operand is invalid (`alignof(F::x)` with `F`
    incomplete, `alignof(1 + )`; clang's `ext_alignof_expr` is emitted
    before the operand is checked; WP-430,
    `expr.alignof/incomplete-qualifier-gnu-expression.cpp`). `alignof` of a data member
    (named ignoring parentheses) of a class still being defined is
    `invalid application of 'alignof' to a field of a class still being
    defined` at the operand's own location (clang `CheckAlignOfExpr`;
    [class.mem.general]/8). Test:
    `tests/conformance/dcl.align/alignof-expression-diag.cpp`.
  - `act_on_tag_attrs` attaches class-head and enum-head attributes.
  - **`init_priority(N)`** (GNU, [dcl.attr.grammar]/5;
    `init_priority.rs`, clang `handleInitPriorityAttr`, WP-189). Parsed
    with an expression argument (`__attribute__` or `[[gnu::]]`); exactly
    one, else `'init_priority' attribute takes one argument` (under
    compat ≥ 22 the `[[gnu::]]` spelling is named `'gnu::init_priority'`).
    `check_decl_attrs` then checks, in clang's order, dropping the
    attribute on the first error: the subject is a variable (`only
    applies to variables`); it is not declared in a function and its type,
    arrays stripped, is a class (`can only use 'init_priority' attribute
    on file-scope definitions of objects of class type`); the argument is
    an integer constant (`'init_priority' attribute requires an integer
    constant`, at the attribute) whose value fits in 32 bits (`integer
    constant expression evaluates to value N that cannot be represented
    in a 32-bit unsigned integer type`, at the expression); and the
    range. Under compat < 22, outside 101..=65535 is `'init_priority'
    attribute requires integer constant between 101 and 65535 inclusive`
    (not checked in a system header). Under compat ≥ 22, above 65535 is
    the same error with bounds 0 and 65535, and below 101 is `requested
    'init_priority' N is reserved for internal use`
    (`-Winit-priority-reserved`, default error; the attribute is kept).
    A surviving attribute stores the priority in `a`; codegen orders the
    variable's initialization by it (spec/codegen.md §7f).
    `tests/conformance/dcl.attr/gnu-init-priority*.cpp`.
  - **Attribute subjects** ([dcl.attr.grammar]/5, `decl_attrs.rs`
    `check_decl_attrs`, clang `diagAppertainsToDecl` and the per-kind
    handlers; WP-324). One table, applied to the attributes of every
    declarator (variables, functions, data members, typedefs), every
    alias-declaration (`using X [[a]] = T;` appertains to the
    typedef-name, [dcl.typedef]/2) and every class-head and enum-head.
    An attribute the subject rejects is diagnosed at its token and
    dropped:
    - `nodiscard`/`warn_unused_result` ([dcl.attr.nodiscard]/1): the
      subject must be a class, an enumeration, a typedef, or a
      declaration whose type is a function type or a pointer or
      reference to one; otherwise `'X' attribute only applies to
      Objective-C methods, enums, structs, unions, classes, functions,
      function pointers, and typedefs` (warning). Then, in order: a
      function type returning `void` (not a constructor) is `attribute
      'X' cannot be applied to functions without return value`
      (warning); the standard `[[nodiscard]]` on a variable or parameter
      is `'nodiscard' attribute only applies to functions, classes, or
      enumerations` (warning); a non-GNU spelling on a typedef is
      `'X' attribute ignored when applied to a typedef; consider using
      '__attribute__((warn_unused_result))' or
      '[[clang::warn_unused_result]]' instead` (warning;
      `__attribute__((warn_unused_result))` there is kept). The GNU
      spellings (`__attribute__`, `[[gnu::...]]`) are told apart by
      `AttrNode::is_gnu_spelling` (the `GNU_SCOPED` flag).
    - The standard `[[noreturn]]` ([dcl.attr.noreturn]/1) on anything
      but a function is `'noreturn' attribute only applies to
      functions` (error). The GNU `noreturn` is accepted on a function
      and on a variable, data member or typedef whose type is a
      function type, a pointer, reference or member pointer to one, or
      dependent; on another such declaration it is `'noreturn' only
      applies to function types; type here is 'T'`, on anything else
      `'noreturn' attribute only applies to functions and methods`
      (warnings).
    - `no_unique_address` ([dcl.attr.nouniqueaddr]/1): only a
      non-bit-field non-static data member (error `... only applies to
      non-bit-field non-static data members`). `carries_dependency`:
      only a function or parameter (error `... only applies to
      parameters, Objective-C methods, and functions`); on a parameter
      of a function declarator that declares no function or lambda (a
      pointer to function, a typedef, an alias; [dcl.attr.depend]/1)
      the error `'[[carries_dependency]]' attribute only allowed on
      parameter in a function declaration or lambda` and it is dropped
      (`carries_dep_attr.rs`, from the parser's parameter clause). On a
      redeclaration of a function (or its parameter i) when no earlier
      one has it ([dcl.attr.depend]/2): `function declared
      '[[carries_dependency]]' after its first declaration` (`parameter
      declared ...`) with the note `declaration missing
      '[[carries_dependency]]' attribute is here` at the first
      declaration (or its parameter i) (WP-954); from C++26
      (P3475R2, WP-743) it is no standard attribute:
      `__has_cpp_attribute(carries_dependency)` is 0 and
      `[[carries_dependency]]` (arguments too, skipped) warns `unknown
      attribute 'carries_dependency' ignored` [-Wunknown-attributes]
      and is dropped, the warning clang gives any unknown attribute
      (GCC 16.1 warns `'carries_dependency' attribute ignored`)
      (`tests/conformance/dcl.attr.depend/`). `packed`: a
      class, an enumeration or a data member; otherwise `'packed'
      attribute ignored` (warning).
    - `indeterminate` ([dcl.attr.indet]/1, C++26 P2795R5, WP-448;
      `AttrKind::INDETERMINATE`): only a variable with automatic
      storage duration (no `static`, `thread_local` or `extern`) or a
      parameter; elsewhere the error `'indeterminate' attribute only
      applies to local variables or function parameters` (clang trunk's
      subject list `LocalVar, ParmVar`). It may repeat ([dcl.attr.grammar]/4,
      P2156R1). On a parameter it must be on the function's first
      declaration: a redeclaration adding it is the error
      `'[[indeterminate]]' attribute on parameter 'b' must appear on the
      first declaration of the function` at the attribute, with `previous
      declaration is here` at the first declaration
      (`check_param_indeterminate`; clang trunk's text, the note is
      kiln's). Before C++26 it is not recognised: `unknown attribute
      'indeterminate' ignored` [-Wunknown-attributes]. Effect: codegen
      writes no erroneous value (spec/codegen.md §7r); constant
      evaluation is unchanged (an indeterminate read is non-constant).
      Tests: `tests/conformance/dcl.attr.indet/`.
    - `clang::uninitialized` / GNU `uninitialized` (WP-452;
      `AttrKind::UNINITIALIZED`, `check_uninitialized_subject`): only a
      variable with automatic storage duration; elsewhere ignored with
      the warning `'clang::uninitialized' attribute only applies to
      local variables` [-Wignored-attributes] (clang's `LocalVar`).
      Effect: codegen writes no fill under `-ftrivial-auto-var-init` or
      C++26's erroneous value (spec/codegen.md §7y). Test:
      `tests/conformance/basic.indet/uninitialized-attr.cpp`.
    - `fallthrough`, `likely`, `unlikely` and `assume` are statement
      attributes: on any declaration `'X' attribute cannot be applied to
      a declaration` (error).
    - `alignas`: the `alignas` subjects rule above.
    - `deprecated`, `maybe_unused` and the GNU attributes kiln models
      are accepted on all of these (a deprecated alias warns at its
      uses).
    Not yet: parameters (`act_on_param_declarator` only checks
    `alignas`) and namespace-definitions are not run through the table.
    Test: `tests/conformance/dcl.attr.grammar/attribute-subjects.cpp`.
  - **`pass_object_size(t)`** / **`pass_dynamic_object_size(t)`** and
    **`overloadable`** (clang extensions; WP-803, `decl.rs`
    `pass_object_size_attribute`, `decl_attrs.rs`
    `check_param_pass_object_size`; glibc's clang `_FORTIFY_SOURCE`
    headers): one argument, an integer constant 0..=3 ("requires integer
    constant between 0 and 3 inclusive" at the argument, "takes one
    argument"); subject a pointer parameter ("'pass_object_size'
    attribute only applies to constant pointer arguments" at the
    parameter's start, the attribute dropped), a `const` one in a
    definition (the same error, at the parameter's name, the name
    unquoted, as clang); on a non-parameter a -Wignored-attributes
    warning. `overloadable` on a non-function is an error. A function
    declared `overloadable` is mangled even inside `extern "C"` and has
    no builtin or known-library identity (clang `getBuiltinID`). Overload
    rules: spec/sema-overload.md §4.3a; calls: spec/codegen.md
    (`__builtin_object_size`). Tests:
    `ext.clang.pass.object.size/pass-object-size-{run,errors}.cpp`.
  - **`enable_if`** (clang extension, `__attribute__((enable_if(cond,
    "message")))` only; WP-460, `enable_if.rs`; clang AttrDocs
    "enable_if", `Sema::CheckEnableIf`):
    - Form: exactly two arguments. The parser parses them with the
      function's parameters in scope (as a trailing requires-clause), so
      the condition can name them. The condition is contextually
      converted to `bool` unless type-dependent; the message must be a
      string literal ("expected string literal as argument of 'enable_if'
      attribute"); another argument count is "requires exactly 2 arguments".
    - Subject: functions. On anything else, "'enable_if' attribute only
      applies to functions" (warning, `-Wignored-attributes`) and the
      attribute is dropped.
    - Identity: a function declaration whose conditions are not those of
      an earlier declaration, in order and by equivalence
      ([temp.over.link]/5 profiles), declares an overload, not a
      redeclaration (clang `IsOverload`; `NewFn::enable_if`). So every
      declaration of one function has the same conditions, and a
      declaration's own attributes are the function's. This holds for
      special members and instantiated class-template members too, and
      the conditions are mangled (spec/sema-overload.md §4.3).
    - Viability: after a candidate is otherwise viable
      (`add_candidate`), each argument is converted to its parameter's
      type with diagnostics captured (a failing one gives its parameter
      no value), an omitted argument is its default argument, and each
      condition in declaration order is evaluated by
      `Oven::evaluate_with_params` (spec/oven.md) with those values for
      the parameters. A condition that is value-dependent, not a
      constant expression, or false makes the candidate not viable;
      its note is "candidate disabled: <message>" at the function
      (`<no message provided>` for an empty one). The arguments bind
      from the first parameter, except that a non-static member
      function's object argument binds to no parameter. Two cases follow
      clang (WP-486): a non-member operator function's first operand
      binds to its first parameter ([over.match.oper]/2); a member
      function with an explicit object parameter gets the arguments
      *without* the object argument, bound from the object parameter on
      (they shift by one, clang `CheckEnableIf` passing the call's
      arguments), and an argument that does not convert to the
      parameter it lands on, or a parameter left with neither an
      argument nor a default argument, disables it by its first
      condition (clang `convertArgsForAvailabilityChecks` failing).
      If evaluating with all
      values fails, the arguments that are not constant expressions on
      their own are dropped and the condition is evaluated again, so
      `__builtin_constant_p(n)` of a non-constant argument is false
      rather than not constant (clang `EvaluateWithSubstitution`).
    - A call without overload resolution (a member call, a call through
      a name of one function) is checked on its converted arguments in
      `convert_call_args`, the object argument of an explicit object
      member function binding to the object parameter ([over.match.funcs.
      general]/4): "no matching function for call to 'f'" at the callee,
      or "no matching member function for call to 'm'" at the member
      name (any member function, clang `BuildCallExpr`'s
      `isa<CXXMethodDecl>`), with the same note. A call that overload
      resolution chose is not checked again (`CallCallee::enable_if`;
      `OvlState::resolved_member` marks a resolved explicit object member
      call, and an operator call never re-checks).
    - Ranking ([over.match.best], last; clang `compareEnableIfAttrs`): a
      candidate with conditions beats one without; between two with
      conditions, compared in order, the one whose list properly extends
      the other's (equivalent prefix) wins; otherwise neither is better
      (an ambiguity).
    - Templates: the condition is substituted with the pattern's
      parameters naming the specialization's (`Substituter::
      enable_if_cond`, from `copy_attrs`), after deduction; a failed
      substitution drops the attribute.
    - Never constant (WP-486, `attr_cond.rs`; clang
      `checkFunctionConditionAttr`, `Expr::isPotentialConstantExprUnevaluated`):
      a condition (of `enable_if` or `diagnose_if`) that is not
      value-dependent and can be a constant expression for no values of
      the parameters is "'enable_if' attribute expression never produces
      a constant expression" (error, at the attribute) with the
      evaluation's notes, and the attribute is dropped. kiln walks the
      condition as clang's evaluator goes on after failing to read a
      parameter: a subexpression naming no parameter (nor `this`) that is
      a prvalue of arithmetic or enumeration type is evaluated on its own
      (a failure with notes decides); one naming a parameter is first
      evaluated whole (a success is a value, e.g. a parameter only in
      `sizeof`), then taken apart — every operand of a built-in operator,
      cast, subscript, member access or call (whose body is then not
      entered), in order; `&&`/`||` stop at an unknown left operand and
      short-circuit on a known one; `?:` takes the known condition's arm,
      and with an unknown condition fails only if both arms do ("both arms
      of conditional operator are unable to produce a constant
      expression", at the expression). Anything else is maybe constant: a
      condition is never rejected that clang accepts, but some that clang
      rejects are missed (a call whose arguments are all known but name a
      parameter; a non-scalar operand). Checked at the declaration
      (`check_decl_attrs`) and, for a dependent condition, after
      substitution (`enable_if_cond`); there the error is a hard one even
      during deduction (clang substitutes attributes in the
      specialization's instantiation context), its context note "in
      instantiation of function template specialization 'f<X>' requested
      here" replacing the deduction's, and the attribute is dropped
      rather than the candidate.
    - Not done: taking the address of a
      function with a non-tautological condition ("cannot take address
      of function 'f' because it has one or more non-tautological
      enable_if conditions"); clang gives a class-type by-value parameter
      initialized by copying an lvalue no value (its implicit copy
      constructor is not yet defined), kiln evaluates the copy.
    - Tests: `tests/conformance/dcl.attr/enable-if-viability.cpp`,
      `enable-if-ranking.cpp`, `enable-if-explicit-object.cpp`,
      `attr-cond-never-constant.cpp`.
  - **`diagnose_if`** (clang extension, `__attribute__((diagnose_if(cond,
    "message", "error"|"warning"[, "group"])))`; WP-486, `diagnose_if.rs`;
    clang AttrDocs "diagnose_if", `Sema::diagnoseArgDependentDiagnoseIfAttrs`
    / `diagnoseArgIndependentDiagnoseIfAttrs`):
    - Form: the parser parses the arguments with the function's parameters
      in scope, as for `enable_if`. Clang 22 takes 3 or 4 arguments ("'x'
      attribute takes at least 3 arguments" / "takes no more than 4
      arguments"), clang 19 exactly 3 ("requires exactly 3 arguments"),
      each at the attribute name. The condition is contextually converted
      to `bool` unless type-dependent; the message, type and group must be
      string literals ("expected string literal as argument of
      'diagnose_if' attribute", at the argument); the type must be "error"
      or "warning" ("invalid diagnostic type for 'diagnose_if'; use
      \"error\" or \"warning\" instead"); the group must name a warning
      group kiln knows ("unknown warning group 'x'"). Any of these drops
      the attribute. Subject: functions; on anything else "'diagnose_if'
      attribute only applies to functions, Objective-C methods, and
      Objective-C properties" (`-Wignored-attributes`).
    - Kind: a condition that names a parameter or `this` is
      *argument-dependent*; any other is *argument-independent*.
    - Argument-dependent conditions are evaluated at each call with the
      converted arguments, default arguments included, by the same
      evaluation as `enable_if` (`cond_with_args`: a non-constant argument
      is dropped and the condition retried). The arguments bind as for
      `enable_if`, the object argument of an explicit object member
      function binding to the object parameter. The diagnostic is at the
      call's `)`, at the operator of an operator call, and at the
      declarator of a constructed variable. Hooks: `convert_call_args`
      (every call, including a static member function named through a
      member access) and `use_ctor`.
    - Argument-independent conditions are evaluated at each use of the
      function (every `DeclRefExpr`, `diagnose_if_use`), also inside a
      template definition.
    - Result: if any true condition is an "error", the first one alone is
      reported, as an error; otherwise each true "warning" is reported, in
      declaration order. Conditions accumulate over redeclarations, oldest
      first. A condition that is not a constant expression (or is
      value-dependent) is ignored. Every diagnostic is the message
      verbatim; warnings are in `-Wuser-defined-warnings` and show in
      system headers (clang `ShowInSystemHeader`, so libc++'s
      `_LIBCPP_DIAGNOSE_WARNING` checks fire). Each has the note "from
      'diagnose_if' attribute on 'f':" at the attribute, except that an
      attribute with a group gets no note. Duplicates are not merged
      (two attributes may report at one location).
    - Templates: a call in a template definition is not diagnosed there.
      A non-dependent call is shared by the instantiations rather than
      rebuilt, so it is recorded (`Sema::diagnose_if_recheck`) and checked
      again whenever an instantiation reaches it (`mark_shared_expr_
      referenced`); a dependent call is checked when it is rebuilt. The
      condition is substituted like `enable_if`'s, and checked like it for
      never being constant.
    - Explicit object member calls inside a template are rebuilt through
      `rebuild_explicit_object_call`, keeping the object argument bound to
      the object parameter (before WP-486 such a rebuilt call was "called
      object type '…' is not a function or function pointer").
    - Not done: a condition naming `this` is evaluated without the object
      (clang evaluates it on a constant object, e.g. `cs.m()` on a
      `constexpr S cs`), so it is ignored; the group argument does not put
      the diagnostic in that group (it stays in `-Wuser-defined-warnings`,
      so `-Wno-<group>` does not silence it); an argument-independent
      "error" does not make the use fail overload resolution.
    - Tests: `tests/conformance/dcl.attr/diagnose-if.cpp`,
      `diagnose-if-form.cpp`, `diagnose-if-form-19.cpp`.
  - **The `trivial_abi` attribute** (clang extension, `[[clang::trivial_abi]]`
    / `__attribute__((trivial_abi))`; WP-460, `trivial_abi.rs`; clang
    AttrDocs "trivial_abi", `Sema::checkIllFormedTrivialABIStruct`):
    - Subject: classes and unions; elsewhere "'trivial_abi' attribute only
      applies to classes" (`-Wignored-attributes`). It takes no arguments
      ("'trivial_abi' attribute takes no arguments", error; the parser
      keeps the argument list so the check can fire).
    - Dropped at the class's `}`, with "'trivial_abi' cannot be applied to
      'X'" (`-Wignored-attributes`) and a note "'trivial_abi' is disallowed
      on 'X' because …" naming the first reason in clang's order: it is
      polymorphic; it has a base that cannot be passed in registers or a
      virtual base ("has a base of a non-trivial class type" / "has a
      virtual base"); it has a field whose class cannot be ("has a field
      of a non-trivial class type"); then, unless the class is trivially
      relocatable (`cxx_relocatable_class`), "its copy constructors and
      move constructors are all deleted". A template instantiation drops
      it silently.
    - In effect, the class's copy and move constructors and destructor are
      trivial for the purposes of calls (`DefinitionBits::TRIVIAL_ABI`).
      Otherwise a declared non-trivial, non-user-provided member is
      trivial for calls when the corresponding members of every base and
      non-reference field class are (clang `addedClassSubobject`), recorded
      as `COPY_CTOR_FOR_CALL` / `MOVE_CTOR_FOR_CALL` / `DTOR_FOR_CALL`
      (`trivial_abi_bits`, computed before the class's properties freeze).
    - clang quirk, modelled: the implicit members clang declares eagerly at
      `}` (`implicit_bits`: copy/move constructors needing overload
      resolution or with inherited constructors, the destructor of a
      dynamic class) are marked trivial for calls *before* the attribute
      is dropped, so a class whose attribute is dropped for a virtual base
      or a non-trivial subobject still passes in registers when its only
      non-trivial members are those (`__can_pass_in_regs(H)` is true for
      `struct [[clang::trivial_abi]] H : N {}` in clang 19 and 22).
    - Callee-destroyed parameters (`mark_callee_destroyed_param`, clang
      `CheckParmsForFunctionDef`): at a function definition, the
      destructor of each by-value parameter the callee destroys
      (`kiln_abi::call::is_destroyed_in_callee`) is referenced, so an
      implicit one is defined. ABI and codegen: spec/abi.md §8.26,
      spec/codegen.md §7ac.
    - `__builtin_is_cpp_trivially_relocatable` is not implied (clang 22
      answers false for a class with user-provided copy/move members);
      `__is_trivially_relocatable` follows `can_pass_in_registers` as in
      clang; at compat 22 it warns `-Wdeprecated-builtins` (builtin
      traits §, WP-486).
    - Tests: `tests/conformance/dcl.attr/trivial-abi-attribute.cpp`,
      `tests/conformance/class.temporary/run-trivial-abi-callee-destroys.cpp`,
      `tests/codegen/trivial_abi_calls.cpp`, `trivial_abi_subobjects.cpp`.
  - **Type-position attributes** ([dcl.spec.general]/1, [dcl.ptr]/1,
    [dcl.array]/1, [dcl.fct]/1; `act_on_type_position_attrs`, clang
    `err_attribute_not_type_attr`; WP-312, WP-324). An
    attribute-specifier-seq at the end of a decl-specifier-seq or of an
    alias-declaration's type-id (`using X = int [[deprecated]];`), after
    a ptr-operator or array declarator, or after a function declarator
    (`void f() [[noreturn]];`) appertains to a type. A declaration
    attribute written there in `[[...]]` or `alignas` syntax is `'X'
    attribute cannot be applied to types` (error), except that after a
    function declarator a `[[gnu::X]]` one is `attribute 'X' ignored,
    because it cannot be applied to a type` (warning); either way it is
    dropped. `__attribute__((...))` there, and the type attributes
    (`vector_size`, `ext_vector_type`, `mode`, ...), are kept (spec/parse.md
    says where they attach).

### 4.6 Namespaces, linkage specifications, static_assert
- **Namespaces:**
  - Named namespaces are reopened ([namespace.def]).
  - **Reopening through the inline namespace set**
    ([namespace.def.general]/3, WP-309): the identifier of a named
    namespace definition is looked up in the enclosing namespace and in
    every element of its inline namespace set. If it finds a namespace
    `A::I::R` (with `I` inline in `A`), `namespace A { namespace R {` extends
    that namespace. The reopening `NamespaceDecl` has semantic ctx `A::I`
    and lexical ctx `A`. Test:
    tests/conformance/namespace.def/reopen-through-inline.cpp.
  - **Mangling and printing an entity of such a reopening** (clang, our
    ABI target; GCC mangles through the semantic path, and GCC
    compatibility is not a goal here). A reopening namespace mangles and
    prints under its lexical ctx, not its semantic one. This is clang's
    `getEffectiveDeclContext` / `printQualifiedName` walk over the
    reopening `NamespaceDecl`, implemented in `kiln_abi::mangle::effective_ctx` and
    `kiln_codegen` `qualified_name`. For the code above, `int y;` declared in the
    reopened `R` is `_ZN1A1R1yE`, and a class `U` there is LLVM type
    `%"struct.A::R::U"`. `int x;` from the original definition stays
    `_ZN1A1I1R1xE`. The namespace part comes from the namespace
    definition that holds the entity's **first declaration** (codegen
    mangles the first declaration):
    - A function or static member declared in `A::I::R` and defined in
      the reopening keeps `_ZN1A1I1R…`.
    - A member of a class keeps its class's path.
    - An explicit specialization mangles in its template's namespace.
    - Known clang divergence: clang mangles whichever redeclaration it
      reaches first. Take an entity declared in `A::I::R` and defined in
      the reopening before any use. Clang uses the reopening's `A::R`
      path and kiln uses `A::I::R`. clang 22 behaves the same. This
      deviation is kept (WP-325): mangling one fixed declaration keeps
      the symbol independent of declaration and use order. It is pinned
      to that case only. An entity redeclared or defined in another
      definition of the same namespace (`namespace A { inline namespace
      I { namespace R {` again, or `namespace A::inline I::R {`) mangles
      identically under both compilers (differential run test
      tests/conformance/namespace.def/redeclaration-mangling.cpp, which
      links only if kiln emits clang's symbols).
    - Test: tests/codegen/namespace_reopen_inline.cpp (IR diff against
      clang++-19 on both triples).
  - An unnamed namespace is created once per scope and gets an
    implicit `UsingDirectiveDecl` ([namespace.unnamed]). "Scope" is the
    primary redeclaration context (`Sema::anon_namespaces`, keyed by
    `primary_context(redecl_context(ctx))`, clang's
    `getAnonymousNamespace` on the original namespace): an unnamed
    namespace in a reopened `namespace N` or inside `extern "C++" { }`
    extends the first one (WP-397). Its declaration location is the
    `{` (clang's `NamespaceDecl` location for an unnamed namespace).
  - **Inline mismatch on reopening** ([namespace.def.general]/3,
    WP-389; clang `DiagnoseNamespaceInlineMismatch`,
    `Sema::check_namespace_inline_mismatch`): when a namespace
    definition extends one whose first definition differs in
    `inline`, the extension takes the first definition's inlineness.
    Reopening an inline namespace without `inline` is the warning
    `sema_inline_namespace_reopened_noninline` ("inline namespace
    reopened as a non-inline namespace",
    `-Winline-namespace-reopened-noninline`); reopening a non-inline
    one with `inline` is the error `sema_inline_namespace_mismatch`
    ("non-inline namespace cannot be reopened as inline"). Both carry
    "previous definition is here" at the first definition and are
    reported at the name (for an unnamed namespace at the `namespace`
    keyword, its note at the first definition's `{`, as clang 19 and
    22). Tests:
    tests/conformance/namespace.def/inline-namespace-reopened-noninline.cpp,
    tests/conformance/namespace.def/inline-namespace-mismatch.cpp,
    tests/conformance/namespace.unnamed/reopened-inline-mismatch.cpp.
- **Using-directives:** the payload is `Payload::None(nominated.raw())`.
- **`extern "C"`/`"C++"`** creates a `LinkageSpecDecl`.
  - Without braces (`extern "C" int x;`) the parser calls
    `act_on_linkage_spec_braceless`, which sets
    `ScopeData::LINKAGE_BRACELESS` in the payload's `aux`.
    `Decls::is_single_line_linkage(d)` is true for the declaration it
    directly contains.
  - [dcl.link]/8: that declaration is treated as if it had `extern`
    when deciding whether it is a definition (`is_var_definition`, the
    redefinition checks in `act_on_var`, codegen's `emit_global_var`).
    The storage class itself stays `None`, so the AST dump shows no
    `extern`, as in clang. `extern "C" int x; extern "C" int x = 1;`
    is a declaration followed by its definition.
  - [dcl.link]/6-7 (`extern_c.rs`): `Sema::extern_c` maps each name
    declared as a C-linkage function or variable (non-template,
    non-local) to its latest declaration. An unqualified function or
    variable declared with C language linkage that finds no
    redeclaration by ordinary lookup links to that entity's redecl
    group, so two `extern "C" void f();` in different namespaces are
    one entity (lookup through both using-directives is then not
    ambiguous). A function whose parameters differ is "conflicting
    types for 'f'" (the note names the first declaration; a
    variable's names the latest, as in clang). A global-scope variable
    and a C-linkage entity of the same name in a namespace conflict
    either way (`err_extern_c_global_conflict` and its note); a
    global function does not.
  - [dcl.link]/6, a redeclaration keeps the first declaration's language
    linkage (`has_incompatible_language_linkage`, clang's
    `haveIncompatibleLanguageLinkages`): the new declaration's innermost
    lexically enclosing linkage specification is looked up through any
    context. `extern "C"` around a redeclaration of an entity whose first
    declaration has C++ language linkage, or `extern "C++"` around one
    whose first declaration is in an `extern "C"` context, is
    `sema_different_language_linkage` ("declaration of 'f' has a
    different language linkage") with "previous declaration/definition
    is here". The redeclaration is not merged (the function or variable
    is invalid). A redeclaration with no linkage specification
    (`extern "C" { void h(); } void h();`) keeps C linkage silently; an
    entity with internal linkage, a class member, a friend and a
    different overload are never checked. For a variable the check
    follows the redefinition check (`extern "C" int w = 1; extern "C++"
    { int w = 2; }` is "redefinition of 'w'"). Not implemented: a first
    declaration at block scope (`void f() { extern void k(); } extern "C"
    void k();`; redeclaration lookup does not find it), and clang's
    "non-static declaration follows static" for a braceless `extern
    "C++" int v;` after `static int v;`.
    Tests: `dcl.link/language-linkage-redecl-errors.cpp`.
  - **Which entities have C language linkage** ([dcl.link]/4-5, WP-492;
    clang `getDeclLanguageLinkage`, `isInExternCContext`):
    `kiln_abi::mangle::is_extern_c` is the single computation (mangling,
    `noundef` on returns, and every sema check above). A function or
    variable has it when its first declaration is lexically inside an
    `extern "C"` specification (the innermost one deciding), walking
    the lexical contexts through namespaces, functions and classes
    (`in_extern_c_context`), and it has external linkage. So a
    block-scope function or `extern` variable declaration in a function
    (or a member function defined in a class) inside `extern "C"`, and
    a friend declared in a class there, name the plain C symbol
    (`void outer() { void lf(int); }` calls `lf`). A class member has
    C++ linkage; an entity with internal linkage has none and is
    mangled as an internal name: `extern "C" { static int sx; }` is
    `_ZL2sx`, `namespace M { static void mf(); }` in it `_ZN1ML2mfEv`,
    and a `const` variable in a braced specification is internal
    (`_ZL2ci`; only a braceless `extern "C" const int c = 1;` is
    external, [dcl.link]/8). A function or variable of an unnamed
    namespace first declared in an `extern "C"` context keeps external
    linkage (clang `getLVForNamespaceScopeDecl`) and its plain name.
    `Sema::extern_c` neither records nor links a `static` declaration,
    so two `extern "C" { static int x; }` in different namespaces are
    distinct entities. Not implemented: the
    `__attribute__((used))` static-in-`extern "C"` aliases (clang
    `EmitStaticExternCAliases`; kiln does not emit `used` statics at
    all yet). Tests: `tests/codegen/extern_c_linkage.cpp`,
    `dcl.link/static-and-local-in-extern-c-run.cpp`.
- **Internal linkage from a `const` type** ([basic.link]/3.2, WP-466):
  a namespace-scope non-template, non-inline variable that is neither
  `extern` nor previously declared with external linkage has internal
  linkage when its type is const-qualified and **not volatile** (for an
  array, its element type's qualifiers, [basic.type.qualifier]/3). A
  `const volatile int cv = 1;` (or array of them) therefore has external
  linkage; `static`, an unnamed namespace, and a const pointer to
  volatile (`volatile int *const p`) stay internal.
  `kiln_abi::mangle::has_internal_linkage` (`internal_by_storage`) is
  the single computation, shared by sema (`extern_c.rs`,
  `unused_file_scope.rs`) and codegen. Tests:
  `basic.link/const-volatile-external-run.cpp`,
  `tests/codegen/const_volatile_linkage.cpp`.
- **Redefinition through an initializer** (clang
  `AddInitializerToDecl`): a namespace-scope variable that was a
  declaration before its initializer (`extern`, or a single-line
  linkage specification) gets "redefinition of 'x'" with "previous
  definition is here" if an earlier redeclaration is a definition. The
  variable becomes invalid and the initializer is dropped.
- **`static_assert`** ([dcl.pre]/10) requires a converted constant
  expression of type `bool`.
  - A failed comparison adds `expression evaluates to 'L op R'` at the
    operator, underlining the whole comparison (clang
    `DiagnoseStaticAssertDetails`). The condition and its conjuncts are
    looked at through parentheses, implicit casts and full-expressions
    (`ExprWithCleanups`, as clang's `IgnoreParenImpCasts` steps through
    every `FullExpr`), so a comparison of a `__builtin_bit_cast`, which
    materializes its operand, gets the note too. A float side prints as
    `ConvertAPValueToString` does, `APFloat::toString` (`1`, `2.5E+10`,
    `+Inf`); a float literal in the requirement text as clang's
    `PrintFloatingLiteral` (spec/ast.md §9.3). A range whose ends come
    from a macro argument ends after the last token, measured where it
    is spelled (WP-410). Test:
    `dcl.pre/static-assert-full-expression-note.cpp`.
  - In a template definition (`in_template()`, or a context flagged
    `DEPENDENT_CTX`) the condition is not evaluated, even when it is not
    value-dependent (P2593R1, a DR; clang's
    `CurContext->isDependentContext()`): `else static_assert(false);` in
    a discarded branch is fine, and the instantiation that keeps it
    fails (libstdc++ ranges:9507; WP-332). Test:
    `dcl.pre/static-assert-false-in-template-definition.cpp`.
  - On failure it reports
    `static assertion failed due to requirement '…'`, with the
    condition printed by `TypeNames::write_expr`, plus the message if
    there is one. A variable template-id prints its explicit arguments
    (`is_v<ND>`, clang `VisitDeclRefExpr`) after its qualifier as
    written (`ns::S::z<char>`, `::ns::f<int *>`): the parser and the
    instantiation of a qualified or `T::template` variable template-id
    record the nested-name-specifier on the `DeclRefExpr`
    (`set_template_id_qualifier` → `ExprArena::set_decl_ref_qualifier`)
    and its begin (the diagnostic's column) at the qualifier. In an
    instantiation the qualifier is substituted: `A::` in the pattern of
    `A` names the specialization (`A<int>::y`), and a substituted
    parameter head prints its canonical replacement without scope
    (`T::y` with `T = ns::S` prints `S::y`, clang's
    `NestedNameSpecifier::print` `SuppressScope`) (WP-302).
  - A concept-id prints as clang's `ConceptReference::print`: its
    qualifier as written (kept through instantiation), the concept's
    name and all its converted arguments, types canonical
    (`!merror<err<why>>` for `!merror<completions>` with a local alias
    `completions`; WP-789). Test:
    `dcl.pre/static-assert-concept-id-requirement.cpp`.
  - The printed names are the substituted ones (clang's
    `FailedBooleanConditionPrinterHelper`, WP-303): a qualified
    `DeclRefExpr` prints its nested-name-specifier with the last type
    component resolved to its class plus all converted arguments,
    canonical, defaults included (`tr<n::A, int>::value` for
    `T::value`, `write_nns_resolved`), then the name, then a variable
    template specialization's canonical arguments; an unqualified one
    prints its explicit arguments canonical (`ok<n::A>(0)`,
    `write_arg_list_canonical`). A call to a function template-id keeps
    the explicit arguments on the resolved `DeclRefExpr`.
    At `-fclang-compat=22` (clang 22, WP-377) the qualifier's class is
    printed fully scoped: every enclosing namespace (`(anonymous
    namespace)::`, inline ones skipped) and class, each class template
    specialization with all its converted arguments, canonical
    (`n::tr<n::A, int>::In::w` for `Y::In::w`;
    `TypeNames::write_converted_scope`, `AstNames::scoped_nns`). Test:
    `dcl.pre/static-assert-substituted-names-clang22.cpp`,
    `static-assert-variable-template-clang22.cpp`.
  - Evaluation notes follow (§8).
  - **User-generated messages** ([dcl.pre]/10-12, P2741, WP-356). A
    message that is not a string literal (spec/parse.md §4) must be an
    object `m` with `m.size()` and `m.data()` (clang
    `Sema::EvaluateAsString`): no such members is
    `sema_static_assert_missing_member_function`, not a class at all
    `sema_static_assert_invalid_message`, and `size()` not convertible
    to `std::size_t` or `data()` not to `const char*` is
    `sema_static_assert_invalid_mem_fn_ret_ty`. A dependent message
    waits for instantiation. When the assertion fails, the message's
    characters are evaluated in the Oven (`Oven::evaluate_chars`, §8):
    `size()` then `data()[0..size)`, read as one evaluation; a message
    that is not a constant expression is
    `sema_static_assert_message_constexpr` plus the failure notes, and
    the text is printed as clang prints a string argument (valid UTF-8
    as is, an unprintable code point as `<U+XXXX>`, a stray byte as
    `<XX>`). The failing assertion's message is evaluated once (in a
    template, on each failing instantiation only), and its diagnostic
    precedes the failed assertion's, as in clang. Wording follows clang
    22 (tests `requires: clang>=22`):
    `dcl.pre/p2741-static-assert-message*.cpp`,
    `dcl.pre/static-assert-message-not-constant-on-failure*.cpp`.
  - **The message of an assertion that holds is an unevaluated
    operand** ([dcl.pre]/12, P2741R3; WP-570). When the converted
    condition is `true`, or the declaration is in a template definition
    (the condition is not evaluated there at all), the message is only
    type-checked (the `size()`/`data()` checks above) and never
    evaluated: a message whose evaluation would throw, read a null
    pointer or call a non-constexpr function is fine, and a consteval
    call in it is not an immediate invocation (the parser builds the
    message in a constant-evaluated context, `parse_constant_expr`).
    This follows the standard and GCC 16.1. **Deviation from clang 19
    and 22**, which evaluate such a message and report
    `sema_static_assert_message_not_constant` as an error by default:
    in kiln that check is `-Winvalid-static-assert-message`, off by
    default (`WarningOff`; `-Werror=invalid-static-assert-message`
    gives clang's behaviour), and the message is evaluated only while
    it is enabled. Tests:
    `dcl.pre/static-assert-message-unevaluated-when-true.cpp` (`oracle:
    gcc`, `xfail(clang)`), `static-assert-message-unevaluated-reflection.cpp`
    (`oracle: gcc`), `p2741-static-assert-message-diags.cpp` (the
    opt-in check).
- **Unevaluated strings with errors** ([lex.string.uneval]/1, WP-1268;
  clang `ParseUnevaluatedStringLiteralExpression`): a string the lexer
  marks `had_error` (an encoding prefix, a ud-suffix, a numeric,
  conditional or unknown escape; spec/lex.md) gives the parser no string
  (`Parser::parse_string_literal(true)` returns `None`), and the
  construct that wanted it is dropped, so no diagnostic cascades from it:
  - `static_assert(c, "...")`: no declaration (no "static assertion
    failed"), the rest up to `;` skipped. The same for a C++26 message
    expression with errors (`static_assert(false, undeclared)`, a
    missing member, no viable call, no literal operator; P2741):
    `Sema::contains_errors` on the message drops it;
  - `[[deprecated("...")]]`, `[[nodiscard("...")]]`,
    `__attribute__((deprecated("...")))` and the other attributes with
    string arguments: the attribute is not attached (no deprecation or
    nodiscard warning at uses);
  - `= delete("...")`: the function is still deleted, without a message
    (a call is "call to deleted function");
  - `extern "\x43" ...`: no `LinkageSpecDecl` (`act_on_start_linkage_spec`
    is not called); the declarations or braced body are parsed in the
    enclosing context.
  Soft errors (`\x` with no digits, value 0) keep the string, so its
  assertion still fails (`a<U+0000>`). `asm("...")` strings are
  ordinary string literals ([dcl.asm]) and accept every escape.
  Tests: `lex.string.uneval/invalid-escape-no-cascade.cpp`,
  `lex.string.uneval/invalid-escape-forms.cpp`,
  `lex.string.uneval/empty-delimited-escape{,-19}.cpp`,
  `dcl.pre/static-assert-invalid-message-no-cascade.cpp`,
  `dcl.pre/static-assert-message-unassigned-code-point.cpp` (the message
  text, spec/diag.md §6.2).

### 4.7 Exception specifications (`except.rs`)
The parser calls these while parsing a function declarator, and stores
the result in `Chunk::Function::exc`; the function type keeps the
specification as written (`TypeNames` prints ` noexcept`,
` noexcept(<expr>)`, ` throw()`), the canonical type folds it to
`Nothrow` or `None`.
- **In a typedef or alias** before C++17 ([except.spec]/2 in C++14,
  WP-747; clang `err_exception_spec_in_typedef`): a function declarator
  chunk of a `typedef` or alias-declaration that has a `noexcept` or
  `throw` specification is an error at the keyword, `exception
  specifications are not allowed in typedefs` / `... in type aliases`,
  once per chunk; the specification is still kept. The parser
  (`parse_function_qualifiers`) emits it, inner chunk first, where clang
  emits a nested declarator's outer chunk first. From C++17 the
  specification is part of the type and is allowed. Tests:
  `tests/conformance/except.spec/exception-spec-in-typedef-*.cpp`.
- **Beyond one level of indirection** before C++17 (C++14
  [except.spec]/2, WP-1102; clang `CheckDistantExceptionSpec`):
  `type_for_declarator_at` checks each pointer, reference, array and
  pointer-to-member chunk it applies (`is_distant_exception_spec`): when
  the type built so far is a pointer or pointer to member (through
  sugar, `exc_pointee`) to a function type with a written specification,
  `exception specifications are not allowed beyond a single level of
  indirection` (`sema_distant_exception_spec`). It is at the
  declarator-id, at the place an abstract declarator's name would go
  (`abstract_at`), and for a pointer to member at its
  nested-name-specifier. The type is still built (no pointer to member
  is formed: `int`) and the declaration is invalid
  (`declarator_distant_exc`, taken by `act_on_declarator` and
  `act_on_param_declarator`); like clang's `Declarator::InvalidType` it
  sticks for the rest of the declaration group
  (`declarator_type_invalid`), whose later declarators are not declared.
  So `void (**j)() throw(int);`, `void (**(*h())())() noexcept;`, an
  array of such pointers, a parameter, a template argument, a
  `new`/`sizeof`/cast type-id are errors, and `void (*fp)() noexcept`,
  `void (*h())() noexcept`, `void (**k)(void pfa() throw(int))` are
  not. From C++17 the specification is part of the type and allowed
  anywhere. Tests: `except.spec/p2-distant-exception-spec-cxx14.cpp`,
  `except.spec/p2-distant-exception-spec-ok-cxx{14,17}.cpp`.
- **`noexcept`** is `ExcSpec::BasicNoexcept`.
- **`act_on_noexcept_spec(e, loc, start)`** ([except.spec]/2): `e` is
  a contextually converted constant expression of type `bool`
  ([expr.const]/12), evaluated with `EvalContext::converted(start)`
  (`Sema::bool_cce`, shared with `explicit(expr)`, spec/sema-class.md
  §4.7).
  The result is `NoexceptExpr { expr: ConstantExpr(e'), value }`.
  Diagnostics point at `start`, the operand's first token, and follow
  clang 19:
  - an integral or unscoped-enumeration value other than 0 or 1:
    `noexcept specifier argument evaluates to 2, which cannot be
    narrowed to type 'bool'` (an error here; clang's is a default-error
    `-Wc++11-narrowing` warning);
  - a type whose conversion to `bool` is not allowed in a converted
    constant expression (pointers, `std::nullptr_t`, floating point):
    `conversion from 'T' to 'bool' is not allowed in a converted
    constant expression`, with `T` as written (`int *const`);
  - a class type converts through its conversion functions (the
    contextual conversion of [conv.general]/4, WP-275), so
    `noexcept(bool_constant<true>())` is `noexcept(true)`; a class type
    without one, or a scoped-enumeration type: `value of type 'T' is not
    implicitly convertible to 'bool'`;
  - a non-constant operand: `noexcept specifier argument is not a
    constant expression`, plus the Oven's notes.

  After an error the specification is `noexcept(false)`, as clang
  recovers. A dependent operand would give `NoexceptValue::Dependent`
  (no templates in M2).
- **`act_on_dynamic_exception_spec(types, ok, loc, rparen)`**:
  `throw()` is `DynamicNone`, accepted silently (clang 19 accepts it
  in C++26 although P1152/C++20 removed it from the grammar). A
  non-empty list is, from C++17 on, `ISO C++17 does not allow dynamic
  exception specifications` (`-Wdynamic-exception-spec`, default error)
  with a `use 'noexcept(false)' instead` note, then `Dynamic(types)`;
  before C++17 it is valid and silent (clang's off-by-default
  `-Wdeprecated-dynamic-exception-spec` is not implemented; WP-709).
  The C++17 error is decided on the types the parser accepted, before
  the checks below drop any (clang `ParseDynamicExceptionSpecification`).
  Template substitution substitutes the type-id-list (WP-709).
- **Listed types** (WP-1052; C++14 [except.spec]/2, clang
  `CheckSpecifiedExceptionType`, `check_specified_exception_type`): an
  array type is adjusted to a pointer, a function type to a pointer to
  function. An rvalue reference is `rvalue reference type 'int &&' is
  not allowed in exception specification`. An incomplete type, or a
  pointer or reference to one other than cv `void *`, is
  `{pointer to |reference to }incomplete type 'Inc' is not allowed in
  exception specification` plus `forward declaration of 'Inc'`, at the
  type's first token. A class being defined, a dependent type and a
  function pointee pass. A failing type is dropped from the list (an
  emptied list is `throw()`).
- **Dependent lists at instantiation** (WP-1052; clang
  `InitFunctionInstantiation`, `SubstExceptionSpec`, `TransformExceptionSpec`).
  From C++11 a list naming a dependent type is substituted when the
  specification is needed, like a dependent `noexcept(expr)` (DR1330,
  `deferred_dynamic_exc_spec`, `lazy_exc_spec`), except in a local
  scope (DR1484). Each substituted type is checked as above
  (`finish_substituted_dynamic_exc`); the first failure makes the
  specification `ExcSpec::None` (clang's `EST_None`), reported at the
  list's `)` (`dyn_exc_rparen`) with `in instantiation of exception
  specification for 'f3<Inc>' requested here`. Under C++98 the list is
  substituted with the declaration; inside template argument deduction
  the substituted list is checked as the specialization's instantiation,
  not as a substitution failure (WP-1137, `as_spec_instantiation`; clang
  `InitFunctionInstantiation` turns the deduction frame into a
  `TemplateInstantiation` one), so a failing type is a hard error with
  `in instantiation of function template specialization 'h<Inc>'
  requested here`. A member template of a
  class specialization whose list was dropped there has none. A list in
  a function type nested in a substituted type (`void (*)() throw(T)`) is
  checked type by type at that function type's first token
  (`templ/subst/types.rs`, clang `TransformExceptionSpec`); a failure
  fails the substitution: a substitution failure in deduction, else an
  invalid declaration (WP-1137).
  From C++17 the dynamic list kept after `-Wdynamic-exception-spec` lives
  only in the written (sugared) function type; the override check
  (`resolved_exc`) and the implicit-specification redeclaration check
  (`exc_of` in excspec.rs) read it with `written_exc_spec`, so they
  behave as before C++17 (WP-1137).
- **GNU `nothrow`** (WP-186, `apply_nothrow_attrs` in `decl_attrs.rs`;
  clang `handleFunctionTypeAttr` `AT_NoThrow`): `__attribute__((nothrow))`
  or `[[gnu::nothrow]]` anywhere in a declaration whose declared type is
  a function type, or a pointer, reference or member pointer to one, is
  a type attribute (not kept on the declaration): with no written
  specification the function type's is `NothrowAttr` (printed
  `__attribute__((nothrow))`, canonically `Nothrow`, so non-throwing,
  `nounwind` in IR and the same type as `noexcept`); `throw(X)` or
  `noexcept(false)` warns `'nothrow' attribute conflicts with exception
  specification; attribute ignored` (`-Wignored-attributes`); a
  non-throwing or unresolved one is unchanged. On another declaration:
  `'nothrow' only applies to function types; type here is 'T'` (when it
  has a type) and `'nothrow' attribute only applies to functions and
  function pointers`, dropped. A redeclaration omitting it gets the
  `-Wmissing-exception-spec` warning (not the error) and an overrider of
  a `nothrow` virtual function that is potentially throwing the
  `-Wmicrosoft-exception-spec` warning `exception specification of
  overriding function is more lax than base version`. Not modelled:
  `'nothrow' attribute takes no arguments` (no attribute checks its
  argument count generically) and GCC's `-Wgcc-compat` position warning.
  Tests: `tests/conformance/except.spec/gnu-nothrow-attribute.cpp`,
  `tests/codegen/gnu_nothrow.cpp`.

**Landed (WP-112).** Member functions, lazy instantiation and the
"needed" points ([except.spec]/13):
- *Complete-class context* ([class.mem.general]/7): a member
  declarator's *noexcept-specifier* is replayed after the class.
  `defer_late_noexcept(f, d)` gives `f`'s type `ExcSpec::Deferred(f)`
  and puts `f` in `unparsed_exc`. The parser brackets the replay with
  `act_on_start_delayed_exception_spec(f) -> bool` (a `this` scope with
  `f`'s cv-qualifiers, [expr.prim.this]/2) and
  `act_on_end_this_scope`. `act_on_exception_spec(f, spec)` then sets
  the written spec on every redeclaration (`set_member_exception_spec`).
  A static `f` gets an unqualified `this` scope too, as in clang, so a
  non-static member named in its spec is found as `this->m`.
  `act_on_exception_spec` then reports the first `CXXThisExpr` in the
  operand: `'this' cannot be [implicitly] used in a static member
  function declaration` (`sema_this_static_member_func`,
  [expr.prim.this]/3; clang `checkThisInStaticMemberFunctionExceptionSpec`;
  WP-280, `class.mem.general/static-member-noexcept-this.cpp`). In a
  class template it is reported at the definition and again in each
  instantiation of the class (`subst::function_decl` reruns the check
  on the pattern's operand; the diagnostic is `per_instantiation`, so
  the definition's report does not swallow it, spec/diag.md §7.4), the
  first per instantiation carrying the `in instantiation of template
  class` note. The specifier itself, instantiated when needed, gets no
  `this` scope for a static member (`instantiate_exception_spec`), so
  its implicit `this` is `invalid use of 'this' outside of a non-static
  member function` under the `in instantiation of exception
  specification` note, as in clang (WP-290,
  `class.mem.general/static-member-noexcept-this-instantiation.cpp`).
  Every implicit object member function gets one: a constructor,
  destructor or conversion function's `noexcept(noexcept(m.f()))` names
  its members as a member function's does (WP-792,
  `except.spec/special-member-noexcept-this-instantiation.cpp`).
  `throw()`, `noexcept` and `noexcept(expr)` all go through this path.
- *Needed*: `exc_spec_needed(f, loc)` runs at every use that marks a
  function referenced, including unevaluated operands such as
  `noexcept(f())`, as clang's `MarkFunctionReferenced` does. A spec
  that is still unparsed gives `exception specification is not
  available until end of class definition`
  (`sema_exception_spec_not_parsed`). Otherwise `resolve_exception_spec(f,
  at)` runs; it now takes the point of instantiation `at`.
  `resolve_exc_for_type(f, loc)` resolves the spec before a
  `DeclRefExpr` or a `&C::m` member pointer takes `f`'s type, because
  the spec is part of the type (clang's `DiagnoseUseOfDecl`).
- *Lazy dependent specs* ([temp.inst]/14, clang `EST_Uninstantiated`):
  when a member of a class template specialization is instantiated and
  its `noexcept(expr)` is value-dependent, and the member is not in a
  still-dependent context (`lazy_exc_spec`; WP-269 below for member and
  friend templates), the member gets `ExcSpec::Deferred(pattern)` and an entry in
  `uninstantiated_exc: FxHashMap<DeclId, DeclId>` (first declaration to
  pattern). `resolve_exc` calls `instantiate_exception_spec(f, poi)`
  first. That function substitutes the pattern's expression in the
  member's context, with its parameters and `this`, under a
  `FrameKind::ExceptionSpec` instantiation frame. Errors carry `in
  instantiation of exception specification for 'f' requested here`
  (`note_template_exception_spec_here`). A failed substitution leaves
  the member potentially throwing (`ExcSpec::None`), as clang does.
  A function whose constraints are not satisfied is never needed
  ([temp.inst]/17, WP-278): the one candidate
  of a call has its constraints checked before its reference is built
  (`finish_overload_ref`, clang's `DiagnoseUseOfDecl` before
  `BuildDeclRefExpr`), and a constrained function found by
  argument-dependent lookup always goes through overload resolution, so
  an unsatisfied one is a non-viable candidate (`no matching function
  for call to 'f'` + `candidate function not viable: constraints not
  satisfied`, as clang's `BuildOverloadedCallExpr`). libc++
  `filter_view`/`join_view`'s hidden `iter_swap` relies on this. Test:
  `tests/conformance/temp.inst/hidden-friend-noexcept-unsatisfied.cpp`.
- *Redeclarations*, overriders more lax than the base, and
  [conv.fctptr] function-pointer conversions use clang's wording. The
  tests are in `tests/conformance/except.spec/`.
- *Overriders* (WP-1017; clang `CheckOverridingFunctionExceptionSpec`).
  A dynamic `throw(T...)` of an overrider is compared type by type with
  the base's (`override_exc_spec_lax`). Each type must be caught by one
  of the base's, by [except.handle]/3 (`handler_can_catch`): the same
  type, an unambiguous public base, a pointer qualification or
  derived-to-base conversion, `void *`, or `std::nullptr_t` for a pointer
  handler. A reference handler to a pointer must be `const`. As in clang
  there is no dependent exemption: a dependent overrider type is caught
  only by an identical base type, so `void k() throw(T);` against a base
  `throw(int)` is an error at the template definition and again in each
  specialization whose `T` is not caught (WP-1052). A deferred written
  list (above) is not an implicit specification.
  The checks clang runs when the overrider is declared (no written
  specification) are reported before the delayed ones, across the whole
  outermost class. Each overridden function is checked, one error per
  base (clang checks every one; the diagnostic opts out of dedup).
  *Implicit members* (WP-957): an implicit destructor or copy or move
  assignment operator overrides like a declared one (the assignment
  operators of a dynamic class are declared eagerly and record their
  overrides, clang's `AddOverriddenMethods`; before WP-957 a call through
  the base dispatched to the base's function). Its computed
  specification is checked against each base: a noexcept(false) one is
  lax against a non-throwing base; before C++17 a computed type list
  (`implicit_thrown_types`: the union of the implicit callees' dynamic
  lists, any type when one is noexcept(false)) is compared type by type
  (`list_allows_more`). The same computed list is compared with a
  redeclaration's dynamic list ([except.spec]/4, WP-1052,
  `check_special_redecl_exc`, `redecl_exc_equiv`): a destructor first
  declared without a specification (`implicit_exc_dtors`, clang
  `hasImplicitExceptionSpec`, which reads the written type) and defined
  with `throw(E1)` must list the same set of adjusted types, else
  `exception specification in declaration does not match previous
  declaration`. A matching one written on only one side is
  `function previously declared with an {explicit|implicit} exception
  specification redeclared with an {implicit|explicit} exception
  specification` (`-Wimplicit-exception-spec-mismatch`, C++11 on).
  An implicit overrider is also held to the
  calling-convention, `final` and return-type rules ([class.virtual]/4,
  /8; WP-1078, WP-1103, `check_implicit_overrider`): clang's `virtual
  function '~D' has different calling convention attributes`
  (`check_override_cc`, clang `CheckOverridingFunctionAttributes`; test
  `dcl.attr.cconv/override-cc-implicit-dtor.cpp`), `declaration of
  'operator=' overrides a 'final' function` and `virtual function
  'operator=' has a different return type`, at the class name, once per
  overridden function, in that order. They run
  inside a `SynthKind::Declaring` context, so the first error of each
  implicit declaration carries `while declaring the implicit copy
  assignment operator for 'D'` (clang `DeclaringSpecialMember`); in a
  class template specialization it precedes the instantiation note.
  *Nested function types* (WP-1078; clang `CheckParamExceptionSpec`,
  C++14 [except.spec]/5, kept by clang in every mode): when the
  overrider's specification passes the subset check (or is
  non-throwing), function types among the return and parameter types
  (through a pointer, reference or pointer to member) must have
  equivalent specifications (`override_nested_exc_spec_error`):
  `exception specifications of return types differ` / `... of argument
  types differ`, at the overrider, no note. Equivalent means both
  non-throwing, both throwing (but no specification and
  `noexcept(false)` differ), or the same set of listed types. Like clang
  19 and 22 (whose loop indexes the source with a constant), each of the
  base's parameter types is compared with the overrider's FIRST
  parameter type; [conv.fctptr] conversions share the helper
  (`param_exc_specs_error`) and the quirk. The specifications compared
  are the written ones (`underlying_fn_proto` walks sugar,
  `sugared_pointee`): from C++17 `void (*)() noexcept(false)` and
  `void (*)()` are one type, so the overrider overrides, yet the two
  differ (WP-1102). Tests:
  `class.virtual/implicit-overrider{,-ok}.cpp`,
  `except.spec/override-nested-exception-spec-{cxx14,cxx17,ok}.cpp`,
  `except.spec/override-nested-exception-spec-noexcept-false-cxx{17,26}.cpp`,
  `class.virtual/exception-spec-dynamic.cpp`,
  `class.virtual/exception-spec-dynamic-pack-nested.cpp`,
  `class.virtual/exception-spec-implicit{,-dynamic}{,-ok}.cpp`,
  `class.virtual/implicit-assignment-override-run.cpp`.
- *Function references* ([dcl.init.ref]/4, [conv.fctptr]). A function
  type `T noexcept` is reference-compatible with `T`; the rule and its
  identity ranking are in spec/sema-conv.md (WP-290). More tests:
  `tests/conformance/dcl.init.ref/function-noexcept-binding.cpp`,
  `function-noexcept-binding-run.cpp`.
- **Invalid function declarations** (WP-288; clang's
  `ActOnFunctionDeclarator`, `MergeFunctionDecl`, `LookupResult::resolveKind`).
  A function declaration is invalid at its source when it has an
  illegal storage class — `mutable` or `register` outside a member
  declaration (namespace or block scope, or an out-of-class member
  definition): error `illegal storage class on function`
  (`sema_illegal_storage_class_on_function`); in a member declaration
  `register` is `storage class specified for a member declaration` and
  `mutable` is `'mutable' cannot be applied to functions`, both leaving
  the member valid — or an invalid parameter (a `void` parameter, or a
  default argument parsed in place that is erroneous: the parameter is
  invalid too). Such a declaration redeclares nothing (no prior
  declaration is looked up for it). `pick_prev_function` prefers the
  newest *valid* corresponding declaration; when only invalid ones
  correspond, the new declaration is invalid without a diagnostic and
  without being compared. A name whose lookup finds only invalid
  functions resolves a call against the last one found (so an
  inviable call is still `no matching function`); a reference to an
  invalid function is an erroneous expression without a diagnostic.
  A function template whose function is invalid (a parameter type that
  names no type) is invalid too (`finish_template_pattern`); an id-expression
  whose lookup finds it beside other declarations ignores it
  (`act_on_id_expression_dn`, clang `resolveKind`), and a call or
  address that reaches it fails deduction with `Tdk::Invalid`, so it is
  no candidate and gets no note (spec/sema-templates.md §5.7). An
  explicit specialization declarator with an invalid parameter matches
  no template and reports nothing more (WP-308). An explicit
  specialization `template <> void f(int);` with nothing named `f`
  visible reports "no function template matches" and declares a plain,
  invalid function `f` (clang, WP-318; spec/sema-templates.md §6.9), so
  a later `f(1)` is quiet.
  A constructor or constructor template with an invalid parameter type
  (`S(u1)`) is invalid (`declare.rs`, `fparams` validity) and, as
  clang's `addedMember`, LookupSpecialMember and
  `ResolveConstructorOverload` ignore invalid constructors, it is not
  user-declared (the class stays an aggregate and keeps its implicit
  default constructor, `implicit.rs` `scan_declared`) and is no
  candidate (`ctor_select.rs` `usable_ctors`, used by constructor
  overload resolution, the implicit conversion sequences and special
  member deletion): `S s{1, 2};` is "excess elements in struct
  initializer" and "no matching constructor" lists no invalid
  constructor. An implicit class template specialization whose pattern
  has an invalid member (a field, variable, function, or a member
  template whose pattern is invalid) is itself invalid, as clang's
  `InstantiateClass` (`templ/class.rs`); its initializations are not
  checked (WP-318,
  `tests/conformance/dcl.init.aggr/invalid-constructor-keeps-aggregate.cpp`).
  Tests: `dcl.stc/function-storage-class.cpp`,
  `dcl.fct.default/invalid-default-arg.cpp`,
  `except.spec/redecl-invalid-decl.cpp`,
  `temp.deduct.general/invalid-template-not-viable.cpp`.
- **Exception specifications on redeclarations** (WP-268,
  `merge_missing_exc_spec`; clang 19's missing-specification path of
  `Sema::CheckEquivalentExceptionSpec`). A function redeclaration
  (including a global `operator new`/`delete`) that writes no
  exception specification where the previous declaration `p` (the
  latest one redeclaration lookup found) has an incompatible one is
  decided by the first matching row:

  | Condition | Result | The redeclaration's type gets |
  |---|---|---|
  | `p` is non-throwing, the entity has C language linkage (its first declaration is in an `extern "C"` context), and `p` is in a system header, has no location, or the first declaration is a builtin (implicit `builtin_id`, or recognized as a `builtin::LIB` library function, §5.4) | accepted silently (C libraries may add the specification: libc++'s `extern "C" double lgamma_r(double, int*);` after glibc's `__THROW` one) | `throw()` |
  | the redeclaration is a replaceable global allocation function ([new.delete], `is_replaceable_global_alloc`) and `p`'s specification is not value-dependent | `ExtWarn` `'f' is missing exception specification 'S'` (`sema_missing_exception_spec_ext`, `-Wmissing-exception-spec`, on by default, suppressed in system headers like any warning) | `p`'s |
  | otherwise | error `'f' is missing exception specification 'S'` (`sema_missing_exception_spec`); the redeclaration is invalid | `p`'s unless value-dependent or deferred |

  `S` is `p`'s specification as written (`noexcept`, `throw()`,
  `noexcept(<expr>)` printed as clang prints it). Both diagnostics take
  a `previous declaration is here` note and clang's fix-it inserting
  ` S` just past the end of the declarator's `)` or, when present, its
  last cv- or ref-qualifier (`Declarator::fn_qual_locs`; none with a
  trailing return type or for a `)` or qualifier from a macro).
  `p` is always valid: an invalid declaration is on no redeclaration
  chain and is never compared (*Invalid function declarations* above;
  WP-288 replaced WP-279's fallback search here), and a redeclaration
  of an invalid `p` (`void e() noexcept = 0; void e();`) is itself
  invalid, silently. "`p`'s" specification is inherited as clang does:
  `noexcept(true)` becomes `noexcept`, `noexcept(false)` none, others
  as written. Only `p`'s location counts, not the redeclaration's: a
  second redeclaration in user code follows the first (now `throw()`,
  not in a system header) and is the error `... 'throw()'`. The
  inherited specification is what `noexcept(f(...))` and codegen's
  `nounwind` see. The global `operator delete` forms never reach the
  table (a deallocation function without a specification is implicitly
  `noexcept`, [except.spec]/11). The function name in these and the
  other redeclaration diagnostics prints word operators as clang does
  (`operator new`). Tests: `tests/conformance/except.spec/redecl-missing-*.cpp`
  (the C-library case with an `-isystem` header in `Inputs/`).
- *Diagnostics*: `TypeNames::aka` gives no `aka` when the written and
  canonical spellings differ only in exception specifications, matching
  clang's `desugarForDiagnostic`. The Oven's `this` trap says `implicit
  use of 'this' pointer ...` for a member named without `this->`.
- Implicit and defaulted special members keep `Deferred(f)` and are
  resolved through `class/special/excspec.rs` (sema-class.md §4.7). The
  `noexcept` operator sees these specs through constructor, destructor
  and member calls (§5.3). `act_on_start_function_def` resolves the spec
  of every non-template definition (clang `ActOnStartOfFunctionDef`,
  WP-431), so an out-of-line destructor nothing uses is still implicitly
  `noexcept` for codegen's terminate scope ([except.spec]/8).

**Landed (WP-269).** Friends, member templates and function template
specializations ([class.mem.general]/7, [except.spec]/13, [temp.inst]/14):
- *Friends*: an unqualified friend function or friend template declared
  in a class has its *noexcept-specifier* saved for the complete-class
  context like a member's (clang 19 does too), so it names members
  declared later (libc++ `move_iterator::iter_move` / `iter_swap` on
  `__current_`). A redeclaration whose spec is not known yet
  (`Deferred`, or a still-uninstantiated one) is matched against the
  previous declaration ignoring the spec, and the pair goes into
  `delayed_exc_equiv` (clang `DelayedEquivalentExceptionSpecChecks`).
  `set_member_exception_spec` then runs `check_delayed_exc_equiv(f)`:
  if the nothrow-ness differs, `exception specification in declaration
  does not match previous declaration` at the friend, noted `previous
  declaration is here`. `set_member_exception_spec` only rewrites
  redeclarations still `Deferred(f)`, so an earlier namespace-scope
  declaration keeps its own spec.
- *Redeclaring a delayed friend* (merged with WP-268's missing-spec
  path): a previous declaration's still-uninstantiated spec is
  instantiated first, as clang does in `CheckEquivalentExceptionSpec`. While
  either spec is still `Deferred`, the redeclaration is never treated as
  missing a spec; the pair is queued and checked when either is
  replayed. A redeclaration written without a spec then takes
  `merge_missing_exc_spec` (`'m' is missing exception specification
  'noexcept(S::x)'`), and otherwise gets the mismatch error.
- *A class template's friend redeclaring a function* (WP-280): when
  the class is instantiated, the friend's lazy spec is instantiated at
  once (`function_decl` in `templ/subst/decls.rs`) and checked against
  the function's *first* declaration, which clang's note names. So a
  later friend is not measured against an earlier friend that was
  already diagnosed as mismatched. Test:
  `temp.friend/friend-redecl-exception-spec-at-instantiation.cpp`.
  At the template definition a non-dependent unqualified friend (no
  template head) is checked too, as clang's `MergeFunctionDecl` does
  for a friend of a dependent context: `pattern_friend_prev` finds the
  namespace function it would redeclare (as for a friend of a
  non-templated class, ignoring other pattern friends) and queues the
  pair on `delayed_exc_equiv`, compared at once or after the replay.
  The friend is not linked into that function's chain. A dependent
  `noexcept(expr)` matches no other specification (clang's
  `CT_Dependent`), so it is a mismatch at the definition and is
  checked again per instantiation. The instantiation's check repeats
  a definition-time report for a `noexcept(expr)` friend
  (`per_instantiation_exc`; clang substitutes and compares it every
  time) and drops it otherwise (diagnostic deduplication, spec/diag.md
  §7.4), as clang does not re-diagnose `noexcept` or a missing
  specification. A pattern friend is invisible to redeclaration
  lookup (`find_prev_function` filters `is_dependent_friend_fn`): a
  later namespace-scope declaration declares the function anew, and
  each instantiated friend then redeclares it and is checked against
  it (WP-290, `temp.friend/friend-exception-spec-pattern-time.cpp`).
- *Unparsed through the redeclaration chain*: `exc_unparsed(f)` is true
  when `f`, its first declaration, or the pattern it waits on is in
  `unparsed_exc`. Both `resolve_exc_for_type` and
  `diagnose_unparsed_exc_spec` use it. A use before the class is
  complete gives the error once (clang 19 gives it twice for a friend
  template, and leaves the specialization unresolved afterwards).
- *Member and friend templates of class template specializations*: the
  redeclared pattern keeps `Deferred` (`lazy_exc_spec(d, p)`: the spec
  is dependent at no more than the template's own level). A friend
  template waits whether it is defined in the class or only declared
  there (WP-287): the class is still incomplete when its friends are
  instantiated, so substituting at once would look up members declared
  after the friend (`no member named '__current_'` for libc++
  `counted_iterator`'s `iter_swap` inside `counted_iterator<counted_iterator<I>>`).
  Its specializations wait on the original pattern
  (`defer_spec_exception_spec`). A specialization of a namespace-scope
  template the friend redeclares keeps that template's spec.
- *Function template specializations* (DR1330, clang
  `InitFunctionInstantiation`): `function_spec_signature` does not
  substitute a dependent `noexcept(expr)` during deduction. Unless the
  pattern is in a local scope, the specialization gets `Deferred(spec)`
  and `uninstantiated_exc[spec] = pat`. `deduce_noexcept` accepts an
  unresolved spec. `substitute_exception_spec` substitutes it later with
  the specialization's full argument levels (`spec_args`,
  `spec_pairs_ctx`), in the lexical class of the specialization's
  template for a friend. Access is therefore checked as the friend: it
  may name its own class's private members, but not those of another
  specialization ([class.friend]/2). The frame note names the
  specialization with its arguments (`'h<long>'`), as clang does.
  Every non-static member function, a constructor, destructor or
  conversion function included, substitutes its spec with `this`
  ([expr.prim.this]/3): `C() noexcept(noexcept(conn(m_)))` calls a
  member through the implicit `this` (WP-789; test
  `except.spec/ctor-noexcept-implicit-this-member-run.cpp`).
- *Consumers*: `noexcept(...)`, the resolution of overloads on a
  noexcept function pointer, member pointers and codegen's `nounwind`
  all go through `exc_spec_needed` / `resolve_exc_for_type`, so each
  sees the resolved spec.
- *Other complete-class contexts* (tests in
  `tests/conformance/class.mem.general/`):
  - A default argument used before the class is complete gives clang's
    `use of default argument to function 'f' that is declared later in
    class 'S'` (sema-class.md §2.7).
  - An NSDMI needed early gives `default member initializer ... needed
    within definition of enclosing class` (existing).
  - A trailing requires-clause is not a complete-class context. A later
    member is undeclared there, and an invalid clause no longer also
    gives `non-templated function cannot have a requires clause`.
- *Divergences from clang*:
  - A local class in a function template gets its members'
    noexcept-specifiers deferred like any class. clang 19 substitutes
    them with the class (`no member 'y' in 'L'; it has not yet been
    instantiated`).

### 4.8 Using-declarations, `using enum`, namespace aliases (`using.rs`)
- **Using-declarators** ([namespace.udecl]):
  `act_on_using_declarator(using_loc, qual, name: DeclName, name_loc,
  if_exists, ellipsis)` handles one declarator of `using a::x, b::y;`.
  The name may be an operator or conversion function name (WP-149).
  A declarator with `...` ([namespace.udecl]/1,
  P0195) is a pack expansion over a dependent qualifier, expanded
  per element when the class template is instantiated
  (`templ/class.rs` `member_using`).
  - The name must be qualified: `using declaration requires a
    qualified name`.
  - The target is looked up with `LookupKind::All` in the qualifier's
    context.
  - Nothing found: `no member named 'x' in namespace 'N'`, unless the
    declaration has `[[clang::using_if_exists]]` (§4.5). Then an
    `UnresolvedUsingIfExists` declaration is introduced in place of a
    target, and only a use of the name is an error: `reference to
    unresolved using declaration`, with a note at the declaration.
  - A namespace target ([namespace.udecl]/3): `using declaration cannot
    refer to a namespace`, with a note suggesting `using namespace`.
  - A class qualifier at namespace or block scope ([namespace.udecl]/8)
    is allowed only for enumerators; otherwise `using declaration
    cannot refer to class member`, with clang's note suggesting an alias
    declaration (types) or a reference (variables).
  - A literal-operator name with a qualifier that is not a namespace (a
    class, an enumeration or a dependent type; [over.literal]/2) reports
    clang's `non-namespace scope 'C::' cannot have a literal operator
    member` at the name, at any scope (clang 22 prints `'C'`; WP-559).
  - On success, a `Using` declaration (not entered in lookup) records
    the qualifier, and one `UsingShadow` per declaration found is entered
    in the current scope (a local scope's `locals`, or the context's
    table).
  - Block scope, operator and literal-operator names (WP-559): the
    shadows of `using N::operator+;` / `using udl::operator""_t;` go to
    the block's `Scope.local_names` (keyed by the full `DeclName`;
    `locals` holds identifiers). Unqualified lookup of that name — a
    ud-suffix's `operator""_t` ([lex.ext]/5, `literal_op_forms`, the
    call), an operator expression's non-member candidates
    ([over.match.oper]/3.2, `lookup_operator_name`) or an
    `operator+(…)` call — stops at the innermost block holding one, from
    the declarator to the end of the block, hiding outer declarations;
    the block's declarators of one name form one overload set (ADL still
    adds its candidates). In a template a non-dependent declarator is
    redone by the instantiation (`subst/stmts.rs` `local_using_decl`).
    A dependent qualifier with an operator name (`using T::operator+;`,
    clang's `UnresolvedUsingValue`, diagnosed at instantiation since
    only enumerators may be named) reports `an operator name with a
    dependent qualifier in a block-scope using-declarator is not
    supported yet`
    (tests/conformance/namespace.udecl/block-scope-using-*.cpp).
  - `using typename q::x;` ([namespace.udecl]/1, /12;
    `act_on_using_typename_declarator`, WP-169): with a dependent
    qualifier in a class member specification, the declarator is an
    `UnresolvedUsingTypename` member `x` (entered in lookup; a type
    name) whose type is its own dependent `UnresolvedUsing` type
    (spec/types.md §5.3, WP-179); at instantiation (`templ/class.rs`
    `member_using_typename`) it is redone as an ordinary
    using-declarator, and the `UnresolvedUsing` type in instantiated
    members resolves to the type of its shadow's target
    (`subst/types.rs` `decl_type`). With a non-dependent qualifier, or
    at instantiation, a target that is not a type reports `'typename'
    keyword used on a non-type` at the name, with the `target of using
    declaration` note (clang's `err_using_typename_non_type`).
  - Dependent using-declarators in templates (WP-179):
    - A class member `using B<T>::m;` is an `UnresolvedUsingValue`
      member (spec/sema-class.md). An unqualified use of `m` in the
      class (`act_on_id_expression_dn`) is a baseless
      `CXXDependentScopeMemberExpr`, also when the class's own
      overloads of `m` join the set (no ambiguity, and the pattern's
      own `f(char)` is not a redefinition of the unresolved `f`,
      `overload/redecl.rs`). The instantiation (`subst/exprs.rs`
      `implicit_dependent_member`) looks the name up again from the
      instantiation context: an implicit member access or a static
      member reference.
    - An `UnresolvedUsingTypename` is a dependent context, so
      `type::value` and `typename type::vt` nest through it.
    - At block scope in a template, a declarator with a dependent
      qualifier (`using E<T>::A;`, [namespace.udecl]/8: an enumerator)
      is a local `UnresolvedUsingValue` (its `using` keyword in
      `cold.begin_loc`); a use is an `UnresolvedLookupExpr` that finds
      it, without ADL (clang's dump). The instantiation redoes the
      declarator (`subst/stmts.rs` `local_using_decl`), so a
      non-enumerator target is diagnosed then, with its note at the
      `using`, and a use (`local_using_ref`) is looked up again in the
      instantiated block. A qualifier error (`T` not a class, or not
      a base class) is reported at the start of the
      nested-name-specifier, as clang does (`Substituter::qualifier_begin`,
      sema-templates.md §5.8); the class-member `using T::member;` and
      `using typename T::type;` (`templ/class.rs`) do the same
      (tests/conformance/namespace.udecl/dependent-qualifier-location.cpp).
      The uses of a declarator that failed
      (`Substituter::failed_usings`) are errors without another
      diagnostic (clang's invalid `UsingDecl`; WP-197).
    - A qualifier that is still dependent after substitution (a
      generic lambda in a function template specialization, or a
      local class in one) leaves the declarator unresolved, rebuilt
      with the substituted qualifier, until the inner specialization
      (`still_dependent_class_using`,
      `still_dependent_using_typename`).
- **Conflicts** ([namespace.udecl]/11, [basic.scope.scope]/4), as in
  clang's `CheckUsingShadowDecl`. Two distinct entities of one name in
  one scope:
  - an unresolved `using_if_exists` target conflicts with nothing;
  - nor does a dependent using-declarator (`using A::f;` in a template
    whose `A` is a dependent base): `using A::f; using P::f;` is valid
    in the definition and checked in each instantiation (WP-811);
  - in a class, a dependent member using-declarator (`using B<T>::f;`,
    an `UnresolvedUsingValue`) does not conflict with a function or
    function template a later using-declarator introduces: it is
    assumed to name functions that overload, and a conflict is found at
    instantiation (clang `CheckOverload`; Boost.Cobalt's promises, WP-812,
    `namespace.udecl/dependent-member-using-overloads-run.cpp`);
  - a tag conflicts only with a tag;
  - two typedefs conflict unless they name the same type;
  - two functions conflict iff their types are the same ignoring
    exception specifications (else they overload);
  - anything else conflicts.
  - A new shadow that conflicts reports `target of using declaration
    conflicts with declaration already in scope`, with notes at the
    target and the conflicting declaration; no shadow is made.
  - A later declaration that conflicts with a shadow
    (`check_using_shadow_conflict`, from variable, function, typedef
    and tag declarations, including operator, literal-operator and
    conversion function names, WP-146) reports `declaration conflicts with
    target of using declaration already in scope`, and is marked invalid;
    in a class a member function instead hides the shadow
    ([namespace.udecl]/14, `Decls::hidden_shadow`).
  - **Member hiding** ([namespace.udecl]/14, WP-612). In a class, a
    member function or member function template and a base member
    function (template) a using-declarator introduces never conflict:
    the member hides the introduced one exactly when both are templates
    or both are not and they *correspond under the member
    using-declaration rules* (`member_hides_using_target`, clang's
    `IsOverload` with `UseMemberUsingDeclRules`): the same
    parameter-type-list, the same cv- and ref-qualifiers of a
    non-static member, equivalent trailing requires-clauses and, for
    templates, equivalent template heads ([temp.over.link]/6). The return
    type takes no part, for templates either (`M& operator=(int)` hides a
    using-declared `B& operator=(int)`; clang 19 also hides a template
    differing only in its return type, gcc does not). Otherwise the two
    overload and both are candidates. A member template is compared as
    the `FunctionTemplate`, not its pattern. Either order works: a member
    declared after the using-declarator marks the shadow hidden
    (`check_using_shadow_conflict`, from ordinary member declarators and
    from special-member ones: `operator=`, conversion functions); one
    declared before it keeps the shadow from being made
    (`introduce_shadow`). In an instantiated class template
    specialization the using-declaration and the members are redone in
    declaration order, so the comparison is between the instantiated
    declarations (`D&` and `Map&` agree once `D` is `Map<T>`; the
    `Map`/`MapBase`/`MatrixBase` `operator=` chain, including
    `using ReadOnlyMapBase::Base::operator=;` naming an indirect base).
    Known gap: a constrained member template of a class template
    specialization keeps its pattern's constraints (`member_constraints`),
    so it is not found equivalent to a constrained template of a
    non-template class (or one at a different template depth); both then
    stay candidates. Tests:
    `tests/conformance/namespace.udecl/hide-*.cpp`.
  - An unqualified friend function (template) in a class of a namespace
    with a using-declarator of that name does not redeclare the shadow
    (redeclaration lookup skips it): a friend that does not correspond to
    the target overloads it as a hidden friend; one that corresponds
    ([basic.scope.scope]/4, `fn_corresponds`) would redeclare the target
    and reports `cannot befriend target of using declaration` with notes
    at the target and the using-declarator, and is marked invalid
    (clang `err_using_decl_friend`; `friend_befriends_using_target`). In
    a templated class a friend that is neither a template nor of
    dependent type is checked at definition, any other when instantiated
    (WP-273; `namespace.udecl/hidden-friend-overloads-using-target.cpp`,
    `friend-befriends-using-target.cpp`,
    `libstdcxx-hidden-friend-vs-using.cpp`).
  - Repeating a using-declaration of the same entity is allowed; the new
    shadow redeclares the old one.
- **Class scope:** kiln has no base classes yet (M3), so every
  using-declarator in a class is ill-formed, and Sema reports clang's
  diagnostic for the qualifier: `using declaration in class refers into
  'N::', which is not a class`, `using declaration refers to its own
  class`, or `using declaration refers into 'C::', which is not a base
  class of 'D'`. A declarator qualified by an
  enumeration works at class scope as anywhere else.
  - (WP-724) [namespace.udecl]/10: a using-declarator in a class
    repeats an earlier one, "redeclaration of using declaration" with
    "previous using declaration", only when both qualifiers name the
    same class (`previous_class_using`, clang
    `CheckUsingDeclRedeclaration`). Through another class (`using
    A::f; using B::f;` with `B` derived from `A`) a member the class
    already has a shadow of gets no second shadow and no diagnostic
    (`introduce_shadow`, clang `CheckUsingShadowDecl`;
    `namespace.udecl/p10-*.cpp`).
  - (WP-724) [class.qual]/2.2: the declarator names the constructors
    of the class `C` its qualifier nominates (an inheriting
    using-declaration) when the name is the qualifier's last spelled
    component (`using B::B;`) or `C`'s own name, its
    injected-class-name, however the qualifier is spelled (`using
    BC::B;` with `typedef B<char> BC`, `using std::regex::basic_regex;`;
    `class_using_declarator`;
    `class.qual/p2-injected-class-name-after-typedef-run.cpp`).
- **`using enum`** ([enum.udecl]): `act_on_using_enum(qual, class, name,
  name_loc)` takes the parser's classification of the (possibly
  qualified) name.
  - Not a type: `unknown type name E`; a type but not an enumeration:
    `'X' is not an enumerated type`. A missing name is a parse error:
    `using enum requires an enum or typedef name`.
  - An incomplete enumeration ([enum.udecl]/2): `enumeration 'E' is
    incomplete`, with a note; the `UsingEnum` declaration is made and marked invalid.
  - A dependent type ([enum.udecl]/1: the declarator designates a
    non-dependent type), such as a member enumeration of the current
    instantiation or a local enumeration of a function template:
    `using-enum cannot name a dependent type`, and no declaration is made.
  - Otherwise a `UsingEnum` declaration names the definition, and each
    enumerator gets a `UsingShadow`, with the conflict checks above
    (`using_enum_of`).
  - At block scope in a template (WP-323), the instantiation maps the
    named enumeration through `find_instantiated_decl` (it stays itself
    unless it is a local or member of the pattern) and redoes
    `using_enum_of` in the instantiation's block scope, so the
    enumerators are visible in the instantiated block (clang
    `TemplateDeclInstantiator::VisitUsingEnumDecl`). Tests:
    `tests/conformance/enum.udecl/libstdcxx-local-using-enum-template.cpp`,
    `tests/conformance/enum.udecl/libstdcxx-using-enum-dependent.cpp`.
- **Namespace aliases** ([namespace.alias]):
  `act_on_namespace_alias(name, name_loc, qual, target, target_loc)`.
  - The target is looked up with `LookupKind::Nested` and must be a
    namespace or alias: `expected namespace name`.
  - `lookup_namespace_name(qual, name)` gives that lookup. A
    using-directive whose name does not classify as a namespace falls
    back to it, so in both forms only namespaces are considered
    ([namespace.udir]/1, [namespace.alias]/1): `using namespace index;`
    with a visible function `::index` nominates the namespace (WP-801,
    `tests/conformance/namespace.udir/p1-function-does-not-hide-namespace-run.cpp`).
  - Redeclaring an alias to the same namespace is allowed (it links to
    the previous one); to a different namespace: `redefinition of 'Q' as
    an alias for a different namespace`, with a note; any other earlier
    declaration of the name: `redefinition of 'Q'` (a namespace) or
    `redefinition of 'Q' as different kind of symbol`.
  - The alias is entered in lookup; at block scope, in `locals`.
- **Nested-name-specifiers:** `nns_global` and
  `nns_component(prefix, resolved, spelled)` build the `NnsId` the
  `Using` declaration records, keeping an alias as written
  (`Nns::NamespaceAlias`). `Sema::nns_str` prints one as written.
- **Uses:** a `DeclRefExpr` found through a shadow records it as its
  found declaration (`build_decl_ref_via`; `DeclRefExtra::found_decl`).

### 4.9 Structured bindings (`struct_bind.rs`)

**Landed (WP-122).** [dcl.struct.bind] for simple-declarations,
conditions, range-for loop variables, namespace scope (`static`,
`thread_local` and plain) and block scope, matching clang 19's AST,
diagnostics and IR.

- **Representation** (spec/ast.md §4). The decomposed object `e` is a
  `DeclKind::Var` with an empty name, located at `[` (clang's
  `DecompositionDecl`). Each name is a `DeclKind::Binding` declared
  before it. The links are write-once cold maps:
  - `set_decomp_bindings(v, list, bindings)` records the bindings, read
    back with `decomp_bindings(v)`, `is_decomposition(v)` and
    `binding_decomp(b)`;
  - `set_binding(b, expr, holding)` records the binding expression
    (`binding_expr(b)`) and, for a tuple-like type, the holding
    variable (`binding_var(b)`, built with `builder.holding_var`).

  `decl_name(v)` prints `[a, b]`, as clang's `printName` does.
- **Declaring.** The parser hands a declarator with a decomposition to
  `act_on_decomposition_declarator`:
  - In a template-declaration it is `decomposition declaration template
    not supported`.
  - In a condition (`DeclaratorContext::Condition`, not an
    init-statement) it warns `ISO C++17 does not permit structured
    binding declaration in a condition` (`-Wbinding-in-condition`) at
    `[` before C++26 (clang 19's wording; clang 22 calls it a C++2c
    extension). In C++26 it is valid and silent ([stmt.pre]/4, P0963,
    WP-356). The condition's value is the decomposed object `e`
    contextually converted to `bool`, and it is evaluated **before** the
    bindings are initialized: codegen (`emit_cond_var_decl`, then the
    condition, then `emit_binding_vars`, clang
    `MaybeEmitDeferredVarDeclInit`) and the Oven (`cond_bindings`) emit
    a tuple-like binding's holding variable (its `get` call) after the
    test, for `if`, `while`, `for` and `switch`, whatever the outcome.
    A `switch` condition is `e` converted to an integral or enumeration
    type by its conversion function (the class-type rule under
    `switch` in §7, WP-364), likewise before the bindings. Tests:
    `dcl.struct.bind/p0963-binding-condition*.cpp`,
    `dcl.struct.bind/p0963-switch-condition.cpp`.
  - Only `static`, `thread_local`, `auto` and cv are allowed
    ([dcl.pre]/6), plus `constexpr` and `constinit` in C++26 (P2686R5,
    WP-454; gated on `-std`, not on `-fclang-compat`). Another storage
    class, `consteval`, `inline`, or `constexpr`/`constinit` before
    C++26 is `structured binding declaration cannot be declared
    'constexpr'`, one error per such specifier at its location (the
    storage class, then `inline`, then the constexpr kind; clang 22,
    WP-468). At `-fclang-compat` < 22 it is clang 19's single
    `decomposition declaration cannot be declared 'constexpr'`, or `...
    with 'register constexpr' specifiers` when there are several, at the
    first one. Clang 22 implements P2686R5 and, at `-std=c++23`, gives this
    error rather than an extension warning; kiln follows it. At block
    scope an `inline` is also `inline declaration of 'a' not allowed in
    block scope` at `inline`, naming the first binding: [dcl.inline]'s
    rule for every block-scope variable (`var_spec`, clang
    `err_inline_declaration_block_scope`; the variable is not inline),
    both compat levels. Tests: `dcl.struct.bind/specifiers-each*.cpp`,
    `dcl.inline/block-scope.cpp`. A
    `consteval` is also `consteval can only be used in function
    declarations` (`sema_consteval_wrong_decl_kind`). The declaration
    continues: a `constexpr` is still applied, which makes `e` const,
    as in clang.
  - **P2686R5.** A `constexpr` decomposition is a constexpr variable
    `e`: its initializer must be a constant expression
    (`check_constexpr_init`, the same diagnostics and notes as any
    `constexpr` variable, including the element-wise copy of an array
    `e` in `init_decomposition_array`), and its bindings are then usable
    in constant expressions ([expr.const]/4, the binding expression
    refers to `e`). A tuple-like type's holding variables are
    `constexpr` too, so each `get` call is constant-evaluated at the
    declaration. `constinit` is recorded on `e` (`record_constinit`)
    and checked like any `constinit` variable. `static constexpr`,
    block scope, namespace scope, templates and the P1061 pack form
    compose unchanged. Tests: `dcl.struct.bind/p2686-*.cpp`,
    `dcl.struct.bind/specifiers-constexpr.cpp` (c++23).
  - Before C++20 (P1091R3, [dcl.pre]/6) `static` and `thread_local`
    (`__thread`, `_Thread_local`) are an extension:
    `structured binding declaration declared 'static' is a C++20
    extension` [-Wc++20-extensions] at each such specifier, before the
    error for a specifier not allowed (clang 22). At
    `-fclang-compat` < 22 it is clang 19's single `decomposition
    declaration declared 'static'` / `... with 'static thread_local'
    specifiers is a C++20 extension` at the first one, and only when
    there is no specifier not allowed (`decomp_cxx20_specifiers`, WP-458;
    `dcl.struct.bind/static-thread-local-*.cpp`).
  - A declarator chunk other than `&`/`&&` is `decomposition declaration
    cannot be declared with parentheses` or `... with type 'T'; declared
    type must be 'auto' or reference to 'auto'`.
  - A constrained placeholder is `decomposition declaration cannot be
    declared with constrained 'auto'`.

  `declare_decomposition` then declares the bindings: redefinitions are
  checked against earlier names and binding attributes are attached. A
  pack `...a` (P1061) is checked first (`check_binding_packs`: only in a
  template, at most one) and then decomposed at instantiation
  (`expand_binding_pack`; spec/sema-templates.md §7.1). It then declares
  `e`. A second declarator in the same
  group is `decomposition declaration must be the only declaration in
  its group` (`act_on_decomposition_not_alone`). A missing initializer
  is `decomposition declaration '[a, b]' requires an initializer`
  (`decomposition_requires_init`).
- **Initializing** (`act_on_init`):
  - `auto [a, b] = arr;` with a plain `cv auto` and an array initializer
    deduces `cv A`. It initializes element-wise with an
    `ArrayInitLoopExpr` (`decomp_array_type`, `init_decomposition_array`,
    clang's `TryArrayCopy`).
  - Otherwise `e` is deduced and initialized as an ordinary variable.
    `auto [a, b]{x}` is direct-list-initialization, so `e` deduces the
    type of `x`; instantiation (`decomp_declare`) keeps the pattern's
    initialization style for that (WP-336).

  `complete_decomposition` (clang's
  `CheckCompleteDecompositionDeclaration`) then gives every binding a
  type and an expression. An invalid `e` or an error initializer
  invalidates the bindings silently. A dependent `e` gives them
  `QualType::DEPENDENT` until instantiation.
  - **Arrays** (`decompose_array`): `e[i]` (an `ArraySubscriptExpr` on a
    `DeclRefExpr` to `e`), of the element type with `e`'s cv.
  - **Tuple-like** (`tuple_like`, `decompose_tuple`): applies when
    `std::tuple_size<E>` is a complete class.
    - A `value` that is not an integral constant expression is `cannot
      decompose this type; 'std::tuple_size<T>::value' is not a valid
      integral constant expression`.
    - Each binding gets a holding variable of type `tuple_element<i,
      E>::type&` (lvalue `e`) or `&&`, with the binding's name. The
      index `i` of `tuple_element` is a `size_t` value argument or, at
      `-fclang-compat=22`, a `size_t` integer literal written as an
      expression argument (`tuple_element<0UL, E>`; §1.1). At both
      levels it is converted to the type of the template's first
      parameter ([temp.arg.nontype]/2; at compat 19 `tuple_element_type`
      runs `check_template_args`, WP-914), so `template <unsigned I,
      class T> struct tuple_element` with an explicit specialization for
      `0` is matched (`dcl.struct.bind/tuple-element-unsigned-index-19.cpp`,
      `tuple-element-unsigned-char-index{,-19}.cpp`). It is
      initialized with `e.get<i>()` when a member template `get` has a
      non-type first parameter, else with an ADL-only `get<i>(xvalue
      e)`, which prints `adl` and, being kiln's call rather than a
      written template-id, is no C++17 `-Wc++20-extensions` "no prior
      declaration" warning (`ovl.binding_get`; WP-683). The `get` index is always a `size_t`
      integer literal written as an expression argument (clang
      `getTrivialIntegralTemplateArgument`), so it is converted to the
      type of each candidate's template parameter
      ([temp.arg.nontype]/2): `template <int I> get` is specialized and
      mangled with an `int` argument (`ILi0E`), not `size_t` (WP-467;
      `tests/codegen/struct_bind_get_mangling.cpp`).
    - The binding's type is `tuple_element<i, E>::type`, as written,
      and its expression a `DeclRefExpr` to the holding variable.
    - `tuple_element` is checked after the `get` call. An incomplete
      specialization gets clang's `implicit instantiation of undefined
      template 'std::tuple_element<0, T>'` (`std_trait_ty(..,
      complain = true)`). A specialization without `type` is `cannot
      decompose this type; 'std::tuple_element<0>::type' does not name a
      type`.
    - A failed `get` call that had candidates is recovered as clang does
      (a `RecoveryExpr`), and the next binding is still checked.
      An empty `get` lookup (`use of undeclared identifier 'get'`) or a
      failed initialization stops at that binding.
    - The holding variable's initialization runs in a
      `FrameKind::BindingInit` frame, whose note is `in implicit
      initialization of binding declaration 'a'`.
  - **Classes** (`decomposable_class`, `decompose_members`): the bindings
    are `e.mi` (a `MemberExpr`) for the non-static data members of the
    one class in the hierarchy that has any. The member's type gets
    `e`'s cv, with `mutable` dropping const.
    - Fields spread over several classes are `cannot decompose class type
      'D': both it and its base class 'B' have non-static data members`
      (or `its base classes 'A' and 'B'`).
    - An ambiguous base is `cannot decompose members of ambiguous base
      class 'B' of 'D'`.
    - A lambda closure is `cannot decompose lambda closure type`.
    - An anonymous struct or union member is `cannot decompose class
      type 'S' because it has an anonymous struct|union member`, with a
      note.
    - An inaccessible member is `cannot decompose private|protected
      member 'x' of 'S'` at the binding, through the usual access
      check.
    - A union or a non-class type is `cannot decompose union|non-class,
      non-array type 'T'`.
    - A bit-field member makes the binding a bit-field lvalue. Every
      `DeclRefExpr` to it is marked `bitfield` (`mark_bitfield`, clang's
      `refersToBitField`), so `int &r = b;` is `non-const reference
      cannot bind to bit-field 'b'`.
  - A binding count that differs from the element count is `type 'T'
    decomposes into N elements, but M names were provided` (`1 name
    was`, `only 1 name was`).
- **Uses.** A `DeclRefExpr` to a binding is an lvalue of the binding's
  type, kind `Binding`. A binding is captured by a lambda as the entity
  it names ([expr.prim.lambda.capture]/12, P1091R3): `has_local_storage`
  and `may_need_capture` follow `binding_decomp`. Codegen loads a
  captured binding through the closure field. Reference binding
  (`conv::bind_reference`) looks through a binding to its expression for
  the bit-field check.
- **Templates.** `decomp_written` records each pattern's declared type.
  `templ/subst/decls.rs` re-declares the bindings and `e` from it, then
  completes the decomposition. A range-for loop variable and a
  condition are re-declared in `templ/subst/stmts.rs`. Binding
  attributes are not instantiated.
- **Constant evaluation and codegen.** The Oven and codegen treat `e`
  as an ordinary variable, followed by the holding variables
  (`kiln_oven` `holding_vars`, codegen `emit_var_decl`). A use of a
  binding evaluates or emits its binding expression. A namespace-scope
  `e` mangles as `_ZDC1a1bE` (`kiln_abi` `mangle_decomp.rs`, `DC
  <source-name>+ E`, never `L`-prefixed). Constant bindings come from
  a `constexpr` `e` (C++26, P2686R5) or, as before, a `const` `e` with
  a constant initializer that is usable in constant expressions; the
  Oven needs no new rule, since
  usability is that of `e` and the holding variables.
- **Constexpr-referenceable automatic objects (C++26, P2686R5, WP-467).**
  An object of automatic storage duration is constexpr-referenceable from
  a point P when its variable (that of its complete object) and P have the
  same smallest enclosing non-block scope and that scope is a function
  parameter scope ([expr.const.init]/2; a lambda body is a scope of its
  own). So at `-std=c++26`, `constexpr int& r = local;`, `constexpr int* p
  = &parm;`, `constexpr auto& [a, b] = local_constexpr_struct;` and the
  tuple-like form (`get<i>` of the local) are constant-initialized; a
  `static`/`thread_local` one is not ([expr.const.init]/4: it must be
  representable at namespace scope), nor is a pointer escaping its
  function (`return &k;`). The rule is the Oven's pointer freeze
  (spec/oven.md "Freezing"): the variable being initialized and the
  referent's variable are both locals of the same function, and the
  variable being initialized is declared `constexpr`. Usability is
  then scoped ([expr.const]/8-9): a variable whose frozen value designates
  an automatic object (`kiln_oven::automatic_referent`, cached per
  variable in `Sema::auto_referent`) is usable in constant expressions
  only where `enclosing_function(ctx)` is that object's function
  (`Sema::representable_here`), so in a lambda body naming such a
  reference or pointer is an odr-use and it is captured. Deliberate
  narrowing (partial: constexpr declarations only): by
  [expr.const.init]/2 a non-`constexpr` `auto& e = local;` or `auto* p =
  &local;` would be constant-initialized too, its reference usable in
  constant expressions and its names `non_odr_use_constant`; kiln does not
  do that yet, so it stays exactly as clang 22 (not constant-initialized,
  odr-used, captured; the AST tests `dcl.struct.bind/{array,bitfield,
  tuple-adl-get}.cpp` run at the default std against clang). That case
  waits for an oracle: neither clang 22 nor GCC 16 implements P2686's
  automatic references (clang 22 only the structured-binding part), so the
  tests of the constexpr cases are `skip(clang)`. A value read through
  such a reference still needs a constant referent ([expr.const]/10:
  `constexpr int& r = i; static_assert(r == 3);` reads non-const `i`).
  Codegen does not fold an initializer or load whose value designates an
  automatic object (spec/codegen.md `fold.rs`). Before C++26 the freeze
  rejects as before. Validation: `tests/conformance/expr.const/constexpr-ref-automatic{,-errors,-run}.cpp`,
  `tests/conformance/dcl.struct.bind/constexpr-ref-automatic.cpp`.
- **Lifetime-extended temporaries of a constexpr local reference (C++26,
  P2686R5, WP-1122).** A temporary bound to a `constexpr` reference (or
  `constexpr` structured binding, or a member of an aggregate initializer)
  declared in a function body is an automatic object whose lifetime is that
  of the reference ([class.temporary]/6), and it is constexpr-referenceable
  from the reference's own scope, so `constexpr const int& r = 42;`,
  `constexpr const S& s = S{1, 2};`, `constexpr const auto& [x, y] = S{3,
  4};` and `constexpr const int (&a)[2] = {5, 6};` in a function are
  constant-initialized. The Oven allocates the variable's automatic
  lifetime-extended temporaries as `Origin::Let` objects alongside its
  static ones (`begin_static_extended`, `compile/class.rs`), so the frozen
  reference is a `PtrBase::Temporary`; `automatic_referent` counts a
  temporary whose extending variable is a local (`value.rs`), so the same
  scoping as above applies (a lambda naming `r` captures it) and codegen
  emits the temporary on the stack, not as a `_ZGR` global. A value read
  through the reference is usable only when the temporary is
  const-qualified ([expr.const]/4: `constexpr int&& m = 3;` is
  constant-initialized, but reading `m` in a constant expression is
  "read of temporary is not allowed in a constant expression outside the
  expression that created the temporary", as clang says at namespace
  scope). A non-`constexpr` `const int& a = 1;` stays not
  constant-initialized (the §4.9 narrowing), and before C++26 the freeze
  rejects as before (`reference to temporary is not a constant
  expression`). Validation:
  `tests/conformance/expr.const/constexpr-ref-automatic-temporary{,-errors,-run,-cxx23}.cpp`
  (all but `-cxx23` are `skip(clang)`: neither clang 22 nor GCC 16
  implements it).
- **Fixed on the way.**
  - A reference that cannot bind (to `void`, or to a non-class type
    with no conversion) is `reference to type 'int' could not bind to
    an rvalue|lvalue of type 'void'` (`sema_reference_bind_failed`, the
    referee as written, clang's `FK_ReferenceInitFailed`).
  - `implicit instantiation of undefined template` names the
    specialization fully qualified (`Sema::decl_ty_str`, clang's
    printing of `getTypeDeclType`).
- **Left.**
  - Instantiating binding attributes.
  - The NoOp `'const M'` cast clang adds when a const class `e` is
    copy-initialized from a prvalue.
  - Clang 19 rejects a capture of a binding by a lambda in a constant
    expression, and kiln accepts it.

### 4.10 Static thread-local storage (`static_tls.rs`, WP-396)

[basic.stc.thread], [dcl.stc]/4; clang `TLS_Static`. `DeclSpec::thread_static`
records a `__thread` or `_Thread_local` spelling, and `var_spec` then gives
the variable `TlsKind::Static`. `thread_local` gives `TlsKind::Dynamic`.
Codegen gives only dynamic TLS a `_ZTW` wrapper and `_ZTH` initializer
(spec/codegen.md §7p). For every *definition* of a static TLS variable,
`check_static_tls` (clang `CheckCompleteVariableDeclaration`) runs after
the initializer, or after default initialization, and before the
`constinit` check:

- A non-dependent, non-reference type that is destructed is
  `type of thread-local variable has non-trivial destruction` at the
  variable.
- Otherwise the initializer is walked by `static_init_culprit` (clang
  `Expr::isConstantInitializer`), and the first part that fails is
  `initializer for thread-local variable must be a constant expression`
  at that part. The walk looks through:
  - parentheses, cleanups, temporaries, `__extension__` and
    substituted non-type template parameters;
  - trivial constructions of a trivially destructible class;
  - value-preserving casts (no-op, to-union, constructor conversion,
    atomic, null-to-pointer);
  - the elements of an array or record list. A glvalue element binds a
    reference, so it must fold as an address.

  An empty list passes. Anything else must fold with the lax Oven entry
  (`Sema::fold_rvalue`, clang `isEvaluatable`). A reference's
  initializer must fold as an address, which is done by folding a
  synthetic `&e`.
- Both diagnostics carry `note: use 'thread_local' to allow this` at the
  variable from C++11 on. They are emitted `per_instantiation`, because
  clang repeats them at every instantiation (an exception to spec/diag.md
  §7.4).
- An instantiation-dependent initializer is checked when it is
  instantiated, as is the destructor of a variable whose type is
  dependent.
- **Compat 19** (`clang_compat < 20`):
  - clang 19 also rejects an instantiation-dependent initializer in the
    template definition, at the initializer, unless it is a braced list.
  - It does not look through a scalar's one-element list, so
    `__thread int x{f()};` is reported at the `{`.
  - A one-expression parenthesized initializer of a dependent type, or
    whose expression is type-dependent, is clang 19's `ParenListExpr`,
    reported at its `(` (`static __thread T w(T(1));`, `int w(T::v)`;
    WP-411). kiln keeps a `ParenListExpr` only for a longer list, so
    `act_on_init` passes the `(` to `check_static_tls`. A value-dependent
    expression of a non-dependent type (`int w(sizeof(T))`) is reported
    at the expression.
- Redeclarations (clang `MergeVarDecl`, `check_tls_redecl`): a
  redeclaration whose thread storage kind differs from the previous one
  is reported before any redefinition check. The texts are
  `thread-local declaration of 'x' follows non-thread-local declaration`,
  the reverse, and `thread-local declaration of 'x' with
  static/dynamic initialization follows declaration with
  dynamic/static initialization`. The note is `previous definition is
  here` or `previous declaration is here`. The declaration is still
  merged. This applies at namespace scope and to out-of-class static data
  members.
- Tests: `basic.stc.thread/static-tls-init.cpp`, `tls-redecl.cpp`,
  `static-tls-template.cpp`, `static-tls-dependent.cpp` and
  `static-tls-paren.cpp` (clang>=22), `static-tls-dependent-19.cpp` and
  `static-tls-paren-19.cpp` (clang<22).

## 5. Expressions (`expr.rs`, `conv/`)
- **Typed nodes:** each `act_on_*` builds clang's typed node, with
  implicit conversions spelled out as `ImplicitCastExpr`
  (sema-conv.md).
- **Errors:** an invalid operand produces an error expression
  (`act_on_error_expr`, tested with `is_error_expr`). Its consumers
  stay silent, so one mistake gives one diagnostic.
- **Supported expressions:**
  - **names:** DeclRefExpr, enumerators, functions with
    function-to-pointer decay
  - **literals:** via `kiln_sema::literal`; user-defined literals
    call literal operators (sema-overload.md §6.7); when no form
    matches, clang's `no matching literal operator` is reported
  - **operators:**
    - unary and binary operators, including compound assignment (its
      computation type promotes the left operand's type, bit-fields
      by width, and an unscoped-enumeration left operand is then an
      error: sema-conv.md §6). `b += p` with a `bool b` and a pointer
      (or array) `p` is valid ([expr.ass]/6, [conv.bool]): its
      computation types are `int` and `p`'s pointer type, and the sum
      converts back to `bool`. No other left type takes a pointer
      (WP-787, `tests/conformance/expr.ass/bool-add-assign-pointer-run.cpp`).
      The Oven rejects it as "subexpression not valid", as clang does.
    - `?:`, the comma operator
    - increment and decrement (of `bool`, or `_Atomic(bool)`: `++` is
      clang's DefaultError `-Wincrement-bool` extension and is then
      accepted, `--` is always an error; WP-165)
  - **calls:** argument conversion, arity notes, variadic default
    promotions. A call returning a non-reference `cv T` is a prvalue of
    type `T` only when `T` is a non-class, non-dependent type; a class
    (or dependent) result keeps its cv-qualifiers, so `decltype(f())`
    for `const X f()` is `const X` ([expr.type]/2, clang
    `getNonLValueExprType`; WP-276). The callee must be a function
    lvalue or a pointer to function ([expr.call]/1): a pointer to
    member function is not callable without an object (`(o.*pmf)(a)`),
    so `pmf(a)` is `called object type 'P' (aka 'int (S::*)(int)') is
    not a function or function pointer` (a substitution failure in a
    requires-expression: libc++'s `invocable<int (S::*)(int), int>` is
    false; WP-335)
  - **member access:** `.` and `->` on records and through
    `IndirectField`; members found in base classes, static members,
    enumerators and member functions, implicit `this->`, `this`,
    `.*`/`->*` and `&C::m`, all access-checked (sema-class.md §2.4–§2.6,
    WP-066b)
  - **member calls:** `CXXMemberCallExpr` for a single candidate
    (sema-class.md §8.3, "Landed (WP-066b)"); overloaded member
    functions wait for WP-068a
  - **ext-vector element access:** §5.19
  - **subscripts:** either operand order; a pointer to function as the
    pointer operand is "subscript of pointer to function type 'void ()'"
    at the pointer operand's begin (clang `CreateBuiltinArraySubscriptExpr`,
    WP-408, test `expr.sub/subscript-function-pointer.cpp`)
  - **`sizeof`/`alignof`:** of types and expressions, using
    `kiln_abi` layout; `alignof` of an expression naming a variable or
    data member uses its declared alignment (§4.5). An invalid operand
    (function type, incomplete type, sizeless type, bit-field) is
    diagnosed as clang `CheckUnaryExprOrTypeTraitOperand`: a type
    operand at the keyword with the `( type-id )` range (the whole
    expression when rebuilt by an instantiation), an expression operand
    at its expression location (`getExprLoc`: its begin, a member call's
    member name) with its range (the bit-field error without one;
    WP-320, test `expr.sizeof/incomplete-operand-location.cpp`)
  - **casts:** C-style, functional, `static_cast`,
    `reinterpret_cast`, `const_cast`, with
    `mark_part_of_explicit_cast`. `const_cast` between pointers needs
    them similar with identical levels (`conv::similar_same_levels`,
    [expr.const.cast]/3: `int A::**` to `const int A::**` is allowed,
    `int *(*)[2]` to `const int *(*)[]` is not, like clang's
    `UnwrapSimilarTypes` without `AllowPiMismatch`; WP-162, test
    `expr.const.cast/similar-multilevel.cpp`). To a reference (`expr/ref_cast.rs`,
    WP-207): `const_cast<T&&>` (and the C-style cast's const step) of a
    class prvalue of the same unqualified class materializes it
    (`MaterializeTemporaryExpr`, [expr.const.cast]/3) and is a `NoOp`
    xvalue; `reinterpret_cast` (and the C-style cast's reinterpret step)
    binds any glvalue, an xvalue as an lvalue reference too (`NoOp` for
    the same type, else `LValueBitCast`); `const_cast` of a non-lvalue to
    `T&`, of a non-class prvalue to `T&&`, and `reinterpret_cast` of a
    prvalue are `{const_cast|reinterpret_cast} from rvalue to reference
    type 'T &'` (clang `err_bad_cxx_cast_rvalue`). A failed cast's
    `RecoveryExpr` has the referenced type and the reference's value
    category, as every `RecoveryExpr` of a reference type does (clang
    `CreateRecoveryExpr`), so it is not diagnosed again. Not yet: the
    C-style cast's own rvalue message (`(S&)mk()` reports the
    initialization failure of its static step instead of `C-style cast
    from rvalue to reference type`). A value of type `std::nullptr_t`
    converts to an integral type (not an enumeration) as `(void*)0`
    does (`PointerToIntegral`, with the pointer path's
    `cast from pointer to smaller type` error, [expr.reinterpret.cast]/4,
    WP-296). A cast to a non-reference, non-class type that no rule
    allows is `{static_cast|…} from 'F' to 'T' is not allowed` (clang
    `err_bad_cxx_cast_generic`, `bad_cast`), where `F` is the operand's
    type after the array-to-pointer and function-to-pointer conversions
    (`'int *'` for an `int[3]`, `'void (*)(int)'` for a function), and
    `const_cast<void>` is `const_cast to 'void', which is not a
    reference, pointer-to-object, or pointer-to-data-member` (WP-327;
    test `expr.static.cast/bad-cast-decayed-operand.cpp`).
- **Constant-folding warnings:** division by zero, negative shifts and
  over-wide shifts warn when the operands fold. They are run-time
  behavior diagnostics anchored at the right operand (clang's
  `DiagRuntimeBehavior` → `DiagIfReachable`, §5.12): they are dropped
  where `runtime_diag_quiet` holds, deferred to the end of a function
  body (and dropped there when unreachable), and skipped in the
  initializer of a namespace-scope `constexpr` variable or of a static
  data member's first declaration (WP-235).
- **Pointer arithmetic on `void` or a function type** ([expr.add]/1;
  `check_pointer_arith`, WP-219): `+`, `-`, `++`, `--` and compound
  assignment on a pointer to `void` or to a function type is an error in
  C++ (`arithmetic on a pointer to void`, `arithmetic on pointers to the
  function type 'void ()'` for two pointer operands), not the GNU C
  extension (`-Wgnu-pointer-arith`, kept for C). A pointer difference
  (`check_pointer_pair_arith`, clang `checkArithmeticBinOpPointerOperands`,
  WP-230) runs after `'T *' and 'U *' are not pointers to compatible
  types` too: a `void` pointee on one side is diagnosed for that side,
  on both as `pointers to void`; likewise a function pointee, the
  two-pointer form naming both pointees when the pointer types differ
  (`arithmetic on pointers to function types 'void ()' and 'void
  (int)'`); otherwise an incomplete pointee, left side first.
- **Relational operators and `std::nullptr_t`** ([expr.rel]/1, CWG583;
  clang's `CheckCompareOperands`, WP-276): the operands of `<`, `>`,
  `<=`, `>=` have arithmetic, enumeration or pointer type, so a
  `std::nullptr_t` operand on either side is `invalid operands to binary
  expression ('std::nullptr_t' and 'int *')` (a SFINAE failure in a
  template argument, as libc++'s `nullptr_t.pass.cpp` `has_less`
  checks); a pointer against an integral null pointer constant is
  `ordered comparison between pointer and zero ('int *' and 'int')`. The
  composite pointer type is formed only for `==`/`!=` or two pointers.
- **Member-pointer equality** ([expr.eq]/4, clang's
  `CheckCompareOperands`, WP-085): when either operand of a comparison
  is a member pointer:
  - `<`, `>`, `<=`, `>=` (and `<=>`, not yet distinguished) are
    `invalid operands to binary expression ('int B::*' and 'int B::*')`;
  - a null pointer constant on either side (`0`, `nullptr`, checked
    right operand first) converts to the other operand's type
    (`NullToMemberPointer`); the result is `bool`;
  - two member pointers of the same type compare directly; otherwise
    the right operand converts to the left's type, then the left to the
    right's, by a standard conversion (`BaseToDerivedMemberPointer` from
    the base's to the derived class's member pointer, plus
    qualification conversions). A conversion through a virtual base
    reports its conversion error at the operand, followed by the
    distinct-pointer error below;
  - no conversion either way: `comparison of distinct pointer types
    ('int B::*' and 'int O::*')` when both are pointers or member
    pointers, otherwise (a non-null integer, `nullptr` as a non-null
    operand of a different kind) `invalid operands`.
  Not yet: the composite type that needs a cv-union of both sides
  for comparisons (`const int B::*` vs `volatile int D::*`;
  [expr.type]/4.6) and constant evaluation of the comparison (WP-076).
- **Composite pointer type** ([expr.type]/4; `composite_pointer` in
  `expr.rs`, used by `==`, `<`, `<=>` and `?:`): after the null,
  same-type and one-way-conversion cases, `cv_combined_pointer` tries a
  pointer to either side's unqualified pointee with the cv-qualifiers of
  both (`void *` vs `const B *` is `const void *`; `B *` vs `const D *`
  is `const B *`), taking the first that both operands reach by a
  standard conversion (WP-155). Member pointers take the same path
  ([expr.type]/4.6, WP-335): a null pointer constant against a member
  pointer converts to it, and two member pointers combine to a member
  pointer of the cv-combined pointee in either side's class (`int S::*`
  vs `const int S::*` is `const int S::*`; `int S::*` vs `int D::*` is
  `int D::*`; a pointer to `noexcept` member function converts to the
  plain one). Unrelated classes have no composite type (`incompatible
  operand types ('int A::*' and 'int B::*')` for `?:`). This is what
  libc++'s `common_with`, `common_reference` and
  `equality_comparable_with` see through `decltype(b ? x : y)`. Two
  similar types of any depth ([conv.qual]/2: pointer, same-class member
  pointer and array levels down to the same innermost type) combine,
  before the one-level path, to their cv-combined type
  (`conv::cv_combined_type`, [conv.qual]/3, WP-162): the union of the
  cv-qualifiers at each level, `const` added to every level above one
  that differs (`int **` vs `const int **` is `const int *const *`,
  `int (*)[3]` vs `const int (*)[3]` is `const int (*)[3]`). `B **` vs
  `D **` and `int **` vs `long **` are not similar and have none. Tests
  `expr.cond/composite-pointer-multilevel*.cpp`,
  `conv.qual/multilevel-compare-run.cpp`.
- **C-style cast of a parenthesized list** (clang
  `ParseSimpleExpressionList`, WP-435): in `(T)(a, b, c)` the parser
  parses every element first, then builds the comma operators left to
  right at the operand's `(` (`Parser::parse_paren_list_rest`), so the
  elements' own diagnostics come before the commas' and a comma's
  diagnostic is at the `(`; a nested `((a, b))` keeps its own comma
  locations, and a `{` element is "expected expression". Test
  `expr.comma/cast-paren-list.cpp`.

### 5.1 Unevaluated operands
`unevaluated > 0` inside `sizeof`, `alignof` and `noexcept` operands.
Names there are not odr-used.

### 5.2 Odr-use
- A DeclRefExpr to a variable is a *potential* odr-use, queued in
  `pending_odr`.
- `act_on_full_expr` resolves the queue at the end of each
  full-expression ([basic.def.odr]/5, [intro.execution]/5):
  - It walks the expression and collects the potential results of
    every lvalue-to-rvalue conversion.
  - A variable usable in constant expressions that is named there is
    marked `non_odr_use_constant`. So is a reference usable in constant
    expressions.
  - Every other queued variable is marked `used`.
  - Cost: linear in the full-expression. The candidates are sorted
    once (`ConstantCandidates`), and each queued reference is a binary
    search, not a scan: a braced initializer of N elements naming
    constant variables (`{a[0], s.m, v, ...}`) was O(N^2) (WP-1335;
    test `tests/stress/init-list-constant-refs-65536.cpp`).
- **Potential results** (`l2r_potential_results`, WP-1144; clang 19/22
  `Sema::CheckLValueToRValueConversionOperand` and
  `rebuildPotentialResultsAsNonOdrUsed`, SemaExpr.cpp):
  - An operand that is volatile or of class type yields none (clang
    skips both; a class operand is copied, not converted).
  - The set of `e` ([basic.def.odr]/3) is `e` for a DeclRefExpr, and
    recurses through `(e)`, `__extension__ e`, a `ConstantExpr`, both
    arms of `?:`, the right operand of a comma, the left operand of `.*`,
    `e.m` for a non-static data member (not `->`, which reads a pointer),
    the array operand of a subscript (`e[i]` and `i[e]`, implicit nodes
    stripped first), and every association of `_Generic`.
  - Deviations kept for clang compatibility: `__builtin_choose_expr`
    recurses into its first operand only (clang rebuilds the LHS twice,
    so the RHS stays an odr-use); a variable whose class type has a
    mutable member stays an odr-use, but only the variable's own type is
    checked (an array of such a class is not); the discarded-value
    bullet is never applied (clang leaves `NOUR_Discarded` unused), so
    `(void)ca` odr-uses `ca`; in a generic lambda with a deduced return
    type `return ca.v` has no conversion until instantiation, so `ca` is
    captured.
  - Codegen (spec/codegen.md): a non-odr-used local named from a
    lambda or local class reads the variable's `__const` global.
  - Tests `basic.def.odr/potential-results-closure-run.cpp` (closure
    size per form), `basic.def.odr/potential-results-lambda-ir.cpp`,
    `expr.prim.lambda.capture/no-capture-default-odr-use.cpp`,
    `expr.prim.lambda.capture/constant-member-access-not-captured.cpp`.
- Names inside unevaluated operands are marked
  `non_odr_use_unevaluated`.
- A default template argument substituted during deduction
  ([temp.param]/14) is its own full-expression: `default_template_arg`
  sets the caller's queue aside and appends its own leftovers back, so
  its `act_on_full_expr` cannot resolve the enclosing call's pending
  uses early (a constexpr local `Size` passed from a lambda,
  `h<T, N = sizeof(T)>(1, Size)`, stays a non-odr-use and needs no
  capture; WP-336).
- The dump reads `used` and `referenced` from the first declaration.

**Undefined but used entities** (WP-840, `undefined_used.rs`; clang
`Sema::checkUndefinedButUsed`, [basic.def.odr]/11, [basic.link]/8):
- Every odr-use outside a template and outside an unevaluated operand
  (`mark_used_at`, and the variables `act_on_full_expr` marks used)
  records the first declaration and the location of its first use, when
  the function or variable has no definition yet (a defaulted or deleted
  function counts as defined).
- A default argument is potentially evaluated only where a call uses it
  ([dcl.fct.default]/5; clang's `PotentiallyEvaluatedIfUsed`): the uses
  recorded while one is parsed (in place or replayed,
  `begin_default_arg_uses` / `end_default_arg_uses`) move to the stored
  argument and are recorded again at each call that uses it
  (`queue_default_arg_uses`).
- At the end of the translation unit (`act_on_end_of_translation_unit`,
  after the pending instantiations and the delegating-constructor
  cycles, before the unused-file-scope warnings), and only when no
  error was emitted, each recorded entity still undefined is diagnosed
  at its first declaration, in first-use order, with a `note: used here`
  at the use. Skipped: deduction guides, invalid declarations, `weakref`
  and `alias` entities, variables with an initializer on any
  redeclaration (an in-class static member initializer), builtins,
  pure virtual functions (kiln does not tell a virtual call from a
  qualified one), and anything in namespace `std` or nested in it (the
  evaluator supplies the `std::meta` metafunctions and a `constexpr`
  `std::uncaught_exceptions` without a definition).
- The classification, in order:
  - external formal linkage, not `extern "C"`, and a type without
    external linkage (a lambda, local class or unnamed-namespace class
    in the function type or the variable type; [basic.link]/15: a class
    template specialization, and a member of one, has linkage whatever
    its template arguments, so `W<Local>` and `W<Local>::N` do not count
    here, `mangle::type_has_external_linkage`, WP-1224): an error,
    "{function|variable} 'N' is used but not defined in this translation
    unit, and cannot be defined in any other translation unit because
    its type does not have linkage";
  - not externally visible (internal linkage, an unnamed namespace, a
    specialization or member taking internal linkage from a template
    argument or its class): `-Wundefined-internal`, "{function|variable}
    'N' has internal linkage but is not defined";
  - an inline function (`inline`, implicitly inline or `constexpr`, no
    `gnu_inline`): `-Wundefined-inline`, "inline function 'N' is not
    defined"; an inline variable: the error "inline variable 'N' is not
    defined".
- A function or variable template specialization is named with its
  template arguments (`g<(lambda at f.cpp:3:5)>`); each specialization of one
  template is reported, all at the template's location.
- Not implemented: clang's vtable-use marking (an undefined virtual
  member of an unnamed-namespace class whose vtable is used is not
  reported).
- Tests: `tests/conformance/basic.def.odr/undefined-but-used-*.cpp`.

### 5.3 The `noexcept` operator (`except.rs`)
`act_on_noexcept_expr(e, loc, rparen)` ([expr.unary.noexcept]) builds
a `CXXNoexceptExpr`: a `bool` prvalue whose operand is unevaluated
(the parser brackets it with `enter_unevaluated`/`exit_unevaluated`).
Its value is fixed at construction, `!can_throw(e)`, and stored on the
node (`ExprStore::noexcept_value`); the Oven and codegen read it as a
constant and never look at the operand.

`can_throw(e)` is true when `e` is potentially-throwing
([except.spec]/6), walking the operand and every subexpression:

| construct | implemented |
|---|---|
| a call whose callee's function type (through a pointer or reference) is not non-throwing; a callee without a prototype throws | yes |
| `throw` expression | yes |
| `new` expression whose allocation function is not non-throwing ([except.spec]/6.1; clang `canThrow`); its initializer and operands are walked as subexpressions | yes (WP-783: placement and `std::nothrow` forms are non-throwing when the initializer is) |
| `delete` expression whose deallocation function, or the destroyed (element) type's destructor, is not non-throwing; a trivial destructor never throws ([except.spec]/8) | yes (WP-783) |
| `dynamic_cast` to a reference type needing a run-time check | yes (not produced by the M2 parser) |
| `typeid` of a glvalue of polymorphic class type | M3 (no polymorphic classes in M2) |
| implicit calls: constructors, destructors, conversion functions, allocation functions | yes: constructors (`CXXConstructExpr`), destructors (`CXXBindTemporaryExpr`) and conversion functions (implicit member calls), each resolving the callee's spec first (WP-112); allocation and deallocation functions as in the `new`/`delete` rows |
| `__attribute__((nothrow))` on the callee | yes (WP-186: the function type is non-throwing, §3 "GNU `nothrow`") |
| a dependent operand (value-dependent `CXXNoexceptExpr`) | yes (lazily instantiated member specs, §4.7) |
| a pseudo-destructor call `t.~T()` ([expr.prim.id.dtor]/2: no effect) | yes (WP-275): never potentially-throwing, and, as clang's `canThrow`, its object expression is not walked either (`noexcept(f().~I())` is true) |

The operands of nested `sizeof`, `alignof` and `noexcept` are not
looked at (they are unevaluated). An erroneous operand gives an error
expression. Before `can_throw` reads them, the exception specifications of the
constructors, bound temporaries' destructors and direct callees in the
operand are resolved ([except.spec]/13, `resolve_operand_exc_specs`,
WP-802): an operand kept unchanged from a template pattern (`V()` in
`noexcept(noexcept(V().f(0, x)))` of a function template, with `V`'s
implicit default constructor) was built without resolving them, and
the instantiation must not count them as potentially-throwing
(`tests/conformance/except.spec/p13-noexcept-operand-from-pattern-resolves-specs-run.cpp`).

### 5.4 Builtin functions (`builtins.rs`)
clang's `LazilyCreateBuiltin`. kiln knows one builtin,
`bool __builtin_is_constant_evaluated() noexcept` (id 722, clang's
`BIis_constant_evaluated`; `kiln_ast::decl::builtin`).
The AArch64 NEON target builtins (`__builtin_neon_*`, a generated
table) are spec/landed/sema/WP-558.md "Target builtins".
The AArch64 ACLE builtins (`__builtin_arm_*`, clang's
`BuiltinsAArch64.def` rows, `kiln_ast::decl::builtin::arm`) are
spec/landed/sema/WP-724.md "ACLE builtins": declared only when the
target is AArch64, like the NEON ones.
The x86-64 target builtins (every `__builtin_ia32_*` name the resource
headers use, `__builtin_ia32_pause` (WP-610), and the header builtins
`_mm_pause`, ...; a generated table) are spec/landed/sema/WP-630.md:
declared only when the target is x86-64 (`lazily_create_builtin` /
`is_builtin_name` / `recognize_builtin_decl` via `target_has_builtin`),
an undeclared identifier (or, for a header builtin, an ordinary
function) elsewhere. WP-961: clang's plain `__rdtsc` (`BUILTIN(__rdtsc,
"UOi", "")`, the target of <ia32intrin.h>'s `_rdtsc()` macro) is an
alias of the `__builtin_ia32_rdtsc` row (`builtin::by_name`): declared
under its own name, the same signature, gating and codegen
(`target.x86/rdtsc-ir.cpp`, `rdtsc-not-aarch64.cpp`).
- **Declaration:** when an unqualified id-expression's lookup finds
  nothing and the name is a builtin (`lazily_create_builtin`), Sema
  declares it in the translation unit: an implicit `LinkageSpecDecl`
  (`extern "C"`) holding an implicit `extern` `FunctionDecl` with the
  builtin's function type, its id in the `builtin_id` cold data, an
  implicit `Builtin` attribute (invalid location, `a` = id), for the
  printf builtins an implicit `format(printf, F, A)` attribute
  (`builtin::printf_format`, WP-282) and an implicit `NoThrow`
  attribute, in clang's order. Later lookups find that declaration.
  WP-726: it is never in a dependent context, even when first used in a
  template, so a reference to it is not instantiation-dependent and
  `decltype(__builtin_f(a))` in a function template's signature is the
  call's type once `a` is substituted
  (`dcl.type.decltype/builtin-call-return-type-run.cpp`).
- **One declaration per builtin** (WP-332): every implicit use looks
  the name up first, as clang does. The memberwise copy of a trivially
  copyable array member (spec/sema-class.md §4.5, clang
  `buildMemcpyForAssignmentOp`) does an ordinary lookup of
  `__builtin_memcpy` in the translation unit and calls what it finds
  (the lazily created builtin of an earlier written call, or a user
  redeclaration); only when nothing is found does it call
  `lazily_create_builtin`, and later written calls then find that
  declaration. A user redeclaration with the builtin's type
  (`extern "C" void *__builtin_memcpy(void *, const void *, size_t)`)
  is a redeclaration of the implicit one, before or after it. A
  redeclaration inherits the builtin id, and a first declaration of a
  builtin's name at file scope with C linkage and the builtin's type
  (ignoring the exception specification) is recognized as the builtin
  (`recognize_builtin_decl`; clang `BuiltinAttr`), so a call through it
  still lowers as the builtin. Tests:
  `tests/conformance/class.copy.assign/libstdcxx-builtin-memcpy-written-first.cpp`,
  `tests/conformance/class.copy.assign/libstdcxx-builtin-memcpy-implicit-first.cpp`.
- **A function template with a builtin's name** (WP-446): a function
  template declared at file scope with a builtin's name
  (`template<class D, class S> void *__builtin_memmove(D*, S*, size_t)`)
  does not redeclare the builtin and does not hide it. clang's
  redeclaration lookup (`HandleDeclarator`'s `CreateBuiltins`) creates the
  builtin in the translation unit first, so an unqualified (or `::`) name
  that finds only file-scope functions and function templates, at least
  one a template and none the builtin, also finds the lazily created
  builtin, and overload resolution chooses: the template for an exact
  `int*` match, the builtin for a tie ([over.match.best]/2.4). libc++
  `copy_move_trivial.pass.cpp`. Test:
  `tests/conformance/over.match.best/builtin-name-template.cpp`.
- **Redeclaring a builtin** (WP-410; clang `MergeFunctionDecl`,
  `CheckEquivalentExceptionSpec`, `ActOnStartOfFunctionDef`): only a
  file-scope (translation-unit redeclaration context), non-`static`
  declaration with C linkage of a builtin's name is checked; one at
  namespace scope, with C++ linkage, or `static` is an ordinary
  function. Library builtins without the `__builtin_` prefix (`sin`)
  are not checked in C++ (clang's incompatible-library-redeclaration
  warning is C only). For a checked first declaration
  (`recognize_builtin_decl`):
  - a builtin with custom type checking is `sema_builtin_redeclare`
    ("cannot redeclare builtin function 'X'");
  - otherwise a type other than the builtin's is
    `sema_conflicting_types`, except that at -fclang-compat=19 a type
    differing only in the return type is
    `sema_ovl_diff_return_type` (clang 19 checks overloading first);
  - each with `note_previous_builtin_declaration` ("'X' is a builtin
    with type T", `builtin_type_note`) at the declaration; the
    declaration is invalid and is not the builtin (a later call finds
    the implicit builtin, `lazily_create_builtin`).
  - A builtin that cannot throw redeclared `noexcept(false)` (or with a
    potentially-throwing dynamic specification) is
    `sema_exception_spec_mismatch` with `note_previous_declaration` at
    the previous declaration (the first declaration itself when there
    is none); one written without a specification gets the builtin's
    `throw()` merged in.
  - A later redeclaration that conflicts with one recognized as the
    builtin reports the same note in place of "previous declaration is
    here".
  - A definition of a declaration recognized as a builtin is
    `sema_builtin_definition` ("definition of builtin function 'X'"),
    and the function is invalid.
  - Not implemented: clang's "different language linkage" error for an
    `extern "C++"` redeclaration of an implicitly declared builtin.
  Tests: `dcl.fct/builtin-redecl*.cpp`, `c.math/builtin-redecl.cpp`.
- **Name:** a `DeclRefExpr` to a builtin is a prvalue of the
  placeholder type `QualType::BUILTIN_FN` (`<builtin fn type>`).
- **Calls:** `act_on_call` converts the callee with
  `BuiltinFnToFnPtr` (a pointer to the builtin's function type) and
  type-checks the call as an ordinary one.
- **Any other use** is `builtin functions must be directly called`
  (`check_builtin_fn_use`), reported at the name by
  `implicit_convert` and `decay` (initialization, operands,
  arguments), by unary `&`, and by an expression statement (which then
  gets no unused-result warning). The expression recovers as an error
  expression.
- **Evaluation:** the Oven answers `true` in a manifestly
  constant-evaluated context and `false` otherwise (spec/oven.md);
  codegen emits `i1 false` (spec/codegen.md).
- **Library functions** (WP-171, clang `ActOnFunctionDeclarator`'s
  library-builtin recognition + `AddKnownFunctionAttributes`,
  `add_known_function_attrs`): the first declaration of a function
  named in `builtin::LIB` (clang 19's library builtins carrying
  `NoThrow` / `Const` / `Pure`: `<string.h>`, `<stdio.h>` `*printf`,
  `<ctype.h>`, `<wchar.h>`, `<math.h>` d/f/l), declared in a namespace
  scope context (TU or namespace, through linkage specs) with C
  language linkage and with the row's type ignoring the exception
  specification (clang's `hasSameFunctionTypeIgnoringExceptionSpec`:
  the `noreturn` bit is part of the type, so a `NoReturn` row such as
  `abort` matches only a declaration with GNU `noreturn`; `void abort();`
  and `[[noreturn]] void abort();`, a declaration attribute, do not,
  WP-938), gets implicit `NoThrow` and `Const`/`Pure` attributes
  (after its written ones). Sig letter `a` is `__builtin_va_list`
  (decayed when an array), `P` the type of the TU-scope `FILE`
  (below), `R` adds `restrict` to the type
  before it (WP-270; skipped when counting `I` arguments). Redeclarations inherit them
  (`Ast::has_nothrow_attr` reads the chain). A `NoThrow` callee makes
  a call non-throwing (`can_throw`, so `noexcept(strlen(s))` is
  true). No `Builtin` attribute is attached, so the call stays an
  ordinary call everywhere but in codegen: WP-419 records the row's
  `__builtin_` twin (`Decls::lib_builtin`, `ceil` -> `__builtin_ceil`,
  at the `-fclang-compat` level; inherited by redeclarations) and
  codegen lowers a direct call of the errno-free `<math.h>` functions
  and (WP-186) of `memcpy` / `memmove` / `memset` / `abs` / `labs` /
  `llabs` as the twin (spec/codegen.md §3, "library builtins"); sema
  does not read it, and the Oven reads it only for the functions clang's
  evaluator evaluates (WP-940, spec/oven.md "Builtins": `strlen`,
  `memcmp`, ... evaluate behind a constant-expression note). WP-419 adds
  C23's `roundeven{,f,l}` (clang 19 and 22), clang 22's
  `fmaximum_num` / `fminimum_num` `{,f,l}` (`builtin::lib_min_compat`:
  not recognized at `-fclang-compat=19`) and the GNU `sincos{,f,l}`
  (clang 22, `ALL_GNU_LANGUAGES`: recognized under `-std=gnu++*` only,
  `builtin::lib_gnu_only`; `NoThrow`, a call is `nounwind`). WP-938 adds
  `abort` (`NoThrow`, `NoReturn`), the `<complex.h>` functions (`cabs`
  ... `ctanh`, `cpow`, `cproj` `{,f,l}`; `NoThrow`, `cproj` also
  `Const`), clang 19 and 22's rows; a call under a pending cleanup is
  then a `call`, not an `invoke`. WP-1055 (audit against clang 19/22
  `Builtins.td`) adds `memcmp` and, GNU modes only, `bcmp`, `bcopy`,
  `bzero`, `index`, `rindex`, `stpcpy`, `stpncpy`, `strdup` and
  `strndup`; under `-std=c++*` `bcmp` is an ordinary function. `memcmp`
  and `bcmp` have no implicit attributes; their twin is recorded for the
  never-constant check and the WP-940 fold. WP-1073 adds clang's
  allocation functions `malloc`, `calloc`, `realloc`, `aligned_alloc`
  and (GNU modes only) `memalign`, which get no `NoThrow` but
  `AddKnownFunctionAttributes`'s implicit `alloc_size` (`(1)`, `(1, 2)`,
  `(2)`, `(2)`, `(2)`; `builtin::lib_alloc`) and, for `aligned_alloc` /
  `memalign`, `alloc_align(1)`, unless the attribute is written; these
  feed codegen's `allocsize` / `align` (spec/codegen.md §3.1) and the
  Oven's `__builtin_object_size` (spec/oven.md "Builtins"). The scanf
  family (`scanf`, `fscanf`, `sscanf`, `vscanf`, `vfscanf`, `vsscanf`;
  `builtin::lib_scanf_format`) gets an implicit `format(scanf, F, A)`
  as the printf family below, checked by `-Wformat`'s scanf rules
  (`format_string::walk_scanf`: conversions take pointers, `%[` sets,
  `*` suppression consumes no argument, an invalid conversion
  specifier consumes none either, so a later surplus argument is still
  "data argument not used"). GNU modes recognize `alloca` (`NoThrow`)
  and `mempcpy`, and `finite{,f,l}` (`NoThrow`, `Const`; glibc's
  `__finite{,f,l}` in every dialect), whose twin is `__builtin_isfinite`
  (`builtin::lib_twin`); `creal` / `cimag` / `conj` `{,f,l}` are `NoThrow`
  and `Const` with the new `__builtin_creal` ... twins, and `bzero` /
  `bcopy` gain `__builtin_bzero` / `__builtin_bcopy` twins. All of these
  are lowered inline by codegen. `ReturnsTwice` (`setjmp`, `_setjmp`,
  `sigsetjmp`, `__sigsetjmp`, `savectx`, `vfork`, `getcontext`) was
  already modelled by codegen's `returns_twice` (`builtins::returns_twice`) on the declaration and
  the call (WP-807, spec/codegen.md §3), and `abort` already matched
  clang (WP-938); WP-1073 confirmed both. A call in
  a constant expression is still a call of a non-constexpr function
  (clang 19 and 22 reject `constexpr double d = ceil(1.5);` and
  `constexpr auto n = strlen("ab");`); only a fold (`__builtin_constant_p`,
  a GNU-folded array bound, codegen) evaluates the `lib_evaluated` set. The printf
  family (`printf`, `fprintf`, `sprintf`, `snprintf`, `vprintf`,
  `vfprintf`, `vsprintf`, `vsnprintf`; `builtin::lib_printf_format`,
  WP-186) gets clang's implicit `format(printf, F, A)` first, unless a
  `format` is written. `FILE` in a row's type (`P`) is the translation
  unit's typedef, alias or (as clang's `LookupNecessaryTypesForBuiltin`)
  tag named `FILE`. Not modelled: clang 22's `modf` to `llvm.modf`.
  Under `-fno-builtin` / `-ffreestanding` (`Sema::no_builtin`) no
  function is recognized, and under `-fno-builtin-<name>`
  (`Sema::no_builtin_fns`) not `<name>` (`lib_builtin_info`; clang
  `Builtin::Context::initializeBuiltins` skips them): no implicit
  attributes, no twin, an ordinary call (WP-808).
  WP-1104 (clang 19.1.7 and 22.1.8 `Builtins.td` diffed against kiln's
  table) adds the `__builtin_` aliases of C library functions that kiln
  lacked (`builtin::LIBC_FWD`, `builtin::is_libc_fwd`; clang's
  `LibBuiltin` rows with `AddBuiltinPrefixedAlias` and plain `Builtin`
  rows): `__builtin_malloc`, `calloc`, `realloc`, `free`, `fprintf`,
  `vfprintf`, `fscanf`, `vfscanf`, `index`, `rindex`, `stpcpy`,
  `stpncpy`, `strcasecmp`, `strncasecmp`, `strcat`, `strcpy`,
  `strncat`, `strncpy`, `strcspn`, `strspn`, `strdup`, `strndup`
  (declared lazily in every dialect; `NoThrow` except `strcasecmp`,
  `strncasecmp`, `fscanf`, `vfscanf`; no `alloc_size`, which clang
  attaches only to the library names), and
  `__builtin_alloca_uninitialized` /
  `__builtin_alloca_with_align_uninitialized`, whose alignment
  argument is checked as `__builtin_alloca_with_align`'s. A
  `__builtin_` row whose type names `FILE` (`P`) is not declared while
  lookup finds no `FILE` (clang `GetBuiltinType`'s `GE_Missing_stdio`,
  for which `LazilyCreateBuiltin` declares nothing): the use is "use of
  undeclared identifier", and a use after a `FILE` declaration finds
  the builtin (this also covers `__builtin___fprintf_chk` /
  `__builtin___vfprintf_chk`). The `__builtin_` scanf forms
  (`__builtin_scanf`, `sscanf`, `vscanf`, `vsscanf`, `fscanf`,
  `vfscanf`; `builtin::scanf_format`) get the implicit
  `format(scanf, F, A)` and `__builtin_fprintf` / `__builtin_vfprintf`
  `format(printf, 2, 3)` / `(2, 0)` (`builtin::printf_format`), checked
  by `-Wformat` as the library names are
  (`ext.gnu.builtins/format-scanf-builtins.cpp`). With the new rows the
  library names `malloc`, `strcpy`, ... also record a `__builtin_` twin
  (`builtin::lib_twin`); codegen and the Oven act on a twin only for
  their listed ids, so nothing else changes. Not added (outside the
  supported language): the SafeStack `__builtin___get_unsafe_stack_*`,
  the matrix-type `__builtin_matrix_*`, the Objective-C
  `__builtin_objc_memmove_collectable` and clang 22's
  `__builtin_trivially_relocate` (P2786, out of scope).
- **GCC's implicit library builtins** (WP-380, `record_gnu_lib_twin`;
  `Sema::gnu_lib_builtins`, set by the driver for `-stdlib=libstdc++`,
  the default): libstdc++'s `<cmath>` double overloads are `using
  ::ceil;` etc., so its C++23 constexpr `std::ceil(double)` (P0533)
  relies on GCC folding the C library function as a builtin. A first
  declaration recognized as a library function above whose name is
  `ceil`, `floor`, `trunc`, `round`, `fabs`, `fmin`, `fmax`,
  `copysign` or `sqrt` (WP-393; or the `f`/`l` variant) records its `__builtin_` twin
  (`builtin::gnu_lib_twin`; the `gnu_lib_twin` cold data);
  redeclarations inherit it. Only the Oven reads it: a call folds as the
  twin (spec/oven.md "Builtins"); every other rule (attributes, codegen,
  diagnostics) treats the call as ordinary. GCC 14 is the reference:
  `rint`/`nearbyint` are not folded (GCC leaves them to the rounding
  mode at run time); `sqrt` folds correctly rounded, so libstdc++'s
  constexpr `std::sqrt(double)` works (clang folds neither). Under `-stdlib=libc++` nothing
  is recorded, as clang (libc++'s `constexpr-cxx23-clang.pass.cpp`
  requires `::ceil` to stay non-constant). A C++-linkage or class-scope
  `ceil`, or one with another type, is an ordinary function
  (`tests/conformance/c.math/gnu-lib-builtins*.cpp`, `gnu-lib-sqrt.cpp`).
- **Custom-checked builtins** (WP-270): the generic
  `__builtin_{add,sub,mul}_overflow` (`builtin_overflow`) and the
  alignment of `__builtin_alloca_with_align` (`check_alloca_align`) are
  checked as clang's `SemaBuiltinOverflow` / `SemaBuiltinAllocaWithAlign`;
  the rules and the builtin table are spec/sema-templates.md §9
  ("checked arithmetic", "stack allocation", "printf family" rows).
- **Library builtins** (WP-282; clang 19 `SemaChecking.cpp`), each
  diagnosed with clang's wording:
  - `__builtin_assume_aligned(p, a[, off])` (custom, `builtin_assume_aligned`,
    clang `BuiltinAssumeAligned`): 2 or 3 arguments (`too few` / `too many
    ... at most` at the `)`); `p` decays and must convert to `const void *`
    (it keeps its own type); `a`, unless value-dependent, an integer
    constant of integral or unscoped enumeration type (an enumerator,
    LLVM's `PickAlignment<T, A>::value`; WP-730) (`argument to '...'
    must be a constant integer` at the call),
    a power of 2 (`requested alignment is not a power of 2`), warned above
    2^32 (`-Wbuiltin-assume-aligned-alignment`); `off` converts to
    `size_t`. The call is a `void *` prvalue.
  - `__builtin_is_within_lifetime(p)` (WP-418; clang 20 `BuiltinIsWithinLifetime`,
    custom, `builtin_is_within_lifetime`; P2641R4): the only consteval
    builtin (`builtin::attr::CONSTEVAL`, so its lazily created
    declaration is `consteval`). One argument (`too few`/`too many`),
    decayed; a non-pointer (`int`, `nullptr`, a member pointer) or a
    function pointer is `{non-|function }pointer argument to
    '__builtin_is_within_lifetime' is not allowed` at the argument. The
    call is a `bool` prvalue and an immediate invocation like any other
    (`check_immediate_invocation`: outside an immediate function context
    it must be a constant expression, and it escalates a constexpr
    template, [expr.const]/16-20); a reference left by an invalid call is
    clang's `cannot take address of consteval function` error, without a
    `declared here` note (a builtin has no declaration). Available at
    `-fclang-compat=22` (`__has_builtin` is 0 below). Evaluation is
    spec/oven.md "Builtins". Tests:
    `tests/conformance/meta.const.eval/is-within-lifetime*.cpp`.
  - `__builtin_start_lifetime(p)` (WP-773; GCC 17 `BUILT_IN_START_LIFETIME`,
    `builtin_start_lifetime`; P3726R2 `std::start_lifetime`): a GCC-only
    builtin (`gcc_only`, `__has_builtin` is 1). One argument (`too
    few`/`too many`), decayed; then, in GCC's order, each failing check
    is `'__builtin_start_lifetime' argument type T is not {pointer
    type|a pointer to aggregate type|a pointer to implicit-lifetime
    type}` at the argument (`sema_builtin_start_lifetime_invalid_arg`),
    except an incomplete pointee, which is clang's `incomplete type 'X'
    where a complete type is required` with the forward-declaration
    note: not a pointer; incomplete pointee; pointee not an aggregate
    (`is_aggregate`); pointee not implicit-lifetime
    (`__builtin_is_implicit_lifetime`). The call is a `void` prvalue and
    `noexcept`. Evaluation is spec/oven.md "Builtins"; codegen evaluates
    the argument and emits nothing else (spec/codegen.md). Tests:
    `tests/conformance/obj.lifetime/start-lifetime-*.cpp`.
  - `__builtin_clear_padding(p)` (WP-774; GCC `BUILT_IN_CLEAR_PADDING`,
    `check_builtin_function_arguments`; clang has none, LLVM issue
    64830): a GCC-only builtin (`gcc_only`, `__has_builtin` is 1 at both
    compat levels, `__has_constexpr_builtin` 0 as in GCC). One argument
    (`too few`/`too many`), decayed. GCC's checks run in GCC's order; the
    first that fails is `argument 1 in call to function
    '__builtin_clear_padding' {does not have pointer type|has pointer to
    'const' type (T)|points to incomplete type (T)|has pointer to a
    non-trivially-copyable type (T)}` at the argument
    (`sema_builtin_clear_padding_invalid_arg`; GCC prints no type for
    the incomplete case, and kiln emits no forward-declaration note). The
    checks are: not a pointer; a `const` pointee; an incomplete or `void`
    pointee (`int (*)[]` included); a pointee that is not trivially
    copyable. A volatile pointee is accepted. The call is a `void`
    prvalue. The constant-evaluation semantics are a kiln extension
    (spec/oven.md "Builtins"); codegen is spec/codegen.md "Clearing
    padding". Tests: `tests/conformance/ext.gnu.clear_padding/`.
  - `__builtin_align_up` / `__builtin_align_down` / `__builtin_is_aligned`
    (custom, `builtin_align`, clang `BuiltinAlignment`; WP-416): two
    arguments. The source must be an integer (not `bool` or an
    enumeration) or a pointer to an object (an array decays): `operand of
    type T where arithmetic or pointer type is required` at the source.
    The alignment must be an integer (not `bool` or an enumeration; `used
    type T where integer is required`); when it is a constant it must be
    at least 1 (`requested alignment must be 1 or greater`), at most
    2^(N-1) for an N-bit source (pointers: the index width; `requested
    alignment must be M or smaller`) and a power of 2 (`requested
    alignment is not a power of 2`), and 1 is warned
    (`-Wtautological-compare`: `aligning a value to 1 byte is a no-op` /
    `... is always true`). The call has the source's unqualified type,
    `bool` for `__builtin_is_aligned`.
  - `__builtin_frame_address(level)` (an `I` constant, `check_builtin_ice_args`):
    `level` in [0, 65535] (`argument value V is outside the valid range`
    at the callee); a nonzero level warns `calling '__builtin_frame_address'
    with a nonzero argument is unsafe` (`-Wframe-address`, in `-Wall`).
    `__builtin_return_address(level)` (WP-432) is checked the same way,
    with its own name in the warning
    (`ext.gnu.builtins/return-address-args.cpp`).
    `__builtin_debugtrap()` takes no argument.
  - `__builtin_prefetch(addr [, rw [, locality]])` (id 976, `vvC*.`,
    `NoThrow, Const`; `check_prefetch`, clang `BuiltinPrefetch`; WP-610):
    `addr` converts to `const void *` as a parameter; more than three
    arguments is `too many arguments to function call, expected at most
    3, have N` at the `)`; a non-dependent `rw` / `locality` must fold
    (`argument to '__builtin_prefetch' must be a constant integer`, at the
    callee) into [0, 1] / [0, 3] (`argument value V is outside the valid
    range [0, M]`, at the callee); a dependent one is checked at
    instantiation. The call is `void` and never a constant expression
    (the Oven traps it; `ext.gnu.builtins/prefetch-{args,template-args,constexpr}.cpp`).
  - `__builtin_memcpy_inline(dst, src, n)` (id 2176, `vv*vC*Iz`,
    `NoThrow`) and `__builtin_assume_separate_storage(a, b)` (id 2177,
    `vvCD*vCD*`, `NoThrow, Const`) are ordinary signature builtins
    (WP-726): the operands convert to the parameter types, a non-constant
    `n` is `argument to '__builtin_memcpy_inline' must be a constant
    integer`, and neither call is a constant expression (clang 19 and 22
    evaluate neither); `__has_builtin` answers 1 for both
    (`ext.gnu.builtins/memcpy-inline-{run,args}.cpp`).
  - `__builtin_unpredictable(x)` (id 2178, `LiLi`, `NoThrow, Const`;
    WP-726) is an ordinary signature builtin whose value is its converted
    operand; like clang 19 and 22 it is not a constant expression
    (`ext.gnu.builtins/unpredictable.cpp`). Codegen emits the operand and,
    above -O0, the `!unpredictable` hint on the branch or `switch` it
    controls (spec/codegen.md, `__builtin_expect`; WP-779).
  - `__builtin_elementwise_add_sat(x, y)` / `_sub_sat(x, y)` (custom,
    `vector_ops.rs` `builtin_elementwise_sat`; WP-432). At 22
    (`BuiltinElementwiseMath` with clang 22's integer restriction): both
    operands are lvalue-converted (arrays decay, no promotion); the first
    must be a scalar or vector of integer type (`bool` and an unscoped
    enumeration count; `1st argument must be a scalar or vector of
    integer types (was T)`), then the second must have the same
    unqualified type (`arguments are of different types`). At 19: `max`
    / `min`'s conversions and checks, then the (element) type must be an
    integer (`1st argument must be a vector of integers (was T)`). The
    call has the (common) operand type.
    `ext.gnu.builtins/elementwise-sat-{args,constexpr}{,-clang22}.cpp`.
  - `__builtin_elementwise_abs(x)` / `_max(x, y)` / `_min(x, y)` (custom,
    `vector_ops.rs`; WP-416), by compatibility level. At 22 (clang 22
    `BuiltinElementwiseMath`): the operands are lvalue-converted (arrays
    decay) and keep their types; `abs` takes a signed integer, floating or
    vector of them (`1st argument must be a scalar or vector of signed
    integer or floating-point types (was T)`; kiln also rejects a pointer,
    class or scoped enumeration operand clang 22 lets through), `max` /
    `min` an integer, floating or vector operand (`... must be a vector,
    integer or floating-point type`) and two operands of the same type
    (`arguments are of different types`). At 19: the usual unary
    conversions first, then for `max` / `min` the usual arithmetic
    conversions when both operands are arithmetic, then the same-type
    check and clang 19's wording (`... signed integer or floating point
    type`, `... vector, integer or floating point type`).
    `tests/conformance/ext.gnu.builtins/elementwise-args*.cpp`,
    `align-frame-address-args.cpp`.
  - `__builtin_object_size` / `__builtin_dynamic_object_size`: the type
    argument (an `I` constant) must be in [0, 3] (`argument value V is
    outside the valid range [0, 3]` at the callee), checked after
    instantiation when value-dependent. Constant evaluation of both
    builtins (assume_aligned's alignment check, object_size's cast and
    side-effect rules, WP-292) is spec/oven.md "library builtins".
  - `__builtin_verbose_trap(category, reason)` (`check_verbose_trap`):
    each argument must be a pointer to a constant string: a string
    literal (no `$`: `argument to __builtin_verbose_trap must not contain
    $`) or a constant-foldable pointer; anything else `must be a pointer
    to a constant string`. A non-literal constant's characters are not
    inspected for `$` (clang reads them).
  - `__builtin_shufflevector(v1, v2, i...)` (custom, `act_on_shufflevector`,
    clang `BuiltinShuffleVector`; a `ShuffleVectorExpr`, spec/ast.md):
    at least 2 arguments; `v1`/`v2` vectors (`first two arguments to
    __builtin_shufflevector must be vectors`) of the same unqualified type
    (`... must have the same type`); each index an integer constant
    (`index for __builtin_shufflevector must be a constant integer`) that
    is -1 (an undefined lane) or below twice the lane count (`index for
    __builtin_shufflevector not within the bounds of the input vectors;
    index of V found at position N`), all at the callee. The result is
    `v1`'s type, or a generic vector of the element type with one lane
    per index. Re-checked at instantiation when any argument is
    dependent. With exactly two arguments (WP-292) the second is a mask:
    an integer vector with `v1`'s lane count (else `first two arguments
    to '__builtin_shufflevector' must have the same type`), and the
    result is `v1`'s type; the `ShuffleVectorExpr` has no indices
    (`ext.gnu.builtins/shufflevector-mask-form.cpp`).
  - `__builtin_powi{,f,l}`, `__builtin_copysignf16` and the `*f128` math
    family are ordinary calls (table rows in spec/sema-templates.md §9.5).
  - The `__fp16` libm builtins (WP-403; `kiln_ast` `builtin::F16_INTR`
    and `F16_LIBM`, clang `Builtins.td`'s `f16` spellings): `ceil`,
    `floor`, `trunc`, `round`, `roundeven`, `rint`, `fabs`, `fma`,
    `fmax`, `fmin`, `frexp`, `canonicalize` (`const`) and `acos`, `asin`,
    `atan`, `cos`, `cosh`, `exp`, `exp2`, `exp10`, `fmod`, `ldexp`,
    `log`, `log2`, `log10`, `pow`, `sin`, `sinh`, `tan`, `tanh` (`f16`
    suffix, `__fp16` parameters and result, `frexpf16(__fp16, int*)`,
    `ldexpf16(__fp16, int)`), all `nothrow`. Declared by
    `builtin::by_name_at(name, clang_compat)`: `atan2f16`, `sincosf16`,
    `fmaximum_numf16` and `fminimum_numf16` exist only at
    `-fclang-compat=22` (`builtin::min_compat`; clang 19 lacks them, so
    a use is an undeclared identifier there), matching `__has_builtin` at
    each level. Folding: spec/oven.md "Builtins"; lowering:
    spec/codegen.md (`tests/conformance/c.math/f16-builtins*.cpp`).
  - WP-419: `__builtin_signbitf(float)` / `__builtin_signbitl(long
    double)` (clang 19 and 22; ordinary argument conversions, unlike the
    custom-checked `__builtin_signbit`), and clang 22's
    `__builtin_fmaximum_numf128` / `fminimum_numf128` / `sincosf128`
    (`__float128`, i.e. `long double` on AArch64) and
    `__builtin_sincospi{,f,l}` (`void(T, T*, T*)`, `nothrow`; only at
    `-fclang-compat=22`, `builtin::min_compat`).
  - WP-419: `__builtin_classify_type(e)` (custom-checked, one argument
    kept unconverted and unevaluated; GCC type classes, identical in
    clang 19 and 22): spec/sema-templates.md §9.4
    (`tests/conformance/expr.const/builtin-classify-type*.cpp`).
- **Format strings** (WP-282, WP-186; `format_string.rs`, clang
  `checkFormatStringExpr` / `CheckPrintfHandler`): for a call whose
  callee (a function or, counting `this`, a member function) has a
  `format` attribute other than `strftime`, the format argument `F` is
  classified through parentheses and casts: a null pointer constant,
  `__func__`-style predefined names, a `const` variable with an
  initializer (pointer: const pointer to const; array of const), a
  string literal plus an offset and `&lit[n]` are literals kiln does not
  check; an ordinary string literal is checked; a conditional operator
  is its taken arm when the condition is a constant, else the less
  literal of its arms; anything else is not a literal. A non-literal
  with `A` > 0 and no data arguments warns `format string is not a
  string literal (potentially insecure)` at the argument
  (`-Wformat-security`; for `printf` with the note `treat the string as
  an argument to avoid this`). For `printf` with `A` > 0 each checked
  literal has its conversions checked against the data arguments from
  `A` on. Supported: `%[flags][width][.precision][l|ll|z]conv` with
  flags `-+ #0`, width/precision digits or `*`, and `conv` one of `d i u
  o x X c s p f F e E g G a A` (`c s p` and the floating ones without a
  length). The specified type is `int`/`long`/`long long` (`d i c`),
  `unsigned` of each (`u o x X`), `size_t` / `ssize_t` for `z`
  (`'size_t' (aka 'unsigned long')`), `double` (floating), `char *`
  (`s`: a pointer to any character type) and `void *` (`p`: any pointer
  or `std::nullptr_t`). An argument is matched by its type before the
  default promotions (through `IntegralCast`/`FloatingCast`) and an
  unscoped enumeration by its underlying type: integers by promoted
  rank class ignoring signedness (`-Wformat-signedness` is off), `float`
  matches `double`. Diagnostics, all warnings:
  - `format specifies type 'X' but the argument has type 'Y'`
    (`underlying type` for an enumeration) at the argument
    (`-Wformat`);
  - `field width|precision should have type 'int', but argument has
    type 'Y'` at the `*` (a width one byte after it, as clang), then the
    walk stops (`-Wformat`);
  - `more '%' conversions than data arguments` at the conversion
    character, then the walk stops (`-Wformat-insufficient-args`);
  - `incomplete format specifier` at the `%` when the string ends in a
    specification, then the walk stops (`-Wformat`);
  - `invalid conversion specifier 'c'` at the character (control
    characters as `\xNN`) for a conversion clang rejects (the printable
    ones in `INVALID_CONVERSIONS`, `"`, `\`, a non-positional `$`):
    the next argument counts as used without being consumed; with none
    left the walk stops (`-Wformat-invalid-specifier`);
  - `data argument not used by format string` at the first unused
    argument (`-Wformat-extra-args`): across the literals of a
    conditional, at the latest first-unused argument of the literals
    that completed, and not at all if one of them used every argument
    (clang `UncoveredArgHandler`).
  `-Wformat` contains the other four and is in `-Wmost`. Not checked
  (the whole string is skipped, where clang has a diagnostic): other
  conversions and length modifiers (`h`, `hh`, `j`, `t`, `L`, `n`, `m`,
  `C`, `S`, `%q`), positional arguments, `%[` / `%{`, an embedded NUL,
  a missing `*` argument, wide strings, class and scoped-enumeration
  arguments, the `scanf` and other archetypes, the contents of a
  `const` variable (clang follows it, with `format string is defined
  here`), a format parameter forwarded to a `va_list` function, and
  `-Wformat-truncation`/`-Wformat-overflow`. Test:
  `tests/conformance/ext.gnu.builtins/format-security-specifiers.cpp`. Dependent calls are checked
  at instantiation.
### 5.5 `throw` (`except.rs`)
`act_on_throw(operand, loc)` ([expr.throw]) builds a `void`
`CXXThrowExpr`:
- With `-fno-exceptions` it is an error
  (`cannot use 'throw' with exceptions disabled`), except as
  "Exceptions disabled" below says.
- The exception object's type is the operand's, with arrays and
  functions decayed and top-level cv removed; it must not be (a
  pointer to) an incomplete type other than cv `void` (clang's
  `CheckCXXThrowOperand`).
- (WP-804) A class-type exception object (not a pointer) must not be
  abstract ("cannot throw an object of abstract type 'T'", with the
  pure virtual notes; [class.abstract]/3), and its destructor is
  potentially invoked ([except.throw]/5): an inaccessible one is
  "exception object of type 'T' has protected/private destructor" at
  the operand, and a deleted one is the deleted-use error
  (`mark_element_dtor_referenced`;
  `expr.throw/abstract-and-inaccessible-dtor.cpp`). A non-trivial one is
  odr-used, so an implicit one is defined in the translation unit, since
  `__cxa_throw` is passed its complete-object destructor (WP-799,
  `except.throw/rethrow-copy-implicit-dtor-run.cpp`).
- The operand is copy-initialized into it after the implicit move
  ([class.copy.elision]/3), which applies only when the named local's
  scope does not extend past the innermost enclosing try-block
  (`thrown_var_in_scope`, WP-100).
- In `?:`, a throw operand gives the other operand's type and value
  category ([expr.cond]/2).
- The Oven treats a `throw` it reaches as not a constant expression,
  whatever the flag; codegen lowers it (spec/codegen.md §7b).

**Exceptions disabled** (WP-607; `Sema::check_exceptions_enabled`, shared
by `act_on_throw` and `act_on_try`). With `-fno-exceptions` a `throw`
expression or `try` block is diagnosed at its keyword except:
1. *System headers* (clang `ActOnCXXThrow` / `ActOnCXXTryBlock`): a
   location whose expansion is in a system header — a header reached
   through `-isystem`, the `--gcc-toolchain` / resource-dir include
   directories, the rest of a file after `#pragma GCC system_header`, or
   a line marker with flag 3 — is not diagnosed, at its definition or in
   any instantiation. A system macro expanded in user code is diagnosed
   (the expansion location is the user file's). This is what lets every
   `-fno-exceptions` TU include libstdc++'s `<exception>`, whose
   uninstantiated `std::throw_with_nested` holds a raw `throw`.
2. *Templates* (GCC: `build_throw` returns a dependent `THROW_EXPR` while
   `processing_template_decl`): inside a template definition
   (`in_template()`: a function template, a member of a class template, a
   generic lambda's body, any of those nested) the check waits for the
   instantiation; a never-instantiated template is accepted. The
   substitution calls `act_on_throw` / `act_on_try` again outside the
   template for a rebuilt node (a dependent operand, every try-block); a
   non-dependent throw-expression the substitution shares instead is kept
   in `exc_disabled_pattern` and reported by `note_shared_throws` when an
   instantiation shares it. Either way the error sits at the pattern's
   keyword with the usual instantiation notes, and a location is
   reported at most once (`exc_disabled_seen`), at its first
   instantiation. clang 19 and 22 instead diagnose a template's `throw`
   / `try` at its definition (and a dependent `throw` again per
   instantiation); the union of the two compilers' acceptance is what
   code built with both needs, so kiln accepts the never-instantiated
   case as GCC does.
3. *Codegen*: a surviving `throw` lowers to the ordinary
   `__cxa_allocate_exception` / `__cxa_throw` sequence as plain calls
   without landing pads, and a surviving `try` to its compound statement
   with the handlers dropped (no landing pad reaches them), byte-identical
   to clang (`tests/codegen/throw_no_exceptions.cpp`).

Tests: `tests/conformance/expr.throw/no-exceptions-*.cpp`,
`tests/conformance/except.pre/try-no-exceptions-*.cpp`.

**Implicit move in `return`** (`stmt.rs` `return_operand`, WP-669;
[class.copy.elision]/3, [expr.prim.id.unqual]/4). From C++23 (P2266)
an operand naming an implicitly movable entity is an xvalue. Before
C++23 the copy-initialization of the result is first tried, in a SFINAE
frame, with the operand as an xvalue; when that fails other than by
selecting a deleted constructor or function, the operand is used as the
lvalue it is (clang's `PerformMoveOrCopyInitialization`): `T& f(T&& t)
{ return t; }` and a class with only a `C(C&)` copy constructor are
accepted in C++20. `decltype(auto)` of `return (x);` is then `T&`.
Test: `tests/conformance/class.copy.elision/p3-cxx20-two-stage.cpp`.
The "possibly parenthesized id-expression" of the implicit move, of the
NRVO candidate and of a thrown local (`implicit_move`,
`copy_elision_candidate`, `thrown_var_in_scope`) is found with clang's
`Expr::IgnoreParens` (`Sema::ignore_parens`, as in clang
`getNamedReturnInfo` and `IsThrownVarInScope`): parentheses, GNU
`__extension__`, and a resolved `_Generic` or `__builtin_choose_expr`
(their selected operand), but not a result- or condition-dependent one,
which is looked through once instantiation resolves it. The xvalue
`ImplicitCastExpr <NoOp>` wraps the whole operand (the selection stays
an lvalue under it), and the variable stays an NRVO candidate (WP-1176).
Tests: `tests/conformance/class.copy.elision/implicit-move-through-*.cpp`,
`nrvo-through-generic-selection-ir.cpp`.

### 5.6 `typeid` (`rtti.rs`)
`act_on_typeid(Ok(expr) | Err(type), loc, rparen)` ([expr.typeid]):
an lvalue of type `const std::type_info`.
- `std::type_info` must have been declared (clang's
  `you need to include <typeinfo> before using the 'typeid' operator`),
  and RTTI enabled.
- References and top-level cv are ignored; a cv-qualified expression
  operand gets a `NoOp` cast to its unqualified type, as in clang.
- A class type must be complete (`incomplete type '…' used in typeid`
  at the `typeid` keyword, with the forward-declaration note). A
  type-id operand naming a dependent class that has no definition and
  is neither a class template's pattern nor one of its specializations
  (a local class or a member class of the current instantiation that is
  only declared) is diagnosed in the template definition, as clang's
  `BuildCXXTypeId` does (`is_incomplete_dependent_record`); an
  injected-class-name, a template-id and an expression operand wait for
  instantiation. GCC 16.1 diagnoses only at instantiation; kiln follows
  clang (WP-882, `expr.typeid/incomplete-in-template.cpp`).
- A glvalue of polymorphic class type is potentially evaluated
  (`typeid_expr`, sema-class.md §5.1); other operands are unevaluated.
- `-Wpotentially-evaluated-expression` (WP-219, `side_effects.rs`
  `check_typeid_side_effects`; in `-Wunevaluated-expression`): a
  potentially evaluated operand with possible side effects (as for
  `-Wunevaluated-expression`, but calls count) warns `expression with
  side effects will be evaluated despite being used as an operand to
  'typeid'` at clang's expression location. Not in instantiations, not
  for instantiation-dependent operands.

### 5.7 `dynamic_cast`
Parsed; `act_on_cast` reports `dynamic_cast` as not supported
yet (it needs polymorphic classes).

### 5.8 GNU extensions (`gnu.rs`)
Kiln accepts these GNU extensions in C++ and matches clang's behaviour
for them.

- **Statement expressions** `({ ... })`:
  - `check_stmt_expr_allowed` rejects them outside a function body
    (`statement expression not allowed at file scope`).
  - `act_on_start_stmt_expr` sets aside the enclosing
    full-expression's pending odr-uses (§5.2), and `act_on_stmt_expr`
    restores them.
  - It also sets aside the enclosing full-expression's pending
    cleanups (`Sema::cleanup`, `class/temporary.rs`).
    `act_on_stmt_expr` restores them, merged with whatever is left
    inside (clang's `ParentCleanup`, `Cleanup.mergeFrom`). Each statement inside is a full-expression
    of its own. Without this, the first one would take the outer
    cleanups and wrap them in its `ExprWithCleanups`. A temporary
    before the statement expression (`D{1}.v + ({ 0; })`) would then
    live to the end of the enclosing block instead of its
    full-expression (WP-1081;
    `tests/conformance/ext.gnu.stmt.expr/temporary-before-stmt-expr-run.cpp`).
  - The final expression statement goes through
    `act_on_stmt_expr_result`: decay, then copy-initialization of a
    prvalue of its cv-unqualified type. A class prvalue is kept as is;
    a class glvalue is copied into the result by its copy (or move)
    constructor (clang `InitializeStmtExprResult`; `arm_neon.h`'s
    `vld2q` macros end in a local struct). Codegen emits that copy
    into the destination inside the statement expression's scope,
    before its locals are destroyed, and the oven initializes a class
    result object the same way (spec/landed/sema/WP-799.md).
  - The type is that of the last non-null statement, looking through
    labels, when that is an expression statement; otherwise `void`.
    The result is always a prvalue.
  - Dependence (clang `computeDependence(StmtExpr*, TemplateDepth)`):
    the type's dependence, plus the result expression's, plus value-
    and instantiation-dependence whenever it appears in a dependent
    context (`Sema::in_template`), less unexpanded-pack (a pack is
    not expanded across a statement expression).
  - An unused statement expression warns as clang's
    `isUnusedResultAWarning` does.
- **Unused results** (`unused_warning`, clang `isUnusedResultAWarning`
  / `DiagnoseUnusedExprResult`; WP-158): a functional, C-style or
  `static_cast` constructor conversion (`X(1);`) is checked through its
  operand, looking through temporary binding, materialization and
  cleanups; a constructor call warns only when the constructor or its
  class is `[[nodiscard]]` (`ignoring temporary created by a constructor
  declared with 'nodiscard' attribute[: msg]`, -Wunused-value). A braced
  or scalar functional cast warns `expression result unused`. A C-style
  cast written `(void*)` (exactly, not through a typedef) warns
  `expression result unused; should this cast be to 'void'?`
  (-Wunused-value, clang `warn_unused_voidptr`) with only the fix-it
  removing the `*` as its range (WP-203). An unused name is reported at
  its begin location, so a qualified name (`N::x;`,
  `::N::x;`, `N::v<int>;`) points at the start of its
  nested-name-specifier, as clang's `DeclRefExpr::getBeginLoc` (WP-318,
  `tests/conformance/expr.prim.id.qual/unused-value-qualified-name-column.cpp`).
  - **Locations**: a built-in subscript `a[i]` is reported at its `]`
    (clang `getRBracketLoc`; also as a comma's left operand and inside
    parentheses, where the parentheses are looked through), a unary
    operator at the operator, a member access at the member name, a
    binary operator at the operator; a subscript yielding a `volatile`
    glvalue, `(void)e` and a call of an overloaded `operator[]` returning
    a reference are not warned (WP-324; conformance test
    `stmt.expr/unused-value-subscript.cpp`).
  - `emit_unused` (WP-182) follows clang's order: a comparison is
    `-Wunused-comparison` only when it is the unused expression itself
    (`top_comparison`: not parenthesized, not a comma's right operand or
    a conditional's arm, which are `expression result unused`); an `==`
    / `!=` (built-in or overloaded) whose left operand is an lvalue (not
    a materialized temporary) adds the note `use '=' to turn this
    equality comparison into an assignment` / `use '|=' to turn this
    inequality comparison into an or-assignment`. A call to a `pure` or
    `const` function with a non-`void` result warns `ignoring return
    value of function declared with pure|const attribute` [-Wunused-value]
    at the call's start. Nothing but `nodiscard` is diagnosed for an
    expression spelled in a macro body (clang `ShouldSuppress`).
  - **Through a cast (WP-461)**: an explicit cast (`static_cast`,
    `const_cast`, `reinterpret_cast`, C-style, functional) of kind
    `NoOp`, `ConstructorConversion` or `IntegralCast` whose operand,
    implicit casts stripped (clang `IgnoreImpCasts`, not parentheses),
    is a call or a construction is checked as that call
    (`unused_call_result`: nodiscard type, nodiscard callee, then
    `pure` / `const`) or construction (`unused_construct_result`), at
    the cast: `static_cast<bool>(s);` calling a `[[nodiscard]]`
    conversion function warns `-Wunused-result`, `static_cast<int>(t)`
    through a `pure` `operator long` warns `pure`. Otherwise the cast is
    `expression result unused` (`bool{s};`: its operand is an init
    list). An overloaded comparison that is not the unused expression
    itself (parenthesized, a comma's right operand, a conditional's
    arm; `unused_comparison_call`) is checked as that call, at its
    operator, before falling back to `expression result unused`
    (`tests/conformance/dcl.attr.nodiscard/through-cast.cpp`).
  - **In an instantiation (WP-217)**: clang rebuilds every expression
    statement and for-increment (`TransformExprStmt` → `ActOnExprStmt`),
    so the warning of a non-dependent one is reported again in each
    instantiation, with its notes: `emit_unused` marks it
    `allow_duplicate` (spec/diag.md §7.4) inside an instantiation. Not
    the comma's left operand (checked once, as the comma is built). A
    deprecated use is not re-diagnosed (the shared node is not rebuilt).
    The warning's range is the whole expression where clang's is the
    callee (a caret difference only).
  - **Comma (WP-182)**: an unused comma is checked through its right
    operand, except that an integer literal `0` there (parentheses looked
    through: `r = 1, 0;`, the idiom hiding an assignment's result) is not
    a warning (clang `isUnusedResultAWarning`, WP-458;
    `expr.comma/unused-zero-idiom.cpp`). The left operand of a built-in
    comma is checked
    the same way when the comma is built (clang `CheckCommaOperands`),
    with the generic warning spelled `left operand of comma operator has
    no effect` [-Wunused-value] (not in a SFINAE context). Nothing is
    checked in an unevaluated operand or for a type-dependent operand
    (it is checked on instantiation, with the instantiation note), and
    an `[[assume(...)]]` argument is checked like any other expression.
  - **Reachability (WP-203)**: the generic warnings (`expression result
    unused`, `left operand of comma operator has no effect`) go through
    `diag_if_reachable` (clang `DiagIfReachable`); the comparison,
    `nodiscard`, `pure`/`const` and `(void*)` warnings are emitted at
    once. In a function body the warning is queued on
    `FnState::runtime_diags` with its expression as anchor; at the end
    of the body, when no error occurred and outside a template, those
    whose anchor the reachability walk (`unreachable_exprs`, the
    structured CFG mirror of §7.5) meets only on dead paths are dropped
    (`return; x;`, `if (0) x;`, the unmatched case of a constant
    switch, the pruned arm of `true ? a : (x, 1)`); a template's are
    emitted unfiltered and an instantiation filters its own; an `if
    consteval` in the body (no CFG) keeps them all. Outside a function
    the warning is not emitted in the initializer of a `constexpr`
    variable or of a static data member's first, non-inline declaration
    (clang's `ManglingContextDecl` test, read from
    `lambda_mangle.var`); otherwise at once.
  - The warning carries the expression's range, so a macro-argument
    location gets clang's `expanded from macro` note (spec/diag.md §8.4).
  - The run-time diagnostics of §5.15 use the same queue and are
    dropped in the same contexts (§5.12, `runtime_diag_quiet`).
  - In a default argument, the parser reports
    `default argument may not use a GNU statement expression`.
- **Unused variables (WP-203; clang `ShouldDiagnoseUnusedDecl` /
  `DiagnoseUnusedDecl`)**: `unused variable 'x'` [-Wunused-variable, in
  -Wunused and so -Wall; off by default] at the variable's location.
  - When a block scope is popped (`pop_scope`; the function-body scope
    from `end_function_body`, before the queued runtime warnings, as
    clang leaves the body scope before `ActOnFinishFunctionBody`), its
    unreferenced local variables are diagnosed sorted by location
    (`unused_var.rs`, `diagnose_unused_locals`), unless an error was
    reported since the scope was entered (`Scope::errors`, clang
    `hasUnrecoverableErrorOccurred`) or the scope belongs to an
    instantiation.
  - Not diagnosed: an invalid or unnamed variable; one referenced or
    used (`sizeof(x)` counts); one marked `[[maybe_unused]]` /
    `__attribute__((unused))`, or whose typedef or class is; a `_`
    placeholder with automatic storage; a variable outside a function,
    `extern` or a handler's exception variable; an init-capture's
    variable (referenced by its capture; clang reports an unused one only
    as `-Wunused-lambda-capture`, spec/landed/sema/WP-865.md; it can
    still be `-Wunused-but-set-variable`; WP-703,
    `expr.prim.lambda.capture/init-capture-not-unused-variable.cpp`);
    one of dependent or incomplete type; one whose (array element) class has a non-trivial
    destructor, or is initialized by a non-trivial constructor call that
    is not a constant initialization (a value referring to an automatic
    object, constant only by P2686R5, counts as not constant, as in
    clang 22; a guard for when the §4.9 constexpr-only narrowing is
    lifted: WP-467, `stmt.dcl/unused-variable-ptr-to-automatic.cpp`),
    or by a type-dependent initializer. A reference lifetime-extending a
    temporary is judged by the temporary's type.
  - A variable of dependent type is diagnosed as its instantiation is
    created (`diagnose_unused_inst_var`, from `subst` `var_decl`, clang
    `TemplateDeclInstantiator::VisitVarDecl`) by the pattern's
    referenced flag, with the instantiation note.
  - Structured bindings (WP-217): a structured binding declaration is
    diagnosed at its `[`, named `[a, b]`, when none of its bindings is
    referenced or marked unused (and not all are placeholders), whatever
    its storage or reference kind; one of dependent type is diagnosed as
    it is instantiated.
  - A class-typed variable with a type-dependent initializer (WP-217) is
    diagnosed in the template definition unless its class declares a
    non-trivial constructor or the initializer is an unresolved
    construction with dependent arguments.
  - `-Wunused-exception-parameter` (WP-217, clang
    `warn_unused_exception_param`, off by default): a named exception
    variable never referenced, when its handler scope is left; not a `_`,
    `[[maybe_unused]]` (the parser takes attributes on the handler's
    declaration) or one whose class has a non-trivial destructor (a
    reference to one still counts).
  - `-Wunused-but-set-variable` (WP-217, clang `DiagnoseUnusedButSetDecl`
    / `RefsMinusAssignments`, in -Wunused): `variable 'x' set but not
    used` for a local whose every reference is the target of a
    discarded-value assignment, compound assignment or increment (an
    expression statement, a comma's left operand, a for-increment, an
    operand cast to void). `Sema::refs_minus_assign` counts references
    minus such targets. Not for a reference, class, dependent,
    `[[maybe_unused]]` or `_` variable, a parenthesized assignment, or a
    compound assignment/increment of a volatile one; diagnosed with the
    unused variables in location order. A reference an instantiation
    shares with its pattern counts again (`mark_shared_expr_referenced`,
    clang `TransformDeclRefExpr`), balancing the assignment its
    instantiated statement subtracts: a variable captured by a generic
    or explicit-object lambda and assigned in its body is not diagnosed
    (WP-162, test `stmt.dcl/unused-but-set-variable-generic-lambda.cpp`).
  - Namespace-scope variables (WP-217, `unused_file_scope.rs`, clang
    `ShouldWarnIfUnusedFileScopedDecl`): at the end of the translation
    unit, a variable with internal linkage declared in the main file and
    never referenced is `unused variable 'x'` [-Wunused-variable], or
    `unused variable 'x'` [-Wunused-const-variable, in -Wunused-variable]
    when `const`, in declaration order and none once an error occurred;
    not a variable template, a static data member, or one referenced but
    not odr-used.
  - Parameters (WP-234): a parameter that is only assigned is
    `parameter 'p' set but not used` [-Wunused-but-set-parameter, in
    -Wextra], by the same rule as a variable (never a `_` placeholder).
    The parameters' warnings come first, in location order, then the
    body's (clang reports them in `ActOnFinishFunctionBody` before the
    body scope is popped).
  - Unneeded file-scope variables (WP-234, clang
    `ShouldRemoveFromUnused`): an internal variable that is referenced
    but not odr-used is `variable 'x' is not needed and will not be
    emitted` [-Wunneeded-internal-declaration, in -Wunused-function and
    -Wunused-template]; an unused variable template is `unused variable
    template 'x'` [-Wunused-template]. Static data members of classes
    with internal linkage, and out-of-line explicit specializations of
    class template members, are diagnosed at their definition; inline
    variables in the main file are diagnosed as clang does. A variable
    named by a non-dependent expression of a template, which the
    instantiation shares rather than rebuilds, is odr-used by the
    instantiation at its full-expression (`mark_shared_expr_referenced`
    queues it on `pending_odr`; WP-418,
    `tests/conformance/basic.def.odr/instantiation-odr-uses-variable.cpp`).
  - Gaps: a variable named only in a default argument counts as
    odr-used, so it is not reported as unneeded; C++26 `_` placeholder
    redeclarations ("redefinition of '_'").
- **`__builtin_offsetof(T, designator)`** (`act_on_builtin_offsetof`
  with `OffsetofComponent`s; clang's `BuildBuiltinOffsetOf`): a
  `size_t` prvalue `OffsetOfExpr`.
  - The byte offset is computed from `kiln_abi` layout.
  - `T`, and every class it steps into, must be a complete class
    type; a bit-field member is an error. Array indices must be
    integral constants; a non-constant index, and a result outside
    0 to 2^32 - 1 (which a negative index gives), are unsupported.
  - (WP-716) A member is found by class member lookup, so through
    base classes too ([class.member.lookup]; an ambiguous name is
    reported as for a member access). The offset of the base
    subobject along the unique path is added; a member of a virtual
    base is clang's error `invalid application of 'offsetof' to a
    field of a virtual base` (`ext.gnu.offsetof/base-member*.cpp`).
  - For a class that is not standard-layout it warns
    `-Winvalid-offsetof`. Standard-layout is computed from members
    and bases (`is_standard_layout`) until the definition bits track
    it.
  - (WP-307) When `T` is dependent or an array index is type-, value-
    or instantiation-dependent, nothing is checked: the result is a
    value- and instantiation-dependent `OffsetOfExpr`
    (`Builder::dependent_offsetof`) whose children are the index
    expressions and whose type and components are kept in
    `ColdExprData::offsetof_pattern`; substitution rebuilds it
    (sema-templates.md §5.4; `ext.gnu.offsetof/dependent*.cpp`).
  - (WP-417) Through a record whose definition is invalid, the
    `OffsetOfExpr` is marked (`ExprArena::offsetof_invalid_record`):
    the Oven fails it as not a constant expression with no note, and a
    constexpr object of such a record stays silent, as clang
    (`ext.gnu.offsetof/invalid-record.cpp`).
- **`__builtin_choose_expr(c, a, b)`** (`act_on_choose_expr`, clang
  `ActOnChooseExpr`; WP-307): `c` must be an integral (or unscoped
  enumeration) constant expression, else `'__builtin_choose_expr'
  requires a constant expression` at `c` with the evaluation's notes
  (no class-type contextual conversion yet). It is wrapped in a
  `ConstantExpr`; nonzero picks `a`. The `ChooseExpr` takes the chosen
  operand's type, value category and object kind with no conversion,
  and its type/value dependence (other dependence bits from all three).
  A type- or value-dependent `c` gives a `<dependent type>` prvalue
  until substitution. `ignore_parens` looks through it to the chosen
  operand, as clang's `IgnoreParens`; codegen and the Oven emit and
  evaluate only the chosen operand
  (`ext.gnu.builtins/choose-expr*.cpp`).
- **`_Generic` selection** (`act_on_generic_selection` in
  `generic_sel.rs`; clang `CreateGenericSelectionExpr`; C11 6.5.1.1, a
  clang extension in C++; WP-1114).
  - **Controlling operand.**
    - An expression operand is unevaluated. It gets the lvalue,
      array-to-pointer and function-to-pointer conversions
      (`conv::decay`): qualifiers are dropped, except that a class
      prvalue keeps its cv-qualifiers.
    - A side effect in that operand is warned, unless the operand is
      dependent (-Wunevaluated-expression).
    - A type operand is matched as written.
  - **Association types**, checked in clang's order:
    - The incomplete-type check applies to an expression operand only.
      It is an error under `-fclang-compat=19`, and the C2y extension
      `incomplete type 'T' in a '_Generic' association is a C2y
      extension` at 22: two ids, because a `_19` compat row must keep
      its kind.
    - `not an object type` (a function, a reference or `void`) also
      applies to an expression operand only.
    - A variably modified type is an error.
    - With an expression operand, an array type or a qualified
      non-class type `will never be selected`
      [-Wunreachable-code-generic-assoc, under -Wunreachable-code]. That
      warning never invalidates the selection, even as an error.
    - Two associations of the same type are an error: `type 'U' in
      generic association compatible with previously specified type
      'T'`, with the note `compatible type 'T' specified here`. In C++,
      "compatible" means the same type.
  - **Selection.** The association whose type is the controlling type
    is selected, else `default`.
    - More than one match is an error: `controlling expression type 'T'
      compatible with N generic association types`, with one note per
      match.
    - No match and no `default` is an error: `controlling expression
      type 'T' not compatible with any generic association type`.
    - Both errors are reported at the operand's begin.
  - **Result.** The `GenericSelectionExpr` (`Builder::generic_selection`)
    takes the selected expression's type, value category and object
    kind with no conversion, and its type/value dependence, as
    `ChooseExpr` does.
  - **Dependent selections.** A type-dependent operand, or any dependent
    association type, leaves a result-dependent `<dependent type>`
    prvalue. Substitution rebuilds it through `act_on_generic_selection`,
    so every check runs at instantiation.
  - **Looking through the selection.** `ExprArena::selected_operand` is
    the selected expression. These all look through it, as clang's
    `IgnoreParens` does: `ignore_parens`, the null pointer constant
    test, bit-field references, the static_assert "evaluates to" note,
    the Oven and codegen. Only the selected association is evaluated
    and emitted (`ext.c11.generic/`).
- **Binary conditional** `x ?: y` (`act_on_binary_conditional`, in
  `forms.rs`): a `BinaryConditionalOperator` with `x` shared through an
  `OpaqueValueExpr`. Unless both operands are glvalues of the same
  type, `x` gets the usual unary conversions first.
- **Compound literals** `( T ) { ... }` (`act_on_compound_literal`;
  clang's `BuildCompoundLiteralExpr`): in C++ a prvalue (in C an
  lvalue, §1.0.1) `CompoundLiteralExpr` of the written type, holding the
  list-initialization of cv-unqualified `T`, done as if by a C-style
  cast at the `(`. clang's diagnostics follow from that:
  - a reference type cannot bind to an initializer list;
  - more than one element for a scalar is a function-style cast
    error, except for a `_Complex` type, whose list takes the real and
    imaginary parts (`(_Complex float){x, 2}`; a third element is
    `excess elements in scalar initializer`; WP-312,
    `tests/conformance/ext.gnu.compound.literal/complex-diagnostics.cpp`);
  - a scalar list narrows as any list-initialization does
    ([dcl.init.list]/7): `(bool){i}` for an `int`, floating or pointer
    `i`, or a constant other than 0 and 1, is the narrowing error
    (`tests/conformance/ext.gnu.compound.literal/bool-narrowing-diagnostics.cpp`);
  - an incomplete type is diagnosed with the forward-declaration note.

  - a class literal (WP-191) is list-initialized as a direct-list
    initialization (`list_construct`, `InitStyle::DirectList`): a
    class with constructors holds a `CXXTemporaryObjectExpr`, an
    aggregate its `InitListExpr`; an abstract class is an error, and a
    class with a non-trivial destructor wraps the literal in a
    `CXXBindTemporaryExpr`;
  - a dependent literal, including an array literal whose element
    type or bound is dependent, holds the list as written (an
    `InitListExpr` of type `void`). Substitution rebuilds it through
    `act_on_compound_literal`, so every check above runs per
    instantiation (WP-235).
  - a literal at namespace scope (no enclosing function; this includes
    a default member initializer and a variable template's
    initializer) is [GNU] and must have a constant initializer (C99
    6.5.2.5p3; clang's `CheckForConstantInitializer`). Otherwise the
    error is "initializer element is not a compile-time constant",
    reported at the culprit (WP-235):
    - for an aggregate list, the first element, recursively, that does
      not fold (`try_fold`; implicit value-initializations are
      constant). The location is that of clang's `getExprLoc`: the
      operator of a binary or member expression, otherwise the first
      token;
    - for a constructor call, the type-id after the `(`;
    - for a scalar or vector literal, its `{`.

    A dependent literal is checked when instantiated.

    The rules above are clang 19's. At `-fclang-compat=22` (WP-413,
    `gnu.rs` `element_culprit_loc`) three things change:
    - the culprit is the element itself, without looking through
      parentheses or casts, at clang's `getExprLoc` (`expr_loc`). An
      implicit conversion, including an lvalue-to-rvalue one, is located
      where it begins, so `p.a` is reported at `p` and `x = 3` at `x`;
    - a scalar, vector or complex literal is reported at its first
      non-constant element instead of the `{`;
    - a literal whose type is dependent but whose written elements are
      not value-dependent is checked at the template definition (written
      elements, at `expr_loc`). A failure makes the literal an error
      expression, so its instantiations do not report it again.

    Tests: `ext.gnu.compound.literal/namespace-scope-diagnostics{,-clang19}.cpp`.

    A valid
    non-array literal is a prvalue, as it is inside a function, and its
    variable is constant-initialized.

    A valid array literal (WP-466; clang `BuildCompoundLiteralExpr`,
    `isFileScope`) is an lvalue naming an unnamed object of static
    storage duration (C99 6.5.2.5p5): `file_scope_compound_literal`
    creates an implicit unnamed `Var` (static storage, the literal's
    initializer, its context the enclosing namespace even for a
    class-scope literal) and records it in `DeclTable::compound_literal`
    (`compound_literal_of`; `ExprTable::compound_literal_var` goes the
    other way, aux 1). A literal whose initializer is not constant
    becomes a recovery expression after the error. In the Oven the
    object is usable in a constant expression only when its type is
    const and not volatile ([expr.const]/4, as for a const variable),
    and is read through its initializer; a read of a non-const literal
    is "subexpression not valid in a constant expression" with the
    "declared here" note at the literal. Codegen: spec/codegen.md §7ab.
    Tests: `ext.gnu.compound.literal/namespace-scope-array-{run,diagnostics}.cpp`,
    `tests/codegen/file_scope_compound_literal.cpp`.

  Deviations:
  - A literal in a constexpr variable template or a class template's
    default member initializer, instantiated from a function body, is at
    file scope in kiln at every compat level, as in clang 19 and GCC:
    `file_scope` is `self.func.is_none()`, and an instantiation runs
    with the specialization's own (namespace or class) context and no
    enclosing function. clang 21+ also requires
    `!getCurScope()->isInCFunctionScope()` (llvm-project 977d8a4bcd83,
    #143852, a C fix for a literal inside a struct member declaration
    in a function body). During instantiation that is the *parser's*
    scope at the point of instantiation, not the template's, so the same
    specialization is a block-scope temporary when first instantiated
    from a function body and a file-scope object when first instantiated
    at namespace scope: clang 22 then warns `-Wdangling` (calling
    `p<int>` a "local variable", `q` a "reference member") and rejects
    the initialization ("must be initialized by a constant expression",
    or the reference-member error). That depends on where the first use
    is, which no rule supports, so it is treated as a clang defect and
    not followed at compat 22 (WP-490; docs/status/deviations.md). Test:
    `ext.gnu.compound.literal/variable-template-from-function-run.cpp`
    (`skip(clang)`; clang 19 and GCC 16 compile and pass it).
  - kiln does not wrap the elements of a namespace-scope literal in
    `ConstantExpr` nodes, as clang does.
- **Flexible array members** (`flex_array.rs`, WP-271; a C99 feature,
  clang `ActOnFields` / `CheckFlexibleArrayInit`). A non-static data
  member of array-of-unknown-bound type `T[]` is accepted when its
  element type is complete (`field_complete_type`; an incomplete or
  abstract element is diagnosed as for a bounded array). The member
  keeps the incomplete type; the class is complete and records
  `DefinitionBits::HAS_FLEXIBLE_ARRAY_MEMBER`. Layout: size zero at the
  aligned end of the preceding members, counting for the alignment
  (`sizeof(struct { int n; char d[]; }) == 4`); LLVM type `[0 x T]`;
  implicit copies copy only the other members. At the `}`
  (`check_flexible_array_members`, run per instantiation; each
  diagnostic is `per_instantiation`, spec/diag.md §7.4), over the
  fields in order:
  - not the last field of a struct/class: error `flexible array member
    'd' with type 'T[]' is not at the end of struct`, note `next field
    declaration is here` at the next field; field and class invalid;
  - in a union: `-Wgnu-flexible-array-union-member`; otherwise, when no
    named field precedes it: `-Wgnu-empty-struct`;
  - a class with a virtual base (direct or inherited): error `...not
    allowed in struct which has a virtual base class`;
  - always `flexible array members are a C99 feature` [-Wc99-extensions];
  - a non-dependent element type with non-trivial destruction: error
    `flexible array member 'd' of type 'T[]' with non-trivial
    destruction`; field and class invalid;
  - a field whose class has a flexible array member gives this class one
    too; in a struct/class it warns `field 'a' with variable sized type
    'A' not at the end of a struct or class is a GNU extension`
    [-Wgnu-variable-sized-type-not-at-end, on by default] when not last,
    else `'a' may not be nested in a struct due to flexible array member`
    [-Wflexible-array-extensions];
  - an array of such a class: `'A' may not be used as an array element
    due to flexible array member` [-Wflexible-array-extensions] at the
    `[` (the declarator's array chunk).

  Except where noted, the warnings are Extensions (`-pedantic`). A
  default member initializer of a `d[]` member is the parse error `array
  bound cannot be deduced from a default member initializer` (at the `=`
  or `{`; the member is invalid and gets no checks). In an aggregate
  initialization the member's initializer is: an empty list, the GNU
  extension `flexible array initialization is a GNU extension`
  [-Wgnu-flexible-array-initializer] with the note `initialized flexible
  array member 'd' is here` and `zero size arrays are an extension`
  [-Wzero-length-array]; elements, an error `initialization of flexible
  array member is not allowed` (same note) unless they initialize a
  variable without automatic storage directly (`Sema::init_var` with no
  `init_entity`). The Oven rejects the decay of a member subobject of
  unknown bound (`array-to-pointer decay of array member without known
  bound is not supported`, clang `addUnsizedArray`).

  Deviation: elements initializing a static variable (clang's extension,
  which emits the variable with a larger type) are reported as not
  supported yet.
- **Zero-length arrays** `T[0]`: `zero size arrays are an extension`
  [-Wzero-length-array, in -Wgnu] at the bound, per instantiation; an
  error in a SFINAE context (above).
- **`__null`** (`gnu_null.rs`; `NULL` in the resource `<stddef.h>`): a
  `GNUNullExpr` prvalue of the signed integer type as wide as a data
  pointer (`int`, `long` or `long long`; `long` on LP64, clang's
  `ActOnGNUNullExpr`), with value 0. It is a null pointer constant
  (`conv::is_null_pointer_constant`), so it converts to any pointer,
  pointer to member and `std::nullptr_t`; as an integer it converts like
  any `long` (so `g(int)`/`g(void*)` is ambiguous). Constant-evaluates to
  0; mangles as the literal `Ll0E`; prints as `__null`. Warnings (both on
  by default, not runtime-behavior diagnostics):
  - `implicit conversion of NULL constant to 'T'` [-Wnull-conversion, in
    -Wconversion; alias -Wconversion-null] for an implicit conversion to
    a scalar type other than its own, a pointer, a pointer to member or
    `std::nullptr_t` (clang's `DiagnoseNullConversion`, from the
    implicit-conversion walk, `impcast.rs`). It is reported at `__null`,
    or at the `NULL` macro name when `__null` came from `NULL`'s
    expansion; after taking both locations out of macro arguments, it is
    dropped when that location and the conversion's context are in
    different files or macro expansions (`#define MYNULL __null` does
    not warn). The same check covers an operand of type
    `std::nullptr_t` (`!nullptr`, `nullptr && b`, `if (nullptr)`, a
    `std::nullptr_t` variable), as `implicit conversion of nullptr
    constant to 'T'`, reported at the operand with no `NULL` macro step;
    a conditional operator source is checked arm by arm (clang's
    `CheckConditionalOperator`; WP-851,
    `conv.bool/nullptr-null-conversion.cpp`). kiln prints no `false`
    fix-it.
  - [-Wnull-arithmetic] (clang's `checkArithmeticNull`), at the operator
    of a built-in `+ - * / % << >> & ^ |`, compound assignment, or
    comparison with a `__null` operand (implicit casts and parentheses
    stripped), unless the other operand is a pointer to member or a
    function: `use of NULL in arithmetic operation` for the arithmetic
    operators; for a comparison, `comparison between NULL and non-pointer
    (NULL and 'T')` / `('T' and NULL)` when only one side is `__null`
    and the other is neither a pointer nor an array.
- **GNU `typeof`** (`gnu_typeof.rs`; parsing in spec/parse.md §4):
  `__typeof ( type-id )` is `TypeOf` sugar over that type
  (`act_on_typeof_type`), printed `typeof(T)`. `__typeof e` /
  `__typeof ( e )` (`act_on_typeof_expr`; clang
  `ActOnTypeofExpression`) is sugar over the unevaluated operand's
  expression type: an lvalue adds no reference (`__typeof(ri)` for
  `int& ri` is `int`), cv-qualifiers are kept, an id-expression or
  bit-field member gives its expression type, a function name its
  function type, and side effects are not diagnosed (unlike
  `decltype`). It prints `typeof (e)` (the parentheses are a
  `ParenExpr`). An erroneous operand or an unresolved overload set
  (`check_overload_use`: "reference to overloaded function could not
  be resolved; did you mean to call it?") makes the type specifier an
  error that combines with no other specifier. A type-dependent
  operand gives a dependent type whose canonical form is keyed on the
  operand's profile (clang `DependentTypeOfExprType`), rebuilt at
  substitution (`templ/subst/types.rs`) and mangled as clang's vendor
  type `u6typeof`; a dependent `__typeof(T)` is canonically `T`.
  `__typeof_unqual` is the same with the cv-qualifiers dropped
  (canonically, `ctx.unqualified`; `typeof_unqual (e)` prints).
  No diagnostics of its own (no pedantic warning, as clang).
  `tests/conformance/ext.gnu.typeof/`.

### 5.9 New- and delete-expressions (`new_delete.rs`, WP-061a)
clang's `ActOnCXXNew`/`BuildCXXNew` and `ActOnCXXDelete` for non-class
and aggregate element types.
- **Implicit global operators** ([basic.stc.dynamic.general]/2):
  `declare_global_new_delete` runs once, the first time a new- or
  delete-expression, a declaration of a global `operator new`/`delete`,
  or an unqualified or `::`-qualified id-expression naming one
  (`::operator new(n)`, WP-158) needs it (clang's
  `DeclareGlobalNewDelete`). It declares, as implicit
  TU-scope functions in clang's order, `operator new`/`new[]` taking
  `(size_t)` and `(size_t, std::align_val_t)`, and `operator
  delete`/`delete[]` taking `(void*)`, `(void*, align_val_t)`,
  `(void*, size_t)` and `(void*, size_t, align_val_t)`, all
  `noexcept`. `std::align_val_t` is the program's own declaration, else
  an implicit `enum class align_val_t : size_t` in an implicit
  namespace `std` (clang's `getStdAlignValT`). That implicit namespace
  is hidden from ordinary lookup, so `int std;` stays valid, until the
  program's first `namespace std {` reopens it (`implicit_std`, WP-958).
  The nothrow forms exist only if the program (`<new>`) declares them.
- **User declarations** of a global `operator new`/`delete`
  (`act_on_global_new_delete_decl`) redeclare a matching implicit one; a
  deallocation function without an exception specification is
  implicitly `noexcept` ([except.spec]/11).
- **The allocated type** ([expr.new]/1-2): not a function type, a
  reference or an incomplete type; `auto` is deduced from a single
  parenthesized or braced argument (`new auto(x)`; `new auto*(p)`
  deduces as `auto *x = p`, tests/conformance/expr.new/new-auto-pointer.cpp). The new-declarator's first bound is converted to `size_t`
  ([expr.new]/8). From C++14 that is an implicit conversion (clang
  `PerformImplicitConversion`, `AssignmentAction::Converting`): a class
  bound uses its non-explicit conversion functions (templates deduced
  against `size_t`); no usable one is `converting 'C' to incompatible
  type`, an ambiguity `conversion from 'C' to ... is ambiguous` with the
  candidate notes, a deleted one `attempt to use a deleted function`.
  Before C++14 it is the contextual implicit conversion to an integral or
  unscoped enumeration type (`switch_conv.rs`, as for `switch`, all
  diagnostics at the new-expression): `array size expression must have
  integral or {unscoped }enumeration type, not 'T'`, `ambiguous
  conversion of array size expression of type 'C' to an integral or
  enumeration type`, `array size expression of type 'C' requires
  explicit conversion to type 'U'`, `array size expression has
  incomplete class type 'C'`, with `conversion to {integral|enumeration}
  type 'U' declared here` notes (WP-374; tests
  `expr.new/array-bound-class-conversion{,-cxx11}.cpp`,
  `expr.delete/class-operand-conversion-run.cpp`). A constant first bound that is
  negative, too large, or smaller than the number of initializers is an
  error; later bounds are converted constant expressions. A `new T[]`
  takes its bound from the initializer. In a parenthesized type-id a
  non-constant bound is clang's warning `when type is in parentheses,
  array cannot have dynamic size` (see the gaps below).
- **Allocation function** ([expr.new]/12-19): `PASS_ALIGN` when
  `alignof(T)` exceeds `__STDCPP_DEFAULT_NEW_ALIGNMENT__` (16 on both
  targets, `kiln_abi::Target::default_new_align`). Lookup is in the
  allocated class first, then global (`::new` only global;
  [expr.new]/12, `alloc_fns`). A class-scope
  `operator new` / `operator delete` (`AllocFns::naming` is its class)
  is access-checked from the context of the expression ([expr.new]/19,
  [expr.delete]/12, clang `CheckAllocationAccess`): `'operator new' is
  a private member of 'P'` with the `declared private here` note, a
  substitution failure in a requires-expression (libc++
  `shared_ptr_Y.pass.cpp`; WP-335). Among the candidates: those whose
  leading parameters are `size_t` (and `align_val_t` when aligned) and
  that accept the placement arguments, preferring the most exact
  matches; with `PASS_ALIGN` a second round without the alignment
  argument follows (clang's two-phase lookup). Failure reports clang's
  `no matching function for call to 'operator new'` with candidate notes,
  or for a placement `new (p) T` with no `(size_t, void*)` form, `no
  matching 'operator new' function for non-allocating placement new
  expression; include <new>`.
- **Deallocation function for new** ([expr.new]/28): for a placement
  form, the one with the same extra parameters; otherwise the sized usual
  one (with `align_val_t` when aligned).
  **Placement selection (P3769R1, Croydon DR; WP-757;
  `Sema::matching_operator_delete`, `new_delete.rs`).** For a placement
  form, every candidate found by the lookup whose parameter types after
  the first match the allocation function's is collected: a non-template
  directly, a template through deduction from those parameter types (so
  `template<int = 0> void operator delete(void*, Tag)` matches). With
  more than one match, those whose constraints are not satisfied are
  dropped (clang does not filter non-templates; kiln does, as the
  wording's "suitable" requires). Exactly one left is the deallocation
  function; none means none is called. Several left is clang's
  `-Wambiguous-delete` warning at the new-expression, "multiple suitable
  'operator delete' functions for 'T'; no 'operator delete'
  function will be invoked if initialization throws an exception", with a
  "member 'operator delete' declared here" note per candidate in
  declaration order (`T` the allocated type), and no deallocation
  function. A candidate with an extra trailing parameter that has a
  default argument does not match. In
  C++14 and later a single match that is a usual (sized) deallocation
  function of a placement new-expression is clang's error "'new'
  expression with placement arguments refers to non-placement 'operator
  delete'" with an "'operator delete' declared here" note, unless a
  class-scope unsized usual form corresponds to it. All of this runs
  only with exceptions enabled (`Sema::exceptions`): then the selected
  function is access-checked, a deleted one is the use-of-deleted error,
  and it is odr-used. Tests:
  `tests/conformance/expr.new/p3769-placement-delete-{selection-run,deleted}.cpp`.
- **Initializer** ([expr.new]/23): none is default-initialization (a
  `const` scalar or array is an error: `default initialization of an
  object of const type 'const int[]'`, the array type printed for array
  new); `()` is an `ImplicitValueInitExpr`; `(x)` copy-initializes a
  scalar (more than one argument is clang's `excess elements in scalar
  initializer`); `{...}` is list-initialization (§6) of
  `T` or of the array `T[n]`, with an `array_filler` for the tail.
  A `const` class array is fine when the element class is
  const-default-constructible ([dcl.init.general]/7: a user-provided
  default constructor, or every member initialized;
  `CAN_CONST_DEFAULT_INIT` once the class is complete), so `new const
  A[n]` with `A() {}` is accepted and `new const P[n]` for an aggregate
  `P { int x; }` is the error above (WP-336).
  A class element's default- or value-initialization
  (`new_class_default`) always selects the default constructor, so a
  deleted, implicitly deleted, ambiguous or inaccessible one is
  diagnosed; no `CXXConstructExpr` is built only when the selected one
  is trivial (then `()` is an `ImplicitValueInitExpr`). A class-wide
  "has a trivial default constructor" bit is not enough: since
  [special]/6 triviality is per eligible member, a class can have a
  trivial and a non-trivial or deleted one (spec/sema-class.md §4.3,
  §4.7.2; WP-1109).
  Class element types report "not supported yet" for default- and
  parenthesized initialization (they need `CXXConstructExpr`,
  WP-067b); braced aggregate initialization works.
- **Delete** ([expr.delete]): the operand is converted to a pointer
  prvalue. A class operand is contextually implicitly converted to a
  pointer to object (or incomplete) type (`switch_conv.rs`, as for
  `switch`; WP-374), at the `delete` keyword: `deleting incomplete class
  type 'C'; no conversions to pointer type` (with the forward
  declaration note), `ambiguous conversion of delete expression of type
  'C' to a pointer`, `converting delete expression from type 'C' to type
  'T *' invokes an explicit conversion function` (then called), each
  conversion function noted `conversion to pointer type 'T *'`; with no
  usable conversion, `cannot delete expression of type 'C'` (tests
  `expr.delete/class-operand-conversion{,-run}.cpp`). Errors and warnings follow clang: `cannot delete expression of type`,
  `cannot delete pointer to incomplete type 'void'` (an error for
  `void*` in C++26), the incomplete-type warning, and `'delete' applied
  to a pointer-to-array type ... treated as 'delete[]'` (then
  `ARRAY_FORM`). The deallocation function is the usual one: sized when
  the type is complete and the form is not array (no array cookies
  without destructors), aligned when over-aligned. Class element types
  with a non-trivial destructor are unsupported (WP-067b / WP-076);
  class-scope `operator delete` is `todo(WP-068b)`.
- **Destroying operator delete** ([expr.delete]/7, /10,
  [basic.stc.dynamic.deallocation]/5; WP-336): for a single-object
  `delete p` on a class `C`, a member `operator delete(C*,
  std::destroying_delete_t[, size_t][, std::align_val_t])`
  (`Ast::is_destroying_operator_delete`: a member named `operator
  delete` whose second parameter is the class `std::destroying_delete_t`)
  is preferred: once one is viable only destroying candidates are kept
  (`usual_delete(.., destroying)`; the new-expression's matching
  deallocation never takes one). Without a virtual destructor the operand
  is converted to the first parameter's type. Codegen calls it with an
  empty tag temporary and runs no destructor; the destructor is still
  selected (and access-checked) as clang does. Through a virtual
  destructor the deallocation is the deleting destructor's (WP-457):
  `check_destructor_delete` selects the class's deallocation function at
  the destructor (`dtor_dealloc_fn`) and, for a destroying one, stores
  `this` converted to its first parameter in `Ast::dtor_delete_this`
  for codegen (spec/codegen.md §7u).
- **Type-aware allocation** (P2719, C++26 DIS; WP-1088). The oracle is
  clang 22, which implements the paper (clang 19 rejects it), and clang's
  `type-aware-*.cpp` tests are ported. There is no feature-test macro.
  - *Declarations* (`alloc_decl.rs`). An allocation or deallocation
    function whose first parameter is a specialization of
    `std::type_identity` (`is_type_identity`) is type-aware. It needs at
    least 3 (new) or 4 (delete) parameters: `type_identity<T>`, then
    `std::size_t` (new) or `void *, std::size_t` (delete), then
    `std::align_val_t`, each non-dependent. A template's `T` may be
    deduced only through the first parameter. A destroying form is an
    error. Every such declaration warns
    `ext_cxx_type_aware_allocators` (`-Wext-cxx-type-aware-allocators`,
    as clang 22). The first one sets `GlobalOps::seen_type_aware`, which
    enables the typed lookups below. At class completion, a class that
    declares a type-aware `operator new` (or `new[]`) without a
    type-aware `operator delete` of the same form, or the reverse, is
    an error at the class with an `unmatched` note
    (`check_type_aware_allocator_pairs`).
  - *Malformed `std::type_identity`* (WP-1112; clang
    `LookupStdClassTemplate`, `IsPotentiallyTypeAwareOperatorNewOrDelete`).
    A first parameter of a class or enumeration type named
    `std::type_identity` makes a declaration type-aware even when it is
    no class template (it gets the type-aware signature checks).
    `lookup_std_type_identity` classifies what ordinary lookup of
    `std::type_identity` finds: nothing, a usable class template (exactly
    one required parameter, the first a type parameter, not a pack), or
    something else. Each attempt to build `std::type_identity<T>`
    (`std_type_identity`: once per typed new-expression, once per
    class-scope typed deallocation candidate) with a malformed one is
    `std::type_identity must be a class template with a single type
    parameter` at the declaration lookup found, every time (clang does not
    remember the lookup), and the expression falls back to the untyped
    functions. clang marks the declarations invalid: a namespace-scope
    one is then not found by lookup, so `resolve_usual_dealloc` skips it
    without a diagnostic; a class member is found, so a class's delete
    gets the malformed error and `no suitable member 'operator delete'`.
    `std::type_identity<T>` is not required to be complete while
    candidates are compared. A typed new-expression requires it at its
    start and is invalid if it is incomplete. A delete-expression or
    deleting destructor requires the selected function's at its location
    (`typed_usual_delete`; clang `CheckDeleteOperator`), invalid likewise.
    Divergence: kiln does not mark the declarations invalid, because
    `is_type_aware_alloc_fn` treats invalid as "not type-aware".
  - *Selection* (`new_delete.rs`, `overload/alloc.rs`; clang
    `resolveAllocationOverload`). In typed mode a new-expression first
    tries `(std::type_identity<T>{}, size[, align], placement...)`. A
    failed aligned attempt is retried without the alignment. Only then is
    the untyped call tried. An ambiguous best candidate, typed or not
    (WP-1112), is `call to 'operator new' is ambiguous` at the
    expression, with clang's ambiguous-candidate notes. It is an error at
    once: there is no retry without the alignment and no untyped
    fallback. A deleted typed best candidate is
    `call to deleted function 'operator new'`, with every candidate's
    note. The lookup for an array of class type is in the element class.
    A delete-expression, a deleting destructor and a new-expression's
    cleanup choose among typed usual deallocation functions first
    (`typed_usual_delete`). The cleanup must be typed exactly when the
    `operator new` is, with the same placement form
    (`sema_mismatching_type_aware_cleanup_deallocator`). A global cleanup
    prefers the sized form, and the aligned form only for an over-aligned
    type, even though a typed `operator new` always takes the alignment
    (clang `FindAllocationFunctions`). The check applies only with
    exceptions enabled; clang's cc1 tests run without them, so the
    ports that depend on it pass `-fno-exceptions`. Under exceptions,
    a deleted cleanup delete makes the new-expression invalid.
  - *Codegen* (`kiln_codegen/src/new_delete.rs`). The calls pass an
    empty `std::type_identity<T>` temporary first. The cleanup on a
    throwing initialization passes it too, plus the saved size for a
    placement form. The cleanup reuses the new-expression's
    `type_identity` temporary. A typed `operator new[]` always gets an
    array cookie, as clang does. With a cookie, the alignment that is
    passed is at least `size_t`'s (clang `cookieAlign`).
  - *Constant evaluation* ([expr.const]/10.18.1 as P2719R5 amends it). A
    namespace-scope type-aware function that is not variadic and, apart
    from the `type_identity` parameter, has the signature of a replaceable
    one (3 or 4 parameters, optionally plus `std::nothrow_t`) is usable
    in constant evaluation. The Oven treats it like the replaceable
    function: the storage is its own, and the function is never called
    (`is_type_aware_global_alloc` in `kiln_oven/src/compile/new_delete.rs`).
    A class-scope typed function is not usable (`call to class-specific
    'operator new'`), the same as an untyped one. This follows the DIS and
    clang 22 (`type-aware-new-constexpr.cpp`). A direct call of a usable
    global typed function (`::operator new(std::type_identity<T>{}, n,
    al)`) is treated as `::operator new(n)` (WP-1112; clang
    `HandleOperatorNewCall` / `HandleOperatorDeleteCall`). It is constant
    only within `std::allocator<T>::allocate` / `deallocate`. Elsewhere it
    is `cannot allocate untyped memory in a constant expression; ...` for
    `operator new`, and a bare `subexpression not valid in a constant
    expression` for `operator delete` (`global_alloc_form`,
    `global_alloc_call`). Within `allocate`, the size is the argument
    after the `type_identity`. Divergence: clang 22 reads the size from
    the first argument there, which gives a zero-sized allocation.
  - *Coroutines* ([dcl.fct.def.coroutine]/10-12, `coroutine.rs`). The
    frame's `operator new` and `operator delete` lookups skip type-aware
    functions. When typed ones exist, it warns
    `type aware 'operator new' will not be used for coroutine allocation`
    (`-Wcoro-type-aware-allocation-function`), or adds a note to the
    unusable-new error, with a note per declaration.
  - *Access through a using-declaration* (WP-1089, [namespace.udecl]/19).
    `AllocFns::via` records the using-shadow a class-scope function was
    found through. The access checks of the new- and delete-expression,
    the cleanup and the deleting destructor check the shadow
    (`found_decl`), not its target. For a destroying delete,
    `check_destructor_delete` forms `this` at the first parameter. A
    failed conversion (a private base) gets the note `while checking
    implicit 'delete this' for virtual destructor` at the destructor.
  - Tests: `basic.stc.dynamic/type-aware-*.cpp`,
    `class.free/type-aware-mismatched-constraints.cpp`,
    `expr.new/type-aware-*.cpp`, `expr.new/ir-type-aware.cpp`,
    `expr.const/type-aware-new.cpp`,
    `dcl.fct.def.coroutine/type-aware-allocators.cpp`,
    `expr.delete/destroying-delete-using-access.cpp`.
- **Access to class allocation functions** ([class.access]/4,
  [expr.new]/28, [class.dtor]/7.3; WP-457): a class-specific `operator
  new` / `operator delete` (usual or destroying) selected by a
  new- or delete-expression is access-checked at the expression, naming
  class the allocated class. A new-expression checks both its `operator
  new` and its matching `operator delete` (whether or not the
  initialization can throw, as clang's `FindAllocationFunctions`; not
  under `-fno-exceptions`), and an inaccessible `operator new` does not
  stop the matching delete from being looked up and checked — two errors
  at one location (`check_member_access_distinct`, which the
  diagnostic engine's dedup would otherwise merge). A user-provided
  virtual destructor checks its `operator delete` at the destructor; an
  implicit or defaulted virtual destructor whose `operator delete` is
  missing, ambiguous or inaccessible from the class is defined as
  deleted (`virtual_dtor_delete_usable`, deletion reason
  `note_deleted_dtor_no_operator_delete` "virtual destructor requires an
  unambiguous, accessible 'operator delete'" at the class), so an
  override of a non-deleted base destructor is the usual deleted-override
  error with that note too (`report_override_mismatch`). Tests:
  `tests/conformance/expr.new/class-operator-delete-access*.cpp`,
  `tests/conformance/class.dtor/virtual-operator-delete-access.cpp`.
- The element class's destructor ([expr.delete]/6, and for `new T[n]`
  [expr.new]/27) is selected (`mark_element_dtor_referenced`, skipped
  for an invalid or dependent class): a deleted one (such as a union's
  with a non-trivial variant member, [class.union.general]/3) is
  `attempt to use a deleted function` with the deletion notes and the
  expression fails; an inaccessible one is `calling a private destructor
  of class 'P'` at the operand (`sema_access_dtor`). For `new T[n]`
  both are at the allocated type, the access error worded as for a
  temporary (`temporary of type 'P' has private destructor`), as clang's
  `ActOnCXXDelete` / `BuildCXXNew` do.
- `-fno-sized-deallocation` (`Sema::sized_deallocation`, WP-808; clang
  `LangOptions::SizedDeallocation`): the implicit global declarations
  ([basic.stc.dynamic.general]/2) omit `operator delete(void*,
  std::size_t)` and its array and aligned twins, so `::operator
  delete(p, n)` without a declaration is `no matching function`; and a
  global usual-deallocation lookup (delete-expression, deleting
  destructor, new-expression cleanup, coroutine frame) never prefers a
  sized function, even a user-declared one (`usual_delete`, as clang
  19). A class's own lookup is unchanged: its only `operator
  delete(void*, std::size_t)` still gets the size.
  `tests/conformance/expr.delete/fno-sized-deallocation-*.cpp`.
- Gaps: `-Wmismatched-new-delete`
  is not reported; a non-constant bound in a parenthesized type-id is a
  plain warning (clang's is an extension warning, so an error under
  `-pedantic-errors`); `__hot_cold_t` overloads are not recognized.
- **Default arguments of an allocation function** ([expr.new]/19,
  [dcl.fct.default]; WP-730). The allocation call is an ordinary call: a
  function whose parameters past the size (and alignment) and placement
  arguments all have default arguments is viable, and the expression's
  placement arguments are followed by `CXXDefaultArgExpr`s for them
  (instantiated then for a class template's member), as clang's
  `GatherArgumentsForCall`; a template instantiation drops them and
  re-adds its own. The `requires N arguments` note counts only the
  required parameters.

### 5.10 Immediate functions (`consteval.rs`, WP-133)
clang's `CheckForImmediateInvocation`, `HandleImmediateInvocations` and
P2564 escalation ([expr.const]/16-20, [dcl.constexpr]/2).
- **Declarations.** `consteval` is accepted on functions, member
  functions, constructors, conversion functions, lambdas (`[]()
  consteval {}`) and function templates, and dumped as clang does
  (`consteval`; an escalated function is `constexpr immediate`, a
  lambda's escalated `__invoke` `implicit immediate`). Errors: on a
  variable or field (`consteval can only be used in function
  declarations`), a destructor, `main`, a parameter (`function parameter
  cannot be constexpr|consteval|constinit`), with `constexpr`, and a
  redeclaration that disagrees. A consteval virtual function is allowed
  ([class.virtual]/19). `FnSpec::is_immediate()` (consteval or
  escalated) is what codegen skips.
- **Immediate function context** ([expr.const]/16): a consteval body,
  the constant-evaluated branch of `if consteval`, an unevaluated
  operand, and the manifestly constant-evaluated contexts the parser
  brackets with `enter/exit_constant_evaluated` (`static_assert`,
  array bounds, template arguments, the `if constexpr` condition,
  `parse_constant_expr`). Nothing is wrapped or checked there.
- **Invocations.** Building a call, member call, operator call or
  construction of an immediate function outside such a context wraps
  it in an unvalued `ConstantExpr` and records it as a candidate
  (`check_immediate_invocation`); the callee's `DeclRefExpr` is dropped
  from the references. `finish_immediate_invocations` runs at the end of
  every full-expression (`act_on_full_expr`): nested candidates lose
  their wrapper, the outermost are evaluated with `eval_const`
  (`EvalKind::Consteval`) and the value stored in the `ConstantExpr`.
  Each is evaluated once ([expr.const]/16, WP-1129): the completion
  checks that fold the full-expression before that point (clang's
  `CheckCompletedExpr`: the implicit-conversion and integer-overflow
  warnings) run after `evaluate_immediates_early`, which evaluates the
  outermost invocations, stores each successful value and keeps every
  result in `ImmState::early` for `finish_immediate_invocations` to
  report (in the same place and order as before). Those folds read a
  valued wrapper as a leaf, and a scalar prvalue call reached through a
  stripped wrapper (`strip_impcast`) from `ImmState::early_values`
  (`early_immediate_value`, checked by `try_fold`, `fold_rvalue` and
  `fold_for_overflow`). A default argument's or default member
  initializer's invocation is evaluated at each use (below); its first
  successful value is stored in the wrapper's `default_immediate_value`,
  which codegen's fold prepass and the Oven (`Compiler::stored_value`,
  outside a use with source-location values) read instead of evaluating
  the call again.
  **Default arguments (WP-1171).** clang rebuilds a default argument
  that holds an immediate invocation or a source-location builtin at
  each use (`BuildCXXDefaultArgExpr`, `ImmediateCallVisitor`,
  `EnsureImmediateInvocationInDefaultArgs`), completes it there
  (`ConvertParamDefaultArgument` → `CheckCompletedExpr`, so its
  `-Winteger-overflow` warnings repeat at the use) and pops its
  evaluation context (`HandleImmediateInvocations`), all while the call
  is built. So `evaluate_immediates_early` checks each live default use
  of a parameter outside an immediate-escalating function then and
  there: its invocations are evaluated in order and valued, the rebuilt
  argument is checked for overflow with the use as the context key
  (`check_default_arg_overflow`), and a failure is reported
  (`check_default_use_with`), before the full-expression's own checks;
  `finish_immediate_invocations` skips the use (`ImmState::
  early_defaults`). In an immediate-escalating function the results are
  kept and the check (which may escalate instead) runs at the finish, as
  before. A default member initializer (`default_init`) is not rebuilt
  this way for the checks (`ConvertMemberDefaultInitExpression` does not
  complete it).
  **Shift counts (WP-1171).** `diagnose_shift_count` runs when the
  operator is built; clang's `DiagnoseBadShiftValues` calls
  `EvaluateAsInt` there, which evaluates an unvalued `ConstantExpr`'s
  operand, so clang evaluates a count `x << f()` twice. kiln evaluates a
  count that is itself a pending invocation (parentheses and integral
  conversions aside) once: `prefold_immediate` evaluates it and keeps the
  result in `ImmPending::prefolded`, and the full-expression's
  `evaluate_immediates_early` / `immediate_result` take it from there
  instead of evaluating again (the failure is reported once, at the
  finish). A count with the invocation under an operator (`x << (f() +
  1)`) is still folded at the operator, as clang.
  The initializer of a variable that is constant-initialized keeps its
  wrappers unvalued (clang's deferred mode). A failure reports `call to
  consteval|immediate function 'f' is not a constant expression` with
  the Oven's notes. When the variable a full-expression initializes is
  constexpr or constinit (`note_checked_var_init_full_expr`), a failing
  outermost invocation is not reported there: it is kept in
  `ImmState::after_var` and `report_deferred_immediates` re-evaluates
  and reports it after `check_constexpr_init`/`check_constinit`, as clang
  checks the variable first (the invocation's note then names the
  variable's initializer; WP-465,
  `meta.const.eval/is-within-lifetime-consteval-var.cpp`). An escalating
  invocation is reported in place. Default arguments and default member initializers
  are not checked at their definition (`note_default_init_full_expr`).
  A constant-expression finished as a full-expression of its own while
  an enclosing one is being built (`act_on_constant_full_expr`: a
  default template argument substituted during that full-expression's
  overload resolution, e.g. `enable_if<...>::type = 0` of a converting
  constructor template, libc++ `error_code`) finishes only the
  candidates, references and default uses inside it; the others stay
  pending for the enclosing full-expression
  (`set_aside_outer_immediates`; clang pushes a `ConstantEvaluated`
  context in `SubstDefaultTemplateArgument`). WP-286;
  `tests/conformance/expr.const/immediate-invocation-default-template-arg-*.cpp`.
- **Invocations validated by constant initialization**
  ([expr.const.imm]/1.2, [expr.const.defns]/1.6, [expr.const.init]/4;
  WP-549). The initializer of a variable that is usable in constant
  expressions or has constant initialization is manifestly
  constant-evaluated, so a consteval call inside it is in an immediate
  function context: it is validated by the variable's
  constant-initialization check, not individually. An intermediate
  result that is not a permitted result on its own (a class prvalue
  whose member owns heap, `flat().d` with `consteval P flat()` and `P`
  holding a vector) is accepted when the whole initializer is constant:
  the temporary is destroyed at the end of the full-expression and only
  the final value is checked. Each outermost invocation is evaluated
  once (WP-1140): `finish_immediate_invocations` stores every successful
  value before it folds the initializer (`try_fold`, at most once), and
  that fold, the variable's own check and codegen read the stored value
  as a leaf (oven.md "Valued immediate invocations"); a value stored
  for a deferred initializer is `CONSTANT_VALUE_HIDDEN`, so the dump
  still prints the wrapper unvalued, as clang's. The rule:
  - constexpr or constinit `v` (namespace scope, local, static data
    member, variable template specialization): a failing outermost
    invocation waits in `after_var`; `report_deferred_immediates(v,
    constinit_ok)` drops it unreported when `v` is constexpr with an
    evaluated value (`VarEvalState::Value`) or `check_constinit`
    returned `true` (constant initializer; a destruction-only failure
    counts), and otherwise re-evaluates and reports it after the
    variable's own error (the WP-465 order). A failed invocation's
    wrapper keeps no value, as clang leaves it unvalued. In a template
    definition a `v` whose initializer is dependent (and has no error)
    has no value to decide by: its entries are dropped, and each
    instantiation or expansion, which shares the non-dependent wrapper,
    checks it again with `v` (`recheck_shared_invocation`, below;
    WP-1263: `constexpr int x = make().get() + sizeof(T);` and `constexpr
    bool b = m == nonstatic_data_members_of(^^S, ctx)[0];` in a
    `template for` body).
  - any other variable that `checks_constant_init` (static or thread
    storage duration, not constexpr/constinit): a failing invocation is
    not reported when the initializer folds (`try_fold`), since it is
    then a constant initializer ([basic.start.static]/2); otherwise it
    is reported in place.
  - a variable template specialization whose initializer is shared with
    the pattern (non-dependent): `templ/subst/decls.rs` `var_init`
    brackets the substitution with `note_var_init_subst(Some(v))`, and
    `recheck_shared_invocation` defers its failed re-check to `v`'s
    `after_var` (flagged as a repeat report) instead of reporting it,
    so the specialization's check decides as above; this also removes
    the second report a failed pattern invocation used to get. When
    the substitution fails, `v`'s pending entries are reported after
    its error.
  - an instantiated initializer whose pattern is a full-expression with
    cleanups is rebuilt from the wrapper's operand and re-wrapped
    around the converted instance (clang's `TransformInitializer`), so
    the temporaries of `template<class T> constexpr T v = q().d;` live
    until the conversion to `T` is done
    (`temp.inst/variable-template-dependent-type-temporary-member.cpp`).

  Elsewhere (a return statement, a constexpr function body, an
  automatic variable, a static-storage variable whose initializer does
  not fold) each outermost invocation is still checked on its own.
  Clang 19 and 22 check the call individually in an in-class static
  data member initializer and, in a template definition, in a local
  constexpr variable's dependent initializer, and clang 19 also in a variable template
  specialization whose call has a value-dependent argument or that is
  first instantiated from a non-constant context; kiln follows GCC 16.1
  (and clang 22 for the latter) and accepts them. Tests:
  `expr.const/immediate-invocation-constant-init{,-run,-neg,-dependent,-dependent-local,-static-member,-reflection,-static-array}.cpp`.
- **Default-argument temporaries** ([class.temporary]/6, WP-216): a
  default argument is its own full-expression at the definition
  (`ExprWithCleanups`), but each use takes over its cleanups: the
  `CXXDefaultArgExpr` child is the argument without that wrapper
  (`Ast::default_arg_expr`, clang `ParmVarDecl::getDefaultArg`) and
  `note_default_use` marks the using full-expression as needing cleanups
  (clang `CheckCXXDefaultArgExpr`), so `g()` with `void g(const X& =
  X())` destroys the `X` after the call. Codegen, the Oven and the AST
  dump read the argument through `default_arg_expr`.
- **Default uses** ([expr.const]/16, [dcl.fct.default]/5, [class.mem]/10;
  WP-137). Each use of a default argument (`convert_call_args`,
  `ctor_args`) or default member initializer (aggregate `field_filler`,
  a constructor's implicit member init in `collect_field_init`) whose
  definition holds an unvalued invocation is recorded with
  `note_default_use` and checked at the end of the using
  full-expression (`check_default_use`): each invocation is evaluated
  afresh, and a failure is `call to consteval function 'h' is not a
  constant expression` at the invocation, then `in the default
  initializer of 'p'` at the use (callee begin, construct loc, ctor
  name, or the closing `}`/`)` of an aggregate init) and `declared here`
  at the parameter or field. A default used inside another default
  argument or initializer is checked at that definition only. In an
  immediate-escalating function (defaulted or implicit constructor,
  `constexpr` template specialization, lambda) the failure escalates
  instead; a constructor's reason note is `'D' is an immediate
  constructor because the default initializer of 'm' contains a call to
  a consteval function 'h' and that call is not a constant expression`.
  An instantiation re-checks the defaults its shared expressions and
  rebuilt init lists use (`templ/subst/shared.rs`, `subst/exprs.rs`),
  so a template use is reported at the definition and at each
  instantiation. Codegen folds each use separately (WP-144). clang 19's
  extra report at a class template's NSDMI instantiation and inside
  `sizeof` are not replicated.
- **References.** A `DeclRefExpr` (or `&C::m`) naming an immediate
  function that is not the callee of a candidate is `cannot take
  address of consteval function 'f' outside of an immediate
  invocation`, with `declared here`. For a qualified name (`&C::m`, also
  inside a member function) the error is at the qualifier, as clang.
  A reference to an immediate function in a dependent context (a member
  of a class template's pattern, `DEPENDENT_CTX`, or a function
  template's pattern) is not recorded (clang `MarkDeclRefReferenced`,
  `!FD->isDependentContext()`): a dependent call of it
  (`_S_quote().substr(0, 1)` with `_S_quote` a consteval static member
  of a class template, libstdc++ 16 `<format>`) is built and checked at
  instantiation, where taking its address is still an error (WP-525,
  `expr.const/consteval-member-dependent-call.cpp`). The callee of a
  dependent call (`dependent_call`, type-dependent callee or arguments)
  is not a reference either: an unqualified callee is rebuilt as an
  `UnresolvedLookupExpr` for ADL, and a qualified one (`::f(t)`,
  `N::f(t)`, a using-declaration's `M::f(t)`, also parenthesized) stays
  the `DeclRefExpr` but is dropped from the full-expression's pending
  references (`forget_immediate_ref`), as a resolved callee is by
  `check_immediate_invocation`; the instantiated call is the immediate
  invocation (WP-533,
  `expr.const/consteval-qualified-dependent-call-{run,neg}.cpp`).
  The Oven also rejects a constant result (or a subobject of one) that
  is a pointer, reference or member pointer to an immediate function:
  `pointer|reference to a consteval declaration is not a constant
  expression` at the invocation's callee, then `declared here`
  ([expr.const]/22; WP-137).
- **Escalation** (P2564, [expr.const]/18-20). Immediate-escalating:
  a lambda call operator, a defaulted special member, a `constexpr`
  function instantiated from a templated entity, and a `constexpr`
  function template while its definition is parsed (not a member of a
  class template's pattern, clang's `TK_NonTemplate`: its failed
  invocation is an error at the definition; WP-227). In one, a failed invocation
  or a bad reference makes the function immediate (`FnSpec::escalated`)
  instead of an error: the failed `ConstantExpr` loses its wrapper and
  its callee `DeclRefExpr` (or the `CXXConstructExpr`) is flagged
  `immediate-escalating` (dumped), and the reason is kept for the note
  `'f' is an immediate function because its body contains a call to ...`
  on a later failed call of `f`. Successful invocations in it are still
  wrapped and folded. A specialization starts non-escalated whatever
  its pattern is; refs already flagged are skipped. A lambda whose call
  operator escalates gets an escalated `__invoke`, and its conversion
  function to a function pointer is escalated too
  ([expr.prim.lambda.closure]/10; clang 19 instead rejects every use of
  the conversion of a consteval lambda).
- **Templates.** A template definition's non-dependent invocations are
  checked at the definition (they are shared with every instantiation),
  including those of a constructor's mem-initializers, which are not
  built until instantiation (sema-class.md §4.9); dependent ones are
  built and checked at instantiation, with `in instantiation of ...`
  notes. A shared invocation that failed at the definition is evaluated
  again by each instantiation that substitutes it
  (`recheck_shared_invocation`): reported again with the instantiation's
  context, as clang reports its rebuilt `ConstantExpr`, or escalating an
  immediate-escalating instantiation (`e<int>` of `template<class T>
  constexpr int e() { return bad(1); }` becomes immediate, so a
  non-constant call of it is an error; WP-227). At clang 21+
  (`clang_compat >= 22`; WP-413) a shared reference the definition
  flagged `immediate-escalating` (an address taken outside an immediate
  invocation, not an invocation's callee) escalates the instantiation
  too (`recheck_shared_ref`, reason "evaluates the address of"); clang
  19 neither escalates nor diagnoses it. An error taking the address of
  an escalated function (`sema_immediate_address`) carries the reason
  note after `declared here`, as clang's `DiagnoseImmediateEscalatingReason`. A `ConstantExpr` is substituted
  through to its operand and re-wrapped by the rebuilt call. A shared
  (not rebuilt) expression is walked at each instantiation and the
  functions it odr-uses (named, constructors, bound destructors,
  `operator new`/`delete`) are marked referenced, which defines or
  instantiates them, as clang's rebuild does (`templ/subst/shared.rs`;
  unevaluated operands and lambda bodies are not entered).
- **Consteval-only expressions** ([expr.const.imm]/2). The leaves
  (`^^`, references to consteval-only variables) and their exemptions
  are WP-472's, `spec/reflection-repr.md` §6
  (`consteval_only.rs`, checked from `finish_immediate_invocations`).
  A non-exempt leaf in an immediate-escalating function escalates it
  through `escalate_consteval_only` (reason note `... because its body
  contains a consteval-only value of type 'info' (aka 'std::meta::info')`,
  WP-473, `spec/consteval-blocks.md` §4); elsewhere it is
  `sema_consteval_only_expr`. The consteval block itself is
  §5.10's immediate function context by construction (a consteval
  lambda).
- **Codegen.** A valued `ConstantExpr` lowers to its constant: scalars
  directly, aggregates by clang 19's `CreateCoercedStore` into the
  destination (one store per top-level element).
- Gaps: the escalation of a generic lambda's conversion function is not
  tracked per specialization; an implicit constructor does not escalate
  through a member whose (non-NSDMI) default construction is a failed
  consteval constructor call.

### 5.11 Atomics (`builtins/atomic.rs`, WP-154)
- **Builtins.** `__c11_atomic_*` and `__atomic_*` calls (the ids in
  `kiln_ast::decl::builtin::atomic`) become an `AtomicExpr`, as clang's
  `Sema::BuildAtomicExpr`: the pointer operand is checked
  (`sema_atomic_builtin_must_be_pointer`, `_op_needs_atomic` for the c11
  forms, `_needs_non_const_atomic`/`_pointer`, incomplete types — c11
  pointer arithmetic needs a complete pointee, and a function pointee is not
  incomplete, as clang's `RequireCompleteType` (WP-446) — `_BitInt`
  prohibited), the value kind per op (int / int-or-fp / int-ptr-or-fp /
  trivially copyable), value operands are converted to the value type (or
  `ptrdiff_t` for pointer arithmetic, or pointers for the generic
  `__atomic_load`/`store`/`exchange`/`compare_exchange` forms), orders to
  `int`, and a constant order invalid for the op is
  `-Watomic-memory-ordering` (`sema_atomic_op_invalid_memory_order`). The
  result type is clang's (void, the value, or `bool`).
- **Lvalues of `_Atomic(T)`.** Lvalue-to-rvalue conversion adds
  `AtomicToNonAtomic`; assignment and initialization from `T` add
  `NonAtomicToAtomic`. Operand checks of the arithmetic, shift, bitwise,
  unary and `switch` conditions use the value type (`value_ty`). `++`/`--`
  and compound assignment are allowed on a modifiable `_Atomic` lvalue
  and yield an lvalue of `T` (`++`) or `_Atomic(T)` (compound assignment),
  as clang 19's `decltype` shows. A braced list initializing an `_Atomic`
  object is `sema_illegal_initializer_type`, as clang, unless it holds
  one element of class type.
- **`_Atomic(T)` of class type `T` (WP-165).** Initialized from a `T` or
  a class derived from `T` (copy, direct, or a one-element braced list)
  as `T` is, then `NonAtomicToAtomic` (clang `InitializeFrom`
  `NeedAtomicConversion`; `Sema::atomic_class_init`); a `return` operand
  too (`convert_for_init`, WP-183), while an argument is not converted
  (clang's implicit conversion sequence has no such step, so the call is
  "no matching function"); other class sources fail as a conversion to
  `_Atomic(T)`. Assignment from `T` or `_Atomic(T)`
  is the right side's lvalue conversion then `NonAtomicToAtomic`
  (`conv::atomic_class_assign`, C11 6.5.16.1). Assignment errors to any
  `_Atomic` target are reported at the operator, as clang's C path.
- `-Watomic-alignment` is reported by codegen (spec/codegen.md §7d).

### 5.12 `__builtin_va_arg` (`builtins.rs`, WP-171)
- **`act_on_va_arg(list, ty, ty_loc, loc, rparen)`** is clang's
  `Sema::BuildVAArgExpr`. The operand initializes a
  `__builtin_va_list &` (an AAPCS64 struct) or, for the x86-64 array
  `va_list`, decays and must be `__va_list_tag *` (otherwise the
  reference-binding / conversion error). The type must be complete
  (`sema_va_arg_incomplete`, with the forward-declaration note),
  non-abstract (`sema_va_arg_abstract`, with the pure-virtual notes) and
  POD (`sema_va_arg_non_pod`, `-Wnon-pod-varargs`, an error by default;
  a reference type is non-POD). A type that undergoes default argument
  promotion (`va_arg_promotion`: `float`, or a type whose integral
  promotion differs from it, an enum compared through its underlying
  type, so `E : short` warns and `enum class` does not) is
  `sema_va_arg_promotable` (`-Wvarargs`) through `diag_runtime_behavior`.
  Dependent operands build a dependent `VAArgExpr`; substitution rebuilds
  through `act_on_va_arg`.
- **Runtime-behavior diagnostics** (`stmt.rs`, clang
  `DiagRuntimeBehavior`): inside a function body they are queued on
  `FnState::runtime_diags` and flushed in `end_function_body`; outside a
  function (a namespace-scope initializer) they are emitted at once.
  `runtime_diag_quiet` drops them where clang's innermost evaluation
  context is not potentially evaluated (WP-235):
  - an unevaluated operand;
  - a manifestly constant-evaluated expression: a constant-expression
    the parser enters with `enter_constant_evaluated` or sema with
    `act_on_constant_full_expr`, and the conversions of a non-type
    template argument. The Oven reports the constant-evaluation failure
    instead (`division by zero` as a note of the error);
  - a quiet context of the body, counted by `FnState::rt_quiet`: the
    body of a `consteval` function, the immediate compound statement of
    `if consteval` / `if !consteval`, and the branch a non-dependent
    `if constexpr` condition discards (`enter_discarded_statement`,
    from the parser's `constexpr_condition_value`).

  A variable's initializer, including its conversions, is potentially
  evaluated again (`enter_var_initializer` zeroes `rt_quiet` until
  `act_on_init` is done; clang's `ActOnCXXEnterDeclInitializer`). So
  `if constexpr (false) { int x = 1 / 0; }` still warns, as in clang.
  Each queued diagnostic has an optional anchor expression; an anchored
  one is dropped at the flush when the anchor is unreachable (see the
  unused-value reachability rule, WP-203).
  **Gaps:** the `diag_runtime_behavior` callers queue without an anchor,
  so clang's CFG reachability filter does not apply to them
  (`if (0) va_arg(ap, char)` is silent there, reported here), and a
  non-dependent expression shared by a template and its instantiation is
  checked once, at definition, except for `-Wnull-dereference` (§5.15).
  At the end of a body in a dependent context the queue is flushed
  unfiltered for a function template declared by name. For any other
  function (a member of a class template, a lambda or local class in a
  template, a generic lambda's call operator) the anchored entries are
  dropped, and their instantiations warn instead: clang finishes a
  `FunctionTemplateDecl` without an analysis policy, and its
  `IssueWarnings` returns early for a dependent context. The unanchored
  ones (`-Wvarargs` `va_arg` promotion, `-Wuninitialized` self-reference)
  are still reported at the definition, because the instantiation shares
  and does not re-check them (clang reports them per instantiation).

### 5.13 Vector conditionals (`vector_ops.rs`, WP-173)
The vector operators themselves are summarized in spec/types.md
(`Vector`). `?:` follows clang 19:
- **Vector condition** (`act_on_vector_conditional`, clang
  `CheckVectorConditionalTypes`): a condition of vector type with an
  integral element is decayed, not converted to `bool`, and selects lane
  by lane.
  - A `void` or `throw` arm is `GNU vector conditional operand cannot be
    void|a throw expression`.
  - Two vector arms must have the same type (`vector operands to the
    vector conditional must be the same type`, with clang 19's stray
    `}`), and must be ext vectors exactly when the condition is
    (`cannot mix vectors and extended vectors in a vector conditional`).
  - One vector arm: the scalar arm is converted as for a binary vector
    operator (`CheckVectorOperands`, `bool` vectors allowed).
  - Two scalar arms: the same type, or the usual arithmetic conversions.
    An enumeration result is `enumeration type T is not allowed in a
    vector conditional`. The result is a vector with the condition's lane
    count and kind (ext or GNU), and both arms are splatted. A
    non-arithmetic result is `incompatible operand types`, a deviation:
    clang 19 builds a malformed vector or crashes.
  - Then the result must have the condition's lane count (`vector
    condition type T and result type U do not have the same number of
    elements`) and lane size (`... do not have elements of the same
    size`).
  - Codegen: a GNU condition is `icmp ne 0` plus `select`. An ext
    condition is the sign-bit mask blend (`icmp slt 0`, `sext`, `and`,
    `or`, with float arms bitcast to the condition type), as clang
    emits it for C++. A `bool` ext-vector condition (accepted from clang
    20: clang 19 rejects its lane size) is one `select` on its `<N x i1>`
    lanes (WP-187; `ext.vector/ir-bool-conditional*.cpp`).
- **Scalar condition with a vector arm** (`scalar_cond_vector_operands`):
  converted to `bool` as usual, and the arms are converted as for a
  binary vector operator without `bool` vectors. A `bool`-vector arm is
  `invalid operands to binary expression`.

### 5.14 `-Waddress-of-packed-member` (`packed.rs`, WP-183)

clang's `RefersToMemberWithReducedAlignment`, `DiscardMisalignedMemberAddress`
and `DiagnoseMisalignedMembers`:

- **Candidates.** `&m` (parentheses stripped) of a `MemberExpr` chain
  `a.b.c` is recorded when all of these hold:
  - every link names a non-static data member (not an indirect field);
  - some link is packed, either the member or its class;
  - the top base, with parentheses and implicit casts stripped, is a
    `DeclRefExpr` or `this`;
  - the member's type is aligned to more than 1 byte;
  - the summed field offsets are not a multiple of that alignment, or
    the outermost class's alignment is lower.

  For `.` on a non-reference variable, the outermost class's alignment
  is raised to the variable's own alignment (type or `alignas`). The
  culprit is the innermost member that is packed or is in a packed
  class. Its alignment is the smaller of its type's and its class's.
- **Discard.** A conversion of the recorded `&m` drops it when the
  target is an integer, a dependent type, or a pointer to an incomplete
  type or to a type aligned no more than the culprit:
  - an explicit cast (C-style, named other than `dynamic_cast`,
    functional), when it is built (`act_on_cast`; clang
    `BuildCStyleCastExpr` / `BuildCXXNamedCast`), before the rest of the
    enclosing expression is parsed (WP-659);
  - every cast node in the full-expression (implicit and explicit);
  - `implicit_convert` in any context except simple assignment (clang's
    `InitializationSequence::Perform` step, even for an identity
    conversion);
  - both operands of a pointer-pointer comparison.

  A conditional operator's arms, a pointer arithmetic operand, `(void)`
  and the right side of `=` do not discard.
- **Report.** At the end of the full-expression (`act_on_full_expr`), at
  the `MemberExpr`'s begin location: `taking address of packed member
  'm' of class or structure 'C' may result in an unaligned pointer
  value`. An anonymous class is named by its typedef.
  Every full-expression reports every pending candidate, including one
  that a nested full-expression (a template argument, a lambda body, a
  function body instantiated mid-expression) finds pending from its
  enclosing expression. Neither kiln nor clang sets the candidates aside
  in `in_inst_context`: in `fv(&p.i, A<3>::w)` with `fv(void *, int)`
  the `&p.i` is reported, by clang too, because the argument conversion
  that would drop it comes after `3`'s full-expression. An explicit cast
  is dropped earlier, so `f((char *)&p.i, A<3>::w)` is not reported.
- **Deviations:**
  - A non-dependent `&m` in a template is reported once, at the
    definition. clang repeats it in every instantiation.
  - An anonymous union or struct member as the culprit prints as `''`.
    clang prints `C::(anonymous union at ...)`.
- Tests: `tests/conformance/expr.unary.op/address-of-packed-member.cpp`,
  `address-of-packed-member-nested-full-expression.cpp`.

### 5.15 `-Wnull-dereference` (`null_deref.rs`, WP-191)
The two syntactic `*null` checks of clang 19
(`CheckForNullPointerDereference`). The pattern is a `*p` found under the
expression with parentheses, casts, full-expressions and materialized
temporaries ignored, where `p` (also with those ignored) is a null
pointer constant: a zero integer literal, a `std::nullptr_t` value or
`__null` (`(1 - 1)` is not one).
- **Indirection** (`check_null_indirection`): run from
  `conv::lvalue_to_rvalue` (clang `DefaultLvalueConversion`) and on the
  left operand of a builtin simple or compound assignment (clang
  `CheckAssignmentOperands`; not for `++`/`--`). The operand must have
  pointer type and the dereference must not be volatile. The warning
  "indirection of non-volatile null pointer will be deleted, not trap"
  [-Wnull-dereference] carries the operand's range and the note
  "consider using __builtin_trap() or qualifying pointer with
  'volatile'", both at the `*`.
- **Reference binding** (`check_null_reference_binding`): run where
  `conv::bind_reference` binds a glvalue directly (clang
  `SK_BindReference`): "binding dereferenced null pointer to reference
  has undefined behavior" [-Wnull-dereference], volatile or not. A
  binding through a converted temporary (`const long &r = *(int *)0`)
  loads instead and gets the indirection warning.
- Both go through `diag_if_reachable` and are dropped in an unevaluated
  operand or an immediate function context (§5.1 reachability). Member
  access, subscripts, `&*`, `(void)` and discarded operands load nothing
  and are not diagnosed.
- **Templates** (WP-235): a dependent `*(T *)0` is checked when the
  instantiation rebuilds it. A non-dependent one is shared by the
  instantiation (spec/sema-templates.md §5.4), so the definition keeps
  each warning it computed, keyed by the dereference and numbered in
  check order (`null_deref_rewarn`). The walk that marks a shared
  expression's functions referenced (`mark_shared_expr_referenced`,
  which skips unevaluated operands and lambda bodies) collects the
  recorded dereferences it meets and queues their warnings again, in
  that order, under the instantiation's notes and reachability, unless
  the context is quiet. It costs nothing while no template has warned.
  Inside an instantiation these warnings are exempt from the engine's
  deduplication against the definition's report (spec/diag.md §7.4)
  and deduplicated per dereference and instantiation context instead
  (`null_deref_seen`), so a shared dereference that a rebuilt parent
  re-checks warns once. Other run-time warnings (division by zero,
  shift counts) are not repeated, as in clang.
- **Deviations:** A default member
  initializer of a class template does not warn again when a
  constructor uses it (clang instantiates it then), and a variable
  template specialization's warning can print later than clang's,
  which instantiates it at its first use.
- Tests: `tests/conformance/expr.unary.op/null-dereference-indirection.cpp`,
  `tests/conformance/dcl.init.ref/null-dereference-binding.cpp`,
  `tests/conformance/temp.inst/null-dereference-rewarn.cpp`.
### 5.16 GNU `_Complex` (`complex.rs`, `conv/`, WP-211)
clang 19's C99/GNU complex support in C++:
- **Types.** `_Complex` combines with `float`, `double`, `long double`,
  `__float128`, `_Float16/32/64/128` and the integer types (`char`
  through `long long`, signed or unsigned). As in GCC 14 (not clang),
  it also combines with `__bf16`, spelled directly or through a
  type-name that denotes it (libstdc++ `<complex>`'s `__complex__
  decltype(0.0bf16)`; WP-365, spec/abi.md §8.17). Plain `_Complex` is
  `_Complex double` with a warning. Any other base (`bool`, `void`,
  `wchar_t`, `char8_t`, `char16_t`, `char32_t`, `__int128`, `auto`,
  another typedef name, spelled `'type-name'`) is
  `sema_invalid_complex_spec`, and the declaration continues with the base
  type (so `_Complex void v;` also reports the incomplete type).
  `-pedantic` warns `'_Complex' is a C99 extension` on each keyword
  (parser, `-Wc99-extensions`).
- **Traits** (clang): `__is_arithmetic`, `__is_fundamental`,
  `__is_scalar` and `__is_aggregate` are true for every complex type;
  `__is_floating_point` is true for a complex floating type;
  `__is_integral`, `__is_signed` and `__is_unsigned` are false.
- **Standard conversions** (`conv/scs.rs`): complex promotion
  (`_Complex float` to `_Complex double`, Promotion rank), complex
  conversion, and complex-real conversion in either direction (a rank
  worse than Conversion), plus complex-to-`bool`. The cast kinds are
  clang's `*RealToComplex`, `*ComplexCast`, `*ComplexToReal`,
  `*ComplexToBoolean`, `IntegralComplexToFloatingComplex` and back.
  Sugar on the target type is kept in the cast type.
- **Usual arithmetic conversions** (`conv/arith.rs`, clang
  `handleComplexConversion`): a real floating operand keeps its realness
  and only its precision is promoted. An integer operand of a complex
  floating type, and a real operand of a complex integer type, become
  complex. Two complex integer types take the elements' common integer
  type without promotion (`_Complex unsigned char + _Complex unsigned
  char` stays `_Complex unsigned char`). A compound assignment's left
  operand is not converted.
- **Operators.** Complex operands allow unary `+ - ~` (`~` is the
  conjugate), `!`, binary `+ - * /`, `==`/`!=`, `&&`/`||`/`?:` conditions,
  assignment and compound assignment. `<`/`>` and `%` are invalid
  operands. `++`/`--` are allowed; under `-pedantic` they give
  `'++' on an object of complex type is a C2y extension`
  (`-Wc2y-extensions`) at the operator.
- **`__real e` / `__imag e`** (clang `CheckRealImagOperand`): the element
  type of a complex operand, or a real arithmetic operand's own type.
  `__real` keeps the operand's value category. `__imag` does for a
  complex operand, and is a zero prvalue otherwise.
- **`__builtin_complex(re, im)`**: two operands of the same real floating
  type `T` give `_Complex T`, including `__bf16` (`_Complex __bf16`,
  which clang 19/22 also give though they reject the spelled type).
  `_Float16` and `__fp16` are rejected at the callee ("'_Complex
  _Float16' is invalid", "'_Complex half' is invalid") below
  `-fclang-compat=22` and accepted from 22, as the matching clang does
  (WP-380). Other operand types are diagnosed as clang does.
- **List initialization** (clang `CheckComplexType`): `{re, im}`
  initializes the parts, each converted as a list element with narrowing
  checked. A third clause is excess. One clause initializes from a
  scalar.
- **GNU imaginary constants.** The lexer sets `imaginary` for an
  `i`/`j` suffix (spec/lex.md). `act_on_{integer,float}_literal` builds
  the plain literal wrapped in an `ImaginaryLiteral` of `_Complex T`, and
  emits `sema_imaginary_constant` ("imaginary constants are a GNU
  extension", `-Wgnu-imaginary-constant`, off by default) at the literal.
  When the suffix is also a valid ud-suffix (`1i`, `1.0if`, `1il`), literal
  operator lookup runs first. If no cooked, raw or template form exists,
  sema falls back to the imaginary constant without a diagnostic (clang
  `LookupLiteralOperator` with `DiagnoseMissing = false`). The `<complex>`
  operators therefore still win where they are visible.
- **`-Wuser-defined-literals` tail.** "no literal will invoke this
  operator" is omitted for any valid ud-suffix: the numeric library
  suffixes and `sv`, using clang's `StringLiteralParser::isValidUDSuffix`.
  Sema has no `-std`, so the C++20 set is used.
- Tests: `tests/conformance/ext.complex/` (`types`, `constexpr`,
  `diagnostics`, `pedantic`, `imaginary`).

### 5.17 Value-dependent static member calls and dependent-base members (WP-211)
- A call to a static member function of the current instantiation is
  value-dependent ([temp.dep.constexpr]/2.3). An `if constexpr` on it in a
  member initializer is deferred to instantiation, as libc++
  `mdspan`'s `__rank_` needs
  (`tests/conformance/temp.dep.constexpr/static-member-function-current-instantiation.cpp`).
- An overloaded `B::f` named through a dependent base that is not `B`
  is called without an object argument. It is diagnosed with clang's
  `diagnoseInstanceReference` wording ("call to non-static member
  function without an object argument").

### 5.18 `-Wabsolute-value` (`absolute_value.rs`, WP-310)
clang's `CheckAbsoluteValueFunction`, unsigned-argument case, run on every
non-dependent call of a named function after argument conversion (also in
instantiations). The callee is an absolute value function when it is an
implicit builtin `__builtin_abs`/`labs`/`llabs`/`fabs`/`fabsf`/`fabsl`/
`cabs`/`cabsf`/`cabsl`, a declaration recognized as the library builtin
`abs`/`labs`/`llabs`/`fabs`/`fabsf`/`fabsl` (§5.4, `lib_builtin_info`), or a
function named `abs` whose context (inline namespaces and linkage
specifications skipped) is `::std`. With exactly one argument whose type,
parentheses and implicit casts stripped, is an unsigned integer type
(clang `isUnsignedIntegerType`: `bool`, unsigned and unsigned-character
types, unsigned `_BitInt`, a complete unscoped enumeration with an unsigned
underlying type): warning `sema_unsigned_abs` "taking the absolute value of
unsigned type 'T' has no effect" (group `absolute-value`, on by default) at
the callee's begin, with note `note_remove_abs` "remove the call to 'F'
since unsigned values cannot be negative" over the callee; F is `std::abs`
for the `std` function, else the builtin's spelling (libc++'s
`std::abs(uc)` resolves to the C library's `::abs`, so F is `abs`). Not
implemented: clang's pointer-argument, wrong-function-kind and too-small
warnings of the same group. Test:
`tests/conformance/c.math.abs/absolute-value-unsigned.cpp`.

### 5.19 Ext-vector element access (`vector_ops.rs`, WP-307)
clang's `ext_vector_type` element access (`v.x`, `v.xy`, `p->zw`;
clang `CheckExtVectorComponent`, `getEncodedElementAccess`):
- `act_on_member_q` routes an unqualified identifier member of an ext
  vector (through a pointer for `->`) to `ext_vector_member` before
  class member lookup, so it is never "not a structure or union".
  A dependent base stays a `CXXDependentScopeMemberExpr` and comes back
  here on substitution; a non-dependent `ExtVectorElementExpr` is formed
  again by its substitution arm (sema-templates.md §5.4).
- The accessor (`kiln_ast::expr::ext_vector_access`) is one of: letters
  of the point set `xyzw` or the color set `rgba`, not mixed; `s`/`S`
  followed by hex lane digits; or `hi`/`lo`/`even`/`odd`, selecting
  halves of `ceil(N/2)` lanes (a lane past an odd-sized vector reads
  poison). A bad letter is `illegal vector component name 'c'`, a lane
  past N `vector component access exceeds type 'T'`, both at the
  member name.
- One lane is the element type as written; several are an ext vector of
  that many lanes, sugared with the first file-scope `ext_vector_type`
  typedef of that exact type (`Sema::ext_vector_typedefs`, recorded by
  `act_on_typedef`), as clang's `ExtVectorDecls`. The result carries the
  base's cv-qualifiers; its value category is the base's for `.`
  (`ObjectKind::VectorComponent` for a glvalue), an lvalue for `->`.
- Assignment (`check_modifiable`): a repeated lane is `vector is not
  assignable (contains duplicate components)`; a prvalue base is
  `expression is not assignable`; a const base is the usual
  const-variable diagnostic. `&v.x` and binding a non-const reference
  are rejected as for any vector element (§5.13).
- A `bool` ext vector has no components (clang `isExtVectorBoolType`;
  a lane is one bit and has no address): every accessor, valid or not,
  is `illegal vector component name 'name'` at the member name, before
  the accessor is parsed, also on substitution (WP-438). clang 19
  quotes the name twice (`''x''`), clang 22 once. A subscript reads a
  lane instead.
- A GNU `vector_size` vector has no components (`member reference base
  type ... is not a structure or union`). An unused access (`v.x;`) is
  `expression result unused` at the base's begin (clang's `getExprLoc`).
- Constant evaluation (oven.md "Vectors", WP-312): an access on a
  prvalue vector (a compound literal, `(v + v)`, a `bit_cast`) folds, as
  in clang 19; one on a vector object is not a constant expression
  (clang 19 rejects it too), reported at the expression's begin.
- Codegen (codegen.md "Vectors"): a glvalue access is an `LV` whose
  `vidx` is `VecLane::Index` (one lane, an `i64` index) or
  `VecLane::Swizzle` (several; a swizzle of a swizzle lvalue composes
  onto the innermost vector); a load is `extractelement`/
  `shufflevector`; a store of several lanes shuffles the value into the
  loaded vector (`store_swizzle`). A prvalue base is stored to a
  temporary first (WP-312, clang `EmitExtVectorElementExpr`).
- Tests: `ext.vector/ext-vector-access.cpp` (run, including templates),
  `ext.vector/ext-vector-access-diagnostics.cpp`,
  `ext.vector/element-access-diagnostics.cpp`,
  `ext.vector/element-access-constexpr-diagnostics.cpp` (strict-diags),
  `ext.vector/bool-element-access-diagnostics{,-clang22}.cpp`,
  `tests/codegen/ext_vector_element.cpp`.

### 5.19a Vector subscript (`vector_ops.rs`, WP-469)
`v[i]` with a vector `v` on the left (clang `CreateBuiltinArraySubscriptExpr`;
`vector_subscript`):
- A GNU/ext vector or an SVE vector type (`isSveVLSBuiltinType`: every
  sizeless builtin but `__SVCount_t`, whose subscript is
  "subscripted value is not an array, pointer, or vector", as is `i[v]` with an SVE `v`)
  is subscripted in place: a prvalue base is materialized (CWG1213), the
  index must be an integer or unscoped enumeration ("array subscript is not
  an integer"), and the result is the element type (an SVE vector's
  `getSveEltType`: `signed char` … `double`, `__fp16`, `__bf16`) with the
  base's qualifiers and value category, an `ObjectKind::VectorComponent`
  unless a prvalue.
- `svbool_t` cannot be subscripted: `sema_subscript_svbool` ("subscript of
  svbool_t is not allowed", clang `err_subscript_svbool_t`), at the `[`,
  before the index is checked.
- The element cannot have its address taken ("address of vector element
  requested") or bind a non-const reference ("non-const reference cannot
  bind to vector element"). Divergence: clang does not apply the second
  rule to SVE vectors (`refersToVectorElement` tests `isVectorType`) and
  binds the reference to the vector's address; kiln keeps it.
- Tests `ext.arm.sve/subscript.cpp`, `subscript-element-reference.cpp`,
  `ir-subscript.cpp` (codegen: spec/codegen.md §7aa).

### 5.20 Variadic arguments (`expr.rs`, WP-311)
- An argument for the ellipsis gets the default argument promotions
  ([expr.call]/12) and is then checked once the call is built (clang
  `checkVariadicArgument`; the call is still formed):
  - an argument of type `void` (`g(1, f())` for `void f()`) is an error,
    "cannot pass expression of type 'void' to variadic function";
  - an object of class type with a non-trivial copy constructor, move
    constructor or destructor (conditionally-supported, [expr.call]/12)
    is the error-by-default warning `-Wnon-pod-varargs`, "cannot pass
    object of non-trivial type 'T' through variadic function; call will
    abort at runtime". It is a run-time behavior diagnostic: dropped in an
    unevaluated operand, a constant-evaluated context, and the initializer
    of a constexpr variable (local or not) or of a static data member's
    first non-inline declaration (clang `DiagIfReachable` without a
    statement reads the initializer's `ManglingContextDecl`); otherwise
    emitted at once (`diag_runtime_behavior_now`);
  - a braced-init-list is an error, "cannot pass initializer list to
    variadic function" (§6), and the call is not formed.

  Each points at the argument's begin. The callee is named as clang's
  `getVariadicCallType` does: `constructor` for a constructor, `method`
  for a non-static member function (a lambda's call operator included)
  or a call through a bound member (`(x.*pmf)(...)`), `function`
  otherwise.
- In a dependent context the checks are not made (clang `checkCall`
  returns for a dependent context); an instantiation makes them. A call
  whose arguments are all non-dependent is shared by the instantiation
  (spec/sema-templates.md §5.4) rather than rebuilt, so the definition
  records each argument it would diagnose (`vararg_recheck`), and the
  walk over a shared expression (§5.10's `mark_shared_expr_referenced`)
  checks those arguments of the calls in it again, under the
  instantiation's context notes.
- In constant evaluation an argument of non-literal class type, for the
  ellipsis or a declared by-value parameter, stops the evaluation at the
  argument with clang's `CheckLiteralType` note "non-literal type 'T'
  cannot be used in a constant expression" (spec/oven.md, call frames;
  WP-320), before any constructor or destructor of it runs.
- Tests: `tests/conformance/expr.call/variadic-void-argument.cpp`,
  `tests/conformance/expr.call/variadic-non-trivial-argument.cpp`,
  `tests/conformance/expr.const/non-literal-argument.cpp`.
- Before C++23 a constexpr function's body shall not define a variable of
  non-literal type ([dcl.constexpr]/3; clang `CheckConstexprDeclStmt`):
  `check_constexpr_body_vars` (crates/kiln_sema/src/constexpr_body.rs)
  reports the first one, "variable of non-literal type 'T' cannot be
  defined in a constexpr {function|constructor} before C++23", with the
  type's "not literal because" notes, and marks the function invalid.
  Variables whose own declaration was already diagnosed are checked too.
  As clang 19's `CheckConstexprFunctionStmt`, the walk looks into an
  if-statement's branches only: its init-statement and condition
  variable are not checked (`if (const AssertionResult r = e)` in
  googletest's `EXPECT_*`), while a switch's, a loop's and a range-for's
  are (WP-726, `dcl.constexpr/p3-if-condition-variable-not-checked.cpp`).
  In a template (a function template, a member of a class template, a
  generic lambda) only a variable of non-dependent type is checked, at the
  definition; instantiations are not checked. A function template made
  invalid this way stays a candidate whose deduction is a substitution
  failure without a reason ("candidate template ignored: substitution
  failure [with T = int]"; `TemplState::body_invalid`), unlike one whose
  declaration is invalid (`Tdk::Invalid`, no candidate); a member of a
  class template made invalid is not instantiated ("no member named").
  A lambda's call operator is checked only when the lambda is declared
  `constexpr` or `consteval` explicitly; an implicitly constexpr one that
  breaks the rule is silently not constexpr ([expr.prim.lambda.closure]/5;
  clang `CheckConstexprKind::CheckValid`), checked again when an enclosing
  template is instantiated, its generic specializations following the
  pattern (`implicit_constexpr_lambda_body_vars`; WP-414). A local whose initialization sema
  rejected (no viable constructor) is marked invalid, and evaluating its
  declaration fails with no note (spec/oven.md "Classes").
  From C++23 clang 22 no longer applies `CheckLiteralType` during
  evaluation (spec/oven.md); clang 19 does.
- The same walk applies the statement rule (clang
  `CheckConstexprFunctionStmt`; WP-444): a GNU `goto *p` is allowed at
  no language level, "statement not allowed in {constexpr|consteval}
  {function|constructor}" at the `goto` when it comes before any
  non-literal variable, the function made invalid as above. Before
  C++23 a body that breaks neither rule but holds a label or a `goto`
  gets "use of this statement in a constexpr {function|constructor} is
  a C++23 extension" (`-Wc++23-extensions`) at the first one. Tests
  `tests/conformance/stmt.goto/computed-goto-constexpr-body{,-cxx20}.cpp`.
  Otherwise, before C++20, the first asm-declaration or try-block
  statement gets "use of this statement in a constexpr
  {function|constructor} is a C++20 extension" (`-Wc++20-extensions`),
  and a function-try-block, checked ahead of the walk, "function try
  block in constexpr {function|constructor} is a C++20 extension" (clang
  `CheckConstexprFunctionBody`, WP-890/WP-891). From C++20 an asm
  statement is valid in the body; evaluating it is not a constant
  expression. Tests
  `tests/conformance/dcl.constexpr/constexpr-asm-{try-cxx17,evaluated,never-constant}.cpp`
  (the last an xfail: the never-constant check only tries parameterless
  functions).
  In C++11 an iteration statement or `continue` is not allowed in the
  body at all (clang does not take them as C++14 extensions): "statement
  not allowed in constexpr function", the function made invalid (WP-455).
- **Missing return statement (WP-455, WP-1056; clang
  `CheckConstexprMissingReturn`).** The same walk notes whether the body
  holds a `return` statement (one in a discarded `if constexpr` branch
  counts; one in a lambda or a statement expression does not, the walk
  not entering expressions). When none is found in a constexpr or
  consteval function other than a constructor, and the body broke no
  rule above: before C++14 it is an error whatever the return type;
  from C++14 an error unless the return type is `void` or dependent (a
  templated function's undeduced `auto` or `decltype(auto)` counts as
  dependent; a non-template's was deduced `void` by then); from C++23
  (P2448R2) never. The error is "no return statement in
  {constexpr|consteval} function" (`sema_constexpr_body_no_return`) at
  the function's name, or at the `[` of a lambda, and the function is
  made invalid as above, so the never-constant check skips it: ending
  the body with `__builtin_unreachable()`, `__builtin_trap()`, a
  `[[noreturn]]` call, `std::unreachable()` or an infinite loop is this
  error in C++11 to C++20 (C++11: the loops are the statement error
  above), not "never produces a constant expression"; from C++23 those
  are accepted, and only `-Winvalid-constexpr` reports them as never
  constant. A destructor is not checked (it returns `void`). clang's
  `-Wc++98-c++11-compat` warning for the accepted C++14 case is not
  implemented.
- **When the rules run (WP-455).** As clang's
  `CheckConstexprFunctionDefinition`, which `ActOnFinishFunctionBody`
  runs after `DiagnoseInvalidJumps` and before the analysis-based
  warnings, kiln checks the virtual-base rule (`check_constexpr_virtual_base`),
  the statement and variable rules and the missing return in
  `end_function_body` (`check_constexpr_definition`), right after the
  jump-scope check: an error there suppresses `-Wreturn-type` and the
  other flow warnings (`hasUncompilableErrorOccurred`), where kiln used
  to emit them first. The never-constant check needs the attached body
  (it runs the Oven) and stays after it. A lambda's call operator gets
  the rules when the lambda is declared `constexpr` or `consteval`
  explicitly (`consteval` alone included, which kiln used to skip).
  Tests: `tests/conformance/dcl.constexpr/no-return-cxx{11,14,20,23}.cpp`,
  `no-return-invalid-constexpr-cxx26.cpp`,
  `no-return-type-warning-after-{error,goto}-cxx20.cpp`,
  `tests/conformance/dcl.fct.def.coroutine/consteval-lambda-coroutine.cpp`.
  Tests of the variable rule:
  `tests/conformance/dcl.constexpr/non-literal-local-cxx{20,23,23-19}.cpp`,
  `tests/conformance/dcl.constexpr/non-literal-local-template-cxx{20,23}.cpp`,
  `tests/conformance/dcl.constexpr/non-literal-local-lambda-cxx20.cpp`,
  `tests/conformance/expr.const/invalid-local-silent.cpp`.
- **Literal return and parameter types (WP-1123; clang
  `CheckConstexprReturnType` / `CheckConstexprParameterTypes`).** Before
  C++23 ([dcl.constexpr]/3, removed by P2448R2) the return type of a
  constexpr or consteval function other than a constructor, and each of
  its parameter types, is a literal type; `void` is not one before C++14
  ([basic.types]/10). `check_constexpr_definition` checks them after the
  virtual-base rule and before the body rules, in clang's order (return
  type, then parameters; the first failure wins):
  "{constexpr|consteval} function's return type 'T' is not a literal
  type" at the function's name, or "{constexpr|consteval}
  {function|constructor}'s Nth parameter type 'T' is not a literal type"
  at the parameter (after its type when unnamed), each with the
  `RequireLiteralType` note (as the variable rule above; before C++20 a
  user-provided destructor gets "'C' is not literal because it has a
  user-provided destructor", from C++20 "its destructor is not
  constexpr"); the function is made invalid, so no body rule or
  never-constant check follows. Not checked: from C++23 (not even under
  `-Winvalid-constexpr`), a dependent, undeduced or erroneous type, an
  incomplete class (its definition reports it), and an instantiation (as
  clang, which checks only the written definition; a non-dependent type
  in a template is diagnosed at the template). An explicitly `constexpr`
  or `consteval` lambda is checked; an implicitly constexpr one whose
  return or parameter type is not literal is silently not constexpr
  (`implicit_constexpr_lambda_body_vars`). Tests
  `tests/conformance/dcl.constexpr/non-literal-return-param-cxx{11,17,20,23}.cpp`,
  `non-literal-return-param-consteval-cxx20.cpp`.
- **`constexpr` destructor before C++20 (WP-1123; clang
  `err_constexpr_dtor`).** "destructor cannot be declared constexpr" at
  the specifier, for every declaration of a destructor
  (`act_on_special_member_declarator`). As clang the specifier is kept,
  so in C++11 a definition also gets the `void` return-type error above.
  Tests `tests/conformance/class.dtor/constexpr-dtor-cxx{11,17}.cpp`.
- **C++11 deduced return type (WP-1123; clang `GetFullTypeForDeclarator`).**
  Before C++14 a function declarator whose decl-specifier type is a
  placeholder has a trailing return type ([dcl.spec.auto], N3638):
  "'auto' return without trailing return type; deduced return types are
  a C++14 extension" for `auto`, "deduced return types are a C++14
  extension" for `decltype(auto)`, at the type specifier, only for the
  function declarator next to the name (not `auto (*f())()`, not a
  lambda's implicit `auto`); the return type recovers as `int`
  (`type_for_declarator_at`). Test
  `tests/conformance/dcl.spec.auto/no-trailing-return-cxx11.cpp`.

### 5.21 Template rules from the libstdc++ clusters (WP-323)
- **Default arguments in dependent template-ids** ([temp.arg.general]/9,
  [temp.over.link]/5): a dependent class template-id that leaves
  parameters to their defaults (`enable_if<B>`) gets a canonical type
  carrying the substituted defaults (`enable_if<B, void>`); the written
  arguments stay the sugar (`template_id`, `dependent_spec_full_args`).
  So an alias template's expansion and the written form are the same
  type, and redeclarations through either match (`std::swap`:
  type_traits:2824 vs move.h:226). Defaults are not added when a
  parameter is a pack, an argument is a pack expansion, or a dependent
  default fails to substitute. Tests:
  `tests/conformance/temp.over.link/libstdcxx-alias-equivalent-redecl.cpp`,
  `tests/conformance/temp.over.link/libstdcxx-default-arg-equivalent-redecl.cpp`.
- **`this->f<A>(...)` for an overloaded member template**
  ([temp.names]/3): a member of the current instantiation that names an
  overload set stays a `CXXDependentScopeMemberExpr`. When lookup in the
  pattern finds a member template under that name, `<` begins a
  template-argument-list (`dependent_member_names_template`,
  `act_on_dependent_member_template_args`; variant:1526). Test:
  `tests/conformance/temp.names/libstdcxx-this-member-template.cpp`.
  The object may also have the type `X<P...>` written inside `X` (or a
  pointer to it): that names the current instantiation like the
  injected-class-name ([temp.dep.type]/1.2), so `copy.f<10>()` needs no
  `template` (WP-686, abseil charconv_bigint.cc). Test:
  `tests/conformance/temp.dep.type/p1-current-instantiation-object-member-template-run.cpp`.
- **Access through a dependent base** ([class.protected]/1,
  [class.access.base]/5): in `has_access`, a class that does not
  (yet) derive from the naming class but has a dependent base anywhere
  in its hierarchy (`has_dependent_bases_in`) may derive from it after
  instantiation. The check is deferred (`speculative_friend`) and
  replayed per instantiation (`replay_dependent_access`), where it
  either passes or reports `'m' is a protected member of 'B'` under the
  instantiation note (messages_members.h:107). Tests:
  `tests/conformance/class.protected/libstdcxx-protected-static-dependent-base.cpp`,
  `tests/conformance/class.protected/libstdcxx-protected-static-dependent-base-recheck.cpp`.
- **Current-instantiation template arguments in an out-of-line return
  type** ([temp.dep.type]/1, [temp.over.link]/5):
  `rebuild_in_current_instantiation` also rebuilds the type arguments of
  a class template-id (`pair<typename tree<K, V>::base_ptr, ...>`). When
  the rebuilt arguments are no longer dependent (the member is a
  non-dependent typedef), the specialization is formed from their
  canonical types, keeping the written arguments as sugar, so it is the
  type the in-class declaration named. When they are still dependent,
  the template-id is formed again from the rebuilt arguments, defaults
  included (stl_tree.h:2107). The rebuild also looks through `*`, `&`
  and `&&` around such a member (`pair<typename X<T>::ptr&, bool>`,
  libc++ `__tree::__find_equal`, WP-398). A template-id is dependent only
  when a *canonical* argument is dependent ([temp.dep.type]/8, clang's
  `anyDependentTemplateArguments`): `pair<nbp>` with `using nbp =
  rebind<T, int>` (an alias of a dependent alias template specialization
  naming `int*`) is the concrete `pair<int*>`, written arguments kept as
  sugar. Tests:
  `tests/conformance/temp.dep.type/p8-out-of-line-nondependent-alias-arg.cpp`,
  `tests/conformance/temp.mem/libstdcxx-out-of-line-current-inst-return.cpp`,
  `tests/conformance/temp.mem/libstdcxx-out-of-line-current-inst-return-args.cpp`.
  A qualifier `typename X<P...>::template M<A...>::m` (or the type
  `X<P...>::template M<A...>` itself) is rebuilt as the specialization
  `M<A...>` of the member class or alias template `M` of the current
  instantiation, as the in-class `typename M<A...>::m` names it
  (`rebuild_ci_member_template_id`; libstdc++ valarray:1100
  `valarray<_Tp>::template _UnaryOp<_Name>::_Rt`, WP-332). Test:
  `tests/conformance/temp.dep.type/libstdcxx-out-of-line-member-template-qualifier.cpp`.
- **Out-of-line members of partial specializations, alias templates and
  nested classes** ([temp.dep.type]/1.2, /6, [temp.spec.partial.member],
  [temp.alias]/2; WP-553): `X<A...>::` written with a partial
  specialization's own arguments names the current instantiation also
  when a non-dependent non-type argument is written unconverted
  (`vec<2, T>`: `current_instantiation_at` converts the arguments as
  the partial specialization's declaration did), in a parameter or
  trailing position (inside, or in the declarator scope of, that
  specialization) as well as before the declarator-id. The rebuild before
  the declarator-id also resolves a qualifier chain through member
  classes (`typename A::B<P>::C::m`, `typename X<P>::C::m`:
  `rebuild_ci_nns_record`) and substitutes an alias template
  specialization again with its rebuilt arguments
  (`remove_extent_t<typename X<T, C>::element_type>`), so the
  out-of-line definition, static data member or member template matches
  the in-class declaration; a qualifier naming another typedef, another
  partial specialization or another nested class still matches nothing.
  Tests: `tests/conformance/temp.dep.type/out-of-line-current-instantiation-*.cpp`.
- **Member template-ids with non-template overloads**
  ([temp.arg.explicit]/4, [over.match.call.general]/1): an unqualified
  function template-id `f<A>(args)` names the whole overload set that
  lookup finds, member functions (`CXXMethod`) and using-shadows
  included. The non-template members are not viable with explicit
  template arguments (`widen_function_template_id`; bitset:1358
  `to_string`). Test:
  `tests/conformance/temp.arg.explicit/libstdcxx-member-template-id-overload-set.cpp`.
- **`T x({a, b})` in a template** ([dcl.init.general]/16,
  [temp.dep.expr]/3): a parenthesized initializer with a braced-init-list
  argument is kept as a `ParenListExpr` for instantiation in two cases:
  the variable's type is dependent (the list has no type of its own), or
  an element of the list is type-dependent (`braced_arg_type_dependent`).
  This is `defer_dependent_init` (<format>:439 `_Utf32_view`). Test:
  `tests/conformance/dcl.init.general/libstdcxx-paren-braced-init-dependent.cpp`.
- **Resolved operator calls at instantiation** ([temp.res.general]/6,
  [over.ref]/1, [over.match.oper]; WP-552): a `CXXOperatorCallExpr`
  resolved in the pattern (non-dependent operands) is formed again from
  its substituted operands by overload resolution, never by copying the
  selected function: `()`/`[]` as a call/subscript, `->*`, `,` and every
  other binary operator function through its built-in binary form, the
  unary ones and postfix `++`/`--` (the extra `int` operand dropped)
  through their unary form. `->` has no built-in form: a member access
  `x->m` re-forms the whole chain from the class object
  (`Sema::arrow_chain_object` strips the `operator->` calls together with
  the qualification conversions and materialized temporaries between the
  links), and any other `operator->` call (a pseudo-destructor `p->~I()`)
  is one link, `operator->` selected again for the substituted object
  (`Sema::rebuild_operator_arrow`). `new`/`delete` operator functions
  are never such a call; the `operator co_await` of a `co_await` is
  formed again from the operand as written. Tests:
  `tests/conformance/temp.dep/overloaded-operator-arrow-std.cpp`,
  `tests/conformance/temp.dep/overloaded-operator-arrow-chain.cpp`,
  `tests/conformance/over.oper/arrow-in-template-other-ops.cpp`,
  `tests/conformance/over.oper/arrow-in-template-run.cpp`.
- **The injected-class-name in pointers to members; base-qualified
  members** ([temp.dep.type]/1, [temp.local]/1, [dcl.mptr]/1,
  [expr.unary.op]/3, [class.qual]/1; WP-608). Inside a class template
  (or a partial specialization), the injected-class-name used as a
  nested-name-specifier (`Scan::`) denotes the current instantiation: its
  specifier type is the `InjectedClassName` type `Scan<P...>`, as when
  `Scan<P...>::` is written (`Sema::nns_component` takes
  `this_class_type` of a class). So a member-pointer declarator
  `typedef void (Scan::*Fn)();` has the dependent type
  `void (Scan<C>::*)()`, which instantiation substitutes to a pointer to
  member of the specialization; `this->*fn` with `this` of type
  `Scan<char>*` is then compatible. `&C::m` of a non-static member forms a
  pointer-to-member constant whether or not `C` is dependent; its class
  type is the declaring class's `this_class_type` (the current
  instantiation in the pattern, clang's `getTypeDeclType` of the parent),
  and its operand `DeclRefExpr` keeps the qualifier as written, so that
  instantiation forms `&C::m` again instead of the implicit member access
  `(*this).C::m` (`Sema::member_pointer_of`). A `?:` of two such operands
  keeps their common pointer-to-member type. A qualified-id `B::m` whose
  `B` is a base class of the current instantiation (dependent or not)
  looks `m` up in `B` only, in the pattern and again at instantiation:
  the implicit `(*this).B::m` of a data member keeps its qualifier on the
  `MemberExpr` (`Sema::field_access_extra`), so the instantiation's
  lookup of `m` starts in `B` and a derived member `m()` never hides it
  (libstdc++ `_State<_CharT>::_M_opcode()` returning
  `_State_base::_M_opcode`; `_Scanner<_CharT>`'s
  `&_Scanner::_M_eat_escape_ecma` in <regex>). Tests:
  `tests/conformance/temp.dep.type/injected-class-name-*.cpp`,
  `tests/conformance/basic.lookup.qual/class-qual-base-member-hidden-in-template.cpp`,
  `tests/conformance/re.alg/regex-match-libstdcxx.cpp` (and its
  `-libstdcxx16` / `-libcxx` siblings).
- **A pack before a defaulted parameter, named again at instantiation**
  ([temp.deduct.call]/1, [temp.param]/14): a parameter pack that is not
  the last template parameter is deduced from the call and the
  parameters after it take their defaults (or deductions) after it, never
  elements of the pack; a member template specialization the class
  template's pattern selected keeps exactly those arguments when it is
  named again in the instantiated class (spec/sema-templates.md,
  `respecialized_member_template`; WP-629). Tests:
  `tests/conformance/temp.deduct.call/pack-before-*.cpp`.

### 5.22 Rules from the libc++ `.verify` cluster B (WP-338)
- **An out-of-line constructor stays explicit** ([dcl.fct.spec]/4,
  [over.match.copy]/1.1): the explicit-specifier belongs to the first
  declaration (it is allowed only in the class). An out-of-line
  definition or member-template definition without `explicit`
  inherits the flag and any `explicit(expr)` operand from the previous
  declaration (`class/special/declare.rs`). Copy-initialization does
  not consider it, and the "explicit constructor is not a candidate"
  note points at the first declaration (`overload/diag.rs`). This
  covers `complex<float> cf = cd;`. Test:
  `tests/conformance/over.match.copy/explicit-ctor-out-of-line-definition.cpp`.
- **The underlying type of an incomplete enumeration** ([dcl.enum]/5,
  [meta.trans.other]): `__underlying_type(E)` for an enumeration
  without a fixed underlying type, before its `}`, is an error.
  The message is "cannot determine underlying type of incomplete
  enumeration type 'E'" (`sema_underlying_type_of_incomplete_enum`),
  with a "forward declaration of" note at `E`
  (`templ/builtins/transforms.rs`). Inside a class template
  instantiation the error is located at the declarator, not at the
  `__underlying_type` keyword. Types carry no locations, so tests give
  only the line. Test:
  `tests/conformance/dcl.enum/underlying-type-incomplete-enum.cpp`.
- **An invalid base makes the derived specialization invalid**
  ([class.derived]/2, clang `CheckBaseSpecifier`): instantiating a
  class whose base specialization is invalid, for example because a
  member typedef failed to substitute, marks the class invalid
  (`templ/class.rs` `class_body_with`). `underlying_type<E>::type`
  after the error is then quiet. Test:
  `tests/conformance/class.derived/invalid-base-makes-derived-invalid.cpp`.
- **An erroneous template argument is diagnosed once**
  ([temp.arg.general]/1): an argument that is an error expression,
  such as an undeclared name, is rejected for a type parameter without
  a second "must be a type" error (`templ/args.rs`
  `check_template_arg`, clang's `RecoveryExpr`). Test:
  `tests/conformance/temp.arg.general/erroneous-argument-single-diagnostic.cpp`.
- **`T{...}` of an invalid class in an instantiation**
  ([temp.inst]/2, [expr.type.conv]): when substituting a braced
  functional cast names a class whose instantiation is invalid (for
  example a failed `static_assert`), the expression fails silently
  (`templ/subst/exprs.rs` `invalid_class_braced_cast`). Only the
  `static_assert` is reported, with no "no viable conversion"
  (`std::numbers::e_v<int>`). `T()` and a non-template `I<char>{}`
  still diagnose, as in clang. Test:
  `tests/conformance/temp.inst/invalid-class-braced-cast.cpp`.
- **Attributes of instantiated static data members**
  ([dcl.attr.deprecated]/4, [temp.inst]/3): a static data member
  instantiated with its class, deferred in-class initializer or not,
  gets all of the pattern's attributes (`class_attrs`), not only
  `alignas`. So `numeric_limits<T>::has_denorm` warns at each use, and
  the out-of-line definition without the attribute inherits it
  through the redeclaration chain. Test:
  `tests/conformance/dcl.attr.deprecated/static-data-member-instantiation.cpp`.
- **"no template named 'X' in 'N'"** ([temp.names]/3): at namespace
  or class scope, a decl-specifier `N::X<...>` whose `X` is not a
  member of the non-dependent `N` gets `sema_no_member_template`
  (clang `DiagnoseUnknownTypeName` with `IsTemplateName`). The argument
  list is skipped and the declarators are not diagnosed again
  (`kiln_parse` `qualified_unknown_type_follows`,
  `parse_qualified_type_rest`). At block scope the statement is
  parsed as an expression, as clang does. Test:
  `tests/conformance/temp.names/no-template-named-qualified.cpp`.
  An unqualified decl-specifier `X<...>` whose `X` is undeclared is a
  template-name by [temp.names]/3.3 (P0846: an unqualified-id followed by
  `<` whose lookup finds nothing), so it gets clang's "no template named
  'X'" (`sema_no_template_named`; clang `ParseImplicitInt`) instead of
  "expected unqualified-id": at namespace or class scope always, at block
  scope when a declarator follows the argument list
  (`undeclared<int> x;`; the tentative parse takes `X<...> ident` as a
  declaration). The argument list is parsed and dropped, the type is
  `int` with the specifier marked invalid, and the declarators are not
  diagnosed again (`kiln_parse` `undeclared_template_type`,
  `undeclared_template_id_before_ident`; libc++
  `locale.stdcvt/remove.verify.cpp`, WP-358). Test:
  `tests/conformance/temp.names/undeclared-template-id-type.cpp`.
  At block scope `X<...>` before a ptr-operator or a cv-qualifier
  (`undeclared<int> *p;`) is ambiguous and the declarator decides, as for
  an undeclared name before a ptr-operator (spec/parse.md; `kiln_parse`
  `undeclared_template_id_before_ptr_operator`; `w<int> & i + 1;` stays
  an expression). Test:
  `tests/conformance/temp.names/undeclared-template-id-stmt.cpp` (WP-372).
- **Undeclared template-id in an expression** ([temp.names]/3.3,
  WP-372): an undeclared unqualified `X<...>` that is not called
  (`(undeclared<int> * 2)`, `undeclared<int>[3]`, `&undeclared<1>`) still
  takes the list as template arguments and reports only "use of
  undeclared identifier 'X'" (clang 22 returns `ExprError` for the
  annotated template-id). Before a binary operator or a closing token
  (`)`, `]`, `}`, `,`, `;`, `:`) the result is an error expression, so
  the enclosing expression and declaration go on and diagnose their other
  operands and declarators; before anything else (`(undeclared<int> 5)`)
  it is a parse error the enclosing construct skips. Followed by `::` it
  is "no template named 'X'" (clang's `ActOnCXXNestedNameSpecifier`).
  Under `-fclang-compat=19` the name is a delayed typo
  (`act_on_undeclared_template_id_typo`, clang 19's `TypoExpr`): an
  error expression parsing goes on around whatever follows, so clang
  19's "expected ')'" (`(v<int> 5)`) and "expected ';' after return
  statement" (`return v<int>{};`) follow, and the typo is reported at the
  end of the full-expression (WP-391).
  `kiln_parse` `parse_id_expression_rest`. Tests:
  `tests/conformance/temp.names/undeclared-template-id-expr.cpp`,
  `undeclared-template-id-expr-22.cpp` (`requires: clang>=22`),
  `undeclared-template-id-expr-19.cpp` (`requires: clang<22`).
- **Template-id of a function that is not called** ([temp.names]/3,
  WP-391): from C++20 an unqualified `f<...>` that ordinary lookup finds
  only non-member functions for, not followed by `(`, names nothing
  either. Clang 22 typo-corrects `f` to itself qualified from its
  namespace (`template_id_typo.rs`, `correct_function_template_id`):
  "use of undeclared identifier 'f'; did you mean '::f'?" (the qualifier
  as for a local friend, `correction_qualifier`), with "'::f' declared
  here" at the latest declaration of the first function declared, or at
  the using-declaration that is the only one found. The expression is an
  overload set of that one function without template arguments: a
  built-in operator resolves it to the function
  (`resolve_single_overload_candidate`, clang's
  `resolveAndFixAddressOfSingleOverloadCandidate`; `f<int> + 1` is
  "arithmetic on a pointer to the function type"), a conversion
  diagnoses it as an overload set ("address of overloaded function 'f'
  does not match required type"), and with a dependent template argument
  it is an error expression. There is no correction, only "use of
  undeclared identifier", when the functions belong to several
  namespaces, or are found both directly and through a using-declaration
  (several using-declarations included).
- **Recovered overload set** ([temp.names]/3, WP-408; clang 22): the
  overload set a corrected uncalled template-id stands for is a
  placeholder. A condition and a conditional's arms resolve its one
  candidate first (`resolve_single_overload_candidate`), so `if
  (val<int>)` warns that the address of '::val<int>' is always true and
  an arm decays to the function pointer; a placeholder type (`auto`,
  `auto *`, `auto &`, `decltype(auto)`) does not deduce from it
  ("address of overloaded function 'val' does not match required type
  'int (int)'" with "candidate function"); a default argument converts
  it to the parameter type with "passing argument to parameter" after;
  `sizeof`/`alignof` resolve it and reject the function type; `.m` on
  it is "base of member reference is a function; perhaps you meant to
  call it with no arguments?" and `->m` "member reference base type 'S
  ()' is not a structure or union". The corrected name prints with its
  qualifier (`typo_nns`; none when the path crosses a class). Tests:
  `tests/conformance/temp.names/undeclared-template-id-recovery-22.cpp`,
  `expr.cond/undeclared-template-id-arm-22.cpp`,
  `expr.sizeof/undeclared-template-id-operand-22.cpp` (and `-19.cpp`,
  clang 19's delayed correction of the same operands).
- **Template-id typo candidates** ([temp.names]/3, WP-399; clang 22's
  `CorrectTypo` from `DiagnoseEmptyLookup`, `correct_template_id_typo`):
  for an uncalled template-id whose name ordinary lookup finds nothing or
  only functions, the candidates are every identifier ordinary lookup
  finds within `(len+2)/3` character edits (`closer_typo_candidate` scans
  the interner), the keywords clang offers in that context
  (`typo_keywords`: type and storage specifiers, named casts, `this` in a
  non-static member function, statement keywords in a function with
  `break`/`continue`/`case`/`default` only where valid, `namespace` /
  `template` and the member specifiers outside one, `using` and
  `static_assert` always), and the name itself or a near name ordinary
  lookup does not find, qualified by every namespace and complete
  non-union, non-template class declaring it (`typo_scopes`). Distance:
  100 per character edit, 110 per qualifier component (spelled from the
  innermost common context, or from `::` when that path starts with an
  enclosing name). The least distance wins; a tie between different
  candidates is no correction, as is a winner whose rounded distance
  exceeds a third of a name of three or more characters (a shorter name
  is only corrected to itself qualified). The winner decides the
  outcome: a function, function template or static member function is
  "did you mean 'y'?" with "'y' declared here" (an overload set named by
  its one function callable with no arguments, else the first declared
  by qualified lookup / last by unqualified), recovered as its overload
  set (a function template with its template arguments); a non-static
  member function adds "must be called" / "without an object argument";
  a variable or enumerator outside a template adds the "expected to be a
  non-type template" pair; a data member or template parameter, a type
  (an alias template noted at its `using`) or a keyword is only
  suggested; a namespace, concept or variable template is no correction.
  Before the correction a name declared in an enclosing class
  (`diagnose_member_found_in_class`) is "member 'f' used before its
  declaration" (declared there) or "explicit qualification required to
  use member 'f' from dependent base class" (inherited, or when the one
  function callable with no arguments was picked), a note per
  declaration and a `this->` fix-it in a non-static member function;
  clang 19 says the same from the class lookup (also for an
  operator-function-id: "member 'operator+' used before its
  declaration"). Each "did you mean" carries a replacement fix-it over
  the name and, under clang 22, the name's range; a keyword suggestion
  has the range but no fix-it under clang 22 (WP-420). Only this path ranks
  candidates: the general undeclared-identifier path has no corrector.
  Tests: `tests/conformance/temp.names/template-id-typo-*-22.cpp`,
  `template-id-class-member-22.cpp`, `args-for-non-template-member-19.cpp`.
- **Member access followed by template arguments** ([expr.ref],
  [temp.names]/3-4, WP-399): `o.g<int>`, `p->g<>`, `X::g<int>` naming one
  non-template member (also of the current instantiation) followed by
  `<>` or `<` type-id ... `>` is "'g' does not name a template but is
  followed by template arguments" over the range of the list, with a
  note at the member, the list skipped and the access an error. The same
  holds for an operator-function-id (`::operator+<int>(a, b)`,
  `N::operator-<int>`, `this->operator+<int>(b)`; WP-420). Unqualified
  and called, such a name is assumed a template; when argument-dependent
  lookup finds none it is "use of undeclared 'operator*'" (clang 22: with
  the range of its first token, as for any call whose
  argument-dependent lookup finds nothing), or inside a class "'operator+'
  does not refer to a template"; an overloaded member or a list that
  need not be template arguments is parsed as a comparison. A bound
  member function as an operand of a binary operator is only "reference
  to non-static member function must be called". A member template-id of
  a dependent object without `template` is "use 'template' keyword to
  treat 'f' as a dependent template name" with the fix-it. Tests:
  `tests/conformance/expr.ref/member-access-followed-by-template-args.cpp`,
  `tests/conformance/temp.names/member-template-id-dependent-object.cpp`,
  `operator-function-id-non-template-args-*.cpp`.
- **Clang 19 correction to the qualified function** ([temp.names]/3,
  WP-399; clang 19's `TypoExpr` and `CorrectDelayedTyposInExpr`): under
  `-fclang-compat=19` an undeclared template-id whose best candidate is
  the name itself qualified (exactly one non-member function or using
  shadow, outside an instantiation, inside a delayed-typo scope) is
  speculatively rebuilt as a reference to that function, template
  arguments dropped (dependent ones too)
  (`speculate_template_id_typo`). The diagnostics engine holds every
  diagnosis (fatal ones aside) from then on (spec/diag.md "Held
  diagnostics"). At the correction point -- the end of the expression
  statement (after its `;`, and before the unused-result check, which is
  then skipped), a `return` operand (before the return's own checks:
  "cannot initialize return object", "void function should not return a
  value" still follow), an initializer (through the initialization, so
  a type mismatch fails), a condition, a decltype operand -- the
  speculation fails when a held error since it began (not a parse error,
  not another typo diagnosis) belongs to it: the first recovery
  expression built after the error contains an expression at the
  template-id's location, or no recovery expression followed. A failed
  speculation drops those errors and reports "use of undeclared
  identifier 'val'"; a successful one "use of undeclared identifier
  'val'; did you mean '::val'?" with "'::val' declared here". Typos
  corrected in one batch fail together (`(val<int>, val<int> + 1)`).
  Calls are never corrected on this path. The scopes (spec/parse.md "Compat-19 correction scopes",
  WP-408) differ only in what follows the correction point: an
  initializer (a declaration's, a condition declaration's) is filtered by
  its initialization, so a type mismatch fails the speculation; a flush
  scope (default member initializer, default argument, template
  argument, `decltype` operand, braced-list element) and an undeduced type (`auto`,
  `decltype(auto)`) correct before any conversion, so only an error in
  the rebuilt expression itself (`sizeof(::val)`) fails it, and the
  conversion's error then follows "did you mean". The location heuristic
  above is the same in every scope. Under compat 19 a condition that is
  itself the corrected name is not checked for `-Wpointer-bool-conversion`
  (clang 19 converts the `TypoExpr`), while any other full-expression
  warning about an expression built on the correction is held and issued
  after "did you mean" (`Sema::emit_after_typos`), or dropped with a
  failed speculation. Tests:
  `tests/conformance/temp.names/undeclared-template-id-correct-19.cpp`,
  `undeclared-template-id-scopes-19.cpp`,
  `undeclared-template-id-function-19.cpp`; clang 22:
  `undeclared-template-id-function-22.cpp`.
- **Clang 19 edit-distance candidates** ([temp.names]/3, WP-408; clang
  19's `CorrectTypoDelayed` and `TransformTypos`): the same speculation
  covers any candidate ordinary or qualified lookup finds
  (`template_id_typo_candidates`, ranked as for clang 22 but with no
  third-of-the-length bound), grouped by distance. clang 19 rebuilds the
  candidates of each group in turn and takes the first valid one unless
  another of the same group is valid too (`AmbiguousTypoExprs`), and
  never a type, template, namespace, concept or non-static member
  function (`typo_kind_19`: a variable, parameter, enumerator, binding,
  non-type template parameter, or a field with an implicit object
  argument is a value; a function, function template or static member
  function a function). Kiln speculates once (`TypoSpec`): on the one
  value of the nearest group, or on its first function when it has none
  (an overload set rebuilt as an unresolved name). A function speculation
  holds a `fallback` "did you mean" for the value of the same group or of
  the nearest later group with exactly one value, taken when the
  function fails; a valid function tied with a value or another function
  is `ambiguous`, reported uncorrected, unless the tie is one value and
  one function and the speculation is a copy-initializer the value's
  type and category cannot implicitly convert to (`settle_typo_tie`:
  `int (*p)(int) = valc<int>;` with `int vala; int valb(int);` is
  `valb`; WP-420). Two values in the nearest group
  are no correction. The correction point validates as before: a
  built-in operand or an initializer rejects a function, a `return`
  operand, condition or discarded expression accepts anything, so there
  a function tied with a value is ambiguous; an ambiguous or failed
  `return` operand is then an error expression
  (`Sema::holds_uncorrected_typo`), so no return conversion is checked.
  When the nearest group is keywords only, clang 19 replaces the name by
  the first one at once (`template_id_keyword_typo`): "did you mean
  'break'?" and the keyword is re-parsed where the template-id was
  ("expected unqualified-id" at it; `template` is consumed with
  "'template' keyword not permitted here" and "expected unqualified-id"
  at the next token, an error operand before a binary operator). Within
  one distance clang orders candidates by a string hash kiln does not
  model, so a keyword tied with a declaration (`swtch` with `switch` and
  a function `swatch`) may differ; an assignment or call argument that
  accepts only one of a tied value and function is still reported
  uncorrected. Tests:
  `tests/conformance/temp.names/template-id-typo-*-19.cpp`.

### 5.23 Library-facing diagnostics from the libc++ `.verify.cpp` clusters (WP-337)
Each rule names the libc++ test it fixed and its corpus test.
- **Deleted call operator** ([over.call.object]/1,
  [dcl.fct.def.delete]/2): a call through an object whose selected
  `operator()` is deleted reports "call to deleted function call operator
  in type 'T'" with the candidate notes (`oper.rs`,
  `diagnose_deleted_use` for `OperForm::Call`). A requires-clause naming
  a deleted member is unsatisfied, with "attempt to use a deleted
  function" as the reason (ranges `begin`/`end`/`rbegin`/`rend`). Test:
  `tests/conformance/over.call.object/deleted-call-operator.cpp`.
- **Invalid classes** ([temp.inst]/2): a class whose instantiation
  failed, or one that dropped a base-specifier that names no class
  ([class.derived.general]/2; `attach_bases` returns false), is marked
  invalid. Initializing an object of an invalid class reports nothing
  more and recovers (`is_invalid_class`). This covers list-init,
  parenthesized aggregate init, `static_cast` and functional casts. As
  an aggregate element, the invalid class takes one initializer
  (out_ptr/inout_ptr). Tests:
  `tests/conformance/temp.inst/invalid-class-quiet-init.cpp`,
  `tests/conformance/class.derived/dependent-base-not-class.cpp`.
- **typename-specifier after a failed qualifier**
  ([temp.res.general]/4): only the first failing component is
  diagnosed, and the parser skips the rest of the specifier
  (tuple_element). Test:
  `tests/conformance/temp.res/typename-failed-qualifier.cpp`.
- **Non-type argument of class type with no conversion**
  ([temp.arg.nontype]/1): reported with the converted-constant wording
  "value of type 'S' is not implicitly convertible to 'int'", at the
  argument (tuple_size_incomplete). Test:
  `tests/conformance/temp.arg.nontype/class-arg-no-conversion.cpp`.
- **Overload set as a non-type argument** ([temp.arg.nontype]/2,
  [over.over]/1; WP-420): an overload set (`f`, `&f`, `(f)`, `tf<int>`,
  `&S::m`) for a non-dependent parameter is resolved against the
  parameter type (pointer to function, reference to function, pointer to
  member function) before the converted-constant check. When no function
  matches, or the parameter has another type, it is "value of type
  '<overloaded function type>' is not implicitly convertible to 'int'"
  over the argument's range, plus (clang 22) "template parameter is
  declared here". A misspelled template-name before the arguments is
  corrected first ("did you mean '::val'?"); clang 19 then shows a
  corrected template-id naming one function with its function type.
  Tests: `tests/conformance/temp.arg.nontype/overload-set-argument-*.cpp`.
- **Conversions a non-type argument may not use** ([temp.arg.nontype]/2,
  [expr.const]/12; clang `CheckConvertedConstantConversions`): the
  standard conversion sequence (the one after a user-defined conversion)
  may use a pointer or pointer-to-member conversion only from
  `std::nullptr_t` (so `&s` is no `const void *`, `const B *` from `D *`,
  `int D::*` from `int B::*`, nor `0` a pointer), and a boolean
  conversion only from an integral or unscoped enumeration type.
  "conversion from 'S *' to 'const void *' is not allowed in a converted
  constant expression" at the argument's start, plus (clang 22)
  "template parameter is declared here"; an explicitly specified
  argument of a function template is an "invalid explicitly-specified
  argument" deduction failure. A cast (`(const void *)&s`) or a
  `const void *` constant is accepted (WP-407). Not yet modelled: the
  "while checking a default template argument used here" note when the
  failing argument is a default (clang notes the `>` of the
  template-id, which `check_template_args` does not receive). Tests:
  `tests/conformance/temp.arg.nontype/pointer-conversion-not-allowed.cpp`
  (+`-clang22`), `.../pointer-conversion-allowed.cpp`.
- **Non-type parameter types substituted from prior arguments**
  ([temp.param]/7):
  - `template <class T, T V>`: `V`'s substituted type is checked at the
    parameter, under a `PriorArgSubstitution` frame ("while
    substituting prior template arguments into non-type template
    parameter 'V' [with T = ...]").
  - A deduced placeholder (`template <Pair P>`) is checked at the
    argument, with "template parameter is declared here".
  - Implicit deduction guides return the canonical injected type.
  
  Covers the pair/array NTTP tests. Test:
  `tests/conformance/temp.param/nttp-structural-prior-args.cpp`.
- **`static_cast<void>`** ([expr.static.cast]/6): its result is never
  unused, unlike `(a == b);` or a comma's left operand
  (size_incompatible_comparison). Test:
  `tests/conformance/expr.static.cast/void-cast-not-unused.cpp`.
- **Assignment through a const-returning call** ([expr.assign]/1): the
  left operand may be a call whose return type is const-qualified
  (through a reference), or a non-const member of one through `.`. The
  error is "cannot assign to return value because function 'f' returns
  a const value", with a note at the return type (get_const). Test:
  `tests/conformance/expr.assign/const-return-call.cpp`.
- **Initializer-list constructors in implicit conversion sequences**
  ([over.ics.list]/7, [over.ics.rank]/3.3): a conversion through an
  initializer-list constructor is an ordinary user-defined conversion,
  with no rank of its own. So `one({1, 2})` with `one(pair<int, int>)` and
  `one(span<const int>)` is ambiguous (span initializer_list, P2447R4).
  Test: `tests/conformance/over.ics.list/init-list-ctor-ambiguous.cpp`.
- **`preferred_name`** (clang extension, `__attribute__` and
  `[[clang::]]`):
  - Placement: on a class template. On anything else it warns "only
    applies to class templates" and is ignored.
  - Argument: a single type, which must be a cv-unqualified typedef for
    a specialization of that template. Otherwise: "argument 'X' to
    'preferred_name' attribute is not a typedef for a specialization of
    'T'", with a note at the typedef.
  - Printing: a specialization prints as the typedef
    (`TypeNames::preferred_name`, `Ast::preferred_name_of`), with scope
    unless printed among a written template-id's arguments. This applies
    in C++ printing only.
  - Attachment (clang): a specialization takes the attribute only when
    it is instantiated from a primary template that has it
    (`instantiate_class`), or when a template-id names it while it is
    still undeclared after the attribute exists (`class_template_type`).
    Until then it prints as the template-id: a typedef written before
    the attribute does not attach it, so an uninstantiated
    `basic_string<char>` prints `s::basic_string<char>`.
    `TemplateTables::mark_preferred_named` records the attachment;
    `Ast::preferred_name_of` reads it (WP-366). Test:
    `tests/conformance/dcl.attr/preferred-name-attach-rule.cpp`.
  
  The pair/array NTTP tests print `std::string`. Test:
  `tests/conformance/dcl.attr/preferred-name.cpp`.
- **Substituted classes among written template arguments** ([temp.names]):
  a class substituted into a written template-id prints unscoped there
  (`'Box<Q, int>'`) and scoped elsewhere (`'N::Q *'`) (tuple.by.type).
  Test: `tests/conformance/temp.names/substituted-args-unscoped.cpp`.

### 5.24 C++23/C++26 cluster (WP-356)

- **`auto(x)` and `auto{x}`** ([expr.type.conv]/1-2, P0849;
  `forms.rs`): a functional cast to the placeholder `auto` deduces the
  type as for `auto v(x);`, so the result is a prvalue decay-copy of
  `x` (arrays and functions decay, cv dropped). clang's
  `BuildCXXTypeConstructExpr` diagnostics, at the type: `auto` before
  C++23 and `decltype(auto)` are `auto not allowed here`; an empty
  initializer, several expressions and a parenthesized or nested braced
  list are `sema_auto_expr_init_no_expression`,
  `sema_auto_expr_init_multiple_expressions` and
  `sema_auto_expr_init_paren_braces`; a failed deduction
  is `sema_auto_expr_deduction_failure`. A type-dependent operand keeps
  the placeholder until instantiation. Tests:
  `expr.type.conv/p0849-auto-cast*.cpp`.
- **Placeholder `_`** ([basic.scope.scope]/5, P2169): a declaration
  named `_` is name-independent when it is a block-scope variable of
  automatic storage duration, a structured binding not at namespace
  scope, an init-capture (spec/sema-lambda.md §2) or a non-static data
  member. Redeclaring such a `_` in the same scope is not a conflict;
  before C++26 it is `sema_ext_placeholder_var`
  (`-Wc++26-extensions`). A *use* of `_` that finds more than one is
  `sema_placeholder_ambiguous` with a `note_placeholder_declared` per
  declaration (clang `err_reference_to_placeholder_ambiguous`; latest
  first for unqualified lookup, in source order for member access).
  A lookup with several results is worded this way when its first
  declaration in source order is name-independent
  (`placeholder_ambiguity`). Tests:
  `basic.scope.scope/p2169-placeholder*.cpp`.
- **Array comparisons** ([expr.eq]/2, [expr.rel]/2, [depr.array.comp],
  P2865; `expr.rs`, clang `diagnoseTautologicalComparison`): `==`,
  `!=`, `<`, `>`, `<=`, `>=` with two operands of array type is, in
  C++26, `sema_array_comparison_cxx26` (`-Warray-compare-cxx26`, an
  error by default); in C++20 and C++23 `sema_depr_array_comparison`
  (`-Wdeprecated-array-compare`), or `sema_array_comparison`
  (`-Warray-compare`) when that is disabled and before C++20. Then,
  where run-time behaviour is diagnosed and both operands name
  non-reference variables, `sema_comparison_always`
  (`-Wtautological-compare`): distinct arrays compare unequal (`==`
  false, `!=` true, a relational "a constant"); the same one is a
  self-comparison. Operands decayed explicitly (`+a`), an array against
  a pointer, and comparisons in a template instantiation (clang checks
  the definition only; a dependent array type is not diagnosed) are not
  affected. Constraint satisfaction is an instantiation here (clang's
  `inTemplateInstantiation` counts `ConstraintsCheck`;
  `InstStack::in_instantiation_record`), so `requires(T& t) { t == t; }`
  holds for `int[5]` (WP-446, libc++'s `equality_comparable<int[5]>`).
  Tests: `expr.eq/p2865-array-comparison*.cpp`.
- P0847 lambdas and P2036: spec/sema-lambda.md §2, §3.1, §4.4. P2360
  and P3176: spec/parse.md. P0963: §4.9. P2741: §4.6.

### 5.25 `-Wconstant-logical-operand` (`expr.rs`, WP-403)

clang's `CheckLogicalOperands`, on the unconverted operands of a built-in
`&&` / `||` ([expr.log.and]/1, [expr.log.or]/1), before their contextual
conversion to `bool` (`check_constant_logical_operand`):
- If either operand, as written (no parentheses stripped), names an
  enumerator whose value is not 0 or 1, that is
  `sema_enum_constant_in_bool_context` (§5.26) and nothing more is
  diagnosed here, whether or not that warning is enabled.
- Otherwise `sema_constant_logical_operand` ("use of logical '&&' with
  constant operand", group `constant-logical-operand`, on by default) is
  issued at the operator, with the right operand's range, when the left
  operand's type is an integer or unscoped enumeration type other than
  `bool`, the right one's too and it is not value-dependent, the
  operator is not spelled by a macro, no template is being instantiated
  (a definition's non-dependent operands are checked), and the right
  operand folds laxly to an integer (`Sema::fold_rvalue_int`, clang
  `EvaluateAsInt`; `__builtin_constant_p` of a non-constant does not
  fold there, spec/oven.md "Trial evaluation"). The value must be other
  than 0 or 1 when the right operand is `bool` or its location is a
  macro's; any value warns otherwise.
- Notes: `note_logical_instead_of_bitwise_change_operator` ("use '&'
  for a bitwise operation", `|` for `||`) replacing the operator, and
  for `&&` only `note_logical_instead_of_bitwise_remove_constant`
  removing from the end of the left operand to the end of the right.
  Both fix-its are file edits (§7.10.1): the removal is dropped when
  either end is inside a macro expansion it does not begin or end
  (`a && G(5)` with `#define G(x) x; (void)0`); the notes stay.
- Tests: `expr.log.and/constant-operand*.cpp`, `expr.log.or/constant-operand.cpp`,
  `kiln_driver` `fixits_in_macro_expansions_like_clang`.

### 5.26 Integers in a boolean context (`int_in_bool.rs`, WP-410)

clang's `DiagnoseIntInBoolContext`, run by the implicit-conversion walk
(`check_implicit_conversions`, `impcast.rs`) on the source of every
implicit conversion to `bool` ([conv.bool]/1) of a completed
full-expression, with parentheses and implicit casts stripped; an
explicit cast to `bool` is not checked. A literal operand is an
`IntegerLiteral`, or one directly under a unary `-` (the literal's own
value is used); no parentheses or implicit casts are looked through.
- `<<` ([expr.shift]): a left operand that is the literal 0 is
  `sema_left_shift_always` ("converting the result of '<<' to a boolean
  always evaluates to false", group `tautological-constant-compare`
  under `tautological-compare`, on by default). Both operands literal,
  the right literal's top bit clear and the expression not
  value-dependent: the same warning with the shift's value, folded as
  clang's constant folder does outside a constant context (a negative
  count shifts right, a count of at least the width shifts by the width
  less one, the result wraps to the promoted type). Otherwise, when the
  shift's type is a signed integer type, `sema_left_shift_in_bool_context`
  ("...; did you mean to compare with '0'?", group
  `int-in-bool-context`, off by default, in `-Wmost`); at
  -fclang-compat=19 `sema_left_shift_in_bool_context_expr` ("...; did
  you mean '(E) != 0'?" with the shift printed as written). At the
  operator.
- Conditional ([expr.cond]): both arms literal, not both of value 0 or
  1, and both non-zero is `sema_integer_constants_in_conditional_always_true`
  (`tautological-constant-compare`) at the start of the condition. Each
  arm whose type is not `bool` is then checked in turn (clang
  `CheckConditionalOperator`).
- `&&` / `||` ([expr.log.and], [expr.log.or]): an operand, as written,
  naming an enumerator whose value is not 0 or 1 is
  `sema_enum_constant_in_bool_context` ("converting the enum constant
  to a boolean", `int-in-bool-context`) at the operator, from
  `builtin_binary` before the operands are converted.
- clang's `'*' in boolean context` is C only and not issued.
- Templates ([temp.inst], WP-419): `check_implicit_conversions` skips a
  value- or type-dependent full-expression (clang's
  `CheckImplicitConversions`), so `return N << 1;` and `(x << 1) && N`
  are checked only in each instantiation. A non-dependent one is checked
  in the definition and again in each instantiation, which runs the walk
  on the shared tree from its `act_on_condition` / `act_on_return` /
  full-expression actions. The warnings above are
  `per_instantiation()` (spec/diag.md §7.4), so each instantiation
  reports them again with its instantiation note. In an instantiation
  the walk also re-runs the `&&` / `||` enumerator check on each
  operator of the full-expression (`recheck_enum_constants_in_bool`;
  clang rebuilds every operator, kiln shares non-dependent ones); an
  operator rebuilt by substitution is then reported twice at one
  location and context and deduplicated. A substituted non-type
  argument is not an integer literal (`a ? N : 5` is not warned).
- Tests: `expr.shift/shift-bool-always.cpp`,
  `expr.shift/shift-in-bool-context-{19,22}.cpp`,
  `expr.cond/cond-int-constants-bool.cpp`,
  `conv.bool/enum-constant-in-bool*.cpp`,
  `conv.bool/int-in-bool-per-instantiation-{19,22}.cpp`.

### 5.27 Always-non-null comparisons (`null_compare.rs`, WP-411)

clang's `DiagnoseAlwaysNonNullPointer`, comparison forms
([conv.ptr]/1, [basic.compound]/3, [dcl.ref]/5, [expr.prim.this]). A
built-in `==` / `!=` with exactly one null pointer constant operand
(`conv::is_null_pointer_constant`: `nullptr`, `__null`, a zero integer
literal) checks the other, unconverted operand `E` in `comparison()`,
before the usual conversions, so it applies alike in function bodies,
default member initializers, namespace-scope and constexpr initializers
and unevaluated operands:
- Skipped when `E`'s location is a macro location and it, or the null
  constant's start, was written in a macro body (clang
  `IsInAnyMacroBody`); a macro argument is diagnosed.
- Through implicit conversions only: `this` is `sema_this_null_compare`.
  Then, through parentheses, a unary operator other than `&` ends the
  check; `&X` checks `X`: a variable or member of reference type or a
  call to a function returning a reference (through parentheses) is
  `sema_address_of_reference_null_compare` (with
  `note_reference_is_return_value` at the function), both
  `-Wtautological-undefined-compare`.
- Otherwise `X` (or `E`) must be a `DeclRefExpr` or `MemberExpr` (not
  parenthesized) of a non-weak declaration: `&` of a non-function, or
  without `&` a function or an array, is `sema_null_pointer_compare`
  ("comparison of {address of|function|array} 'X' ...",
  `-Wtautological-pointer-compare`, `X` printed as written). A function
  adds `note_function_warning_silence` (insert `&`) and, when it can be
  called without arguments and returns a pointer (or an integer, against
  a literal zero), `note_function_to_function_call` (append `()`).
- An operand holding a clang 19 delayed typo (`holds_typo_expr`: an
  undeclared template-id corrected to `::val`) is not checked: clang's
  `TypoExpr` makes the comparison type-dependent and the correction
  rebuilds it under a SFINAE trap, which drops warnings
  (`temp.names/undeclared-template-id-correct-19.cpp`).
- The result selects `true` for `!=`, `false` for `==`. The location is
  the member name of a member access, else the start of the operand;
  ranges are the operand's and the null constant's.
- A template definition checks its non-dependent comparisons; the
  instantiation warns again: a rebuilt comparison re-checks, and a shared
  non-dependent operand reissues the definition's warning from
  `null_compare_rewarn` (`mark_shared_expr_referenced`'s walk), in
  definition order, with the instantiation notes.
- Before `DeclRefExpr`/`MemberExpr` (WP-421): a call (through
  parentheses and implicit conversions) to a function declared
  `returns_nonnull` is `sema_nonnull_expr_compare` "comparison of
  nonnull function call 'E' ... is '{true|false}' on first encounter",
  `E` printed as written; a lambda's conversion to a function pointer
  (`l == nullptr`, an implicit `operator R(*)()` call), outside any
  instantiation, is `sema_null_pointer_compare` select 3 ("address of
  lambda function pointer conversion operator"), at the lambda's
  object, ranged from its `[`; a parameter made non-null is the
  "nonnull parameter 'p'" form. Both nonnull forms add
  `note_declared_nonnull` at the attribute.
- A parameter is non-null (`nonnull.rs` `nonnull_param_attr`) by its
  own `nonnull` attribute on any declaration of its function, else by a
  function `nonnull` attribute with no index or naming its one-based
  index (the implicit object parameter counts); a function template's
  pattern has none (clang checks the specialization). Not once the
  function body has assigned it or taken its address before the check
  (`FnState::modified_params`, clang `ModifiedNonNullParams`, recorded
  by `assign` and `address_of`).
- Conversion to `bool` (spec/sema-conv.md §3) shares the classifier
  (`NullUse::Bool`), with the context location instead of the null
  constant: the condition's keyword (`if`, `while`, `for`, `do`), the
  operator of `?:`, `!`, `&&`, `||`, the variable of an initialization,
  `return`, the start of a call. Its nonnull forms are
  `sema_cast_nonnull_to_bool` "nonnull {function call|parameter} 'E'
  will evaluate to 'true' on first encounter".
- Ranges: the operand's (`expr_range`, so a template-id `ft<int>` up to
  its `>`, recorded by the parser with `set_template_id_end`) and the
  null constant's or the context token's; the `()` fix-it goes after the
  operand's last token. A null constant written as a macro argument keeps
  its spelled range.
- Not checked: a type- or value-dependent full-expression root
  (clang `CheckImplicitConversions`); a bool conversion re-checks per
  instantiation from the full-expression checks, so it keeps no
  `null_compare_rewarn` entry. An instantiation shares the definition's
  converted null operand (`NullToPointer` cast), which is looked
  through before the null-constant test.
- Attribute checks (`nonnull.rs`): `nonnull(i...)` indices are integer
  constants (`sema_attribute_param_not_ice`, a value-dependent one too);
  on a function (`sema_attribute_wrong_decl_type` otherwise; a parameter
  takes it unindexed); an index past the parameters of a non-variadic
  function or 0 is `sema_attribute_param_out_of_bounds`, 1 in an
  implicit object member function `sema_attribute_implicit_this_arg`
  (both drop the attribute); an index of a non-pointer parameter is
  dropped with `sema_attribute_pointer_args_only` (when all are dropped
  the attribute covers every parameter, as in clang); no index on a
  non-variadic function without a pointer or dependent parameter (not
  from a macro) is `sema_attribute_nonnull_no_pointers`.
  `returns_nonnull` needs a function (`sema_attribute_wrong_decl_type`
  "Objective-C methods and functions") returning a pointer
  (`sema_attribute_pointers_only`). A dependent type counts as a pointer
  from clang 22 on; clang 19 (`-fclang-compat=19`) warns.
  Not implemented: the parameter-declaration and return-type ranges of
  those warnings (kiln keeps no declarator source range), and
  `-Wnonnull` "null passed to a callee that requires a non-null
  argument".
- Tests: `expr.eq/null-pointer-compare.cpp`,
  `expr.eq/null-pointer-compare-forms.cpp`, `expr.eq/nonnull-compare.cpp`,
  `conv.bool/pointer-bool-conversion.cpp`, `dcl.attr/nonnull-attribute*.cpp`.

### 5.28 Vector conversions (`conv/scs.rs`, WP-453)
The implicit conversions between two distinct vector types (clang
`IsVectorConversion`), by compatibility level:
- At 22 (clang 20+): an integer ext vector converts to a `bool` ext
  vector with the same lane count, a Conversion-rank `BooleanConversion`
  second step (`SecondStep::BooleanConversion`, so an exact overload
  wins and a candidate needing it ties with any other Conversion-rank
  match) applied as `CK_IntegralToBoolean` straight to the `bool` vector
  type; each lane is true when nonzero. In initialization, assignment,
  a return, a call argument, template argument deduction's later
  conversion, `static_cast` and a C-style cast alike. No other pair of
  distinct ext vector types converts: a floating, GNU (`vector_size`)
  or different-lane-count source, and a `bool` vector to an integer
  vector, are `cannot initialize ...` / `C-style cast from vector ...
  of different size`.
- List-initialization of a `bool` vector from an integer vector
  (`b4 a{I}`, `b4 b = {I}`) is a type narrowing ([dcl.init.list]/7;
  clang 22 `type 'i4' (...) cannot be narrowed to 'b4' (...) in
  initializer list` with `insert an explicit cast to silence this
  issue`; `init.rs` `check_narrowing_impl` admits a vector target).
- The conversion is not a constant expression (clang 22's
  `VectorExprEvaluator` has no case for it): the Oven reports the generic
  `subexpression not valid in a constant expression`
  (spec/oven.md "Vectors").
- At 19 (clang 19) none of these convert: `cannot initialize ...`,
  `assigning to ... from incompatible type`, `no known conversion`,
  `static_cast from ... is not allowed`.
Tests: `ext.vector/bool-from-int-vector-clang22.cpp`,
`ext.vector/bool-from-int-vector-diagnostics-clang22.cpp`,
`ext.vector/bool-from-int-vector-19.cpp`, `ext.vector/ir-bool-from-int-vector.cpp`;
codegen spec/codegen.md §7u.

### 5.28 `-Winteger-overflow` (`int_overflow.rs`, WP-458, WP-468)

A full-expression that is not constant-evaluated is checked for signed
overflow ([expr.pre]/4) once it is complete (`act_on_full_expr_at`;
clang `CheckCompletedExpr` → `CheckForIntOverflow` →
`EvaluateForOverflow`):

- **Where.** Not a constant-expression (`act_on_constant_full_expr`), an
  `if constexpr` condition, a `constexpr` variable's initializer, an
  unevaluated operand or a value-dependent full-expression: there the
  evaluation's own error reports the overflow (`constexpr int h = g + 3;`
  is the error and its note only). A non-dependent full-expression in a
  template is checked at the definition and again in each instantiation
  (`per_instantiation`), as clang.
- **What is evaluated.** The full-expression, parentheses and casts
  ignored, if it is a binary or unary operator; otherwise the elements of
  an initializer list as written (not `int{x}`), the arguments of a call
  or a construction, an array index, a compound literal's initializer
  and an array `new`'s size, in turn. A top-level conditional is not.
- **Evaluation.** A signed integer `+`, `-`, `*` or unary `-` (at most
  128 bits) whose operands have values (an integer literal, or a lax fold,
  `fold_rvalue_int`, so a `const` or `constexpr` variable reads as its
  value) and whose exact result is outside its type warns `overflow in
  expression; result is -2'147'483'646 with type 'int'` at the operator
  (the wrapped value with digit separators in clang 19 and 22), then
  yields the wrapped value, so an operator on it can warn again
  (`(g + 1) + (g + 1)` warns three times). Both operands are evaluated
  even when one has no value; integral casts convert the value; a
  conditional evaluates the chosen arm when its condition folds and every
  part otherwise; `&&` / `||` skip a right operand the left one decides.
  Other expressions are searched for such operators (calls' arguments
  included; not `sizeof`, `noexcept`, `typeid`, a lambda or a statement
  expression) and folded only when they contain none. Unsigned
  arithmetic is silent. The walk is bounded (65 536 nodes, depth 256)
  and skipped when the warning is disabled at the full-expression.
- **Folds that cannot succeed (WP-798).** An operand is not folded when
  every evaluation of it reaches a call of a member function the Oven
  cannot call ([expr.const]/10.2; the Oven's own rule: not `constexpr`
  on its latest declaration, and neither virtual, trivial, deleted nor a
  closure's conversion function or invoker): looking through calls,
  casts, member accesses, temporaries and operators other than `&&` /
  `||`, nothing guarded by a condition. The fold would compile a thunk
  and fail at that call; its answer is the same (no value, no record).
  A failed fold that is tried gives its thunk back (spec/oven.md
  "Evaluation entry points"). Test: driver
  `overflow_scan_skips_folds_that_cannot_succeed`.
- **Calls (WP-468).** An operand that calls a `constexpr` or
  `consteval` function or constructor (directly or below it; the cheap
  filter counts such a call as a candidate) is evaluated in the Oven with
  the body (`Oven::fold_for_overflow`, clang `EM_IgnoreSideEffects` with
  `CheckingForUndefinedBehavior`): a lax fold in which a signed `+`, `-`,
  `*` or negation that overflows (`Oven::wrap_overflow`, also a compound
  assignment's `+=`, `-=`, `*=`, and `++` / `--`) yields its wrapped value
  and is recorded; the evaluation goes on. Each record warns at its
  operator, in order, with the type of the operation (the compound
  assignment's type for `+=`), but `++` and `--`, which clang only notes
  (`HandleOverflow`). A function called twice warns twice at the same
  place (`inc(g) + inc(g)`). The evaluation enters no nested evaluation,
  global initialization or `__builtin_constant_p` in this mode and is not
  memoized. When it fails without a record (a non-constexpr callee, a
  read of a non-`const` variable), the operand is searched as above, a
  constexpr call found under it is evaluated in turn. A call that is the
  full-expression itself is not evaluated (only its arguments), as
  clang. Shifts, division and `<<=` never warn here.
- **Default arguments and valued invocations (WP-1171).** clang's
  `handleCallExpr` evaluates a call's arguments before it looks at the
  callee and `VisitCXXDefaultArgExpr` visits the default argument, so
  an overflow in a default argument warns at each use under an operator
  (`f() + 1` with `int f(int = INT_MAX + 1)`), whatever the callee. The
  search descends into a `CXXDefaultArgExpr`'s argument
  (`default_arg_expr`) instead of evaluating the call, and reads a
  `ConstantExpr` that has a value (or a default-use value) as a leaf,
  never evaluating the invocation again (clang `VisitConstantExpr`). A
  default argument rebuilt at the use (§5.10) is also checked on its own
  there, before the call's full-expression. Test: driver
  `immediate_invocations_are_not_folded_again`; corpus
  `dcl.fct.default/integer-overflow-in-default-argument.cpp`.
- **Reporting.** The warning carries the operation's source range and is
  reported as clang's `Diags.Report`: no instantiation notes (in a
  template a non-dependent overflow warns at the definition and in each
  instantiation, notes-free), not deduplicated against another
  full-expression or another overflow of the same one (the context key is
  the instantiation's, the full-expression and the overflow's ordinal), so
  checking a full-expression twice does not repeat it.
- **Compound assignment.** `x op= y` is checked as a binary operator
  would be: `<<=` / `>>=` diagnose a constant count that is negative or
  not less than the promoted left type's width (`-Wshift-count-*`), `/=`
  and `%=` a constant zero divisor (`-Wdivision-by-zero`), at the
  operator with the right operand's range, through the runtime-behaviour
  deferral (clang `CheckShiftOperands` / `CheckMultiplyDivideOperands` /
  `CheckRemainderOperands` with `IsCompAssign`).
- **Never constant (WP-468, WP-873, WP-945).** A `constexpr` or `consteval`
  function that can never be constant is ill-formed, no diagnostic
  required ([dcl.constexpr]/6); clang runs `isPotentialConstantExpr` on
  each written definition when `CheckConstexprFunctionBodies` is on:
  before C++23, else only under `-Winvalid-constexpr` (the last of
  `-Winvalid-constexpr` / `-Wno-invalid-constexpr` decides,
  `DiagEngine::group_flag`; `-Weverything`, `-Werror=` and pragmas do
  not turn it on). Kiln (`never_constant.rs` `check_never_constant`,
  called from `constexpr_body.rs` after the body's own warnings, for
  functions, members, constructors, destructors, conversions and
  explicitly `constexpr` or `consteval` lambdas; not in a template or an
  instantiation, not invalid, so not after a missing-return error, §5.20
  "Missing return statement") reports `{constexpr|consteval}
  {function|constructor} never produces a constant expression`
  (`sema_constexpr_never_constant`, an error by default, group
  `invalid-constexpr`, so `-Wno-error=invalid-constexpr` makes it a
  warning) at the function's name (`~` for a destructor) with the
  failure's notes, when the first failure that does not stem from an
  unknown value lies in the function's own body:
  - parameters, `this`, captures and (in kiln) every local variable are
    unknown, and a read of one fails silently; evaluation goes on past
    it, and the first reportable failure wins;
  - a call of any constexpr function is unknown and silent (clang
    `CheckCallLimit` refuses every nested call; the oven's potential
    mode, `Oven::evaluate_potential` and `evaluate_call` past its own
    frame, fails such a call with `Fault::Quiet`), so a failure inside a
    callee, an undefined callee, `new` and `__builtin_is_constant_evaluated`
    are never reported;
  - reportable: a call of a non-constexpr function (with `declared
    here`), a read of a non-const global, a `throw` before C++26 (from
    C++26 a throw is a constant subexpression, P3068), an overflow, and
    any other failure whose notes are not about an uninitialized read, a
    leak, an unknown initializer, an undefined callee or an unhandled
    exception;
  - a non-member or static function without parameters returning a
    scalar or `void` is evaluated as a call (`Oven::evaluate_call`);
    any other body is walked in order (constructor mem-initializers and
    default member initializers first) as far as the path is known: a
    subexpression that names nothing unknown is evaluated on its own,
    one that does is taken apart operand by operand (`&&`, `||` and
    `?:` as in `enable_if` conditions); statements follow clang's
    `EvaluateStmt` (WP-1133):
    - an expression statement or `for` increment whose value is unknown,
      or which may call a function (a call of anything but a builtin, a
      non-trivial constructor), or a variable initializer that fails
      quietly, notes a side effect (clang `EvaluateIgnoredValue`,
      `noteSideEffect`) and the walk goes on: past a call with unknown
      arguments and past a callee's quiet failure alike, as clang refuses
      every nested call in this mode. A discarded glvalue is evaluated
      as an lvalue only (clang `EvaluateIgnoredValue`), so naming a
      by-value parameter or a local, its member (not through `->`), its
      address, or a comma, parentheses or a `void` cast of these notes
      nothing (WP-1150); a reference parameter, `this`, or a read of a
      parameter's value notes a side effect;
    - destroying an object fails quietly (clang `HandleDestruction`)
      when its class has a non-trivial destructor, and, once a side
      effect has been noted, for a block's local variables and a
      discarded class prvalue. The failure stops the walk at the end of
      the statement (a discarded class prvalue's temporary, a
      condition's temporaries) or block (its locals, a lifetime-extended
      temporary); inside an `ExprWithCleanups` (a temporary bound by a
      member access, or discarded by a cast) it only notes a side
      effect;
    - an `if`, `while`, `for` or `do` condition must be known (an unknown
      one, a structured binding or a range-based `for` stops the walk).
      A condition variable is defined like a local below, and an unknown
      one stops the walk (clang `EvaluateCond`, `ESR_Failed`). A loop
      runs while its condition is true, `break` leaves it and `continue`
      goes to its increment and condition. Each `while` iteration, and
      each `for` iteration's condition variable, condition, body and
      increment, is a scope for its locals (clang `BlockScopeRAII`,
      `IterScope`). Each
      statement evaluated takes a step: a walk that runs out (clang
      `-fconstexpr-steps`, default 1048576) fails with "constexpr
      evaluation hit maximum step limit; possible infinite loop?" at the
      statement clang would be about to evaluate. A loop whose
      statements name only known locals (no `return`, label, `goto`,
      `try`, range-based `for`, `[[assume]]` or `this`/parameter in it)
      runs in one Oven evaluation however many iterations it takes
      (`Oven::evaluate_loop_in_frame`, WP-1163), counting steps as clang
      does, with the remaining step budget; it reports its failure at
      the statement it fails at, step limit included. Any other loop
      runs iteration by iteration; when an iteration leaves the frame's
      bytes as it found them and takes steps, the walk jumps to the step
      limit instead of running every step;
    - a `switch` with a known condition runs its body from the matching
      `case` or `default` label, searched as clang does (into blocks,
      `if` branches, loop bodies and labelled statements; a skipped
      static variable still fails), and `break` leaves it; no matching
      label skips the body. The search does not enter a try-block or a
      statement expression, so a label found only there (clang
      `ESR_CaseNotFound` from `EvaluateSwitch`) fails with "this use of
      statement expressions is not supported in a constant expression"
      at the label (WP-1150);
    - a `try` statement (and a function-try-block's body) runs its try
      block only; a label is its sub-statement; a `goto` (forward,
      backward or computed) fails with "subexpression not valid in a
      constant expression" at the statement;
    - the walk stops at a `return` and an `[[assume]]`;
    - locals (WP-1150). clang keeps a body's locals in the call frame
      while the parameters stay unknown. Likewise the walk tracks each
      block-scope variable and condition variable the Oven can hold in a
      potential-constant frame (spec/oven.md, `Oven::frame_local`: no
      reference, pointer to member, union, `mutable` or volatile
      subobject, no destructor). Pointer locals are tracked (WP-1163):
      null, the address of a local, a global or a string literal, and
      pointer arithmetic on those, so `int *p = nullptr; return *p;`
      fails with clang's null-dereference note. A pointer that would
      dangle once its evaluation ends (into the heap, a temporary, a
      block-scope object of that evaluation) makes the evaluation fail
      quietly. A full-expression, condition or initializer that
      names no parameter, `this` or untracked local is evaluated in
      that frame (`Oven::evaluate_in_frame`): modifications stay, and a
      failure is reported with its notes. One that names both is also
      evaluated in that frame first, in clang's order (WP-1163): a
      failure before its first unknown read is reported, and that read
      fails quietly (clang's potential mode); the walk then takes it
      apart, still reporting an operand that names no local. An
      assignment evaluates its right operand first. A local's value is
      never checked as a constant result (clang does not in potential
      mode), and the read of an object whose lifetime ended in an
      earlier evaluation fails quietly. A reference local bound to a
      tracked local designates it. A local whose initializer is unknown,
      or that is not tracked, is unknown: reading it notes a side
      effect. Once a side effect has been noted, every local is unknown
      (clang fails quietly on any local access after `noteSideEffect`).
      After 4096 frame evaluations in one body (only a loop the walk
      runs iteration by iteration reaches it), the walk treats its
      locals as unknown;
  - the walk also fails, as clang's `EvaluateStmt` (WP-1123), at the
    definition of a static or `thread_local` local variable not usable in
    constant expressions ("control flows through the definition of a
    {static|thread_local} variable" at the variable, before its
    initializer; a block-scope `extern` declaration defines nothing), and
    at the `}` of a body that does not return when the function returns
    non-`void` ("control reached end of constexpr function"; not a
    constructor, destructor or function-try-block). The Oven's `evaluate_call`
    applies the same static rule, `thread_local` and a `const` integral or
    enumeration static (taken as usable) included;
  - the callee of a direct call (a decayed reference to a function, in a
    `CallExpr` or `CXXOperatorCallExpr`) is not evaluated as a value:
    naming a builtin or a consteval function there is no failure, only
    the arguments (and a member call's object) are walked (WP-962);
  - builtins follow clang's own table, not the Oven (WP-945): kiln's
    Oven folds more than clang's evaluator (the P0533 `<cmath>` builtins
    and their library twins, as GCC does) and notes some calls its own
    way, so a builtin call never takes its result or failure from the
    Oven. `Sema::clang_constexpr_builtin` (set by the parser) asks
    kiln_pp's `__has_builtin` / `__has_constexpr_builtin` tables for the
    target, `-std`, GNU mode and `-fclang-compat`
    (`__builtin_abs` is constexpr from clang 22):
    - a non-library builtin clang does not evaluate (`__builtin_floor`,
      `__builtin_trap`, `__builtin_sqrt`, `__builtin_unreachable`) fails
      at its callee, before its arguments: "subexpression not valid in a
      constant expression" at the callee's start;
    - any other non-library builtin (constexpr in clang, or unknown to
      it) is opaque: its arguments are walked and its value is unknown;
      `__builtin_constant_p`, `__builtin_object_size`,
      `__builtin_dynamic_object_size`, `__builtin_classify_type`,
      `__builtin_assume` and `__assume` walk no argument,
      `__builtin_expect{,_with_probability}` only the first;
    - a library builtin (C linkage, `lib_builtin_info`) that clang
      constant-evaluates (`strlen`, `memcpy`, `strcmp`, `memcmp`, and
      in GNU modes `bcmp`, WP-1055) always fails:
      "non-constexpr function 'NAME' cannot be used in a constant
      expression" at the call, without `declared here` (clang's
      `isBuiltinCall` refusal outside a constant-evaluated context);
    - any other library builtin (`fabs`, `abs`) is an ordinary
      non-constexpr function: its arguments are evaluated first, so a
      call fails only when they are all known, with `declared here` at
      the latest declaration;
    - a decayed builtin that reaches the Oven other than as a direct
      callee (`BuiltinFnToFnPtr`) gets the generic "subexpression not
      valid" note, as clang's `PointerExprEvaluator` has no case for it.
    For a function evaluated as a call (below), a first walk in which
    only such builtin calls can fail runs before the Oven call, so the
    Oven never decides a builtin call. A failure noted as a kiln
    limitation (`kiln: ... not supported yet`) is never reported;
  - an `asm` statement fails ("subexpression not valid in a constant
    expression" at the statement, clang `EvaluateStmt`'s default), with
    or without parameters (WP-915);
  - a function declared in a system header is never checked, even under
    `-Wsystem-headers` (clang skips `CheckConstexprFunctionBody` there;
    libstdc++'s `<cmath>` overloads call `__builtin_acosf`);
  Kiln never reports a function clang accepts. It misses some clang
  reports: a pointer local holding a heap allocation is not kept between
  evaluations (clang's "read of heap allocated object that has been
  deleted"), and a range-based `for` stops the walk (WP-1163
  follow-ups). Under the default `-fclang-compat=22` a local of
  non-literal type is not itself a failure from C++23 (P2448R2), as in
  clang 22.
- Tests: `expr.add/integer-overflow-folded.cpp`,
  `expr.add/integer-overflow-constexpr-error.cpp`,
  `expr.pre/integer-overflow-in-call.cpp`,
  `expr.ass/compound-assign-overflow.cpp`,
  `dcl.constexpr/never-constant-overflow-cxx{20,23,26}.cpp`,
  `dcl.constexpr/never-constant-body-cxx{20,23}.cpp`,
  `dcl.constexpr/never-constant-lambda-member-cxx20.cpp`,
  `dcl.constexpr/never-constant-end-static-cxx23.cpp`,
  `dcl.constexpr/never-constant-after-call-cxx20.cpp`,
  `dcl.constexpr/never-constant-{call,temporaries,loop-break,switch,try-goto}-cxx20.cpp`,
  `dcl.constexpr/never-constant-infinite-loop{,-steps}-cxx20.cpp`,
  `dcl.constexpr/never-constant-walk-end-cxx23.cpp`,
  `dcl.constexpr/never-constant-local-values-cxx20.cpp`,
  `dcl.constexpr/never-constant-{local-tracking,discarded-parameter,switch-search}-cxx20.cpp`,
  `dcl.constexpr/never-constant-{pointer-locals,evaluation-order,long-loop,potential-result}-cxx20.cpp`,
  `dcl.constexpr/never-constant-pointer-null-{cxx20,clang19}.cpp`,
  `dcl.constexpr/never-constant-default-cxx23.cpp`,
  `dcl.constexpr/never-constant-no-error-cxx20.cpp`,
  `dcl.constexpr/potential-constant-builtin-{fabs,strcmp,not-constexpr}.cpp`,
  `dcl.constexpr/potential-constant-builtins-spread.cpp`,
  `dcl.constexpr/potential-constant-consteval-operator-template.cpp`,
  `dcl.constexpr/never-constant-direct-callee-cxx20.cpp`,
  `dcl.constexpr/never-constant-builtin-not-evaluated-cxx20.cpp`,
  `dcl.constexpr/never-constant-library-builtin-cxx20.cpp`,
  `dcl.constexpr/never-constant-builtin-abs-clang19.cpp`,
  `dcl.constexpr/potential-constant-builtin-abs-clang22.cpp`,
  `dcl.constexpr/never-constant-builtin-cxx23.cpp`,
  `dcl.constexpr/potential-constant-builtin-cxx23.cpp`,
  `dcl.constexpr/potential-constant-builtin-off-path-cxx20.cpp`,
  `dcl.constexpr/potential-constant-system-header-cxx20.cpp`,
  `dcl.constexpr/constexpr-asm-never-constant.cpp`.
- **Instantiations the check's evaluation requires (WP-1258).** A
  reflection query in the body (`is_complete_type(^^C<T>)`) may
  instantiate a specialization while the check evaluates it, and an
  error in that instantiation is real and reported. Its context names
  only the program's frames: the check's evaluation of the function is
  no call of the program's, so it adds no "in call to" note
  (`Oven::paused_call_notes` is empty for a potential-constant
  evaluation's mark, spec/sema-templates.md §10.2): the error prints as
  an instantiation outside any evaluation does.
  Test: `meta.reflection.queries/is-complete-type-never-constant-frames.cpp`.

### 5.29 C expression semantics (`expr/c_ops.rs`, WP-1285)
In C (`Sema::is_c()`) clang accepts several C constraint violations as
warnings and converts the operands as GCC does. kiln follows its C paths
(`CheckCStyleCast`, `CheckCompareOperands`, `CheckConditionalOperands`):
- **Pointer/integer casts** (C17 6.3.2.3p5-6, 6.5.4; `c_pointer_int_cast`).
  A cast between a pointer and an integer or enumeration of any width is
  valid. A pointer cast to a non-`_Bool` integer narrower than a pointer
  warns "cast to smaller integer type {1} from {0}" at the `(`:
  `-Wvoid-pointer-to-int-cast` from `void *`, `-Wpointer-to-enum-cast` /
  `-Wvoid-pointer-to-enum-cast` to an enumeration, else
  `-Wpointer-to-int-cast`. A non-`_Bool`, non-enumeration integer that is
  not an integer constant expression, cast to a wider pointer, warns
  "cast to {1} from smaller integer type {0}" (`-Wint-to-pointer-cast`,
  `-Wint-to-void-pointer-cast`). The casts are `IntegralToPointer`,
  `PointerToIntegral` and `PointerToBoolean`. A floating or complex type
  on either side of a pointer is an error at the operand (6.5.4p4).
- **`void *` and function-pointer arithmetic** (GNU). `+`, `-`, `++`,
  `--`, compound assignment and a pointer difference on a pointer to
  `void` or to a function take element size 1; they warn only under
  `-Wpointer-arith` (off by default). A `void *` may be subscripted (the
  element lvalue has type `void`; codegen steps by bytes). `sizeof`,
  `_Alignof` of `void` or of a function type (or a function designator)
  are 1, under the same group (`check_trait_operand_named`;
  `Types::sizeof_value` in the Oven). In C++ all of these stay errors.
- **Comparisons** (C17 6.5.8, 6.5.9; `c_pointer_comparison`). Two
  pointers to incompatible types (qualifiers aside) warn
  `-Wcompare-distinct-pointer-types` (an equality operator with a `void *`
  operand is valid, except that a function pointer against a non-null
  `void *` is a pedantic extension). Ordered function pointers warn
  `-Wordered-compare-function-pointers`. A pointer against a non-null
  integer warns `-Wpointer-integer-compare` (equality) or with no group
  (relational); an ordered pointer against a null pointer constant is
  pedantic. The integer converts to the pointer type
  (`NullToPointer`/`IntegralToPointer`); distinct pointers bit-cast the
  right operand to the left's type, or the left to the right's when only
  the left is a null pointer constant. The result is `int`.
- **Conditional operator** (C17 6.5.15p3-6; `c_conditional`). Arithmetic
  operands take the usual arithmetic conversions. A `void` operand
  converts both to `void` (only one `void`: a pedantic extension). Two
  structures of the same type give it unqualified. A null pointer
  constant takes the other pointer's type. Two pointers to compatible
  types give a pointer to their composite type (6.2.7p3: the bounded
  array, the prototyped function) carrying both qualifier sets; a pointer
  to `void` against an object pointer gives a pointer to `void` with both
  qualifier sets. Incompatible pointers warn "pointer type mismatch"
  (`-Wpointer-type-mismatch`) and give `void *`; a pointer and an integer
  warn "pointer/integer type mismatch in conditional expression"
  (`-Wconditional-type-mismatch`) and give the pointer type.
- **Incompatible pointer assignment** (C17 6.5.16.1p1;
  `conv/c_assign.rs`). It is a warning at `-fclang-compat=19` and an
  error by default (still `-Wincompatible-pointer-types`, so
  `-Wno-error=` demotes it) at 22, as each clang release.
- **Tags in type names** (C17 6.7.2.3). A `struct`, `union` or `enum`
  may be defined in a cast, `sizeof`, `_Alignof`, `_Generic` association,
  `__builtin_offsetof` or a function's result type; C has no base clause,
  so `_Generic(e, struct S: 1)` names `S`. A forward reference to an
  `enum` is a pedantic extension, not C++'s error. A tag defined in a
  parameter type is scoped to the prototype (6.2.1p4): a named tag or
  any `enum` warns "declaration of 'T' will not be visible outside of
  this function" (`-Wvisibility`, on by default), an unnamed `struct` or
  `union` is silent (`tests/c/decl/param-tag-visibility.c`).
- **`void` indirection** (C17 6.5.3.2p4, DR 106). `*p` of a pointer to
  (qualified) `void` is a `void` expression: an on-by-default
  `-Wvoid-ptr-dereference` extension warning, not C++'s error
  (`tests/c/expr/void-pointer-dereference.c`). The `void` lvalue is never
  loaded, `volatile void` included: as an arm of the (prvalue, WP-1318)
  `void` conditional it lowers to its pointer operand's value, as clang
  `VisitUnaryDeref`, so two such arms meet in a `phi ptr`; elsewhere it is
  discarded (`emit_arm`, WP-1347; `tests/c/run/void-deref-conditional-arms.c`,
  `tests/c/expr/void-deref-conditional-ir.c`).
- **Comma operator** (C17 6.5.17p2). The right operand takes the
  array/function-to-pointer and lvalue conversions (`conv::decay`), so
  the result is a prvalue: `sizeof (0, f)` is a pointer's size.
Tests: `tests/c/expr/`, `tests/c/conv/*-clang22.c`, `incompatible-pointer-error.c`.

## 6. Initialization (`init.rs`)
- **`perform_init(ty, init, direct_list)`** implements [dcl.init]:
  - an invalid element (WP-408): an expression standing for clang's
    `ExprError` (`Sema::is_expr_error`: an undeclared identifier or a
    qualified name not found, as opposed to a `RecoveryExpr` such as a
    failed call) among the parenthesized arguments or anywhere in a
    braced list leaves the initialization unperformed and undiagnosed
    (clang's `ActOnInitializerError`): `int w(S::nom, 2);` reports no
    "excess elements in scalar initializer". A braced list holding one
    is itself such an expression as a call argument or in `T{...}`
    (clang's `ParseBraceInitializer` fails). Test:
    `tests/conformance/dcl.init.general/invalid-element-initializer.cpp`.
  - a failed braced aggregate initialization (WP-420; clang's
    `InitListChecker::hadError`): once an element failed to initialize
    (`int a[] = {g(1, 2), 2};` with a `void g`: "cannot initialize an
    array element of type 'int' with an rvalue of type 'void'") or a list
    had excess clauses, nested lists included, no further "excess
    elements" error is reported for the same outermost list
    (`Sema::agg_error_mark`, the unrecoverable error count when it
    started, so a narrowing error does not count). The excess error is at
    the begin location of the first excess clause (`{1, 1 + 2}` at the
    `1`). The outermost list's initialization is then an `ExprError`
    (`is_expr_error`): its full-expression is not checked for implicit
    conversions and the variable has no constant initializer. Test:
    `tests/conformance/dcl.init.aggr/element-init-failure.cpp`.
  - copy- and direct-initialization of scalars, through the implicit
    conversion sequence, with narrowing enforced for list-init
    ([dcl.init.list]/7); a list element (`init_element`, an array or
    aggregate member) converted through a user-defined conversion
    checks its second standard conversion (`struct B { operator
    double(); }; int a[] = {B{}};` narrows, clang
    `checkNarrowing(SCS.After)`; WP-303); a parenthesized initializer
    left empty by a pack expansion (`T x(a...)`, [temp.variadic]/7) is
    `()` and value-initializes a non-class, non-reference `T` as an
    `ImplicitValueInitExpr` ([dcl.init.general]/16.4; WP-787,
    `tests/conformance/temp.variadic/empty-expansion-paren-init-value-initializes.cpp`)
  - references, binding directly or to a temporary
  - class objects by constructor ([dcl.init.general]/16.6.2.1; spec/sema-class.md
    §5): a direct-initialization from one expression of an unrelated class
    is overload resolution over the constructors (`construct_class`,
    `InitStyle::Direct`), whose copy or move constructor may bind its
    reference through an explicit conversion function, plus clang's
    CWG2327 conversion-function (and conversion-function-template)
    candidates (spec/sema-overload.md §2.6, §10.5 "User-defined conversions")
  - a class `C` initialized from an expression of type cv `C` or of a class
    derived from `C` (copy-, direct- or direct-list-initialization, a
    mem-initializer, a by-value argument, a function-style cast; any value
    category) is initialized by `C`'s constructors only
    ([dcl.init.general]/16.6.2.1, [over.match.ctor]/1): a conversion
    function of the source, template or not, explicit or not, converting
    to `C` or to a base of the source is never used ([class.conv.fct]/1,
    [over.ics.user]/4). It stays a candidate, but a non-viable one
    (clang `AddConversionCandidate`, `ovl_fail_trivial_conversion`),
    whatever its result type and before its result is looked at, in every
    candidate set that adds conversion functions for a class target
    (`add_conversion_candidate`, `overload/ics.rs`): it never ties with or
    beats the copy or move constructor (no "ambiguous" against the copy
    constructor, no deleted `operator C() &&` chosen for a prvalue, no
    access check of a conversion function as if it were a constructor),
    and it is noted as `candidate function` after the constructors when
    no constructor is viable (sorted as an arity mismatch: it has no
    parameter for the one argument, clang `EffectiveFailureKind`). A
    conversion function template is still deduced for the target (its
    declaration only, [temp.inst]/11), never defined; an unrelated class
    `E` is still initialized through `operator T()` with `T = E`. A
    copy-initialization from `C` or a derived class goes through `C`'s
    constructors like the direct form, so a constructor parameter may be
    initialized by a user-defined conversion of the source (`A(X)` for a
    `const B` with `B : A` and `B::operator X()`); only a copy-initialization
    from an unrelated class suppresses user-defined conversions there
    ([over.best.ics]/4, `resolve_ctor_ex`, `class/ctor_select.rs`). WP-609;
    tests `tests/conformance/over.match.ctor/same-class-conversion-*.cpp`,
    `same-class-copy-init-user-conversion-run.cpp`,
    `template-bodies-not-instantiated.cpp`,
    `unrelated-class-uses-conversion-template-run.cpp`,
    `class.conv.fct/self-conversion-not-viable-diags.cpp`.
  - character arrays from string literals: the bound is deduced, and
    an over-long string is an error
    (`initializer-string for char array is too long`, two arguments).
    The literal may be enclosed in braces ([dcl.init.string]/1,
    [dcl.init.list]/3.3): a braced list (top level, member or element)
    for a character array whose first element is a string literal
    initializes the array from it (`braced_string_init`), and a further
    element is `excess elements in char array initializer` (WP-668;
    tests `tests/conformance/dcl.init.string/braced-string-literal-*.cpp`)
  - String literal initializers: the literal's kind must suit the element
    type (`Sema::string_init_compat`, clang `IsStringInit`): an ordinary
    literal initializes `char`/`signed char`/`unsigned char`; with
    `char8_t` on (C++20, or `-fchar8_t`) a `u8` literal initializes
    `char8_t`, `char` and `unsigned char` (P2513), and with it off is an
    ordinary literal; `L`/`u`/`U` initialize only `wchar_t`/`char16_t`/
    `char32_t` (qualifiers ignored). A mismatch with a character element
    type is an error at the entity: `initializing 'char8_t' array with
    plain string literal` (with the note `add 'u8' prefix to form a
    'char8_t' string literal` at the literal), `initializing wide char
    array with non-wide string literal`, `... with incompatible wide
    string literal`, `initializing char array with wide string literal`,
    and for `signed char` from `u8` `initialization of char array with
    UTF-8 string literal is not permitted by {'-fchar8_t'|C++20}`. A
    mismatched literal in braces is not a string initialization: it
    initializes the first element (`cannot initialize an array element
    ...`), and brace elision treats it the same. WP-697 (WP-823); tests
    `dcl.init.string/literal-kind-mismatch{,-cxx17-fchar8t}.cpp`,
    `dcl.init.string/utf8-literal-fno-char8t.cpp`.
  - an array from an expression that is not a braced list or (for a
    character array) a string literal ([dcl.init.general]/16.5) is
    `array initializer must be an initializer list`, at the entity: the
    declarator name of a variable, the initializer of a default member
    initializer (WP-300). The suffix follows clang's element-type test:
    ` or string literal` for (signed/unsigned) `char`, ` or wide string
    literal` for unqualified `wchar_t`/`char16_t`/`char32_t`, none
    otherwise (`char8_t`, `const wchar_t`). With a dependent element
    type or a type-dependent initializer (`T a[N] = "abc";` in a
    template) neither rule applies until instantiation: the initializer
    is kept as written ([temp.dep.type]/9; WP-681,
    `dcl.init.string/string-literal-dependent-element-{run,errors}.cpp`)
  - aggregates, with brace elision, designators (C++20, in order) and
    unions (first member, or the designated one); an aggregate class
    whose list holds one undesignated element of the same or a derived
    class type is initialized from that element ([dcl.init.list]/3.2)
  - brace elision never enters an aggregate with no elements
    ([dcl.init.aggr]/2: no direct bases and no members but unnamed
    bit-fields, or an array of bound 0; `aggregate_has_no_elements`).
    [dcl.init.aggr]/16 takes no clause for such a subaggregate; clang and
    GCC reject the unbraced clause instead, and kiln follows them:
    `initializer for aggregate with no elements requires explicit braces`
    (`sema_implicit_empty_initializer`) at the clause, which is consumed
    (the next clause initializes the next element) while the element
    recovers. With `struct E {};`, `struct D2 { E e; int i; }; D2 d{1, 2};`
    has one error (at `1`), `struct F { E e[2]; int x; }; F f{1, 2};` has
    two (one per array element) and `D2 d{{}, 2};` none. The overload
    viability walk (`aggregate_list_walk`, spec/sema-overload.md) uses
    the same predicate. Test:
    tests/conformance/dcl.init.aggr/brace-elision-empty-aggregate.cpp
  - vector members (WP-173, clang `CheckSubElementType`): a GNU vector
    member cannot be initialized from a scalar, so a scalar starts a
    brace-elided list of up to N elements (`elided_vector_init`). A
    designator after its first item ends that list. A vector initializer
    copy-initializes the member, and an ext vector member splats a scalar
    instead. A vector item inside a vector's element list, braced or
    elided, is `cannot initialize a vector element of type T with an
    rvalue|lvalue of type V`. Deviation: clang 19 does not stop the list at
    a designator, so `{1, 2, .z = 5}` is an error there (a `void`
    "vector element"). The C99 mixture of designated and positional
    items is diagnosed on the written list in both cases (below).
  - a non-class `T` list-initialized from one class-type element (or
    `bool` from `nullptr`) is initialized from the element itself
    (`class_element_list_init`; clang's core-24034 rewrite in
    `TryListInitialization`): direct-initialization for a direct list
    (an explicit conversion function is a candidate), failures at the
    entity (variable name, `T` of `T{e}`, the `{` of an argument or
    return value; an argument adds its "passing argument" note), no
    `-Wbraced-scalar-init`, and narrowing checked on the user
    conversion's result
  - a scalar argument or return value from a one-element braced list
    (`f({1})`, `return {a == b};`) is `braces around scalar initializer`
    [-Wbraced-scalar-init] where written, never in an instantiation
    (clang 19 and 22 are silent there; `too many braces` still shows;
    WP-418, tests/conformance/dcl.init.list/braced-scalar-instantiation.cpp)
  - a non-aggregate class parameter copy-list-initialized from a braced
    argument (`f({1})`, a default argument, a member call) is constructed
    from the list; when the constructor chosen is explicit ("chosen
    constructor is explicit in copy-initialization" + "explicit
    constructor declared here") or deleted, the error ends with the
    "passing argument to parameter 'p' here" note (clang
    `PrintInitLocationNote`; `list_param_note`, taken by the outermost
    list constructor call only, so an element's own construction does
    not carry it; WP-366)
- **Designators** (WP-192; clang `SemaInit.cpp`). The parser reads
  `.name`, nested `.a.b`, `[i]` (C99), the GNU range `[a ... b]`
  (WP-193; `-Wgnu-designator`, an Extension shown under `-pedantic`, at
  the `...`) and mixtures of them. A leading
  `[` begins a designator unless it begins a lambda (clang
  `MayBeDesignationStart`): `[=`, `[...` and `[]` are lambdas, and
  `[&`, `[*` or `[identifier` scan to the matching `]` and are a
  designator when `=` follows.
  - Syntactic warnings come from `act_on_init_list_syntax`, once per
    written list at parse time, never on instantiation. All are
    `-Wc99-designator` (in `-Wc99-extensions`):
    - nested designators: at the first designator's `.`
    - array designators: at the `[`
    - the mixture of designated and positional items: at the designator
      begin, with a note on the first non-designated item, or, when
      item 0 is designated, at item 0 with the note on the positional
      item. A designated pack expansion `.m = e...` counts as
      non-designated here, as in clang.
  - Designation checks (WP-193; clang `ActOnDesignatedInitializer`) run
    in `act_on_designation` when the parser has read a designation,
    on each non-dependent index (`designator_index`), all at the
    index's begin:
    - not integral or unscoped enumeration type: `integral constant
      expression must have integral or unscoped enumeration type, not T`
    - not constant: `expression is not an integral constant expression`
      with the evaluator's notes, unless it folds (`fold_not_ice`)
    - negative: `array designator value '-1' is negative`
    - an empty range: `array designator range [3, 1] is empty`, at the
      `...`.
    A failed designation makes the parser drop the whole braced list,
    after reading it through its `}` (clang `ParseBraceInitializer`).
    Nothing more is diagnosed for that list (no syntactic warnings, no
    conversion of its clauses), and its declaration is invalid. The
    same checks run again on instantiated indices in `check_array`.
  - Semantic checks run in `check_aggregate_in` and `check_array`,
    again at every instantiation. A designator names a direct field or
    a field of an anonymous member (`designated_field`). A nested or
    anonymous designator descends into the subobject (`Sub { braced:
    false, entry, prior }`), and the positional items that follow keep
    filling it (`.p.y = 1, 7, 8` sets `p.z` and `w`). A subobject named
    again merges with its prior initialization, seeded from the prior
    `InitListExpr` (`prior_list`). Seeded slots count as explicit unless
    they are fillers (`ImplicitValueInitExpr`, `CXXDefaultInitExpr`).
    Descending into a subobject that an expression initialized keeps
    that object and re-initializes only part of it ([dcl.init.aggr]/4,
    C99 6.7.9p19): the slot becomes a `DesignatedInitUpdateExpr` whose
    base is the old initializer and whose updater is an `InitListExpr`
    of the subobject's type, with `NoInitExpr` in the slots it does not
    touch (an array updater runs to its highest index, with
    `NoInitExpr` holes and filler; a union updater holds only the
    designated member). Lists nested in an updater fill with
    `NoInitExpr` too (`Sub::update`, clang `FillWithNoInit`), except a
    braced value, which value-initializes, and a merged prior list,
    which keeps its own fill (`is_update_list`). A designator path that
    ends at an aggregate whose value is an expression (`.m = 1` for
    `int m[2]`) starts a brace-elided list whose first clause is that
    value with its designator consumed (`Sub::direct`, clang
    `CheckSubElementType` with `DirectlyDesignated`); the positional
    clauses that follow continue it, and it warns `brace elision for
    designated initializer is a C99 extension` (`-Wc99-designator`) at
    the value. The next designator
    warns `initializer partially overrides prior initialization of
    this subobject` with the old initializer noted; keeping the old
    object of a class with a non-trivial destructor is an error
    (`initializer would partially override prior initialization of
    object of type T with non-trivial destruction`). A full override
    of an initializer that may have side effects notes that they will
    not occur. Codegen emits the base and then the updater; the Oven
    does not evaluate it, as clang's evaluator does not (`constexpr
    variable must be initialized by a constant expression`).
    - out of declaration order: `-Wreorder-init-list` (ExtWarn, in
      `-Wreorder`), at the later value, with the note
      `previous initialization for field` on the earlier one. It covers
      only the list's own designators.
    - a re-initialized subobject: `-Winitializer-overrides` at the new
      value, with the note `previous initialization is here`. A union
      initialized through a different member is the DefaultError form,
      at the designator.
    - Order and override diagnostics are substitution failures
      whatever their severity (clang's `SFINAEFailure` ExtWarns;
      `is_sfinae_failure`), so a constraint or a SFINAE'd signature
      using them fails. The `-Wc99-designator` warnings are not.
    - In overload resolution ([over.ics.list]/8), a list that
      initializes a subobject twice converts to no aggregate
      (`designators_override`, clang `InOverloadResolution`): the same
      path, a whole object after a part of it, a part after the whole,
      another member of the same union, through anonymous members.
      Order does not matter there, and a braced value's own designators
      are not compared.
    - an unknown field: `field designator 'q' does not refer to any field
      in type T`, at the name.
    - a field designator into a scalar: `field designator cannot
      initialize a non-class type`, at the `.`.
    - an array designator into a class: `array designator cannot
      initialize non-array type`, at the `[`.
    - an index (or range end) at or past a known bound: `array
      designator index (5) exceeds array bounds (3)`, at that index.
    - a range `[a ... b]` initializes each element from the same
      clause, re-checked per element; positional items continue at
      `b + 1`.
    - an array designator leaving a hole gives the list an
      `array_filler`, even when the element count equals the bound.
    - non-aggregate types are `designated_non_aggregate`.
    - a union initialized by `{}` initializes the variant member with a
      default member initializer, directly or within an anonymous
      member (`union_default_member`, [dcl.init.aggr]/5; clang
      `CheckStructUnionTypes`), and otherwise value-initializes its
      first member.
  - In templates (spec/ast.md §5.6, spec/sema-templates.md §5.4), the
    designators are carried in `DesignatedInitExpr`s and in the written
    form of non-dependent lists. The semantic checks run on the
    instantiated type at the written locations. A designated pack
    expansion gives one designated clause per element. A non-dependent
    list that the instantiation shares rather than rebuilds keeps the
    order and override diagnostics its definition check issued
    (`designator_rewarn`), and each instantiation issues them again
    under its instantiation note (`reissue_designator_warnings`), as
    clang's rebuilt list does; a rebuilt list warns again by itself.
    Neither is deduplicated against the definition's.
  - Deviations:
    - A range designator whose clause has side effects evaluates the
      clause once per element. clang's codegen refuses it ("cannot
      compile this GNU array range designator extension yet"), and GCC
      evaluates it once.
- **`init_from_list`** builds clang's semantic `InitListExpr`, with
  `array_filler`.
  - In a templated entity (WP-613), a declaration's braced initializer
    is analyzed at definition and initialized again at each
    instantiation from the list as written, never from its semantic
    form: `Substituter::init_list` drops the trailing fillers the
    definition's check appended for omitted members
    (`ImplicitValueInitExpr`, `CXXDefaultInitExpr`), so the omitted
    members are initialized anew ([dcl.init.aggr]/5). Fed back as a
    written clause, the `ImplicitValueInitExpr` of an omitted array
    member (`S addr{};` with `char path[108]`) would start brace elision
    and initialize `path[0]` from an expression of type `char[108]`.
    A written `{}` is an `InitListExpr`, never a filler, and a filler
    before a written clause only follows a designator, whose list is
    rebuilt from its written form (spec/sema-templates.md, "Braced
    lists"). An element without a location of its own that fails to
    initialize is diagnosed at its list's `{` (`init_element_at`).
    Tests: `dcl.init.aggr/array-member-filler-in-template{,-cxx17,-cxx20}.cpp`,
    `dcl.init.aggr/array-element-from-array.cpp`.
  - Deviation shared with clang 19 and 22: a parenthesized aggregate
    initialization (P0960) in a template that omits an array member
    (`S a(1);`) is rejected at instantiation ("array initializer must
    be an initializer list or string literal", with no location); GCC
    16.1 accepts it.
- **`std::initializer_list<E>` from a list** (`std_initializer_list_init`,
  [dcl.init.list]/3.6, /5): a `CXXStdInitializerListExpr` over the
  materialized backing array `const E[N]`. An empty list takes this path
  only when the specialization has no default constructor (CWG1070; with
  one, /3.5 value-initializes it first, as clang does): the backing array
  is `const E[0]` and `size()` is 0 in a run and in a constant expression.
  Tests: `tests/conformance/dcl.init.list/empty-initializer-list-no-default-ctor{,-run}.cpp`.
- **References** (`reference_list_init`, [dcl.init.list]/3.10): when
  the single element is not reference-related, a temporary of the
  referenced type is list-initialized with the reference's own kind
  (direct for `T& r{..}`, copy for `T& r = {..}`, a call argument or
  a `return`). A failed constructor call for that temporary names the
  reference type (`no matching constructor for initialization of
  'const E &'`, and `call to deleted constructor of 'const J &'`), via
  `ctor_diag_ty`, which `use_ctor` takes on entry so nested
  constructions cannot see it. While an aggregate (class or array)
  temporary is list-initialized, the first `excess elements` error in
  it takes clang's `in initialization of temporary of type 'const H'
  created to list-initialize this reference` note at the reference
  (`ref_temp_note`, clang's `diagnoseListInit`); a scalar temporary
  (`const int& r = {1, 2};`) gets no note. Too many elements for a
  scalar report at the initialization's location: the declarator, the
  `{` of a scalar member's nested list (`H h = {1, {2, 3}};`), and the
  `{` of a default member initializer
  (`tests/conformance/dcl.init.list/{reference-temporary-errors,scalar-excess-locations}.cpp`).
- **Results** are attached with `act_on_init`. `act_on_uninit` handles
  default-initialization: const objects without an initializer are an
  error, and incomplete types are diagnosed.
- **Uninitialized definitions before the validity check**
  (`uninit_array_or_reference`, clang `ActOnUninitializedDecl`, WP-308).
  A non-`extern` variable definition without an initializer whose type
  is an array of unknown bound is `definition of variable with array
  type needs an explicit size or an initializer` (a `constexpr` one is
  `constexpr variable 'c' must be initialized by a constant
  expression`), even when the element type is dependent or the
  declaration is already invalid, and the variable becomes invalid. An
  invalid variable whose type is a non-dependent reference (`S::c &r;`
  with no `S::c`, its type recovered to `int &`, spec/parse.md §2.1)
  still reports `declaration of reference variable 'r' requires an
  initializer`; no other check runs for an invalid variable. A
  dependent reference type (`T &r;` in a template) is reported at the
  template definition, with no note, as clang checks a reference type
  before it stops at a dependent one, and again with the instantiation
  note for each instantiation (`per_instantiation`, WP-318); a dependent
  type that is not syntactically a reference waits for instantiation
  (`tests/conformance/dcl.ref/dependent-uninitialized-reference.cpp`).
- **Initializer of an invalid variable** (clang `AddInitializerToDecl`,
  WP-308): `act_on_init` discards it unchecked, so `S::c &r = 1;`
  reports only the decl-specifier
  (`tests/conformance/dcl.type.simple/unknown-type-recovers-to-int.cpp`).
- **Constexpr variables:** `check_constexpr_init` evaluates the
  initializer of a `constexpr` variable, and of a const integral
  variable, which is usable in constant expressions. The value is
  stored (§8.3).
- **Self-reference** (`self_ref.rs`, WP-171; clang `CheckSelfReference`):
  after the initialization is performed, `act_on_init` walks the
  converted initializer of a variable without automatic storage
  duration, or of class or reference type, for uses of the variable
  itself ([basic.life]/7). A use warns
  `variable {0} is uninitialized when used within its own
  initialization` (namespace scope, or class type),
  `reference {0} is not yet bound to a value ...` (references, any use)
  or `static variable {0} is suspiciously used ...` (static locals,
  `-Wstatic-self-init`, inside `-Wuninitialized`), through
  `diag_runtime_behavior`. As in clang: a value use is an
  lvalue-to-rvalue conversion, increment/decrement, compound-assignment
  operand, overloaded-operator argument, copy-constructor argument,
  `std::move` argument, or a non-static member function call on the
  variable; `T a = a;` with non-class `T` and copy-initialization is
  the silencing idiom; a member read in an aggregate initializer list
  is fine once that member is initialized; unevaluated operands,
  `constexpr` variables, static data members of non-class type and
  variables in a linkage specification (clang checks the declaration
  context) are not diagnosed. Other locals are left to the CFG
  uninitialized-values analysis (§7.14).
- **Verify-only initialization** (WP-328; clang's `VerifyOnly` in
  `TryOrBuildParenListInitialization`): an attempt that only asks
  "would this initialization succeed" must leave no trace when it
  fails. `verify_paren_aggregate_init(ty, args, at, rparen)` runs
  `paren_aggregate_init` inside `enter_unevaluated`/`exit_unevaluated`
  (nothing is odr-used, so no function template specialization is
  instantiated and no implicit member is defined; the cleanups it
  pushes are popped), under `Sema::capturing` (its diagnostics,
  deferred ones included, are dropped), and restores `braced_args`
  (the braced-argument placeholders it consumed). It returns whether
  the result is valid; the throwaway expression is never used. On
  success the caller builds the initialization again for real, so its
  warnings and instantiations happen once ([temp.inst]/5: a
  specialization is instantiated only when needed). Used by the
  single-operand cast to an aggregate class with no viable
  constructor (`class/construct.rs` `construct_class`,
  spec/sema-class.md §5.6). Tests:
  `tests/conformance/dcl.init.general/paren-aggregate-cast-verify-only.cpp`
  (a converting constructor template whose `static_assert` would fire
  if the failed attempt instantiated it),
  `tests/conformance/dcl.init.general/paren-aggregate-cast-run.cpp`.

### 6.1 Braced lists in expressions and explicit type conversions (`forms.rs`)
- **Call arguments** `f({...})` ([expr.call]/7):
  - `act_on_braced_arg` returns a `void` `InitListExpr` placeholder,
    kept in `braced_args`.
  - `act_on_call` replaces it by the list-initialization of the
    parameter.
  - Viability follows [over.ics.list]: a scalar takes at most one
    unbraced, convertible element. For an aggregate, the list is walked
    under brace elision and each element's conversion is checked; a
    designated list instead takes at most as many elements as brace
    elision reaches (spec/sema-overload.md [over.ics.list]). A non-viable list is clang's
    `cannot convert initializer list argument to 'T'` candidate note,
    except that a single class-type element for a non-class parameter
    notes the element's own conversion ([over.ics.list]/10: `no known
    conversion from 'N' to 'int' for 1st argument`).
  - A list for the ellipsis of a variadic function is an error (the
    callee named as in §5.19).
- **`return {...};`** ([stmt.return]/2, `act_on_return_list`):
  copy-list-initialization of the returned object.
- **`x = {...}`** ([expr.assign]/8, `act_on_assign_list`): the list
  initializes a temporary of the left operand's type, which is then
  assigned. A type-dependent left operand or list element makes it a
  dependent `BinaryOperator` with the written list as its right operand,
  assigned again from the substituted list at instantiation.
- **Other operators:** a braced list as their operand is ill-formed
  (`act_on_misplaced_init_list`).
- **Functional casts** ([expr.type.conv]):
  - `T()` (`act_on_value_init_cast`) value-initializes; `void()` is
    a `void` prvalue.
  - `T{...}` (`act_on_list_cast`) is direct-list-initialization.
    For an enumeration with a fixed underlying type that includes
    [dcl.init.list]/3.8 (`E{v}`: `v` converted to the underlying type,
    narrowing rejected, then to `E`), as `E e{v};` does.
    A class `T{y}` whose initialization elides to `y`'s conversion
    function (an `ImplicitCastExpr` of `UserDefinedConversion`, not a
    construct expression) is wrapped, as clang does, in a `NoOp`
    `CXXFunctionalCastExpr` to `T`, so substitution of a pattern holding
    it (`T2{TP{y}}`) converts to the written `T` again rather than
    losing it with the stripped implicit cast (WP-792). Test:
    `expr.type.conv/list-cast-conversion-function-in-template.cpp`.
  - `T(a, b)` (`act_on_paren_list_cast`) is ill-formed for a scalar.
    The expression-list may hold braced-init-lists (`T(a, {b})`, `T({})`).
    They are the placeholders of `act_on_braced_arg`, and the
    construction list-initializes its parameter from them. A dependent
    operand keeps the list as written (`dependent_init_list`), and
    substitution makes it a placeholder again. A lone parenthesized list
    for a non-class `T` is `sema_list_init_in_parens` (clang's
    FK_ParenthesizedListInitForScalar).
  - Class, array and reference `T` need temporaries and are
    unsupported (`temporary_unsupported`) until classes have
    constructors (WP-065).
  - A function type `T` in `T()`, `T{...}` or `T(a, b)` is `cannot
    create object of function type 'T'` at the type, ranging over the
    whole expression (clang `BuildCXXTypeConstructExpr`
    `err_init_for_function_type`, after the single-operand cast — `T(a)`
    is a functional cast — and before completeness and abstractness;
    `function_type_construct`, WP-465). Test:
    `expr.type.conv/function-type-construct.cpp`.
  - Array `T{...}` (`array_list_cast`, WP-216; `using A = int[2]; A{1,
    2}`) list-initializes an array prvalue that keeps the cv-qualifiers
    of `T` ([expr.type]/2). An array of class type with a non-trivial
    destructor is bound (`CXXBindTemporaryExpr` of the array type,
    `maybe_bind_temporary` descends to the element type), and so is a
    braced list bound to a reference to array (`reference_list_init`).
  - Reference `R{...}` (`reference_list_cast`, WP-690; `using CR = const
    double &; CR{}`) direct-list-initializes the reference
    (`reference_list_init`, [dcl.init.list]/3.9-10) and is an lvalue (an
    xvalue for an rvalue reference to an object) `CXXFunctionalCastExpr`
    of the referenced type, as clang. Tests:
    `expr.type.conv/reference-list-cast.cpp`,
    `reference-list-cast-errors.cpp`.

## 7. Statements (`stmt.rs`)
- **Compound statements and scopes:** see §2.
- **Discarded volatile glvalues** ([expr.context]/2, WP-416; clang
  `IgnoredValueConversions` / `isReadIfDiscardedInCPlusPlus11`). From
  C++11, `ignored_value_conversions(e)` applies the lvalue-to-rvalue
  conversion (`conv::lvalue_to_rvalue`, so not to a class or array) to a
  discarded-value expression that is a non-dependent glvalue of
  volatile-qualified type and, ignoring parentheses, an id-expression, a
  subscript, a member access, an indirection, a `.*` / `->*`, a comma
  whose right operand qualifies, or a conditional (also `a ?: b`) both of
  whose arms qualify. It runs on an expression statement and a `for`
  increment (before `act_on_full_expr`), the left operand of a built-in
  comma, and the operand of a cast to `void`. A call returning a volatile
  reference is not read. Such a `.*` / `->*` is not `expression result
  unused` (clang sees the cast).
- **`if`, `while`, `for`:** condition conversion to `bool` (contextual
  conversion), plus `cond_var` for condition declarations
  (`act_on_condition_var`). A condition variable of function type
  (`decltype(auto) x = f`) is "a function type is not allowed here" at
  its declarator after the failed initialization ([stmt.pre]/5, clang
  `CheckConditionVariable`; WP-408, test
  `stmt.pre/condition-decltype-auto-function.cpp`). The condition
  resolves a lone overload candidate before converting
  (`resolve_single_overload_candidate`).
- **`act_on_if(kind, init, cond_var, cond, then, els, loc)`** builds
  every `IfKind`:
  - **`if constexpr`** ([stmt.if]/2): `act_on_constexpr_condition(e)`
    converts the condition contextually to `bool` and evaluates it with
    `EvalKind::IfConstexpr`; the result is `ConstantExpr(cond, value)`.
    A failure is `constexpr if condition is not a constant expression`
    at the condition's first token, with the Oven's notes. In M2 both
    branches are analyzed and kept; the discarded statement
    ([stmt.if]/2: not instantiated in a template, return statements not
    used for deduction) is M3, with templates. Codegen and the Oven
    emit only the selected branch.
  - **`if consteval` / `if !consteval`** ([stmt.if]/4): no condition;
    both substatements are compound statements (the parser enforces it:
    `expected { after consteval` / `expected { after else`). The
    compound executed when constant-evaluated (the `then` branch of
    `if consteval`, the `else` branch of `if !consteval`) is an
    immediate function context ([expr.const]/16):
    `enter_immediate_context`/`exit_immediate_context` bracket it and
    `in_immediate_context()` reports it, and its immediate invocations
    are neither wrapped nor checked (§5.10). In a template the branches
    are substituted the same way at instantiation.
- **`switch`:**
  - integral promotion of the condition
  - a condition of class type ([stmt.switch]/2, [conv.general]/5;
    `switch_conv.rs`, clang `PerformContextualImplicitConversion`
    with the C++14 rules, WP-364) is contextually implicitly converted
    before promotion. The same helper (`contextual_class_conversion`,
    parameterized by `ContextualConv`) converts a `delete` operand and a
    pre-C++14 array-new bound with their own wording (§ new/delete,
    WP-374). All diagnostics are at the `switch` keyword:
    1. An incomplete class (after instantiating a specialization) is
       `switch condition has incomplete class type 'T'` with the
       `forward declaration` note.
    2. The visible conversion functions (`conversion_functions_all`)
       whose non-reference result type is integral or enumeration
       (scoped included) are collected; a conversion function template
       is kept but contributes no target type. Explicit ones are set
       aside.
    3. The non-explicit, non-template ones must agree on one
       cv-unqualified target `T`; otherwise `multiple conversions from
       switch condition type 'C' to an integral or enumeration type`
       with one `conversion to {integral|enumeration} type 'U'` note per
       conversion function (at its name). A conversion function
       template among them is noted `candidate function template`.
    4. Overload resolution ([over.match.conv]) among the non-explicit
       matching ones (templates deduced against `T`) picks the function
       called (`call_conversion_function`), then a standard conversion
       to `T` (so a `const int&` result is read). An ambiguity is the
       same `multiple conversions` error; a conversion function template
       is noted by its deduced specialization, `candidate function [with
       T = int]` (`candidate function template` when deduction failed).
       clang 19-22 crash on both template cases
       (`diagnoseAmbiguousConversion` casts the template to a conversion
       function), so this wording is kiln's (WP-374;
       `stmt.switch/ambiguous-conversion-template.cpp`, `skip(clang)`).
    5. With none viable (none, deleted, only templates, a non-viable
       object qualifier) and exactly one explicit matching non-template
       conversion function: `switch condition type 'C' requires
       explicit conversion to 'U'` plus the note, and that function is
       called (recovery). Otherwise the generic `statement requires
       expression of integer type ('C' invalid)`.
    The converted condition then gets `-Wswitch-bool` and promotion
    like any other, and `-Wswitch` sees an enumeration result. Codegen
    and the Oven evaluate it once. Tests:
    `stmt.switch/class-condition-{diags,run}.cpp`.
  - duplicate `case` values (with a note), each case located at its
    value's begin (clang `getBeginLoc`). A case value outside `i128` (an
    `unsigned __int128` above its maximum, a `_BitInt(N)` with N > 128;
    WP-465) keeps its exact constant in the `ConstantExpr`, has no `i128`
    key, and is compared with the other such values by its words
    (`wide_duplicate_case`, printed in decimal); codegen emits it from the
    constant and the Oven compares it limb by limb (spec/oven.md
    "`_BitInt`"). Tests: `stmt.switch/bit-int-wide-{duplicate-case,condition}.cpp`.
  - multiple `default` labels
  - `case` values converted to the promoted type as a converted
    constant expression ([expr.const]/12, clang
    `BuildConvertedConstantExpression`; WP-465): the implicit conversion
    sequence is formed first (`case_cce_check`); none is `value of type
    'X' is not implicitly convertible to 'T'`, and a standard (or a
    user-defined sequence's second standard) conversion whose second step
    is not identity, an integral promotion or conversion, a boolean
    conversion from an integral or unscoped enumeration type, or a
    pointer conversion from `nullptr_t` is `conversion from 'X' to 'T' is
    not allowed in a converted constant expression` (`case 1.5:`, a class
    with `operator double`, a string literal); both at the value with its
    range, and the case is dropped. An integral value that does not fit is
    `case value evaluates to V, which cannot be narrowed to type 'T'`,
    after the full-expression's conversion warnings, as are an NTTP's and
    a fixed-type enumerator's. Test:
    `stmt.switch/case-converted-constant.cpp`.
  - `act_on_start_switch(loc, …)` and
    `act_on_finish_switch(…, cond_var)` bracket the body
  - GNU case ranges `case lo ... hi` (WP-205): the parser emits the
    `-Wgnu-case-range` extension at the `...`; `act_on_case_range`
    converts both bounds like a case value and `act_on_case(value,
    Some(hi), …)` builds the `CaseStmt`. At the end of the switch
    (`check_case_ranges`, clang `ActOnFinishSwitchStmt`) the ranges are
    sorted by `lo`; an empty one (`lo > hi`) is `empty case range
    specified` and dropped; one overlapping a case value reports the
    largest such value, and one overlapping the previous range reports
    that range's `hi`, both as `duplicate case value` at the range with
    a `previous case defined here` note. Template instantiation goes
    through the same entry points.
  - `-Wswitch-bool` (`check_switch_bool`, before promotion): a
    condition clang's `isKnownToHaveBooleanValue` accepts (a `bool`,
    `!`, a comparison, `&&`/`||`, a bitwise or `?:` of such, an
    assignment/comma whose result is such) warns `switch condition has
    boolean value` at the `switch` keyword.
  - Enumeration and constant checks (`switch_enum.rs`
    `check_switch_cases`, WP-219), in clang `ActOnFinishSwitchStmt`
    order, after `check_case_ranges`; skipped for a dependent or invalid
    condition and for a value-dependent case label (clang
    `HasDependentValue`: -Wswitch-default included, so a template whose
    labels depend on a parameter warns only per instantiation, WP-230):
    1. no `default`: `-Wswitch-default` (off by default) `'switch'
       missing 'default' label`;
    2. nothing more if the case list is broken (an unfolded label, a
       duplicate value, an overlapping range);
    3. no `default` and a constant condition: `no case matching constant
       switch condition 'N'` (no group) when no case or range holds it;
       nothing more either way;
    4. only a complete enumeration with enumerators (the condition
       before promotion) continues;
    5. a case value, then each range's `lo` and `hi`, that is no
       enumerator's value: `case value not in enumerated type 'E'`
       [-Wswitch]; a label naming a const variable of static or
       thread storage duration of the same enumeration is exempt, and so
       is every value of an `enum_extensibility(open)` enumeration; for a
       `flag_enum` one "an enumerator's value" means any value whose bits
       are all bits of its positive single-bit enumerators (clang
       `ShouldDiagnoseSwitchCaseNotInEnum`, `IsValueInFlagEnum`);
    6. enumerators sorted by value (first of equal values kept),
       deprecated, `[[maybe_unused]]` and unused ones skipped, not
       covered by a case or range: none and a `default` is `default
       label in switch which covers all enumeration values`
       [-Wcovered-switch-default, off] for a closed non-flag enumeration
       (clang `isClosedNonFlag`); otherwise `enumeration value 'A'
       not handled in switch` (two: `values 'A' and 'B'`, three: `'A',
       'B', and 'C'`, more: `N enumeration values not handled in switch:
       'A', 'B', 'C'...`) [-Wswitch] without a `default`, or the same with
       `not explicitly handled` [-Wswitch-enum, off] with one, at the
       condition's expression location.
    Like clang these repeat in every instantiation (emitted with
    `allow_duplicate`).
- **Jumps:**
  - `break` and `continue` outside a loop or switch are errors. Which
    loop is open at a jump in a GNU statement expression in a loop or
    switch header is §7.4a.
  - `return`:
    - a value in a `void` function is an error
    - a missing value in a non-`void` function is an error
    - the value is converted as if by copy-initialization
- **Falling off the end:** see §4.3.

### 7.4 Labels and goto
- Labels are `LabelDecl`s in the function, with payload
  `Payload::None(0)`.
- `goto` to an undeclared label is reported at the end of the function
  (`use of undeclared label 'x'`).
- A redefinition is an error, with a note.
- **Landed (WP-153): jump-scope checking** (`jump_scope.rs`, clang
  `JumpScopeChecker`), run at the end of every function body with a
  label, `switch` or `asm goto` (in templates, at instantiation).
  Protected scopes: a variable whose initialization a jump would bypass
  ([stmt.dcl]/3: non-trivial default init, any initializer, a VLA or a
  non-trivial destructor), try blocks and handlers ([except.handle]/2),
  each arm of a constexpr/consteval `if` ([stmt.if]/2), GNU statement
  expressions and lifetime-extended temporaries with a destructor. A
  `goto` or `case` entering one is `cannot jump from this goto statement
  to its label` / `cannot jump from switch statement to this case label`
  with one `jump bypasses …` note per scope entered; an `asm goto` may
  neither enter nor leave a scope with a destructor (`cannot jump from
  this asm goto statement to one of its possible targets`, the
  `possible target of asm goto statement` note and `jump exits …`
  notes). Leaving scopes through `goto` is fine; codegen runs their
  cleanups (spec/codegen.md).
- **Landed (WP-444): GNU computed goto.** `act_on_indirect_goto`
  ([stmt.goto], GNU): an operand with an unexpanded pack is diagnosed
  and the statement dropped; otherwise, unless type-dependent, it is
  converted to `const void *` as by assignment (clang
  `CheckSingleAssignmentConstraints`; a non-pointer operand gets that
  conversion's error), made a full-expression, and the function is
  marked as holding an indirect goto, which runs the jump-scope checker.
  `VerifyIndirectJumps` (`verify_indirect_jumps`): with no `&&l` in the
  body the first indirect goto is "indirect goto in function with no
  address-of-label expressions"; otherwise each scope holding an
  indirect goto must reach each scope holding an address-taken label by
  leaving only scopes without an "exits" note and then entering only
  scopes without a "bypasses" note, else "cannot jump from this
  indirect goto statement to one of its possible targets" at the
  `goto`, "possible target of indirect goto statement" at the label and
  one note per scope (one jump and one label stand for each scope, in
  clang's `DenseMap` order, `dense_map_order`). Unlike a direct `goto`,
  leaving a scope with a destructor is an error (an indirect goto runs no
  cleanups). `&&l` in a lambda or local class names a label of that
  body, so a label of the enclosing function is "use of undeclared
  label" there. A `&&l` in a template's body is instantiation-dependent
  (spec/sema-templates.md). Tests:
  `tests/conformance/stmt.goto/computed-goto-{jump-scope,lambda,dispatch-run,reachability}.cpp`.
- **Landed (WP-455): the walk enters lambda bodies.** As clang's
  `JumpScopeChecker`, whose walk reaches a lambda's body through
  `LambdaExpr::children`, the jump-scope check of a function also walks
  the body of every lambda in it (nested ones included), as a scope
  nested where the lambda expression appears: the lambda's labels,
  `&&l` operands, `goto`s, indirect gotos and `asm goto`s join the
  enclosing function's check. So an indirect goto of the function sees
  the labels whose address a lambda takes ("cannot jump from this
  indirect goto statement to one of its possible targets", with the
  notes for the scopes of the lambda's declarations and of the variable
  it initializes), and one in a lambda sees the function's. A lambda's
  own body is still checked when it ends, so an error within the lambda
  is reported twice by clang and once by kiln (identical diagnostics are
  merged, spec/diag.md §7.4.1; the test uses `dup(clang)`). The bodies
  of default arguments and of local classes' member functions are not
  walked (clang does not either: an indirect goto there with no `&&l`
  of its own is "indirect goto in function with no address-of-label
  expressions"). Test
  `tests/conformance/stmt.goto/computed-goto-jump-scope-lambda.cpp`.
- `[[noreturn]]`: the fall-off analysis (§4.3) treats a call as not
  returning when any redeclaration of the callee is `[[noreturn]]`,
  including member calls.

### 7.4a Jumps in loop and switch headers (WP-1075)
A GNU statement expression can put a `break` or `continue` in a loop's
or switch's header ([stmt.break]/1, [stmt.cont]/1 name only the
innermost enclosing loop). Kiln follows clang (19 and 22 agree, C++):
- **Where a loop is open.** Sema's breakable stack (`FnState::breakables`,
  clang's `BreakScope | ContinueScope`) gets the loop pushed:
  - `while`: from its condition, including a condition variable, to the
    end of the statement;
  - `do`: from its body to the end of its condition;
  - `for`: after its init-statement, from its condition through its
    increment and body;
  - range-based `for`: from its body only, so its init-statement and
    range belong to the enclosing statement;
  - `switch`: from its body only (a `break` scope), so a jump in its
    init-statement or condition targets the enclosing loop or switch.

  The parser (`parse_while`, `parse_do`, `parse_for_rest`) and template
  substitution (`templ/subst/stmts.rs`) push at those points. A jump with
  no open target is "'break' statement not in loop or switch statement" /
  "'continue' statement not in loop statement" at the keyword, as before.
  A lambda or local class body starts a new function, so a jump there
  never reaches a loop of the enclosing function.
- **Condition variable of a `for`.** A `continue` whose target is the
  `for` loop whose condition variable is being initialized would skip
  that initialization on its way to the increment, so it is clang's
  "cannot jump from this continue statement to the loop increment; jump
  bypasses initialization of loop condition variable"
  (`sema_continue_from_cond_var_init`, `err_continue_from_cond_var_init`)
  at the `continue`, whether or not the loop has an increment
  (`begin_for_cond_var`/`end_for_cond_var` mark the declaration). A
  `while` loop's condition variable is valid: its `continue` re-evaluates
  the condition. Template substitution does not recheck it; the
  definition already did.
- **Codegen** (`kiln_codegen/src/stmt.rs`) mirrors clang's
  `BreakContinueStack`, with the same windows: `while` pushes its targets
  before the condition (`continue` re-evaluates the condition), `do`
  before the body and pops after the condition (`continue` goes to
  `do.cond`), `for` after the init (`continue` goes to the increment, or
  to the condition without one; the increment's own `continue` re-runs
  the increment), range-based `for` and `switch` after their headers.
- **CFG** (§7.5.1): clang's analysis graph differs from its codegen here,
  and kiln's warnings follow the graph. A `while` or `do` condition's
  `break` and `continue`, and the `continue` of a `for` header, go to the
  enclosing loop; the `break` of a `for` header and of a range-based
  `for`'s init or range goes to that loop's exit (a header frame pushed
  before the init); a `switch`'s init, condition variable and condition
  `break` go to the switch's exit. A jump that has no target in the graph
  is clang's `badCFG`: the function gets `Status::NoCfg`, and no
  CFG-based warning (-Wreturn-type, -Wuninitialized, unreachable code) is
  reported for it.
- **Constant evaluation.** clang rejects any jump that leaves a statement
  expression during constant evaluation ("this use of statement
  expressions is not supported in a constant expression", at the
  statement of the statement expression that holds the jump). The Oven
  checks this before its own "outside a loop" trap, so a header jump
  gets clang's note. A loop wholly inside a statement expression
  evaluates normally.
- **Oracle disagreement.** GCC rejects every `break`/`continue` in a
  loop or switch header ("break statement not within loop or switch");
  kiln follows clang.
- **Newer clang.** clang's main branch (the P1306 and labelled-loop
  work) opens every loop only for its body (`Scope::EnterLoopBody`;
  `LeaveLoopBody` before a `do`'s condition), so a jump in a `while`,
  `do` or `for` header targets the enclosing statement there. Kiln
  follows the released clangs; revisit when the oracle moves past 22.
- **Expansion statements' headers** (`template for`): §7.6a.
- **Not covered:** clang's dropping of the erroneous jump (a `StmtError`), which lets it
  still report "no case matching constant switch condition" after
  `switch (({ continue; 1; }))` with no loop; kiln keeps the statement and
  misses that warning.

Tests: `tests/conformance/stmt.break/stmt-expr-loop-header-{run,cfg}.cpp`,
`tests/conformance/stmt.cont/stmt-expr-loop-header-errors.cpp`,
`tests/conformance/stmt.while/ir-stmt-expr-loop-header-jump.cpp`,
`tests/conformance/ext.gnu.stmt.expr/constexpr-loop-header-jump.cpp`.

### 7.5 Landed (WP-100): try-blocks (`try_stmt.rs`)
- `act_on_exception_decl` ([except.handle]/1-3): an rvalue-reference
  type is `cannot catch exceptions by rvalue reference`; (a pointer or
  reference to) an incomplete type other than cv `void` is
  `cannot catch [pointer|reference] to incomplete type T`; an abstract
  class is the usual abstract-variable error; array and function types
  decay. A class type by value is copy-initialized from an
  `OpaqueValueExpr` (clang's `ExceptionDecl` init) and its destructor is
  checked (access, deleted).
- With `-fno-exceptions`, `try` is `cannot use 'try' with exceptions
  disabled` (the statement is still analyzed), except in a system header
  or a template definition (§5.5 "Exceptions disabled").
- `act_on_try`: a `catch (...)` that is not last is `catch-all handler
  must come last`; a handler whose type is the same as, or a public
  unambiguous derived class of, an earlier one gets `-Wexceptions`
  `exception of type T will be caught by earlier handler` with the note
  `for type U` (clang `CheckCXXTryBlock`, references and pointers
  compared by their pointee). Exact duplicates use clang's
  `CatchHandlerType` key (WP-160): the top-level cv-qualifiers are dropped
  first, then a reference or pointer is stripped and the pointee keeps its
  cv-qualifiers. So `int` then `const int &` is not a duplicate, while
  `const int` then `int &` is. The public-base search ignores cv.
- `push_try_scope` marks the scope a try barrier (`Scope::try_barrier`)
  for the implicit move of thrown locals (§5.5).
- Function-try-blocks: `check_function_try_block` (from
  `act_on_finish_function_def`) diagnoses `return` in a constructor's
  handler (`return in the catch of a function try block of a
  constructor is illegal`) and warns (`-Wexceptions`, WP-118) at each
  member access whose base is `this` (written or implicit, under
  implicit casts only, so not `(this)->m` or `(*this).m`, and not in a
  lambda) in a constructor's or destructor's handler: `cannot refer to
  a non-static member from the handler of a constructor|destructor
  function try block` ([except.handle]/10; clang
  `IsInFnTryBlockHandler`, once for a template at its definition);
  falling off a constructor or destructor
  handler rethrows ([except.handle]/14), so the fall-off analysis
  (§4.3) treats such handlers as not returning, and a value-returning
  function's handler that falls off is `-Wreturn-type`.
- Analysis warnings ([except.spec]/5, [dcl.attr.noreturn]/2), skipped
  in templates: a `throw` that can escape a non-throwing function is
  `-Wexceptions` `'f' has a non-throwing exception specification but
  can still throw` with `function declared non-throwing here` (or the
  destructor/deallocator note); in an instantiated function template
  specialization the name carries its arguments, unqualified (`'k<int>'`,
  `except.spec/throw-in-nothrow-specialization.cpp`); a `[[noreturn]]`
  function that can fall off is `-Winvalid-noreturn`. Only the throws
  reachable in the body's graph count (§7.5.1, `cfg_escaping_throws`;
  a body with no throw-expression builds none). The fall-off analysis
  behind -Wreturn-type / -Winvalid-noreturn (`falls_off`) reads the
  graph's live exit predecessors (`exit_kinds`, clang
  `CheckFallThrough`). A `return`/`co_return` edge, a fake edge (a
  throw with no enclosing try) and an abnormal edge (a `noreturn` call
  or destructor, or a try dispatch with no catch-all and no enclosing
  try, a dispatch with no predecessor counting as live) are the
  non-plain kinds; the end of the body is the plain edge. No plain edge
  is `Never`; a plain edge plus any other is `Maybe` (`... in all
  control paths`); a plain edge alone is `Always`. A body with no graph
  (an `if consteval` under clang 19, or over the size cap) is `Never`.
  A body whose last statement is a `return` is `Never` without a graph
  (WP-798): the closing brace is reached only through that statement.
  The graph costs 8 bytes per expression and nothing else may ask for
  it, so building it for every value-returning body cost
  bench/overload/arrow-chain.cpp 24 MB
  (`stmt.return/return-ending-body-no-cfg.cpp`).
- Initialization (one condition in `init.rs`, flagged): brace elision
  ([dcl.init.aggr]/16) only descends into an aggregate element; a class
  with a user-provided constructor is copy-initialized from the element
  instead (`brace-elision-class.cpp` AST golden).
- **Left (WP-100):** a written vs implicit `noexcept` on deallocators in
  the note. (The handler member-reference warning, throw reachability,
  `arr()` member value-initialization of a class array and the
  `new K[2][3]` constructor initializer landed in WP-118.)

#### 7.5.1 The control-flow graph (`cfg/`, WP-509)

One graph per analysed function body (clang `CFG`, built by
`AnalysisBasedWarnings::IssueWarnings` with `PruneTriviallyFalseEdges`,
`AddInitializers`, `AddImplicitDtors`, `AddTemporaryDtors`, no EH edges;
docs/design/reachability.md). Every flow-sensitive consumer queries the
same graph.

- **Lifetime** (`cfg/body.rs`). The end of a body that is analysed (no
  uncompilable error, not a dependent context) *plans* a graph
  (`cfg_plan`) with the side tables its enabled consumers need
  (`CfgReq`): the deferred runtime-behavior diagnostics' anchors
  (`probes`), the statement flow when `-Wimplicit-fallthrough` or the
  misplaced-`[[fallthrough]]` check is on, the unreachable-code tables
  when a `-Wunreachable-code*` id is on outside an instantiation. The
  first query (`with_cfg`) builds it. The next body clears it: the
  vectors are kept, so steady-state memory is bounded by the largest
  body, and a body no consumer asks about builds nothing.
- **Model** (`cfg/mod.rs`). Flat vectors of blocks, elements and edges.
  A block holds ranges into them, so there is no `Rc`, no pointer graph
  and no `unsafe`. Block 0 is the entry and block 1 the exit; the body's
  blocks follow in creation order. An element is one of:
  - a statement (a `[[fallthrough]]`, a `goto`, a `return`, a
    declaration);
  - an expression, after its operands;
  - a constructor's member initializer;
  - the destructor call of an automatic variable;
  - the destructor call of a temporary.

  A block ends in a terminator: `if`, a loop, `switch`, a try dispatch,
  `&&`/`||`, `?:`, a temporary-destructor decision, a jump, `asm goto`
  or the indirect-goto block. Each edge carries bits:
  - `LIVE`: clang's pruned graph;
  - `SCAN`: the edge as clang's unreachable-code scan follows it;
  - `NULL`: a dropped loop edge;
  - `RETURN`, `FAKE` (a throw leaving the body) and `ABNORMAL` (a
    `noreturn` call or destructor, or a dispatch with no catch-all
    leaving the body), which classify the exit's predecessors.

  A pruned edge stays in the graph as clang's unreachable alternate.
- **Builder** (`cfg/build.rs`). One forward walk over the body (clang's
  `CFGBuilder` walks backwards; the graphs agree up to numbering). Blocks
  are made lazily: when an element is pushed, when a label or case
  starts one, and at joins.
  - A condition that `known_truth` folds (`cfg/truth.rs`, clang
    `tryEvaluateBool`: constant folding, `&&`/`||` decided by one side,
    `checkIncorrectLogicOperator`) prunes the other edge of an `if`, a
    loop, a `?:` or an `&&`/`||`. In the scan bits, a configuration
    value (clang `isConfigurationValue`) does not prune and an
    `if constexpr` prunes nothing. An `if` with a condition variable has
    an unknown condition, except an `if constexpr`, whose condition is
    folded even then: an instantiation keeps only one branch, and the
    dropped one's edge must be dead for -Wreturn-type (WP-792).
  - An `if`/`while`/`for` condition (without a condition variable) that
    is `&&`/`||` is clang's `VisitLogicalOperator`. Each left operand's
    block ends in its operator, and the short-circuit edges go straight
    to the statement's targets. A `do` condition, and an operator used
    as a value, joins in one block that holds the value.
  - `return`, `break`, `continue` and `goto` run the destructors of the
    scopes they leave and jump. A label starts a block: a forward
    `goto` is patched when its label is met, and a backward one targets
    the label's block.
  - A `switch` has an edge to each case. A constant condition makes only
    the matching case (else the default) live, and a GNU case range
    matches `[lo, hi]`. The default edge, or the edge past the body when
    there is no default, comes last. It is not live after a match, nor
    over an enumeration whose enumerators all have cases (clang
    `isAllEnumCasesCovered`). The scan follows every case edge.
  - A throw-expression ends its block. With no enclosing try it goes to
    the exit (`FAKE`); otherwise it goes to the innermost try's dispatch
    block. A dispatch has an edge to each handler, then, without a
    catch-all, to the enclosing dispatch (or the exit, `ABNORMAL`).
    Calls have no exception edges.
  - A call to a `noreturn` function, or a `noreturn` destructor (of a
    variable or a temporary), ends a `NORETURN` block whose only
    successor is the exit (`ABNORMAL`).
  - Temporaries (clang `TempDtorContext`). Inside an
    `ExprWithCleanups`, each `CXXBindTemporaryExpr` that is not
    lifetime-extended records its destructor. Those destructors follow
    the full-expression in reverse construction order. A temporary made
    in a conditional operand (an arm of `?:`, the right operand of
    `&&`/`||`) is destroyed under a decision block (`(Temp Dtor)`)
    branching on that operand's first temporary, unless the operand is
    known to run. When both arms of a `?:` have temporaries, the false
    arm's decision falls back to the true arm's.
  - GNU `goto *p` jumps to one indirect-goto block, which has an edge
    to every label whose address the body takes (`AddressTakenLabels`).
    A live `asm goto` has an edge to each of its labels besides the
    next statement.
  - The operands of `sizeof`/`alignof` are not walked; a `noexcept`
    operand is.
  - Under `-fclang-compat=19`, an `if consteval` anywhere in the body
    means clang builds no graph (`Status::NoCfg`); clang 22 branches on
    an unknown condition.
- **Budget.** A graph over `-fkiln-cfg-max-elems` elements (default
  33554432, about 1 GiB at the cap) is not built (`Status::TooLarge`).
  Its body's flow-sensitive warnings are skipped, with the kiln remark
  `-Rkiln-analysis-skipped` (`flow-sensitive warnings for 'f' skipped:
  its control-flow graph exceeds -fkiln-cfg-max-elems=N`), emitted once.
- **Queries** (`cfg/query.rs`), each a forward reachability over the
  edge bits from the entry:
  - `unreachable_probes`: the anchors met only in unreachable blocks.
    The runtime-behavior filter drops their diagnostics (an anchor the
    walk never meets counts as reachable).
  - `exit_kinds`: the live predecessors of the exit by kind. A dispatch
    block with no predecessor is also a root, as clang
    `CheckFallThrough` rescans it.
  - `cfg_escaping_throws`: the reachable throw-expressions that no
    enclosing handler catches (`throw_caught`).
  - `stmt_flow` and `dead_scan`: the fallthrough and unreachable-code
    tables (§7.10).
- **Dump.** `-fkiln-dump-cfg` prints every planned graph in clang's
  `debug.DumpCFG` layout: `[B<n> (ENTRY)]`, then the body's blocks from
  the highest number, then `[B0 (EXIT)]`. Each block shows:
  - its numbered elements as `[B<n>.<i>]`;
  - `T: <terminator>`;
  - its `Preds`;
  - its `Succs`, with `(Unreachable)` and `NULL` alternates.

  The `expect: cfg` tests (spec/harness.md "cfg tests") compare its
  shape with clang's.
- **Titles and `-fclang-compat`.** Each graph is titled with its
  function's qualified name and `()` (`Sema::cfg_title`, also the
  `-fkiln-cfg-stats` name). A closure type always prints as `(lambda)`
  (`lambda(int)::(lambda)::operator()()`), whatever the
  `-fclang-compat` level. Diagnostics at `-fclang-compat=19` print it as
  `(anonymous class)`, clang 19's `printQualifiedName`. The dump is
  different because no clang release has a lambda title to follow: clang's
  analyzer dumps no lambda call operator. So the title stays the same
  under either oracle, one golden serves both, and the harness can drop
  kiln's `(lambda)` graphs from the comparison with the oracle
  (`cfg_shape::fn_name`). Nothing else in the dump depends on the compat
  level.
- **Stats.** `-fkiln-cfg-stats` (a measurement aid, spec/driver.md)
  builds every planned graph and prints one line per body:
  `cfg-stats: 'f' status=Ok blocks=N elems=N edges=N bytes=N build_ns=N`,
  `bytes` being the lengths of the block, element, edge and adjacency
  vectors times their sizes (`Cfg::bytes`; the side tables are not
  counted). The numbers of record are in `spec/landed/sema/WP-509.md`.
- **Hooks for the uninitialized-values pass (WP-510).** The graph keeps
  the element order of evaluation inside each block, the declaration
  statements as elements, and every block's predecessors, which a
  forward dataflow needs; nothing else is built for it yet.

#### 7.5.2 Consumers of the graph (WP-509)

Each consumer runs at the end of a non-dependent body with no
uncompilable error, in this order: the runtime-behavior filter
(`unreachable_probes`, §5.12: a queued diagnostic whose anchor only an
unreachable block holds is dropped), the fall-off analysis
(-Wreturn-type / -Winvalid-noreturn, §7.5 above; clang 22's inferred
`noreturn`, §7.5.3, runs first), -Winfinite-recursion
and the throw checks of -Wexceptions (§7.5 above), then implicit
fallthrough (§2, `[[fallthrough]]`) and -Wunreachable-code* (§7.10)
from `stmt_flow` / `dead_scan`. The AST walks they replace
(`try_stmt/reach.rs`) are deleted.

#### 7.5.3 -Winfinite-recursion and -Wmissing-noreturn (WP-509)

- **-Winfinite-recursion** (`infinite_recursion.rs`, clang
  `checkRecursiveFunction`; in -Wall via -Wmost, the diag off by
  default otherwise): `all paths through this function will call
  itself` at the start of the body, when every live path from the
  graph's entry meets a recursive call before reaching the exit and the
  exit has a live predecessor ([intro.progress] makes no promise; the
  warning is clang's). A path stops at the first block holding a
  recursive call. A recursive call is a call, member call or operator
  call whose callee's first declaration is the function's, except a
  call through a name qualified by a template specialization
  (`A<T>::f()`) and a virtual member call on an object other than
  `this`. Not checked: a specialization of a function template (clang
  checks non-templated functions and members of class template
  specializations, whose instantiation context note it prints). Before
  building a graph the body's expressions are scanned for a recursive
  call (a constructor's initializers, built earlier, are always
  graphed), so a non-recursive body costs no graph.
- **-Wmissing-noreturn, graph form** (`missing_noreturn.rs`, clang
  `CheckFallThroughForBody` `NeverFallThroughOrReturn`; not in -Wall or
  -Wextra): `function 'f' could be declared with attribute 'noreturn'`
  at the start of the body of a function returning `void` whose graph
  has no live plain or return predecessor of the exit. Not for a
  `noreturn` (declared or inferred) function, a virtual function, a
  lambda's call operator, a coroutine, a `naked` function or anything
  instantiated from a template. A body with no graph does not warn.
- **-Wmissing-noreturn, inferred form** (clang 22 `inferNoReturnAttr`,
  `-fclang-compat=22` only; run when -Wreturn-type or -Wmissing-noreturn
  is enabled, before the body's other flow checks): a body that is a
  compound statement whose only statement is a throw-expression (under
  an `ExprWithCleanups`) makes a function that is not `noreturn` and
  not virtual *inferred noreturn* (`Sema::inferred_noreturn`), with the
  warning at the function's name. The pattern of a function template is
  skipped; an instantiation of another templated entity (a member of a
  class template, a lambda in a template) inherits its pattern's
  inference without a second warning, while a function template
  specialization is inferred, and warned, afresh. A lambda's call
  operator below C++23 is inferred without the warning (clang 22 warns
  for one only from C++23; WP-841, range-v3 `chunk_by.cpp`
  `[](int, int) -> bool { throw 0; }` under `-std=c++20`). An inferred function
  is not suggested again by the graph form. In a value-returning
  caller, a body whose last statement is a throw-expression, or a call
  whose direct callee is inferred noreturn, has no -Wreturn-type
  (clang's syntactic check ahead of `CheckFallThroughForBody`).
- Tests: `expr.call/infinite-recursion*.cpp`,
  `dcl.attr.noreturn/missing-noreturn*.cpp` (the `-inferred` pair under
  `requires: clang>=22` / `clang<22`; `-lambda-cxx20` and
  `-lambda-cxx23`),
  `stmt.pre/cfg-analysis-skipped.cpp` (the size cap remark).

### 7.6 Landed (WP-089): range-based `for` (`range_for.rs`)
Sema desugars `for (init; decl : range) body` ([stmt.ranged]/1) the way
clang's `ActOnCXXForRangeStmt`/`BuildCXXForRangeStmt` does, into a
`CXXForRangeStmt` (ast.md, `RangeForData`) holding `__range{d}`
(`auto&&`), `__begin{d}` and `__end{d}` (`auto`), the condition
`__begin != __end`, the increment `++__begin` and the loop variable
initialized from `*__begin`. `d` is clang's scope depth of the for's
control scope halved (`clang_scope_depth()/2`, with the depth outside the
template saved in `range_for_outer_depth` for instantiations).

- **Parser contract.** `parse_for` pushes the control scope and scans the
  head for a top-level `:` (`at_for_range_head`). The range path calls
  `act_on_declarator` for the loop variable, then
  `act_on_for_range_decl(ds, v)` (storage classes:
  `loop variable 'x' may not be declared 'static'`; a type definition:
  `types may not be defined in a for range declaration`), parses the
  range inside `begin_for_range_init(extend)`/`end_for_range_init`
  (P2718 collection, sema-class.md §5.4), and calls
  `act_on_for_range(init, loop_var, range, temps, colon, loc)`, which
  returns a `RangeForHead`; after the body,
  `act_on_finish_for_range(head, body)` builds the statement. A failed
  head (`None`) or a failed declaration (`act_on_for_range_error`) marks
  the loop variable invalid and the body is still parsed. `for (x : r)`
  is `range-based for loop requires type for loop variable` (the
  variable gets type `auto`); a structured binding is declared by
  `act_on_decomposition_declarator` (§4.9); an
  init-statement before C++20 is `-Wc++20-extensions`.
- **Range kinds.** An array range (a constant array; a VLA is
  unsupported) uses `__range` and `__range + N` (`N` a `ptrdiff_t`
  literal). A class range uses member `begin`/`end` when the class has
  either member ([stmt.ranged]/1.3, P0962), else ADL-only calls
  `begin(__range)`/`end(__range)` (`build_begin_end`), each checked for
  viability before it is built. A braced-init-list range deduces
  `std::initializer_list<E>` for `__range`.
- **Diagnostics (clang's text and locations).**
  `invalid range expression of type T; no viable 'begin'|'end' function
  available` with the overload candidates as notes; a lone member is
  `member is not a candidate because range type T has no 'end'|'begin'
  member`; an incomplete class is `cannot use incomplete type T as a
  range` with the forward-declaration note; a pointer whose pointee is a
  valid range is `invalid range expression of type T; did you mean to
  dereference it with '*'?` and the loop is rebuilt on `*range`
  (`rebuild_with_dereference`); an iterator or loop-variable failure gets
  `selected 'begin' function with iterator type T` (the candidate notes
  are captured through `diag_capture` while each attempt runs and
  released on success of the diagnosis). The capture holds only the
  step's own diagnostics: it records the instantiation-stack depth it
  started at, so an error raised in a deeper frame (a candidate's
  deduction substituting an alias or a default template argument) still
  reaches that frame's SFINAE trap (sema-templates.md §5.7) instead of
  being held and released as a hard error. Overload-candidate notes
  (`trailing_notes`) attach to the held error before it is held.
  One such deeper SFINAE context is deducing a converting constructor
  template for a rewritten `operator==` candidate of `__begin != __end`
  (WP-271); another is the requires-expression of libc++
  `sentinel_wrapper::operator==` instantiated by `__begin != __end`
  (WP-280, `temp.inst/range-for-member-substitution-context.cpp`).
  (WP-274.)
- **Templates.** A type-dependent range (or loop-variable type) keeps
  only `__range` and the loop variable in the pattern; instantiation
  (`templ/subst/stmts.rs` `range_for`) substitutes the range under a new
  P2718 collection, declares the loop variable, and finishes the head
  with `instantiate_for_range`. A non-dependent range is desugared in
  the definition.
- **Fixed on the way (WP-089).** Reference binding and reference ICS
  compare reference-relatedness on types made unqualified through array
  levels (clang `getUnqualifiedArrayType`), so `int[3]` binds to
  `const int (&)[3]`. `auto` deduction under a reference or pointer
  declarator keeps an array result as `Auto` sugar deduced to the
  cv-unqualified-through-arrays array (clang's `AutoType`), so
  `auto &r = a;` prints `int &[2]` as clang does; the dump printer keeps a
  dependent `auto` (deduced as `<dependent type>`) as `auto`.
- **Left (WP-089):** `err_range_on_array_parameter` (needs
  `ParamData.original_ty`, which `act_on_param_declarator` does not set,
  so an array parameter gets the generic `'int *'` diagnostic);
  VLA ranges; the default-on
  `-Wempty-body`-style `warn_empty_range_based_for_body` and the
  DefaultIgnore `-Wrange-loop-construct`/`-Wrange-loop-bind-reference`
  warnings (groups declared, not emitted); the begin/end-types-differ
  pre-C++17 extension warning; clang's recovery for `for (int x, y : a)`.

### 7.6a Landed (WP-447): Expansion statements (`expansion.rs`)
P1306R5 `template for` ([stmt.expand]). The parser accepts
`template for ( init-statement_opt for-range-declaration :
expansion-initializer ) compound-statement` in every language mode; before
C++26 it warns `ext_expansion_statements` (-Wc++26-extensions). The body
is a compound-statement; any other statement is accepted (WP-533, clang
23 and GCC 16.1 do) with the off-by-default `Extension`
`parse_ext_expansion_stmt_body_not_compound_stmt` ("ISO C++ requires the
body of an expansion statement to be a compound statement",
-Wexpansion-stmt-missing-braces, so -pedantic shows it and
-pedantic-errors rejects it) at the body's first token, labels and
attributes looked through; a compound-statement after attributes (`[[]]
{...}`, `__attribute__((...)) {...}`) gets
`parse_ext_expansion_stmt_body_attr` ("ISO C++ forbids attributes before
the compound statement of an expansion statement") instead. A
non-compound body is parsed as any loop's sub-statement (its own block
scope, [stmt.pre]/1; `break`/`continue` as in a braced body) and kept
as written; its element block `{ decl = element; body }` spans the
body's own location (no `}`). `template`
followed by anything but `for` in a block keeps clang's `'template' here is
not supported yet`.

- **Synthetic template level.** `act_on_expansion_head` opens a template
  level of its own (`push_expansion_level`, a parameterless head at the
  next depth, raising `dependent_depth`) and declares the
  for-range-declaration in it, initialized from a `CXXExpansionSelectExpr`
  (form, depth). The body is parsed as a template pattern at that level.
  Inside a template the statement stays a pattern (`CXXExpansionStmt`
  without `expanded`) and is expanded when the enclosing template is
  instantiated; otherwise `act_on_finish_expansion` expands it at once.
  The element variable's uses in the body are checked as consteval-only
  leaves in each expansion, as a plain `constexpr` variable's are
  (spec/reflection-repr.md §6, spec/consteval-blocks.md §4; WP-1330).
- **Expansion** (`Substituter::expand_expansion`, templ/subst): a child
  Substituter whose `expansions` stack maps the level's depth to the
  current element; each `CXXExpansionSelectExpr` at that depth becomes the
  element, each element gets a block `{ decl = element; body }`
  ([stmt.expand]/5). An inner pattern that depends on the element is
  expanded within it. The form is chosen on the substituted initializer:
  - *enumerating*: an expansion-init-list; element i is its i-th
    initializer-clause. The list is an expression-list
    ([stmt.expand]/1), so a top-level designator is `expected
    expression` (clang 23 parses it with `ParseExpressionList`). A
    braced element (WP-454) is kept as written, a `void`-typed
    `InitListExpr` (`dependent_init_list`; a braced pack expansion
    `{x}...` is expanded in a template as a call argument's is), and
    `Substituter::initializer` turns the select it replaces into
    `Initializer::List`: the for-range-declaration is
    copy-list-initialized exactly as `decl = {...};` ([stmt.expand]/5.1,
    [dcl.init.list]/3; designated lists against an aggregate per
    [dcl.init.list]/3.1, `auto` deducing `std::initializer_list` per
    element, a structured binding of it ill-formed as for `auto [a, b] =
    {1, 2};`).
  - *Element temporaries* (WP-1106; [class.temporary]/8 as amended by
    CWG3043 in the C++26 DIS). A temporary created in element E persists
    for the lifetime of the for-range-declaration E initializes, whatever
    its type (`auto x : {D{2}.v}` prints `x2 ~D2`), and is destroyed after
    it, in reverse order of construction ([class.temporary]/11), at the
    end of the element's block `{ decl = E_i; body }`. Sema: the element
    expression is built before its declaration, so `expansion_select`
    re-marks its temporaries for the declaration's full-expression
    (`note_shared_temporaries`, which gives the initializer its
    `ExprWithCleanups`), and `expansion_element` then calls
    `extend_init_temporaries` (`class/temporary.rs`), which marks every
    `MaterializeTemporaryExpr` of storage `FullExpression` in the
    initializer `Automatic`, extended by the variable (a
    `LifetimeExtendedTemporaryDecl` each), not entering nested
    full-expressions (statement expressions, lambda bodies) or
    unevaluated operands. Codegen's `push_temporary_cleanup` and the
    Oven's `begin_extended`/`end_extended` then handle them as a
    reference's ¶6 temporaries: destroyed after the variable, on every
    path out of the block. In the Oven a non-reference local's
    initializer may now extend temporaries, so its block opens a cleanup
    scope (`open_scope`, `extends_temporaries`) and a scalar register
    local preallocates them too. Evaluated from outside (codegen's
    constant-folding prepass of a POD or `const` initializer), such a
    temporary is destroyed at the end of the evaluated initializer, so
    a non-constexpr destructor makes it non-constant as in clang
    (`discardCleanups`) and is not folded away. The lifetime check
    (`check_var_lifetime`) is skipped for the declaration
    (`expansion_element_init`): nothing in it dangles. **GCC 16.1**
    implements the earlier resolution (no extension: `~D2 x2`), and
    upstream clang implements the DIS (llvm#218710; clang 22 has no
    P1306), so the run tests carry kiln goldens (`oracle: none`).
    A discarded class prvalue (`(void)D{1}`, a comma's left operand) is
    a temporary created in the element too ([expr.context]/2,
    [expr.static.cast]/6): the expansion-init-list is a
    lifetime-extending context (the parser, and `expansion_stmt` when it
    substitutes a pattern's list, open an `extend_collector`), so
    `materialize_discarded` materializes it and it is extended like any
    other (WP-1117; spec/sema-class.md §5.4;
    `element-discarded-temporary{,-constexpr}-run.cpp`). In an ordinary
    declaration it is only bound and dies with the full-expression.
    *Default arguments* (WP-1149; [intro.execution]/5): a default
    argument's temporaries are created in the full-expression of the
    call that uses it, so one used in an element is extended with the
    element's (`{dv(), 8}` with `int dv(const D& = D{7})` prints
    `w7 ~D7 w8`), including a default argument's own default argument.
    The parameter's default argument is one expression shared by every
    call, so, as clang does in a lifetime-extending context, each use
    (`CXXDefaultArgExpr`) is given its own copy with fresh
    `MaterializeTemporaryExpr`s (`fresh_temporaries`, a structural copy
    of the path from the root to each `FullExpression` temporary,
    `ExprArena::clone_with_children`), recorded in the cold map
    `default_arg_rewritten`; `Ast::default_arg_expr` returns the copy, so
    codegen and the Oven evaluate and destroy it as the use's own, and
    the Oven's `extended_temps`/`extends_temporaries` walks enter it.
    `extend_init_temporaries` then extends the copy's temporaries like the
    initializer's own. The same rewrite serves a destructuring
    expansion-initializer and the P2718 for-range-initializer
    (`extend_range_default_args`, `range_for.rs`; spec/sema-class.md).
    WP-1166: the copy covers every evaluated kind on the way to a
    temporary (compound assignments, rewritten operators, `new`/`delete`,
    a potentially-evaluated `typeid`, casts, `__builtin_choose_expr`, GNU
    `?:` with its `OpaqueValueExpr`s rebound to the copied common
    expression, `ExprArena::clone_opaque`;
    `stmt.ranged/p2718-default-argument-kinds-run.cpp`), and a
    `CXXDefaultInitExpr` (below), except a few vendor builtins
    (`fresh_temporaries` lists them). The jump-scope
    checker and the CFG enter the copy (clang 19 and 22 do not: their
    `CXXDefaultArgExpr` has no children), so an indirect `goto` out of
    the loop leaves the copy's extended temporaries' scope
    (`stmt.goto/computed-goto-default-argument-temporary.cpp`) and a
    `[[noreturn]]` call in the copy ends the path
    (`stmt.ranged/p2718-default-argument-noreturn.cpp`); a default
    argument that is not rewritten stays one CFG element, as in clang.
    The never-constant walk stops at range-based `for` and expansion
    statements, so it never reaches a copy made there (its temporaries
    scan enters one anyway, and a variable's member initializer copy,
    below, too). The module declaration
    image carries the copy (cold field `default_arg_rewritten`,
    spec/modules.md §11.11), so an importer building an inline or
    constexpr body from the image extends the same temporaries
    (`module.interface/image-default-temporaries-run.cpp`).
    *Default member initializers* (WP-1177; [class.temporary]/6-7,
    [intro.execution]/5): a default member initializer an aggregate
    initialization uses is part of the enclosing full-expression too, so
    in a lifetime-extending context its temporaries are extended like a
    default argument's. clang's rule (`Sema::BuildCXXDefaultInitExpr`):
    the field's initializer is rebuilt for the use
    (`EnsureImmediateInvocationInDefaultArgs::TransformInitializer`, which
    drops its `ExprWithCleanups`) when it holds an immediate invocation,
    or when the context is lifetime-extending
    (`needsRebuildOfDefaultArgOrInit`: `InLifetimeExtendingContext`, set
    for a C++23 for-range-initializer and inherited by the aggregate's
    `InitListChecker`) and the initializer is an `ExprWithCleanups`; the
    rebuilt temporaries are then extended by `__range` like the others.
    The immediate-invocation rebuild is not needed in kiln (its
    `ConstantExpr`s are evaluated at the use, WP-1106). kiln follows the
    lifetime-extending half: `extend_walk` gives each `CXXDefaultInitExpr`
    use whose member initializer is an `ExprWithCleanups` a copy of its
    operand with fresh temporaries (`fresh_temporaries`, the same map
    `default_arg_rewritten`; `Ast::default_init_expr` returns it, so
    codegen and the Oven evaluate and destroy the use's own), in the
    aggregate written in the range-initializer, in a default argument's,
    and nested (`mk(Outer{})`, a default argument's own use, a class
    template's instantiated initializer, which `templ/class.rs` `nsdmi`
    rebuilds without the pattern's `ExprWithCleanups` so the conversion
    to the field's type is inside the new one, as clang has it). A
    member initializer that is no `ExprWithCleanups` is not rebuilt, so
    the temporaries of the member initializers it uses in turn stay its
    own (`struct O2 { Agg a = Agg{}; }`, as clang 22; GCC 16.1 extends
    them), and neither is an array filler, one initializer for every
    element (clang 22 and GCC 16.1 agree). A default argument used by
    the member initializer is extended with it (GCC 16.1; clang 22 does
    not, as for a nested default argument). Outside a lifetime-extending
    context the use shares the field's initializer and its temporaries
    end with the full-expression
    (`class.temporary/default-member-initializer-full-expression-run.cpp`).
    CWG1815 ([class.temporary]/6): a temporary that a reference member's
    default member initializer binds, in a variable's aggregate
    initialization, is extended with the variable
    (`temporaries_bound_by`/`temporaries_in_aggregate` rebuild the use,
    `rebuilt_default_init`, for a reference member and for a class or
    array member, so an `std::initializer_list` member's backing array
    too). That copy keeps the initializer's `ExprWithCleanups`, as
    clang's does: the variable's initializer has none of its own when
    the shared one held every temporary, and the copy's unextended
    temporaries end at its end; a later lifetime-extending walk looks
    through it (`rewritten_operand`). Like clang 22, codegen destroys a
    temporary so bound in an aggregate temporary that a reference
    variable extends (`const Q& q = Q{};`) at the end of the
    full-expression, since its cleanup is pushed in two nested
    full-expression scopes; GCC 16.1 extends it. Before WP-1177 the
    shared initializer's temporary died with it (a dangling reference).
    Tests: `stmt.ranged/p2718-default-member-initializer{,-nested,-default-argument}-run.cpp`,
    `-constexpr.cpp` (the Oven), `class.temporary/cwg1815-default-member-initializer-run.cpp`,
    `ast/class.temporary/default-member-initializer-rewritten.cpp` (clang
    22's dump) and `module.interface/image-default-member-init-temporaries-run.cpp`
    (the image's cold field carries the copy; before, its decoder
    rejected the field on a `CXXDefaultInitExpr`, so such a unit had no
    image).
    GCC 16.1 extends no element temporaries and rejects reading one in
    constant evaluation, so the tests carry kiln goldens
    (`element-default-argument-temporaries-run.cpp`, `oracle: none`).
  - *Element immediate invocations* (WP-1106; [expr.const]/16): an
    immediate invocation in an element is evaluated at the end of its
    declaration's full-expression. The element's `ConstantExpr` was built
    with the list, before any full-expression that uses it, so
    `expansion_select` hands its unevaluated invocations to the
    declaration's full-expression (`note_deferred_invocations`,
    `consteval.rs`); previously they were never evaluated and the program
    called the consteval function at run time. A temporary inside an
    immediate invocation ends within it: `extend_init_temporaries` does
    not enter a `ConstantExpr`. Test: `element-immediate-invocation-run.cpp`
    (GCC oracle).
  - *iterating* (`range_for.rs` `expansion_iterable`): not an array, and
    member `begin`/`end` are found or ADL finds viable candidates;
    `constexpr` `__range`, `__begin`; element i is `*(__begin + i)`
    through a per-element `constexpr auto __iter = __begin + i`. All three
    are `static` unless the range is bound by automatic reference (see
    "Binding a constexpr range" below), when they are automatic.
    N is the first i for which `__begin + i != __end` is false
    ([stmt.expand]/5.2 counts with `++` in a consteval lambda; `__begin +
    i` is what each element uses). *Fast path* (WP-454,
    `expansion_size_difference`): when `__begin` and `__end` have the same
    type and it is (a) a pointer to a complete object type, or (b) a
    specialization of libc++'s `std::__wrap_iter`, `std::__bounded_iter`
    or `std::__static_bounded_iter`, or libstdc++'s
    `__gnu_cxx::__normal_iterator`, whose first template argument is a
    pointer (the iterators of `std::array`, `std::span`,
    `std::basic_string_view`, `std::basic_string` and `std::vector` are
    these or pointers), N is `__end - __begin` evaluated once, when that
    is a constant expression with a non-negative integral value. For
    these types `+`, `-` and `!=` are the pointer's, so the count is the
    same. Any other iterator, or a failed or negative difference, is
    counted incrementally, one evaluation per i, capped at 2^20
    (`sema_expansion_size_not_constant` when an evaluation fails or the
    cap is reached); the fast path has no cap. A class iterator whose `!=`
    disagrees with its `-` gets the comparison's count
    (`tests/conformance/stmt.expand/iterating-count.cpp`). *One pass*
    (WP-467, `expansion_count_elements`): the incremental count and the
    elements are built together. For i ≥ 1 element i's `__iter` is built
    first and the count tests `__iter != __end` (i = 0 tests `__begin !=
    __end`, so an empty range never builds `__begin + 0`), so `__begin + i`
    is resolved and evaluated once per element, not once for the count and
    again for the element; the `__iter` built for i = N is dropped. The
    left operand of `!=` is then a `const` lvalue, as the wording's `i !=
    end` names a variable, rather than the prvalue `__begin + i`. The pass
    runs with its diagnostics captured; on any error (or a non-constant
    test) it is discarded and the separate count and element loop above
    run and diagnose, so diagnostics are unchanged. Per element it still
    resolves `+`, `!=` and unary `*` and evaluates `__iter`, the test and
    the element (docs/status/perf-history.md "WP-467";
    `tests/stress/expansion-iterating-user-iterator.cpp` budgets the
    evaluations and steps).
  - *destructuring*: otherwise, as by `auto&& [__u0, ...] = init;`
    (`struct_bind.rs`); element i is `__u_i`. With a `constexpr`
    for-range-declaration the declaration is `constexpr` (`static`
    unless bound by automatic reference, below). The
    initializer's temporaries persist for the lifetime of that reference,
    to the end of the whole statement ([class.temporary]/9, CWG3043;
    `expansion_destructuring` calls `extend_init_temporaries` on the
    declaration that won, after `expansion_decomp` or the constexpr
    `expansion_fallback` attempts, so a discarded attempt marks nothing;
    a no-op for the `static constexpr` form, whose temporaries are part
    of its constant initialization). The expansion-initializer is a
    lifetime-extending context (WP-1149): the parser, and
    `Substituter::expansion_stmt` when it substitutes a pattern's
    initializer, open an `extend_collector`, so a discarded class prvalue
    (`((void)D{1}, f())`, a comma's left operand) is materialized and
    extended, and so are the temporaries of the default arguments it uses
    (above). `expansion_decomp` re-marks the initializer's temporaries for
    the declaration's full-expression (`note_shared_temporaries`), so an
    lvalue initializer (`id(T2{...})`) gets its `ExprWithCleanups`.
    GCC 16.1 agrees, including default arguments
    (`tests/conformance/stmt.expand/destructuring-temporaries-run.cpp`,
    `destructuring-initializer-temporaries-run.cpp`).
    *Shared pattern nodes* (WP-1149): `extend_init_temporaries` sets a
    `MaterializeTemporaryExpr`'s storage in place (`extend_temporary`,
    write-once). The Substituter reuses a non-dependent subtree of a
    pattern in every instantiation, and a nested statement's elements
    reuse its initializer, so one node can be marked by several
    declarations. That is safe: the node is at the same syntactic
    position in each, so the mark is the same decision every time, and
    codegen and the Oven create a temporary per evaluation keyed on its
    storage duration, not on the extending declaration; the first
    declaration's `LifetimeExtendedTemporaryDecl` is only a dump label.
    A rewritten default-argument use shared the same way reuses its
    copy. A dependent temporary is rebuilt per instantiation
    (`shared-pattern-temporaries-run.cpp`, GCC oracle, run time and
    constant evaluation).
    The iterating form's `__range` is `static constexpr` or a
    `constexpr` reference to a glvalue (Automatic, below), so its
    temporaries are part of its constant initialization and never
    observable at run time.
- **Binding a constexpr range** (`expansion_fallback`, `RangeBind`).
  A prvalue initializer is bound by value (`RangeBind::Copy`, `static
  constexpr auto`, const, so its value is usable in constant expressions;
  a lifetime-extended temporary would not be). A glvalue is bound by
  reference, trying in order, each attempt in a block scope of its own
  with its diagnostics captured (a failed constant initialization does not
  mark the variable INVALID, so failure is `stash_has_error`):
  1. `RangeBind::Static`, `static constexpr auto&&`: a range designating
     an object of static storage duration;
  2. `RangeBind::Automatic` (C++26, and only when the
     for-range-declaration is `constexpr`; P2686R5, WP-1122): a non-static
     `constexpr auto&&` (the draft's [stmt.expand]/5 wording, `constexpr`
     only when the for-range-declaration is), with `__begin`, `__end` and
     each `__iter` automatic too, so a range naming a `constexpr` local
     (or a parameter-scope automatic object constexpr-referenceable from
     the statement, §4.9) is bound by reference, not copied: element
     addresses are the local's, and a non-copyable range works;
  3. `RangeBind::Copy`, `static constexpr auto`: the fallback for a
     glvalue that is not constexpr-referenceable but whose value is
     constant (before C++26, or a non-`constexpr` for-range-declaration).
  A non-`constexpr` for-range-declaration never tries Automatic: by the
  wording its `__range` is then not `constexpr` at all, and kiln keeps
  rejecting a `__range` that is not a constant expression
  (`iterating-neg.cpp`). When all attempts fail, the first attempt's
  diagnostics are emitted. Tests:
  `tests/conformance/stmt.expand/constexpr-range-automatic{,-run,-neg}.cpp`
  (`skip(clang)`: clang 22 has no expansion statements and neither it nor
  GCC 16 implements P2686's automatic references).
- **Holding variables.** The holding variables of a `constexpr` structured
  binding declaration are `constexpr` (P2686 [dcl.struct.bind]/4), so the
  Oven sees initialized constants rather than a static variable's
  definition. User-written `constexpr` structured bindings are §4.9
  (P2686R5, WP-454).
- **Return type deduction.** A `return` in the pattern of an expansion
  statement of a non-template function does not deduce the placeholder
  return type (`in_template_outside_expansions`); its expansions do.
- **Control flow.** `break` leaves the whole statement and `continue`
  ends the current element; the statement counts as a loop for the
  jump-scope, fall-off and reachability analyses (`children` yields the
  init, prelude and expansions of an expanded statement and the init and
  body of a pattern).
- **Jumps in the header** (WP-1081). A GNU statement expression can put
  `break`, `continue`, `return` or `goto` in the init-statement or the
  expansion-initializer. clang 22 has no P1306, so the oracles are GCC
  16.1, clang's P1306 branch (`ParseForStatement`,
  `EmitCXXExpansionStmtInstantiation`) and its constant evaluator; they
  agree on everything below except where noted.
  - *Sema.* The parser opens the expansion statement as a loop
    (`act_on_start_loop` in `parse_expansion_rest`) for its body only, as
    for a range-based `for` (§7.4a). A header `break`/`continue` needs an
    enclosing loop, or a switch for `break`. An outer expansion statement
    is such a loop. Without one, it is "'break' statement not in loop or
    switch statement" / "'continue' statement not in loop statement" at
    the keyword. This is checked on the pattern, so a template definition
    is diagnosed. A lambda in the header starts a new function.
  - *Run time.* The init-statement, an iterating range and a
    destructuring initializer belong to the preamble, which runs once
    before the elements. Their `break`/`continue` reach the enclosing
    loop or switch. An enumerating element initializes the
    for-range-declaration inside its own element block
    (`{ decl = E_i; body }`). There `continue` ends that element and
    `break` leaves the expansion statement, although Sema required an
    enclosing target. Codegen (`kiln_codegen`), the CFG
    (`cfg_expansion`: init and prelude outside the per-element jump
    frames) and the Oven all follow that structure. `return` leaves the
    function and `goto` leaves to its label. Destructors run on every
    path: the init-statement's variable, and a destructuring
    initializer's temporary, when a jump leaves the preamble.
  - *CFG.* The init-statement's `break` leaving an enclosing `for(;;)`
    makes -Wreturn-type fire. An element's `break` does not. GCC warns
    the same way; clang's CFG does not model expansion statements.
  - *Constant evaluation.* A taken jump out of the statement expression
    is clang's "this use of statement expressions is not supported in a
    constant expression" at the statement holding it (§7.4a). An untaken
    jump, or a loop wholly inside the statement expression, evaluates
    normally. GCC 16.1 evaluates the jumps; kiln follows clang.
  - *Iterating range.* `__range` is `static constexpr` (P1306R5), so a
    statement-expression jump in the range is "constexpr variable
    '__range' must be initialized by a constant expression" plus the
    statement-expression note. GCC 16.1 accepts it with a non-constexpr
    range (CWG3131, which clang's branch does not implement).
  - Tests: `tests/conformance/stmt.expand/header-jump-{run,errors,cfg,consteval,consteval-neg}.cpp`.
- **N = 0.** No block is built, so a body that is ill-formed for every
  element is never instantiated.
- **Feature macro.** `__cpp_expansion_statements` 202506L at C++26 at
  every `-fclang-compat` (`KILN_FEATURES`; clang 22 does not define it).
- **Left.** `define_static_array` and reflection ranges (since WP-478
  `members_of` returns a `std::vector<info>`);
  P2686's constexpr-referenceable automatic objects (§4.9).
- Validation: `tests/conformance/stmt.expand/` (all `requires: clang>=23`),
  `tests/codegen/expansion_stmt.cpp`, `kiln_codegen` test
  `expansion_statement_lowers_in_order`.

### 7.7 Landed (WP-140): GNU asm statements (`asm.rs`)
`act_on_gcc_asm_stmt` follows clang's `ActOnGCCAsmStmt`: output and input
constraints are validated per target (`kiln_abi::asm`, aarch64 and
x86_64), outputs must be modifiable lvalues
(`invalid lvalue in asm output`), a register-only input is converted to a
prvalue (lvalue-to-rvalue) and a memory input stays an lvalue, an
immediate input (`i`, `n`) must be an integer constant expression, a
tied operand must have a compatible type, and the template is parsed
(`%N`, `%[name]`, `%=`, `%%`, modifiers) with clang's errors for an
invalid operand number, unknown name and a mismatched modifier (the
aarch64 `-Wasm-operand-widths` warning with its `%w` fix-it note).
Clobbers are checked against the target's register names. Each operand
ends a full-expression (its odr-uses are resolved). The `!srcloc`
values clang attaches are precomputed into `AsmData.srclocs` (literal
begin, then every later template line; offset + 2). Templates are
substituted (`subst/stmts.rs`) and rechecked at instantiation.
`act_on_file_scope_asm` builds a `FileScopeAsmDecl`. Not implemented:
immediate range checks
(`validateInputSize`/`validateOutputSize`), `=@cc` flag outputs, register
variables (`register int r asm("x0")`) as operands and their clobber
conflict check;
`!srcloc` values in headers and macro expansions do not match clang's
encoding.

**Deferred (WP-160 item 4): macro `!srcloc` parity.** For a string from a
macro expansion, clang emits `2^31 + offset`, where the offset is into
clang's unified source-location space. That space holds, in order: entry 0
(2 bytes), the main file (size + 1), the predefines buffer (20928 bytes on
aarch64 for C++26, which is the `-dM` text plus 4213 bytes of other
builtins), and then one entry of length + 1 per macro expansion. Matching
it needs two things:
- kiln_source keeping a clang-compatible offset counter across files.
- The same sequence of expansion entries as clang: argument
  pre-expansion runs, the 4060-byte scratch buffer, expansions inside
  `#if`, and included files.

That is an ABI-free, preprocessor-wide change with no effect on codegen.
The `!srcloc` value only labels inline-asm backend diagnostics, so it is
left for a pp/source WP. WP-182 re-evaluated this: kiln_source and
kiln_pp still have no clang-compatible offset space (expansions are
recorded per `ExpansionInfo`, not as a running offset), so the deferral
stands.

### 7.8 Landed (WP-140): predefined identifiers (`predefined.rs`)
`act_on_predefined_expr` is clang's `BuildPredefinedExpr`: the enclosing
function (or, outside one, the translation unit with
`-Wpredefined-identifier-outside-function`); a dependent context gives a
dependent `PredefinedExpr` that template instantiation rebuilds;
otherwise a `const char[N]` lvalue holding the name.
`compute_predefined_name(kind, f)` is clang 19's
`PredefinedExpr::ComputeName`: `__func__`/`__FUNCTION__` are the bare
name (`operator int`, `operator()`, `top level` is only pretty);
`__PRETTY_FUNCTION__` prints `virtual `/`static `, the return type
(`auto` for lambdas, none for constructors and destructors, the
instantiation pattern's otherwise), the scope with specialization
arguments (`A<int>::`), the pattern's parameters (`, ...` if variadic),
cv and ref qualifiers, and ` [T = int, ...]`: enclosing implicit class
specializations' arguments (outermost first) then the function template
specialization's, printed with their types (`2U`, `'a'`, packs
`<int, char *>`, `x:auto` for a generic lambda's invented parameter).
A type argument prints canonical with every name scoped, as clang stores
the converted arguments: `[T = (anonymous namespace)::N::S]`,
`[T = std::basic_string<char>]`, `[T = unsigned long]` for `size_t`
(LLVM's `getTypeName<T>()` reads these; WP-730).
The scope's specialization arguments print the same way (WP-740,
clang prints the canonical arguments it stores): `C<N::X>::f`, an
alias or a defaulted argument as what it names, an unnamed namespace
as `N::(anonymous namespace)::Z`, a closure type as `(lambda at
F:L:C)`, both in the scope and in the bracket (`C<(lambda at
F:L:C)>::f() [T = (lambda at F:L:C)]`). The closure that is itself the
scope of a lambda's call operator prints `(lambda)` at clang_compat >=
22 and `(anonymous class)` before (`auto f()::(lambda)::operator()()
const`).
The source-location builtins use the same printer (§9.5 of
sema-templates.md), and a `__builtin_FUNCTION`/`__builtin_source_location`
in a dependent context, or a default argument evaluating one used there,
is marked instantiation-dependent and recomputed at instantiation.
Tests: `tests/conformance/dcl.fct.def.general/predefined-*.cpp`,
`expr.const/builtins-srcloc-template-run.cpp`, `tests/codegen/predefined.cpp`.
Gap: a function with a deduced return type prints `auto` where clang
evaluates the source-location name after deduction.

### 7.9 `-Wempty-body` (`empty_body.rs`, WP-191)
clang 19's `DiagnoseEmptyStmtBody` / `DiagnoseEmptyLoopBody`. The body is
a null statement whose `;` is spelled on the presumed line of the
statement's `)` (for `switch`, of the condition's end). The warning
("if statement has empty body", "switch statement has empty body",
"range-based for loop has empty body", "for loop has empty body",
"while loop has empty body" [-Wempty-body]) and the note "put the
semicolon on a separate line to silence this warning" are both at the
`;`.
- The parser calls `check_empty_body` for an `if` without `else` (not
  `if consteval`; `if constexpr` counts) and a range-based `for`, and
  `check_empty_switch_body` for a `switch`, when the statement is built.
- A `for` or `while` is recorded (`note_loop_body`, in
  `Sema::empty_loop_bodies`). When its enclosing compound statement is
  finished, `check_empty_loop_bodies` warns if the next statement of
  that block is a compound statement or begins in a later presumed
  column than the loop. `do ; while (c);` never warns.
- A `;` preceded by a macro that expanded to nothing (`if (c) EMPTY;`)
  does not warn. clang reads `NullStmt::hasLeadingEmptyMacro`, a
  preprocessor token flag. kiln reads the source instead: anything other
  than whitespace and comments between the `)` and the `;`. A `;` that
  comes from a macro is spelled on another line and does not warn.
- Nothing is diagnosed in a template instantiation; a template definition
  warns once.
- **Deviation:** a block-scope declaration with leading attributes
  (`[[maybe_unused]] int q;`) begins at its decl-specifiers in kiln, so
  a loop before it may warn where clang (which begins at the `[[`) does
  not.
- Test: `tests/conformance/stmt.pre/empty-body-warning.cpp`.

### 7.10 `-Wunreachable-code` (`unreachable_code.rs`, WP-234)
clang 19's `CheckUnreachable` (`ReachableCode.cpp`) with Sema's
`UnreachableCodeHandler`, run with the other flow warnings at the end of
a function body, never in a dependent context, in an instantiation or
after an error (`uncompilable_error_count`, spec/diag.md §7.5).
- The scan runs on the body's CFG (§7.5.1; `Cfg::dead_scan`, clang
  `reachable_code::ScanReachableFromBlock`), from the entry and every
  catch dispatch block, element by element, over the edges clang's
  scan follows. A constant branch condition prunes its dead edge unless
  it is a configuration value (`is_configuration_value`: a `constexpr`
  call, a global or `const` local, `sizeof`, or a parenthesized or
  macro-expanded literal); the builder decides that once and marks the
  edge. An `ExprWithCleanups` is no element of its own
  (clang `VisitExprWithCleanups`): the temporaries' destruction after
  `throw E()` is not dead code
  (`expr.throw/unreachable-code-temporary.cpp`). The AST walk this
  replaced (`try_stmt/reach.rs`) is gone (WP-509).
- Each round reports one dead root: the last dead element, in
  evaluation order, that no reached or dead element flows into. It is
  then treated as reached and the scan repeats, so the regions come out
  last first, as clang scans its blocks in reverse. The cost is
  O(regions × body).
- A root that is a `break` is `'break' will never be executed`
  [-Wunreachable-code-break]; a `return`, or part of a returned value
  whose evaluation does not branch, is `'return' will never be executed`
  [-Wunreachable-code-return]; otherwise `code will never be executed`
  [-Wunreachable-code]. The warning is at clang's `GetUnreachableLoc`:
  an operator's token, a member's name, else where the element begins.
- Not reported: a root in a macro expansion, the literal or enumerator
  condition of `do { ... } while (0)`, or a reference to
  `__builtin_unreachable`.
- A dead `for` increment is `loop will run at most once (loop increment
  never executed)` [-Wunreachable-code-loop-increment] at the increment.
- A root whose block's first predecessor (reachable or not; a loop
  edge clang drops outright does not count) ends in a condition with a
  bare literal (`if`, `while`, `for`, `do`, `switch`, `?:`, the left
  operand of `&&`/`||`; clang `getTerminatorCondition`) has the note
  `silence by adding parentheses to mark code as explicitly dead`, with
  the `/* DISABLES CODE */ (` ... `)` fix-its around the literal. A root
  whose note repeats the previous root's literal is dropped.
- A dead `[[fallthrough]];` is reported by -Wunreachable-code-fallthrough
  (§ implicit fallthrough) instead when that is enabled.
- Groups: -Wunreachable-code = {-Wunreachable-code-fallthrough,
  -Wunreachable-code-loop-increment}; -Wunreachable-code-aggressive adds
  -break and -return. None is in -Wall.
- Tests: `tests/conformance/stmt.return/unreachable-code*.cpp`,
  `stmt.break/unreachable-code-break.cpp`,
  `stmt.if/unreachable-code-config.cpp`, `stmt.*/cfg-*.cpp` (the
  graphs, WP-509).

#### 7.10.1 Fix-its at macro locations (`macro_fixit.rs`, WP-419)

The silencing note and the `-Wconstant-logical-operand` notes (§5.25)
build their fix-its as clang does, as edits of the file:
- The end of a token (`Sema::file_token_end`, clang
  `Lexer::getLocForEndOfToken`) at a macro location exists only when the
  token ends every expansion it is in (a macro argument ends its
  expansion unless the argument continues in the next entry); it is then
  the end of the outermost invocation. Without it the silencing note is
  not emitted and the removal fix-it is not attached.
- `Sema::file_fixits` maps a note's fix-its (clang `edit::Commit`,
  `Lexer::makeFileCharRange`): an insertion at the start of every
  expansion it is in moves to the invocation, and a macro-argument
  location to where the argument is written; a range keeps a file end,
  maps a macro begin by the start rule and a macro end by the end rule,
  and a range lying in one macro argument maps to its spelling. When any
  fix-it of the note does not map, the note keeps its message and loses
  all of its fix-its (clang `mergeFixits`): `if (X 1)` with
  `#define X h() && !` gets the note without `/* DISABLES CODE */ (`.
- A token's end is measured from the source text (`Sema::token_end`):
  an identifier, a number or the longest punctuator ([lex.operators];
  `>>` and `>>=` included). A one-character `>` that the parser split
  off a `>>` or `>>=` closing a template argument list ([temp.names]/4;
  `Parser::split_greater` calls `Sema::note_split_greater`) measures
  one character, as clang measures the split token. Unit test:
  `impcast::tests::token_end_measures_shift_tokens_and_split_greaters`.
- Tests: `kiln_driver` `fixits_in_macro_expansions_like_clang` (stderr of
  clang 19 and 22), `expr.log.and/constant-operand-macro-fixits.cpp`.

### 7.11 `-Wfor-loop-analysis` (`for_loop_analysis.rs`, WP-234)
clang 19's `CheckForLoopConditionalStatement` and
`CheckForRedundantIteration`. `act_on_for` runs both as each `for` is
built, so a template definition is checked, and every instantiation is
checked again (the warnings are `allow_duplicate`, so each
instantiation repeats them, as clang does).
- **Condition** (not with a condition variable). The condition must be
  simple (clang `DeclExtractor`): literals (integer, floating, `bool`,
  character, `__null`, imaginary), casts, parentheses, built-in
  unary operators except `*`, binary and compound-assignment operators,
  `?:` and `?:` with an omitted middle operand, and names of variables
  or parameters. Unevaluated operands (`sizeof`, `noexcept`, an
  unevaluated `typeid`) are skipped. Anything else (a member, a call, an
  array subscript, an enumerator, `nullptr`, a template parameter) is
  not simple and nothing is checked.
- None of those variables may be `volatile` or lack automatic storage
  (a parameter has it; a `static`, `thread_local` or namespace-scope
  variable does not).
- The condition, the increment and the body are then searched (clang
  `DeclMatcher`, an evaluated-expression walk): any `return`, `break`
  or `goto`, or any use of one of the variables that is not read
  through an lvalue-to-rvalue conversion (an assignment, `++`, `&x`,
  binding a reference, a discarded-value `x;`), silences the warning.
  A read through `?:` arms counts as a read. Lambda bodies are not
  searched, only their capture initializers.
- Otherwise: `variable 'i' used in loop condition not modified in loop
  body`, `variables 'i' and 'n' ...`, up to four names, and `variables
  used ...` with five or more [-Wfor-loop-analysis] at the first name.
- **Redundant iteration.** The body is a braced, non-empty block whose
  last statement and the increment are both `++x`/`x++` (or both
  `--`), built-in or overloaded, applied directly to the same name (no
  parentheses). Unless the body has a `continue` of this loop (a nested
  `switch` is searched; nested loops other than their init-statements
  are not): `variable 'i' is incremented both in the loop header and in
  the loop body` [-Wfor-loop-analysis] at the body's name, with the note
  `incremented here` at the header's name.
- Groups: -Wloop-analysis = {-Wfor-loop-analysis,
  -Wrange-loop-analysis}; not in -Wall.
- Macros are not suppressed: the warning fires whether the `for`, its
  condition, increment or the body's last statement comes from a macro
  expansion, with the usual `expanded from macro` notes (§diag 8). A
  dependent loop warns once per instantiation, with the instantiation
  note (WP-419).
- Tests: `tests/conformance/stmt.for/for-loop-analysis.cpp`,
  `stmt.for/loop-analysis.cpp`, `stmt.for/for-loop-analysis-macros.cpp`.

### 7.12 `-Wbitfield-constant-conversion` (`bitfield_constant.rs`, WP-234)
clang 19's `AnalyzeBitFieldAssignment`. An integral or enumeration
constant (implicit casts and parentheses stripped, folded) stored into a
bit-field of width N that cannot hold it reports `implicit truncation
from T to bit-field changes value from V to W` [-Wbitfield-constant-
conversion, on by default; a subgroup of -Wconstant-conversion], where T
is the constant's type and W the value of its low N bits read with the
bit-field's signedness ([conv.integral]/3). Storing 1 into a one-bit
wide signed bit-field reports `... to a one-bit wide bit-field ...`
[-Wsingle-bit-bitfield-constant-conversion, a subgroup].
- Nothing is reported for a `bool` bit-field, a dependent or invalid
  bit-field, or a value-dependent or non-constant value. A value of 128
  or more bits (`__int128`, a wide `_BitInt`; WP-465) gets the same rule
  computed on its words (`wide_truncation`), with exact decimal values
  (`class.bit/bit-int-wide-bit-field.cpp`).
- The value needs the width of its type, except that a negated or
  complemented constant (`-5`, `~0u`), or any unsigned constant written
  that way, needs only its significant bits (`APInt::getSignificantBits`
  of the bit pattern), so `u2 = -1` and `u4 = ~0u` are silent.
- **Sites.** A simple assignment `b = v` (not a compound one), at the
  operator, from the implicit-conversion walk of the full-expression
  (`impcast.rs`); when it is reported the conversions of `v` are not
  analyzed further. The initialization of a bit-field member (clang
  `CheckBitFieldInitialization`): aggregate initialization (including
  designated and brace-elided elements, `new T{...}`) and parenthesized
  aggregate initialization, at the element; a mem-initializer, at the
  member name; a default member initializer, at the initializer (the
  `{` of a braced one). A braced scalar initializer `{v}` is reported at
  `v` with `v`'s type and again at the initialization's location with
  the bit-field's declared type (clang checks the element and then the
  `InitListExpr`; the significant-bits rule does not apply to the
  second).
- Each template instantiation is checked again (`allow_duplicate`).
- **Gaps.** A default member initializer of a class template
  specialization is instantiated with the class rather than at its
  first use, so its warnings carry `in instantiation of template class`
  instead of clang's default-member-initializer notes. The enumeration
  checks of `AnalyzeBitFieldAssignment` for a non-constant value
  (`-Wbitfield-enum-conversion`) and `-Wsigned-enum-bitfield` are not
  implemented.

### 7.13 Coroutines (`coroutine.rs`, WP-351)

Clang's `SemaCoroutine.cpp`, rule for rule; every error below has
clang's wording (the ids are the `sema_coroutine_*` family).

**[dcl.fct.def.coroutine]/1: what is a coroutine.** The first
`co_await`, `co_yield` or `co_return` of a function body makes the
function a coroutine (`FnSpec::is_coroutine`, a LATE bit) and records
the keyword for `function is a coroutine due to use of '<kw>' here`.
Context checks, in clang's order (`isValidCoroutineContext`):

- outside a function body (a namespace-scope or default-argument
  initializer, a class member initializer): `'<kw>' cannot be used
  outside a function`;
- in a constructor, a destructor, `main`, a `constexpr` or `consteval`
  function, a function with a deduced return type still undeduced, or
  a C-variadic function: `'<kw>' cannot be used in <a constructor|...>`.
  A lambda call operator that is constexpr only implicitly
  ([expr.prim.lambda.closure]/5) silently stops being constexpr;
- `co_await`/`co_yield` only ([expr.await]/2): in an unevaluated
  operand, `'<kw>' cannot be used in an unevaluated context`; inside a
  handler of a try-block (up to the function scope), `'<kw>' cannot be
  used in the handler of a try block`.
- A `return` statement in a coroutine is `return statement not allowed
  in coroutine; did you mean 'co_return'?` (reported at the body's end,
  with the coroutine note).

**The promise ([dcl.fct.def.coroutine]/4-5).** At the first keyword (not
in a template pattern) Sema declares the parameter copies, then the
promise variable `__promise` of type
`std::coroutine_traits<R, P...>::promise_type`. `R` is the declared
return type; `P...` are the parameter types, preceded by the implicit
object parameter's type (`cv C&`, or `cv C&&` with a `&&`
ref-qualifier) for a non-static member function, lambda call operators
included. Errors: `std::coroutine_traits` not found (`std::coroutine_
traits type was not found; include <coroutine> before defining a
coroutine`) or not a class template; an incomplete specialization
(`missing definition of specialization`); no member `promise_type`
(`this function cannot be a coroutine: 'std::coroutine_traits<R>' has
no member named 'promise_type'`); a promise type that is not a class or
is incomplete. A lookup that already failed with an error type (e.g.
`typename R::promise_type` inside the primary template) adds nothing.
The promise is direct-initialized from `*this` (a member function that
is not a lambda call operator) followed by the parameter copies when
that initialization is valid, else default-initialized
([dcl.fct.def.coroutine]/5.7, as clang).

**Parameter copies ([dcl.fct.def.coroutine]/13).** Each parameter of
non-reference type gets a local copy initialized from an xvalue of the
parameter (`std::move`); references are not copied. Uses of a parameter
in the body refer to the copy (codegen remaps them, spec/codegen.md
§7c).

**Suspend points ([dcl.fct.def.coroutine]/5).** At the body start Sema
builds the implicit `co_await __promise.initial_suspend()` and
`co_await __promise.final_suspend()` (implicit `CoawaitExpr`s located at
the function). `co_await __promise.final_suspend()` must not be
potentially-throwing ([dcl.fct.def.coroutine]/15): every called function
in it, and the destructor of every class-type result, must be
non-throwing (`__builtin_coro_resume` excepted); else `the expression
'co_await __promise.final_suspend()' is required to be non-throwing`
with one `must be declared with 'noexcept'` note per offending function,
at the end of its definition (the closing `}`) or, for a declaration
without body, at the end of its declarator (clang `getEndLoc`, WP-370):
the start of its last token, counting cv/ref-qualifiers, the exception
specification, a trailing return type or requires-clause, an asm label,
GNU attributes and the last virt-specifier, but not trailing C++11
attribute-specifiers (`ColdDeclData::declarator_end`, recorded by the
parser for bodiless member and namespace- or block-scope function
declarations, WP-381; an instantiated function uses its pattern's, or
the out-of-line definition its definition was instantiated from, WP-428).

**`co_await e` ([expr.await]/3-5).** In order:

1. `e` is transformed by `__promise.await_transform(e)` when the promise
   has any member named `await_transform` (a failed call adds `call to
   'await_transform' implicitly required by 'co_await' here`).
2. The awaiter is the result of the best `operator co_await` (member
   or non-member, found by ordinary unqualified lookup and ADL, through
   operator overload resolution), else the operand itself.
3. A prvalue awaiter is materialized; an lvalue `OpaqueValueExpr` stands
   for it in the three calls `await_ready()` (contextually converted to
   `bool`, its own full-expression), `await_suspend(h)` with `h` =
   `std::coroutine_handle<P>::from_address(__builtin_coro_frame())`, and
   `await_resume()` (the expression's type and value category).
4. `await_suspend` returning `void` or `bool` is used as is; returning a
   class (a `coroutine_handle<Z>`) the suspend expression becomes
   `await_suspend(h).address()` (symmetric transfer,
   `SuspendKind::Handle`); anything else is `return type of
   'await_suspend' is required to be 'void' or 'bool' (have <T>)`.

`co_yield e` ([expr.yield]/1) is `co_await __promise.yield_value(e)`
(steps 2-4, no `await_transform`); a braced-init-list operand is a
single argument. The whole expression is a `CoyieldExpr` with the same
parts.

**`co_return` ([stmt.return.coroutine]).** `co_return;` and `co_return
e;` with `e` of type `void` call `__promise.return_void()` (after
evaluating `e`); otherwise `__promise.return_value(e)` (a braced list is
the argument as written). An operand naming an implicitly movable
entity is tried as an xvalue first ([class.copy.elision]/3). A promise
declaring both `return_value` and `return_void` is `the coroutine
promise type '<P>' declares both 'return_value' and 'return_void'` with
two `member '<m>' first declared here` notes. Flowing off the end
([stmt.return.coroutine]/3) is an implicit `co_return;` when the promise
has `return_void`; otherwise `-Wreturn-type` warns `non-void coroutine
does not return a value` (`... in all control paths` when only some
paths fall off).
Immediate invocations in the operand are evaluated at the end of the
full-expression holding it ([expr.const]/16, WP-802): the promise
variable and the suspend points a first `co_return` builds are their
own full-expressions and do not take them (`coro_isolated`), and in a
template pattern the operand is finished as a full-expression, so an
instantiation keeps the non-dependent values (`co_return sq(3) + t;`,
`tests/conformance/stmt.return.coroutine/immediate-invocation-in-template-operand-run.cpp`).

**The body (`complete_coroutine_body`, clang
`CheckCompletedCoroutineBody`).** At the end of a non-template
coroutine's body Sema replaces the body with a `CoroutineBodyStmt`
(spec/ast.md §6) holding, as clang builds them:

- `get_return_object()`, either initializing the returned object
  directly (same type) or a `__coro_gro` variable converted at the
  first return to the caller; `get_return_object()` returning `void`
  for a non-void function is an error;
- `unhandled_exception()` (required with exceptions on: `<P> is required
  to declare the member 'unhandled_exception()'`), the implicit
  fallthrough `co_return;`;
- the frame allocation ([dcl.fct.def.coroutine]/9-11): the promise
  class's `operator new` is looked up first; when it exists the call
  tries the placement form `(size, [*this,] params...)` then `(size)`;
  otherwise the global `operator new(size_t)`. The promise's `operator
  new` templates take part in each try as in a new-expression, so
  `template <class A> operator new(size_t, allocator_arg_t, A)` serves
  `task f(allocator_arg_t, A)` (WP-789). When the promise declares
  `get_return_object_on_allocation_failure()` (which must be a static
  member function), the allocation must be non-throwing: a global or
  missing `operator new` becomes `::operator new(size_t, std::nothrow)`
  (errors: `std::nothrow was not found...`, `unable to find
  '::operator new(size_t, nothrow_t)'`, `<new> is required to have a
  non-throwing noexcept specification`), and the failure path returns
  `P::get_return_object_on_allocation_failure()`. A class `operator new`
  that fits neither form is `'operator new' provided by <P> is not
  usable with the function signature of '<f>'`;
- the deallocation: the promise class's usual `operator delete`, else
  the global one, the sized form `(ptr, size)` preferred. Both lookups
  compare parameter types without top-level cv ([dcl.fct]/5), so
  `operator new(const std::size_t)` and `operator delete(void *, const
  std::size_t)` are the plain and sized forms (WP-812,
  `dcl.fct.def.coroutine/promise-new-delete-const-size-run.cpp`);
- `-fcoro-aligned-allocation` (default off, as clang; P2014R0 option 2,
  not in the standard): the frame is allocated with its alignment. Each
  lookup above first tries the `std::align_val_t` form, so a promise's
  `operator new` is ranked `(size, align, params...)`, `(size, align)`,
  `(size, params...)`, `(size)`; the alignment argument is
  `static_cast<std::align_val_t>(__builtin_coro_align())`. When the
  promise only has unaligned forms the chosen one gets `warning: under
  -fcoro-aligned-allocation, the non-aligned allocation function for the
  promise type '<f>' has higher precedence than the global aligned
  allocation function` (`-Wcoro-non-aligned-allocation-function`) at its
  declaration; the nothrow form is `(size, align_val_t, nothrow_t)` (the
  error then names it). The aligned usual `operator delete` is preferred
  and gets the alignment when it takes one
  (`dcl.fct.def.coroutine/aligned_frame.cpp`,
  `aligned_allocation_diags.cpp`, `tests/codegen/coro_aligned_alloc.cpp`).
  Without the flag an over-aligned frame is allocated with the plain
  `operator new` and may be misaligned, as in clang 19/22 and GCC 14.

Any failure marks the function invalid and keeps the plain body; codegen
never sees a half-built coroutine.

**Templates.** In a template pattern Sema checks only the context; it
keeps `DependentCoawaitExpr` (operand only), a one-operand
`CoyieldExpr` and a `CoreturnStmt` without promise call. Instantiation
rebuilds each through `build_coawait`/`build_coyield`/`build_coreturn`,
which builds the coroutine as above in the instantiated function.
Coroutine lambdas follow the same rules (spec/sema-lambda.md).

### 7.14 Uninitialized locals (`uninit.rs`, WP-862)

A port of clang 19's `UninitializedValues.cpp` and of the reporting in
`AnalysisBasedWarnings.cpp` (`UninitValsDiagReporter`,
`DiagnoseUninitializedUse`, `DiagUninitUse`,
`SuggestInitializationFixit`); docs/design/uninitialized-values.md is
the design. Tests: `basic.indet/uninit-*.cpp`, and the fix-it golden
`uninit_fixits_like_clang` in `crates/kiln_driver/tests/driver.rs`.

**Groups.** `-Wuninitialized` (in `-Wall` and `-Wmost`) holds
`-Wsometimes-uninitialized`, `-Wuninitialized-const-reference` and
`-Wstatic-self-init`. `-Wconditional-uninitialized` is off by default.

**When it runs.** The analysis runs once per function body at the end
of the body ([`Sema::late_analysis`], after the unreachable-code scan,
over the body's CFG, §7.5.1), when all of these hold:
- no uncompilable error has occurred in the TU so far (clang
  `hasUncompilableErrorOccurred`). A warning promoted by `-Werror` does
  not count. Functions finished before the first error are still
  analyzed;
- the body is not in a dependent context. Each function template
  specialization is analyzed when it is instantiated, and its warnings
  carry the instantiation note;
- one of the four warnings is enabled at the function's location;
- the body declared an automatic variable without an initializer, or
  named a variable inside its own initializer (`note_uninit_var`). This
  is clang's "no tracked variable" fast path, tightened: C++ forbids
  jumping past an initialization, so a variable with an initializer can
  be uninitialized only through a self-reference.

**Tracked variables** (clang `isTrackedVar`). A non-implicit automatic
local, declared alone by a declaration statement of the body, of
scalar, vector or non-empty class type. A class is non-empty when some
field is not an unnamed bit-field, not zero-size and not of empty class
type; base classes do not count. Each variable has two bits per program
point, `Unknown`, `Initialized`, `Uninitialized` or `MayUninitialized`,
packed into dense `u64` rows, one row per block.

**Classification** (clang `ClassifyRefs`). Each reference to a tracked
variable is classified:
- `Use`: an lvalue-to-rvalue read (including a by-copy lambda
  capture), `++`/`--`, a compound assignment, or the argument of a
  `std::move` call.
- `SelfInit`: `T x = x;`. It is treated as an initialization. The
  self-init itself is reported only when `x` is also used while always
  uninitialized.
- `ConstRefUse`: a const glvalue argument of a call. It becomes
  `Ignore` when the callee has a trivial (empty) body.
- `Ignore`: the operand of a `(void)` cast, the left operand of a comma,
  or a `&x` argument for a pointer-to-const parameter.
- `Init`: any other reference, which may initialize the variable. This
  covers the left operand of `=`, an address taken, a non-const
  reference bound, a by-reference capture and an asm output.

Unevaluated operands are not in the CFG, so they are never seen.

**Fixed point.** A worklist runs in reverse post-order of the live
edges. The entry state is all `Uninitialized`. A block's input is the
merge of its analyzed predecessors' outputs. A declaration without an
initializer sets `Uninitialized`, even on a later loop iteration.
An `Init` sets `Initialized`. A
call to a `returns_twice` function (`setjmp`) makes every
`Uninitialized` variable `MayUninitialized`. An `asm goto`'s
`Uninitialized` outputs become `MayUninitialized` on its edges. The
blocks that saw a use are then run once more, in clang's block order,
to collect reports.

**Reports.** For each variable, uses are recorded through the
reporting pass, then flushed in variable-declaration order (clang's
reporter). The kind of each use is computed by `getUninitUse`:
- `Always`, `AfterDecl` or `AfterCall` for `Uninitialized`.
- `Sometimes` when one predecessor branch of the use decides it. The
  branches are found by searching backwards from the use for blocks
  from which the use is reachable while one successor leaves it.
- `Maybe` otherwise.

Each use produces one of these warnings:
- `variable 'x' is uninitialized when used here` [-Wuninitialized]. The
  `AfterDecl` wording is `... when used within its own initialization`.
  A self-reference inside the initializer (clang `ContainsReference`,
  which also enters statement-expression bodies) is reported with the
  name and use ranges.
- `variable 'x' is used uninitialized whenever <branch>` [-Wsometimes-
  uninitialized], for each deciding branch. The branch is one of:
  `'if' condition is true|false`, `'for'|'while' loop is entered|exits
  because its condition is false`, `'?:' condition is true|false`,
  `'&&'|'||' condition is true|false`, `switch case is taken`, `switch
  default is taken`, or `function 'f' is called` (an always-uninit use
  under `AfterCall`). Each warning has the note `uninitialized use
  occurs here`, plus `remove the 'if' if its condition is always
  true|false` (with the clang fix-its) or `remove the condition if it is
  always true|false` (replacing the condition with `true`/`false`).
- `variable 'x' may be uninitialized when used here`
  [-Wconditional-uninitialized].

The last warning for a variable carries the note `initialize the
variable 'x' to silence this warning`. It has a fix-it after the
declarator: ` = 0`, ` = 0.0`, ` = false`, ` = '\0'`, ` = nullptr` (a
pointer or member pointer), or none for a vector. It is placed after
the declarator's last token, so after `)` for a function pointer, and
dropped in a macro.

The uses of each variable are ordered most confident first, then by
location. The first use that produces a warning is reported, and the
rest are dropped (clang reports one point per variable).

A self-initialized variable (`SelfInit`) is handled differently:
- if one of its uses is always-uninitialized, the self-init itself is
  reported instead;
- otherwise every use is downgraded to `Maybe`.

`variable 'x' is uninitialized when passed as a const reference
argument here` [-Wuninitialized-const-reference] is reported once per
variable, for a `ConstRefUse` reached `Uninitialized`.

**Suppressions.**
- `__attribute__((uninitialized))` and `[[indeterminate]]` (P2795) do
  not silence the warning, matching clang 19 and 22. clang 19 also warns
  that `indeterminate` is an unknown attribute.
- A class variable whose declaration runs a non-trivial constructor is
  `Initialized` by it. The syntactic self-reference checker
  (`self_ref.rs`) reports class-type self-references (`A a(a.get())`).
  The analysis does not report them a second time, because
  `ContainsReference` sees the same `DeclRefExpr` and reports it once.

**Gaps.**
- clang 21's `-Wuninitialized-const-pointer` (a `const T*` argument
  `&x`) is not implemented, so clang 22 warns there and kiln does not.
- Field uses in constructor initializers (`S() : x(std::move(x))`) belong
  to clang's `DiagnoseUninitializedFields`, not to this analysis.

## 8. Constant evaluation (`oven_bridge.rs`)

Sema does no constant evaluation of its own: every constant it needs
comes from the Oven (`kiln_oven`, spec/oven.md; "Integration" there
covers the Oven's side). `constfold.rs` keeps only integer helpers
(`int_info`, `wrap`, `fits`). The M2 `Folder`, its tree-walking `Eval`
interpreter and `aggval` were removed by WP-054.

### 8.1 One Oven per translation unit
- `Sema` owns one `Oven`, in `Sema.oven: Option<Box<Oven>>`. Compiled
  bytecode, resolved globals and the memo cache are shared by every
  evaluation in the TU. The Oven is boxed so that taking it out and
  putting it back moves a pointer, not the evaluator (WP-645).
- `oven_bridge.rs` implements `SemaBridge` for the duration of one call.
  `with_oven` takes the boxed Oven out of its `Option` and hands the
  call a `Bridge` that holds `&mut Sema`:
  - `ensure_body(f, oven)` returns the body of the function's
    definition. Since WP-070b, a never-marked specialization or member
    of a class specialization is instantiated first, during a
    manifestly constant-evaluated evaluation only. Sema lends the
    Oven back by swapping it into `Sema.oven` while it instantiates, so
    nested evaluations run on the same Oven (R-OVEN-1;
    spec/sema-templates.md §4.11);
  - `ensure_layout` uses `kiln_abi::ensure_layout`;
  - `diag` emits into the engine (for `-Wenum-constexpr-conversion`).
- `set_constexpr_limits(Budget, backtrace_limit)` applies the driver's
  `-fconstexpr-*` flags, reaching Sema through
  `kiln_parse::ConstexprLimits`.
- `oven()` and `into_parts() -> (Ast, Oven)` hand the Oven's statistics
  to the driver for `-fconstexpr-report`.

### 8.2 Entry points
| method | Oven call | use |
|---|---|---|
| `eval_const(e, EvalContext)` | `evaluate` | manifestly constant-evaluated contexts; recorded in the stats and the report; failures carry notes |
| `eval_int(e, ctx)` | `evaluate` | the same, for an integral or `bool` result |
| `try_fold(e)` / `fold_int(e)` | `try_fold` | silent, unrecorded speculative folds for warnings |
| `is_constant_initialized(var)` | `is_constant_initialized` | [basic.start.static]/2, usable-in-constant-expressions checks |
| `const_int(v)` / `const_truth(v)` | (reads `ast.consts`) | the integer or truth value of a scalar constant |

### 8.3 Evaluation sites
Each site picks the `EvalContext` that clang 19 uses there. The context
sets the report `kind` and whether an out-of-range conversion to an
unscoped enumeration is reported (spec/oven.md "Integration").

| construct | context |
|---|---|
| `static_assert` condition | `EvalKind::StaticAssert` |
| initializer of a `constexpr` variable | `EvalContext::init(var, loc)`; on success the variable becomes `VarEvalState::Value(id)` |
| `case` label, enumerator of an enumeration with a fixed underlying type, `noexcept(e)` specifier | `EvalContext::converted(loc)`; a converted constant expression ([expr.const]/12) |
| `if constexpr` condition | `EvalKind::IfConstexpr` |
| array bound, bit-field width, enumerator without a fixed type, `alignas`, array designator | `EvalKind::Other` |

- **GNU folding** (`Sema::fold_not_ice`, clang
  `VerifyIntegerConstantExpression` with `AllowFold`, WP-230): for a
  bit-field width, an enumerator without a fixed underlying type and an
  array designator, an expression that is not an integral constant
  expression but folds laxly (`fold_rvalue_int`: a `const` non-volatile
  variable reads as its initializer, e.g. `(int)d` for `const double d =
  2.0`) is accepted with its folded value. It warns `expression is not an
  integral constant expression; folding it to a constant is a GNU
  extension` [-Wgnu-folding-constant] (an Extension, so off unless
  `-pedantic` or the group is enabled), with the evaluator's notes, at
  the expression's begin (a lone "subexpression not valid" note moves
  the location instead). Array bounds have their own folding (`variable
  length array folded to constant array`, above). A `case` label,
  `static_assert` and `alignas` never fold.

- **Speculative folds (`try_fold`):**
  - narrowing checks in list-initialization and fixed-type enumerators
  - division by zero and shift-amount warnings
  - the `static_assert` failed-comparison explanation (`'5 == 6'`)
  - constant `if`/loop conditions in the falls-off-the-end analysis
    (`-Wreturn-type`)
  - the alignment of an `alignas` expression for layout

  A speculative fold of an integer expression made of literals,
  enumerators, `sizeof`/`alignof`, integral casts and integer operators is
  answered by the Oven's trial-fold fast path (spec/oven.md, WP-656)
  without compiling or running bytecode; its result is the full
  evaluation's, so which folds sema asks for, and what it does with the
  answers, is unchanged.
- **Usable in constant expressions** ([expr.const]/4) means:
  - a `constexpr` variable, or
  - a `const`, non-`volatile` integral or enumeration variable,
  - whose initializer is a constant initializer
    (`is_constant_initialized`).

  A reference is usable when it binds to a static lvalue.
- **Constant initialization warning** (WP-415, clang
  `Sema::CheckCompleteVariableDeclaration`): the initializer of a variable
  that is neither `constexpr` nor `constinit` but has static or thread
  storage duration, is a reference, or is a `const` non-`volatile`
  integral or enumeration variable (locals included) is evaluated as a
  constant initializer, which is manifestly constant-evaluated. When it
  calls `__builtin_is_constant_evaluated()` or `std::is_constant_evaluated()`
  directly, `-Wconstant-evaluated` ("... will always evaluate to 'true' in
  a manifestly constant-evaluated expression") is reported from that
  evaluation even when it then fails (`int g = __builtin_is_constant_evaluated()
  ? x : 7;`); the variable is then initialized dynamically or by
  codegen's re-fold (spec/codegen.md). A non-const local, a const
  floating local and a `volatile` variable do not warn. Sema runs it
  only when a bounded walk of the initializer finds such a direct call
  (`oven_bridge.rs::warn_constant_evaluated_init`, `Oven::warn_constant_evaluated`).
  Same wording at `-fclang-compat=19`. Deviation: a variable template's
  instantiations warn once (oven-bridge diagnostics carry no
  instantiation context), clang once per instantiation. Validation:
  `tests/conformance/expr.const/is-constant-evaluated-failed-constant-init.cpp`.

  From C++23 on (P2280, a DR; WP-357) a reference that is not usable
  (a parameter named outside its call, an `extern` reference, one with a
  non-constant initializer) may still be *named* in a constant
  expression: the Oven binds it to an object of unknown value and
  diagnoses only accesses of that object, its dynamic type, address
  comparisons and a constant result referring to it (spec/oven.md
  "Unknown references"). Sema passes the language mode through
  `SemaBridge::cxx_year`.
- **Exceptions** (C++26, P3068; WP-357): a throw-expression and
  try-blocks are evaluated by the Oven (spec/oven.md "Exceptions"). An
  exception leaving a non-throwing function ends the evaluation; the
  Oven asks `SemaBridge::is_nothrow(f)`, which answers from the resolved
  exception specification (`Sema::known_nothrow`) and otherwise resolves
  a deferred or uninstantiated one first (`Sema::resolve_nothrow`, at the
  evaluation's location, lending the Oven for nested evaluation of a
  `noexcept(expr)`).
- **Failures:**
  - The primary diagnostic is sema's, at the site, as clang reports it
    (for example `static assertion expression is not an integral
    constant expression`).
  - The `EvalFailure`'s notes follow it (`with_failure_notes`): the
    reason, the `in call to` backtrace, and any `declared here` notes.
  - An expression that already contains errors is not evaluated.
  - A failed evaluation whose expression names (directly or through the
    initializers of the variables it names, a bounded walk) a variable
    that is invalid is silent: a `static_assert` fails without a
    diagnostic, a `constexpr` variable's initializer is not reported,
    and nor is a non-type template argument (`C<v>`, templ/args.rs;
    `temp.arg.nontype/invalid-variable-specialization-argument.cpp`, WP-446)
    (`Sema::refers_to_invalid_var`; clang marks the `DeclRefExpr`
    error-dependent). This is what keeps one bad variable-template
    instantiation from cascading (`temp.inst/var-spec-invalid-init-silent.cpp`,
    WP-418).

## 8a. Reflection (`reflect.rs`, `meta/`; `-freflection`)
Specified in **`spec/reflection.md`**:
- the value type and the `^^` operand (§2);
- intrinsic recognition and the per-family sema dispatch in `meta/` (§3);
- the consteval boundary (§4);
- splices (§5);
- `define_aggregate` and the `__kiln_define_member` rules (§6);
- annotations, including `type_of` of one (§7; WP-571);
- the error model (§8).

Everything is gated on `Sema::reflection` (driver `-freflection`).

## 8b. Snapshots (`snap.rs`)
`Sema` is a configured root of the snapshot encoding (spec/snap.md
"Hand-written roots"; WP-1382). Its fields are listed once, in
`sema_fields!`, in exactly one class each; `SNAPSHOT_FIELDS` exports the
table and the module docs give each non-state field its reason.

- **Classification rule.** *State* (written) is the default when in
  doubt: anything that accumulates or lingers after its construct ends
  (end-of-TU lists, "already warned" tables, pragma stacks, memos whose
  hits suppress a diagnostic, counters that number things).
  *Configuration* (32 fields: the target, `-std=`, the language switches
  `kiln_parse`'s `configure` sets) is not written; the consumer sets it
  and WP-1383's fingerprint covers it. `inst.limits` and
  `inst.report.format` inside the state `inst`, and the thread's stack
  guard `inst.guard`, are kept from the consumer's `Sema` on restore.
  *Transient* fields (construct stacks, per-call flags) are not written
  and must be at rest. *Scratch* buffers and *rebuilt* caches
  (`spare_oven`, `classify_memo`) are not written. The diagnostics engine
  is a *reference*; the AST and the Oven are *owned roots*, written by
  their crates' roots (the Oven is a TEMPORARY no-op until WP-1381).
- **Snapshot point.** The end of a prefix's main file, at top level,
  before the end-of-translation-unit work (pending instantiations,
  `act_on_end_of_translation_unit`): `kiln_parse::parse_prefix`.
- **Quiescence.** `Sema::snapshot_ready` refuses, naming the field: a
  declaration context other than the translation unit; more than the
  translation unit's scope; any transient field not as a fresh `Sema`
  has it; an Oven on loan; a running instantiation, an open template
  head or pending immediate invocations; declaration-image or carried
  recording on. An open `#pragma pack(push)` is state, not a refusal: the
  main file packs with it, as it would after an `#include`.
- **Write and restore.** `snap_put` writes the AST, the Oven, then each
  state field. `snap_restore` applies a snapshot to a fresh `Sema` built
  by `kiln_parse::new_sema` from the consumer's options; the decoded
  AST must share the consumer's interner (read earlier in the same
  stream, so its `Arc` is a back-reference), else the restore is
  refused. Errors are `SnapError`s, never panics. `snap_sizes` reports
  the encoded size of each part.

## 9. Target (`target.rs`)
- `pub use kiln_abi::{Arch, Target}`.
- `target_for_types` maps `TargetTypes` to a layout target.
- `target_of(ast)` recovers the target an AST was built for.
- Layout is `kiln_abi`'s Itanium implementation (spec/abi.md).

## 10. To be implemented (M3+)
- **Classes:** special members and construction (WP-067), virtual
  functions and virtual bases (WP-078); members, `this`, access control
  and non-virtual bases landed in WP-066b. Design:
  [sema-class.md](sema-class.md).
- **Overloading:** overload resolution beyond arity, user-defined
  conversions, operator overloading.
- **Templates:** templates, concepts, packs; `auto` and `decltype`
  beyond the M2 basics. Design: spec/sema-templates.md (the type-trait
  list, including the clang 22 traits of WP-263, is its §9.1).
- **Other language features:** lambdas, reflection, contracts.
  (Exceptions landed with WP-100: §5.5, §7.5; coroutines with WP-351:
  §7.13.)
- **WP-064 follow-ups:**
  - prototype scope (§4.3)
  - `dynamic_cast` and polymorphic `typeid`
  - class and array temporaries (§6.1) and compound literals (§5.8)
  - statement expressions: `-Wgnu-statement-expression`, jumps into
    one, and class-glvalue results
- **Constant evaluation** (what the Oven supports in M2 is in
  spec/oven.md):
  - the discarded statement of `if constexpr` ([stmt.if]/2), with
    templates
  - template arguments (`EvalKind::TemplateArg`)
  - constant-initialization of `constinit` variables
  - constant-initialization checks of non-`constexpr` variables with
    static storage and of local `const` integral variables
    ([basic.start.static]/2): codegen folds them with
    `fold_constant_init` (spec/oven.md), but Sema does not evaluate
    them, so `-Wconstant-evaluated` is not reported in their
    initializers as clang reports it
  - Sema still rejects some M2 constructs before evaluation: a
    reference bound to a temporary, a cast to a reference type,
    list-initialization of a temporary, and initialization of a class
    object from anything but a braced list
- **Rules not checked yet:**
  - jumps past initialization ([stmt.dcl]/3)
  - full [basic.def.odr] for inline functions and variables across
    redeclarations
- **Using-declarations** (§4.8):
  - using-declarators at class scope that name base-class members
    (needs base classes), and their `typename`, `operator` and pack
    (`...`) forms, which are diagnosed as unsupported
  - overload sets through using-declarations beyond what M2 overloading
    supports
  - conflicts between a shadow and an enumerator of the same scope
  - `previousDecl` links for redeclared shadows and aliases in the dump
  - an attributed `[[...]] using` declaration as a block-scope statement

## 11. Tests
- `crates/kiln_sema/tests/`: per-rule tests that drive `Sema`
  directly, for example `constfold.rs`.
- `tests/conformance/expr.const/`: constant evaluation through the
  driver (WP-054), including `oven:` budgets.
- `crates/kiln_parse/tests/`: rules that are easier to state in
  source, including `sema.rs`.
- `diag:` conformance tests under `tests/conformance/`, which also
  pass with `--compiler clang++-19`.
- AST goldens under `tests/conformance/ast/`.

## 12. Open questions
- **Additive `kiln_ast` APIs added by WP-048:**
  - `label_stmt`/`goto_stmt`
  - `set_non_odr_use`, `replace_enumerator_init`, `mark_bitfield`
  - `typedef_for_anon`/`set_typedef_for_anon`
  - `mark_part_of_explicit_cast`
  - `ExprArena::update_string_init_type`

  These should be reviewed for the AST spec.
- **Orphaned cast nodes:** `cast_to` merges a same-kind cast into the
  existing node and orphans the node it replaced. The arena keeps
  those orphaned nodes.
- **`parse_file` location:** `parse_file` lives in kiln_parse. It
  could instead live in a `kiln_driver` crate once kiln_pp feeds the
  parser.
- **Where to fold constants:** constant-expression warnings (division
  by zero, shifts) fold eagerly in Sema. With the Oven, they could be
  computed lazily instead.
