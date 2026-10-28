// [expr.ass]/1: an rvalue is not assignable
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:5 error "expression is not assignable"
//@ diag: 8:9 error "expression is not assignable"
void f(int a) {
  1 = 2;
  a + 1 = 3;
}
