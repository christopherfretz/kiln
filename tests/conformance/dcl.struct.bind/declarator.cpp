//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:8 error "declaration '[k, l]' requires an initializer"
//@ diag: 17:7 error "declaration cannot be declared with type 'int'; declared type must be 'auto' or reference to 'auto'"
//@ diag: 17:7 error "no viable conversion from 'S' to 'int'"
//@ diag: 18:8 error "declaration must be the only declaration in its group"
//@ diag: 19:9 error "declaration cannot be declared with type 'auto *'; declared type must be 'auto' or reference to 'auto'"
//@ diag: 19:9 error "non-class, non-array type 'S *'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
// [dcl.pre]/6, [dcl.struct.bind]/1: a structured binding declaration needs
// an initializer, must be alone in its declaration, and its declared type
// must be `auto` or a reference to `auto` (cv-qualified).
struct S { int a, b; };
void f(S s) {
  auto [k, l];
  int [m, n] = s;
  auto [o, p] = s, q = s;
  auto *[r, t] = &s;
}
