// [expr.static.cast]/4, [expr.cast]/4, [conv.lval]/1, [expr.comma]/1: the
// value read through `static_cast<const T&>(f())` of a reference-returning
// call (and the C-style, nested, member `operator<<` and comma forms) is
// the referee's value (WP-632; the IR is pinned by
// ir-ref-cast-of-ref-call.cpp).
//@ expect: run
//@ stdout: 2.5,2.5,2.5,7,2.5,9.5,3.5,7,2.5,
//@ stdout: 2.5,2.5,2.5,
#include <stdio.h>
static double g = 2.5;
static int gi = 7;
const double& coeff() { return g; }
const int& icoeff() { return gi; }
void sink(double v) { printf("%g,", v); }
void sinki(int v) { printf("%d,", v); }
struct M {
  double a[2];
  const double& coeff(int i) const { return a[i]; }
};
struct O {
  O& operator<<(double v) { printf("%g,", v); return *this; }
};
double ret() { return static_cast<const double&>(coeff()) + 1.0; }
int main() {
  M m{{9.5, 3.5}};
  O s;
  sink(static_cast<const double&>(coeff()));
  sink((const double&)coeff());
  sink(static_cast<const double&>(static_cast<const double&>(coeff())));
  sinki(static_cast<const int&>(icoeff()));
  sink(static_cast<const double&&>(coeff()));
  s << static_cast<const double&>(m.coeff(0)) << static_cast<const double&>(m.coeff(1));
  sink((sinki(gi), static_cast<const double&>(coeff())));
  printf("\n");
  // Regression pins.
  sink(coeff());
  const double& r = coeff();
  sink(static_cast<const double&>(r));
  sink(ret() - 1.0);
  printf("\n");
  return 0;
}
