// [temp.deduct.type]/2, /8: a non-deduced mismatch notes the innermost mismatching pair of
// types (canonical below the top level, template names for a template mismatch, the elided
// template diff for two specializations of one template)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:3 error "no matching function for call to 'f'"
//@ diag: 29:24 note "candidate template ignored: could not match 'vec' against 'pr'"
//@ diag: 40:3 error "no matching function for call to 'g'"
//@ diag: 30:24 note "candidate template ignored: could not match 'pr' against 'D'"
//@ diag: 41:3 error "no matching function for call to 'h'"
//@ diag: 31:24 note "candidate template ignored: could not match 'pr' against 'D'"
//@ diag: 42:14 error "no matching function for call to 'k'"
//@ diag: 32:24 note "candidate template ignored: could not match 'pr' against 'D'"
//@ diag: 43:3 error "no matching function for call to 'm'"
//@ diag: 33:24 note "candidate template ignored: could not match 'int' against 'char'"
//@ diag: 44:3 error "no matching function for call to 'm'"
//@ diag: 33:24 note "candidate template ignored: could not match 'pr<type-parameter-0-0, int>' against 'int'"
//@ diag: 45:3 error "no matching function for call to 'g'"
//@ diag: 30:24 note "candidate template ignored: could not match 'pr<T, int>' against 'S'"
//@ diag: 46:3 error "no matching function for call to 'n'"
//@ diag: 34:40 note "candidate template ignored: could not match 'int' against 'char'"
//@ diag: 47:3 error "no matching function for call to 'o'"
//@ diag: 35:24 note "candidate template ignored: could not match 'pr<[...], char>' against 'pr<[...], int>'"
//@ diag: 48:3 error "no matching function for call to 'p'"
//@ diag: 36:24 note "candidate template ignored: could not match 'T *' against 'S'"
template<class A, class B> struct pr {};
template<class A> struct vec {};
template<class A> struct D : pr<A, char> {};
template<class T> void f(vec<T>);
template<class T> void g(pr<T, int>);
template<class T> void h(pr<T, int>*);
template<class T> void k(pr<T, int>&);
template<class T> void m(vec<pr<T, int>>);
template<template<class> class X> void n(X<int>);
template<class T> void o(pr<T, pr<int, char>>);
template<class T> void p(T*);
struct S {};
void t() {
  f(pr<int, char>());
  g(D<int>());
  h((D<int>*)0);
  D<int> dd; k(dd);
  m(vec<pr<int, char>>());
  m(vec<int>());
  g(S());
  n(vec<char>());
  o(pr<int, pr<int, int>>());
  p(S());
}
