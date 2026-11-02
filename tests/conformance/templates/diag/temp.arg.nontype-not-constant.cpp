// [temp.arg.nontype]/1: a non-type template argument is a converted
// constant expression; a non-const variable's value is not usable in one
// ([expr.const]/5).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:3 error "non-type template argument is not a constant expression"
//@ diag: 14:3 note "read of non-const variable 'n' is not allowed in a constant expression"
//@ diag: 13:5 note "declared here"
//@ diag: 16:3 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ requires: clang<22
// clang 22 emits error: conversion from 'double' to 'int' is not allowed in a converted constant ex...; WP-355 residual
template <int N> struct V {};
int n = 3;
V<n> v;
template <int M> struct W {};
W<1.5> w;
