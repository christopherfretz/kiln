// [class.ctor.general]/1, [dcl.meaning.general]/2: `X<C, T>::X(bar *)` is
// still a constructor declarator, and it matches no constructor (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ diag: 8:55 error "out-of-line definition of 'foo<C, T>' does not match any declaration in 'foo<C, T>'"
template<class> struct B { typedef int bar; };
template<template<class> class C, class T> struct foo { typedef typename C<T>::bar bar; foo(bar); };
template<template<class> class C, class T> foo<C, T>::foo(bar *) {}
