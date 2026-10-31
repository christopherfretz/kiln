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
| 4 friend templates ([temp.friend]) | c936c33, 4af684f, b9338cb, e2057bb | done, green |
| 5 declarator parameters / trailing requires-clause / out-of-line scope | f439d2e, next | in progress |

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

### Item 4 follow-ups (e2057bb)
- **AST dumper is cycle-safe in general** (coordinator ask 1). The dumper
  keeps the chain of declarations it is inside. A declaration reached
  again inside itself prints ` <cycle>` and is not descended into. Depth is
  also capped at 4096. This fixes the 2 -> 24 GB blow-up that the friend
  template redeclaration cycle (`A` befriends `B`, `B` befriends `A`)
  caused through the `template_children` specialization lists.
- A friend class template naming an existing template is a real
  redeclaration in the template's group. It does not mark the original
  template dependent. Before this fix, `C<char>` inside `A<T>` became
  dependent.
- Specializations are dumped under the first non-friend declaration
  (`spec_owner`), as clang does. `friend_undeclared` is printed only on a
  direct friend introduction.
- A class first introduced by a friend declaration (a hidden friend,
  [namespace.memdef]/3) becomes visible at a later namespace-scope
  declaration, `friend class X; ... class X; X *p;`. This affected
  libc++'s `basic_string<char>`.
- The injected-class-name of a class template used as a functional cast,
  `X(args)` / `X{args}`, works (libc++ `reverse_iterator::operator+`).
- Tests:
  - `temp.friend/{friend-template-cycles,injected-functional-cast}.cpp` (run)
  - `ast/temp.friend/{friend-template-cycles,friend-declared-later}.cpp`
    (clang-19 goldens)
- **Memory-budgeted variant: not possible yet** (coordinator ask 2). The
  harness has no per-test RSS or address-space limit; WP-125 tracks that.
  `oven:` budgets only constant evaluation, and `tests/stress/` is not
  discovered by the harness (`xtask/src/test/discover.rs` walks
  `tests/conformance` and `tests/differential` only).
  - So the regression tests stay in `tests/conformance/{ast/,}temp.friend/`,
    `friend-templates.cpp` among them.
  - Once WP-125 lands, `tests/conformance/stress/` is the place for an RSS
    budget variant, for example a wide friend cycle under `expect: ast`.
  - Until then every manual kiln run and corpus run goes under
    `ulimit -v 8388608`.

### Item 5 (in progress)
- **Trailing requires-clause** (f439d2e and the next commit). The function
  parameters, and `this` in a member, are in scope again in a trailing
  requires-clause ([basic.scope.param]/1, [expr.prim.this]/2).
  - Satisfaction checking substitutes the parameters' types into fresh
    parameters, the same as clang's `addInstantiatedParametersToScope`.
    It also sets the specialization's `this` type.
  - This was libc++'s `reverse_iterator.h:209` blocker: `operator==`
    `requires requires { __x.base() == __y.base(); }`.
  - Tests: `basic.scope.param/trailing-requires-params.cpp` (run; function
    templates, a member template using `this`, a member of a class
    template) and `trailing-requires-params-fail.cpp` (strict-diags,
    identical to clang).
- **A trailing return type in a template body already works**:
  `-> decltype(t.f())`, `-> decltype(v + u)` in a member template, and a
  nested class template.
- **Next: out-of-line member declarators.** In `auto S::f() -> decltype(v)`
  and `auto S::g() -> T`, the parameters, array bounds and trailing return
  type after the declarator-id `S::f` must be looked up in `S`'s scope
  ([basic.scope.class], [dcl.meaning]).

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
