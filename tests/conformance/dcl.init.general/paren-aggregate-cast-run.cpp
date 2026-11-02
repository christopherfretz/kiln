//@ expect: run
//@ stdout: C<int> 1
//@ stdout: C<double> 2.5
//@ stdout: 1 3 2.5 0
//@ stdout: live 0
// [dcl.init.general]/16.6.2.2, [expr.type.conv]/2, [expr.static.cast]/4:
// a cast to an aggregate class with no viable constructor that
// parenthesized aggregate initialization satisfies is built after its
// verify-only attempt (WP-328): the converting constructor templates it
// selects are instantiated and run, the omitted member is
// value-initialized, and the temporaries are destroyed.
#include <cstdio>
static int live = 0;
struct C {
  double v;
  template <class V> C(V x) : v(x) {
    ++live;
    std::printf("%s %g\n", sizeof(V) == sizeof(int) ? "C<int>" : "C<double>", v);
  }
  C(const C &o) : v(o.v) { ++live; }
  ~C() { --live; }
};
struct A { C c; int n; };
int main() {
  {
    double a = A(1).c.v;
    A b = static_cast<A>(2.5);
    std::printf("%g %d %g %d\n", a, (int)sizeof(A) > 0 ? 3 : 0, b.c.v, b.n);
  }
  std::printf("live %d\n", live);
}
