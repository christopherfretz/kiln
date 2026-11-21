// [cfenv.syn], C23 7.6.1: a function with a scope under `#pragma STDC
// FENV_ACCESS ON`, `#pragma clang fp exceptions(maytrap|strict)` or
// `#pragma float_control(except, on)` is `strictfp`: every FP operation in
// it (arithmetic, `fp_contract(on)` fusion, comparisons, conversions, the
// rounding and min/max builtins) is an `llvm.experimental.constrained.*`
// call with the scope's rounding mode (`round.dynamic` under FENV_ACCESS
// ON, else `round.tonearest`) and exception behavior, constants are not
// folded, and every call in it is `strictfp`. A lambda's body is its own
// function; `FENV_ACCESS OFF` at the end of the block leaves the function
// unconstrained; a template instantiation keeps its pattern's state.
// Compared with clang 22's IR exactly (spec/codegen.md "Floating-point
// semantics", WP-1744).
//@ expect: ir
//@ requires: clang>=22
//@ args: -std=c++26
double ext(double);
void sink();

double access(double a, double b) {
#pragma STDC FENV_ACCESS ON
  double c = a + b * a;
  int i = (int)c;
  float g = (float)c;
  unsigned u = (unsigned)b;
  long double w = a;
  return c / b + i + g + u + ext(a) + (a < b) + (a == b) + (bool)a - (double)w;
}

double outer(double a, double b) {
  {
#pragma STDC FENV_ACCESS ON
    a = a * b;
  }
  sink();
  __builtin_memcpy(&a, &b, sizeof a);
  float g = b;
  a++;
  return -a * b + g + __builtin_fma(a, b, a) + __builtin_sqrt(a) + __builtin_fabs(b) +
         __builtin_floor(a) + __builtin_rint(b) + __builtin_fmax(a, b);
}

double maytrap(double a, double b) {
#pragma clang fp exceptions(maytrap)
  return a * b - a / 3.0;
}

double except_on(double a, double b) {
#pragma float_control(except, on)
  return a - b >= 1.0 ? a : b;
}

double nearest(double a) {
#pragma STDC FENV_ACCESS ON
#pragma STDC FENV_ROUND FE_TONEAREST
  return a * 1.5 + 2.0;
}

double dyn(double a) {
#pragma STDC FENV_ROUND FE_DYNAMIC
#pragma clang fp exceptions(strict)
  return a + 1.0;
}

double plain(double a, double b) { return a * b + 1.0; }

double off(double a, double b) {
#pragma STDC FENV_ACCESS ON
#pragma STDC FENV_ACCESS OFF
  return a + b;
}

template <class T> T tsum(T a, T b) {
#pragma STDC FENV_ACCESS ON
  return a + b;
}
template float tsum(float, float);

double lam(double a) {
  return [](double x) {
#pragma STDC FENV_ACCESS ON
    return x / 3.0;
  }(a) + a * 2.0;
}

#pragma STDC FENV_ACCESS ON
double after(double a) { return a / 7.0; }
