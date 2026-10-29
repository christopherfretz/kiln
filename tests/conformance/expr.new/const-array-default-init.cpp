// [dcl.init.general]/8: const elements must be initialized.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:27 error "default initialization of an object of const type 'const P[]'"
struct P { int x; };
void f(int n) { new const P[n]; }
