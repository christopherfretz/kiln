// [expr.new]/2: an array of a placeholder type cannot be allocated.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:30 error "cannot allocate array of 'auto'"
void f() { auto *p = new auto[2]{1, 2}; }
