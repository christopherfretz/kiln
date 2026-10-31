//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:18 error "binding 'r' cannot appear in the initializer of its own decomposition declaration"
//@ diag: 10:22 error "binding 'v' cannot appear in the initializer of its own decomposition declaration"
// [dcl.struct.bind]/1 (C++26 [basic.scope.pdecl]/2): a binding cannot be
// used in the initializer of its own structured binding declaration.
struct S { int a, b; };
void f(S s) {
  auto [r, t] = (r, s);
  auto [u, v] = S{1, v};
}
