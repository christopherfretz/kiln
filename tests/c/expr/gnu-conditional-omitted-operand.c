// GNU `a ?: b` (clang `ext_gnu_conditional_expr`, at the `:`): a
// pedantic -Wgnu-conditional-omitted-operand extension, in -Wgnu
// (WP-1312, WP-1306 follow-up).
//@ args: -pedantic
//@ strict-diags
//@ diag: 9:15 warning "use of GNU ?: conditional expression extension, omitting middle operand"
//@ diag: 10:14 warning "use of GNU ?: conditional expression extension, omitting middle operand"
int f(int x, int *p) {
  int *q = p ?: 0;
  return (x ?: 1) + (q ? 1 : 2);
}
