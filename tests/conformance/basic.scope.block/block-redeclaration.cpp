// [basic.scope.block]/2: a name is declared once per block, and not again as a parameter
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:7 error "redefinition of 'x'"
//@ diag: 9:7 note "previous definition is here"
//@ diag: 11:7 error "redefinition of 'p'"
//@ diag: 8:12 note "previous definition is here"
void f(int p) {
  int x;
  int x;
  int p;
}
