// [expr.new]/8: the array bound must be of integral or unscoped enumeration type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:26 error "incompatible pointer to integer conversion converting 'int *' to type 'unsigned long'"
void f(int *n) { new int[n]; }
