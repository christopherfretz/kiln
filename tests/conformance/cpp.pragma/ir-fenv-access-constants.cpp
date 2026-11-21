// [expr.const], [cfenv.syn]: under `#pragma STDC FENV_ACCESS ON` a
// constant expression is evaluated in the default floating-point
// environment (a `constexpr` function's division, a `constexpr` or static
// variable's initializer, a namespace-scope initializer), but a run-time
// expression is not folded: an automatic `const double` and a condition
// are computed by constrained operations. Compared with clang 22's IR
// exactly (spec/codegen.md "Floating-point semantics", WP-1744).
//@ expect: ir
//@ requires: clang>=22
//@ args: -std=c++26
constexpr double third() {
#pragma STDC FENV_ACCESS ON
  return 1.0 / 3.0;
}
constexpr double t1 = third();
static_assert(t1 == 1.0 / 3.0);

constexpr double strict_third() {
#pragma clang fp exceptions(strict)
  return 1.0 / 3.0;
}
static_assert(strict_third() == third());

double rt(int n) {
#pragma STDC FENV_ACCESS ON
  static const double s = 1.0 / 3.0;
  constexpr double c = 1.0 / 3.0;
  const double l = 1.0 / 3.0;
  double m = 1.0 / 2.0;
  double t = third();
  if (1.0 / 3.0 > 0.3)
    m += 1;
  return s + c + l + m + t + n;
}

#pragma STDC FENV_ACCESS ON
double gl = 1.0 / 3.0;
const double gc = 1.0 / 3.0;
double use() { return gc + gl; }
