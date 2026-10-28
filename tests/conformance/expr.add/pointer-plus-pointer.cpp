// [expr.add]/1: two pointers cannot be added
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:14 error "invalid operands to binary expression ('int *' and 'int *')"
int *p, *q;
void f() { p + q; }
