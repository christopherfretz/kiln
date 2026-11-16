//@ expect: run
//@ strict-diags
//@ args: -std=c++20
//@ diag: 28:10 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 28:10 note "read of non-constexpr variable 'a' is not allowed in a constant expression"
//@ diag: 26:11 note "declared here"
//@ diag: 28:8 warning "variable length array folded to constant array as an extension"
//@ diag: 29:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 29:9 note "read of non-constexpr variable 'a' is not allowed in a constant expression"
//@ diag: 26:11 note "declared here"
//@ diag: 29:7 warning "variable length array folded to constant array as an extension"
//@ diag: 30:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 30:9 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 27:7 note "declared here"
//@ stdout: 12 12 5 4
// [dcl.init.general]/16.5 with a bound that is not a constant expression
// but that clang can fold: an automatic array with an initializer is
// given the folded constant bound (with -Wgnu-folding-constant at the
// name), so the initializer is allowed, where a bound that does not fold
// still makes "variable-sized object may not be initialized" an error.
// Found running GCC's C++ testsuite.
extern "C" int printf(const char *, ...);

struct R { int r; };
int main() {
  const R a = {12};
  int k = 4;
  char c[a.r] = "12345678901";
  int d[a.r / 3 + 1] = {1, 2, 3, 4, 5};
  int e[k];
  (void)e;
  static_assert(sizeof c == 12, "");
  printf("%d %d %d %d\n", (int)sizeof c, (int)sizeof d / 2 + 2, d[4], (int)(sizeof e / sizeof e[0]));
}
