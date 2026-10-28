// [expr.sizeof]/1: sizeof does not apply to incomplete or function types
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:19 error "invalid application of 'sizeof' to an incomplete type 'I'"
//@ diag: 7:8 note "forward declaration of 'I'"
//@ diag: 10:25 error "invalid application of 'sizeof' to a function type"
struct I;
unsigned long a = sizeof(I);
int fn();
unsigned long b = sizeof(fn);
