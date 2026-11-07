// [dcl.contract.func]/7 (P2900R14), [temp.res.general]/6: a parameter of
// non-dependent type that a postcondition odr-uses and that is not const
// makes the template ill-formed on its definition, without an
// instantiation: a member of a class template, a function template, and a
// friend of a class template, also a redeclaration without a seq of a
// namespace function whose seq has such a postcondition. Dependent
// parameters wait for the instantiation. GCC 16.1 reports the member and
// friend definitions on the pattern too ([-Wtemplate-body]) but the
// friend redeclarations only on instantiation (and the definition `h` not
// at all); nothing is instantiated here.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 24:40 error "a value parameter used in a postcondition must be const"
//@ diag: 24:40 note "declared here"
//@ diag: 25:29 error "a value parameter used in a postcondition must be const"
//@ diag: 25:29 note "declared here"
//@ diag: 27:47 error "a value parameter used in a postcondition must be const"
//@ diag: 27:47 note "declared here"
//@ diag: 29:47 error "a value parameter used in a postcondition must be const"
//@ diag: 29:47 note "declared here"
//@ diag: 30:47 error "a value parameter used in a postcondition must be const"
//@ diag: 30:47 note "declared here"
template<class T> struct M { int n(int a) post(r: r == a); };
template<class T> int q(int a, T) post(r: r == a);
int f(const int a) post(r: r == a);
template<class T> struct S { friend int f(int a); };
int h(const int a) post(r: r == a);
template<class T> struct U { friend int h(int a) { return a; } };
template<class T> struct X { friend int n(int a) post(r: r == a); };
// Dependent: checked on instantiation.
template<class T> struct D { int m(T a) post(r: r == a); };
template<class T> int k(T a) post(r: r == a);
// Const, or a reference: no diagnostic.
template<class T> struct C { friend int f(const int a); int m(int& a) post(r: r == a); };
