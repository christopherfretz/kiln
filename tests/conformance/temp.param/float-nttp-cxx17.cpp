// [temp.param]/4 (C++17): a non-type template parameter has integral,
// enumeration, pointer, lvalue reference, pointer-to-member or
// std::nullptr_t type; floating-point and class types are allowed only from
// C++20 on. Found by the clang negative-test run (WP-912), fixed by WP-1017.
//@ std: c++17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:17 error "a non-type template parameter cannot have type 'float' before C++20"
//@ diag: 16:14 error "a non-type template parameter cannot have type 'FT' (aka 'double') before C++20"
//@ diag: 20:13 error "a non-type template parameter cannot have type 'L' before C++20"
//@ diag: 24:13 error "a non-type template parameter cannot have type 'N'"
//@ diag: 26:3 error "a non-type template parameter cannot have type 'double' before C++20"
//@ diag: 25:16 note "template parameter is declared here"
template <float F> struct S {};
using FT = double;
template <FT D> void f();
struct L {
  int i;
};
template <L l> struct T {};
struct N {
  N();
};
template <N n> struct U {};
template <auto A> struct V {};
V<1.0> v;

enum E { e };
template <int I, E X, int *P, int &R, int L::*M, decltype(nullptr) Z> struct OK {};
template <auto A> struct W {};
W<1> w;
