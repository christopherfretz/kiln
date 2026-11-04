// [temp.dep.type]/1, [expr.unary.op]/3, [expr.cond]/7: in a class template,
// `c ? &Scan::f : &Scan::g` has two pointer-to-member operands of the same
// type `R (Scan<C>::*)()` and keeps it (no decay, no overload set), both
// in the pattern and after instantiation; so does a `?:` of two
// pointers to data members (WP-608).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 30 40
#include <cstdio>

template <class C> struct Scan {
  int a = 30, b = 40;
  int f() { return 1; }
  int g() { return 2; }
  int pick(bool c) {
    auto p = c ? &Scan::f : &Scan::g;
    static_assert(__is_same(decltype(p), int (Scan<C>::*)()));
    return (this->*p)();
  }
  int field(bool c) {
    int Scan::*m = c ? &Scan::a : &Scan::b;
    return this->*m;
  }
};

int main() {
  Scan<char> s;
  int r[4] = {s.pick(true), s.pick(false), s.field(true), s.field(false)};
  std::printf("%d %d %d %d\n", r[0], r[1], r[2], r[3]);
  if (r[0] != 1 || r[1] != 2)
    return 1;
  return r[2] == 30 && r[3] == 40 ? 0 : 2;
}
