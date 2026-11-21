// GNU `const`/`pure` on a function returning `void` is dropped, so its
// declaration and calls carry no `memory(none)`/`memory(read)`; with
// both, `pure` is dropped and `const` stays (clang
// `Sema::CheckFunctionDeclaration`). The diagnostics are in
// const-pure-void-return.cpp. clang 19's IR differs in the AArch64
// datalayout, so this compares with clang 22 and the `-19` sibling with
// clang 19. C++23, so that `S s;` is not given C++26's erroneous-value
// fill. spec/sema.md §4.5, WP-1838.
//@ requires: clang>=22
//@ std: c++23
//@ expect: ir
__attribute__((const)) void f1();
__attribute__((pure)) void f2();
__attribute__((pure, const)) int f3();
__attribute__((const)) int f4();
struct S { __attribute__((const)) S(); __attribute__((pure)) ~S(); };
void call() { f1(); f2(); (void)f3(); (void)f4(); S s; }
template<class T> struct A {
  __attribute__((const)) void f();
  __attribute__((pure)) T g();
  template<class U> __attribute__((const)) U h();
};
A<int> ai;
A<void> av;
void u() { ai.f(); av.g(); (void)ai.h<int>(); ai.h<void>(); }
template<class T> __attribute__((const, pure)) T k();
void w() { k<void>(); (void)k<int>(); }
