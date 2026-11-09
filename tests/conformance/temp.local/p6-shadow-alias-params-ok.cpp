// [temp.local]/6 valid twin: tag lookup does not find a non-type template
// parameter, so 'friend struct T;' declares '::T'; a member template's
// parameter hides a class member of that name; names that differ do not
// shadow (WP-1042).
//@ expect: compile-ok
//@ strict-diags
//@ std: c++20
template<int T> struct A {
  friend struct T;
  void f(int U);
};
struct S {
  typedef int U;
  template<class U> void g(U);
  void h(int U);
};
template<class T> struct B {
  template<class U> using V = U;
  void f(int t) { try {} catch (int u) {} }
};
B<int> b;
