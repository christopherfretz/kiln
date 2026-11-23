// [temp.param]/12 with a friend function template definition that has a
// default template argument: kiln reports a redeclaration only where clang
// 19 and 22 do (the declaration at namespace scope before the class
// template's instantiation). Not reported, as in clang: a redeclaration
// after the instantiation, one of a non-template class's friend, and the
// friends of two specializations that are different templates. Invalid
// pair: friend-template-default-arg-redeclared.cpp (WP-2363).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template<class T> struct S { template<class U = int> friend int g(S, U) { return 1; } };
S<int> s;
template<class U> int g(S<int>, U);
struct A;
template<class U> int f(A, U);
struct A { template<class U = int> friend int f(A, U) { return 2; } };
template<class T> struct P { template<class U = int> friend int p(P, U) { return 3; } };
P<int> p1;
P<long> p2;
int k = g(s, 0) + f(A(), 0) + p(p1, 0) + p(p2, 0);
