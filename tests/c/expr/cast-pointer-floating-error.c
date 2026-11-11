// C17 6.5.4p4: a pointer type shall not be converted to any floating type, nor
// a floating type to a pointer type (clang `CheckCStyleCast`: "pointer cannot be
// cast to type", "operand of type ... cannot be cast to a pointer type"). WP-1285.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:21 error "pointer cannot be cast to type 'float'"
//@ diag: 13:20 error "operand of type 'float' cannot be cast to a pointer type"
//@ diag: 14:22 error "operand of type 'double' cannot be cast to a pointer type"
//@ diag: 15:40 error "pointer cannot be cast to type '_Complex double'"
void f(int *ip, double d) {
  float fl = (float)ip;
  int *p1 = (int *)1.0f;
  void *p2 = (void *)d;
  _Complex double z = (_Complex double)ip;
  (void)fl, (void)p1, (void)p2, (void)z;
}
