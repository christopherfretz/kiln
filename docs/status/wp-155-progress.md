# WP-155 progress: out-of-line member classes, traits, SVE, parse/sema follow-ups

Branch `wp/155-member-class-oot-defs`, worktree `kiln/.worktrees/wp-155`,
base b8af85f, merged with main 0dc6691 before CI. `variant:1141` (the
non-type pack partial substitution) went to WP-150.

libc++ M3 gate: 317/585 before (319 on main 0dc6691), **332/585 after**
(253 fail, 57 unsupported). These clusters are gone:
- the `__is_trivially_*` stubs (25 first);
- `__SVFloat32_t` (14 first);
- "constructors is not supported yet" (9 first);
- "comparison of distinct pointer types" (8 first).

## Items
1. [x] Out-of-line definitions of member classes and enums of class
   templates, including members of partial specializations and member
   class templates (`templ/mod.rs`, `decl.rs` `act_on_tag`, the parser's
   `parse_tag_qualifier`). Tests: `temp.mem.class/` (9), `temp.mem.enum/`.
2. [x] `__is_trivially_constructible` / `__is_trivially_assignable`
   (`templ/builtins/traits.rs`, clang's `hasNonTrivialCall`).
3. [x] AArch64 SVE sizeless builtins `__SVInt8_t` … `__SVBool_t`
   (ids 34..=47; spec/types.md §5.1, §11).
4. [x] `vector:769`: a comma in an NSDMI or a default argument inside a
   possible template argument list. This ports clang's
   `ConsumeAndStoreInitializer` heuristic (`class.rs` `capture_initializer`,
   `tentative.rs` `comma_ends_saved_initializer`).
5. [x] `function.h:377` and `function.h:927`:
   - `:377` is the composite pointer type `void *` vs `const B *`, which is
     `const void *` (`expr.rs` `cv_combined_pointer`).
   - `:927` is `X<R(A...)>::X()`, a constructor of a partial
     specialization. The parser annotated the template scope ahead of
     `at_special_declarator` (`kiln_parse/src/decl.rs` ~492).
   - Same family, `bitset:486`: `B<1, S>::g`. The written non-type argument
     is now converted before the partial specialization lookup (`templ/actions.rs`
     `entered_partial_by_converted_args`).
   - `<functional>` now stops at `function.h:519`,
     `constexpr … = { …, &typeid(void) }`. That is WP-105.
6. Cheap extras:
   - [x] The explicit-object cv/ref-qualifier diagnostic now points at the
     first qualifier and quotes them all, cv first
     (`dcl.fct/explicit-object-qualifiers.cpp`).
   - [x] `T t = {3}` in a function template already matches clang (nothing
     to do).
   - [ ] Explicit-object-parameter lambdas are still "not supported yet":
     not cheap (closure-type deduction for `this auto`).

## Follow-ups
- Explicit-object-parameter lambdas.
- `&typeid(T)` in a constant initializer (WP-105).
- Dependent template-ids are neither argument-checked nor converted outside
  declarator scopes (clang checks every template-id).
- Multi-level composite pointer types (`int **` vs `const int **`).
- Explicit instantiation definitions should get weak_odr/comdat linkage.
- A static const member used in an NSDMI is loaded instead of folded.
- SVE divergences: `&l`, `x = x`, lambda SVE returns, non-static member
  references.
