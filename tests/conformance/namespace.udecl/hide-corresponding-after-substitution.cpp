// [namespace.udecl]/11, [basic.scope.scope]/4: a member template hides a
// base member template named by a using-declarator when they correspond,
// and the trailing requires-clauses are compared after substituting the
// derived class's template arguments: `requires C<T>` of B<int> and
// `requires C<int>` of A correspond (in either direction, and when the base
// is a constrained partial specialization), so the call selects the
// derived member and never the deleted one. Constraints that still differ
// after substitution do not correspond: both members are candidates.
// Found running GCC's C++ testsuite. WP-1509.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:19 error "call to member function 'f' is ambiguous"
//@ diag: 43:26 note "candidate function [with U = int]"
//@ diag: 48:26 note "candidate function [with U = int]"
template <class T> concept C = true;
template <class T> concept D = sizeof(T) > 0;

struct A {
  template <class U> int f(U) requires C<int> = delete;
};
template <class T>
struct B : A {
  using A::f;
  template <class U> int f(U) requires C<T> { return 1; }
};
int b1 = B<int>().f(42);

template <class T> struct P;
template <class T>
struct P<T*> {
  template <class U> int f(U) requires C<T> = delete;
};
template <class T>
struct Q : P<T> {
  using P<T>::f;
  template <class U> int f(U) requires C<int> { return 2; }
};
int b2 = Q<int*>().f(42);

// A different concept: no correspondence, so neither hides the other and
// neither constraint subsumes the other.
struct E {
  template <class U> int f(U) requires C<int> { return 3; }
};
template <class T>
struct F : E {
  using E::f;
  template <class U> int f(U) requires D<T> { return 4; }
};
int b3 = F<int>().f(42);
