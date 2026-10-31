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

## In progress
- Injected-class-name / member typedef lookup in parameter types of
  out-of-line class-template member declarators (`A<T>::f(const A&)`).

## Next
- mangle_vs_clang green; IR corpus case; multi-TU run test.
- Deliverables 2–5.
