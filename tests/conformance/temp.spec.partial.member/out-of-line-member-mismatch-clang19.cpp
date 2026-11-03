// [temp.spec.partial.member]/1: clang 19 prints the partial
// specialization with its canonical arguments (clang 22:
// out-of-line-member-mismatch.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 16:36 error "out-of-line definition of 'f' does not match any declaration in 'P<type-parameter-0-0, int>'"
//@ diag: 17:36 error "return type of out-of-line definition of 'P<type-parameter-0-0, int>::g' differs from that in the declaration"
//@ diag: 15:53 note "previous declaration is here"
//@ diag: 18:36 error "redefinition of 's' with a different type: 'char' vs 'int'"
//@ diag: 15:72 note "previous declaration is here"
//@ diag: 19:36 error "out-of-line definition of 'h' does not match any declaration in 'P<type-parameter-0-0, int>'"
//@ diag: 20:37 error "nested name specifier 'P<T *, int>::' for declaration does not refer into a class, class template or class template partial specialization"
template <class T, class U> struct P { void f(); int g(int); static int s; };
template <class T> struct P<T, int> { void f(); int g(int); static int s; };
template <class T> void P<T, int>::f(long) {}
template <class T> long P<T, int>::g(int) { return 0; }
template <class T> char P<T, int>::s = 0;
template <class T> void P<T, int>::h() {}
template <class T> void P<T*, int>::f() {}
