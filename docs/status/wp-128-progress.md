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
4. [ ] `::new (p) T(args...)`
5. [ ] `::__builtin_memmove`
6. [ ] `__get_iterator_concept` / `__do_test`
