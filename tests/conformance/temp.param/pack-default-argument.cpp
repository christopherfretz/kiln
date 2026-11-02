// [temp.param]/14: a template parameter pack (type, non-type or
// template template) shall not have a default argument. clang reports it
// at the `=`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:21 error "template parameter pack cannot have a default argument"
//@ diag: 11:19 error "template parameter pack cannot have a default argument"
//@ diag: 12:38 error "template parameter pack cannot have a default argument"
//@ diag: 13:20 error "template parameter pack cannot have a default argument"
template<class... T = int> struct A;
template<int... N = 1> struct B;
template<template<class> class... TT = A> struct C;
template<auto... V = 0> void f();
