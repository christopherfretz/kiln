// CWG2351 before clang 20: `void{}` is "illegal initializer type 'void'"
// at the `{` (kiln at -fclang-compat=19, the clang 19 oracle lane).
//@ expect: compile-fail
//@ requires: clang<20
//@ strict-diags
//@ diag: 8:16 error "illegal initializer type 'void'"
//@ diag: 9:22 error "illegal initializer type 'void'"
void f() { void{}; }
void g() { (void)void{}; }
