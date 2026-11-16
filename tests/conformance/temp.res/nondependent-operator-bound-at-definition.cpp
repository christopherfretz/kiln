// [temp.res.general]/6 (non-dependent names are bound at the point of
// definition): the non-dependent `x == x` in a template's body is resolved
// when the template is defined, so an operator template declared later is
// not a candidate at instantiation, even though it would be more
// specialized. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags

template <class T> struct wrap {};
template <class T> bool operator==(wrap<T>, wrap<T>);

template <class T> void g(T, wrap<wrap<int>> x) {
  bool b = x == x;
  (void)b;
}

template <class T> void operator==(wrap<wrap<T>>, wrap<wrap<T>>);

void h() {
  wrap<wrap<int>> x;
  g(17, x);
}
