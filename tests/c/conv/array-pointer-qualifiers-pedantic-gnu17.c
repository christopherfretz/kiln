// C17 6.7.3p10: before C23 a qualifier on an array type applies to its
// element only, so `int [3]` and `const int [3]` are not compatible and
// converting `int (*)[3]` to `const int (*)[3]` (initialization,
// assignment, argument passing, return), comparing the two or choosing
// between them with `?:` violates a constraint (6.5.16.1p1, 6.5.9p2,
// 6.5.15p3), which needs a diagnostic (5.1.1.3). kiln accepts these uses
// as C23 6.7.3p9 (N2607) does and, under -pedantic, warns as GCC does
// ("invalid use of pointers to arrays with different qualifiers in ISO C
// before C23", -Wc23-array-pointer-qualifiers); a pointer to void, an
// identically qualified pair and C23 (array-pointer-add-qualifier-gnu23.c)
// are not diagnosed. clang 19 and 22 are silent, a deliberate difference
// (docs/status/c-mode.md, spec/diag.md D17). The columns are kiln's: GCC
// 14 reports an assignment at its `=` and `?:` at its `:`. WP-1435.
//@ args: -std=gnu17 -pedantic -Wall
//@ expect: compile-ok
//@ strict-diags
//@ xfail(clang): clang 19 and 22 do not diagnose pointers to differently qualified arrays before C23 (C17 6.7.3p10; kiln warns under -pedantic as GCC does)
//@ diag: 29:57 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 31:43 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 33:24 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 34:8 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 35:8 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 36:27 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 37:34 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 38:23 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 39:16 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
//@ diag: 40:25 warning "invalid use of pointers to arrays with different qualifiers in ISO C before C23"
typedef unsigned int Buckets[256];
unsigned int sum(Buckets *h) { const Buckets *const c = h; return (*c)[0]; }
void take(const int (*p)[3]);
const int (*ret(int (*p)[3]))[3] { return p; }
int f(int c, int (*a)[3], int (*m)[2][3], volatile int (*vq)[3]) {
  const int (*ca)[3] = a;
  ca = a;
  take(a);
  const int (*cm)[2][3] = m;
  const volatile int (*cvq)[3] = vq;
  const int (*z)[3] = (int (*)[3])0;
  int same = a == ca;
  const int (*k)[3] = c ? a : ca;
  const void *pv = a;
  int (*same_q)[3] = a;
  return (*ca)[0] + (*cm)[1][2] + (*cvq)[0] + (*z)[0] + same + (*k)[0] + (pv != 0) + (*same_q)[0];
}
