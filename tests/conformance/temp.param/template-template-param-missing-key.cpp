// [temp.param]/1: a template template parameter's template-head is
// followed by `class` or `typename`. Clang 22 reports a missing key only
// where it was plausibly forgotten (before a name, `,`, `>`, `>>` or
// `...`, or at `struct`, which it consumes); anything else that cannot be
// the parameter's name is "expected identifier" and the parameter is
// dropped (the list goes on after a `,`). A list left empty makes the
// class a non-template: "extraneous 'template<>' in declaration of struct".
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 23:26 error "expected identifier"
//@ diag: 23:1 error "extraneous 'template<>' in declaration of struct 'A6'"
//@ diag: 25:26 error "expected identifier"
//@ diag: 25:1 error "extraneous 'template<>' in declaration of struct 'A7'"
//@ diag: 25:55 error "no template named 'T7'"
//@ diag: 26:26 error "expected identifier"
//@ diag: 26:1 error "extraneous 'template<>' in declaration of struct 'A10'"
//@ diag: 27:26 error "expected identifier"
//@ diag: 29:35 error "expected identifier"
//@ diag: 31:26 error "template template parameter requires 'class' or 'typename' after the parameter list"
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
