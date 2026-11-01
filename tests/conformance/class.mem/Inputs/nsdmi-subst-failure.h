// Templates whose default member initializers fail to substitute (included by
// ../nsdmi-subst-failure.cpp; the errors here are not part of its expectations).
struct X {};
template <class T> struct A {
  int v = T::nope;
  int w = 1;
};
template <class T> struct C {
  int v = T::nope;
  C() {}
  C(int) : v(0) {}
};
