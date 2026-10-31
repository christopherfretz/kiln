# WP-128 progress — libc++ gate blockers

Branch `wp/128-gate-blockers`, worktree `kiln/.worktrees/wp-128`, base
2fc5c27 (WP-122 merge; clang self-check 1604/1635, kiln-mode 1622/1635).

Probe: `kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0` on a
file with one `#include`, error count.

| step | `<compare>` | `<utility>` | `<span>` | `<array>` | `<string_view>` |
|---|---|---|---|---|---|
| base | 0 | 6 | 15 | 25 | 41 |
| 1 void return of dependent operand | 0 | 6 | 15 | 22 | 38 |
| 2 block-scope using of overload sets | 0 | 4 | 13 | 18 | 34 |
| 3 braced arguments in dependent calls | 0 | 2 | 13 | 16 | 32 |
| 4 dependent `::new` with pack initializer | 0 | 2 | 13 | 12 | 28 |
| 5 `::__builtin_*` | 0 | 2 | 13 | 10 | 25 |
| 6b alias substituted in its own context (+ dependent explicit-args callee) | 0 | 1 | 13 | 8 | 23 |

## Items
1. [x] `return f(args...);` in a void function template (`stmt.rs`
   `act_on_return`: a type-dependent operand is checked at instantiation).
2. [x] `using std::swap;` in `pair`'s member functions: the shadows of
   one using-declarator no longer conflict with each other (function
   templates were "anything else conflicts"); two using-declarators may
   introduce same-signature functions ([namespace.udecl]/11 example);
   block-scope lookup returns every function of the name (it returned
   the last one only); block-scope using-declarations instantiate.
3. [x] braced arguments in a dependent call: kept as a dependent
   `InitListExpr` (nested lists too; a dependent list makes the call
   dependent) and rebuilt as braced arguments at instantiation.
4. [x] `::new (p) T(args...)`: the parser takes `...` in a new-placement /
   new-initializer list; a new-expression with a dependent type or
   operands is kept as written (`dependent_new`, no allocation-function
   lookup) and rebuilt at instantiation (subst `new_expr`). Tests:
   `expr.new/dependent-operands.cpp`, `dependent-placement-error.cpp`
   (kiln and clang++-19 pass; `expr.new/` 46/46). Full corpus NOT re-run
   after item 4 (stopped by the host-reset request); after item 3 it was
   1628/1641, 0 failed.
5. [x] `::__builtin_memmove`: `act_on_id_expression_dn` declares the
   builtin for a bare `::` qualifier too (the builtin lives in the global
   namespace). Tests: `expr.prim.id.qual/global-qualified-builtin.cpp`,
   `global-qualified-builtin-error.cpp`.
6. `__get_iterator_concept` / `__do_test`:
   - 6b [x] `alias_template_type` (templ/class.rs) substitutes the aliased
     type inside `in_inst_context(<alias's context>)`, as variable
     templates already did, so `f<int>(T{})` in a member alias finds the
     class's `f` when the alias is used from elsewhere. This also removed
     the "`__get_iterator_concept` undeclared" half of 6a (a namespace
     alias `ranges::__iterator_concept` used from `std::_IterOps`). Going
     further exposed a panic (`constant_value` on a non-`ConstantExpr`):
     the subst `callee` ADL path attached substituted explicit template
     arguments to the rebuilt name without its dependence, so
     `g<I>()` with a still-dependent `I` instantiated `g`'s body; the
     name is now rebuilt with the arguments (type-dependent), and a
     value-dependent `if constexpr` condition in subst keeps both
     branches instead of panicking. Tests:
     `templates/alias/temp.alias-lookup-context.cpp` (the `__do_test`
     repro, plus class-template and namespace variants),
     `templates/alias/temp.alias-dependent-explicit-args.cpp`.
   - 6a [ ] OUT of scope: `'consteval' is not supported yet`
     (ranges_iterator_concept.h:31) — the function is now found, but its
     declaration is still rejected.

## Remaining blockers seen at the final probe (listed, not fixed)
- CTAD deduction guides, "expected unqualified-id" at the guide
  (`pair.h:484`, `subrange.h:197-210`, `array:384`, `span:604-620`,
  `string_view:694,699`, `_LIBCPP_CTAD_SUPPORTED_FOR_TYPE` at
  `string_view:680`) — WP-071b. All of `<utility>`'s last error.
- `consteval` functions (6a; `ranges_iterator_concept.h:31`).
- `__datasizeof(_Tp)` builtin not parsed (`datasizeof.h:31`, then
  "substitution into RecoveryExpr").
- `static_cast<decltype(_Tp::value)>(_Tp()) == _Tp::value` inside a
  template argument list ("expected '(' for function-style cast",
  `span:595`), cascading to `__integral_constant_like` unknown
  (`span:600-601`).
- `Base<float>::operator()(v)`: qualified operator-function-id after a
  template-id (`hash.h:434,444,485`).
- `switch` on a dependent-typed operand (`hash.h:62`).
- Local classes in templates (`hash.h:270`), literal operators
  (`string_view:906`), `__builtin_va_list`, `__builtin_constant_p`,
  `__is_assignable`, `__invoke` ambiguity (`find.h:48`),
  `__assign_trivially_copyable` no match (`constexpr_c_functions.h:221`).
- Not in the probe yet: a member template of an explicit specialization
  named through it (`template <> struct O<int> { template <class I>
  using c = I; }; O<int>::c<long> x;` → "expected unqualified-id");
  clang accepts.

## Status at end
Done: 1, 2, 3, 4, 5, 6b. Out of scope: 6a. See the corpus lines in the
handback.
