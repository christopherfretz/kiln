# WP-133 progress (`consteval` / immediate functions)

Status: complete, ready for review. Branch `wp/133-consteval`.
Design and gaps: spec/sema.md §5.10 (the brief's "§5.6" is `typeid`; the
section was added as §5.10).

| deliverable | status |
|---|---|
| 1. accept `consteval` (functions, members, ctors, conversions, lambdas, templates), dump, misuse errors | done; consteval virtual functions are allowed ([class.virtual]/19, P1064), so the brief's "virtual is an error" was not implemented (`class.virtual/consteval-override.cpp` now passes) |
| 2. immediate invocations: `eval_const` + `EvalKind::Consteval`, `ConstantExpr`, clang's errors/notes, immediate function contexts, P2564 escalation, address-of error | done |
| 3. consteval calls in templates checked at instantiation | done (non-dependent ones at the definition, as clang) |
| 4. libc++ probe | see below |
| 5. tests, spec, this file | 11 new tests (below); spec/sema.md §5.10, spec/ast-dump.md `ConstantExpr` row |

## Tests

| file | covers |
|---|---|
| `ast/dcl.constexpr/consteval-decls.cpp` | parse/dump of every consteval declaration form |
| `ast/dcl.constexpr/consteval-aggregate.cpp` | folded scalar and class-type invocations, nested ones unwrapped, deferred (unvalued) wrappers in a constant initializer |
| `ast/dcl.constexpr/consteval-if.cpp` | `if consteval` / `if !consteval`: no wrapping in the consteval branch |
| `dcl.constexpr/consteval-decl-errors.cpp` | variable, field, destructor, `main`, parameter, `consteval constexpr`, redeclaration mismatches |
| `dcl.constexpr/consteval-not-constant.cpp` | not-constant errors with Oven notes (unknown parameter, division by zero, non-const global, member function) |
| `dcl.constexpr/consteval-ctor.cpp` | consteval constructors: direct, functional cast, copy-init, braced, temporaries |
| `dcl.constexpr/consteval-lambda.cpp` | consteval lambda; a lambda escalated by calling one; a capturing one |
| `dcl.constexpr/consteval-template.cpp` | dependent invocation checked per instantiation; non-dependent and NTTP ones fold |
| `dcl.constexpr/consteval-escalation.cpp` | P2564: escalated specializations with the "because" note, two levels |
| `dcl.constexpr/consteval-address.cpp` | address of consteval functions and `&C::m`; allowed inside immediate contexts |
| `dcl.constexpr/consteval-run.cpp` | run test vs clang: scalar, aggregate (small and memset-sized), constructor, lambda, `if consteval` at run time and in a constant |

The `xfail(kiln)` lines of `class.virtual/consteval-override.cpp` and
`ast/dcl.constexpr/consteval-calls.cpp` were replaced by comments (they pass).

## Fixes outside consteval found on the way

- `function parameter cannot be constexpr|consteval|constinit` was not diagnosed.
- The `CXXConstructExpr` of `T(e)` / `static_cast<T>(e)` / `(T)e` was located at the
  operand; clang locates it at the cast's start (affects every diagnostic on it).
- The Oven said "read of non-const variable" for a non-const variable of class type;
  clang says "non-constexpr" unless the type is integral or enumeration.
- The `cargo xtask clang-ast` oracle now accepts unvalued prvalue `ConstantExpr`s and
  aggregate values of integers (`{3, 4}`), and maps `isImmediateEscalating`.

## libc++ probe (errors per header, `-fsyntax-only`)

| build | `<compare>` | `<utility>` | `<span>` | `<array>` | `<string_view>` |
|---|---|---|---|---|---|
| main 10156ed (before) | 0 | 0 | 4 | 2 | 15 |
| wp-133 before merge | 0 | 0 | 4 | 1 | 14 |
| wp-133 + main e42ea07 (after) | 0 | 0 | 0 | 0 | 8 |

The consteval error at `ranges_iterator_concept.h:31` is gone. The other drops
come from WP-130 (datasizeof, builtins). No new blockers. These are the
remaining `<string_view>` errors, none of them consteval:
- `__is_constructible` is not implemented.
- The `__invoke` call is ambiguous (`find.h:48`).
- Literal operators (`string_view:906`).
- Local class declarations in templates (`hash.h` ×4).
- A dependent `_Size` switch condition (`hash.h:62`).

## Deferred / open

- Default-argument and default-member-initializer invocations are elided at the
  definition and not checked at the use (clang checks them where used).
- Functions called only from non-dependent expressions of a template (shared with
  its instantiations) dump `referenced` rather than `used` (pre-existing).
- Defining a constructor does not mark the class's virtual functions `used`
  (pre-existing, not consteval-specific; kept out of the dump test).
- An implicit `operator=`'s `DeclRefExpr` type lacks `noexcept` (pre-existing).
