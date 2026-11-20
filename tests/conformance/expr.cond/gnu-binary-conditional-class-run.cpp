// [expr.cond] GNU binary conditional `x ?: y` with operands of class type
// (clang `BinaryConditionalOperator`): `x` is evaluated once, a class
// prvalue is materialized ([conv.rval]) and contextually converted to
// `bool` ([expr.cond]/1, an explicit `operator bool` included), and the
// result follows [expr.cond]/4-7 with `x` as the second operand: two
// lvalues of one class give an lvalue, an lvalue and a prvalue a copy.
// WP-1738 (GAP-052; g++.dg/ext/cond3.C, cond4.C).
//@ expect: run
//@ stdout: 1 1|1 1|7|1|make 3|3|make 0|make 5|5|7|2|6|1|11|11 4 9|
#include <cstdio>
#include <utility>
static int ctors, dtors, conv;
struct S {
  int i;
  S(int i) : i(i) { ++ctors; }
  S(const S &s) : i(s.i) { ++ctors; }
  ~S() { ++dtors; }
  explicit operator bool() const {
    ++conv;
    return i != 0;
  }
};
struct T {
  int v;
  operator bool() const { return v; }
};
S make(int i) {
  std::printf("make %d|", i);
  return S(i);
}
template <class U> U pick(U a, U b) { return a ?: b; }
template <class U> U &pickref(U &a, U &b) { return a ?: b; }
int main() {
  S a(1), z(0);
  S &r = a ?: z;
  std::printf("%d %d|", &r == &a, r.i);
  S &r2 = z ?: a;
  std::printf("%d %d|", &r2 == &a, r2.i);
  (z ?: a).i = 7;
  std::printf("%d|", a.i);
  S &&x = std::move(a) ?: std::move(z);
  std::printf("%d|", &x == &a);
  S p = make(3) ?: make(4);
  std::printf("%d|", p.i);
  S q = make(0) ?: make(5);
  std::printf("%d|", q.i);
  S w = a ?: make(9);
  std::printf("%d|", w.i);
  T t{0}, u{2};
  T v = t ?: u;
  std::printf("%d|", v.v);
  S s1 = pick(S(0), S(6));
  std::printf("%d|", s1.i);
  S &s2 = pickref(z, a);
  std::printf("%d|", &s2 == &a);
  int k = 0;
  S m = k ?: S(11);
  std::printf("%d|", m.i);
  std::printf("%d %d %d|\n", ctors, dtors, conv);
}
