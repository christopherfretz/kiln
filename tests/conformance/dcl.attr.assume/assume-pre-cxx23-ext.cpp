// [dcl.attr.assume] (WP-160): before C++23 the attribute is accepted as an
// extension; -pedantic warns once per attribute (-Wc++23-attribute-extensions,
// not part of -Wc++23-extensions), including in an uninstantiated template.
//@ expect: compile-ok
//@ std: c++20
//@ args: -pedantic
//@ strict-diags
//@ diag: 13:5 warning "use of the 'assume' attribute is a C++23 extension"
//@ diag: 14:5 warning "use of the 'assume' attribute is a C++23 extension"
//@ diag: 14:16 warning "use of the 'assume' attribute is a C++23 extension"
//@ diag: 18:5 warning "use of the 'assume' attribute is a C++23 extension"
int f(int x) {
  [[assume(x > 0)]];
  [[assume(x), assume(x != 3)]];
  return x;
}
template <class T> T t(T x) {
  [[assume(x > 0)]];
  return x;
}
int u(int y) { return t(y); }
