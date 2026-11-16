// C17 6.6p6: with -Wvla, a bound that reads a `const int` object is
// reported as a variable length array at the bound, including where clang
// then folds it to a constant array (-Wgnu-folding-constant at the name,
// silenced here); `-Wno-gnu-folding-constant` leaves only -Wvla. A C23
// `constexpr` object is a named constant (C23 6.6p7): no warning at all.
// Found running GCC's C++ testsuite.
//@ std: c23
//@ args: -Wvla -Wno-gnu-folding-constant
//@ expect: compile-ok
//@ strict-diags
//@ diag: 16:8 warning "variable length array used"
//@ diag: 18:9 warning "variable length array used"
//@ diag: 19:9 warning "variable length array used"
const int m = 3;
constexpr int k = 3;
int fb[m];
void f(void) {
  int b[m] = {1};
  int c[m];
  int e[k];
  static int d[k];
  (void)b; (void)c; (void)d; (void)e;
}
