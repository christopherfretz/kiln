// [temp.variadic]/12: a declarator whose type names an unexpanded pack
// (a variable, pointer, function, data member, typedef or alias, parameter,
// non-type template parameter) is ill-formed, as is a default argument or
// template default argument naming one. Each is one error at clang's
// location; the declaration is invalid, uses of it are silent and an
// instantiation does not cascade.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:6 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 33:7 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 34:7 warning "empty parentheses interpreted as a function declaration"
//@ diag: 34:7 note "replace parentheses with an initializer to declare a variable"
//@ diag: 34:6 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 35:14 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 36:9 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 42:6 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 43:7 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 44:13 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 45:6 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 46:8 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 48:8 error "data member type contains unexpanded parameter pack 'Ts'"
//@ diag: 53:27 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 54:29 error "declaration type contains unexpanded parameter pack 'Ts'"
//@ diag: 55:39 error "default argument contains unexpanded parameter pack 'Ts'"
//@ diag: 56:50 error "default argument contains unexpanded parameter pack 'Ts'"
//@ diag: 57:51 error "default argument contains unexpanded parameter pack 'Ts'"
//@ diag: 58:54 error "default argument contains unexpanded parameter pack 'Ts'"
//@ diag: 59:48 error "non-type template parameter type contains unexpanded parameter pack 'Ts'"
//@ diag: 60:29 error "declaration type contains unexpanded parameter pack 'Ts'"
template <class... Ts> int f() {
  Ts x;
  Ts *p;
  Ts g();
  typedef Ts T;
  using U = Ts;
  x = 1;
  return x + sizeof(p);
}
int fi = f<int>();
template <class... Ts> struct S {
  Ts m;
  Ts *pm;
  static Ts sm;
  Ts g(int);
  void h(Ts a);
  void h2(Ts... a) {}
  Ts (*fp)();
  int k() { return m; }
};
S<int> s;
int si = s.k();
template <class... Ts> Ts gv;
template <class... Ts> void k(Ts a);
template <class... Ts> void w(int a = sizeof(Ts));
template <class... Ts> struct M { void w(int a = sizeof(Ts)); };
template <class... Ts> struct D { template <class U = Ts> void z(); };
template <class... Ts> struct D2 { template <int N = sizeof(Ts)> void z(); };
template <class... Ts> struct E { template <Ts N> void z(); };
template <class... Ts> void e(Ts N);
template <class... Ts> int ok(Ts... a) { return sizeof...(a); }
int o = ok(1, 2);
