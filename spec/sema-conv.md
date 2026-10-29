# spec: kiln_sema conversions and literal typing

Status: **implemented** (WP-049, M2). Normative for `kiln_sema::conv`
and `kiln_sema::literal`. §9 lists where kiln deliberately differs from
clang 19, and that list takes precedence.

Standard: [conv] (all subclauses), [expr.arith.conv], [over.ics.scs],
[over.ics.rank]/3.2 and /4, [dcl.init.list]/7, [dcl.init.ref]/4–5,
[expr.assign]/3, [stmt.return]/2, [lex.icon], [lex.fcon], [lex.ccon],
[lex.string].
Reference: `clang++-19 --target=x86_64-linux-gnu -std=c++26
-fsyntax-only -Xclang -ast-dump`. The `ImplicitCastExpr` chain clang
builds is the contract: same kinds, same order, same target types (sugar
included) and same value categories.

## 1. Context: `Sema`
`Sema<'a, 'sm> { ast: Ast, diags: &DiagEngine, target: TargetTypes }` is a
minimal shim that WP-048 extends. Every function in this spec takes
`&Sema` or `&mut Sema`. Helpers:
- `diag(id, loc) -> Diag` and `emit(Diag)`.
- `ty_arg(q)` gives the quoted type as clang prints it in diagnostics,
  `'T'` or `'T' (aka 'U')` when sugar hides the canonical type.
  **Diagnostic type arguments are pre-quoted strings**, so the
  `kiln_diag` templates never add quotes themselves.
- `ty_str(q)` gives the same type without quotes.

`names::SemaNames` gives the type printer access to declaration names.

## 2. Operand conversions (`conv/mod.rs`)
| fn | rule | clang |
|---|---|---|
| `lvalue_to_rvalue(e)` | [conv.lval]: a glvalue of non-class, non-array, non-function `cv T` becomes a prvalue `T` (cv stripped while keeping sugar, `strip_cv`) | `LValueToRValue`; `NullToPointer` for a `std::nullptr_t` glvalue |
| `decay(e)` | [conv.array], [conv.func], else lvalue-to-rvalue | `DefaultFunctionArrayLvalueConversion` |
| `integral_promotion(e)` | [conv.prom] for a prvalue | `IntegralCast` to the promoted type |
| `usual_unary_conversions(e)` | decay, then [conv.prom] / bit-field promotion. **No** [conv.fpprom] (clang promotes `float` only for variadic arguments) | `UsualUnaryConversions` |
| `bit_field_width(e)` | the width of the bit-field `e` designates, looking through parens, member access and casts | |
| `is_null_pointer_constant(e)` | [conv.ptr]/1: an integer literal 0 (through parens and casts), or a `std::nullptr_t` prvalue | |

`promoted_type(q, bit_field)` ([conv.prom]/1–7) handles these cases:
- types of rank below `int`;
- `wchar_t`, `char8_t`, `char16_t` and `char32_t`, which promote to the first of int/uint/long/ulong/llong/ullong that holds them (aarch64's unsigned 32-bit `wchar_t` becomes `unsigned int`);
- an unscoped enum without a fixed type, which uses the same list over its value range;
- an enum with a fixed underlying type, which promotes to that type, as recorded on `EnumData`;
- bit-fields, which promote to `int` when it holds every value, else `unsigned int`, else nothing ([conv.prom]/5);
- `bool`, which promotes to `int`.

Scoped enums never promote.

## 3. Standard conversion sequences (`conv/scs.rs`, `apply.rs`)
`standard_conversion(s, e, to, ctx) -> Option<StandardConversion>` computes
the sequence ([over.ics.scs]):
- `first: FirstStep`: Identity, LvalueToRvalue, ArrayToPointer or FunctionToPointer;
- `second: SecondStep`: Identity, IntegralPromotion, FloatingPromotion, IntegralConversion, FloatingConversion, FloatingIntegral, PointerConversion, MemberPointerConversion, BooleanConversion or NullPointer;
- `fn_ptr: bool`;
- `third: ThirdStep`: Identity or Qualification;
- the `rank()`: Exact, Promotion or Conversion;
- the data the ranking rules need.

`standard_conversion_from` does the same from a type and category, with no
expression. `ctx` matters only for `nullptr_t → bool`, which is allowed
only when `ctx` is a direct-initialization ([conv.bool]).

`apply_standard_conversion(e, scs, to)` builds the casts. Cast targets:
1. The first step casts to the decayed or unqualified type.
2. The second step casts to `to` **as written**, sugar kept.
3. The qualification step adds a `NoOp` to `to` without top-level cv, and
   only when that is a different type from the previous one.
4. Identity steps add nothing.

The kinds produced are:
- integral and floating-point: `IntegralCast`, `FloatingCast`, `IntegralToFloating`, `FloatingToIntegral`, `IntegralToBoolean`, `FloatingToBoolean`;
- pointers: `PointerToBoolean`, `MemberPointerToBoolean`, `NullToPointer`, `NullToMemberPointer`, `BitCast` (object pointer to `void*`);
- classes and members: `DerivedToBase` (path from `path_list`), `BaseToDerivedMemberPointer` (path);
- other: `NoOp`.

A `DerivedToBase` cast's path lists the base-specifier indices, as
`cast_path` stores them.

Qualification ([conv.qual]) is `qualification_relation(from, to) ->
QualRelation`:
- `Same`;
- `NotSimilar`;
- `Similar { convertible, drops_outer, drops_nested }`.

It implements the cv-decomposition rules, including P0388 array-of-unknown-bound unification. An array's cv is the union of its element's cv (`effective_quals`). The function pointer conversion ([conv.fctptr]) is `is_function_pointer_conversion`.

Base lookup (`conv/bases.rs`): `find_base(derived, base) -> BaseSearch {
NotDerived, Unique(BasePath), Ambiguous(Vec<BasePath>) }` searches the
complete base graph depth first, with a bounded visit count. Two paths to
the same virtual base count as one. Access is **not** checked (§10).

## 4. Ranking (`compare_standard_conversions`)
`compare_standard_conversions(a, b) -> IcsCompare {Better, Worse,
Indistinguishable}` implements these rules, in order:
- [over.ics.rank]/3.2.1: proper subsequence, with the lvalue transformation ignored;
- /3.2.2: rank;
- /3.2.5: same type except top-level cv, where the more cv-qualified one is worse (qualification-adjusted similarity);
- /4.1: a conversion that does not convert a pointer, member pointer or `nullptr_t` to `bool` beats one that does;
- /4.2: promotion to a fixed enum's underlying type beats promotion to its promoted type;
- /4.3: for floating-point conversions between types of equal rank, the one whose target matches the source's conversion rank is better;
- /4.4: the derived/base pointer, member-pointer and reference rules.

`float_order(a, b) -> FloatOrder {Less, Equal, Greater, Unordered}`
compares floating-point conversion ranks ([conv.rank]/2) through the
target's formats (`format_subset`). bf16 and f16 are unordered.

## 5. Implicit conversion (`implicit_convert`)
`implicit_convert(s, e, to, ctx) -> Result<ExprId, ConvError>`:
- Error and dependent types pass through unchanged.
- A reference target binds directly to a reference-compatible glvalue ([dcl.init.ref]/5.1, 5.3.1, the xvalue case). The result is `NoOp` or `DerivedToBase` (with path) to `cv T`, keeping the category.
- A class target is the identity for a same-type prvalue; any other case returns `sema_conv_unsupported` (M3).
- A class source returns "no viable conversion" if the class has no conversion functions, else `sema_conv_unsupported`.
- Otherwise the standard conversion sequence, or a `ConvError` worded for `ctx`.

`ConvCtx` values:
- `Initialization` and `DirectInitialization`;
- `Assignment`;
- `Argument`;
- `Return`;
- `Comparison`;
- `Condition`, which is contextual bool and counts as direct ([conv.general]/4);
- `Cast`.

`ConvError { id, args }` is emitted by the caller with
`err.diag(s, e)`, which puts the caret at `e`.

Wording table (all verified against clang 19; tests in `conv_errors.rs`):
- **Initialization, Argument, Return:** `sema_init_conversion_failed`, "cannot initialize {a variable|a parameter|return object|a value} of type T with an {rvalue|lvalue} of type U". An xvalue prints as "rvalue", which covers the implicitly moved `return` operand. The caller marks that operand as an xvalue ([class.copy.elision]/3).
- **Class source:** `sema_no_viable_conversion` / `_return`.
- **Assignment** ([expr.assign]/3):
  - int↔pointer cases, with fix-it text: "take the address with &", "dereference with *", "remove *", "remove &";
  - discards qualifiers (outer or nested);
  - incompatible pointer types;
  - incompatible type.
- **Condition:** `sema_not_contextually_convertible_to_bool`.
- **References:**
  - "{non-const|volatile} lvalue reference … cannot bind to a temporary / a value of unrelated type". The select is `volatile` iff the referee is volatile.
  - "rvalue reference … cannot bind to lvalue".
  - "binding reference of type T to value of type U drops 'cv' qualifier[s]". T is the referee, and the qualifiers are quoted and space-joined.
- **Ambiguous base:** the message lists each path, one per line, as `\n    struct D -> B -> A`. The member-pointer conversions use the same format, plus the via-virtual-base error.

`contextually_convert_to_bool(e)` is `implicit_convert(e, bool,
Condition)`.

## 6. Usual arithmetic conversions (`conv/arith.rs`)
`usual_arithmetic_conversions_kind(s, l, r, kind) -> Result<(l', r', T),
ConvError>`. `usual_arithmetic_conversions` is the same with
`ArithConvKind::Arithmetic`.

`ArithConvKind` (Arithmetic, Bitwise, Comparison, Conditional,
CompoundAssign) selects only the diagnostic wording and the
compound-assignment shape.

**Compound assignment** ([expr.ass]/6: `E1 op= E2` computes `E1 op E2`
with `E1` evaluated once). The LHS is returned unconverted, but the
common type (the `ComputeLHSTy`) is computed from its promoted type, in
clang's order:
1. the RHS gets the usual unary conversions; if the cv-unqualified LHS
   type and the RHS type are the same, that type is the result
   (`unsigned c : 9; c += 2u` computes in `unsigned int`, and
   `short s; s += (short)1` in `int`, since the RHS was promoted);
2. otherwise the LHS type is replaced by its integral promotion
   (`promoted_type(lt, bit_field_width(l))`: [conv.prom]/1–4, and /5 for
   a bit-field, so `c += 2` computes in `int`, `long l3 : 3; l3 += 1` in
   `int`, `char32_t x; x += 1` in `unsigned int`; a bit-field wider than
   `int` keeps its type, and an enumeration bit-field promotes as its
   enumeration). An unscoped enumeration LHS thereby counts as
   arithmetic;
3. the rules below continue with the promoted LHS type.

The computation result must convert back to the LHS type. For an
unscoped enumeration LHS it never does ([conv.prom]/3 made it an integer
or the RHS made it floating), so `compound_assign` in `expr.rs` reports
clang's `assigning to 'E' from incompatible type 'int'`
(`sema_assign_incompatible`), for the shift operators too.

The rules, per [expr.arith.conv]/1:
- /1.1: a scoped enum mixed with another type is an error.
- /1.2 (C++26): enum with a different enum, and enum with a floating type, are errors (`sema_arith_enum_enum`, `sema_arith_enum_float`).
- /1.3: the floating-point rules, using `float_order`. Unordered types are an error (`sema_arith_float_unordered`).
- /1.4.x: integral promotion, then the rank and sign rules (1.4.1–1.4.5).

Non-arithmetic operands produce `sema_invalid_operands`. These errors are
**returned, not emitted**. Both operands get `decay` first.

## 7. Narrowing (`conv/narrow.rs`)
`narrowing_kind(s, scs, init, value: Option<ConstValueId>) -> Narrowing
{NotNarrowing, Type, Variable, Constant { value }, Dependent}` implements
[dcl.init.list]/7.1–7.5 for the standard conversion `scs` of `init`.
The source's value is the caller-supplied `value`, else a literal folded
here (integer, floating and bool literals, through negation, parens and casts).

The checks:
- Integer to integer checks range against the target width, sign and enum underlying type.
- Floating to integer is always narrowing.
- Integer to floating checks that the value round-trips exactly. This uses the soft-float in `conv/soft.rs`, which implements IEEE half, bf16, single, double, x87 and quad with round-to-nearest-even.
- Floating to floating checks [dcl.init.list]/7.2: the value is in range and exactly representable.

`narrowing_diag(s, n, init, to) -> Option<Diag>` returns clang's warning
(warning-as-error, group `c++11-narrowing`) with its "insert an explicit
cast" note. The message renders the constant as clang does (`%e`-style for
floating, `format_e6`).

## 8. Literal typing (`literal.rs`)
These functions take the lexer's classification (`kiln_lex::literal`) and
the target:
- `type_integer_literal` / `classify_integer_literal`: [lex.icon] table 8, with the decimal and non-decimal columns, and suffixes `u l ll z` and their combinations. Overflow is handled in two ways:
  - a non-decimal literal that fits only in `unsigned long long` gets `sema_int_literal_implicitly_unsigned` (ExtWarn);
  - a literal that fits in nothing returns `LiteralError(sema_int_literal_too_large)`.
- `type_float_literal` / `classify_float_literal`: [lex.fcon]. Values are rounded exactly from the decimal or hex string, using bignum rationals in `soft.rs`, into `FloatBits`. Suffixes none, `f`, `l` and `f16` are typed. `f32`, `f64`, `f128` and `bf16` get `sema_invalid_float_suffix`, matching clang 19, which has no P1467 extended floating-point types. Overflow and underflow produce `sema_float_literal_overflow` / `_underflow` (group `literal-range`).
- `type_char_literal`: [lex.ccon], giving `char`, `wchar_t`, `char8_t`, `char16_t`, `char32_t`, or `int` for a multicharacter literal.
- `string_literal_type(encoding, units)`: [lex.string]/10, giving `const C[units]`, where `units` includes the terminating NUL.
- `float_const(ty, bits)`: interns the constant.

`LiteralError { id, args }` has `diag(s, loc)` for the caller to emit.

## 9. Deviations from clang 19 (authoritative)
1. `1.0q` (`__float128`) is not supported: `kiln_lex`'s `FloatSuffix` has no `q`.
2. An integer literal too large for any type is an error with no
   recovered value. clang recovers it as `unsigned long long` 0.
3. Not implemented: `-Wconstant-conversion`, `-Wliteral-conversion`,
   `-Wnull-conversion`, `-Wimplicit-int-conversion` and the other
   conversion warnings.
4. The deprecated conversion from a string literal to `char*` is rejected,
   as C++11 and later require. clang accepts it with a warning under
   `-Wwritable-strings`.
5. Binding a reference to a temporary, and to the result of a conversion
   function, is `sema_conv_unsupported` (M3: MaterializeTemporaryExpr).
6. PowerPC double-double `long double` is approximated. Narrowing and
   literal rounding treat it as IEEE double, and the rank comparison
   (`float_order`) treats it as a 106-bit-mantissa format. No supported
   target uses it yet.
7. Narrowing diagnostics carry no fix-it.
8. A `std::nullptr_t` source of a pointer conversion gets no
   `LValueToRValue` step (clang uses `NullToPointer` directly).
9. Ranking treats `nullptr_t → bool` as a boolean conversion for
   [over.ics.rank]/4.1.
10. `bit_field_width` sees bit-fields only through member access, parens
    and casts, not through `?:` or comma.
11. Overload-context argument errors ("no known conversion from … for 1st
    argument") belong to overload resolution. `ConvCtx::Argument` gives the
    single-function wording "cannot initialize a parameter".
12. The multicharacter-literal warning is the lexer's (`kiln_lex`).

## 10. Not yet (M3+)
- User-defined conversions ([over.ics.user]), class initialization and
  constructors.
- Access checking of base conversions ([conv.ptr]/3, [class.access.base]).
- MaterializeTemporaryExpr and reference binding to temporaries.
- Conversion warnings (§9.3) and fix-its.

## 11. Tests
All tests are in `crates/kiln_sema/tests/`, are built with `AstBuilder`,
and each one names its standard section.

| file | sections | tests |
|---|---|---|
| `conv_standard.rs` | [conv.lval] 3, [conv.array] 4, [conv.func] 2, [conv.qual] 6, [conv.prom]/[conv.fpprom] 2, [conv.integral] 3, [conv.double]/[conv.fpint] 2, [conv.bool] 3, [conv.ptr] 4, [conv.mem] 2, [conv.fctptr] 1, [conv.general]/6 1 | 33 |
| `conv_arith.rs` | [conv.prom]/1–7 8, [expr.arith.conv] 12, [expr.ass] 1 | 21 |
| `conv_ics.rs` | [over.ics.scs] 2, [over.ics.rank]/3.2 and /4 9 | 11 |
| `conv_narrowing.rs` | [dcl.init.list]/7.1–7.4 9 | 9 |
| `conv_literal.rs` | [lex.icon] 7, [lex.fcon] 6, [lex.ccon] 1, [lex.string] 1 | 15 |
| `conv_errors.rs` | [dcl.init.general]/16 3, [conv.qual]/3 1, [stmt.return] 1, [expr.assign]/3 2, [conv.general]/4 1, [dcl.init.ref]/5 3, [conv.ptr]/3 1, [conv.mem]/2 1 | 12 |
