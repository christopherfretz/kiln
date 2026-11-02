// [temp.param]/1 (C++14): before C++17 only `class` may follow a template
// template parameter's template-head; the missing-key error names only it.
//@ expect: compile-fail
//@ args: -std=c++14
//@ strict-diags
//@ diag: 9:26 error "template template parameter requires 'class' after the parameter list"
//@ diag: 10:26 warning "template template parameter using 'typename' is a C++17 extension"
template<class> struct E {};
template<template<class> T1> struct A1 {};
template<template<class> typename T2> struct A2 {};
