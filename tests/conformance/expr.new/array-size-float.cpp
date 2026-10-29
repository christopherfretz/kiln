// [expr.new]/8: a floating-point bound is not converted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:23 error "conversion from 'double' to 'unsigned long' is not allowed in a converted constant expression"
void f() { new int[2][1.5]; }
