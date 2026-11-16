// [dcl.stc]/2: in C++98 `register` is a storage class specifier, without a
// diagnostic (WP-1467).
//@ std: c++98
//@ strict-diags
int f(register int a) {
  register int x = a;
  return x;
}
