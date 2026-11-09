// [dcl.fct.default]/8-9 allow, in a default argument: an unevaluated use of
// a local variable, a use of a constant local that is not an odr-use, a
// namespace-scope or static member variable, an unevaluated member, a
// capture-less lambda and an init-capture of a constant. Invalid pair:
// local-entity-default-arg-forms.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-ok
int g0;
struct A {
  int n;
  static int s;
  void f(int = sizeof(n));
  void k(int = s);
  void q(int = [] { return 2; }());
};
int f() {
  int i = 0;
  const int ci = 1;
  constexpr int c = 3;
  extern int a(int = sizeof(i));
  extern int b(int = ci);
  extern int b2(long = c);
  extern int d(int = decltype(i)());
  extern int e(int = g0);
  extern int h(int = [x = ci] { return x; }());
  extern int p(int = [] { return 1; }());
  struct L { int n(int x = ci) { return x; } int m(int x = sizeof(i)) { return x; } };
  return L().n() + L().m() + i;
}
template <class T> int tf() {
  const int ci = 1;
  extern int x(int = ci);
  return 0;
}
int z = tf<int>();
