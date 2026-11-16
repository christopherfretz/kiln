// [expr.type.conv]/1 (P0849): `auto(x)` and `auto{x}` with a
// type-dependent operand are type-dependent ([temp.dep.expr]/3); the
// placeholder is deduced at instantiation as for `auto v(x);`, a prvalue
// of the decayed type. An operator on the cast is resolved then.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ diag: 29:42 error "invalid argument type 'S' to unary expression"
//@ diag: 30:9 note "in instantiation of function template specialization 'bad<S>' requested here"
struct S { int m; };
template <class T> void f(T t, const T &r, T &&x) {
  (void)+auto(t);
  (void)-auto{r};
  (void)!auto(x);
  static_assert(__is_same(decltype(auto(r)), T));
  static_assert(__is_same(decltype(auto{x}), T));
}
template <class T> auto g(T &t) { return auto(t).m + auto{t}.m; }
template <class T> auto h(T &a) -> decltype(auto(a)) { return a; }
void use() {
  int i = 1;
  f(1, i, 2);
  S s{1};
  (void)g(s);
  int v[2] = {};
  static_assert(__is_same(decltype(h(v)), int *));
}
template <class T> int bad(T t) { return +auto(t); }
int e = bad(S{2});
