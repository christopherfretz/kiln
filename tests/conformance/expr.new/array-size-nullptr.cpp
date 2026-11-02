// [expr.new]/8: std::nullptr_t does not convert to std::size_t.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:20 error "converting 'std::nullptr_t' to incompatible type '__size_t' (aka 'unsigned long')"
//@ requires: clang>=22
void f() { new int[nullptr]; }
