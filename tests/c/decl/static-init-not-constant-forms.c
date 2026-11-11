// C23 6.7.11p4, 6.6p7-p10: the forms a C static initializer folds to (IEEE
// floating results, address arithmetic, differences within one object,
// label address differences) leave these not constant, as clang: a read
// of a non-const variable (also as a divisor), an address cast to an
// integer and then multiplied or subtracted from, the difference of
// addresses of two objects, and a label address minus an object's.
//@ expect: compile-fail
//@ strict-diags
//@ args: -w
//@ diag: 17:9 error "initializer element is not a compile-time constant"
//@ diag: 18:19 error "initializer element is not a compile-time constant"
//@ diag: 19:12 error "initializer element is not a compile-time constant"
//@ diag: 20:21 error "initializer element is not a compile-time constant"
//@ diag: 21:16 error "initializer element is not a compile-time constant"
//@ diag: 23:24 error "initializer element is not a compile-time constant"
int g, g2;
int d = g;
long k = (long)&g * 2;
long m = 5 - (long)&g;
long q = (char *)&g - (char *)&g2;
double z = 1.0 / g;
void f(void) {
  static long u = &&l1 - (void *)&g;
  (void)u;
l1:;
}
