// [expr.cond]/2: one void operand needs a throw-expression on the other
// side or both void.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:13 error "left operand to ? is void, but right operand is of type 'int'"
//@ diag: 10:13 error "right operand to ? is void, but left operand is of type 'int'"
void v();
void f(int x) {
  int a = x ? v() : 1;
  int b = x ? 1 : v();
  x ? v() : throw 1;
  int &c = x ? x : throw x;
  (x ? throw 1 : x) = 2;
}
