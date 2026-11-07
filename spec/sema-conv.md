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
The conversion machinery reads `Sema`'s `ast`, `diags` and `target:
TargetTypes` (the full state is sema.md §1). Every function in this spec
takes `&Sema` or `&mut Sema`. Helpers:
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
- `second: SecondStep`: Identity, IntegralPromotion, FloatingPromotion, IntegralConversion, FloatingConversion, FloatingIntegral, PointerConversion, MemberPointerConversion, BooleanConversion, NullPointer or DerivedToBase (a class argument for a base-class parameter or reference, Conversion rank: [over.best.ics]/6, [over.ics.ref]/1; WP-068a);
- `fn_ptr: bool`;
- `third: ThirdStep`: Identity or Qualification;
- the `rank()`: Exact, Promotion or Conversion;
- `rb: RefBinding`, the reference binding the sequence performs (WP-068a;
  clang's `StandardConversionSequence` binding bits): `IS_REF`, `DIRECT`,
  `LVALUE_REF`, `BINDS_RVALUE`, `BINDS_FUNCTION_LVALUE`,
  `OBJECT_NO_REF_QUAL` (an implicit object parameter of a member without a
  ref-qualifier) and `IGNORED` (a static member's object parameter).
  `StandardConversion::identity(from, to, rb)` is the identity sequence;
  overload resolution (spec/sema-overload.md §3.3) fills `rb`;
- the data the ranking rules need.

`standard_conversion_from` does the same from a type and category, with no
expression. `ctx` matters only for `nullptr_t → bool`, which is allowed
only when `ctx` is a direct-initialization ([conv.bool]).

`apply_standard_conversion(e, scs, to)` builds the casts. A `DerivedToBase`
second step casts a glvalue to its base with the path, checking access and
reporting an ambiguous base (a class prvalue reaches a base by a
constructor instead). Cast targets:
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

It implements the cv-decomposition rules, including P0388 array-of-unknown-bound unification. An array's cv is the union of its element's cv (`effective_quals`). An array is not a level of its own for [conv.qual]/3.3 ([basic.type.qualifier]/3; clang's `UnwrapSimilarTypes` merges it with its element): a cv added to an array's elements requires `const` only at the levels above the array's enclosing pointer, so `int (*)[]` converts to `volatile int (*)[]` (WP-207). The function pointer conversion ([conv.fctptr]) is `is_function_pointer_conversion`.

Base lookup (`conv/bases.rs`): `find_base(derived, base) -> BaseSearch {
NotDerived, Unique(BasePath), Ambiguous(Vec<BasePath>) }` searches the
complete base graph depth first, with a bounded visit count. Two paths to
the same virtual base count as one. Access is **not** checked (§10).
`find_base` itself does not instantiate. A caller that asks whether a class
template specialization is derived first completes it with
`require_complete_silent`, as clang's `isCompleteType` does
([temp.inst]/2). The callers that do this are: the derived-to-base pointer
conversion (`pointer_conversion`), reference-relatedness (`ref_related`),
reference binding (`bind_reference`, which completes the initializer's
class as clang's `CompareReferenceRelationship` does, so `S<int>& r = d;`
instantiates a `D<int>` named so far only through references or pointers;
WP-381, `tests/conformance/dcl.init.ref/derived-to-base-instantiates-derived.cpp`),
and static_cast up and down (`class_relation`, `static_downcast`).

A C-style or functional cast converts between a derived class and its
base even when the base is inaccessible ([expr.cast]/4, clang's
`IgnoreBaseAccess`). Pointers and references, upcasts and downcasts are
all covered: `class_relation` and `static_downcast` skip the access check
for those styles, and `build_explicit_cast` sets
`Sema::cast_ignores_base_access` while it applies the standard conversion,
so `check_base_access` passes. `static_cast` and implicit conversions still
check access. In a SFINAE context `(D*)(B*)` is therefore valid whenever
`B` is a non-virtual base, which is how Boost.TypeTraits
`is_virtual_base_of` tells a virtual base from a non-virtual one (WP-787,
`tests/conformance/expr.cast/c-style-cast-inaccessible-base.cpp`).

## 4. Ranking (`compare_standard_conversions`)
`compare_standard_conversions(a, b) -> IcsCompare {Better, Worse,
Indistinguishable}` implements these rules in clang 19's
`CompareStandardConversionSequences` order (WP-068a; spec/sema-overload.md
§13.2.1):
- [over.ics.rank]/3.2.1: proper subsequence, with the lvalue transformation ignored;
- /3.2.2: rank;
- /4.1: a conversion that does not convert a pointer, member pointer or `nullptr_t` to `bool` beats one that does;
- /4.2: promotion to a fixed enum's underlying type beats promotion to its promoted type;
- /4.4.1–4.4.2: conversions to `void*`, then the derived/base pointer orderings;
- /3.2.5: same type except top-level cv, where the more cv-qualified one is worse (qualification-adjusted similarity);
- /3.2.3, /3.2.4: the reference-binding kinds (rvalue reference to an rvalue over lvalue reference; lvalue reference to a function lvalue over rvalue reference), not for the object parameter of a member without a ref-qualifier;
- /3.2.6: two reference bindings to the same type but for top-level cv, the less qualified referent better (array element cv counts as the array's: `int(&)[]` beats `const int(&)[]`; `same_but_cv`);
- /4.3: for floating-point conversions between types of equal rank, the one whose target matches the source's conversion rank is better;
- /4.4.3–4.4.8: the derived/base member-pointer, class (`DerivedToBase`) and reference rules.

Before these rules, a sequence whose second step breaks P1467's rank rule
(`fp_rank_violating`: a floating-point conversion that [conv.double]/1
does not allow, §6) is worse than any sequence that does not, as GCC 14
ranks its pedantic "bad" conversions: `void f(_Float16); void f(int);
f(1.0)` picks `f(int)`. Test:
`tests/conformance/basic.extended.fp/p1467-overload.cpp`.

`float_order(a, b) -> FloatOrder {Less, Equal, Greater, Unordered}`
compares floating-point conversion ranks ([conv.rank]/2) through the
target's formats (`format_subset`). bf16 and f16 are unordered. For equal
value sets a standard type has the greater subrank ([conv.rank]/2.4:
`float` over `_Float32`, `double` over `_Float64`, and `long double` over
`_Float64` where it is IEEE double), and `extended_subrank` orders the
extended types of equal value sets `__float128` (0) < GCC's `_Float32x`
/ `_Float64x` (1) < `_Float32`/`_Float64`/`_Float128` (2): `_Float128`
above the vendor `__float128` is kiln's choice (GCC 14 has both only on
x86-64 and gives them distinct ranks; clang 22 has no `_Float128`), and
`_FloatN` above `_FloatNx` is GCC 14's (`_Float32x + _Float64` is
`_Float64`; on AArch64 `_Float64x + _Float128` is `_Float128`; WP-380). `__fp16` and `_Float16` stay equal, as in clang, so
`h += f16` still computes in `half`.

**P1467 rank rule** (`rank::fp_rank_violation(s, from, to) ->
Option<unordered>`, [conv.double]/1 C++23): when either type is one of
the C++23 extended floating-point types (`_Float16`, `_Float32`,
`_Float64`, `_Float128`, `__bf16`; `BuiltinType::is_std_extended_floating`),
an implicit floating-point conversion requires the destination's value set
to contain the source's. GCC 14 accepts a violating conversion with a
pedantic warning, and so does kiln: `apply.rs` `fp_rank_check` emits
`sema_fp_conversion_rank` (ExtWarn, group `float-conversion-rank` in the
`narrowing` umbrella; spec/diag.md §12.6) at the converted expression for
every implicit `FloatingConversion` (initialisation, argument passing,
return, assignment) and, from `compound_assign` in `expr.rs`, for the
conversion of an `E1 op= E2` result back to `E1`'s type. Explicit casts
([expr.static.cast]/11) are silent. Floating-point promotion
([conv.fpprom]) stays `float` → `double` only: no extended type
promotes, so `_Float32` → `double` is a conversion. `__bf16` ↔ `_Float16`
(unordered) converts with the warning; `__bf16` ↔ `__fp16` keeps clang's
rejection. Test: `tests/conformance/basic.extended.fp/p1467-conversions.cpp`.

## 5. Implicit conversion (`implicit_convert`)
`implicit_convert(s, e, to, ctx) -> Result<ExprId, ConvError>`:
- Error and dependent types pass through unchanged.
- A reference target binds directly to a reference-compatible glvalue ([dcl.init.ref]/5.1, 5.3.1, the xvalue case). The result is `NoOp` or `DerivedToBase` (with path) to `cv T`, keeping the category.
  - Function types ([dcl.init.ref]/4, WP-290): a function lvalue of type `F noexcept` is reference-compatible with `F` (pointer to `F noexcept` converts to pointer to `F` by a function pointer conversion; `conv::function_ref_compatible`), so `void (&r)(int) = g;` binds a noexcept `g` directly, while `void (&r)(int) noexcept = h;` with a throwing `h` is "non-const lvalue reference to type 'void (int) noexcept' cannot bind to a value of unrelated type 'void (int)'". The binding's conversion sequence is the identity ([over.ics.ref]/1, clang's `ReferenceConversions::Function`), so `n(void(&)(int) noexcept)` against `n(void(&)(int))` is ambiguous for a noexcept function, overload set or function template (`ref_ics`; `over.ics.ref/function-noexcept-reference-binding.cpp`).
  - P0388R4 ([dcl.init.ref]/4, WP-197): `T[N]` is reference-compatible with `T[]` (`unknown_bound_of`: same canonical element type), so `int (&r)[] = a;` binds directly with a `NoOp` to `int[]`. As in clang, the pair is unrelated when the binding would drop qualifiers, so a mismatch is `sema_reference_bind_failed` (`lvalue` only for an lvalue) rather than a temporary. In ranking, `h(int(&)[])` against `h(int(&)[3])` is ambiguous (clang 19; the deeper similarity of non-top-level bounds is not modelled).
  - A variable of type reference to `T[]` initialized by a braced list binds a lifetime-extended `T[N]` temporary ([dcl.init.ref]/5.4.2, [class.temporary]/6), and its declared type becomes reference to `T[N]` (`Sema::ref_to_bound_array`, as clang's `VarDecl` type update for an `InitListExpr`; one bound to an lvalue keeps `T[]`), so `sizeof` and `decltype` see the bound (WP-233). Tests: `tests/conformance/dcl.init.ref/unknown-bound-braced-temporary.cpp`, `binding-failure-wording.cpp`, `implicit-move-return.cpp`.
- A class target, or a class source, goes through overload resolution
  (WP-068a, spec/sema-overload.md §10.3): `try_implicit_conversion` computes
  the implicit conversion sequence and a user-defined one is applied by
  `user_conversion` (constructor or conversion function, with the
  candidates as notes of clang's error when there is none).
- Otherwise the standard conversion sequence, or a `ConvError` worded for `ctx`.

`ConvCtx` values:
- `Initialization` and `DirectInitialization`;
- `Assignment`;
- `Argument`;
- `Return`;
- `Comparison`;
- `Condition`, which is contextual bool and counts as direct ([conv.general]/4);
- `Cast`.

`ConvError { id, args, notes }` is emitted by the caller with
`err.diag(s, e)`, which puts the caret at `e` (`err.diag_at(s, loc)` at
`loc`); `notes` are the candidates of a failed user-defined conversion.

Wording table (all verified against clang 19; tests in `conv_errors.rs`):
- **Initialization, Argument, Return:** `sema_init_conversion_failed`, "cannot initialize {a variable|a parameter|return object|a value} of type T with an {rvalue|lvalue} of type U" plus, for member-pointer and pointer/reference-to-function pairs, clang's `HandleFunctionTypeMismatch` suffix (": different classes ('D' vs 'C')", ": different number of parameters (0 vs 1)", ": different qualifiers (unqualified vs 'const')", ...; WP-083). An xvalue prints as "rvalue", which covers the implicitly moved `return` operand. The caller marks that operand as an xvalue ([class.copy.elision]/3). The entity of an `Initialization` failure follows clang's `InitializedEntity` kind (`Sema::init_entity`, set with `with_init_entity`; constants in `init::entity`, WP-216): "a variable" for a variable (reset at each variable's initialization, so a variable inside a lambda in another entity's initializer names itself), "a value" for `T{x}`, "a new value" for `new`, "a member subobject" for an aggregate member (braced or parenthesized, a union member, a member of a base element), a mem-initializer and a default member initializer, "an array element" for an array element, "a base class" for a base element or base mem-initializer, "an exception object" for the thrown object and "a lambda capture" for an init-capture. The last three never reach this scalar diagnostic in practice: a base or exception object is a class, converted by overload resolution, and an init-capture's type is deduced. A variable's or member's initializer that fails either way (`sema_init_conversion_failed` or `sema_reference_bind_failed` from `init.rs`) underlines the initializer expression's range, as clang's `Args[0]->getSourceRange()` does (WP-279).
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
  - "reference to {type|incomplete type} T could not bind to an {rvalue|lvalue} of type U" (`sema_reference_bind_failed`, clang `FK_ReferenceInitFailed`): an unrelated non-class pair that creates no temporary — arrays (P0388), a function lvalue bound to a reference to another function type (`void (&&r)() = h;` with `int h(int)`), a referee that is an incomplete class. An xvalue prints as "rvalue".
  - A related non-class glvalue that is not an lvalue (an xvalue, also the implicitly moved `return x;` of P2266, [class.copy.elision]/3) binds no non-const lvalue reference: "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'" (clang `FK_NonConstLValueReferenceBindingToTemporary`), checked before the bit-field, vector-element and dropped-qualifier failures.
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
3. the rules below continue with the promoted LHS type; a floating LHS
   with an integer RHS computes in that type, except that a storage-only
   `__fp16` LHS computes in `float` (clang `handleFloatConversion`). The
   usual unary conversions already turn an `__fp16` RHS into `float`, so
   `h += h2` computes in `float`, while `h += f16` (a `_Float16` RHS)
   computes in `__fp16`, as in clang.

The computation result must convert back to the LHS type. For an
unscoped enumeration LHS it never does ([conv.prom]/3 made it an integer
or the RHS made it floating), so `compound_assign` in `expr.rs` reports
clang's `assigning to 'E' from incompatible type 'int'`
(`sema_assign_incompatible`), for the shift operators too.

The rules, per [expr.arith.conv]/1:
- /1.1: a scoped enum mixed with another type is an error.
- /1.2 (C++26): enum with a different enum, and enum with a floating type, are errors (`sema_arith_enum_enum`, `sema_arith_enum_float`).
- /1.3: the floating-point rules, using `float_order`: the operand of lesser rank converts to the other's type, and for equal ranks to the type of greater subrank (`_Float32 + float` is `float`, `_Float32 + double` is `double`, `_Float32 + _Float64` is `_Float64`). Unordered types (`_Float16` with `__bf16`) are an error (`sema_arith_float_unordered`), also in `?:` ([expr.cond]/7.2), comparisons and compound assignment. The usual unary conversions do not promote an extended type.
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
- Floating to floating checks [dcl.init.list]/7.2: the value is in range and exactly representable. When either type is a C++23 extended floating-point type the "lower rank" test is the P1467 value-set rule (`format_subset`; C++23 wording "whose floating-point conversion rank is neither greater than nor equal to"), so `_Float32{1.0}` (constant, exact) is fine and `_Float32{d}` for a `double` variable is narrowing; otherwise clang's ranks.

`narrowing_diag(s, n, init, to) -> Option<Diag>` returns clang's warning
(warning-as-error, group `c++11-narrowing`) with its "insert an explicit
cast" note. The message renders the constant as clang does (`%e`-style for
floating, `format_e6`). Both underline the initializer, and the note
carries clang's two insertions `static_cast<T>(` at its start and `)` after
its last token (only for a spellable `T`, a typedef or builtin; WP-421).

The full-expression's -Wliteral-conversion / -Wconstant-conversion
(§9 item 3) still checks a narrowed element, so `S{3.5, 'x'}` reports
both, as clang does. At `-fclang-compat=22` the one element of a braced
functional cast (`int{6.5}`, `I{7.5}` for a typedef `I`, `T{4.5}` of a
one-member aggregate, `int{(9.5)}`) is not checked (clang 22
`CheckImplicitConversion` stops at the explicit cast and skips a
one-element list under it), so only the narrowing error is reported; a
list of several elements and a nested list are checked. At 19 every
element is (WP-465, `impcast.rs` `check_implicit_conversions`; tests
`expr.type.conv/braced-cast-literal-conversion{,-19}.cpp`).

## 8. Literal typing (`literal.rs`)
These functions take the lexer's classification (`kiln_lex::literal`) and
the target:
- `type_integer_literal` / `classify_integer_literal`: [lex.icon] table 8, with the decimal and non-decimal columns, and suffixes `u l ll z` and their combinations. Overflow is handled in two ways:
  - a non-decimal literal that fits only in `unsigned long long` gets `sema_int_literal_implicitly_unsigned` (ExtWarn);
  - a literal that fits in nothing returns `LiteralError(sema_int_literal_too_large)`.
- `type_float_literal` / `classify_float_literal`: [lex.fcon]. Values are rounded exactly from the decimal or hex string, using bignum rationals in `soft.rs`, into `FloatBits`. Suffixes none, `f`, `l`, `f16`, and (P1467, WP-361) `f32`/`F32`, `f64`/`F64`, `f128`/`F128` and `bf16`/`BF16` are typed, as `_Float32`, `_Float64`, `_Float128` and `__bf16`, each rounded once from the exact decimal or hex value (`bf16` too, not through `float`; spec/lex.md). Before C++23 they are an extension (`lit_float_suffix_ext`, `-Wc++23-extensions`). Overflow and underflow produce `sema_float_literal_overflow` / `_underflow` (group `literal-range`).
- `type_char_literal`: [lex.ccon], giving `char`, `wchar_t`, `char8_t`, `char16_t`, `char32_t`, or `int` for a multicharacter literal.
- `string_literal_type(encoding, units)`: [lex.string]/10, giving `const C[units]`, where `units` includes the terminating NUL. `C` is `string_literal_elem(std, encoding)`: a UTF-8 string literal's is `char8_t` from C++20 and `char` before (C++17 [lex.string]/8, P0482R6), also for the literal-operator call of `u8"..."_x` ([lex.ext]/5; WP-691; also WP-686's `tests/conformance/lex.string/u8-literal-char-before-cxx20-run.cpp`).
- `float_const(ty, bits)`: interns the constant.

`LiteralError { id, args }` has `diag(s, loc)` for the caller to emit.

## 9. Deviations from clang 19 (authoritative)
1. `1.0q` (`__float128`) is not supported: `kiln_lex`'s `FloatSuffix` has no `q`.
2. An integer literal too large for any type is an error with no
   recovered value. clang recovers it as `unsigned long long` 0.
3. Implicit-conversion warnings (`impcast.rs`, clang
   `CheckImplicitConversion`) cover `-Wliteral-conversion` (floating
   constants) and, since WP-217, `-Wconstant-conversion` (on by default,
   `implicit conversion from 'int' to 'const signed char' changes value
   from 300 to 44`): an integral constant whose value needs more bits
   than the target (significant bits when negative, active bits
   otherwise), or a non-negative signed value of exactly the target's
   width that becomes negative unless spelled as a hex, octal or binary
   literal. The target type prints with its qualifiers. The second range
   is the context token (an operator), measured as a whole punctuator
   (`&&` underlines two columns; `>>` is measured as `>`, the half a
   template argument list may close with; WP-410). Not for a `bool`
   target, the same canonical type, dependent or error operands, an
   unevaluated operand, an immediate or constant-evaluated context, or a
   constant expression (`act_on_constant_full_expr`, which sets
   `Sema::constant_full_expr` for array bounds, bit-field widths,
   enumerators, `static_assert`, `alignas`, case values and template
   arguments). Conditional arms are checked separately. The warning goes
   through `diag_if_reachable`. Those exemptions and `diag_if_reachable`
   apply to the first rule only (clang `DiagRuntimeBehavior`); the
   same-width rule is a plain diagnostic, so `case 64:` on a
   `_BitInt(7)` condition, a `signed char` NTTP or an enumerator of a
   fixed `signed char` type, a namespace-scope `constexpr` initializer
   and unreachable code report it too, before the narrowing error
   of a converted constant expression (WP-465;
   `conv.integral/constant-conversion-same-width.cpp`). A constant truncated by the bit-field it
   is assigned to reports -Wbitfield-constant-conversion instead and its
   conversions are skipped (spec/sema.md §7.12). Not implemented:
   `-Wnull-conversion` (it waits on `__null`, which glibc's and the
   resource headers' `NULL` expands to and kiln does not parse yet; its
   check belongs in `impcast.rs` `check_conversion`, keyed on a
   `GNUNullExpr`/`nullptr_t` source converted to a non-pointer),
   `-Wimplicit-int-conversion` and the other conversion warnings; non-dependent full-expressions are not
   re-checked per instantiation.
   A pointer, array or function converted to `bool` (WP-408, clang
   `DiagnoseAlwaysNonNullPointer`, same wording at 19 and 22; on by
   default, `-Wbool-conversion` under `-Wconversion`):
   `this`, `&r` for a reference `r` and `&g()` of a call returning a
   reference (a glvalue call, through a pointer too; clang
   `CheckForReference`, WP-420) are -Wundefined-bool-conversion
   ("'this' pointer cannot be null in well-defined C++ code; ...",
   "reference cannot be bound to dereferenced null pointer ..."); the
   reference forms point at the operand's expression location (member
   name, overloaded operator), carry the operand's range (with its
   parentheses) and the context token's, and a direct callee adds the
   note "'g' returns a reference" at its declaration; a name
   (or member access) of a function or array, or `&x` of an object, is
   -Wpointer-bool-conversion "address of {''|function |array }'x' will
   always evaluate to 'true'", the name printed as written, with the
   expression's range. A function adds "prefix with the address-of
   operator to silence this warning" (fix-it `&`) and, when it is
   callable with no arguments and returns `bool`, "suffix with
   parentheses to turn this into a function call" (fix-it `()`). Not
   for `&f` of a function, a weak declaration, the address of a
   prvalue call, an unevaluated operand, or a source or conversion
   point in a macro body. The walk visits a binary operator's right
   operand first, so `f && g` warns for `g` first. The classification is
   spec/sema.md §5.27's (`null_compare.rs` `always_non_null`, WP-421),
   which adds the reference-returning call note, the `returns_nonnull`
   call and `nonnull` parameter forms, the lambda conversion, and the
   context token's range. Tests: `conv.bool/pointer-bool-conversion.cpp`,
   `tests/conformance/expr.unary.op/pointer-bool-conversion.cpp`,
   `expr.log.and/pointer-bool-conversion-operands.cpp`,
   `intro.compliance/wbool-conversion-members.cpp`,
   `expr.unary.op/address-of-reference-call-bool.cpp`.
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
13. P1467 (clang 19/22 have no `_Float32`, `_Float64`, `_Float128` or
    `bf16` literals; GCC 14 is the reference). The rank warning is
    `-Wfloat-conversion-rank` in kiln and `-Wnarrowing`/`-Wpedantic` in
    GCC; kiln's group sits in the `narrowing` umbrella. A narrowing
    list-initialisation such as `_Float32{d}` is kiln's
    `c++11-narrowing` error with its note, where GCC 14 only warns. An
    ambiguous call caused by two rank-violating candidates says
    "ambiguous" where GCC says "no matching function", and the rank
    warning is not repeated per candidate.

## 10. Not yet (M3+)
- Conversion warnings (§9.3) and fix-its.

User-defined conversions, class initialization, base access checks,
temporaries and reference-binding ranking landed with WP-067/WP-068a
(spec/sema-overload.md, spec/sema-class.md).

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
