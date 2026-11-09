// [dcl.type.class.deduct]/1, [temp.res.general]/4: 'typename T::X x = t;'
// with X a member class template deduces its arguments (CTAD). From clang's
// SemaCXX/cxx14-compat.cpp. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-ok
template <typename T> void g(T t) { typename T::X x = t; }
struct A {
  template <typename T> struct X { X(T); };
};
void h(A a) { g(a); }
