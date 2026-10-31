# WP-122 progress — deduced returns in templates, structured bindings

Branch `wp/122-structured-bindings`, worktree `kiln/.worktrees/wp-122`,
base a1e8e9c (WP-075 merge, kiln-mode 1561/1577).

## Steps
1. [x] Item 1: deduced return types in templates ([dcl.spec.auto.general]/12)
2. [ ] Structured bindings: parser + sema (array, tuple-like, members)
3. [ ] Oven + codegen for bindings
4. [ ] Tests (`tests/conformance/dcl.struct.bind/`, ≥ 20) + spec

## Merge note (item 1)
Item 1 is complete and green on its own at commit **4abe197**
(`cargo xtask ci --quick`: 1565 passed / 16 xfail / 1581 tests; base
1561/1577). Merge 4abe197 if structured bindings are still in progress:
the later `WP-122 WIP:` commits (structured bindings) are mid-work and
do not build yet.

## Item 1 — header probe
`kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`, errors:

| header | base | after item 1 |
|---|---|---|
| `<array>` | 78 | 71 |
| `<string_view>` | 142 | 135 |
| `<span>` | 59 | 56 |
| `<compare>` | 3 | 0 |
| `<utility>` | 20 | 17 |

New first blockers:
- `<array>`, `<span>`, `<string_view>`: `__iterator/iterator_traits.h:252`
  `expected unqualified-id` — `.operator->()` member access in a
  requires-expression / decltype (wp-121's area, `expr.rs` postfix).
- `<utility>`, `<string_view>`: `__utility/integer_sequence.h:22`
  `template <size_t...>` — an unnamed NTTP pack whose type is a
  typedef is not parsed (`expected ',' or '>'`).
- Also seen, pre-existing: `void function '__do_deallocate_handle_size'
  should not return a value` (`<new>`: `return f(args...)` of a void
  function template call in a void function).

## Log
- Item 1: `begin_function_body` gives a templated function's pattern
  the return type `dependent_auto(declared)`; `deduce_return` defers in
  any dependent context; `act_on_return` skips missing/void checks
  against a dependent return type; no NRVO candidate in the pattern;
  canonical dependent `auto` stays `auto` (mangles `Da`). Also fixed:
  `x.g()` to an undeduced member of a class template specialization,
  and codegen of a glvalue call used as a prvalue (`int r = g(i)` with
  `int& g(int&)`, was a ptr store / IR type error).
- Open: an out-of-line member definition of a class template
  (`template <class T> auto S<T>::s() {...}`) is emitted by codegen as
  a non-template function `_ZN1S1sEv` (pre-existing at base, also for
  non-`auto` returns).
