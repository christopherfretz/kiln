// [temp.dep.expr]/3, [dcl.init.general]/16.6.2: `T x(U(a))` with `a`
// type-dependent is initialized at instantiation even though `U(a)` (a
// `CXXUnresolvedConstructExpr`) already has class type `U`; it is then
// direct-initialization, so an explicit constructor of `T` taking a `U` is
// a candidate. A static local and a parenthesized argument are the same
// (Boost.Exception `get_static_exception_object`).
//@ expect: run
//@ stdout: 4 4 4
extern "C" int printf(const char*, ...);

struct B {
  int v = 4;
};
template <class X> struct D : B {};
template <class T> struct sp {
  template <class Y> explicit sp(Y* p) : p(p) {}
  T* p;
};
struct E {
  explicit E(sp<const B> const& s) : v(s.p->v) {}
  int v;
};
template <class X> int local(X x) {
  E a((sp<const B>(x)));
  return a.v;
}
template <class X> int stat() {
  D<X> c;
  static E ep(sp<const B>(new D<X>(c)));
  return ep.v;
}
template <class X> int copy_list(X x) {
  E a{sp<const B>(x)};
  return a.v;
}
int main() {
  B b;
  printf("%d %d %d\n", local(&b), stat<int>(), copy_list(&b));
}
