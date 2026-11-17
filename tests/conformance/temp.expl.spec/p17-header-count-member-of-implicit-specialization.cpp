// [temp.expl.spec]/17: in an explicit specialization of a member of an
// implicitly instantiated class template specialization, each enclosing
// specialization takes one `template<>`; the declaration's own
// `template<>` makes it a function template specialization. One header
// too few names a member that is not a template (a member template does
// not match it, and a template-id needs its header); one too many makes
// a non-template member a function template specialization that no
// template matches.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -std=c++20
//@ diag: 27:26 error "out-of-line declaration of 'f' does not match any declaration in 'A<int>'"
//@ diag: 21:27 note "A defined here"
//@ diag: 28:26 error "template specialization requires 'template<>'"
//@ diag: 29:38 error "no function template matches function template specialization 'f2'"
//@ diag: 30:39 error "no function template matches function template specialization 'f2'"
//@ diag: 31:28 error "out-of-line definition of 'f' does not match any declaration in 'A<short>'"
//@ diag: 21:27 note "A defined here"
template <class T> struct A {
  template <class U> void f(U);
  void f2(int);
  template <class U> void h(U);
  void h(long);
};
template <> void A<int>::f(int);
template <> void A<int>::f<int>(int);
template <> template <> void A<int>::f2(int);
template <> template <> void A<long>::f2(int) {}
template <> void A<short>::f(int) {}
