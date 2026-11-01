# WP-166 progress: NYI type traits

Base b61aa714, merged with main f5dc929c and then f948983a. Branch `wp/166-nyi-type-traits`.

## Inventory

Method: every `__`-prefixed identifier in `/usr/lib/llvm-19/include/c++/v1`
(718 names) was probed with `__has_builtin` under clang 19. Of these, 316 are
builtins, and 60 are names libc++ itself tests with `__has_builtin`. After
WP-166, kiln's `__has_builtin` matches clang 19 on all 60 tested names and
answers 1 for all 316 builtins. `table.rs` no longer has a `Status::Owner`
row.

At the base, every trait row that libc++ headers *use* was already
implemented, except the four marked "libc++" in the table below. Those four
were staged off (`__has_builtin` was 0), so libc++ took its fallbacks. The
other rows are clang 19 builtins that the libc++ 19 headers do not
reference. They were also staged off (a use emitted "not yet implemented:
type trait") and are implemented here for conformance.

| Builtin | libc++ 19 use | Before (b61aa714) | After | Implementation |
|---|---|---|---|---|
| `__is_trivially_equality_comparable` | `__type_traits/is_equality_comparable.h` | staged off, NYI diag | clang 19 | `traits/special.rs:154` (`defaulted_equality` :178) |
| `__is_trivially_relocatable` | `__type_traits/is_trivially_relocatable.h` | staged off, NYI diag | clang 19 (`canPassInRegisters`) | `traits.rs:1026` |
| `__type_pack_element` | `variant`, `__tuple/tuple_element.h`, `make_tuple_types.h` | staged off (already implemented by WP-072) | on | `table.rs` row |
| `__has_unique_object_representations` | `has_unique_object_representation.h` | on; did not require trivially copyable | clang 19 (`tc` gate) | `traits.rs:1101`, `unique_reps` :1133 |
| `__has_nothrow_assign` | none | staged off | clang 19 | `traits/special.rs:33` |
| `__has_nothrow_move_assign` | none | staged off | clang 19 | `traits/special.rs:33` |
| `__has_nothrow_copy` | none | staged off | clang 19 | `traits/special.rs:33` |
| `__has_nothrow_constructor` | none | staged off | clang 19 | `traits/special.rs:33` |
| `__can_pass_in_regs` | none | staged off | clang 19, including the operand error | `traits/special.rs:261`, `:280` |
| `__is_bitwise_cloneable` | none | staged off | clang 19 | `traits/special.rs:332` |
| `__is_layout_compatible` | none | staged off, NYI diag | clang 19, including the completeness errors | `traits/special.rs:361` |
| `__is_pointer_interconvertible_base_of` | none | staged off | clang 19 | `traits/special.rs:534` |

Adjacent fix: the meta array tests failed on "array has incomplete element
type". Arrays of an incomplete class type can now be formed, as in clang's
`BuildArrayType` (`decl.rs`). Completeness is checked where it is required,
using clang's wording:

- The variable and field checks name the element type.
- `T vv[2] = {}` names the full type at the variable.
- A subscript reports "subscript of pointer to incomplete type".

Second adjacent fix: forming a zero-length array is a substitution failure
in a SFINAE context, as in clang's `BuildArrayType`
(`err_typecheck_zero_array_size`). So `int[0]` matches neither `T[N]` nor
`T[]`, which fixes libc++ `is_bounded_array`. Outside SFINAE a zero-length
array is still accepted as the GNU extension (`decl.rs` `Chunk::Array`,
`templ/subst/types.rs` `ConstantArray`).

## Gate

`cargo xtask libcxx --gate m3 -j 2` gives the following results:

| Run | Pass |
|---|---:|
| base b61aa714 | 364/585 |
| branch merged with main f5dc929c | 410/585 (that main: 405) |
| branch merged with main f948983a, plus the zero-length fix | 415/585 |

No test fails on an unimplemented trait, first or anywhere.

**The "28 tests" cluster.** WP-154 counted 28 tests whose first failure
was an NYI trait. Most of that cluster was `__is_trivially_assignable` and
`__is_trivially_constructible`, and `98108541`, which is on the base,
already cleared it. The older gate logs name 25 of those tests. Today:

- **16 pass:**
  - `is_trivially_{assignable,constructible,copy_assignable,copy_constructible,default_constructible,move_assignable,move_constructible}`
  - `pairs.pair/ctor.copy_move_trivial`
  - the seven `utility.intcmp` tests
  - `numeric.version`
- **9 fail first on `'typename' using-declarators is not supported yet`:**
  - `span.elem/at`
  - `thread.version` and `vector.version`
  - `is_swappable`, `is_swappable_with`, `is_nothrow_swappable` and `is_nothrow_swappable_with`
  - `mem.res.aliases/header_vector_synop` and `header_vector_synop2`

The other failing trait-adjacent meta tests fail on:

- static asserts: `is_constructible`, `is_nothrow_default_constructible`,
  `is_convertible`, `is_invocable_r(_v)`, `decay` and `forward_like`
- "constexpr variable must be initialized by a constant expression":
  `is_invocable`, `is_nothrow_invocable`, `common_type` and `result_of`
- ambiguous partial specializations: `conjunction`
- "no type named 'type'": `common_reference`

## Tests

Each test below covers true, false and ill-formed operands, and passes
against both clang++-19 and kiln:

- `tests/conformance/ext.traits/` has one file per trait (11 files).
- `tests/conformance/dcl.array/incomplete-element.cpp` and `zero-length-sfinae.cpp`
  cover the array fixes.
- `has-builtin-staged.cpp` gains the 11 rows; `has-builtin-staged-off.cpp`
  keeps only the `__has_constexpr_builtin` row.
- `trait-unimplemented.cpp` is deleted.

## Divergences and follow-ups

- `kiln_abi` call lowering (`call.rs` `can_pass_in_registers`) treats a
  class with a trivial copy constructor and destructor as passed in
  registers even when its move constructor is non-trivial. Sema's
  `can_pass_in_registers` follows clang. Codegen was not changed.
- `__type_pack_element<-1, int>` gives the narrowing error but not clang's
  second "out of bounds index" error. The narrowing error also lacks the
  `[-Wc++11-narrowing]` tag.
- kiln does not honour `#pragma clang diagnostic`, so the tests use
  `//@ args: -Wno-deprecated-builtins`.
- `'typename' using-declarators` (14 gate tests first, 32 any) are the next
  blocker for 9 of the former trait-cluster tests.
- `count_new.h:377` `static MemCounter counter((MemCounter::MemCounterCtorArg_()));`
  gives "expected expression" (18 gate tests first). A parenthesized,
  value-initialized temporary does not parse as a direct-initializer.
