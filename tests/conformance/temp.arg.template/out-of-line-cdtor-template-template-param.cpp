// [class.ctor.general]/1, [class.qual]/2: `X<C, T>::X(` names the
// constructor also when the parameter type is a member typedef of the class
// template, found only in its scope (the lookahead cannot enter the
// template's scope yet): constructors and destructors of class templates
// with template template parameters, a constructor template, a nested
// class's constructor, a partial specialization's constructor, a pack of
// template template parameters (WP-1495; found running GCC's C++
// testsuite).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
template<class T> struct B { typedef int bar; };
template<template<class> class C, class T> struct foo {
  typedef typename C<T>::bar bar;
  foo(bar);
  ~foo();
  template<class U> foo(U, bar);
  struct N { typedef long n; N(n); };
};
template<template<class> class C, class T> foo<C, T>::foo(bar) {}
template<template<class> class C, class T> foo<C, T>::~foo() {}
template<template<class> class C, class T> template<class U> foo<C, T>::foo(U, bar) {}
template<template<class> class C, class T> foo<C, T>::N::N(n) {}
template<class T> struct foo<B, T*> { typedef char pb; foo(pb); };
template<class T> foo<B, T*>::foo(pb) {}
template<template<class> class... Cs> struct P { typedef int q; P(q); };
template<template<class> class... Cs> P<Cs...>::P(q) {}
template<class T> struct Plain { typedef int t; Plain(t); };
template<class T> Plain<T>::Plain(t) {}
template struct foo<B, int>;
template struct P<B, B>;
template struct Plain<int>;
void use() { foo<B, int> f(1); foo<B, int> g(1.0, 2); foo<B, int*> h('c'); }
