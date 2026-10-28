// [expr.ass]/1: the left operand must be a modifiable lvalue
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:14 error "cannot assign to variable 'c' with const-qualified type 'const int'"
//@ diag: 6:11 note "variable 'c' declared const here"
const int c = 1;
void f() { c = 2; }
