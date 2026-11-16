// [temp.param]/12: a default template argument may not be given in a
// template head of the definition of a member of a class template outside
// its class: neither in the heads matched to the class template nor in a
// member template's own; clang's error is at the parameter. Found running
// GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:19 error "cannot add a default template argument to the definition of a member of a class template"
//@ diag: 16:37 error "cannot add a default template argument to the definition of a member of a class template"
//@ diag: 17:19 error "cannot add a default template argument to the definition of a member of a class template"
//@ diag: 18:37 error "cannot add a default template argument to the definition of a member of a class template"
//@ diag: 19:19 error "cannot add a default template argument to the definition of a member of a class template"
template<typename T> struct A { struct B; template<class U> struct C; void f(); template<class U> void g(); static int v; };
template<typename T = int> struct A<T>::B { int i; };
template<typename T> template<class U = int> struct A<T>::C { };
template<typename T = int> void A<T>::f() {}
template<typename T> template<class U = int> void A<T>::g() {}
template<typename T = int> int A<T>::v = 0;
