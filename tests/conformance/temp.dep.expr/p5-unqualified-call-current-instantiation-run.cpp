// [temp.dep.expr]/3, /5, [temp.dep.type]/4, [basic.lookup.argdep]/3.1: an
// unqualified call whose name denotes members of the current
// instantiation is decided by what the name denotes, not by its spelling:
// a set of static members (`so(1)`) and a member template named by a
// template-id (`tm<X>(x)`, static or not) resolve at definition when
// nothing can change at instantiation, so `.m<0>()` on the result needs
// no `template`. Also from a static member function, a lambda, and a class
// nested in the template (the enclosing class's static members). Class
// member lookup finding them suppresses argument-dependent lookup, so the
// hidden friends `tm` and `so` of `X` are not candidates. A class with a
// dependent base keeps such a call dependent (`template` stays needed:
// `N`). clang 22 and GCC 16.1 keep the calls type-dependent, so the text
// decides and no oracle is compared (WP-1628, WP-1609).
//@ expect: run
//@ skip(clang): clang 22 treats a call naming members of the current instantiation as type-dependent
struct R {
  int v;
  template <int N> int m() { return v + N; }
};
struct Q {};
struct X {
  template <class U> friend Q tm(X) { return {}; }
  friend Q so(X) { return {}; }
};
template <class T> struct S {
  static R so(int) { return R{1}; }
  static R so(X) { return R{2}; }
  template <class U> static R tm(U) { return R{10}; }
  template <class U> R itm(U) { return R{100}; }
  int f() {
    X x;
    auto l = [] { return so(1).m<0>(); };
    return so(1).m<0>() + so(x).m<0>() + tm<X>(x).m<0>() + itm<int>(1).m<0>() + l();
  }
  static int g() { return so(1).m<0>() + tm<long>(1).m<0>(); }
  struct In {
    int h() { return so(1L).m<0>(); }
  };
};
template <class T> struct N : T {
  static R so(int) { return R{1000}; }
  int f() { return so(1).template m<0>(); }
};
int main() {
  if (S<int>().f() != 1 + 2 + 10 + 100 + 1)
    return 1;
  if (S<int>::g() != 11 || S<int>::In().h() != 1)
    return 2;
  return N<Q>().f() == 1000 ? 0 : 3;
}
