# spec/reflection-values.md — values and static storage (M6-10, WP-479)

[meta.reflection.extract], [meta.reflection.result] and P3491R3
([meta.define.static], [meta.string.literal]). The ops are a family of
the reflection runtime of spec/reflection.md §3; this file is their
contract. Landed notes: spec/landed/reflection/WP-479.md.

## 1. Where things live

| Piece | Where |
|---|---|
| Op rows (`ValueOp`), freeze/thaw, range reading | `crates/kiln_oven/src/meta/value.rs` |
| Reflection-byte accounting | `crates/kiln_oven/src/meta/intern.rs`, `Memory::refl` |
| Type rules, reflectee choice, `extract` | `crates/kiln_sema/src/meta/value.rs` |
| Library functions (`define_static_*`, range forms, `is_string_literal`) | `tests/support/kiln_meta.h` (WP-479 block; `<meta>` is WP-484) |
| Run-time objects | `crates/kiln_codegen/src/consts.rs` (`const_chars`), `fold.rs` |

## 2. Op table

Every row is `Stable` (spec/reflection.md §3.3): its result is a pure
function of its operands, because the operand is frozen into the
content-interned constant store first and the reflection table interns
by reflectee. A call that reaches only these ops stays memo-eligible
(§3.6; `stress/reflect-constant-1e5.cpp` line 29 is a memo hit).

| Row (intrinsic name) | Signature | Result |
|---|---|---|
| `extract` | `(info)` | `T` (from the specialization) |
| `reflect_constant` | `(const T&)` | `info`; `(info)` is `ReflectConstantInfo` |
| `reflect_object` | `(T&)` | `info` |
| `reflect_function` | `(T&)` | `info` |
| `__kiln_reflect_constant_string` | `(R&&)`, `R` a built-in array or a `span` | `info` |
| `__kiln_reflect_constant_array` | likewise | `info` |
| `__kiln_is_string_literal` | `(const charT*)` | `bool` |

`reflect_constant_string`, `reflect_constant_array`,
`define_static_string`, `define_static_array`, `define_static_object` and
`std::is_string_literal` are library function templates over these, as
libstdc++ defines them. The range forms take any input range: a built-in
array or a contiguous range (`data()`/`size()`) is passed as is or as a
`span`; any other range is copied into a transient `new T[n]` buffer
first (freed before return). The intrinsics have `__kiln_` names because
`Sema::meta_intrinsic_of` matches any `std::meta` function by name and
shape, so a public name would claim the library template itself.

## 3. Reflecting values ([meta.reflection.result])

1. `reflect_constant(e)`: `T` (the decayed, unqualified parameter type)
   must be structural ([temp.param]/7) or the call fails with
   "'T' is not a structural type". The value is frozen; it must satisfy
   the template-argument rules of [temp.arg.nontype]/6 or the call fails
   naming the offending subobject (a pointer or reference to a string
   literal, a temporary, a local, a subobject of a string literal, a
   `typeid` result, `__func__`). A scalar gives `Reflectee::Value(cv)`;
   a class value gives `Reflectee::Object` of the template parameter
   object of `const T` with that value, so `reflect_constant(K{1}) ==
   reflect_constant(K{1})` and both designate the object `C<K{1}>`
   would name. A consteval-only `T` other than `info` is valid; an `info`
   operand is `ReflectConstantInfo` and gives a value reflection of type
   `info`.
2. `reflect_object(o)`: `o` must designate an object with static storage
   duration usable as a template argument (not a local, a temporary or a
   string-literal subobject); the result is `Reflectee::Object` of a
   pointer constant to it. Equal designations are one reflection.
3. `reflect_function(f)`: `T` must be a function type; the result is the
   reflection of the function's entity (`Sema::entity_decl`), equal to
   `^^f`. An immediate function fails: a reference to it is not a
   constant expression ([expr.const]). GCC 16.1 accepts this; kiln
   follows the IS.
4. `reflect_constant_string(r)` / `reflect_constant_array(r)`: the
   element type must be structural (arrays are peeled to their innermost
   element); characters for the string form, which appends a null
   terminator. The result is `Reflectee::Object` of a `PtrBase::Const`
   constant array of the elements, interned by content, so equal
   contents give one object and one reflection. An empty
   `reflect_constant_array` is `unsupported` (the IS gives `array<T, 0>`).

## 4. Static storage ([meta.define.static])

1. `define_static_string(r)` is `extract<const T*>(reflect_constant_string(r))`;
   `define_static_array(r)` is the `span<const T>` of
   `extract<const T*>(reflect_constant_array(r))` and the count (an empty
   range gives an empty span); `define_static_object(t)` is
   `extract<const T*>(reflect_constant_array(span(&t, 1)))`, so it works
   for classes, unions, arrays, scalars and `info` alike.
2. `is_string_literal(p)`: whether `p` points into a string literal
   object. The objects of §3 item 4 are not string literals.
3. Identity: equal contents give the same object at compile time and at
   run time (`define_static_string("a") == define_static_string("a")`),
   and a `define_static_string` result is never a string literal, so as a
   template argument it is accepted (P3491 §3.5.1).
4. Run time: codegen emits each frozen object once, with internal
   linkage (`private unnamed_addr`) and a deterministic name. Character
   arrays go to the string pool (`.str.N`), so an equal string literal
   shares the global ([intro.object]/9 allows both to be non-unique);
   other arrays are `.constant.N`; class values are template parameter
   objects (`_ZTA...`, `linkonce_odr`, as for any). A constant initializer
   that sema already evaluated is emitted from the stored value, not
   re-folded (codegen cannot run reflection intrinsics).
5. `template for` over `define_static_array(v)` (or the span stored in a
   `constexpr` variable) expands over the frozen array
   ([stmt.expand]; WP-447).

## 5. `extract<T>(r)` ([meta.reflection.extract])

1. A value reflection: `T` must equal the value's type modulo
   cv-qualification, else the call fails ("'r' reflects a value of type
   'U', not 'T'"). A class value is copied into the result object.
2. An object or variable: `T` a reference `U&` binds to the object when
   `U` is reference-compatible with its type; `T` a pointer `U*` to an
   array element points at the first element when `T` is an array of `U`
   ([meta.reflection.extract]/3). A constexpr reference variable extracts
   through to its referent. Otherwise "cannot be extracted as".
3. A function: `T` a function pointer (noexcept may be dropped);
   `T` a function reference is ill-formed (IS, as GCC).
4. A non-static data member or member function: `T` a member pointer of
   matching class and type; a bit-field fails.
5. An enumerator: its value as the enumeration type.
6. Fast path: a scalar value or pointer extract writes the result
   register from the stored constant with no Oven allocation
   (`meta.reflection.result/reflect-constant-budget.cpp`: 10,000
   `extract<int>` calls, `allocs<=0`).

Every failure is a `MetaError` note on the call (spec/reflection.md §8.1),
never a panic.

## 6. Accounting (D5)

What a call adds to the translation unit's interned tables (new content
bytes in the constant store, plus `REFL_ENTRY_BYTES` per new reflection
table entry) is charged to the running evaluation as `refl_bytes`
(spec/oven.md "Limits and metrics", "Reporting protocol"). It is part of
`peak_bytes`, `peak_arena` and the `-fconstexpr-memory` limit, and a
separate `-fconstexpr-report` counter. A call whose result is already
interned charges nothing, so 10,000 `reflect_constant(i % 16)` calls
make 16 entries (`meta.reflection.result/reflect-constant-budget.cpp`);
`tests/stress/reflect-constant-1e5.cpp` pins the bound at 10^5 calls
and, under a small `-fconstexpr-memory`, the existing memory-limit error.
Content counts only for the value and name families, and a byte a nested
call already charged is not charged again by the enclosing one
(spec/oven.md "Limits and metrics").

## 7. Open items

- type_of / is_value on value and object reflections (WP-476).
- `extract` of a reflection of a local inside a consteval function, and
  of a non-constexpr reference variable.
- An empty `reflect_constant_array` (IS: `array<T, 0>`).
- Substituting a `PtrBase::Const` pointer NTTP in a template body
  (`crates/kiln_sema/src/templ/nttp.rs`, `nttp_value_expr`).
- kiln accepts a string literal as a pointer template argument
  (`C<"nope">`, [temp.arg.nontype]/6).
