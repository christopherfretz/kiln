// [dcl.array], [expr.static.cast], [expr.eq] (the variable length array
// extension in C++): unlike C, where they are compatible (C23
// 6.7.7.3p6; tests/c/run/vla-type-compatibility.c), two variable length
// array types are never the same type in C++, even with the same element
// type and the same size expression, so a pointer to one converts neither
// to a pointer to another nor from `void *` by `static_cast`, and two such
// pointers do not compare (as clang 19 and 22; GCC accepts). A
// variable's own type keeps working (`sizeof`, arithmetic). WP-1386.
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
//@ diag: 18:9 error "cannot initialize a variable of type 'int (*)[m]' with an rvalue of type 'int (*)[m]'"
//@ diag: 19:9 error "cannot initialize a variable of type 'int (*)[n]' with an lvalue of type 'int (*)[m]'"
//@ diag: 21:17 error "comparison of distinct pointer types ('int (*)[n]' and 'int (*)[m]')"
int f(void *x, int m, int n) {
  int (*p)[m] = nullptr;
  int (*a)[m] = static_cast<int (*)[m]>(x);
  int (*q)[n] = p;
  int s = sizeof *p + (int)((char *)(p + 1) - (char *)p);
  return s + (q == p) + (a != nullptr);
}
