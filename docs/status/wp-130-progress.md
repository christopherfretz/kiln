# WP-130 progress — libc++ gate blockers, batch 2

Branch `wp/130-gate-blockers-2`, worktree `kiln/.worktrees/wp-130`, base
a9b245d.

Probe: `kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0` on a
file with one `#include`, error count (libc++ 19.1.7, aarch64).

| step | `<compare>` | `<utility>` | `<span>` | `<array>` | `<string_view>` |
|---|---|---|---|---|---|
| base | 0 | 1 | 13 | 8 | 23 |
| 1a `__datasizeof`, `__builtin_va_list` | 0 | 1 | 13 | 7 | 20 |
| 1b `__builtin_constant_p` | 0 | 1 | 13 | 7 | 19 |
| 1c `__is_assignable` | 0 | 1 | 13 | 7 | 17 |
| 2 named casts in template argument lists | 0 | 1 | 9 | 7 | 17 |
| 4 member templates of explicit/partial specializations | 0 | 1 | 9 | 7 | 17 |
| 3 qualified operator-function-ids | 0 | 1 | 9 | 7 | 17 |

Item 3 removed the three `hash.h:434/444/485` errors from `<string_view>`;
parsing past them exposed three more local-class-in-template errors
(`hash.h:245/258/270`), so the count is unchanged.

## Items
1. [x] `__datasizeof(T)` (the record's data size, else `sizeof`);
   `__builtin_va_list` predeclared for the target (AArch64 `std::__va_list`,
   x86-64 `__va_list_tag[1]`); `__builtin_constant_p(e)` folded by the
   Oven through a speculative sub-run (`Op::Speculate`: a `NotConstant`
   fault answers 0, other faults propagate; 0 at run time);
   `__is_assignable(T, U)` (an unevaluated `declval<T>() = declval<U>()`
   under SFINAE). Tests in `tests/conformance/templates/builtins/`.
2. [x] `static_cast<T>(e)` (and `dynamic_`/`reinterpret_`/`const_cast`)
   inside a template argument list: the lookahead scan treats the cast's
   `<` as opening its own list (`span:595`). Test
   `templates/parse/temp.names-named-cast-arg.cpp`.
3. [x] `N::operator@` / `X<T>::operator T` as id-expressions
   (`B<float>::operator()(v)`, `&S::operator+`, `N::operator*(a, b)`);
   `DependentScopeDeclRefExpr` carries a `DeclName` so the dependent form
   resolves at instantiation. Test
   `expr.prim.id.qual/qualified-operator-function-id.cpp`.
4. [x] `O<int>::c<long>` where only the explicit (or a partial)
   specialization declares `c`: the parser's lookahead classifier falls
   back to the specializations when the primary pattern lacks the name.
   Test `templates/spec/temp.expl.spec-member-template-named.cpp`.
   The `expected unqualified-id` errors at `pair.h:484`, `subrange.h`,
   `span:604-620`, `array:384` and `string_view:680-699` are not this
   pattern: they are all CTAD deduction guides (WP-071b).

## New blockers (listed, not fixed)
- `<string_view>`: local classes in templates (`hash.h:245/258/270`, 4
  errors); dependent `switch` (`hash.h:62`); `consteval`
  (`ranges_iterator_concept.h:31`, also `<array>`); `__is_constructible`
  (`is_constructible.h:24`); `__invoke` ambiguity (`find.h:48`); literal
  operators (`string_view:906`).
- Codegen: a concept-id evaluated at run time (`return K<int>;`) is
  "ConceptSpecializationExpr is not supported yet".
- Parser: `C<sizeof(T) >= 4>` in an expression closes the list at `>=`
  (clang parses `>=` as an operator in an expression argument).
- Default arguments that depend on a function template's parameters
  (`template<class T> void f(C<sizeof(T)> = {}); f<int>();`) find no
  viable function.
- `__builtin_constant_p` speculation does not roll back writes made by
  called functions, nor reclaim heap allocations, when it answers 0.
