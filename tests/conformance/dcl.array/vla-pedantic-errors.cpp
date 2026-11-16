//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++11 -pedantic-errors
//@ diag: 25:10 error "variable length arrays in C++ are a Clang extension"
//@ diag: 25:15 note "read of non-constexpr variable 'cf' is not allowed in a constant expression"
//@ diag: 17:13 note "declared here"
//@ diag: 27:10 error "variable length arrays in C++ are a Clang extension"
//@ diag: 27:10 note "read of non-constexpr variable 'a' is not allowed in a constant expression"
//@ diag: 26:11 note "declared here"
//@ diag: 27:8 error "variable length array folded to constant array as an extension"
// [dcl.array]/1 under -pedantic-errors: -Wvla-cxx-extension is an
// extension warning (clang `ext_vla_cxx`, an ExtWarn), so -pedantic-errors
// makes it an error, and so is -Wgnu-folding-constant for a bound that
// clang folds (an off-by-default extension, in -Wpedantic). Bounds that
// are integral constant expressions are no extension at all. Found running
// GCC's C++ testsuite.
const float cf = 3.0f;
enum E { ES = sizeof(int) * 2 };
struct R { int r; };
void f() {
  const int n = 3;
  int a1[n];
  int a2[(int)3.0];
  int a3[ES];
  int a4[(int)cf];
  const R a = {12};
  char c[a.r] = "12345678901";
  (void)a1; (void)a2; (void)a3; (void)a4; (void)c;
}
