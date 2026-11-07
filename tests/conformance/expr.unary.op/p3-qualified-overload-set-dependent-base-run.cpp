// [expr.unary.op]/3, [over.over]/1, [temp.dep.type]: inside a member of a
// class template with a dependent base, `&N::f` naming an overload set of
// a class `N` that `*this` might derive from is the qualified name itself,
// not the implicit `this->N::f`: the set is resolved against the target
// type, whether `N` turns out unrelated to the instantiated class
// (`D<int>`), a base reached through the dependent base (`E<X>`), or a
// non-dependent base (`F<int>`). The set may hold static members and
// member function templates (WP-715).
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ stdout: 18 3 44 6 7
#include <cstdio>

struct Int {
  int v;
  int f(int b) { return v + b; }
  int f(int b, int c) { return v + b * c; }
  static int s(int) { return 1; }
  static int s(long) { return 2; }
  template <class T> int t(T x) { return v + int(sizeof(T)) * 0 + int(x); }
  int t(char) { return -1; }
};
struct X : Int {};
template <class P> struct Base {
  int q(int) { return 30; }
  int q(long) { return 40; }
};

template <class P> struct D : Base<P> {
  int g() {
    int (Int::*f1)(int) = &Int::f;
    int (Int::*f2)(int, int) = &Int::f;
    int (*s1)(long) = &Int::s;
    Int i{1};
    return (i.*f1)(2) + (i.*f2)(3, 4) + s1(0);
  }
  int h() {
    int (Int::*t1)(int) = &Int::t;
    Int i{1};
    return (i.*t1)(2);
  }
};

template <class P> struct E : P {
  int g() {
    int (Int::*f1)(int) = &Int::f;
    int (Base<int>::*q)(long) = &Base<int>::q;
    Base<int> b;
    X x;
    x.v = 2;
    return (x.*f1)(2) + (b.*q)(0);
  }
};
struct BX : Base<int>, X {};

template <class P> struct F : Base<P>, X {
  int g() {
    int (Int::*f1)(int, int) = &Int::f;
    return (this->*f1)(1, 1);
  }
};

int main() {
  F<int> fx;
  fx.v = 5;
  auto lam = [] { int (Int::*f1)(int) = &Int::f; Int i{6}; return (i.*f1)(1); };
  std::printf("%d %d %d %d %d\n", D<int>().g(), D<char>().h(), E<BX>().g(), fx.g(), lam());
}
