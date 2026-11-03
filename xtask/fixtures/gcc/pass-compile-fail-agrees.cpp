//@ expect: compile-fail
//@ oracle: gcc
//@ diag: 4 error "undeclared identifier"
int f() { return nope; }
