// [expr.new]/8: std::nullptr_t does not convert to std::size_t.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:20 error "converting 'std::nullptr_t' to incompatible type 'unsigned long'"
void f() { new int[nullptr]; }
