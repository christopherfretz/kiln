# WP-121 progress (operator/destructor/qualified member access, dependent NNS, friend templates)

Updated: 2026-10-31

## State: items 1-5 done; final full `cargo xtask ci` green at 2ac86ea

**Item 1 is green and can merge early.** Its commit is `66ab349`, and quick CI
on it was 1500 passed / 0 failed / 15 xfail. Every later WIP commit also kept
quick CI green. The branch is never left red.

| item | commits | state |
|---|---|---|
| 1 member access by declared name | 66ab349 | done, green |
| 2 qualified member access | d0a8a38, 82f9e06, 5599dbf | done, green |
| 3 dependent NNS chains, typename/template diags | db0fcfd, 01dc8b5 | done, green |
| 4 friend templates ([temp.friend]) | c936c33, 4af684f, b9338cb, e2057bb | done, green |
| 5 declarator parameters / trailing requires-clause / out-of-line scope | f439d2e, 271b0c0, 5b0b7a4, 2786912, ba13c42 | done, green |

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

### Item 5: declarator parameters and out-of-line scope (done)
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
- **Out-of-line declarator scope** ([dcl.meaning]/1, [basic.scope.class],
  clang `ActOnCXXEnterDeclaratorScope`/`EnterDeclaratorContext`).
  - After a qualified declarator-id `N::f` / `S::f` / `C<X>::f`, the
    scopes from the qualifier's context down to the current context are
    pushed, and `Sema.ctx` becomes the qualifier's definition until the
    declarator ends. Parameters, array bounds, the trailing return type,
    `noexcept` and `this` are then looked up in the class or namespace.
  - The same scope is entered for the initializer of a qualified variable
    (`ActOnCXXEnterDeclInitializer`: `int S::v = k;`,
    `const T C<X>::y = z + 1;`) and for a trailing requires-clause.
  - Current-instantiation rebuild (clang
    `RebuildDeclaratorInCurrentInstantiation`): a decl-spec type
    `typename C<X>::T` in an out-of-line member of `C<X>` resolves to the
    member `T`, so `typename C<X>::T C<X>::h(typename C<X>::T)` matches
    its in-class declaration. Only a top-level decl-spec type is rebuilt
    (a gap; nested positions are not).
  - An out-of-line static data member whose initializer names another
    out-of-line-defined member (`z` in `C<X>::y = z + 1`) found the
    pattern's redeclaration of `z`; instantiation now maps it through its
    first declaration (`find_instantiated_decl`).
  - The satisfaction of a member's trailing requires-clause maps pattern
    contexts to the specialization's (`inst_context_pairs`), so
    `requires(sizeof(V) > 1)` with a member typedef `V` works.
  - Tests: `dcl.meaning/qualified-declarator-scope.cpp` (run, namespace
    and class members) and `qualified-declarator-scope-template.cpp`
    (compile-ok; a class template's out-of-line members). It is
    compile-ok rather than run because of a pre-existing codegen panic,
    below.
- **Dependent builtin template-id as an NNS** (ba13c42):
  `typename __make_integer_seq<S, T, E - S>::template to<S>` (libc++
  `__make_indices_imp`) is a dependent context. Test:
  `temp.res/dependent-builtin-template-nns.cpp`.
- **WP-116 `temp.over.link-head-locations` xfail flipped** (ba13c42): an
  unnamed template parameter is located at the token where its name
  would be (clang's `NameLoc`), and a type parameter's `class` /
  `typename` key is recorded as its begin location.
- **Gap, pre-existing on main:** codegen panics at
  `kiln_abi/src/irtype.rs:489` when a program defines a member of a class
  template out of line and the object file is emitted (`emit_global_fn`
  reaches the dependent pattern). `-fsyntax-only` and compile-ok are
  unaffected.

## Evidence
- Full `cargo xtask ci` at 2ac86ea: fmt, clippy `-D warnings`, `cargo
  test --workspace` all green; corpus (clang++-19 self-check) 1508
  passed / 0 failed / 3 xfail / 26 skipped (baseline 1482/1511); corpus
  (kiln) 1524 passed / 0 failed / 13 xfail (baseline 1496/1511).
- Corpus (kiln, `-j 2`) at ba13c42: 1524 passed; 0 failed; 13 xfail; 0
  xpass (1537 tests; baseline 1496/1511 kiln-mode). The new tests pass on
  clang 19 as well.
- libc++ probe at ba13c42 (error counts): array 37, string_view 53,
  span 21, utility 12 (baseline 83/147/62/23).
  - Remaining first blockers, none in WP-121's scope: `new:298` (a void
    function returning a void call), `construct_at.h:38` (`::new`
    parsing), `common_comparison_category.h:44` range-for (WP-089) and
    its deduced-return fallout, lambdas (WP-075), deduction guides,
    `consteval`, literal operators, using-declaration conflicts
    (`using ranges::get;`), braced arguments in a dependent call.

### Earlier evidence
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
