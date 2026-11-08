// [driver] The MSVC barrier and hint builtins `__dmb`, `__dsb`, `__isb`,
// `__yield`, `__wfe`, `__wfi`, `__sev` and `__sevl` (clang 19
// `BuiltinsAArch64.def` LANGBUILTINs, ALL_MS_LANGUAGES) exist only under
// `-fms-extensions`, which kiln++ does not accept: outside it they are
// undeclared and `__has_builtin` denies them, as clang 19's, so the
// `arm_acle.h` macros and inline functions stand in (clang 22 declares
// them as `arm_acle.h` header builtins in every mode; kiln ships clang
// 19's header and keeps clang 19's answer at every compatibility level)
// (spec/pp.md "Feature queries"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ verify
static_assert(!__has_builtin(__dmb) && !__has_builtin(__dsb) && !__has_builtin(__isb));
static_assert(!__has_builtin(__yield) && !__has_builtin(__wfe) && !__has_builtin(__wfi));
static_assert(!__has_builtin(__sev) && !__has_builtin(__sevl));
void f() {
  __dmb(1); // expected-error {{use of undeclared identifier '__dmb'}}
  __yield(); // expected-error {{use of undeclared identifier '__yield'}}
}
