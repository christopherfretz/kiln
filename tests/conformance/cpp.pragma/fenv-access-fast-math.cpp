// `-ffast-math` (like any flag that turns off precise FP semantics)
// makes `#pragma STDC FENV_ACCESS ON` an error (clang `err_pragma_fenv_requires_precise`; WP-1744); the default
// flags accept it (cpp.pragma/ir-fenv-access.cpp).
//@ expect: compile-fail
//@ args: -ffast-math
//@ strict-diags
//@ diag: 9:14 error "'#pragma STDC FENV_ACCESS ON' is illegal when precise is disabled"
double f(double a) {
#pragma STDC FENV_ACCESS ON
  return a * 2.0;
}
