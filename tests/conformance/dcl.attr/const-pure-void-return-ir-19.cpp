// GNU `const`/`pure` on a function returning `void` is dropped from the
// IR. The clang 19 sibling of const-pure-void-return-ir.cpp.
// spec/sema.md §4.5, WP-1838.
//@ requires: clang<22
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
