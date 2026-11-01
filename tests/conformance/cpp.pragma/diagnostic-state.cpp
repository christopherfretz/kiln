//@ strict-diags
//@ expect: compile-fail
//@ diag: 21:20 warning "expression result unused"
//@ diag: 27:20 error "expression result unused"
//@ diag: 29:20 warning "expression result unused"
//@ diag: 33:20 warning "expression result unused"
//@ diag: 39:9 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 39:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 38:13 note "declared here"
//@ diag: 52:9 error "variable length arrays in C++ are a Clang extension"
//@ diag: 52:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 51:14 note "declared here"
//@ diag: 56:29 warning "pragma diagnostic pop could not pop, no matching push"
//@ diag: 57:34 warning "unknown warning group '-Wno-such-warning', ignored"
//@ diag: 34:36 warning "expression result unused"
//@ diag: 36:13 note "in instantiation of function template specialization 'k<int>' requested here"
// #pragma clang/GCC diagnostic push/pop/ignored/warning/error: the
// state at a diagnostic's location decides its severity, so a late
// diagnostic (an instantiation at the end of the file) still sees the
// pragmas around its location.
void f1(int a) { a + 1; }
#include "Inputs/diag-ignore.h"
void f2(int b) { b + 1; }
#pragma clang diagnostic push
#pragma clang diagnostic warning "-Wunused-value"
#pragma clang diagnostic error "-Wunused-value"
void f3(int c) { c + 1; }
#pragma clang diagnostic warning "-Wunused-value"
void f4(int d) { d + 1; }
#pragma clang diagnostic pop
void f5(int e) { e + 1; }
#pragma GCC diagnostic warning "-Wunused-value"
void f6(int g) { g + 1; }
template <class T> void k(T x) { x + 1; }
#pragma clang diagnostic ignored "-Wunused-value"
void f7() { k(1); }
#define IGNORE_VLA _Pragma("clang diagnostic ignored \"-Wvla-cxx-extension\"")
void f8(int n) {
  int v[n];
  v[0] = 0;
}
IGNORE_VLA
void f9(int n) {
  int v[n];
  v[0] = 0;
}
#pragma clang diagnostic push
#pragma clang diagnostic warning "-Weverything"
#pragma clang diagnostic ignored "-Weverything"
#pragma clang diagnostic error "-Wvla-cxx-extension"
void f10(int n) {
  int v[n];
  v[0] = 0;
}
#pragma clang diagnostic pop
#pragma clang diagnostic pop
#pragma clang diagnostic ignored "-Wno-such-warning"
