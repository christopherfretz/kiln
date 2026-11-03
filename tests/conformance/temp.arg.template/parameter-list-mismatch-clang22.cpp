// [temp.arg.template]/3 (P0522): a template template argument must be at
// least as specialized as the parameter; otherwise "different template
// parameters" with clang's TemplateParameterListsAreEqual notes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:3 error "too few template arguments for class template 'B'"
//@ diag: 29:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 27:32 note "previous template template parameter is here"
//@ diag: 25:35 note "template is declared here"
//@ diag: 27:24 error "template argument for non-type template parameter must be an expression"
//@ diag: 30:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 27:32 note "previous template template parameter is here"
//@ diag: 26:14 note "template parameter is declared here"
//@ diag: 31:22 error "template argument for template type parameter must be a type"
//@ diag: 32:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 31:30 note "previous template template parameter is here"
//@ diag: 24:16 note "template parameter is declared here"
//@ diag: 33:27 error "template argument for non-type template parameter must be an expression"
//@ diag: 35:3 note "template template argument has different template parameters than its corresponding template template parameter"
//@ diag: 33:35 note "previous template template parameter is here"
//@ diag: 26:14 note "template parameter is declared here"
//@ requires: clang>=22
// clang 22 spelling of parameter-list-mismatch.cpp (WP-394).
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
