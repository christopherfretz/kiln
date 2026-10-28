// [expr.sizeof]/5: sizeof... names a pack
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:29 error "'n' does not refer to the name of a parameter pack"
int n = 1;
unsigned long s = sizeof...(n);
