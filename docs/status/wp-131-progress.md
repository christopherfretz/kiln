# WP-131 progress — libc++ gate blockers, batch 3 (Sema)

Branch `wp/131-gate-blockers-3`, worktree `kiln/.worktrees/wp-131`.
Status: complete, ready for review.

Probe: `kiln -std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0` on a
file with one `#include`, error count (libc++ 19.1.7, aarch64).

| step | `<compare>` | `<utility>` | `<span>` | `<array>` | `<string_view>` | `<optional>` | `<tuple>` |
|---|---|---|---|---|---|---|---|
| base | 0 | 0 | 0 | 1 | 9 | — | — |
| 2, 4, 5 (dependent `switch`, `__is_constructible`, CWG1395, traits) | 0 | 0 | 0 | 1 | 6 | — | — |
| 1 local classes in templates | 0 | 0 | 0 | 1 | 2 | — | — |
| 3 literal operators | 0 | 0 | 0 | 1 | 1 | — | — |
| merge main (WP-133 `consteval`) | 0 | 0 | 0 | 0 | 0 | 25 | 23 |

The `<array>` error and the last `<string_view>` one were `consteval`
(`ranges_iterator_concept.h:31`), fixed by WP-133. All five WP-130/131
gate headers are now clean.

## Items
1. [x] Local classes (and their members) in templated functions:
   instantiated with the enclosing function, internal linkage, the
   Itanium `Z...E` local-entity mangling with discriminators (`hash.h`).
   Tests `templates/class/temp.inst-local-class.cpp`,
   `codegen/local_class_template.cpp`; spec/abi.md, spec/sema-templates.md.
2. [x] `switch` on a type-dependent condition: checked at instantiation
   (`hash.h:62`). Tests `stmt.switch/dependent-condition.cpp`,
   `stmt.switch/dependent-condition-inst-fail.cpp`.
3. [x] Literal operators and user-defined literals: `operator""_x`
   declarations (both `"" _x` and `""_x`), clang's
   `CheckLiteralOperatorDeclaration` checks (scope, `extern "C"`,
   parameter lists, templates, default arguments, reserved suffixes),
   and the [lex.ext] form selection for integer, floating, character and
   string literals (raw and template forms, the class-NTTP and GNU
   `<class C, C...>` string templates). The call is a plain `CallExpr`
   (clang builds `UserDefinedLiteral`; deviation in spec/sema-overload.md
   §6.7). `"abc"sv` / `u8"xy"sv` compile and run. Tests
   `lex.ext/user-defined-literal-call.cpp`,
   `over.literal/literal-operator-decl.cpp`,
   `codegen/user_defined_literals.cpp`.
4. [x] `__is_constructible` / `__is_nothrow_constructible` (and on the
   way `__is_convertible`, `__is_nothrow_convertible`,
   `__is_nothrow_assignable`). Tests in `templates/builtins/`.
5. [x] `__invoke` ambiguity (`find.h:48`): partial ordering with
   function parameter packs per CWG1395
   (`templates/order/temp.func.order-packs.cpp`).
   `__assign_trivially_copyable`: unevaluated operands are no longer
   evaluated when rebuilt at instantiation
   (`templates/diag/temp.inst-unevaluated-operand.cpp`,
   `templates/sfinae/assign-trivially-copyable.cpp`).
6. [x] Re-probe (above); `libcxx-syntax.md` updated.

## New blockers (listed, not fixed)

`<optional>` (25 errors):
- Inheriting constructors from a dependent base,
  `using __base::__base;` (`optional`): "using declaration refers into
  '...::', which is not a base class of ..." (11), with follow-ons
  "use of undeclared identifier 'reset'/'has_value'" and "constructor
  cannot be redeclared" (3).
- `decltype(std::declval<const _Tp&>() > std::declval<const _Up&>())`:
  a `>` operator inside parentheses inside a template argument list is
  taken as the list's closer, "expected ')'" (5).

`<tuple>` (23 errors):
- A pack expansion in a mem-initializer list,
  `__tuple_leaf<_Uf, _Tf>(std::forward<_Up>(__u))...`: "expected '(' or
  '{'" (4).
- `sizeof...(_Tp) >= 1` in a template argument: "expected '>'".
- A member-template partial specialization inside a class template
  (`template <class _Up> struct _IsThisTuple<_Up>`): "expected member
  name or ';'" (3).
- `typename tuple<common_reference_t<_TQual<_TTypes>,
  _UQual<_UTypes>>...>` in a requires-expression (`tuple:1020-1028`):
  "partially substituted pack expansion" not implemented (4) and a
  spurious "pack expansion does not contain any unexpanded parameter
  packs" (4).
- A functional cast whose type is a template-id with a pack expansion,
  `tuple<_Tp&...>(__t...)`: "expected ')'" (5).
