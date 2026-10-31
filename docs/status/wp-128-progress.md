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
5. [ ] `::__builtin_memmove` — not started. Diagnosis: `act_on_id_expression_dn`
   (expr.rs ~778) calls `lazily_create_builtin` only when `qualifier.is_none()`;
   allow it when the qualifier is the TU (`::`).
6. [ ] `__get_iterator_concept` / `__do_test` — not started. Diagnosis:
   `__get_iterator_concept` is the 'consteval' cascade (ranges_iterator_concept.h:31).
   `__do_test`: repro `struct B { template <class Tr, class T> static T f(T);
   template <class T> using ok = decltype(f<int>(T{})); };
   template <class T> struct C { B::ok<T> m; }; C<int> c;` — an unqualified
   template-id callee with ADL is looked up again from the instantiation
   context (subst/exprs.rs `callee`, `act_on_call_name_dn`), which lacks
   B's scope when the alias is instantiated from outside B. Without `<int>`
   it works.

## Status at stop (host reset)
Done: 1, 2, 3, 4. Not started: 5, 6. Full `cargo xtask ci` not run.
