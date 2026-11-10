//@ expect: compile-fail
//@ diag: 4 error "undeclared identifier"
// fake-gcc: accept
int f() { return nope; }
