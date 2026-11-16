// C17 6.7.6.2p4, 6.5.3.4p2: an array type whose bound reads a `const int`
// object is a variable length array type. Where clang folds such a bound
// to a constant (an array declarator at file scope, a static local, a
// structure member, a typedef at file scope, a compound literal) it warns
// with -Wgnu-folding-constant: at the declarator's name, or at the `(` of
// a compound literal. A type-name in sizeof/_Alignof is never folded, and
// neither is an automatic array or a typedef at block scope. A bound
// that is an enumeration constant or a macro expanding to a literal is an
// integer constant expression and folds nothing.
// Found running GCC's C++ testsuite.
//@ std: c17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 24:5 warning "variable length array folded to constant array as an extension"
//@ diag: 29:11 warning "variable length array folded to constant array as an extension"
//@ diag: 32:16 warning "variable length array folded to constant array as an extension"
//@ diag: 33:13 warning "variable length array folded to constant array as an extension"
//@ diag: 37:14 warning "variable length array folded to constant array as an extension"
//@ diag: 41:12 warning "variable length array folded to constant array as an extension"
//@ diag: 42:18 warning "variable length array folded to constant array as an extension"
const int m = 3;
enum { E = 3 };
#define D 3
int a1[m];
int a2[E];
int a3[D];
unsigned long s2 = sizeof(int[E]) + sizeof(int[D]);
unsigned long s3 = _Alignof(int[m]);
int *cl = (int[m]){1, 2, 3};
int *cl2 = (int[E]){1, 2, 3};
int *cl3 = (int[D]){1, 2, 3};
struct S { int x[m]; };
typedef int T[m];
_Static_assert(sizeof(a1) == 3 * sizeof(int) && sizeof(T) == sizeof a1, "folded");
_Static_assert(sizeof(struct S) == sizeof(int[E]), "folded member");
int g(void) {
  static int b1[m];
  int b2[m];
  unsigned long q = sizeof(int[m]);
  unsigned long q2 = _Alignof(int[m]);
  int *c = (int[m]){1, 2, 3};
  struct L { int x[m]; };
  typedef int TL[m];
  static int b3[E];
  return b2[0] + (int)q + c[0] + b1[0] + (int)q2 + b3[0] + (int)sizeof(TL);
}
