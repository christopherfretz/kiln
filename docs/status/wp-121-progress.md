# WP-121 progress (operator/destructor/qualified member access, dependent NNS, friend templates)

Updated: 2026-10-31

## State: item 1 done; items 2-5 not started

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

### Pending
- 2: qualified member access `e.N::m`, `this->Base::x`, `e.::N::m`.
- 3: dependent NNS chains rebuilt on substitution; missing typename/template diags.
- 4: friend templates.
- 5: dependent-context member references (parameters in a trailing
  return type: `-> decltype(t.f())` fails substitution in
  `find_instantiated_decl`).

## Evidence
- Item 1 `cargo xtask ci --quick`: corpus (kiln) 1500 passed; 0 failed; 15 xfail; 0 xpass (1515 tests; baseline 1496/1511).
