// [over.ics.list]/3 (CWG1467, CWG2076): a single-element initializer list
// whose element is of the parameter's class (or a derived class) converts
// by the element's own conversion, for aggregates and non-aggregates
// alike, so `f({a})` is an exact match (copy constructor) and outranks a
// user-defined conversion through another constructor. WP-958.
//@ std: c++17
//@ expect: run
//@ stdout: 1 2 1
#include <cstdio>
struct A {
  A() {}
  A(const A &) {}
};
struct D : A {};
struct W {
  W(A) {}
};
int f(A) { return 1; }
int f(W) { return 2; }
struct P {
  explicit P(int) {}
  P(const P &) = default;
};
int g(P) { return 1; }
int main() {
  A a;
  D d;
  P p(0);
  std::printf("%d %d %d\n", f({a}) + f({d}) - 1, f(W{a}) , g({p}));
}
