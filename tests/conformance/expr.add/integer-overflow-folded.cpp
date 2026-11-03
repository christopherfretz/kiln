//@ expect: compile-ok
//@ strict-diags
//@ diag: 24:12 warning "overflow in expression; result is -2'147'483'646 with type 'int'"
//@ diag: 25:12 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 26:13 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 26:23 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 26:18 warning "overflow in expression; result is 0 with type 'int'"
//@ diag: 27:10 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 28:38 warning "overflow in expression; result is -9'223'372'036'854'775'808 with type 'long long'"
//@ diag: 30:19 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 36:7 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 36:19 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 37:13 warning "overflow in expression; result is 2'147'483'645 with type 'int'"
//@ diag: 38:18 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
// [expr.pre]/4: a signed `+`, `-`, `*` or unary `-` whose operands fold
// and whose result is not representable is undefined; outside a
// constant-expression clang warns (-Winteger-overflow, at the operator)
// and the evaluation goes on with the wrapped value, so an operator
// applied to a wrapped result can overflow again. An operand naming a
// `constexpr` or `const` variable folds like a literal. Unsigned
// arithmetic wraps silently; an unevaluated operand is not looked at.
constexpr int g = 2147483647;
const int c = 2147483647;
int a1 = g + 3;
int a2 = c * 2;
int a3 = (g + 1) + (g + 1);
int a4 = -(-g - 1);
long long a5 = 9223372036854775807LL + 1;
int x();
int a6 = x() + (g + 1);
unsigned a7 = 4294967295u + 1u;
int a8 = sizeof(g + 1);
int a9 = true ? 1 : g + 1;
int f(int);
void h(int x) {
  f(g + 1); x = g + 1;
  if (x + g * 3) {}
  int t = x + (g + 1);
  (void)t;
}
