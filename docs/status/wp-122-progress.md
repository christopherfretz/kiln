# WP-122 progress — deduced returns in templates, structured bindings

Branch `wp/122-structured-bindings`, worktree `kiln/.worktrees/wp-122`,
base a1e8e9c (WP-075 merge, kiln-mode 1561/1577).

## Steps
1. [x] Item 1: deduced return types in templates ([dcl.spec.auto.general]/12)
2. [x] Structured bindings: parser + sema (array, tuple-like, members)
3. [x] Oven + codegen for bindings
4. [x] Tests (`tests/conformance/dcl.struct.bind/`: 28) + spec
5. [ ] `git merge main`, final full CI

## Merge note (item 1)
Item 1 is complete and green on its own at commit **4abe197**
(`cargo xtask ci --quick`: 1565 passed / 16 xfail / 1581 tests; base
1561/1577). Merge 4abe197 if structured bindings are still in progress:
structured bindings are now complete too (step 4); merge the branch
head once the final CI is green.

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
- Structured bindings (sema.md §4.9): the hidden variable `e` is an
  unnamed `Var` + cold `decomp_bindings`; `DeclKind::Decomposition`
  (unused) was removed. Array / tuple-like (member and ADL `get`,
  holding vars) / data members (bases rule, bit-fields, access),
  reference forms, conditions (`-Wbinding-in-condition`), range-for,
  lambda captures, namespace scope (static, thread_local), templates
  (re-declared from `decomp_written`). Oven: holding vars evaluated
  after `e`; codegen: `emit_var_decl`, `_ZDC...E` mangling;
  `tests/codegen/structured_bindings.cpp` exact on 2/2 triples.
- Tests: 28 in `tests/conformance/dcl.struct.bind/` (9 AST goldens,
  12 strict diag, 1 constexpr compile-ok, 6 run); flipped
  `stmt.ranged/structured-binding.cpp`. Corpus (kiln mode, -j 2):
  1594 passed / 0 failed / 15 xfail / 1609 tests.
- Fixed on the way: `reference to type 'X' could not bind to an
  rvalue|lvalue of type 'Y'` (`sema_reference_bind_failed`), qualified
  name in `implicit instantiation of undefined template 'N::X<int>'`
  (`decl_ty_str`), clang-ast normalizer: `DecompositionDecl`,
  `BindingDecl`, `CallExpr adl`, bitfield on a binding `DeclRefExpr`.
- Open (pre-existing, not WP-122): local `thread_local` not emitted as
  TLS; constant-initialized reference global emitted `global` not
  `constant`; dynamic init of statics/globals (M3); missing "in call to
  'S(s)'" note for trivial copy ctors; diagnostic ranges; "inline
  declaration not allowed in block scope"; `P const &` sugar in
  instantiations; general ADL marking; binding attributes not
  instantiated; NoOp `'const M'` cast for const class copy-init from a
  prvalue; spurious aka for namespace-member types in overload notes;
  M3 constants with base classes or bit-fields; clang 19 rejects a
  constexpr lambda capture of bindings, kiln accepts it.
