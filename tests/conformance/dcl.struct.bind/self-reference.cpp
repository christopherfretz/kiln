//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:18 error "binding 'r' cannot appear in the initializer of its own decomposition declaration"
//@ diag: 12:22 error "binding 'v' cannot appear in the initializer of its own decomposition declaration"
//@ requires: clang<22
// clang 22 emits error: binding 'r' cannot appear in the initializer of its own structured binding ...; WP-355 residual
// [dcl.struct.bind]/1 (C++26 [basic.scope.pdecl]/2): a binding cannot be
// used in the initializer of its own structured binding declaration.
struct S { int a, b; };
void f(S s) {
  auto [r, t] = (r, s);
  auto [u, v] = S{1, v};
}
