// [temp.param]/1: clang 19 shape of template-template-param-missing-key.cpp.
// Clang 19 reports every missing `class`/`typename` after a template
// template parameter's template-head, then the same recovery: a token that
// cannot be the parameter's name is "expected identifier", the parameter is
// dropped, and a list left empty makes the class a non-template.
//@ requires: clang<22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 27:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 27:26 error "expected identifier"
//@ diag: 27:1 error "extraneous 'template<>' in declaration of struct 'A6'"
//@ diag: 29:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 29:26 error "expected identifier"
//@ diag: 29:1 error "extraneous 'template<>' in declaration of struct 'A7'"
//@ diag: 29:55 error "no template named 'T7'"
//@ diag: 30:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 30:26 error "expected identifier"
//@ diag: 30:1 error "extraneous 'template<>' in declaration of struct 'A10'"
//@ diag: 31:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 31:26 error "expected identifier"
//@ diag: 33:35 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 33:35 error "expected identifier"
//@ diag: 35:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
//@ diag: 36:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
template<class> struct E {};
template<template<class> int N6> struct A6 {};
A6 a6;
template<template<class> static class T7> struct A7 { T7<int> x; };
template<template<class> union T10> struct A10 {};
template<template<class> static class T11, class U> struct A11 { U u; };
A11<int> a11;
template<class T, template<class> static class T12> struct A12 { T t; };
A12<int> a12;
template<template<class> struct> struct A13 {};
template<template<class> = E> struct A14 {};
