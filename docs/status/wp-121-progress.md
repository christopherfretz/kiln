# WP-121 progress (operator/destructor/qualified member access, dependent NNS, friend templates)

Updated: 2026-10-31

## State: items 1-4 done (WIP commits on the branch); item 5 in progress

**Item 1 is green and can merge early.** Its commit is `66ab349`, and quick CI
on it was 1500 passed / 0 failed / 15 xfail. Every later WIP commit also kept
quick CI green. The branch is never left red.

| item | commits | state |
|---|---|---|
| 1 member access by declared name | 66ab349 | done, green |
| 2 qualified member access | d0a8a38, 82f9e06, 5599dbf | done, green |
| 3 dependent NNS chains, typename/template diags | db0fcfd, 01dc8b5 | done, green |
| 4 friend templates ([temp.friend]) | c936c33, 4af684f, b9338cb | done (quick CI re-run after the restart) |
| 5 trailing-return-type member refs | - | in progress |

### Item 1: member access by declared name (done)
- `x.operator@(…)`, `x.operator T()`, `x.operator->()`, and unqualified
  `operator@(…)` / `operator T()` inside members (implicit `this->`).
- `p->~T()` / `x.~T()` on classes (a `CXXMemberCallExpr` of the
  destructor) and on scalars or dependent objects (new
  `CXXPseudoDestructorExpr`; its call is `CallExpr 'void'`). clang's
  destructor and pseudo-destructor errors match byte for byte.
- Interface change: the member name in `CXXDependentScopeMemberExpr` and
  `UnresolvedLookupExpr` is now a `DeclName` (`dependent_member_name`,
  `unresolved_name`). `act_on_member_dn`, `act_on_id_expression_dn`,
  `act_on_call_name_dn`, and `act_on_undeclared_call_name_dn` take a
  `DeclName`; the `Symbol` forms wrap them.
- Oven and codegen: a pseudo-destructor call evaluates its object
  expression and does nothing else. The oven does not track the end of
  lifetime here, the same as for an explicit destructor call.
- The `[temp.over.link]` profile has a rule for `CXXPseudoDestructorExpr`.
- Tests:
  - `tests/conformance/expr.ref/{operator-function-member,pseudo-destructor}.cpp` (run)
  - `destructor-name-errors.cpp` (strict-diags)
  - `tests/conformance/ast/expr.ref/operator-and-destructor-members.cpp` (clang-19 golden)

### Item 2: qualified member access (done)
- `e.N::m`, `p->B::f()`, `this->Base::x`, and dependent qualifiers.
- Tests: `expr.ref/qualified-member{,-errors}.cpp` and
  `ast/expr.ref/qualified-member.cpp`.
- Flipped the `class.member.lookup/ambiguous.cpp` xfail.

### Item 3: dependent NNS chains (done)
- A dependent NNS that is still dependent is rebuilt on substitution.
- `find_instantiated_decl` keeps a member of a parent that depends only on
  retained levels.
- Unnamed NTTP packs.
- The missing `typename`/`template` diagnostics use the canonical context.
- Tests: `temp.res/{dependent-nns-chain,missing-typename-template}.cpp` and
  `ast/temp.res/dependent-nns-chain.cpp`.

### Item 4: friend templates (done)
- Forms: `template<class U> friend class X;`, `template<class U> friend
  R f(U);`, and friend specializations `friend R f<>(...)`, `friend bool
  operator==<>(...)`, `friend R N::g<T>(...)`.
- Instantiated per specialization. A dependent friend specialization is a
  `FriendSpec` that is matched when its class is instantiated.
- Access checks honour them, including inside the definition of a
  befriended function template.
- clang's errors: no candidate for a dependent friend specialization (with
  a not-a-template note), no matching template, and access.
- Template redeclarations share one identity: `Ast::first`/`latest` resolve
  a template through its pattern's redeclaration group.
- Inherited default template arguments ([temp.param]/12) are now
  supported.
- Deduction completes a dependent `P` with its defaulted arguments
  ([temp.deduct.type]).
- Tests: `temp.friend/{friend-templates,friend-template-access,
  friend-specializations,friend-spec-errors}.cpp`,
  `ast/temp.friend/friend-templates.cpp`,
  `temp.param/inherited-defaults.cpp`, and
  `templates/deduce/temp.deduct.type-defaulted-args.cpp`.

### Item 5 (in progress)
- A trailing return type in a template body already works
  (`-> decltype(t.f())`, `-> decltype(v + u)` in a member template, a
  nested class template).
- Bug: in an out-of-line member definition (`auto S::f() -> decltype(v)`,
  `auto S::g() -> T`), the trailing return type is not looked up in the
  class scope. The result is "use of undeclared identifier" and then "does
  not match any declaration". Fixing this next.

## Evidence
- Item 1 `cargo xtask ci --quick`: corpus (kiln) 1500 passed; 0 failed; 15 xfail; 0 xpass (1515 tests; baseline 1496/1511).
- libc++ probe (`#include <h>`, `-fsyntax-only -ferror-limit=0`, error count):

  | header | baseline | after 1 |
  |---|---|---|
  | array | 83 | 73 |
  | string_view | 147 | 136 |
  | span | 62 | 54 |
  | utility | 23 | 22 |

  First blockers after item 1:
  - array: `__iterator/concepts.h:128` dependent nested name specifier (item 3).
  - string_view, span, utility: `common_comparison_category.h:44` range-for (WP-089),
    then lambdas (WP-075).
  - span: `bounded_iter.h:99` friend templates (item 4).
