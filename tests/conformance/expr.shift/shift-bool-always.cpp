// [expr.shift]/2, [conv.bool]/1: a `<<` converted to `bool` whose left
// operand is the literal 0, or whose operands are both integer literals
// (a literal under a unary minus counts), always gives the same value
// (-Wtautological-constant-compare, on by default); the shift folds as
// clang's constant folder does it (a negative count shifts right, an
// over-wide count by the width less one). A non-literal (parenthesized,
// character, `bool`) operand is not a literal, and the -Wint-in-bool-
// context case is off by default (WP-410).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 21:9 warning "converting the result of '<<' to a boolean always evaluates to false"
//@ diag: 22:11 warning "converting the result of '<<' to a boolean always evaluates to false"
//@ diag: 23:10 warning "converting the result of '<<' to a boolean always evaluates to true"
//@ diag: 24:10 warning "converting the result of '<<' to a boolean always evaluates to true"
//@ diag: 25:9 warning "converting the result of '<<' to a boolean always evaluates to true"
//@ diag: 26:18 warning "converting the result of '<<' to a boolean always evaluates to false"
//@ diag: 27:10 warning "converting the result of '<<' to a boolean always evaluates to true"
//@ diag: 28:14 warning "converting the result of '<<' to a boolean always evaluates to true"
//@ diag: 29:12 warning "converting the result of '<<' to a boolean always evaluates to false"
bool f(int a, unsigned b) {
  if (0 << a) return 1;
  if (0x0 << a) return 1;
  if (1L << 2) return 1;
  if (1u << 2) return 1;
  if (2 << 1) return 1;
  if (0x10000000 << 4) return 1;
  if (-1 << 1) return 1;
  bool x = 1 << 40;
  if (!(0u << b)) return 1;
  if ((1) << 2) return 1;
  if ('a' << 1) return 1;
  if (a << 1) return 1;
  if (b << 1) return 1;
  bool y = (bool)(0 << a);
  return x && y;
}
