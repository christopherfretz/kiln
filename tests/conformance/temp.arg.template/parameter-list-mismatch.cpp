// [temp.arg.template]/3 (P0522): a template template argument must be at
// least as specialized as the parameter; otherwise "different template
// parameters" with clang's TemplateParameterListsAreEqual notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 19:1 note "too many template parameters in template template argument"
//@ diag: 21:10 note "previous template template parameter is here"
//@ diag: 24:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 20:14 note "template parameter has a different kind in template argument"
//@ diag: 21:24 note "previous template template parameter is here"
//@ diag: 26:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 18:16 note "template parameter has a different kind in template argument"
//@ diag: 25:22 note "previous template template parameter is here"
//@ diag: 29:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 20:14 note "template parameter has a different kind in template argument"
//@ diag: 27:27 note "previous template template parameter is here"
template<class T> struct A {};
template<class T, class U> struct B {};
template<int N> struct C {};
template<template<class> class P> struct Q {};
Q<A> qa;
Q<B> qb;
Q<C> qc;
template<template<int> class P> struct R {};
R<A> ra;
template<template<class...> class P> struct S {};
S<A> sa;
S<C> sc;
template<class... T> struct V {};
Q<V> qv;
