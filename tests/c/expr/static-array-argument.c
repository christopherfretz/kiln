// C17 6.7.6.3p7: an argument for a parameter declared `T a[static N]`
// points to at least N elements. clang's `CheckStaticArrayArgument`
// warns at the call: a null pointer constant (-Wnonnull), an array of
// fewer elements, or of another element type and fewer bytes
// (-Warray-bounds), noting the parameter (WP-1312, WP-1296 follow-up).
//@ strict-diags
//@ diag: 28:3 warning "array argument is too small; contains 2 elements, callee requires at least 4"
//@ diag: 19:12 note "callee declares array parameter as static here"
//@ diag: 30:3 warning "null passed to a callee that requires a non-null argument"
//@ diag: 19:12 note "callee declares array parameter as static here"
//@ diag: 31:3 warning "null passed to a callee that requires a non-null argument"
//@ diag: 19:12 note "callee declares array parameter as static here"
//@ diag: 32:3 warning "array argument is too small; contains 2 elements, callee requires at least 4"
//@ diag: 19:12 note "callee declares array parameter as static here"
//@ diag: 33:3 warning "array argument is too small; contains 3 elements, callee requires at least 4"
//@ diag: 19:12 note "callee declares array parameter as static here"
//@ diag: 34:3 warning "array argument is too small; is of size 4, callee requires at least 16"
//@ diag: 19:12 note "callee declares array parameter as static here"
void h(int a[static 4]);
void q(int a[static 2][3]);
void g(int a[4]);
void k(void) {
  int x[2];
  int y[4];
  const int ci[3] = {0};
  short s[2];
  int big[2][3];
  h(x);
  h(y);
  h(0);
  h((void *)0);
  (h)(x);
  h((int *)ci);
  h((int *)s);
  q(big);
  q(big + 1);
  g(x);
  g(0);
}
