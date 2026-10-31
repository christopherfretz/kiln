# WP-117 progress — template mangling correctness + WP-106 follow-ups

## Done (uncommitted -> WIP commits)
- Headline bug root cause: codegen emitted *templated* out-of-line member
  definitions (pattern `A<T>::g`) as `_ZN1A1gEv`. Fix: `ModuleCx::is_templated`
  (walks ctx chain; template kinds / Pattern / PartialSpecialization /
  DependentFriend links) — skipped in `emit_members`.
- Implicit instantiations of static data members: `linkonce_odr` + comdat
  (clang GVA_DiscardableODR).
- IR struct names: enclosing specializations print their args
  (`%"struct.A<int>::B"`).
- Parser: `scan_qualifier` so `template<class T> A<T>::A()`, `A<T>::~A()` parse.
- `tests/mangle/out_of_line.cpp` (46 clang symbols).

- Qualified declarator-id lookup ([basic.lookup.unqual]/1): parser calls
  `Sema::act_on_enter_declarator_scope(q)` / `act_on_exit_declarator_scope(n)`
  around the declarator suffix (clang EnterDeclaratorScope). Fixes
  `S::vt S::f(vt)`, `void N::f(T)` (also non-template). Limitation: grouping
  parens `(S::f)(vt)` pop early; initializer `int S::x = k;` not yet in scope.
- Injected-class-name found via declarator scope (`injected_class_for` falls
  back to the scope stack).
- `rebuild_in_current_instantiation` (clang RebuildDeclaratorInCurrentInstantiation):
  `typename A<T>::vt A<T>::g(vt)` matches the in-class declaration.
- Partial-spec qualifiers `P<T*>::f`: nested-name template-id with
  entering_context resolves to the partial; `match_template_lists_to_scope`
  counts partials.
- `find_instantiated_decl` falls back to the first decl (out-of-line
  `template<class T> T A<T>::st;` instantiated body no longer NullStmt).
- Codegen miscompile (also on main): reference-returning call in rvalue
  context (LValueToRValue of a glvalue call) now loads through the reference.
- mangle_vs_clang: 628 clang symbols/triple (was 582).

## In progress
- IR diff of out_of_line.cpp vs clang: LLVM context-wide struct-name uniquing
  counter; explicit-specialization static member must be strong; extra memcpy.

## Gaps found (out of scope, for handback)
- codegen `UserDefinedConversion` cast unsupported (even non-template).
- `a.operator int()` -> "expected unqualified-id" (wp-121?).

## Next
- mangle_vs_clang green; IR corpus case; multi-TU run test.
- Deliverables 2–5.
