// [expr.mul]/4: division by zero is undefined (warned)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 6:25 warning "division by zero is undefined"
//@ diag: 6:33 warning "remainder by zero is undefined"
int f(int x) { return x / 0 + x % 0; }
