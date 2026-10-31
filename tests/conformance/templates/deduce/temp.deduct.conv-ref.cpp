// [temp.deduct.conv]/2-4: a reference conversion type deduces from the
// referred type; mismatches name the canonical form of P.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:5 error "no viable conversion from 'A' to 'int'"
//@ diag: 10:31 note "candidate template ignored: could not match 'W<type-parameter-0-0>' against 'int'"
//@ diag: 15:7 error "no viable conversion from 'C' to 'int'"
//@ diag: 14:31 note "candidate function [with T = int]"
template <class T> struct W {};
struct A { template <class T> operator W<T>() const; };
int x = A();
struct B { template <class T> operator T&() const; };
const int& y = B();
struct C { template <class T> operator T&() const; };
int&& z = C();
