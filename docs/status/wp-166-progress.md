# WP-166 progress: NYI type traits

Base b61aa714, merged with main f5dc929c. Branch `wp/166-nyi-type-traits`.

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

## Tests

Each test below covers true, false and ill-formed operands, and passes
against both clang++-19 and kiln:

- `tests/conformance/ext.traits/` has one file per trait (11 files).
- `tests/conformance/dcl.array/incomplete-element.cpp` covers the array fix.
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
