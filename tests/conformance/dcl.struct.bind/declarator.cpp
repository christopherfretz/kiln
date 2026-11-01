//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:8 error "decomposition declaration '[k, l]' requires an initializer"
//@ diag: 15:7 error "decomposition declaration cannot be declared with type 'int'; declared type must be 'auto' or reference to 'auto'"
//@ diag: 15:7 error "no viable conversion from 'S' to 'int'"
//@ diag: 16:8 error "decomposition declaration must be the only declaration in its group"
//@ diag: 17:9 error "decomposition declaration cannot be declared with type 'auto *'; declared type must be 'auto' or reference to 'auto'"
//@ diag: 17:9 error "cannot decompose non-class, non-array type 'S *'"
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
