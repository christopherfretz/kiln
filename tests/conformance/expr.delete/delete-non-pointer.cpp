// [expr.delete]/1: the operand must be a pointer (or convert to one).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:17 error "cannot delete expression of type 'int'"
void f(int x) { delete x; }
