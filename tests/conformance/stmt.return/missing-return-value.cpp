// [stmt.return]/2: a non-void function must return a value
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:11 error "non-void function 'f' should return a value"
int f() { return; }
