// clang's `#pragma clang fp eval_method` under `-ffast-math`: one error
// for each value-changing option it conflicts with (`-fapprox-func`,
// reassociation, reciprocals), at the pragma's `fp`
// (`err_setting_eval_method_used_in_unsafe_context`, WP-1744). The
// options alone are fine (fp-eval-method-errors.cpp has the pragma form).
//@ expect: compile-fail
//@ requires: clang>=22
//@ args: -std=c++26 -ffast-math
//@ strict-diags
//@ diag: 14:15 error "'#pragma clang fp eval_method' cannot be used with option 'fapprox-func'"
//@ diag: 14:15 error "'#pragma clang fp eval_method' cannot be used with option 'mreassociate'"
//@ diag: 14:15 error "'#pragma clang fp eval_method' cannot be used with option 'freciprocal'"
float f(float a, float b) {
#pragma clang fp eval_method(double)
  return a * b;
}
float g(float a, float b) { return a * b; }
