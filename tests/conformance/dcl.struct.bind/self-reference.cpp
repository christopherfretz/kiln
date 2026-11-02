//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:18 error "binding 'r' cannot appear in the initializer of its own"
//@ diag: 12:22 error "binding 'v' cannot appear in the initializer of its own"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
// [dcl.struct.bind]/1 (C++26 [basic.scope.pdecl]/2): a binding cannot be
// used in the initializer of its own structured binding declaration.
struct S { int a, b; };
void f(S s) {
  auto [r, t] = (r, s);
  auto [u, v] = S{1, v};
}
