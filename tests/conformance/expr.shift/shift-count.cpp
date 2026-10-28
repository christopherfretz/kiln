// [expr.shift]/1: shift counts must be in range (warned)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 6:11 warning "shift count >= width of type"
//@ diag: 7:11 warning "shift count is negative"
int a = 1 << 40;
int b = 1 << -1;
