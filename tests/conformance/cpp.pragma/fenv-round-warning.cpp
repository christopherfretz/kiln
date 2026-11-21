// [cfenv.syn], C23 7.6.2: every `#pragma STDC FENV_ROUND`, directed or
// not, has clang's "not supported" warning (WP-806) and nothing else: the
// directed modes and `#pragma clang fp eval_method` are implemented
// (WP-1744; their semantics are in fenv-round-constexpr.cpp,
// ir-fenv-round*.cpp and ir-fp-eval-method*.cpp).
//@ expect: compile-ok
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 14:34 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 16:37 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 17:35 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 24:38 warning "pragma STDC FENV_ROUND is not supported"
// The warnings are at the end of the pragma.
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
