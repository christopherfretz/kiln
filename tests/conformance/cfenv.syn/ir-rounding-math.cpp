// [cfenv.syn], clang `-frounding-math`: the dynamic rounding mode is in
// effect everywhere, so every function and initialization function is
// `strictfp` and its FP operations are constrained calls with
// `round.dynamic` and `fpexcept.ignore`; `FE_TONEAREST` and a directed
// `FENV_ROUND` give their own mode, FENV_ACCESS ON strict exceptions; a
// function defined under a file-scope FE_TONEAREST is not `strictfp`. An
// automatic `const` initializer is not folded; a constant initializer
// is, in the default environment. Compared with clang 22's IR exactly
// (spec/codegen.md "Floating-point semantics", WP-1798).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu -std=c++26 -Wno-unknown-pragmas -frounding-math
double h(double);
double add(double a, double b) { return a + b; }
float narrow(double a) { return a; }
bool less(double a, double b) { return a < b; }
double fused(double a, double b, double c) { return a * b + c; }
double flr(double a) { return __builtin_floor(a); }
double call(double a) { return h(a); }
double g = 1.0 / 3.0;
double gd = h(1.0) + 1.0;
thread_local double tl = h(2.0) - 1.0;
double* ptl() { return &tl; }
double stat(double a) { static double z = h(a) + 0.5; return z; }
double folded() { const double x = 1.0 / 3.0; return x; }
constexpr double third(double a) { return a / 3.0; }
constexpr double k = third(1.0);
double usek() { return k; }
double near(double a, double b) {
#pragma STDC FENV_ROUND FE_TONEAREST
  return a + b;
}
double up(double a, double b) {
#pragma STDC FENV_ROUND FE_UPWARD
  return a + b;
}
double fenv(double a, double b) {
#pragma STDC FENV_ACCESS ON
  return a + b;
}
// Defined under a file-scope FE_TONEAREST: `strictfp` only when the
// command line's exceptions are not ignored.
#pragma STDC FENV_ROUND FE_TONEAREST
double near_file(double a, double b) { return a / b; }
