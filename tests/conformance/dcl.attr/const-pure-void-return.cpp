// GNU `const` and `pure` as clang checks them on a function declaration
// (clang `Sema::CheckFunctionDeclaration`): both on one declaration is
// "'const' attribute imposes more restrictions; 'pure' attribute
// ignored" and `pure` is dropped; either on a function returning `void`
// (a constructor and a destructor too, a trailing `-> void`, but not a
// deduced `auto`; a template once instantiated) is "attribute ignored"
// and the attribute is dropped, so a call is not "declared with const
// attribute" and the IR has no `memory(...)`
// (const-pure-void-return-ir.cpp). Only the declaration's own
// attributes count, not a redeclaration's.
// spec/sema.md §4.5, WP-1838.
//@ verify
__attribute__((const)) void f1(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((pure)) void f2(); // expected-warning {{'pure' attribute on function returning 'void'; attribute ignored}}
[[gnu::const]] void f3(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
[[gnu::pure]] void f4(); // expected-warning {{'pure' attribute on function returning 'void'; attribute ignored}}
__attribute__((__const__)) void f5(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
struct S {
  __attribute__((const)) S(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
  __attribute__((pure)) ~S(); // expected-warning {{'pure' attribute on function returning 'void'; attribute ignored}}
  __attribute__((const)) void m(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
  __attribute__((const)) operator int();
};
__attribute__((const, pure)) void f6(); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}} expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((const)) int f7();
__attribute__((const)) void (*fp)();
void def1() __attribute__((const)); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
void def1() {}
void g1();
__attribute__((const)) void g1(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((const)) void g2(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
void g2();
__attribute__((pure, const)) int g3(); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}}
__attribute__((pure)) int g4();
__attribute__((const)) int g4();
typedef void V;
__attribute__((const)) V g6(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((const)) auto g7() -> void; // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((const)) auto g8() { return; }
__attribute__((const)) auto g9() { return 1; }
__attribute__((const)) const void g10(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((pure)) __attribute__((const)) int g11(); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}}
[[gnu::const, gnu::pure]] int g12(); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}}
[[gnu::pure]] [[gnu::const]] void g13(); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}} expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}

template<class T> __attribute__((const)) T tf(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
template<class T> struct TT {
  __attribute__((pure)) T m(); // expected-warning {{'pure' attribute on function returning 'void'; attribute ignored}}
};
TT<void> tv; // expected-note {{in instantiation of template class 'TT<void>' requested here}}
template<class T> struct A {
  __attribute__((const)) void f(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
  template<class U> __attribute__((const)) U h(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
};
A<int> ai;
template<class T> __attribute__((const, pure)) T k(); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}} expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}

void use() {
  f1(); f2(); f6(); tv.m(); ai.f();
  f7(); // expected-warning {{ignoring return value of function declared with const attribute}}
  tf<void>(); // expected-note {{in instantiation of function template specialization 'tf<void>' requested here}}
  tf<int>(); // expected-warning {{ignoring return value of function declared with const attribute}}
  ai.h<void>(); // expected-note {{in instantiation of function template specialization 'A<int>::h<void>' requested here}}
  (void)ai.h<int>();
  k<void>(); // expected-note {{in instantiation of function template specialization 'k<void>' requested here}}
  (void)k<int>();
}
