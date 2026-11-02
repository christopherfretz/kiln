// [dcl.fct.default]/1, [expr.cast]/4: a default argument whose explicit
// cast is ill-formed (`(int)"s"`) is erroneous, so its parameter and the
// function are invalid (clang's `ActOnParamDefaultArgumentError`). A call
// that would use it is not diagnosed again, including a constant-evaluated
// one and a delayed member default argument; a function template that is
// invalid matches no call ("no matching function", no candidate note), and
// a redeclaration of the invalid function adds no missing-default-argument
// error, so a call without arguments finds the redeclaration's arity (WP-293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:16 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 30:33 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 31:35 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 32:40 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 33:25 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 34:23 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 40:42 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 41:18 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 44:18 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 45:16 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 51:3 error "no matching function for call to 't'"
//@ diag: 52:3 error "no matching function for call to 'u'"
//@ diag: 53:3 error "no matching function for call to 'u'"
//@ diag: 54:3 error "no matching function for call to 'v'"
//@ diag: 55:3 error "no matching function for call to 'v'"
//@ diag: 56:3 error "no matching function for call to 'w'"
//@ diag: 35:6 note "candidate function not viable: requires 2 arguments, but 0 were provided"

void f(int x = (int)"s");
template <class T> void t(T x = (int)"s");
template <class T> void u(int x = (int)"s");
template <class T> void v(T y, int x = (int)"s");
constexpr int c(int x = (int)"s") { return x; }
void w(int a, int x = (int)"s");
void w(int a = 1, int x);
static_assert(c(2) == 2);
constexpr int k = c();

struct S {
  template <class T> void m(T x, int y = (int)"s");
  void n(int y = (int)"s");
};
template <class U> struct C {
  void p(int y = (int)"s");
  void q(U y = (int)"s");
};

int use() {
  f();
  f(1);
  t<int>(1);
  u<int>(1);
  u<int>();
  v(1);
  v(1, 2);
  w();
  S{}.m(1);
  S{}.m(1, 2);
  S{}.n();
  C<int>{}.p();
  C<int>{}.p(1);
  C<int>{}.q();
  return 0;
}
