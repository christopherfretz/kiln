// [temp.param]/12: a friend function template declaration that specifies a
// default template argument shall be a definition and the only declaration
// of the template. A class template's friend definition, instantiated after
// the template was declared at namespace scope, redeclares it: reported at
// the default, in the instantiation (as clang 19 and 22; GCC 16 accepts).
// Reduced from real code (WP-2363). Valid pair:
// friend-template-default-arg-redeclared-ok.cpp.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 15:45 error "default template argument not permitted on a friend template"
//@ diag: 17:8 note "in instantiation of template class 'S<int>' requested here"
//@ diag: 18:45 error "default template argument not permitted on a friend template"
//@ diag: 20:11 note "in instantiation of template class 'R<int>' requested here"
//@ strict-diags
template<class T> struct S { template<class U = int> friend void g(S, U) {} };
template<class U> void g(S<int>, U);
S<int> s;
template<class T> struct R { template<class U = int> friend int h(R, U) { return 0; } };
template<class U> int h(R<int>, U);
int k = h(R<int>(), 1);
