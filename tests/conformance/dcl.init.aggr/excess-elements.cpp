// [dcl.init.aggr]/4: too many initializers
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:19 error "excess elements in array initializer"
//@ diag: 8:11 error "excess elements in struct initializer"
int a[2] = {1, 2, 3};
struct P { int x; };
P p = {1, 2};
