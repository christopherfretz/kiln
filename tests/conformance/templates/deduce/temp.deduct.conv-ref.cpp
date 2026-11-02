// [temp.deduct.conv]/2-4: a reference conversion type deduces from the
// referred type; mismatches name the canonical form of P.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:5 error "no viable conversion from 'A' to 'int'"
//@ diag: 12:31 note "candidate template ignored: could not match 'W<type-parameter-0-0>' against 'int'"
//@ diag: 17:7 error "no viable conversion from 'C' to 'int'"
//@ diag: 16:31 note "candidate function [with T = int]"
//@ requires: clang<22
// clang 22 emits error: no viable conversion from 'A' to 'int'; WP-355 residual
template <class T> struct W {};
struct A { template <class T> operator W<T>() const; };
int x = A();
struct B { template <class T> operator T&() const; };
const int& y = B();
struct C { template <class T> operator T&() const; };
int&& z = C();
