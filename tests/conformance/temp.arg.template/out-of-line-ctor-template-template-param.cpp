// [class.ctor.general]/1, [class.qual]/2: an out-of-line constructor of a
// class template whose parameters include a template template parameter
// (`foo<C, T>::foo(bar)`, `bar` a member typedef of `C<T>`). Reduced from
// GCC's g++.old-deja/g++.oliva/template2.C; found running GCC's C++
// testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ xfail(kiln): WP-1495, 'foo<C, T>::foo(' with a template template parameter is not taken as a constructor declarator
template<typename T> struct B { typedef int bar; };
template<template<typename> class C, typename T> class foo {
  typedef typename C<T>::bar bar;
  foo(bar);
};
template<template<typename> class C, typename T> foo<C, T>::foo(bar) {}
template class foo<B, int>;
