// [dcl.stc]/1 (C++17 removed `register`, P0001R1): `register` is diagnosed
// "ISO C++17 does not allow 'register' storage class specifier"
// (-Wregister, an error by default) at the specifier on parameters
// (including lambda and function template parameters), block-scope and
// for/range-for variables and structured bindings; the declaration
// continues. A variable with an asm register label is exempt. In a
// condition the declaration is a type name and gets the parser's
// storage-class error instead (WP-293).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 25:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 26:27 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 27:15 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 31:3 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 32:3 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 33:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 35:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 35:8 error "loop variable 'e' may not be declared 'register'"
//@ diag: 36:3 error "decomposition declaration cannot be declared 'register'"
//@ diag: 36:3 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 38:7 error "type name does not allow storage class to be specified"

void f(register int x);
void g(register int);
template <class T> void t(register T x);
auto lam = [](register int q) { return q; };

struct P { int a, b; };
int h(P p) {
  register int y = 1;
  register int *ptr = &y;
  for (register int i = 0; i < 2; ++i) {}
  int arr[2] = {};
  for (register int e : arr) {}
  register auto [a, b] = p;
  register int r asm("x1");
  if (register int z = 1) {}
  return y + a + b + r + *ptr;
}
