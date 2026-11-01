# WP-113 progress: explicit instantiation in codegen

Branch `wp/113-explicit-instantiation-codegen` (base main 80361733).

## Survey (what WP-158/WP-170 already cover)
Probes against clang 19 (aarch64 + x86_64) are exact for: `template struct X<int>;`
with virtuals, static data members (out-of-line, inline, constexpr), nested
classes, member templates (not instantiated), inline members, C5/D5 COMDATs
with `weak_odr` aliases, virtual bases, partial specializations, hidden
friends, defaulted members; `extern template struct Y<int>;` (external
vtable, no member definitions, implicit special members still emitted);
function/variable/static-member explicit instantiation definitions and
declarations; implicit-then-explicit re-emission as `weak_odr`.

## Gaps found (deliverables)
- [ ] G1 `extern template struct V<int>; ... template struct V<int>;` emits the
      members at the declaration (wrong order; callers become `nounwind`).
- [ ] G2 explicitly specialized members of an explicitly instantiated class:
      sema instantiates the pattern anyway (duplicate function bodies, the
      specialized static member gets the pattern's initializer, structors
      become `weak_odr`). Miscompile.
- [ ] G3 RAUW'd complete-object structor (`D1` -> `D2`): clang moves the base
      variant to the position of the later complete-object declaration
      (`applyReplacements`); kiln keeps the first position.
- [ ] G5 `template struct O<long>::I;` (member class of a specialization):
      nothing emitted.
- [ ] G6 explicit instantiation definition of a function template defined
      later in the TU: clang emits it at the end of the TU.
- [ ] spec/codegen.md section + GVA linkage table.

## Follow-ups (not in scope)
- sema: `template <> struct X<int>::N { void n2() {} };` is rejected.
- sema: `extern template int vv<long>;` (type mismatch) is not diagnosed.
