// clang's `CheckUnaryExprOrTypeTraitOperand`: `sizeof` of an array
// parameter, which is a pointer (-Wsizeof-array-argument, noting the
// parameter), and of a binary operator whose array operand decays to the
// result's type (-Wsizeof-array-decay, at the operator); both in -Wmost
// (WP-1312, WP-1302 follow-up).
//@ strict-diags
//@ diag: 17:15 warning "sizeof on array function parameter will return size of 'int *' instead of 'int[10]'"
//@ diag: 14:12 note "declared here"
//@ diag: 18:15 warning "sizeof on array function parameter will return size of 'int *' instead of 'int[]'"
//@ diag: 14:23 note "declared here"
//@ diag: 20:18 warning "sizeof on pointer operation will return size of 'int *' instead of 'int[3]'"
//@ diag: 21:18 warning "sizeof on pointer operation will return size of 'int *' instead of 'int[3]'"
//@ diag: 22:20 warning "sizeof on pointer operation will return size of 'int *' instead of 'int[3]'"
void f(int a[10], int b[], int *c) {
  int x[3];
  int *p = x;
  (void)sizeof(a);
  (void)sizeof((b));
  (void)sizeof(c);
  (void)sizeof(x + 1);
  (void)sizeof(1 + x);
  (void)sizeof((x) + 1);
  (void)sizeof(x - x);
  (void)sizeof(x == p);
  (void)sizeof(*x + 0);
  (void)sizeof x;
}
