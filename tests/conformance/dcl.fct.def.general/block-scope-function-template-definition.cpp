// [dcl.fct.def.general]/2: a function-definition shall appear only in
// namespace or class scope. A function template definition in a block is
// clang 22's "function definition is not allowed here" at its `{`
// (`ParseSingleDeclarationAfterTemplate`, before the template-scope
// check); the body is skipped and the block goes on. Clang 19 gives
// "expected expression" (the -19 sibling; WP-1091).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 18:33 error "function definition is not allowed here"
//@ diag: 19:60 error "function definition is not allowed here"
//@ diag: 20:68 error "function definition is not allowed here"
//@ diag: 21:54 error "function definition is not allowed here"
//@ diag: 22:34 error "function definition is not allowed here"
//@ diag: 28:33 error "function definition is not allowed here"
//@ diag: 29:14 error "function definition is not allowed here"
void f() {
  template<typename T> void g() {}
  template<class T> requires requires(T t) { t; } int h(T) { return 0; }
  template<class T, class U = decltype(sizeof(int) >> 1)> void m() {}
  template<template<class> class TT> auto n() -> int { return 0; }
  template<class T> int (*q())() { return 0; }
  int after = 0;
  (void)after;
}
struct S {
  void k() {
    template<class T> void l(T) {}
    void p() {}
  }
};
