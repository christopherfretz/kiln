// [basic.indet]: clang runs its uninitialized-use analysis only while the
// translation unit has no uncompilable error: a function finished before the
// first error is analyzed, the erroneous function and later ones are not.
//@ expect: compile-fail
//@ args: -Wuninitialized
//@ strict-diags
//@ diag: 12:10 warning "variable 'x' is uninitialized when used here"
//@ diag: 11:8 note "initialize the variable 'x' to silence this warning"
//@ diag: 17:14 error "use of undeclared identifier 'undeclared'"
int before() {
  int x;
  return x;
}

int broken() {
  int y;
  return y + undeclared;
}

int after() {
  int z;
  return z;
}
