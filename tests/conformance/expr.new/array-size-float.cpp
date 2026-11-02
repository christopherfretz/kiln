// [expr.new]/8: a floating-point bound is not converted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:23 error "conversion from 'double' to '__size_t' (aka 'unsigned long') is not allowed in a converted constant expression"
//@ requires: clang>=22
void f() { new int[2][1.5]; }
