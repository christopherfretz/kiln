//@ expect: compile-ok
//@ strict-diags
// The warning is not in the (empty) `diag:` list; GCC's are not checked.
#warning "not listed"
int f() { return 1; }
