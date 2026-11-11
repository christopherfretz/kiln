// C17 6.7.6p1 (declarators): a parameter-type-list ending in `...` has at
// least one parameter before it; C23 allows `(...)` alone
// (tests/c/decl/ellipsis-only-parameter-c23.c).
//@ expect: compile-fail
//@ std: c17
//@ strict-diags
//@ diag: 9:7 error "ISO C requires a named parameter before '...'"
//@ diag: 10:8 error "ISO C requires a named parameter before '...'"
int h(...);
void k(...) {}
int m(int a, ...);
