// [namespace.udecl]/14: a member hides the base member a
// using-declarator names only when their trailing requires-clauses
// correspond. Here they name the class's own parameter, bound to `int`
// in B<int> but to `int *` in the base A<int *>, so both members are
// visible and the calls are ambiguous.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:5 error "call to member function 'f' is ambiguous"
//@ diag: 18:26 note "candidate function [with U = int]"
//@ diag: 24:26 note "candidate function [with U = int]"
//@ diag: 30:5 error "call to member function 'g' is ambiguous"
//@ diag: 19:7 note "candidate function"
//@ diag: 25:7 note "candidate function"
template <class T> concept Any = true;
template <class T> struct A {
  template <class U> int f(U) requires Any<T> { return 1; }
  int g() requires Any<T> { return 1; }
};
template <class T> struct B : A<T *> {
  using A<T *>::f;
  using A<T *>::g;
  template <class U> int f(U) requires Any<T> { return 2; }
  int g() requires Any<T> { return 2; }
};
int main() {
  B<int> b;
  b.f(42);
  b.g();
}
