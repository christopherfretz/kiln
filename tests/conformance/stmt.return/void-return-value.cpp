// [stmt.return]/2: a void function returns no value
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:12 error "void function 'f' should not return a value"
void f() { return 1; }
