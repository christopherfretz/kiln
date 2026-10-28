// [stmt.goto]/1: the label must be in the function
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:17 error "use of undeclared label 'nowhere'"
void f() { goto nowhere; }
