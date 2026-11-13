// [expr.add]/5, [dcl.array] (zero-length arrays and variable length
// arrays, GNU extensions in C++): subtracting pointers to a type of zero
// size warns (-Wpointer-arith, on by default), and clang counts every
// variable length array as one (its static size is zero), also through a
// typedef or nested in a constant array; an empty class has size one.
// The `aka` of a pointer to a typedef of a variable length array keeps
// the parentheses (`int (*)[m]`). As clang 19 and 22. WP-1405.
//@ expect: compile-fail
//@ args: -Wno-vla-cxx-extension -Wno-zero-length-array
//@ strict-diags
//@ diag: 26:15 warning "subtraction of pointers to type 'int[0]' of zero size has undefined behavior"
//@ diag: 26:25 warning "subtraction of pointers to type 'int[n]' of zero size has undefined behavior"
//@ diag: 26:35 warning "subtraction of pointers to type 'int[2][n]' of zero size has undefined behavior"
//@ diag: 26:45 warning "subtraction of pointers to type 'Z' (aka 'int[n * 0]') of zero size has undefined behavior"
//@ diag: 29:7 error "cannot initialize a variable of type 'int' with an lvalue of type 'A *' (aka 'int (*)[m]')"
//@ diag: 32:7 error "cannot initialize a variable of type 'int' with an lvalue of type 'B' (aka 'int (*)[m]')"
struct E {};
long f(int n, int m) {
  int (*p)[0] = nullptr;
  int (*q)[n] = nullptr;
  int (*r)[2][n] = nullptr;
  typedef int Z[n * 0];
  Z *a = nullptr;
  E *e = nullptr;
  int (*c)[2] = nullptr;
  long s = (p - p) + (q - q) + (r - r) + (a - a) + (e - e) + (c - c);
  typedef int A[m];
  A *ap = nullptr;
  int x = ap;
  using B = A *;
  B bp = nullptr;
  int y = bp;
  return s + x + y;
}
