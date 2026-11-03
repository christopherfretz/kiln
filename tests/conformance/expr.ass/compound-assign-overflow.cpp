//@ expect: compile-ok
//@ strict-diags
//@ diag: 28:10 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 29:10 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 23:37 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 24:37 warning "overflow in expression; result is -2 with type 'int'"
//@ diag: 25:38 warning "overflow in expression; result is 2'147'483'647 with type 'int'"
//@ diag: 38:5 warning "shift count >= width of type"
//@ diag: 39:5 warning "shift count is negative"
//@ diag: 40:5 warning "division by zero is undefined"
//@ diag: 41:5 warning "remainder by zero is undefined"
// [expr.ass]/6, [expr.pre]/4: `E1 op= E2` is `E1 = E1 op E2` with E1
// evaluated once, so a signed `+=`, `-=` or `*=` whose result is not
// representable is undefined. Outside a constant-expression clang warns
// (-Winteger-overflow) at the operator of an overflowing right operand,
// and at the compound operator itself when the evaluation reaches it
// with known operands: inside the body of a `constexpr` function the
// full-expression calls. `<<=` never warns this way (a shift is only
// undefined, not an overflow, in clang's check). A constant shift count
// out of range and a constant zero divisor are diagnosed for the
// compound forms as for the binary ones ([expr.shift]/1, [expr.mul]/4).
constexpr int g = 2147483647;
constexpr int cadd() { int i = g; i += 1; return i; }
constexpr int cmul() { int i = g; i *= 2; return i; }
constexpr int csub() { int i = -g; i -= 2; return i; }
constexpr int cshl() { int i = g; i <<= 1; return i; }
void h(int x) {
  x += g + 1;
  x -= 2 * g;
  int d = cadd() + 0;
  int e = cmul() + 0;
  int f = csub() + 0;
  int s = cshl() + 0;
  int t = cadd();
  (void)d; (void)e; (void)f; (void)s; (void)t;
}
void k(int q) {
  q <<= 40;
  q >>= -1;
  q /= 0;
  q %= 0;
}
