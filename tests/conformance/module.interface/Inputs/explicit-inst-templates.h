// One template of each kind (explicit-inst-gmf-run.cpp,
// explicit-inst-header-unit-run.cpp).
template <class T> T twice(T t) { return t + t; }
template <class T> struct S {
  T get(T t) { return t + 1; }
};
template <class T> struct V {
  static T v;
};
template <class T> T V<T>::v = T(7);
template <class T> T vt = T(5);
template <class T> struct C {
  T f(T t) { return t * 2; }
  static T s;
};
template <class T> T C<T>::s = T(3);
template <class T> struct D {
  virtual T f(T t) { return t * 3; }
};
