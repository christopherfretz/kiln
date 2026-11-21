// [cfenv.syn], C23 7.6.2: kiln implements `#pragma STDC FENV_ACCESS`, the
// exception behavior pragmas and `FENV_ROUND FE_TONEAREST|FE_DYNAMIC`
// (WP-1744), but not a directed static rounding mode (`FE_UPWARD`, ...:
// constant evaluation and constrained operations in that mode) or an
// evaluation method other than the source's (`clang fp
// eval_method(double|extended)`): those are errors, never ignored.
// FENV_ROUND also has clang's "not supported" warning (WP-806).
//@ expect: compile-fail
//@ skip(clang): kiln rejects the floating-point environment pragmas clang implements
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 21:34 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 21:34 error "'#pragma STDC FENV_ROUND FE_UPWARD' (the floating-point environment) is not supported yet"
//@ diag: 22:15 error "'#pragma clang fp eval_method(double)' (the floating-point environment) is not supported yet"
//@ diag: 23:37 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 24:35 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 31:38 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 31:38 error "'#pragma STDC FENV_ROUND FE_TOWARDZERO' (the floating-point environment) is not supported yet"
//@ diag: 36:15 error "'#pragma clang fp eval_method(extended)' (the floating-point environment) is not supported yet"
// The pragmas are reported where they take effect.
#pragma STDC FENV_ROUND FE_UPWARD
#pragma clang fp eval_method(double)
#pragma STDC FENV_ROUND FE_TONEAREST
#pragma STDC FENV_ROUND FE_DYNAMIC
#pragma clang fp eval_method(source)
#pragma STDC FENV_ACCESS ON
#pragma float_control(except, on)
#pragma clang fp exceptions(maytrap)
#pragma STDC FENV_ACCESS OFF
double f(double a, double b) {
#pragma STDC FENV_ROUND FE_TOWARDZERO
  return a + b;
}

double g(double a, double b) {
#pragma clang fp eval_method(extended)
  return a * b;
}
