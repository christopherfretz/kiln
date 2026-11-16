// [dcl.fct.default]/4 with [temp.expl.spec]: an explicit specialization of a
// function template, or of a member function (template) of an implicitly
// instantiated class template specialization, may not add a default
// argument; clang's error is at each such parameter, also on a
// redeclaration of the specialization. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:27 error "default argument not permitted on an explicit specialization of function 'foo'"
//@ diag: 17:31 error "default argument not permitted on an explicit specialization of function 'f'"
//@ diag: 18:47 error "default argument not permitted on an explicit specialization of function 'g'"
//@ diag: 20:34 error "default argument not permitted on an explicit specialization of function 'h'"
//@ diag: 22:36 error "default argument not permitted on an explicit specialization of function 'h'"
template<int N> int foo(int a) { return a; }
template<> int foo<0>(int a = 123) { return a + 1; }
template<class T> struct S { void f(int); template<class U> void g(U); };
template<> void S<int>::f(int x = 1) {}
template<> template<> void S<int>::g<int>(int x = 1) {}
template <class T> void h(T, int);
template <> void h<int>(int, int k = 2);
template <> void h<long>(long, int);
template <> void h<long>(long, int k = 3) {}
