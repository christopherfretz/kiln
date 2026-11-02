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
//@ diag: 26:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 27:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 28:27 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 29:15 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 33:3 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 34:3 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 35:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 37:8 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 37:8 error "loop variable 'e' may not be declared 'register'"
//@ diag: 38:3 error "declaration cannot be declared 'register'"
//@ diag: 38:3 error "ISO C++17 does not allow 'register' storage class specifier [-Wregister]"
//@ diag: 40:7 error "type name does not allow storage class to be specified"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).

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
  register int r asm("r8");
  if (register int z = 1) {}
  return y + a + b + r + *ptr;
}
