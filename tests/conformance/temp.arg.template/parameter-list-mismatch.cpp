// [temp.arg.template]/3 (P0522): a template template argument must be at
// least as specialized as the parameter; otherwise "different template
// parameters" with clang's TemplateParameterListsAreEqual notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 21:1 note "too many template parameters in template template argument"
//@ diag: 23:10 note "previous template template parameter is here"
//@ diag: 26:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 22:14 note "template parameter has a different kind in template argument"
//@ diag: 23:24 note "previous template template parameter is here"
//@ diag: 28:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 20:16 note "template parameter has a different kind in template argument"
//@ diag: 27:22 note "previous template template parameter is here"
//@ diag: 31:3 error "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 22:14 note "template parameter has a different kind in template argument"
//@ diag: 29:27 note "previous template template parameter is here"
//@ requires: clang<22
// clang 19 spelling; parameter-list-mismatch-clang22.cpp has clang 22's (WP-394).
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
