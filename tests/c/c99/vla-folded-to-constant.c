// C17 6.6p6, 6.7.6.2p4: a bound that reads a `const int` object is no
// integer constant expression, so the array is a variable length array.
// Where one is not allowed or is initialized (file scope, a structure
// member, a static local, a block-scope `extern`, a local with an
// initializer) clang folds the bound and makes a constant array as an
// extension, with -Wgnu-folding-constant (on by default) at the
// declarator's name; a plain automatic array stays variable length.
// An enumeration constant is an integer constant expression, so its
// arrays are plain constant arrays.
// Found running GCC's C++ testsuite.
//@ std: c17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 25:5 warning "variable length array folded to constant array as an extension"
//@ diag: 26:16 warning "variable length array folded to constant array as an extension"
//@ diag: 29:7 warning "variable length array folded to constant array as an extension"
//@ diag: 31:14 warning "variable length array folded to constant array as an extension"
//@ diag: 34:5 warning "variable length array folded to constant array as an extension"
//@ diag: 36:14 warning "variable length array folded to constant array as an extension"
const int m = 3;
enum { E = 3 };
int ce[E];
struct CE { int x[E]; };
_Static_assert(sizeof(ce) == 3 * sizeof(int), "constant array");
int fb[m];
struct S { int x[m]; };
_Static_assert(sizeof(fb) == 3 * sizeof(int) && sizeof(struct S) == sizeof fb, "folded");
void f(void) {
  int b[m] = {1};
  int c[m];
  static int d[m];
  (void)b; (void)c; (void)d;
}
int g[m * 2 + E];
void h(void) {
  extern int e[m];
  (void)e;
}
