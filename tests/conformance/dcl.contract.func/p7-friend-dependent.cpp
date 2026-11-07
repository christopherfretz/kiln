// [dcl.contract.func]/7 (P2900R14): "If the predicate of a postcondition
// assertion of a function f odr-uses a non-reference parameter of f, that
// parameter and the corresponding parameter on all declarations of f shall
// have const type." A friend of a class template whose parameter type is
// dependent is a declaration of the namespace function once its class is
// instantiated ([temp.inst]/3), so the rule is checked on each
// instantiation: S<int> and D<int> (a friend definition) are ill-formed,
// as is K<int, const int>; C<const int>, the second parameter of K and a
// reference parameter are not. GCC 16.1 agrees.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 29:45 error "a value parameter used in a postcondition must be const"
//@ diag: 35:8 note "in instantiation of template class 'S<int>' requested here"
//@ diag: 29:45 note "declared here"
//@ diag: 30:45 error "a value parameter used in a postcondition must be const"
//@ diag: 36:8 note "in instantiation of template class 'D<int>' requested here"
//@ diag: 30:45 note "declared here"
//@ diag: 31:54 error "a value parameter used in a postcondition must be const"
//@ diag: 38:19 note "in instantiation of template class 'K<int, const int>' requested here"
//@ diag: 31:54 note "declared here"

int m(const int a) post(r: r == a);
int h(const int a) post(r: r == a);
int k(const int a, int b) post(r: r == a);
int q(int& a) post(r: r == a);

template<class T> struct S { friend int m(T a); };
template<class T> struct D { friend int h(T a) { return a; } };
template<class T, class U> struct K { friend int k(T a, U b); };
template<class T> struct C { friend int m(T a); };
template<class T> struct R { friend int q(T a); };

S<int> s1;
D<int> d1;
K<const int, int> k1;
K<int, const int> k2;
C<const int> c1;
R<int&> r1;
